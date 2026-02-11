use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::Result;
use image::DynamicImage;
use ratatui_image::picker::{Picker, ProtocolType};

use crate::config::ImageMode;
use ratatui_image::protocol::StatefulProtocol;

use crate::document::Document;
use crate::editor::EditorBuffer;
use crate::image::ImageLoader;
use crate::ui::viewport::Viewport;

/// Hash a byte slice for content comparison.
pub(super) fn hash_bytes(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// Mermaid diagram display width as a percentage of the image target width.
///
/// Mermaid SVGs contain their own internal layout and render best at a
/// narrower width than regular images (which scale to fill available space).
/// 100 would match regular image width; 60 was chosen empirically as a
/// good balance between readability and not overwhelming the terminal.
const MERMAID_WIDTH_PERCENT: u32 = 60;

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

/// A directory entry shown in the browse-mode TOC.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// Display name (filename or "..")
    pub name: String,
    /// Full path to the entry
    pub path: PathBuf,
    /// Whether this entry is a directory
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionState {
    Pending,
    Dragging,
    Finalized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineSelection {
    pub anchor: usize,
    pub active: usize,
    pub state: SelectionState,
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
    /// Stores (protocol, `width_cols`, `height_rows`)
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
    /// Forced image rendering mode (overrides auto-detection)
    pub image_mode: Option<ImageMode>,
    /// Current line selection state (mouse drag)
    pub selection: Option<LineSelection>,
    /// Whether inline images are enabled
    pub images_enabled: bool,
    /// Optional maximum content wrap width in columns
    pub wrap_width: Option<u16>,
    /// Whether directory browse mode is active
    pub browse_mode: bool,
    /// Current directory being browsed
    pub browse_dir: PathBuf,
    /// Directory entries shown in browse-mode TOC
    pub browse_entries: Vec<DirEntry>,
    /// Whether the editor is active (edit mode vs view mode)
    pub editor_mode: bool,
    /// The editor text buffer (populated when entering edit mode)
    pub editor_buffer: Option<EditorBuffer>,
    /// Scroll offset for the editor viewport (line index of first visible line)
    pub editor_scroll_offset: usize,
    /// Hash of the file on disk when edit mode was entered (for conflict detection)
    pub editor_disk_hash: Option<u64>,
    /// Whether the file on disk has changed since edit mode was entered
    pub editor_disk_conflict: bool,
    /// Set after first save attempt when disk conflict detected; allows second save to force
    pub save_confirmed: bool,
    /// Set after first quit attempt with unsaved editor changes; allows second quit to proceed
    pub quit_confirmed: bool,
    /// Set after first Esc press with unsaved editor changes; allows second Esc to discard
    pub exit_confirmed: bool,
}

impl std::fmt::Debug for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Model")
            .field("file_path", &self.file_path)
            .field("toc_visible", &self.toc_visible)
            .field("watch_enabled", &self.watch_enabled)
            .field("editor_mode", &self.editor_mode)
            .finish_non_exhaustive()
    }
}

impl Model {
    /// Create a new model with default settings.
    pub fn new(file_path: PathBuf, document: Document, terminal_size: (u16, u16)) -> Self {
        let total_lines = document.line_count();
        let base_dir = file_path
            .parent()
            .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);

