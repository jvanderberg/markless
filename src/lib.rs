//! # Gander
//!
//! A terminal markdown viewer with image support.
//!
//! Gander renders markdown files in the terminal with:
//! - Syntax-highlighted code blocks
//! - Image support (Kitty, Sixel, half-block fallback)
//! - Table of contents sidebar
//! - File watching for live preview
//!
//! ## Architecture
//!
//! Gander uses The Elm Architecture (TEA) pattern:
//! - **Model**: Application state
//! - **Message**: Events and actions
//! - **Update**: Pure state transitions
//! - **View**: Render to terminal
//!
//! ## Modules
//!
//! - [`app`]: Main application loop and state
//! - [`document`]: Markdown parsing and rendering
//! - [`ui`]: Terminal UI components
//! - [`input`]: Event handling and keybindings
//! - [`highlight`]: Syntax highlighting
//! - [`image`]: Image loading and rendering
//! - [`watcher`]: File watching
//! - [`search`]: Search functionality

pub mod app;
pub mod config;
pub mod document;
pub mod highlight;
pub mod image;
pub mod input;
pub mod perf;
pub mod search;
pub mod ui;
pub mod watcher;

/// Re-export commonly used types
pub mod prelude {
    pub use crate::app::{App, Message, Model};
    pub use crate::document::Document;
    pub use crate::ui::viewport::Viewport;
}
