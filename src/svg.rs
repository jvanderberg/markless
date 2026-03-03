//! Shared SVG rasterization.
//!
//! Provides a single `rasterize_svg()` function used by both
//! [`crate::math`] and [`crate::mermaid`] to convert SVG strings to
//! raster images. The `resvg` font database is loaded once and cached
//! for the lifetime of the process.

use std::sync::{Arc, OnceLock};

use anyhow::Result;
use image::DynamicImage;
use resvg::usvg::fontdb;

/// Return the cached `resvg` font database, loading system fonts once.
fn cached_fontdb() -> &'static Arc<fontdb::Database> {
    static DB: OnceLock<Arc<fontdb::Database>> = OnceLock::new();
    DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        Arc::new(db)
    })
}

/// Rasterize an SVG string to a [`DynamicImage`].
///
/// Scales the SVG so its width matches `target_width_px`, preserving
/// aspect ratio. The font database is cached so repeated calls don't
/// re-scan system fonts.
///
/// # Errors
///
/// Returns an error if the SVG cannot be parsed or the resulting
/// pixmap dimensions are zero.
pub fn rasterize_svg(svg: &str, target_width_px: u32) -> Result<DynamicImage> {
    let opts = resvg::usvg::Options {
        fontdb: Arc::clone(cached_fontdb()),
        ..Default::default()
    };

    let tree = resvg::usvg::Tree::from_str(svg, &opts)?;
    let size = tree.size();

    let scale = f64::from(target_width_px) / f64::from(size.width());

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (f64::from(size.width()) * scale).ceil() as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (f64::from(size.height()) * scale).ceil() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap {width}x{height}"))?;

    #[allow(clippy::cast_possible_truncation)]
    let scale_f32 = scale as f32;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale_f32, scale_f32),
        &mut pixmap.as_mut(),
    );

    let rgba = pixmap.data().to_vec();
    let img_buf = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow::anyhow!("failed to create image from pixmap data"))?;

    Ok(DynamicImage::ImageRgba8(img_buf))
}
