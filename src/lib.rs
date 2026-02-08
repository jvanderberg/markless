#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_possible_wrap,
    clippy::similar_names,
    clippy::module_name_repetitions,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    clippy::return_self_not_must_use,
    clippy::unused_self,
    clippy::unnecessary_wraps,
    clippy::implicit_hasher,
    clippy::used_underscore_binding,
    clippy::multiple_crate_versions,
    clippy::doc_markdown,
    clippy::struct_field_names,
    clippy::needless_pass_by_value,
    clippy::redundant_closure_for_method_calls,
    clippy::option_if_let_else,
    clippy::match_same_arms,
    clippy::collapsible_if,
    clippy::collapsible_else_if,
    clippy::items_after_statements,
    clippy::suboptimal_flops,
    clippy::manual_abs_diff,
    clippy::significant_drop_tightening,
    clippy::map_unwrap_or,
    clippy::use_self,
    clippy::default_trait_access,
    clippy::pub_underscore_fields,
    clippy::missing_const_for_fn,
    clippy::uninlined_format_args
)]

//! # Markless
//!
//! A terminal markdown viewer with image support.
//!
//! Markless renders markdown files in the terminal with:
//! - Syntax-highlighted code blocks
//! - Image support (Kitty, Sixel, half-block fallback)
//! - Table of contents sidebar
//! - File watching for live preview
//!
//! ## Architecture
//!
//! Markless uses The Elm Architecture (TEA) pattern:
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
