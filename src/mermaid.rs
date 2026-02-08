//! Mermaid diagram rendering.
//!
//! Renders mermaid diagram source text to raster images using `mermaid-rs-renderer`
//! for SVG generation and `resvg` for rasterization.

use std::sync::Arc;

use anyhow::Result;
use image::DynamicImage;
use resvg::usvg::fontdb;

/// Render a mermaid diagram to an SVG string.
///
/// Generates SVG via `mermaid-rs-renderer` and fixes font-family quoting
/// so the result can be parsed by standard SVG tools.
///
/// # Errors
///
/// Returns an error if the mermaid source cannot be parsed.
pub fn render_to_svg(mermaid_source: &str) -> Result<String> {
    let svg = mermaid_rs_renderer::render(mermaid_source)?;
    Ok(fix_svg_font_families(&svg))
}

/// Render a mermaid diagram to a raster image.
///
/// Uses `mermaid-rs-renderer` to generate SVG, then `resvg` to rasterize it.
/// The `target_width_px` controls the rasterization width so the SVG is
/// rendered at the final display resolution (no lossy upscaling).
///
/// # Errors
///
/// Returns an error if the mermaid source cannot be parsed or the SVG
/// cannot be rasterized.
pub fn render_to_image(mermaid_source: &str, target_width_px: u32) -> Result<DynamicImage> {
    let svg = render_to_svg(mermaid_source)?;
    rasterize_svg(&svg, target_width_px)
}

/// Fix unescaped double quotes inside font-family attributes.
///
/// `mermaid-rs-renderer` emits font-family values like:
///   `font-family="Inter, ... "Segoe UI", sans-serif"`
/// The inner `"Segoe UI"` breaks XML parsing. We replace inner double
/// quotes with single quotes so resvg can parse the SVG.
fn fix_svg_font_families(svg: &str) -> String {
    const MARKER: &str = "font-family=\"";
    let mut result = String::with_capacity(svg.len());
    let mut rest = svg;

    while let Some(pos) = rest.find(MARKER) {
        // Copy everything up to and including the opening quote.
        result.push_str(&rest[..pos + MARKER.len()]);
        rest = &rest[pos + MARKER.len()..];

        // Scan for the closing quote: a `"` followed by `>`, ` `, `/`, or end.
        let mut value = String::new();
        let mut end_offset = rest.len();
        for (i, ch) in rest.char_indices() {
            if ch == '"' {
                // Check what follows this quote.
                let after = rest.get(i + 1..i + 2).unwrap_or("");
                if after.is_empty()
                    || after.starts_with('>')
                    || after.starts_with(' ')
                    || after.starts_with('/')
                {
                    // Real closing quote.
                    result.push_str(&value.replace('"', "'"));
                    result.push('"');
                    end_offset = i + 1;
                    break;
                }
                // Inner quote — part of value.
                value.push('"');
            } else {
                value.push(ch);
            }
        }
        rest = &rest[end_offset..];
    }
    result.push_str(rest);
    result
}

/// Rasterize an SVG string to a `DynamicImage`.
///
/// Scales the SVG so its width matches `target_width_px`, preserving aspect
/// ratio. This avoids lossy upscaling since the vector is rasterized directly
/// at the final display resolution.
fn rasterize_svg(svg: &str, target_width_px: u32) -> Result<DynamicImage> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let opts = resvg::usvg::Options {
        fontdb: Arc::new(db),
        ..Default::default()
    };

    let tree = resvg::usvg::Tree::from_str(svg, &opts)?;
    let size = tree.size();

    let scale = target_width_px as f32 / size.width();

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = (size.width() * scale).ceil() as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = (size.height() * scale).ceil() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap {width}x{height}"))?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let rgba = pixmap.data().to_vec();
    let img_buf = image::RgbaImage::from_raw(width, height, rgba)
        .ok_or_else(|| anyhow::anyhow!("failed to create image from pixmap data"))?;

    Ok(DynamicImage::ImageRgba8(img_buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_svg_font_families_replaces_inner_quotes() {
        let input = r#"<text font-family="Inter, "Segoe UI", sans-serif" font-size="14">"#;
        let fixed = fix_svg_font_families(input);
        assert_eq!(
            fixed,
            r#"<text font-family="Inter, 'Segoe UI', sans-serif" font-size="14">"#
        );
    }

    #[test]
    fn test_fix_svg_font_families_no_op_when_clean() {
        let input = r#"<text font-family="Inter, sans-serif" font-size="14">"#;
        let fixed = fix_svg_font_families(input);
        assert_eq!(fixed, input);
    }

    #[test]
    fn test_render_to_svg_returns_valid_svg() {
        let source = "flowchart LR\n    A[Start] --> B[End]";
        let svg = render_to_svg(source).unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn test_render_flowchart_to_image() {
        let source = "flowchart LR\n    A[Start] --> B[End]";
        let img = render_to_image(source, 1200).unwrap();
        assert_eq!(img.width(), 1200);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_render_sequence_diagram_to_image() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi";
        let img = render_to_image(source, 1200).unwrap();
        assert_eq!(img.width(), 1200);
        assert!(img.height() > 0);
    }
}
