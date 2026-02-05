use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::Result;
use image::DynamicImage;
use ratatui_image::picker::{Picker, ProtocolType};
use ratatui_image::protocol::StatefulProtocol;

use crate::document::Document;
use crate::image::ImageLoader;
use crate::ui::viewport::Viewport;

use super::update::{closest_heading_to_line, refresh_search_matches};

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
    /// URL currently hovered in the document pane (mouse capture mode)
    pub hovered_link_url: Option<String>,
    /// Pending visible-link picker items for quick follow (`o`)
    pub link_picker_items: Vec<crate::document::LinkRef>,
    toast: Option<Toast>,
    /// Current search query
    pub search_query: Option<String>,
    /// Rendered line indices that match the current search query
    pub(super) search_matches: Vec<usize>,
    /// Current selected match index inside `search_matches`
    pub(super) search_match_index: Option<usize>,
    /// Allow searching short (<3 char) queries after explicit Enter.
    pub(super) search_allow_short: bool,
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
    pub force_half_cell: bool,
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
            hovered_link_url: None,
            link_picker_items: Vec::new(),
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
        let images_to_process: Vec<_> = self
            .document
            .images()
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
                let original: Option<DynamicImage> = if let Some(img) = self.original_images.get(&src)
                {
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
                    let height_rows =
                        (scaled_height_px as f32 / font_size.1 as f32).ceil() as u16;

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

    pub(super) fn bump_image_scroll_cooldown(&mut self) {
        self.image_scroll_cooldown_ticks = 3;
    }

    pub(super) fn set_resize_pending(&mut self, pending: bool) {
        self.resize_pending = pending;
    }

    pub fn search_match_count(&self) -> usize {
        self.search_matches.len()
    }

    pub fn current_search_match(&self) -> Option<(usize, usize)> {
        self.search_match_index
            .map(|idx| (idx + 1, self.search_matches.len()))
    }

    pub(super) fn layout_width(&self) -> u16 {
        crate::ui::document_content_width(self.viewport.width(), self.toc_visible)
    }

    pub(super) fn toc_visible_rows(&self) -> usize {
        // TOC uses full frame height with a 1-cell border at top/bottom.
        self.viewport.height().saturating_sub(1) as usize
    }

    pub(super) fn max_toc_scroll_offset(&self) -> usize {
        self.document
            .headings()
            .len()
            .saturating_sub(self.toc_visible_rows())
    }

    pub(super) fn sync_toc_to_viewport(&mut self) {
        let Some(selected) = closest_heading_to_line(self.document.headings(), self.viewport.offset()) else {
            self.toc_selected = None;
            self.toc_scroll_offset = 0;
            return;
        };
        self.toc_selected = Some(selected);
        self.toc_scroll_offset = selected.min(self.max_toc_scroll_offset());
    }

    pub(super) fn reflow_layout(&mut self) {
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

    pub(super) fn show_toast(&mut self, level: ToastLevel, message: impl Into<String>) {
        self.toast = Some(Toast {
            level,
            message: message.into(),
            expires_at: Instant::now() + Duration::from_secs(4),
        });
    }

    pub(super) fn expire_toast(&mut self, now: Instant) -> bool {
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

    pub fn link_picker_active(&self) -> bool {
        !self.link_picker_items.is_empty()
    }

    pub(super) fn reload_from_disk(&mut self) -> Result<()> {
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
            hovered_link_url: None,
            link_picker_items: Vec::new(),
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
