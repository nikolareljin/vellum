use vellum::video::extract_thumbnail;

#[test]
fn test_extract_thumbnail_missing_file_errors() {
    let result = extract_thumbnail("/tmp/vellum_definitely_missing.mp4");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(msg.contains("not found") || msg.contains("No such"), "got: {msg}");
}

#[test]
fn test_is_video_src_recognises_extensions() {
    use vellum::video::is_video_src;
    assert!(is_video_src("demo.mp4"));
    assert!(is_video_src("clip.WEBM"));
    assert!(is_video_src("file.mov"));
    assert!(!is_video_src("image.png"));
    assert!(!is_video_src("doc.md"));
}
