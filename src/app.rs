//! Application state and main event loop.
//!
//! This module implements The Elm Architecture (TEA):
//! - [`Model`]: The complete application state
//! - [`Message`]: All possible events and actions
//! - [`update`]: Pure function for state transitions
//! - [`App::run`]: Main event loop with rendering

use std::collections::HashMap;
use std::io::stdout;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use image::DynamicImage;
use ratatui::layout::Rect;
use ratatui::{DefaultTerminal, Frame};
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::document::Document;
use crate::image::ImageLoader;
use crate::ui::viewport::Viewport;
use crate::watcher::FileWatcher;

/// The complete application state.
///
/// All state lives here - no global or scattered state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
struct Toast {
    level: ToastLevel,
    message: String,
    expires_at: Instant,
}

pub struct Model {
    /// The loaded markdown document
    pub document: Document,
    /// Viewport managing scroll position
    pub viewport: Viewport,
    /// Path to the source file
    pub file_path: PathBuf,
    /// Base directory for resolving relative image paths
    pub base_dir: PathBuf,
    /// Whether TOC sidebar is visible
    pub toc_visible: bool,
    /// Selected TOC entry index
    pub toc_selected: Option<usize>,
    /// Scroll offset for TOC viewport
    pub toc_scroll_offset: usize,
    /// Whether file watching is enabled
    pub watch_enabled: bool,
    /// Global config path shown in help
    pub config_global_path: Option<PathBuf>,
    /// Local override path shown in help
    pub config_local_path: Option<PathBuf>,
    /// Whether help overlay is visible
    pub help_visible: bool,
    toast: Option<Toast>,
    /// Current search query
    pub search_query: Option<String>,
    /// Rendered line indices that match the current search query
    search_matches: Vec<usize>,
    /// Current selected match index inside `search_matches`
    search_match_index: Option<usize>,
    /// Allow searching short (<3 char) queries after explicit Enter.
    search_allow_short: bool,
    /// Whether the app should quit
    pub should_quit: bool,
    /// Focus: true = TOC, false = document
    pub toc_focused: bool,
    /// Image protocols for rendering (keyed by image src)
    /// Stores (protocol, width_cols, height_rows)
    pub image_protocols: HashMap<String, (StatefulProtocol, u16, u16)>,
    /// Cache of original images (before scaling) for fast resize
    original_images: HashMap<String, DynamicImage>,
    /// Image picker for terminal rendering
    pub picker: Option<Picker>,
    /// Viewport width used when images were last scaled (for detecting resize)
    last_image_scale_width: u16,
    /// Reserved image heights in document layout (terminal rows)
    image_layout_heights: HashMap<String, usize>,
    /// True when a resize is pending and expensive work should be paused
    resize_pending: bool,
    /// Short cooldown used only for iTerm2 inline image placeholdering while scrolling
    image_scroll_cooldown_ticks: u8,
    /// Force half-cell image rendering path (used for debug and filter tuning)
    force_half_cell: bool,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("file_path", &self.file_path)
            .field("toc_visible", &self.toc_visible)
            .field("watch_enabled", &self.watch_enabled)
            .finish_non_exhaustive()
    }
}

impl Model {
    /// Create a new model with default settings.
    pub fn new(file_path: PathBuf, document: Document, terminal_size: (u16, u16)) -> Self {
        let total_lines = document.line_count();
        let base_dir = file_path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        Self {
            document,
            viewport: Viewport::new(
                terminal_size.0,
                terminal_size.1.saturating_sub(1),
                total_lines,
            ),
            file_path,
            base_dir,
            toc_visible: false,
            toc_selected: None,
            toc_scroll_offset: 0,
            watch_enabled: false,
            config_global_path: None,
            config_local_path: None,
            help_visible: false,
            toast: None,
            search_query: None,
            search_matches: Vec::new(),
            search_match_index: None,
            search_allow_short: false,
            should_quit: false,
            toc_focused: false,
            image_protocols: HashMap::new(),
            original_images: HashMap::new(),
            picker: None,
            last_image_scale_width: terminal_size.0,
            image_layout_heights: HashMap::new(),
            resize_pending: false,
            image_scroll_cooldown_ticks: 0,
            force_half_cell: false,
        }
    }

    /// Set the image picker.
    pub fn with_picker(mut self, picker: Option<Picker>) -> Self {
        self.picker = picker;
        self
    }

    /// Load images that are near the viewport (lazy loading with lookahead).
    pub fn load_nearby_images(&mut self) {
        if self.resize_pending {
            crate::perf::log_event("image.load_nearby.skip", "resize_pending=true");
            return;
        }
        let Some(picker) = &self.picker else { return };

        let current_width = self.viewport.width();
        let width_changed = self.last_image_scale_width != current_width;
        let use_halfblocks = matches!(picker.protocol_type(), ProtocolType::Halfblocks);
        let quantize_halfblocks = use_halfblocks && !crate::image::supports_truecolor_terminal();
        if width_changed {
            self.last_image_scale_width = current_width;
        }

        let font_size = picker.font_size();
        let target_width_cols = (current_width as f32 * 0.65) as u16;
        let target_width_px = target_width_cols as u32 * font_size.0 as u32;

        // Load images within 2 viewport heights of current position
        let lookahead = self.viewport.height() as usize * 2;
        let vp_start = self.viewport.offset();
        let vp_end = vp_start + self.viewport.height() as usize;
        let load_start = vp_start.saturating_sub(lookahead);
        let load_end = vp_end + lookahead;

        // Collect image refs to process (avoid borrow issues)
        let images_to_process: Vec<_> = self.document.images()
            .iter()
            .filter(|img_ref| {
                let img_start = img_ref.line_range.start;
                let img_end = img_ref.line_range.end;
                img_end > load_start && img_start < load_end
            })
            .map(|img_ref| img_ref.src.clone())
            .collect();
        crate::perf::log_event(
            "image.load_nearby.begin",
            format!(
                "viewport={}..{} lookahead={} width={} target_cols={} candidates={}",
                vp_start,
                vp_end,
                lookahead,
                current_width,
                target_width_cols,
                images_to_process.len()
            ),
        );

        let loader = ImageLoader::new(self.base_dir.clone());

        for src in images_to_process {
            // Check if we need to load/reload this image's protocol
            let needs_protocol = match self.image_protocols.get(&src) {
                None => true,
                Some((_, w, _)) => width_changed && *w != target_width_cols,
            };

            if needs_protocol {
                // Try to get original image from cache, or load from disk
                let original: Option<DynamicImage> =
                    if let Some(img) = self.original_images.get(&src) {
                    Some(img.clone())
                } else if let Some(img) = loader.load_sync(&src) {
                    self.original_images.insert(src.clone(), img.clone());
                    Some(img)
                } else {
                    None
                };

                if let Some(img) = original {
                    // Scale to fit target width, preserving aspect ratio
                    let scale = target_width_px as f32 / img.width() as f32;
                    let scaled_height_px = (img.height() as f32 * scale) as u32;

                    let mut scaled = img.resize(
                        target_width_px,
                        scaled_height_px,
                        if use_halfblocks || self.force_half_cell {
                            image::imageops::FilterType::CatmullRom
                        } else {
                            image::imageops::FilterType::Nearest
                        },
                    );
                    if quantize_halfblocks {
                        scaled = crate::image::quantize_to_ansi256(&scaled);
                    }

                    // Calculate dimensions in terminal cells
                    let width_cols = target_width_cols;
                    let height_rows = (scaled_height_px as f32 / font_size.1 as f32).ceil() as u16;

                    let protocol = picker.new_resize_protocol(scaled);
                    self.image_protocols
                        .insert(src.clone(), (protocol, width_cols, height_rows));
                    crate::perf::log_event(
                        "image.load_nearby.protocol",
                        format!(
                            "src={} width_cols={} height_rows={} width_changed={} halfblocks={} ansi256={}",
                            src,
                            width_cols,
                            height_rows,
                            width_changed,
                            use_halfblocks,
                            quantize_halfblocks
                        ),
                    );
                }
            }
        }

        let current_layout_heights: HashMap<String, usize> = self
            .image_protocols
            .iter()
            .map(|(src, (_, _, height_rows))| (src.clone(), *height_rows as usize))
            .collect();

        if current_layout_heights != self.image_layout_heights {
            crate::perf::log_event(
                "image.layout.reflow",
                format!(
                    "old={} new={}",
                    self.image_layout_heights.len(),
                    current_layout_heights.len()
                ),
            );
            self.image_layout_heights = current_layout_heights;
            self.reflow_layout();
        }
    }

