use vellum::image::{load_image, ImageCache};

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
fn test_no_remote_images_env_blocks_fetch() {
    // VELLUM_NO_REMOTE_IMAGES set → get_or_load must refuse http(s) URLs without
    // making any network connection.
    std::env::set_var("VELLUM_NO_REMOTE_IMAGES", "1");
    let mut cache = ImageCache::default();
    let result = cache.get_or_load("https://example.com/image.png");
    std::env::remove_var("VELLUM_NO_REMOTE_IMAGES");
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("VELLUM_NO_REMOTE_IMAGES"),
        "expected env-var name in error, got: {msg}"
    );
}
