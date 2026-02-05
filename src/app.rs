//! Application state and main event loop.
//!
//! This module implements The Elm Architecture (TEA):
//! - [`Model`]: The complete application state
//! - [`Message`]: All possible events and actions
//! - [`update`]: Pure function for state transitions
//! - [`App::run`]: Main event loop with rendering

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseEvent, MouseEventKind};
use image::DynamicImage;
use ratatui::{DefaultTerminal, Frame};
use ratatui_image::picker::Picker;
use ratatui_image::protocol::StatefulProtocol;

use crate::document::Document;
use crate::image::ImageLoader;
use crate::ui::viewport::Viewport;

/// The complete application state.
///
/// All state lives here - no global or scattered state.
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
    /// Whether file watching is enabled
    pub watch_enabled: bool,
    /// Current search query
    pub search_query: Option<String>,
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
            watch_enabled: false,
            search_query: None,
            should_quit: false,
            toc_focused: false,
            image_protocols: HashMap::new(),
            original_images: HashMap::new(),
            picker: None,
            last_image_scale_width: terminal_size.0,
            image_layout_heights: HashMap::new(),
            resize_pending: false,
            image_scroll_cooldown_ticks: 0,
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

                    let scaled = img.resize(
                        target_width_px,
                        scaled_height_px,
                        image::imageops::FilterType::Nearest,
                    );

                    // Calculate dimensions in terminal cells
                    let width_cols = target_width_cols;
                    let height_rows = (scaled_height_px as f32 / font_size.1 as f32).ceil() as u16;

                    let protocol = picker.new_resize_protocol(scaled);
                    self.image_protocols
                        .insert(src.clone(), (protocol, width_cols, height_rows));
                    crate::perf::log_event(
                        "image.load_nearby.protocol",
                        format!(
                            "src={} width_cols={} height_rows={} width_changed={}",
                            src,
                            width_cols,
                            height_rows,
                            width_changed
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
            if let Ok(document) = Document::parse_with_layout_and_image_heights(
                self.document.source(),
                self.viewport.width(),
                &current_layout_heights,
            ) {
                crate::perf::log_event(
                    "image.layout.reflow",
                    format!(
                        "old={} new={}",
                        self.image_layout_heights.len(),
                        current_layout_heights.len()
                    ),
                );
                self.document = document;
                self.viewport.set_total_lines(self.document.line_count());
                self.image_layout_heights = current_layout_heights;
            }
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
    /// Jump to selected TOC heading
    TocSelect,
    /// Collapse TOC entry
    TocCollapse,
    /// Expand TOC entry
    TocExpand,
    /// Switch focus between TOC and document
    SwitchFocus,

    // File watching
    /// Toggle file watching
    ToggleWatch,
    /// File changed externally, reload
    FileChanged,
    /// Force reload file
    ForceReload,

    // Search
    /// Start search mode
    StartSearch,
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
        }
        Message::ToggleTocFocus => {
            model.toc_visible = !model.toc_visible;
            model.toc_focused = model.toc_visible;
            if model.toc_visible && model.toc_selected.is_none() {
                model.toc_selected = Some(0);
            }
        }
        Message::TocUp => {
            if let Some(sel) = model.toc_selected {
                model.toc_selected = Some(sel.saturating_sub(1));
            }
        }
        Message::TocDown => {
            if let Some(sel) = model.toc_selected {
                let max = model.document.headings().len().saturating_sub(1);
                model.toc_selected = Some((sel + 1).min(max));
            }
        }
        Message::TocSelect => {
            if let Some(sel) = model.toc_selected {
                if let Some(heading) = model.document.headings().get(sel) {
                    model.viewport.go_to_line(heading.line);
                }
            }
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
        Message::FileChanged | Message::ForceReload => {
            // Reload is handled in the event loop (side effect)
            // The model update happens after reload
        }

        // Search
        Message::StartSearch => {
            model.search_query = Some(String::new());
        }
        Message::SearchInput(query) => {
            model.search_query = Some(query);
        }
        Message::NextMatch | Message::PrevMatch => {
            // TODO: Implement search navigation
        }
        Message::ClearSearch => {
            model.search_query = None;
        }

        // Window
        Message::Resize(width, height) => {
            model.viewport.resize(width, height.saturating_sub(1));
            if let Ok(document) = Document::parse_with_layout_and_image_heights(
                model.document.source(),
                model.viewport.width(),
                &model.image_layout_heights,
            ) {
                model.document = document;
                model.viewport.set_total_lines(model.document.line_count());
            }
        }
        Message::Redraw => {
            // No state change needed
        }

        // Application
        Message::Quit => {
            model.should_quit = true;
        }
    }
    model
}

/// Main application struct that owns the terminal and runs the event loop.
pub struct App {
    file_path: PathBuf,
    watch_enabled: bool,
    toc_visible: bool,
    force_half_cell: bool,
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
        let size = terminal.size()?;
        drop(_init_scope);

        let _parse_scope = crate::perf::scope("app.parse_with_layout");
        let document = Document::parse_with_layout(&content, size.width)?;
        drop(_parse_scope);

        // Create initial model
        let mut model = Model::new(self.file_path.clone(), document, (size.width, size.height))
            .with_picker(picker);
        model.watch_enabled = self.watch_enabled;
        model.toc_visible = self.toc_visible;

        // Pre-load images from the document
        let _images_scope = crate::perf::scope("app.load_nearby_images.initial");
        model.load_nearby_images();
        drop(_images_scope);
        model.ensure_highlight_overscan();

        // Main loop
        let result = self.event_loop(&mut terminal, &mut model);

        // Restore terminal
        ratatui::restore();

        result
    }

    fn event_loop(&self, terminal: &mut DefaultTerminal, model: &mut Model) -> Result<()> {
        let start = std::time::Instant::now();
        let mut resize_debouncer = ResizeDebouncer::new(100);
        let mut frame_idx: u64 = 0;
        let mut needs_render = true;

        loop {
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
                    *model = update(std::mem::take(model), msg);
                    needs_render = true;
                }

                // Coalesce key repeat bursts into a single render.
                let mut drained = 0_u32;
                while event::poll(std::time::Duration::from_millis(0))? {
                    let msg =
                        self.handle_event(event::read()?, model, now_ms, &mut resize_debouncer);
                    if let Some(msg) = msg {
                        drained += 1;
                        *model = update(std::mem::take(model), msg);
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
        // Handle TOC-focused navigation
        if model.toc_focused && model.toc_visible {
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(Message::TocDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Message::TocUp),
                KeyCode::Enter => Some(Message::TocSelect),
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

            // Search
            KeyCode::Char('/') => Some(Message::StartSearch),
            KeyCode::Char('n') => Some(Message::NextMatch),
            KeyCode::Char('N') => Some(Message::PrevMatch),
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
            watch_enabled: false,
            search_query: None,
            should_quit: false,
            toc_focused: false,
            image_protocols: HashMap::new(),
            original_images: HashMap::new(),
            picker: None,
            last_image_scale_width: 80,
            image_layout_heights: HashMap::new(),
            resize_pending: false,
            image_scroll_cooldown_ticks: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
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
