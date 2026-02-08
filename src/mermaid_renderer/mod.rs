//! Vendored copy of [mermaid-rs-renderer](https://github.com/1jehuang/mermaid-rs-renderer)
//! (MIT license — see `LICENSE` in this directory).
//!
//! Only the library core is included (no CLI, no `layout_dump`).

pub mod config;
pub mod ir;
pub mod layout;
pub mod parser;
pub mod render;
mod text_metrics;
pub mod theme;

use config::LayoutConfig;
use layout::compute_layout;
use parser::parse_mermaid;
use render::render_svg;
use theme::Theme;

/// Render a Mermaid diagram to SVG with default options.
///
/// # Errors
///
/// Returns an error if the diagram syntax is invalid.
pub fn render(input: &str) -> anyhow::Result<String> {
    let parsed = parse_mermaid(input)?;
    let theme = Theme::modern();
    let layout_config = LayoutConfig::default();
    let laid_out = compute_layout(&parsed.graph, &theme, &layout_config);
    let svg = render_svg(&laid_out, &theme, &layout_config);
    Ok(svg)
}
