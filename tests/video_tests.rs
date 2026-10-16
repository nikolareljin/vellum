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

/// Real ffmpeg round trip: the `file:` input form reads a local path that
/// contains a colon, which ffmpeg would otherwise parse as a protocol.
#[test]
fn test_extract_thumbnail_reads_local_path_with_colon() {
    let ffmpeg_ok = std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    if !ffmpeg_ok {
        eprintln!("ffmpeg not installed; skipping");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let video = dir.path().join("http:clip.mp4");
    let made = std::process::Command::new("ffmpeg")
        .args(["-y", "-f", "lavfi", "-i", "color=c=red:s=32x32:d=1", "-pix_fmt", "yuv420p"])
        .arg(format!("file:{}", video.display()))
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(made.success());
    let thumb = vellum::video::extract_thumbnail(&video).expect("thumbnail");
    assert!(std::fs::metadata(thumb.path()).unwrap().len() > 0);
}
