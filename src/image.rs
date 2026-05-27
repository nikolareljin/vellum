use anyhow::Result;
use image::DynamicImage;
use std::collections::HashMap;
use std::path::Path;

/// Load an image from a local file path.
pub fn load_image<P: AsRef<Path>>(path: P) -> Result<DynamicImage> {
    let img = image::open(path.as_ref())?;
    Ok(img)
}

/// In-memory image cache: src string → loaded DynamicImage.
#[derive(Default)]
pub struct ImageCache {
    cache: HashMap<String, DynamicImage>,
}

impl ImageCache {
    pub fn get_or_load(&mut self, src: &str) -> Result<&DynamicImage> {
        if !self.cache.contains_key(src) {
            // Remote images not yet supported (Phase 3+)
            if src.starts_with("http://") || src.starts_with("https://") {
                anyhow::bail!("remote images not yet supported — use a local path");
            }
            let img = load_image(src)?;
            self.cache.insert(src.to_owned(), img);
        }
        Ok(self.cache.get(src).unwrap())
    }
}
