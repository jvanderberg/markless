use std::io::{Write, stdout};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use ratatui::DefaultTerminal;

use crate::app::{App, Message, Model, ToastLevel, update};
use crate::watcher::FileWatcher;

pub(super) struct ResizeDebouncer {
    delay_ms: u64,
    pending: Option<(u16, u16, u64)>,
}

impl ResizeDebouncer {
    pub(super) const fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: None,
        }
    }

    pub(super) const fn queue(&mut self, width: u16, height: u16, now_ms: u64) {
        self.pending = Some((width, height, now_ms));
    }

    pub(super) fn take_ready(&mut self, now_ms: u64) -> Option<(u16, u16)> {
        let (width, height, queued_at) = self.pending?;
        if now_ms.saturating_sub(queued_at) >= self.delay_ms {
            self.pending = None;
            Some((width, height))
        } else {
            None
        }
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

pub(super) struct BrowseDebouncer {
    delay_ms: u64,
    pending: Option<(usize, u64)>,
}

impl BrowseDebouncer {
    pub(super) const fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: None,
        }
    }

    pub(super) const fn queue(&mut self, idx: usize, now_ms: u64) {
        self.pending = Some((idx, now_ms));
    }

    pub(super) fn take_ready(&mut self, now_ms: u64) -> Option<usize> {
        let (idx, queued_at) = self.pending?;
        if now_ms.saturating_sub(queued_at) >= self.delay_ms {
            self.pending = None;
            Some(idx)
        } else {
            None
        }
    }

    pub(super) const fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) const fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

impl App {
    /// Run the main event loop.
    ///
    /// # Errors
    ///
    /// Returns an error if terminal initialization, document parsing,
    /// or the event loop encounters an I/O or parsing failure.
    pub fn run(&mut self) -> Result<()> {
        let _run_scope = crate::perf::scope("app.run.total");

        // Create image picker BEFORE initializing terminal (queries stdio)
        let picker = if self.images_enabled {
            let picker_scope = crate::perf::scope("app.create_picker");
            let picker = crate::image::create_picker(self.image_mode);
            drop(picker_scope);
            picker
        } else {
            None
        };

        // Determine the file to load (may be overridden in browse mode)
        let initial_file = if self.browse_mode {
            Self::find_first_viewable_file(&self.file_path)
        } else {
            Some(self.file_path.clone())
        };

        // Initialize terminal
        let init_scope = crate::perf::scope("app.ratatui_init");
        let mut terminal = ratatui::try_init()
            .context("Failed to initialize terminal — markless requires an interactive terminal")?;
        let size = terminal.size()?;
        drop(init_scope);

        // Load the document
        let read_scope = crate::perf::scope("app.read_file");
        let toc_visible = self.toc_visible || self.browse_mode;
        let terminal_content_width = crate::ui::document_content_width(size.width, toc_visible);
        let layout_width = match self.wrap_width {
            Some(w) if w > 0 => terminal_content_width.min(w),
            _ => terminal_content_width,
        };
        crate::perf::log_event(
            "init.layout",
            format!(
                "terminal={}x{} toc_visible={} content_w={} wrap_width={:?} layout_width={}",
                size.width,
                size.height,
                toc_visible,
                terminal_content_width,
                self.wrap_width,
                layout_width
            ),
        );
        let (document, effective_file) = if let Some(ref file) = initial_file {
            let raw_bytes = std::fs::read(file)?;
            let doc = crate::document::prepare_document_from_bytes(file, raw_bytes, layout_width);
            (doc, file.clone())
        } else {
            // No viewable file found; show empty document
            (crate::document::Document::empty(), self.file_path.clone())
        };
        drop(read_scope);

        // Create initial model
        let mut model =
            Model::new(effective_file, document, (size.width, size.height)).with_picker(picker);
        model.watch_enabled = self.watch_enabled;
        model.toc_visible = toc_visible;
        model.image_mode = self.image_mode;
        model.images_enabled = self.images_enabled;
        model.wrap_width = self.wrap_width;
        model
            .config_global_path
            .clone_from(&self.config_global_path);
        model.config_local_path.clone_from(&self.config_local_path);

        // Initialize browse mode
        if self.browse_mode {
            model.browse_mode = true;
            model.toc_focused = true;
            let browse_dir = self.file_path.clone();
            if let Err(err) = model.load_directory(&browse_dir) {
                model.show_toast(ToastLevel::Warning, format!("Browse failed: {err}"));
            } else if let Some(ref file) = initial_file {
                // Highlight the loaded file in the listing (compare by name
                // since load_directory canonicalizes paths)
                if let Some(name) = file.file_name() {
                    let name = name.to_string_lossy();
                    if let Some(idx) = model.browse_entries.iter().position(|e| e.name == *name) {
                        model.toc_selected = Some(idx);
                    }
                }
            }
        }

        // Pre-load images from the document
        let images_scope = crate::perf::scope("app.load_nearby_images.initial");
        model.load_nearby_images();
        drop(images_scope);
        model.ensure_hex_overscan();
        model.ensure_highlight_overscan();

        // Main loop
        let result = Self::event_loop(&mut terminal, &mut model);

        // Restore terminal
        let _ = execute!(stdout(), DisableMouseCapture);
        ratatui::restore();

        result
    }

