//! Application state and main event loop.
//!
//! This module implements The Elm Architecture (TEA):
//! - [`Model`]: The complete application state
//! - [`Message`]: All possible events and actions
//! - [`update`]: Pure function for state transitions
//! - [`App::run`]: Main event loop with rendering

mod effects;
mod event_loop;
mod input;
mod model;
mod update;

pub use model::{Model, ToastLevel};
pub use update::{Message, update};

use std::path::PathBuf;

use crate::config::ImageMode;

/// Main application struct that owns the terminal and runs the event loop.
pub struct App {
    file_path: PathBuf,
    watch_enabled: bool,
    toc_visible: bool,
    image_mode: Option<ImageMode>,
    images_enabled: bool,
    config_global_path: Option<PathBuf>,
    config_local_path: Option<PathBuf>,
    browse_mode: bool,
}

impl App {
    /// Create a new application for the given file.
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            watch_enabled: false,
            toc_visible: false,
            image_mode: None,
            images_enabled: true,
            config_global_path: None,
            config_local_path: None,
            browse_mode: false,
        }
    }

    /// Enable or disable file watching.
    pub fn with_watch(mut self, enabled: bool) -> Self {
        self.watch_enabled = enabled;
        self
    }

    /// Set initial TOC visibility.
    pub fn with_toc_visible(mut self, visible: bool) -> Self {
        self.toc_visible = visible;
        self
    }

    /// Force a specific image rendering mode, bypassing auto-detection.
    pub const fn with_image_mode(mut self, mode: Option<ImageMode>) -> Self {
        self.image_mode = mode;
        self
    }

    /// Enable or disable inline image rendering.
    pub fn with_images_enabled(mut self, enabled: bool) -> Self {
        self.images_enabled = enabled;
        self
    }

    /// Enable directory browse mode.
    pub fn with_browse_mode(mut self, enabled: bool) -> Self {
        self.browse_mode = enabled;
        self
    }

    /// Set config paths to show in help.
    pub fn with_config_paths(
        mut self,
        global_path: Option<PathBuf>,
        local_path: Option<PathBuf>,
    ) -> Self {
        self.config_global_path = global_path;
        self.config_local_path = local_path;
        self
    }
}

#[cfg(test)]
mod tests;
