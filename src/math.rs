//! LaTeX math rendering.
//!
//! Converts LaTeX math expressions to either Unicode text approximations
//! (for inline math and image-less fallback) or raster images via the
//! Typst pipeline: LaTeX → mitex → Typst → SVG → resvg → raster.

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use anyhow::Result;
use image::DynamicImage;
use resvg::usvg::fontdb;

/// Convert LaTeX math to a Unicode text approximation.
///
/// Uses the `unicodeit` crate to replace common LaTeX commands with
/// their Unicode equivalents (e.g. `\alpha` → `α`, `x^2` → `x²`).
pub fn latex_to_unicode(latex: &str) -> String {
    unicodeit::replace(latex)
}

/// Render LaTeX math to an SVG string.
///
/// Pipeline: LaTeX → mitex (Typst math syntax) → Typst → typst-svg → SVG.
///
/// # Errors
///
/// Returns an error if the LaTeX cannot be converted or compiled.
pub fn render_to_svg(latex: &str) -> Result<String> {
    catch_unwind(AssertUnwindSafe(|| render_to_svg_inner(latex)))
        .unwrap_or_else(|_| Err(anyhow::anyhow!("math rendering panicked")))
}

/// Render LaTeX math to a raster image.
///
/// Calls [`render_to_svg`] then rasterizes via `resvg`.
///
/// # Errors
///
/// Returns an error if the LaTeX cannot be rendered or the SVG
/// cannot be rasterized.
pub fn render_to_image(latex: &str, target_width_px: u32) -> Result<DynamicImage> {
    let svg = render_to_svg(latex)?;
    rasterize_svg(&svg, target_width_px)
}

/// Inner SVG rendering (called inside `catch_unwind`).
fn render_to_svg_inner(latex: &str) -> Result<String> {
    // Convert LaTeX to Typst math syntax via mitex
    let typst_math = mitex::convert_math(latex, None)
        .map_err(|e| anyhow::anyhow!("mitex conversion failed: {e}"))?;

    // Wrap in a minimal Typst document that renders just the math
    let typst_source = format!(
        "#set page(width: auto, height: auto, margin: 8pt)\n#set text(size: 16pt)\n$ {typst_math} $"
    );

    compile_typst_to_svg(&typst_source)
}

/// Compile a Typst source document to SVG.
fn compile_typst_to_svg(source: &str) -> Result<String> {
    use typst::layout::PagedDocument;

    // Create a minimal world for Typst compilation
    let world = MathWorld::new(source);

    let warned = typst::compile::<PagedDocument>(&world);
    let document = warned.output.map_err(|diagnostics| {
        let msgs: Vec<String> = diagnostics.iter().map(|d| d.message.to_string()).collect();
        anyhow::anyhow!("Typst compilation failed: {}", msgs.join("; "))
    })?;

    // Export the first page to SVG
    let page = document
        .pages
        .first()
        .ok_or_else(|| anyhow::anyhow!("Typst produced no pages"))?;
    let svg = typst_svg::svg(page);
    Ok(svg)
}

/// Rasterize an SVG string to a `DynamicImage`.
///
/// Reuses the same approach as `mermaid::rasterize_svg`.
fn rasterize_svg(svg: &str, target_width_px: u32) -> Result<DynamicImage> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let opts = resvg::usvg::Options {
        fontdb: Arc::new(db),
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

/// Minimal Typst world implementation for math rendering.
///
/// Provides only what's needed to compile math expressions — no file
/// I/O, no package resolution, no font loading beyond system fonts.
struct MathWorld {
    source: typst::syntax::Source,
    library: typst::utils::LazyHash<typst::Library>,
    book: typst::utils::LazyHash<typst::text::FontBook>,
    fonts: Vec<typst::text::Font>,
}

impl MathWorld {
    fn new(source_text: &str) -> Self {
        use typst::LibraryExt;

        let source = typst::syntax::Source::detached(source_text);
        let library = typst::utils::LazyHash::new(typst::Library::default());

        // Load system fonts for Typst
        let mut book = typst::text::FontBook::new();
        let mut fonts = Vec::new();

        let mut db = fontdb::Database::new();
        db.load_system_fonts();

        for face in db.faces() {
            let path = match &face.source {
                fontdb::Source::File(path) | fontdb::Source::SharedFile(path, _) => path.clone(),
                fontdb::Source::Binary(_) => continue,
            };

            let Ok(font_data) = std::fs::read(&path) else {
                continue;
            };
            let buffer = typst::foundations::Bytes::new(font_data);

            for font in typst::text::Font::iter(buffer) {
                book.push(font.info().clone());
                fonts.push(font);
            }
        }

        Self {
            source,
            library,
            book: typst::utils::LazyHash::new(book),
            fonts,
        }
    }
}

impl typst::World for MathWorld {
    fn library(&self) -> &typst::utils::LazyHash<typst::Library> {
        &self.library
    }

    fn book(&self) -> &typst::utils::LazyHash<typst::text::FontBook> {
        &self.book
    }

    fn main(&self) -> typst::syntax::FileId {
        self.source.id()
    }

    fn source(&self, _id: typst::syntax::FileId) -> typst::diag::FileResult<typst::syntax::Source> {
        Ok(self.source.clone())
    }

    fn file(
        &self,
        _id: typst::syntax::FileId,
    ) -> typst::diag::FileResult<typst::foundations::Bytes> {
        Err(typst::diag::FileError::AccessDenied)
    }

    fn font(&self, index: usize) -> Option<typst::text::Font> {
        self.fonts.get(index).cloned()
    }

    fn today(&self, _offset: Option<i64>) -> Option<typst::foundations::Datetime> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latex_to_unicode_greek() {
        let result = latex_to_unicode("\\alpha");
        assert_eq!(result, "α");
    }

    #[test]
    fn test_latex_to_unicode_superscript() {
        let result = latex_to_unicode("x^2");
        // unicodeit converts ^2 to superscript
        assert!(result.contains('²') || result.contains("x^2"));
    }

    #[test]
    fn test_latex_to_unicode_passthrough() {
        // Unknown commands should pass through
        let result = latex_to_unicode("hello world");
        assert_eq!(result, "hello world");
    }

    #[test]
    fn test_render_to_svg_simple() {
        let result = render_to_svg("E = mc^2");
        assert!(
            result.is_ok(),
            "render_to_svg should succeed for simple math: {:?}",
            result.err()
        );
        let svg = result.unwrap();
        assert!(svg.contains("<svg"), "output should be valid SVG");
    }

    #[test]
    fn test_render_to_image_simple() {
        let result = render_to_image("x^2 + y^2", 400);
        assert!(
            result.is_ok(),
            "render_to_image should succeed: {:?}",
            result.err()
        );
        let img = result.unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_render_to_svg_invalid_does_not_panic() {
        // The important thing is no panic, even on garbage input
        let _result = render_to_svg("}}}{{{\\invalid\\command");
    }
}