    /// Find the first viewable file in a directory.
    fn find_first_viewable_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return None;
        };
        let mut files: Vec<_> = entries
            .filter_map(std::result::Result::ok)
            .filter(|e| {
                e.file_type().ok().is_some_and(|ft| ft.is_file())
                    && !e.file_name().to_string_lossy().starts_with('.')
            })
            .map(|e| e.path())
            .collect();
        files.sort();
        // Prefer markdown files
        if let Some(md) = files.iter().find(|f| {
            f.extension()
                .is_some_and(|ext| ext == "md" || ext == "markdown")
        }) {
            return Some(md.clone());
        }
        files.into_iter().next()
    }

    const fn update_browse_debouncer(
        model: &Model,
        msg: &Message,
        now_ms: u64,
        debouncer: &mut BrowseDebouncer,
    ) {
        if !model.browse_mode {
            return;
        }
        match msg {
            Message::TocUp | Message::TocDown | Message::TocScrollUp | Message::TocScrollDown => {
                if let Some(sel) = model.toc_selected {
                    debouncer.queue(sel, now_ms);
                }
            }
            Message::TocSelect | Message::TocClick(_) | Message::TocExpand => {
                debouncer.cancel();
            }
            _ => {}
        }
    }

    fn event_loop(terminal: &mut DefaultTerminal, model: &mut Model) -> Result<()> {
        let start = Instant::now();
        let mut resize_debouncer = ResizeDebouncer::new(100);
        let mut file_watcher = if model.watch_enabled {
            match Self::make_file_watcher(&model.file_path) {
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
        let mut watched_path = model.file_path.clone();
        let mut browse_debouncer = BrowseDebouncer::new(400);
        let mut frame_idx: u64 = 0;
        let mut needs_render = true;
        let mut mouse_capture_enabled = false;

        loop {
            // Recreate watcher if the viewed file changed (e.g. browse mode navigation)
            if model.watch_enabled && model.file_path != watched_path {
                match Self::make_file_watcher(&model.file_path) {
                    Ok(w) => file_watcher = Some(w),
                    Err(err) => {
                        crate::perf::log_event(
                            "watcher.rewatch.error",
                            format!("path={} err={err}", model.file_path.display()),
                        );
                    }
                }
                watched_path.clone_from(&model.file_path);
            }
            let should_enable_mouse = true;
            if should_enable_mouse != mouse_capture_enabled {
                if should_enable_mouse {
                    execute!(stdout(), EnableMouseCapture)?;
                    set_mouse_motion_tracking(true)?;
                } else {
                    set_mouse_motion_tracking(false)?;
                    execute!(stdout(), DisableMouseCapture)?;
                }
                mouse_capture_enabled = should_enable_mouse;
            }

            if model.expire_toast(Instant::now()) {
                needs_render = true;
            }

            let was_settling = model.is_image_scroll_settling();
            model.tick_image_scroll_cooldown();
            if was_settling && !model.is_image_scroll_settling() {
                // Repaint once after scroll placeholders expire to restore inline images.
                needs_render = true;
                crate::perf::log_event("image.scroll.settled", format!("frame={frame_idx}"));
            }

            let now_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);

            if let Some((width, height)) = resize_debouncer.take_ready(now_ms) {
                crate::perf::log_event(
                    "event.resize.apply",
                    format!("frame={frame_idx} width={width} height={height}"),
                );
                *model = update(std::mem::take(model), Message::Resize(width, height));
                needs_render = true;
            }

            // Auto-load file in browse mode after navigation settles
            if let Some(sel) = browse_debouncer.take_ready(now_ms)
                && model.browse_mode
                && let Some(entry) = model.browse_entries.get(sel).cloned()
                && !entry.is_dir
            {
                if let Err(err) = model.load_file(&entry.path) {
                    model.show_toast(ToastLevel::Error, format!("Open failed: {err}"));
                }
                needs_render = true;
            }

            if model.watch_enabled
                && file_watcher
                    .as_mut()
                    .is_some_and(FileWatcher::take_change_ready)
            {
                *model = update(std::mem::take(model), Message::FileChanged);
                Self::handle_message_side_effects(model, &mut file_watcher, &Message::FileChanged);
                needs_render = true;
            }

            model.set_resize_pending(resize_debouncer.is_pending());

            // Handle events
            let poll_ms = if needs_render {
                0
            } else if resize_debouncer.is_pending() || browse_debouncer.is_pending() {
                10
            } else {
                250
            };
            if event::poll(Duration::from_millis(poll_ms))? {
                // Refresh timestamp after poll wait so debouncers use accurate times.
                let event_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                let msg =
                    Self::handle_event(&event::read()?, model, event_ms, &mut resize_debouncer);
                if let Some(msg) = msg {
                    crate::perf::log_event(
                        "event.message",
                        format!("frame={frame_idx} msg={msg:?}"),
                    );
                    let side_msg = msg.clone();
                    *model = update(std::mem::take(model), msg);
                    Self::handle_message_side_effects(model, &mut file_watcher, &side_msg);
                    Self::update_browse_debouncer(
                        model,
                        &side_msg,
                        event_ms,
                        &mut browse_debouncer,
                    );
                    needs_render = true;
                }

                // Coalesce key repeat bursts into a single render.
                let mut drained = 0_u32;
                while event::poll(Duration::from_millis(0))? {
                    let drain_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
                    let msg =
                        Self::handle_event(&event::read()?, model, drain_ms, &mut resize_debouncer);
                    if let Some(msg) = msg {
                        drained += 1;
                        let side_msg = msg.clone();
                        *model = update(std::mem::take(model), msg);
                        Self::handle_message_side_effects(model, &mut file_watcher, &side_msg);
                        Self::update_browse_debouncer(
                            model,
                            &side_msg,
                            drain_ms,
                            &mut browse_debouncer,
                        );
                        needs_render = true;
                    }
                }
                if drained > 0 {
                    crate::perf::log_event(
                        "event.drain",
                        format!("frame={frame_idx} drained={drained}"),
                    );
                }
            }

            if needs_render {
                frame_idx += 1;

                // Load images near viewport before rendering (skip during active resize)
                let load_start = Instant::now();
                model.load_nearby_images();
                model.ensure_hex_overscan();
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
                let draw_start = Instant::now();
                terminal.draw(|frame| Self::view(model, frame))?;
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
        if mouse_capture_enabled {
            let _ = set_mouse_motion_tracking(false);
            let _ = execute!(stdout(), DisableMouseCapture);
        }
        Ok(())
    }
}

fn set_mouse_motion_tracking(enable: bool) -> std::io::Result<()> {
    // Request any-event mouse motion reporting (1003) with SGR encoding (1006).
    // This improves hover support in terminals like Ghostty.
    let mut out = stdout();
    if enable {
        out.write_all(b"\x1b[?1003h\x1b[?1006h")?;
    } else {
        out.write_all(b"\x1b[?1003l\x1b[?1006l")?;
    }
    out.flush()
}