    pub fn ensure_highlight_overscan(&mut self) {
        let height = self.viewport.height() as usize;
        let extra = height * 2;
        let start = self.viewport.offset().saturating_sub(extra);
        let end = (self.viewport.offset() + height + extra).min(self.document.line_count());
        self.document.ensure_highlight_for_range(start..end);
    }

    pub fn tick_image_scroll_cooldown(&mut self) {
        self.image_scroll_cooldown_ticks = self.image_scroll_cooldown_ticks.saturating_sub(1);
    }

    pub fn is_image_scroll_settling(&self) -> bool {
        self.image_scroll_cooldown_ticks > 0
    }

    fn bump_image_scroll_cooldown(&mut self) {
        self.image_scroll_cooldown_ticks = 3;
    }

    pub fn set_resize_pending(&mut self, pending: bool) {
        self.resize_pending = pending;
    }

    pub fn search_match_count(&self) -> usize {
        self.search_matches.len()
    }

    pub fn current_search_match(&self) -> Option<(usize, usize)> {
        self.search_match_index
            .map(|idx| (idx + 1, self.search_matches.len()))
    }

    fn layout_width(&self) -> u16 {
        crate::ui::document_content_width(self.viewport.width(), self.toc_visible)
    }

    fn toc_visible_rows(&self) -> usize {
        // TOC uses full frame height with a 1-cell border at top/bottom.
        self.viewport.height().saturating_sub(1) as usize
    }

    fn max_toc_scroll_offset(&self) -> usize {
        self.document
            .headings()
            .len()
            .saturating_sub(self.toc_visible_rows())
    }

    fn sync_toc_to_viewport(&mut self) {
        let Some(selected) = closest_heading_to_line(self.document.headings(), self.viewport.offset()) else {
            self.toc_selected = None;
            self.toc_scroll_offset = 0;
            return;
        };
        self.toc_selected = Some(selected);
        self.toc_scroll_offset = selected.min(self.max_toc_scroll_offset());
    }

    fn reflow_layout(&mut self) {
        if let Ok(document) = Document::parse_with_layout_and_image_heights(
            self.document.source(),
            self.layout_width(),
            &self.image_layout_heights,
        ) {
            self.document = document;
            self.viewport.set_total_lines(self.document.line_count());
            self.toc_scroll_offset = self.toc_scroll_offset.min(self.max_toc_scroll_offset());
            let allow_short = self.search_allow_short;
            refresh_search_matches(self, false, allow_short);
        }
    }

    fn show_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast {
            level,
            message: message.into(),
            expires_at: Instant::now() + Duration::from_secs(4),
        });
    }

    fn expire_toast(&mut self, now: Instant) -> bool {
        if self
            .toast
            .as_ref()
            .is_some_and(|toast| toast.expires_at <= now)
        {
            self.toast = None;
            return true;
        }
        false
    }

    pub fn active_toast(&self) -> Option<(&str, ToastLevel)> {
        self.toast
            .as_ref()
            .map(|toast| (toast.message.as_str(), toast.level))
    }

    fn reload_from_disk(&mut self) -> Result<()> {
        let content = std::fs::read_to_string(&self.file_path)?;
        let document = Document::parse_with_layout_and_image_heights(
            &content,
            self.layout_width(),
            &self.image_layout_heights,
        )?;
        self.document = document;

        // Drop cached image entries that are no longer present in the document.
        let valid_images: std::collections::HashSet<_> = self
            .document
            .images()
            .iter()
            .map(|img| img.src.clone())
            .collect();
        self.image_protocols
            .retain(|src, _| valid_images.contains(src));
        self.original_images
            .retain(|src, _| valid_images.contains(src));
        self.image_layout_heights
            .retain(|src, _| valid_images.contains(src));

        self.viewport.set_total_lines(self.document.line_count());
        self.toc_scroll_offset = self.toc_scroll_offset.min(self.max_toc_scroll_offset());
        let allow_short = self.search_allow_short;
        refresh_search_matches(self, false, allow_short);
        if self.toc_visible && !self.toc_focused {
            self.sync_toc_to_viewport();
        }
        Ok(())
    }
}

/// All possible events and actions in the application.
///
/// These represent user input, system events, and internal actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    // Navigation
    /// Scroll up by n lines
    ScrollUp(usize),
    /// Scroll down by n lines
    ScrollDown(usize),
    /// Scroll up one page
    PageUp,
    /// Scroll down one page
    PageDown,
    /// Scroll up half page
    HalfPageUp,
    /// Scroll down half page
    HalfPageDown,
    /// Go to beginning of document
    GoToTop,
    /// Go to end of document
    GoToBottom,
    /// Go to specific line
    GoToLine(usize),
    /// Go to percentage through document
    GoToPercent(u8),

    // TOC
    /// Toggle TOC sidebar visibility
    ToggleToc,
    /// Toggle TOC and focus it
    ToggleTocFocus,
    /// Move TOC selection up
    TocUp,
    /// Move TOC selection down
    TocDown,
    /// Scroll TOC viewport up
    TocScrollUp,
    /// Scroll TOC viewport down
    TocScrollDown,
    /// Jump to selected TOC heading
    TocSelect,
    /// Select and jump to TOC heading by index
    TocClick(usize),
    /// Collapse TOC entry
    TocCollapse,
    /// Expand TOC entry
    TocExpand,
    /// Switch focus between TOC and document
    SwitchFocus,

    // File watching
    /// Toggle file watching
    ToggleWatch,
    /// Toggle help overlay
    ToggleHelp,
    /// Hide help overlay
    HideHelp,
    /// File changed externally, reload
    FileChanged,
    /// Force reload file
    ForceReload,

    // Search
    /// Start search mode
    StartSearch,
    /// Start search mode with initial query text
    StartSearchWith(String),
    /// Update search query
    SearchInput(String),
    /// Go to next search match
    NextMatch,
    /// Go to previous match
    PrevMatch,
    /// Clear search
    ClearSearch,

    // Window
    /// Terminal resized
    Resize(u16, u16),
    /// Redraw screen
    Redraw,

    // Application
    /// Quit the application
    Quit,
}

