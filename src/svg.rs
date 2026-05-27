use anyhow::Result;
use image::{DynamicImage, RgbaImage};

/// Maximum dimension (width or height) at which an SVG is rasterised.
/// Larger SVGs are downscaled proportionally to fit within this box.
const MAX_DIM: f32 = 2048.0;

/// Rasterise an SVG byte stream to a [`DynamicImage`].
///
/// Uses the SVG's intrinsic viewport size, capped at `MAX_DIM` in each
/// dimension so huge graphics don't OOM the process.  System fonts are
/// loaded so SVGs containing text render correctly.
pub fn rasterize(data: &[u8]) -> Result<DynamicImage> {
    let mut opt = resvg::usvg::Options::default();
    // Load system fonts for SVGs that include <text> elements
    opt.fontdb_mut().load_system_fonts();

    let tree = resvg::usvg::Tree::from_data(data, &opt)
        .map_err(|e| anyhow::anyhow!("SVG parse error: {e}"))?;

    let svg_size = tree.size();
    let (w_f, h_f) = (svg_size.width(), svg_size.height());

    // Scale down proportionally if either dimension exceeds MAX_DIM
    let scale = (MAX_DIM / w_f).min(MAX_DIM / h_f).min(1.0_f32);
    let w = ((w_f * scale).round() as u32).max(1);
    let h = ((h_f * scale).round() as u32).max(1);

    let mut pixmap = resvg::tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| anyhow::anyhow!("SVG: failed to allocate {w}×{h} pixmap"))?;

    let transform = if scale < 1.0 {
        resvg::tiny_skia::Transform::from_scale(scale, scale)
    } else {
        resvg::tiny_skia::Transform::default()
    };

    resvg::render(&tree, transform, &mut pixmap.as_mut());

    let rgba = RgbaImage::from_raw(w, h, pixmap.take())
        .ok_or_else(|| anyhow::anyhow!("SVG: pixmap → RgbaImage conversion failed"))?;

    Ok(DynamicImage::ImageRgba8(rgba))
}

/// Returns `true` when the path looks like an SVG file.
pub fn is_svg_path(src: &str) -> bool {
    let lower = src.to_lowercase();
    lower.ends_with(".svg") || lower.ends_with(".svgz")
}
