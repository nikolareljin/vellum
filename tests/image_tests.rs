use std::sync::Mutex;
use vellum::image::{is_remote_url, load_image, ImageCache};

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
        msg.contains("VELLUM_NO_REMOTE_IMAGES"),
        "expected env-var name in error, got: {msg}"
    );
}
