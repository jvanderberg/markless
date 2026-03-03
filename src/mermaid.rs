//! Mermaid diagram rendering.
//!
//! Renders mermaid diagram source text to raster images using `mermaid-rs-renderer`
//! for SVG generation and `resvg` for rasterization.

use std::panic::{AssertUnwindSafe, catch_unwind};

use anyhow::Result;
use image::DynamicImage;

/// Render a mermaid diagram to an SVG string.
///
/// Generates SVG via `mermaid-rs-renderer` and fixes font-family quoting
/// so the result can be parsed by standard SVG tools.
///
/// # Errors
///
/// Returns an error if the mermaid source cannot be parsed.
pub fn render_to_svg(mermaid_source: &str) -> Result<String> {
    let mut opts = mermaid_rs_renderer::RenderOptions::default();
    opts.theme.font_family = "Helvetica, Arial, sans-serif".to_string();
    let svg = catch_unwind(AssertUnwindSafe(|| {
        mermaid_rs_renderer::render_with_options(mermaid_source, opts)
    }))
    .unwrap_or_else(|_| Err(anyhow::anyhow!("mermaid-rs-renderer panicked")))?;
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
    crate::svg::rasterize_svg(&svg, target_width_px)
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
    fn test_render_to_svg_uses_universal_fonts() {
        let source = "flowchart LR\n    A[Start] --> B[End]";
        let svg = render_to_svg(source).unwrap();
        // Should not require Inter (not installed everywhere).
        assert!(
            !svg.contains("font-family=\"Inter"),
            "SVG should not use Inter as primary font"
        );
    }

    #[test]
    fn test_render_to_svg_does_not_panic_for_invalid_input() {
        // Verifies that catch_unwind prevents a crash on bad input.
        // The library may return Ok (best-effort SVG) or Err — we don't
        // assert either way because the important property is "no panic".
        let _result = render_to_svg("this is not valid mermaid at all }{}{}{");
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
