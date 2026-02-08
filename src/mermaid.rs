//! Mermaid diagram rendering.
//!
//! Renders mermaid diagram source text to raster images using `mermaid-rs-renderer`
//! for SVG generation and `resvg` for rasterization.

use anyhow::Result;
use image::DynamicImage;

/// Render a mermaid diagram to a raster image.
///
/// Uses `mermaid-rs-renderer` to generate SVG, then `resvg` to rasterize it.
///
/// # Errors
///
/// Returns an error if the mermaid source cannot be parsed or the SVG
/// cannot be rasterized.
pub fn render_to_image(mermaid_source: &str) -> Result<DynamicImage> {
    let svg = mermaid_rs_renderer::render(mermaid_source)?;
    let svg = fix_svg_font_families(&svg);
    rasterize_svg(&svg)
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
fn rasterize_svg(svg: &str) -> Result<DynamicImage> {
    let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default())?;
    let size = tree.size();

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let width = size.width().ceil() as u32;
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let height = size.height().ceil() as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height)
        .ok_or_else(|| anyhow::anyhow!("failed to create pixmap {width}x{height}"))?;

    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::default(),
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
    fn test_render_flowchart_to_image() {
        let source = "flowchart LR\n    A[Start] --> B[End]";
        let img = render_to_image(source).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }

    #[test]
    fn test_render_sequence_diagram_to_image() {
        let source = "sequenceDiagram\n    Alice->>Bob: Hello\n    Bob-->>Alice: Hi";
        let img = render_to_image(source).unwrap();
        assert!(img.width() > 0);
        assert!(img.height() > 0);
    }
}
