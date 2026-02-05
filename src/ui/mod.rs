//! Terminal UI components.
//!
//! This module contains all UI-related code including:
//! - [`viewport`]: Scroll position and visible range management
//! - [`widgets`]: Ratatui widgets for rendering
//! - [`style`]: Theming and colors

pub mod style;
pub mod viewport;
pub mod widgets;

mod images;
mod overlays;
mod render;
mod status;

pub use overlays::{link_picker_content_top, link_picker_rect};
pub use render::{document_content_width, render, split_main_columns};

pub const DOCUMENT_LEFT_PADDING: u16 = 2;
pub const TOC_WIDTH_PERCENT: u16 = 30;
pub const DOC_WIDTH_PERCENT: u16 = 70;

#[cfg(test)]
mod tests;
