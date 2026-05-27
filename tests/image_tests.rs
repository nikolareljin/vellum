use vellum::image::load_image;

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