/// Pure function that updates the model based on a message.
///
/// This is the core of TEA - all state transitions happen here.
/// No side effects should occur in this function.
pub fn update(mut model: Model, msg: Message) -> Model {
    let should_sync_toc = !matches!(
        &msg,
        Message::TocUp
            | Message::TocDown
            | Message::TocScrollUp
            | Message::TocScrollDown
            | Message::TocSelect
            | Message::TocClick(_)
            | Message::TocCollapse
            | Message::TocExpand
    );
    match msg {
        // Navigation
        Message::ScrollUp(n) => {
            model.viewport.scroll_up(n);
            model.bump_image_scroll_cooldown();
        }
        Message::ScrollDown(n) => {
            model.viewport.scroll_down(n);
            model.bump_image_scroll_cooldown();
        }
        Message::PageUp => {
            model.viewport.page_up();
            model.bump_image_scroll_cooldown();
        }
        Message::PageDown => {
            model.viewport.page_down();
            model.bump_image_scroll_cooldown();
        }
        Message::HalfPageUp => {
            model.viewport.half_page_up();
            model.bump_image_scroll_cooldown();
        }
        Message::HalfPageDown => {
            model.viewport.half_page_down();
            model.bump_image_scroll_cooldown();
        }
        Message::GoToTop => {
            model.viewport.go_to_top();
            model.bump_image_scroll_cooldown();
        }
        Message::GoToBottom => {
            model.viewport.go_to_bottom();
            model.bump_image_scroll_cooldown();
        }
        Message::GoToLine(line) => {
            model.viewport.go_to_line(line);
            model.bump_image_scroll_cooldown();
        }
        Message::GoToPercent(percent) => {
            model.viewport.go_to_percent(percent);
            model.bump_image_scroll_cooldown();
        }

        // TOC
        Message::ToggleToc => {
            model.toc_visible = !model.toc_visible;
            if model.toc_visible && model.toc_selected.is_none() {
                model.toc_selected = Some(0);
            }
            model.reflow_layout();
        }
        Message::ToggleTocFocus => {
            model.toc_visible = !model.toc_visible;
            model.toc_focused = model.toc_visible;
            if model.toc_visible && model.toc_selected.is_none() {
                model.toc_selected = Some(0);
            }
            model.reflow_layout();
        }
        Message::TocUp => {
            if let Some(sel) = model.toc_selected {
                let next = sel.saturating_sub(1);
                model.toc_selected = Some(next);
                if next < model.toc_scroll_offset {
                    model.toc_scroll_offset = next;
                }
            }
        }
        Message::TocDown => {
            if let Some(sel) = model.toc_selected {
                let max = model.document.headings().len().saturating_sub(1);
                let next = (sel + 1).min(max);
                model.toc_selected = Some(next);
                let visible = model.toc_visible_rows();
                if visible > 0 {
                    let bottom = model.toc_scroll_offset + visible.saturating_sub(1);
                    if next > bottom {
                        model.toc_scroll_offset = (next + 1)
                            .saturating_sub(visible)
                            .min(model.max_toc_scroll_offset());
                    }
                }
            }
        }
        Message::TocSelect => {
            if let Some(sel) = model.toc_selected {
                if let Some(heading) = model.document.headings().get(sel) {
                    model.viewport.go_to_line(heading.line);
                }
            }
        }
        Message::TocClick(idx) => {
            model.toc_selected = Some(idx);
            if let Some(heading) = model.document.headings().get(idx) {
                model.viewport.go_to_line(heading.line);
            }
        }
        Message::TocScrollUp => {
            model.toc_scroll_offset = model.toc_scroll_offset.saturating_sub(1);
        }
        Message::TocScrollDown => {
            model.toc_scroll_offset =
                (model.toc_scroll_offset + 1).min(model.max_toc_scroll_offset());
        }
        Message::TocCollapse | Message::TocExpand => {
            // TODO: Implement collapse/expand
        }
        Message::SwitchFocus => {
            if model.toc_visible {
                model.toc_focused = !model.toc_focused;
            }
        }

        // File watching
        Message::ToggleWatch => {
            model.watch_enabled = !model.watch_enabled;
        }
        Message::ToggleHelp => {
            model.help_visible = !model.help_visible;
        }
        Message::HideHelp => {
            model.help_visible = false;
        }
        Message::FileChanged | Message::ForceReload => {
            // Reload is handled in the event loop (side effect)
            // The model update happens after reload
        }

        // Search
        Message::StartSearch => {
            model.search_query = Some(String::new());
            model.search_matches.clear();
            model.search_match_index = None;
            model.search_allow_short = false;
        }
        Message::StartSearchWith(query) => {
            model.search_query = Some(query);
            model.search_allow_short = false;
            let allow_short = model.search_allow_short;
            refresh_search_matches(&mut model, true, allow_short);
        }
        Message::SearchInput(query) => {
            model.search_query = Some(query);
            model.search_allow_short = false;
            let allow_short = model.search_allow_short;
            refresh_search_matches(&mut model, true, allow_short);
        }
        Message::NextMatch => {
            if model.search_matches.is_empty() {
                model.search_allow_short = true;
                refresh_search_matches(&mut model, true, true);
            }
            if !model.search_matches.is_empty() {
                let next = match model.search_match_index {
                    Some(idx) => (idx + 1) % model.search_matches.len(),
                    None => 0,
                };
                model.search_match_index = Some(next);
                if let Some(line) = model.search_matches.get(next).copied() {
                    model.viewport.go_to_line(line);
                }
            }
        }
        Message::PrevMatch => {
            if model.search_matches.is_empty() {
                model.search_allow_short = true;
                refresh_search_matches(&mut model, true, true);
            }
            if !model.search_matches.is_empty() {
                let prev = match model.search_match_index {
                    Some(0) | None => model.search_matches.len() - 1,
                    Some(idx) => idx - 1,
                };
                model.search_match_index = Some(prev);
                if let Some(line) = model.search_matches.get(prev).copied() {
                    model.viewport.go_to_line(line);
                }
            }
        }
        Message::ClearSearch => {
            model.search_query = None;
            model.search_matches.clear();
            model.search_match_index = None;
            model.search_allow_short = false;
        }

        // Window
        Message::Resize(width, height) => {
            model.viewport.resize(width, height.saturating_sub(1));
            model.reflow_layout();
        }
        Message::Redraw => {
            // No state change needed
        }

        // Application
        Message::Quit => {
            model.should_quit = true;
        }
    }
    if should_sync_toc && model.toc_visible && !model.toc_focused {
        model.sync_toc_to_viewport();
    }
    model
}

