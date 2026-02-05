use std::time::Duration;

use crate::app::{App, Message, Model, ToastLevel};
use crate::watcher::FileWatcher;

impl App {
    pub(super) fn make_file_watcher(&self) -> notify::Result<FileWatcher> {
        FileWatcher::new(&self.file_path, Duration::from_millis(200))
    }

    pub(super) fn handle_message_side_effects(
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
            Message::OpenVisibleLinks => {
                self.open_visible_links(model);
            }
            Message::FollowLinkAtLine(line) => {
                self.follow_link_on_line(model, *line);
            }
            Message::SelectVisibleLink(index) => {
                self.follow_link_picker_index(model, *index);
            }
            _ => {}
        }
    }

    fn open_visible_links(&self, model: &mut Model) {
        let start = model.viewport.offset();
        let end = start + model.viewport.height() as usize;
        let mut visible: Vec<_> = model
            .document
            .links()
            .iter()
            .filter(|link| link.line >= start && link.line < end)
            .cloned()
            .collect();
        visible.truncate(9);

        match visible.len() {
            0 => model.show_toast(ToastLevel::Info, "No visible links"),
            1 => self.follow_resolved_link(model, &visible[0].url),
            _ => {
                model.link_picker_items = visible;
                model.show_toast(ToastLevel::Info, "Select link: 1-9 (Esc to cancel)");
            }
        }
    }

    fn follow_link_picker_index(&self, model: &mut Model, index: u8) {
        if index == 0 {
            return;
        }
        let idx = (index - 1) as usize;
        let Some(link) = model.link_picker_items.get(idx) else {
            return;
        };
        let url = link.url.clone();
        model.link_picker_items.clear();
        self.follow_resolved_link(model, &url);
    }

    fn follow_link_on_line(&self, model: &mut Model, line: usize) {
        let Some(link) = model.document.links().iter().find(|link| link.line == line) else {
            return;
        };
        let url = link.url.clone();
        model.link_picker_items.clear();
        self.follow_resolved_link(model, &url);
    }

    fn follow_resolved_link(&self, model: &mut Model, url: &str) {
        if let Some(name) = url.strip_prefix("footnote:") {
            if let Some(target) = model.document.footnote_line(name) {
                model.viewport.go_to_line(target);
                model.show_toast(ToastLevel::Info, format!("Jumped to footnote [^{name}]"));
            } else {
                model.show_toast(ToastLevel::Warning, format!("Footnote [^{name}] not found"));
            }
            return;
        }

        if let Some(anchor) = url.strip_prefix('#') {
            if let Some(target) = model.document.resolve_internal_anchor(anchor) {
                model.viewport.go_to_line(target);
                model.show_toast(ToastLevel::Info, format!("Jumped to #{anchor}"));
            } else {
                model.show_toast(ToastLevel::Warning, format!("Anchor #{anchor} not found"));
            }
            return;
        }

        match open_external_link(url) {
            Ok(()) => model.show_toast(ToastLevel::Info, format!("Opened {url}")),
            Err(err) => model.show_toast(
                ToastLevel::Error,
                format!("Open failed: {err}"),
            ),
        }
    }
}

fn open_external_link(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()?.wait()?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()?
            .wait()?;
        return Ok(());
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()?
            .wait()?;
        Ok(())
    }
}
