use vellum::video::extract_thumbnail;

#[test]
fn test_extract_thumbnail_missing_file_errors() {
    let result = extract_thumbnail("/tmp/vellum_definitely_missing.mp4");
    assert!(result.is_err());
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("not found") || msg.contains("No such"),
        "got: {msg}"
    );
}
