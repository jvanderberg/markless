//! Markdown document parsing and rendering.
//!
//! This module handles:
//! - Parsing markdown with comrak
//! - Extracting document structure (headings, links, images)
//! - Rendering to styled lines for display

mod parser;
mod types;

pub use parser::{parse, parse_with_image_heights, parse_with_layout};
pub use types::{
    Document, HeadingRef, ImageRef, InlineColor, InlineSpan, InlineStyle, LineType, LinkRef,
    RenderedLine,
};
