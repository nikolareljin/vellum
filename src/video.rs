use anyhow::{bail, Result};
use std::path::Path;
use tempfile::NamedTempFile;

/// Extract the first frame of a video file as a PNG using ffmpeg.
/// Returns a `NamedTempFile` (caller must keep it alive — drop = delete).
pub fn extract_thumbnail<P: AsRef<Path>>(video_path: P) -> Result<NamedTempFile> {
    let video_path = video_path.as_ref();
    if !video_path.exists() {
        bail!("video file not found: {}", video_path.display());
    }

    let tmp = tempfile::Builder::new()
        .prefix("vellum_thumb_")
        .suffix(".png")
        .tempfile()?;

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            &video_path.to_string_lossy(),
            "-vframes",
            "1",
            "-q:v",
            "2",
            tmp.path().to_string_lossy().as_ref(),
        ])
        .stderr(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .status()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                anyhow::anyhow!(
                    "ffmpeg not found — install ffmpeg to enable video thumbnails"
                )
            } else {
                anyhow::anyhow!("ffmpeg failed: {}", e)
            }
        })?;

    if !status.success() {
        bail!("ffmpeg exited with {}", status);
    }

    Ok(tmp)
}
