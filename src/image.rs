use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;

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

/// Fetch and decode a remote image over HTTP or HTTPS.
pub fn load_image_url(url: &str) -> Result<DynamicImage> {
    let mut buf = Vec::new();
    ureq::get(url)
        .call()
        .map_err(|e| anyhow::anyhow!("fetch {url}: {e}"))?
        .into_reader()
        .read_to_end(&mut buf)?;
    if svg::is_svg_path(url) {
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
