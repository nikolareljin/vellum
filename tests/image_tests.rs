use std::sync::Mutex;
use vellum::image::{is_remote_url, load_image, safe_error_url, ImageCache};

// Serialize tests that mutate VELLUM_NO_REMOTE_IMAGES so they don't race.
static ENV_MUTEX: Mutex<()> = Mutex::new(());

/// RAII guard: restores `key` to `prev` on drop, even if the test panics.
struct EnvRestore {
    key: &'static str,
    prev: Option<std::ffi::OsString>,
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        match self.prev.take() {
            Some(v) => std::env::set_var(self.key, v),
            None => std::env::remove_var(self.key),
        }
    }
}

#[test]
fn test_load_png_succeeds() {
    let path = "/tmp/vellum_test.png";
    image::RgbImage::from_pixel(2, 2, image::Rgb([255u8, 255, 255]))
        .save(path)
        .unwrap();
    let img = load_image(path).unwrap();
    assert_eq!(img.width(), 2);
    assert_eq!(img.height(), 2);
}

#[test]
fn test_load_missing_file_errors() {
    let result = load_image("/tmp/vellum_definitely_missing_xyz.png");
    assert!(result.is_err());
}

#[test]
fn test_load_svg_via_load_image() {
    // write a minimal SVG to a temp file and load it via the unified load_image path
    let path = "/tmp/vellum_test.svg";
    std::fs::write(
        path,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="8" height="8">
          <rect width="8" height="8" fill="green"/>
        </svg>"#,
    )
    .unwrap();
    let img = load_image(path).expect("SVG should load via load_image");
    assert_eq!(img.width(), 8);
    assert_eq!(img.height(), 8);
}

#[test]
fn test_safe_error_url() {
    // strips query and fragment
    assert_eq!(
        safe_error_url("https://example.com/img.png?token=secret#frag"),
        "https://example.com/img.png"
    );
    // strips userinfo (credentials)
    assert_eq!(
        safe_error_url("https://user:pass@example.com/img.png"),
        "https://example.com/img.png"
    );
    // strips both
    assert_eq!(
        safe_error_url("https://user:pass@example.com/img.png?tok=x#y"),
        "https://example.com/img.png"
    );
    // plain URL unchanged
    assert_eq!(
        safe_error_url("https://example.com/img.png"),
        "https://example.com/img.png"
    );
    // local path unchanged
    assert_eq!(safe_error_url("/local/path/img.png"), "/local/path/img.png");
}

#[test]
fn test_is_remote_url() {
    assert!(is_remote_url("http://example.com/img.png"));
    assert!(is_remote_url("https://example.com/img.png"));
    // RFC 3986 §3.1: schemes are case-insensitive
    assert!(is_remote_url("HTTP://example.com/img.png"));
    assert!(is_remote_url("HTTPS://example.com/img.png"));
    assert!(is_remote_url("Http://example.com/img.png"));
    assert!(!is_remote_url("/local/path/img.png"));
    assert!(!is_remote_url("relative/img.png"));
    assert!(!is_remote_url("ftp://example.com/img.png"));
    assert!(!is_remote_url(""));
    // Edge case: bare scheme prefix with no host — still matches (valid prefix)
    assert!(is_remote_url("http://"));
    assert!(is_remote_url("https://"));
    // Shorter than any scheme → never remote
    assert!(!is_remote_url("http:/"));
    assert!(!is_remote_url("http:"));
}

#[test]
fn test_no_remote_images_env_blocks_fetch() {
    // VELLUM_NO_REMOTE_IMAGES set → get_or_load must refuse http(s) URLs without
    // making any network connection.
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    let _restore = EnvRestore {
        key: "VELLUM_NO_REMOTE_IMAGES",
        prev: std::env::var_os("VELLUM_NO_REMOTE_IMAGES"),
    };
    std::env::set_var("VELLUM_NO_REMOTE_IMAGES", "1");
    let mut cache = ImageCache::default();
    let result = cache.get_or_load("https://example.com/image.png");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("remote images are off"),
        "expected the opt-out message, got: {msg}"
    );
}

// ── Remote images and SVG references ─────────────────────────────────────────

/// The opt-in is decided once per process; this test owns that decision.
#[test]
fn remote_images_are_off_by_default() {
    use vellum::image::{load_image_url, remote_images_enabled, set_remote_images};
    let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
    if std::env::var_os("VELLUM_REMOTE_IMAGES").is_some() {
        eprintln!("VELLUM_REMOTE_IMAGES set in the environment; skipping");
        return;
    }
    assert!(
        !remote_images_enabled(),
        "remote images must be off by default"
    );
    let err = load_image_url("https://example.com/a.png").unwrap_err();
    assert!(err.to_string().contains("remote images are off"), "{err}");

    // The first decision sticks for the session; a document cannot flip it.
    set_remote_images(true);
    assert!(
        !remote_images_enabled(),
        "first decision sticks for the session"
    );
}

#[test]
fn svg_cannot_embed_local_files() {
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("secret.png");
    image::RgbImage::from_pixel(4, 4, image::Rgb([255u8, 0, 0]))
        .save(&png)
        .unwrap();
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="4" height="4"><image width="4" height="4" xlink:href="{}"/></svg>"#,
        png.display()
    );
    let img = vellum::svg::rasterize(svg.as_bytes()).unwrap().to_rgba8();
    assert_eq!(
        img.get_pixel(1, 1)[3],
        0,
        "local file was drawn into the SVG"
    );
}