fn closest_heading_to_line(headings: &[crate::document::HeadingRef], line: usize) -> Option<usize> {
    if headings.is_empty() {
        return None;
    }
    let next = headings.partition_point(|h| h.line < line);
    if next == 0 {
        return Some(0);
    }
    if next >= headings.len() {
        return Some(headings.len() - 1);
    }
    let prev_idx = next - 1;
    let prev_dist = line.saturating_sub(headings[prev_idx].line);
    let next_dist = headings[next].line.saturating_sub(line);
    if prev_dist <= next_dist {
        Some(prev_idx)
    } else {
        Some(next)
    }
}

fn refresh_search_matches(model: &mut Model, jump_to_first: bool, allow_short: bool) {
    let Some(query) = model.search_query.as_deref() else {
        model.search_matches.clear();
        model.search_match_index = None;
        return;
    };

    if !allow_short && query.trim().chars().count() < 3 {
        model.search_matches.clear();
        model.search_match_index = None;
        return;
    }

    model.search_matches = crate::search::find_matches(&model.document, query);
    if model.search_matches.is_empty() {
        model.search_match_index = None;
        return;
    }

    if jump_to_first || model.search_match_index.is_none() {
        model.search_match_index = Some(0);
        if let Some(line) = model.search_matches.first().copied() {
            model.viewport.go_to_line(line);
        }
    } else if let Some(idx) = model.search_match_index {
        let clamped = idx.min(model.search_matches.len() - 1);
        model.search_match_index = Some(clamped);
    }
}

/// Main application struct that owns the terminal and runs the event loop.
pub struct App {
    file_path: PathBuf,
    watch_enabled: bool,
    toc_visible: bool,
    force_half_cell: bool,
    config_global_path: Option<PathBuf>,
    config_local_path: Option<PathBuf>,
}

struct ResizeDebouncer {
    delay_ms: u64,
    pending: Option<(u16, u16, u64)>,
}

impl ResizeDebouncer {
    fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: None,
        }
    }

    fn queue(&mut self, width: u16, height: u16, now_ms: u64) {
        self.pending = Some((width, height, now_ms));
    }

    fn take_ready(&mut self, now_ms: u64) -> Option<(u16, u16)> {
        let (width, height, queued_at) = self.pending?;
        if now_ms.saturating_sub(queued_at) >= self.delay_ms {
            self.pending = None;
            Some((width, height))
        } else {
            None
        }
    }

    fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

