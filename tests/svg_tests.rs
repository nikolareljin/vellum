use vellum::svg;

// Minimal valid SVG — 10×10 red rectangle
const SIMPLE_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10">
  <rect width="10" height="10" fill="red"/>
</svg>"#;

// SVG with no explicit width/height (uses viewBox only)
const VIEWBOX_ONLY_SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 50">
  <circle cx="50" cy="25" r="20" fill="blue"/>
</svg>"#;

#[test]
fn test_rasterize_simple_svg_produces_correct_dimensions() {
    let img = svg::rasterize(SIMPLE_SVG).expect("simple SVG should rasterise");
    assert_eq!(img.width(), 10);
    assert_eq!(img.height(), 10);
}

#[test]
fn test_rasterize_viewbox_svg_succeeds() {
    let img = svg::rasterize(VIEWBOX_ONLY_SVG).expect("viewBox-only SVG should rasterise");
    assert!(img.width() > 0);
    assert!(img.height() > 0);
}

#[test]
fn test_rasterize_empty_bytes_errors() {
    assert!(svg::rasterize(b"").is_err(), "empty data should fail");
}

#[test]
fn test_rasterize_garbage_errors() {
    assert!(
        svg::rasterize(b"not svg data at all").is_err(),
        "garbage data should fail"
    );
}

#[test]
fn test_is_svg_path_detects_svg_extension() {
    assert!(svg::is_svg_path("diagram.svg"));
    assert!(svg::is_svg_path("DIAGRAM.SVG"));
    assert!(svg::is_svg_path("path/to/icon.svg"));
    assert!(svg::is_svg_path("archive.svgz"));
}

#[test]
fn test_is_svg_path_rejects_non_svg() {
    assert!(!svg::is_svg_path("photo.png"));
    assert!(!svg::is_svg_path("image.jpeg"));
    assert!(!svg::is_svg_path("video.mp4"));
    assert!(!svg::is_svg_path("document.md"));
    assert!(!svg::is_svg_path("noextension"));
}
