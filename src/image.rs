use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

use crate::svg;

/// Load an image from a local file path.
/// SVG/SVGZ files are rasterised via `resvg`; all other formats go through
/// the `image` crate's built-in decoders.
pub fn load_image<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let path_ref = path.as_ref();
    if svg::is_svg_path(&path_ref.to_string_lossy()) {
        let data = std::fs::read(path_ref)?;
        return svg::rasterize(&data);
    }
    let img = image::open(path_ref)?;
    Ok(img)
}

/// Maximum bytes accepted from a remote image response (20 MiB).
const MAX_REMOTE_BYTES: u64 = 20 * 1024 * 1024;
/// Maximum pixel count before decode is rejected (~50 MP, ~200 MiB at 32bpp).
const MAX_REMOTE_PIXELS: u64 = 50_000_000;

/// Fetch and decode a remote image over HTTP or HTTPS.
pub fn load_image_url(url: &str) -> Result<DynamicImage> {
    if !url.starts_with("http://") && !url.starts_with("https://") {
        anyhow::bail!("load_image_url: expected http:// or https:// URL, got: {url}");
    }
    if std::env::var_os("VELLUM_NO_REMOTE_IMAGES").is_some() {
        anyhow::bail!("remote image blocked (VELLUM_NO_REMOTE_IMAGES is set)");
    }
    // Strip query string and fragment for error messages to avoid leaking tokens.
    let safe_url = url
        .split('?')
        .next()
        .and_then(|s| s.split('#').next())
        .unwrap_or(url);
    let mut buf = Vec::new();
    ureq::get(url)
        .timeout(Duration::from_secs(10))
        .call()
        .map_err(|e| anyhow::anyhow!("fetch {safe_url}: {e}"))?
        .into_reader()
        .take(MAX_REMOTE_BYTES + 1)
        .read_to_end(&mut buf)?;
    if buf.len() as u64 > MAX_REMOTE_BYTES {
        anyhow::bail!(
            "remote image too large (>{} MiB)",
            MAX_REMOTE_BYTES / 1_048_576
        );
    }
    // Guard against decompression-bomb images: probe dimensions before
    // allocating the full decoded buffer.
    if let Ok(sz) = imagesize::blob_size(&buf) {
        let pixels = (sz.width as u64).saturating_mul(sz.height as u64);
        if pixels > MAX_REMOTE_PIXELS {
            anyhow::bail!(
                "remote image dimensions too large ({} × {} px)",
                sz.width,
                sz.height
            );
        }
    }
    if svg::is_svg_path(safe_url) {
        return svg::rasterize(&buf);
    }
    Ok(image::load_from_memory(&buf)?)
}

/// In-memory image cache: resolved src path → loaded [`DynamicImage`].
#[derive(Default)]
pub struct ImageCache {
    cache: HashMap<String, DynamicImage>,
}

impl ImageCache {
    pub fn get_or_load(&mut self, src: &str) -> Result<&DynamicImage> {
        if !self.cache.contains_key(src) {
            let img = if src.starts_with("http://") || src.starts_with("https://") {
                load_image_url(src)?
            } else {
                load_image(src)?
            };
            self.cache.insert(src.to_owned(), img);
        }
        Ok(self.cache.get(src).unwrap())
    }
}