impl App {
    /// Create a new application for the given file.
    pub fn new(file_path: PathBuf) -> Self {
        Self {
            file_path,
            watch_enabled: false,
            toc_visible: false,
            force_half_cell: false,
            config_global_path: None,
            config_local_path: None,
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

    /// Force image rendering to use half-cell fallback mode.
    pub fn with_force_half_cell(mut self, enabled: bool) -> Self {
        self.force_half_cell = enabled;
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

    fn make_file_watcher(&self) -> notify::Result<FileWatcher> {
        FileWatcher::new(&self.file_path, Duration::from_millis(200))
    }

    fn handle_message_side_effects(
        &self,
        model: &mut Model,
        file_watcher: &mut Option<FileWatcher>,
        msg: &Message,
    ) {
        match msg {
            Message::ToggleWatch => {
                if model.watch_enabled {
                    match self.make_file_watcher() {
                        Ok(watcher) => {
                            *file_watcher = Some(watcher);
                            model.show_toast(ToastLevel::Info, "Watching file changes");
                        }
                        Err(err) => {
                            model.watch_enabled = false;
                            *file_watcher = None;
                            model.show_toast(
                                ToastLevel::Warning,
                                format!("Watch unavailable: {err}"),
                            );
                            crate::perf::log_event(
                                "watcher.error",
                                format!("failed path={} err={err}", model.file_path.display()),
                            );
                        }
                    }
                } else {
                    *file_watcher = None;
                    model.show_toast(ToastLevel::Info, "Watch disabled");
                }
            }
            Message::ForceReload | Message::FileChanged => {
                if let Err(err) = model.reload_from_disk() {
                    model.show_toast(
                        ToastLevel::Error,
                        format!("Reload failed: {err}"),
                    );
                    crate::perf::log_event(
                        "reload.error",
                        format!("failed path={} err={err}", model.file_path.display()),
                    );
                } else if matches!(msg, Message::ForceReload) {
                    model.show_toast(ToastLevel::Info, "Reloaded");
                }
            }
            _ => {}
        }
    }

    /// Run the main event loop.
    pub fn run(&mut self) -> Result<()> {
        let _run_scope = crate::perf::scope("app.run.total");

        // Create image picker BEFORE initializing terminal (queries stdio)
        let _picker_scope = crate::perf::scope("app.create_picker");
        let picker = crate::image::create_picker(self.force_half_cell);
        drop(_picker_scope);

        // Load the document
        let _read_scope = crate::perf::scope("app.read_file");
        let content = std::fs::read_to_string(&self.file_path)?;
        drop(_read_scope);

        // Initialize terminal
        let _init_scope = crate::perf::scope("app.ratatui_init");
        let mut terminal = ratatui::init();
        execute!(stdout(), EnableMouseCapture)?;
        let size = terminal.size()?;
        drop(_init_scope);

        let _parse_scope = crate::perf::scope("app.parse_with_layout");
        let layout_width = crate::ui::document_content_width(size.width, self.toc_visible);
        let document = Document::parse_with_layout(&content, layout_width)?;
        drop(_parse_scope);

        // Create initial model
        let mut model = Model::new(self.file_path.clone(), document, (size.width, size.height))
            .with_picker(picker);
        model.watch_enabled = self.watch_enabled;
        model.toc_visible = self.toc_visible;
        model.force_half_cell = self.force_half_cell;
        model.config_global_path = self.config_global_path.clone();
        model.config_local_path = self.config_local_path.clone();

        // Pre-load images from the document
        let _images_scope = crate::perf::scope("app.load_nearby_images.initial");
        model.load_nearby_images();
        drop(_images_scope);
        model.ensure_highlight_overscan();

        // Main loop
        let result = self.event_loop(&mut terminal, &mut model);

        // Restore terminal
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();

        result
    }

    fn event_loop(&self, terminal: &mut DefaultTerminal, model: &mut Model) -> Result<()> {
        let start = std::time::Instant::now();
        let mut resize_debouncer = ResizeDebouncer::new(100);
        let mut file_watcher = if model.watch_enabled {
            match self.make_file_watcher() {
                Ok(watcher) => Some(watcher),
                Err(err) => {
                    model.watch_enabled = false;
                    model.show_toast(ToastLevel::Warning, format!("Watch unavailable: {err}"));
                    crate::perf::log_event(
                        "watcher.error",
                        format!("failed path={} err={err}", model.file_path.display()),
                    );
                    None
                }
            }
        } else {
            None
        };
        let mut frame_idx: u64 = 0;
        let mut needs_render = true;

        loop {
            if model.expire_toast(Instant::now()) {
                needs_render = true;
            }

            let was_settling = model.is_image_scroll_settling();
            model.tick_image_scroll_cooldown();
            if was_settling && !model.is_image_scroll_settling() {
                // Repaint once after scroll placeholders expire to restore inline images.
                needs_render = true;
                crate::perf::log_event("image.scroll.settled", format!("frame={}", frame_idx));
            }

            let now_ms = start.elapsed().as_millis() as u64;

            if let Some((width, height)) = resize_debouncer.take_ready(now_ms) {
                crate::perf::log_event(
                    "event.resize.apply",
                    format!("frame={} width={} height={}", frame_idx, width, height),
                );
                *model = update(std::mem::take(model), Message::Resize(width, height));
                needs_render = true;
            }

            if model.watch_enabled
                && file_watcher
                    .as_mut()
                    .is_some_and(FileWatcher::take_change_ready)
            {
                *model = update(std::mem::take(model), Message::FileChanged);
                if let Err(err) = model.reload_from_disk() {
                    model.show_toast(ToastLevel::Error, format!("Reload failed: {err}"));
                    crate::perf::log_event(
                        "reload.error",
                        format!("failed path={} err={err}", model.file_path.display()),
                    );
                } else {
                    model.show_toast(ToastLevel::Info, "File changed, reloaded");
                }
                needs_render = true;
            }

            model.set_resize_pending(resize_debouncer.is_pending());

            // Handle events
            let poll_ms = if needs_render {
                0
            } else if resize_debouncer.is_pending() {
                10
            } else {
                250
            };
            if event::poll(std::time::Duration::from_millis(poll_ms))? {
                let msg = self.handle_event(event::read()?, model, now_ms, &mut resize_debouncer);
                if let Some(msg) = msg {
                    crate::perf::log_event("event.message", format!("frame={} msg={msg:?}", frame_idx));
                    let side_msg = msg.clone();
                    *model = update(std::mem::take(model), msg);
                    self.handle_message_side_effects(model, &mut file_watcher, &side_msg);
                    needs_render = true;
                }

                // Coalesce key repeat bursts into a single render.
                let mut drained = 0_u32;
                while event::poll(std::time::Duration::from_millis(0))? {
                    let msg =
                        self.handle_event(event::read()?, model, now_ms, &mut resize_debouncer);
                    if let Some(msg) = msg {
                        drained += 1;
                        let side_msg = msg.clone();
                        *model = update(std::mem::take(model), msg);
                        self.handle_message_side_effects(model, &mut file_watcher, &side_msg);
                        needs_render = true;
                    }
                }
                if drained > 0 {
                    crate::perf::log_event(
                        "event.drain",
                        format!("frame={} drained={}", frame_idx, drained),
                    );
                }
            }

            if needs_render {
                frame_idx += 1;

                // Load images near viewport before rendering (skip during active resize)
                let load_start = std::time::Instant::now();
                model.load_nearby_images();
                model.ensure_highlight_overscan();
                crate::perf::log_event(
                    "frame.prep",
                    format!(
                        "frame={} prep_ms={:.3} viewport={}..{} resize_pending={}",
                        frame_idx,
                        load_start.elapsed().as_secs_f64() * 1000.0,
                        model.viewport.offset(),
                        model.viewport.offset() + model.viewport.height() as usize,
                        resize_debouncer.is_pending()
                    ),
                );

                // Render
                let draw_start = std::time::Instant::now();
                terminal.draw(|frame| self.view(model, frame))?;
                crate::perf::log_event(
                    "frame.draw",
                    format!(
                        "frame={} draw_ms={:.3}",
                        frame_idx,
                        draw_start.elapsed().as_secs_f64() * 1000.0
                    ),
                );
                needs_render = false;
            }

            if model.should_quit {
                break;
            }
        }
        Ok(())
    }

    fn handle_event(
        &self,
        event: Event,
        model: &Model,
        now_ms: u64,
        resize_debouncer: &mut ResizeDebouncer,
    ) -> Option<Message> {
        match event {
            Event::Key(key) => self.handle_key(key, model),
            Event::Mouse(mouse) => self.handle_mouse(mouse, model),
            Event::Resize(w, h) => {
                crate::perf::log_event("event.resize.queue", format!("width={} height={}", w, h));
                resize_debouncer.queue(w, h, now_ms);
                None
            }
            _ => None,
        }
    }

    fn handle_mouse(&self, mouse: MouseEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            return None;
        }

        if model.toc_visible {
            let total_area = Rect::new(
                0,
                0,
                model.viewport.width(),
                model.viewport.height().saturating_add(1),
            );
            let chunks = crate::ui::split_main_columns(total_area);
            let toc_area = chunks[0];
            let in_toc = mouse.column >= toc_area.x
                && mouse.column < toc_area.x + toc_area.width
                && mouse.row >= toc_area.y
                && mouse.row < toc_area.y + toc_area.height;

            if in_toc {
                match mouse.kind {
                    MouseEventKind::Up(MouseButton::Left) => {
                        let headings_len = model.document.headings().len();
                        if headings_len == 0 {
                            return None;
                        }
                        if mouse.row <= toc_area.y
                            || mouse.row >= toc_area.y + toc_area.height.saturating_sub(1)
                        {
                            return None;
                        }
                        let inner_height = toc_area.height.saturating_sub(2) as usize;
                        if inner_height == 0 {
                            return None;
                        }
                        let max_start = headings_len.saturating_sub(inner_height);
                        let start = model.toc_scroll_offset.min(max_start);
                        let rel_row = (mouse.row - toc_area.y - 1) as usize;
                        let idx = start + rel_row;
                        if idx < headings_len {
                            return Some(Message::TocClick(idx));
                        }
                        return None;
                    }
                    MouseEventKind::ScrollDown => return Some(Message::TocScrollDown),
                    MouseEventKind::ScrollUp => return Some(Message::TocScrollUp),
                    _ => {}
                }
            }
        }

        match mouse.kind {
            MouseEventKind::ScrollDown => {
                if model.viewport.can_scroll_down() {
                    Some(Message::ScrollDown(3))
                } else {
                    None
                }
            }
            MouseEventKind::ScrollUp => {
                if model.viewport.can_scroll_up() {
                    Some(Message::ScrollUp(3))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn handle_key(&self, key: event::KeyEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::F(1) => Some(Message::HideHelp),
                _ => None,
            };
        }

        if let Some(active_query) = model.search_query.as_ref() {
            return match key.code {
                KeyCode::Esc => Some(Message::ClearSearch),
                KeyCode::Enter => Some(Message::NextMatch),
                KeyCode::Backspace => {
                    let mut next = active_query.clone();
                    next.pop();
                    Some(Message::SearchInput(next))
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    let mut next = active_query.clone();
                    next.push(c);
                    Some(Message::SearchInput(next))
                }
                _ => None,
            };
        }

        // Handle TOC-focused navigation
        if model.toc_focused && model.toc_visible {
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(Message::TocDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Message::TocUp),
                KeyCode::Enter | KeyCode::Char(' ') => Some(Message::TocSelect),
                KeyCode::Char('h') | KeyCode::Left => Some(Message::TocCollapse),
                KeyCode::Char('l') | KeyCode::Right => Some(Message::TocExpand),
                KeyCode::Tab => Some(Message::SwitchFocus),
                KeyCode::Char('t') => Some(Message::ToggleToc),
                KeyCode::Char('q') => Some(Message::Quit),
                KeyCode::Esc => Some(Message::SwitchFocus),
                _ => None,
            };
        }

        // Normal key handling
        match key.code {
            // Navigation
            KeyCode::Char('j') | KeyCode::Down => {
                if model.viewport.can_scroll_down() {
                    Some(Message::ScrollDown(1))
                } else {
                    None
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if model.viewport.can_scroll_up() {
                    Some(Message::ScrollUp(1))
                } else {
                    None
                }
            }
            KeyCode::Char(' ') | KeyCode::PageDown => {
                if model.viewport.can_scroll_down() {
                    Some(Message::PageDown)
                } else {
                    None
                }
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                if model.viewport.can_scroll_up() {
                    Some(Message::PageUp)
                } else {
                    None
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_down() {
                    Some(Message::HalfPageDown)
                } else {
                    None
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_up() {
                    Some(Message::HalfPageUp)
                } else {
                    None
                }
            }
            KeyCode::Char('g') | KeyCode::Home => Some(Message::GoToTop),
            KeyCode::Char('G') | KeyCode::End => Some(Message::GoToBottom),

            // TOC
            KeyCode::Char('t') => Some(Message::ToggleToc),
            KeyCode::Char('T') => Some(Message::ToggleTocFocus),
            KeyCode::Tab if model.toc_visible => Some(Message::SwitchFocus),

            // File
            KeyCode::Char('w') => Some(Message::ToggleWatch),
            KeyCode::Char('R') => Some(Message::ForceReload),
            KeyCode::Char('r') => Some(Message::ForceReload),
            KeyCode::Char('?') | KeyCode::F(1) => Some(Message::ToggleHelp),

            // Search
            KeyCode::Char('/') => Some(Message::StartSearch),
            KeyCode::Esc => Some(Message::ClearSearch),

            // Quit
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::Quit)
            }

            _ => None,
        }
    }

    fn view(&self, model: &mut Model, frame: &mut Frame) {
        crate::ui::render(model, frame);
    }
}

// Implement Default for Model to allow std::mem::take
impl Default for Model {
    fn default() -> Self {
        Self {
            document: Document::empty(),
            viewport: Viewport::new(80, 24, 0),
            file_path: PathBuf::new(),
            base_dir: PathBuf::from("."),
            toc_visible: false,
            toc_selected: None,
            toc_scroll_offset: 0,
            watch_enabled: false,
            config_global_path: None,
            config_local_path: None,
            help_visible: false,
            toast: None,
            search_query: None,
            search_matches: Vec::new(),
            search_match_index: None,
            search_allow_short: false,
            should_quit: false,
            toc_focused: false,
            image_protocols: HashMap::new(),
            original_images: HashMap::new(),
            picker: None,
            last_image_scale_width: 80,
            image_layout_heights: HashMap::new(),
            resize_pending: false,
            image_scroll_cooldown_ticks: 0,
            force_half_cell: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use tempfile::tempdir;
    use std::time::{Duration, Instant};

    fn create_test_model() -> Model {
        let doc = Document::parse("# Test\n\nHello world").unwrap();
        Model::new(PathBuf::from("test.md"), doc, (80, 24))
    }

    fn create_long_test_model() -> Model {
        // Create a document with 100 lines so we can test scrolling
        let mut md = String::from("# Test Document\n\n");
        for i in 1..=50 {
            md.push_str(&format!("Line {} of content.\n\n", i));
        }
        let doc = Document::parse(&md).unwrap();
        Model::new(PathBuf::from("test.md"), doc, (80, 24))
    }

    fn create_many_headings_model() -> Model {
        let mut md = String::new();
        for i in 1..=20 {
            md.push_str(&format!("## Heading {}\n\nBody {}\n\n", i, i));
        }
        let doc = Document::parse(&md).unwrap();
        Model::new(PathBuf::from("test.md"), doc, (80, 8))
    }

    #[test]
    fn test_scroll_down_updates_viewport() {
        let model = create_long_test_model();
        let model = update(model, Message::ScrollDown(5));
        assert_eq!(model.viewport.offset(), 5);
    }

    #[test]
    fn test_scroll_up_updates_viewport() {
        let mut model = create_long_test_model();
        model.viewport.scroll_down(10);
        let model = update(model, Message::ScrollUp(3));
        assert_eq!(model.viewport.offset(), 7);
    }

    #[test]
    fn test_toggle_toc_changes_visibility() {
        let model = create_test_model();
        assert!(!model.toc_visible);

        let model = update(model, Message::ToggleToc);
        assert!(model.toc_visible);

        let model = update(model, Message::ToggleToc);
        assert!(!model.toc_visible);
    }

    #[test]
    fn test_toggle_toc_selects_first_entry() {
        let model = create_test_model();
        assert!(model.toc_selected.is_none());

        let model = update(model, Message::ToggleToc);
        assert_eq!(model.toc_selected, Some(0));
    }

    #[test]
    fn test_toggle_watch_changes_state() {
        let model = create_test_model();
        assert!(!model.watch_enabled);

        let model = update(model, Message::ToggleWatch);
        assert!(model.watch_enabled);
    }

    #[test]
    fn test_force_reload_reloads_document_from_disk() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("doc.md");
        std::fs::write(&file_path, "# One\n\nalpha").unwrap();

        let doc = Document::parse_with_layout("# One\n\nalpha", 80).unwrap();
        let mut model = Model::new(file_path.clone(), doc, (80, 24));

        std::fs::write(&file_path, "# Two\n\nbeta\n\nmore").unwrap();
        model.reload_from_disk().unwrap();

        assert!(model.document.source().contains("# Two"));
        assert!(model.document.line_count() >= 3);
    }

    #[test]
    fn test_force_reload_message_triggers_reload_side_effect() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("doc.md");
        std::fs::write(&file_path, "# One\n\nalpha").unwrap();
        let doc = Document::parse_with_layout("# One\n\nalpha", 80).unwrap();
        let mut model = Model::new(file_path.clone(), doc, (80, 24));
        let app = App::new(file_path.clone());
        let mut watcher = None;

        std::fs::write(&file_path, "# Updated\n\nbeta").unwrap();
        model = update(model, Message::ForceReload);
        app.handle_message_side_effects(&mut model, &mut watcher, &Message::ForceReload);

        assert!(model.document.source().contains("# Updated"));
    }

    #[test]
    fn test_toggle_help_changes_visibility() {
        let model = create_test_model();
        assert!(!model.help_visible);

        let model = update(model, Message::ToggleHelp);
        assert!(model.help_visible);

        let model = update(model, Message::HideHelp);
        assert!(!model.help_visible);
    }

    #[test]
    fn test_toast_lifecycle() {
        let mut model = create_test_model();
        model.show_toast(ToastLevel::Warning, "watch failed");
        let (msg, level) = model.active_toast().expect("toast should be set");
        assert_eq!(msg, "watch failed");
        assert_eq!(level, ToastLevel::Warning);
        assert!(!model.expire_toast(Instant::now()));
        assert!(model.expire_toast(Instant::now() + Duration::from_secs(5)));
        assert!(model.active_toast().is_none());
    }

    #[test]
    fn test_toc_down_scrolls_when_selection_hits_bottom_row() {
        let mut model = create_many_headings_model();
        model.toc_visible = true;
        model.toc_selected = Some(5);
        model.toc_scroll_offset = 0;

        let model = update(model, Message::TocDown);
        assert_eq!(model.toc_selected, Some(6));
        assert_eq!(model.toc_scroll_offset, 1);
    }

    #[test]
    fn test_toc_up_scrolls_when_selection_hits_top_row() {
        let mut model = create_many_headings_model();
        model.toc_visible = true;
        model.toc_selected = Some(3);
        model.toc_scroll_offset = 3;

        let model = update(model, Message::TocUp);
        assert_eq!(model.toc_selected, Some(2));
        assert_eq!(model.toc_scroll_offset, 2);
    }

    #[test]
    fn test_toc_auto_sync_selects_heading_near_viewport_top() {
        let mut model = create_many_headings_model();
        model = update(model, Message::ToggleToc);

        let target_idx = 8;
        let target_line = model.document.headings()[target_idx].line;
        model = update(model, Message::GoToLine(target_line));

        assert_eq!(model.toc_selected, Some(target_idx));
        assert_eq!(
            model.toc_scroll_offset,
            target_idx.min(model.max_toc_scroll_offset())
        );
    }

    #[test]
    fn test_toc_auto_sync_picks_previous_on_equal_distance() {
        let mut model = create_many_headings_model();
        model = update(model, Message::ToggleToc);

        let headings = model.document.headings();
        let between = (headings[5].line + headings[6].line) / 2;
        model = update(model, Message::GoToLine(between));

        assert_eq!(model.toc_selected, Some(5));
    }

    #[test]
    fn test_quit_sets_should_quit() {
        let model = create_test_model();
        let model = update(model, Message::Quit);
        assert!(model.should_quit);
    }

    #[test]
    fn test_resize_updates_viewport() {
        let model = create_test_model();
        let model = update(model, Message::Resize(120, 40));
        assert_eq!(model.viewport.width(), 120);
        assert_eq!(model.viewport.height(), 39); // -1 for status bar
    }

    #[test]
    fn test_resize_reflows_document_using_content_width() {
        let md = "This is a line that should wrap after resize because the content area is narrower.";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
        let model = update(model, Message::Resize(20, 24));
        let lines = model.document.visible_lines(0, 20);
        let paragraph_lines: Vec<_> = lines
            .iter()
            .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
            .collect();
        assert!(paragraph_lines.len() > 1, "expected wrapped paragraph lines");
        for line in paragraph_lines {
            assert!(
                line.content().len() <= 18,
                "line should wrap to content width (20 - 2 padding): {}",
                line.content()
            );
        }
    }

    #[test]
    fn test_toggle_toc_reflows_document_to_narrower_width() {
        let md = "This is a long paragraph line used to verify that enabling TOC reduces wrapping width and forces narrower rendered lines in the document pane.";
        let doc = Document::parse_with_layout(md, 80).unwrap();
        let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
        let before_max = model
            .document
            .visible_lines(0, 50)
            .iter()
            .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
            .map(|l| l.content().chars().count())
            .max()
            .unwrap_or(0);

        let model = update(model, Message::ToggleToc);
        let after_max = model
            .document
            .visible_lines(0, 50)
            .iter()
            .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
            .map(|l| l.content().chars().count())
            .max()
            .unwrap_or(0);
        assert!(
            after_max < before_max,
            "expected narrower lines when TOC is visible (before={}, after={})",
            before_max,
            after_max
        );
    }

    #[test]
    fn test_resize_with_toc_visible_reflows_using_toc_width() {
        let md = "A long paragraph line used to verify wrapping width honors the visible TOC pane.";
        let doc = Document::parse_with_layout(md, 100).unwrap();
        let mut model = Model::new(PathBuf::from("test.md"), doc, (100, 24));
        model = update(model, Message::ToggleToc);
        model = update(model, Message::Resize(60, 24));

        let expected_max = crate::ui::document_content_width(60, true) as usize;
        for line in model.document.visible_lines(0, 50) {
            if *line.line_type() == crate::document::LineType::Paragraph {
                assert!(
                    line.content().chars().count() <= expected_max,
                    "line exceeds TOC-aware width: {} > {} ({})",
                    line.content().chars().count(),
                    expected_max,
                    line.content()
                );
            }
        }
    }

    #[test]
    fn test_go_to_top() {
        let mut model = create_test_model();
        model.viewport.scroll_down(100);
        let model = update(model, Message::GoToTop);
        assert_eq!(model.viewport.offset(), 0);
    }

    #[test]
    fn test_switch_focus_toggles_between_toc_and_document() {
        let mut model = create_test_model();
        model.toc_visible = true;
        assert!(!model.toc_focused);

        let model = update(model, Message::SwitchFocus);
        assert!(model.toc_focused);

        let model = update(model, Message::SwitchFocus);
        assert!(!model.toc_focused);
    }

    #[test]
    fn test_switch_focus_does_nothing_when_toc_hidden() {
        let model = create_test_model();
        assert!(!model.toc_visible);
        assert!(!model.toc_focused);

        let model = update(model, Message::SwitchFocus);
        assert!(!model.toc_focused);
    }

    #[test]
    fn test_ensure_highlight_overscan_highlights_visible_code() {
        let doc = Document::parse_with_layout("```rust\nfn main() {}\n```", 80).unwrap();
        let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
        model.ensure_highlight_overscan();

        let lines = model.document.visible_lines(0, 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("fn main"))
            .expect("code line missing");
        let spans = code_line.spans().expect("code spans missing");
        assert!(spans.iter().any(|s| s.style().fg.is_some()));
    }

    #[test]
    fn test_ensure_highlight_overscan_still_highlights_while_scrolling() {
        let doc = Document::parse_with_layout("Lead line\n\n```rust\nfn main() {}\n```", 80).unwrap();
        let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

        // Highlighting should still run while actively scrolling.
        model = update(model, Message::ScrollDown(1));

        model.ensure_highlight_overscan();
        let lines = model.document.visible_lines(model.viewport.offset(), 10);
        let code_line = lines
            .iter()
            .find(|l| l.content().contains("fn main"))
            .expect("code line missing");
        let spans = code_line.spans().expect("code spans missing");
        assert!(spans.iter().any(|s| s.style().fg.is_some()));
    }

    #[test]
    fn test_search_input_finds_matches_and_jumps_to_first() {
        let doc = Document::parse("alpha\n\nbeta\n\nalpha again").unwrap();
        let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

        let model = update(model, Message::StartSearch);
        let model = update(model, Message::SearchInput("alpha".to_string()));

        assert_eq!(model.search_match_count(), 2);
        assert_eq!(model.current_search_match(), Some((1, 2)));
        assert_eq!(model.viewport.offset(), 0);
    }

    #[test]
    fn test_next_match_wraps() {
        let doc = Document::parse("alpha\n\nbeta\n\nalpha again").unwrap();
        let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
        let model = update(model, Message::StartSearch);
        let model = update(model, Message::SearchInput("alpha".to_string()));

        let model = update(model, Message::NextMatch);
        assert_eq!(model.current_search_match(), Some((2, 2)));

        let model = update(model, Message::NextMatch);
        assert_eq!(model.current_search_match(), Some((1, 2)));
    }

    #[test]
    fn test_short_query_does_not_auto_search_until_enter() {
        let doc = Document::parse("alpha\n\nbeta\n\natom").unwrap();
        let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
        let model = update(model, Message::StartSearch);
        let model = update(model, Message::SearchInput("a".to_string()));
        assert_eq!(model.search_match_count(), 0);

        let model = update(model, Message::NextMatch);
        assert!(model.search_match_count() > 0);
    }

    #[test]
    fn test_search_mode_char_input_appends_query() {
        let app = App::new(PathBuf::from("test.md"));
        let mut model = create_test_model();
        model = update(model, Message::StartSearch);
        model = update(model, Message::SearchInput("a".to_string()));

        let msg = app.handle_key(event::KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE), &model);
        assert_eq!(msg, Some(Message::SearchInput("an".to_string())));
    }

    #[test]
    fn test_search_mode_enter_moves_to_next_match() {
        let app = App::new(PathBuf::from("test.md"));
        let mut model = create_test_model();
        model = update(model, Message::StartSearch);
        model = update(model, Message::SearchInput("test".to_string()));

        let msg = app.handle_key(event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE), &model);
        assert_eq!(msg, Some(Message::NextMatch));
    }

    #[test]
    fn test_toc_focus_space_selects_heading() {
        let app = App::new(PathBuf::from("test.md"));
        let mut model = create_test_model();
        model.toc_visible = true;
        model.toc_focused = true;
        model.toc_selected = Some(0);

        let msg = app.handle_key(event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE), &model);
        assert_eq!(msg, Some(Message::TocSelect));
    }

    #[test]
    fn test_question_mark_opens_help_when_not_searching() {
        let app = App::new(PathBuf::from("test.md"));
        let model = create_test_model();

        let msg = app.handle_key(event::KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT), &model);
        assert_eq!(msg, Some(Message::ToggleHelp));
    }

