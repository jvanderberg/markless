use std::io::{stdout, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event;
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use ratatui::DefaultTerminal;

use crate::app::{update, App, Message, Model, ToastLevel};
use crate::watcher::FileWatcher;

pub(super) struct ResizeDebouncer {
    delay_ms: u64,
    pending: Option<(u16, u16, u64)>,
}

impl ResizeDebouncer {
    pub(super) fn new(delay_ms: u64) -> Self {
        Self {
            delay_ms,
            pending: None,
        }
    }

    pub(super) fn queue(&mut self, width: u16, height: u16, now_ms: u64) {
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

    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
}

impl App {
    /// Run the main event loop.
    pub fn run(&mut self) -> Result<()> {
        let _run_scope = crate::perf::scope("app.run.total");

        // Create image picker BEFORE initializing terminal (queries stdio)
        let picker = if self.images_enabled {
            let _picker_scope = crate::perf::scope("app.create_picker");
            let picker = crate::image::create_picker(self.force_half_cell);
            drop(_picker_scope);
            picker
        } else {
            None
        };

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
        let layout_width = crate::ui::document_content_width(size.width, self.toc_visible);
        let document = crate::document::Document::parse_with_layout(&content, layout_width)?;
        drop(_parse_scope);

        // Create initial model
        let mut model = Model::new(self.file_path.clone(), document, (size.width, size.height))
            .with_picker(picker);
        model.watch_enabled = self.watch_enabled;
        model.toc_visible = self.toc_visible;
        model.force_half_cell = self.force_half_cell;
        model.images_enabled = self.images_enabled;
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
        let start = Instant::now();
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
        let mut mouse_capture_enabled = false;

        loop {
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
            if event::poll(Duration::from_millis(poll_ms))? {
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
                while event::poll(Duration::from_millis(0))? {
                    let msg = self.handle_event(event::read()?, model, now_ms, &mut resize_debouncer);
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
                let load_start = Instant::now();
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
                let draw_start = Instant::now();
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