        Self {
            document,
            viewport: Viewport::new(
                terminal_size.0,
                terminal_size.1.saturating_sub(1),
                total_lines,
            ),
            file_path,
            base_dir: base_dir.clone(),
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
            image_mode: None,
            selection: None,
            images_enabled: true,
            wrap_width: None,
            browse_mode: false,
            browse_dir: base_dir,
            browse_entries: Vec::new(),
            editor_mode: false,
            editor_buffer: None,
            editor_scroll_offset: 0,
            editor_disk_hash: None,
            editor_disk_conflict: false,
            save_confirmed: false,
            quit_confirmed: false,
            exit_confirmed: false,
        }
    }

    /// Set the image picker.
    #[must_use]
    pub fn with_picker(mut self, picker: Option<Picker>) -> Self {
        self.picker = picker;
        self
    }

    /// Whether mermaid diagrams should be rendered as images.
    ///
    /// True only when images are enabled and the terminal supports a real
    /// graphics protocol (Kitty, Sixel, iTerm2) — not half-block fallback.
    pub fn should_render_mermaid_as_images(&self) -> bool {
        if !self.images_enabled {
            return false;
        }
        let Some(picker) = &self.picker else {
            return false;
        };
        !matches!(picker.protocol_type(), ProtocolType::Halfblocks)
    }

    /// Load images that are near the viewport (lazy loading with lookahead).
    pub fn load_nearby_images(&mut self) {
        if self.resize_pending {
            crate::perf::log_event("image.load_nearby.skip", "resize_pending=true");
            return;
        }
        if !self.images_enabled {
            crate::perf::log_event("image.load_nearby.skip", "images_enabled=false");
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
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // target_width_cols is always positive and within u16 range (65% of a u16).
        let target_width_cols = (f32::from(current_width) * 0.65) as u16;
        let target_width_px = u32::from(target_width_cols) * u32::from(font_size.0);

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
                // Try to get original image from cache, or load/render
                let original: Option<DynamicImage> =
                    if let Some(img) = self.original_images.get(&src) {
                        Some(img.clone())
                    } else if src.starts_with("mermaid://") {
                        let mermaid_width_px = target_width_px * MERMAID_WIDTH_PERCENT / 100;
                        self.document
                            .mermaid_sources()
                            .get(&src)
                            .and_then(|mermaid_text| {
                                crate::mermaid::render_to_image(mermaid_text, mermaid_width_px)
                                    .inspect_err(|e| {
                                        crate::perf::log_event(
                                            "mermaid.render.error",
                                            format!("src={src} err={e}"),
                                        );
                                    })
                                    .ok()
                            })
                            .inspect(|img| {
                                self.original_images.insert(src.clone(), img.clone());
                            })
                    } else if let Some(img) = loader.load_sync(&src) {
                        self.original_images.insert(src.clone(), img.clone());
                        Some(img)
                    } else {
                        None
                    };

                if let Some(img) = original {
                    // Scale to fit target width, preserving aspect ratio.
                    let scale = f64::from(target_width_px) / f64::from(img.width());
                    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                    // Scaled image height is always positive and well within u32 range.
                    let scaled_height_px = (f64::from(img.height()) * scale) as u32;

                    let mut scaled = img.resize(
                        target_width_px,
                        scaled_height_px,
                        if use_halfblocks {
                            image::imageops::FilterType::CatmullRom
                        } else {
                            image::imageops::FilterType::Nearest
                        },
                    );
                    if quantize_halfblocks {
                        scaled = crate::image::quantize_to_ansi256(&scaled);
                    }

                    let protocol = picker.new_resize_protocol(scaled);
                    let (width_cols, height_rows) =
                        protocol_render_size(&protocol, target_width_cols);
                    self.image_protocols
                        .insert(src.clone(), (protocol, width_cols, height_rows));
                    crate::perf::log_event(
                        "image.load_nearby.protocol",
                        format!(
                            "src={src} width_cols={width_cols} height_rows={height_rows} width_changed={width_changed} halfblocks={use_halfblocks} ansi256={quantize_halfblocks}"
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

    /// Ensure hex lines are cached for the current viewport with overscan.
    pub fn ensure_hex_overscan(&mut self) {
        let height = self.viewport.height() as usize;
        let extra = height * 2;
        let start = self.viewport.offset().saturating_sub(extra);
        let end = (self.viewport.offset() + height + extra).min(self.document.line_count());
        self.document.ensure_hex_lines_for_range(start..end);
    }

    pub fn ensure_highlight_overscan(&mut self) {
        let height = self.viewport.height() as usize;
        let extra = height * 2;
        let start = self.viewport.offset().saturating_sub(extra);
        let end = (self.viewport.offset() + height + extra).min(self.document.line_count());
        self.document.ensure_highlight_for_range(start..end);
    }

    pub const fn tick_image_scroll_cooldown(&mut self) {
        self.image_scroll_cooldown_ticks = self.image_scroll_cooldown_ticks.saturating_sub(1);
    }

    pub const fn is_image_scroll_settling(&self) -> bool {
        self.image_scroll_cooldown_ticks > 0
    }

    pub(super) const fn bump_image_scroll_cooldown(&mut self) {
        self.image_scroll_cooldown_ticks = 3;
    }

    pub(super) const fn set_resize_pending(&mut self, pending: bool) {
        self.resize_pending = pending;
    }

    pub const fn search_match_count(&self) -> usize {
        self.search_matches.len()
    }

    pub fn current_search_match(&self) -> Option<(usize, usize)> {
        self.search_match_index
            .map(|idx| (idx + 1, self.search_matches.len()))
    }

    pub(super) fn layout_width(&self) -> u16 {
        let terminal_width =
            crate::ui::document_content_width(self.viewport.width(), self.toc_visible);
        match self.wrap_width {
            Some(w) if w > 0 => terminal_width.min(w),
            _ => terminal_width,
        }
    }

    pub(super) const fn toc_visible_rows(&self) -> usize {
        // TOC uses full frame height with a 1-cell border at top/bottom.
        self.viewport.height().saturating_sub(1) as usize
    }

    /// Number of entries in the TOC pane (browse entries or headings).
    pub fn toc_entry_count(&self) -> usize {
        if self.browse_mode {
            self.browse_entries.len()
        } else {
            self.document.headings().len()
        }
    }

    pub(super) fn max_toc_scroll_offset(&self) -> usize {
        self.toc_entry_count()
            .saturating_sub(self.toc_visible_rows())
    }

    pub(super) fn sync_toc_to_viewport(&mut self) {
        let Some(selected) =
            closest_heading_to_line(self.document.headings(), self.viewport.offset())
        else {
            self.toc_selected = None;
            self.toc_scroll_offset = 0;
            return;
        };
        self.toc_selected = Some(selected);
        self.toc_scroll_offset = selected.min(self.max_toc_scroll_offset());
    }

    pub(super) fn reflow_layout(&mut self) {
        // Hex mode documents have fixed-width layout — no reflow needed.
        if self.document.is_hex_mode() {
            return;
        }
        let width = self.layout_width();
        let mermaid = self.should_render_mermaid_as_images();
        if let Ok(document) = Document::parse_with_all_options(
            self.document.source(),
            width,
            &self.image_layout_heights,
            mermaid,
        ) {
            self.document = document;
            self.viewport.set_total_lines(self.document.line_count());
            self.toc_scroll_offset = self.toc_scroll_offset.min(self.max_toc_scroll_offset());
            let allow_short = self.search_allow_short;
            refresh_search_matches(self, false, allow_short);
            self.clamp_selection();
        }
    }

    /// Scan a directory and populate `browse_entries`.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read or an entry's
    /// file type cannot be determined.
    pub fn load_directory(&mut self, dir: &Path) -> Result<()> {
        let dir = dir.canonicalize().unwrap_or_else(|_| dir.to_path_buf());
        self.browse_dir.clone_from(&dir);
        self.browse_entries.clear();

        // Add parent directory entry
        self.browse_entries.push(DirEntry {
            name: "..".to_string(),
            path: dir.parent().unwrap_or(&dir).to_path_buf(),
            is_dir: true,
        });

        let mut dirs = Vec::new();
        let mut files = Vec::new();

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            // Skip hidden files/dirs
            if name.starts_with('.') {
                continue;
            }
            let path = entry.path();
            let is_dir = entry.file_type()?.is_dir();
            if is_dir {
                dirs.push(DirEntry { name, path, is_dir });
            } else {
                files.push(DirEntry {
                    name,
                    path,
                    is_dir: false,
                });
            }
        }

        dirs.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        files.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

        self.browse_entries.extend(dirs);
        self.browse_entries.extend(files);
        self.toc_scroll_offset = 0;

        Ok(())
    }

    /// Build a `Document` from raw file bytes, respecting current mermaid and
    /// image-layout settings.
    fn document_from_bytes(&self, path: &Path, raw_bytes: Vec<u8>) -> Result<Document> {
        if crate::document::is_binary(&raw_bytes) || crate::document::is_image_file(path) {
            return Ok(crate::document::prepare_document_from_bytes(
                path,
                raw_bytes,
                self.layout_width(),
            ));
        }
        let text = match String::from_utf8(raw_bytes) {
            Ok(s) => s,
            Err(e) => e.to_string(),
        };
        let is_md = is_markdown_ext(&path.to_string_lossy());
        let content = crate::document::prepare_content(path, text.clone());
        let was_wrapped = content != text;
        // Use the markdown parser for .md files and for files that
        // prepare_content wrapped in code fences (code/csv/image).
        // Everything else is plain text — render verbatim.
        if is_md || was_wrapped {
            Document::parse_with_all_options(
                &content,
                self.layout_width(),
                &self.image_layout_heights,
                self.should_render_mermaid_as_images(),
            )
        } else {
            Ok(Document::from_plain_text(&content))
        }
    }

    /// Load a file into the document area.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or the document
    /// fails to parse.
    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let raw_bytes = std::fs::read(path)?;
        let document = self.document_from_bytes(path, raw_bytes)?;
        self.file_path = path.to_path_buf();
        self.base_dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
        self.document = document;

        // Clear image caches for old file
        self.image_protocols.clear();
        self.original_images.clear();
        self.image_layout_heights.clear();

        self.viewport.set_total_lines(self.document.line_count());
        self.viewport.go_to_top();
        self.toc_scroll_offset = self.toc_scroll_offset.min(self.max_toc_scroll_offset());
        let allow_short = self.search_allow_short;
        refresh_search_matches(self, false, allow_short);
        self.clamp_selection();
        Ok(())
    }

    /// Return the index and path of the first viewable file in `browse_entries`,
    /// preferring markdown files over other types.
    pub fn first_viewable_file_index(&self) -> Option<(usize, PathBuf)> {
        // Prefer markdown files
        if let Some((idx, entry)) = self
            .browse_entries
            .iter()
            .enumerate()
            .find(|(_, e)| !e.is_dir && is_markdown_ext(&e.name))
        {
            return Some((idx, entry.path.clone()));
        }
        // Fall back to first non-directory entry
        self.browse_entries
            .iter()
            .enumerate()
            .find(|(_, e)| !e.is_dir)
            .map(|(idx, e)| (idx, e.path.clone()))
    }

    /// Whether the editor has unsaved changes.
    pub fn editor_is_dirty(&self) -> bool {
        self.editor_mode
            && self
                .editor_buffer
                .as_ref()
                .is_some_and(crate::editor::EditorBuffer::is_dirty)
    }

    /// Hash the contents of a file on disk, returning `None` if the file can't be read.
    pub fn file_disk_hash(&self) -> Option<u64> {
        let bytes = std::fs::read(&self.file_path).ok()?;
        Some(hash_bytes(&bytes))
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

    pub const fn link_picker_active(&self) -> bool {
        !self.link_picker_items.is_empty()
    }

    pub(super) fn reload_from_disk(&mut self) -> Result<()> {
        let raw_bytes = std::fs::read(&self.file_path)?;
        let path = self.file_path.clone();
        let document = self.document_from_bytes(&path, raw_bytes)?;
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
        self.clamp_selection();
        if self.toc_visible && !self.toc_focused {
            self.sync_toc_to_viewport();
        }
        Ok(())
    }

    pub fn selection_range(&self) -> Option<std::ops::RangeInclusive<usize>> {
        let selection = self.selection?;
        let line_count = self.document.line_count();
        if line_count == 0 {
            return None;
        }
        let max = line_count.saturating_sub(1);
        let start = selection.anchor.min(selection.active).min(max);
        let end = selection.anchor.max(selection.active).min(max);
        Some(start..=end)
    }

    pub fn selected_text(&self) -> Option<(String, usize)> {
        let range = self.selection_range()?;
        let mut lines = Vec::new();
        for idx in range {
            if let Some(line) = self.document.line_at(idx) {
                let link_refs: Vec<_> = self
                    .document
                    .links()
                    .iter()
                    .filter(|link| link.line == idx && !link.url.starts_with("footnote:"))
                    .collect();
                if let Some(text) = clean_selected_line(line, &link_refs) {
                    lines.push(text);
                }
            }
        }
        if lines.is_empty() {
            return None;
        }
        let count = lines.len();
        Some((lines.join("\n"), count))
    }

    pub fn selection_dragging(&self) -> bool {
        self.selection
            .as_ref()
            .is_some_and(|sel| sel.state == SelectionState::Dragging)
    }

    pub const fn clear_selection(&mut self) {
        self.selection = None;
    }

    fn clamp_selection(&mut self) {
        let Some(selection) = self.selection else {
            return;
        };
        let line_count = self.document.line_count();
        if line_count == 0 {
            self.selection = None;
            return;
        }
        let max = line_count.saturating_sub(1);
        let clamped = LineSelection {
            anchor: selection.anchor.min(max),
            active: selection.active.min(max),
            state: selection.state,
        };
        self.selection = Some(clamped);
    }
}

fn protocol_render_size(
    protocol: &ratatui_image::protocol::StatefulProtocol,
    target_width_cols: u16,
) -> (u16, u16) {
    use ratatui::layout::Rect;
    use ratatui_image::Resize;
    let resize = if matches!(
        protocol.protocol_type(),
        ratatui_image::protocol::StatefulProtocolType::Halfblocks(_)
    ) {
        Resize::Scale(Some(image::imageops::FilterType::CatmullRom))
    } else {
        Resize::Scale(None)
    };
    let area = Rect::new(0, 0, target_width_cols, u16::MAX);
    let rect = protocol.size_for(resize, area);
    (rect.width.max(1), rect.height.max(1))
}

fn clean_selected_line(
    line: &crate::document::RenderedLine,
    links: &[&crate::document::LinkRef],
) -> Option<String> {
    use crate::document::LineType;

    let content = line.content();
    if *line.line_type() == LineType::CodeBlock {
        if content.starts_with('┌') || content.starts_with('└') {
            return None;
        }
        if let Some(stripped) = content.strip_prefix("│ ") {
            let stripped = stripped.strip_suffix(" │").unwrap_or(stripped);
            return Some(stripped.trim_end_matches(' ').to_string());
        }
        return Some(content.to_string());
    }
    if let Some(spans) = line.spans() {
        let mut out = String::new();
        let mut in_link = false;
        let mut link_idx = 0usize;
        for span in spans {
            if span.style().link {
                if !in_link {
                    if let Some(link) = links.get(link_idx) {
                        out.push_str(&link.url);
                    } else {
                        out.push_str(span.text());
                    }
                    link_idx += 1;
                    in_link = true;
                }
            } else {
                in_link = false;
                out.push_str(span.text());
            }
        }
        return Some(out);
    }
    Some(content.to_string())
}

pub(super) fn is_markdown_ext(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(_, ext)| {
        ext.eq_ignore_ascii_case("md") || ext.eq_ignore_ascii_case("markdown")
    })
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
            image_mode: None,
            selection: None,
            images_enabled: true,
            wrap_width: None,
            browse_mode: false,
            browse_dir: PathBuf::from("."),
            browse_entries: Vec::new(),
            editor_mode: false,
            editor_buffer: None,
            editor_scroll_offset: 0,
            editor_disk_hash: None,
            editor_disk_conflict: false,
            save_confirmed: false,
            quit_confirmed: false,
            exit_confirmed: false,
        }
    }
}