    #[test]
    fn test_help_mode_esc_closes_help() {
        let app = App::new(PathBuf::from("test.md"));
        let mut model = create_test_model();
        model.help_visible = true;

        let msg = app.handle_key(event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE), &model);
        assert_eq!(msg, Some(Message::HideHelp));
    }

    #[test]
    fn test_resize_debouncer_waits_for_quiet_period() {
        let mut debouncer = ResizeDebouncer::new(100);
        debouncer.queue(120, 40, 0);

        assert!(debouncer.take_ready(50).is_none());
        assert_eq!(debouncer.take_ready(100), Some((120, 40)));
    }

    #[test]
    fn test_resize_debouncer_uses_latest_size() {
        let mut debouncer = ResizeDebouncer::new(100);
        debouncer.queue(120, 40, 0);
        debouncer.queue(140, 50, 20);

        assert!(debouncer.take_ready(80).is_none());
        assert_eq!(debouncer.take_ready(120), Some((140, 50)));
    }

    #[test]
    #[ignore = "performance test; run with cargo test paging_perf_test_rendering -- --ignored --nocapture"]
    fn paging_perf_test_rendering() {
        let md = include_str!("../test-rendering.md");
        let doc = Document::parse_with_layout(md, 120).unwrap();
        let mut model = Model::new(PathBuf::from("test-rendering.md"), doc, (120, 40));
        model.ensure_highlight_overscan();

        let backend = TestBackend::new(120, 40);
        let mut terminal = Terminal::new(backend).unwrap();

        let mut frame_times = Vec::<Duration>::new();
        let mut pages = 0usize;

        // Warm-up first frame; measure paging renders only.
        terminal
            .draw(|frame| crate::ui::render(&mut model, frame))
            .unwrap();

        while model.viewport.can_scroll_down() {
            model = update(model, Message::PageDown);
            model.ensure_highlight_overscan();

            let t0 = Instant::now();
            terminal
                .draw(|frame| crate::ui::render(&mut model, frame))
                .unwrap();
            frame_times.push(t0.elapsed());

            pages += 1;
            assert!(pages < 10_000, "paging loop runaway");
        }

        assert!(!frame_times.is_empty());
        let total = frame_times.iter().copied().sum::<Duration>();
        let max = frame_times.iter().copied().max().unwrap_or_default();

        let total_ms = total.as_secs_f64() * 1000.0;
        let max_ms = max.as_secs_f64() * 1000.0;

        eprintln!(
            "[perf:paging] frames={} total={:.2}ms max={:.2}ms",
            frame_times.len(),
            total_ms,
            max_ms
        );

        let max_limit_ms = std::env::var("GANDER_PERF_MAX_FRAME_MS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(50.0);
        let total_limit_ms = std::env::var("GANDER_PERF_TOTAL_MS")
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(1500.0);

        assert!(
            max_ms <= max_limit_ms,
            "max frame time {:.2}ms exceeded {:.2}ms",
            max_ms,
            max_limit_ms
        );
        assert!(
            total_ms <= total_limit_ms,
            "total frame time {:.2}ms exceeded {:.2}ms",
            total_ms,
            total_limit_ms
        );
    }
}
