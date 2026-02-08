use std::io::{Write, stdout};
use std::path::PathBuf;
use std::time::Duration;

use crate::app::{App, Message, Model, ToastLevel};
use crate::watcher::FileWatcher;
use base64::Engine;

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
                    model.show_toast(ToastLevel::Error, format!("Reload failed: {err}"));
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
            Message::FollowLinkAtLine(line, col) => {
                self.follow_link_on_line(model, *line, *col);
            }
            Message::SelectVisibleLink(index) => {
                self.follow_link_picker_index(model, *index);
            }
            Message::EndSelection(_) => {
                self.copy_selection(model);
                model.clear_selection();
            }
            Message::TocSelect | Message::TocClick(_) | Message::TocExpand if model.browse_mode => {
                self.browse_activate_selected(model);
            }
            Message::TocCollapse if model.browse_mode => {
                self.browse_navigate_parent(model);
            }
            Message::EnterBrowseMode => {
                let dir = model
                    .file_path
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                    .map_or_else(|| PathBuf::from("."), std::path::Path::to_path_buf);
                if let Err(err) = model.load_directory(&dir) {
                    model.show_toast(ToastLevel::Error, format!("Browse failed: {err}"));
                } else {
                    // Highlight the current file in the listing (compare by name
                    // since load_directory canonicalizes paths)
                    if let Some(name) = model.file_path.file_name() {
                        let name = name.to_string_lossy();
                        if let Some(idx) = model.browse_entries.iter().position(|e| e.name == *name)
                        {
                            model.toc_selected = Some(idx);
                        }
                    }
                }
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

    fn follow_link_on_line(&self, model: &mut Model, line: usize, col: Option<usize>) {
        if let Some(col) = col
            && let Some(link) = self.link_at_column(model, line, col)
        {
            let url = link.url;
            model.link_picker_items.clear();
            self.follow_resolved_link(model, &url);
            return;
        }
        if let Some(link) = model.document.links().iter().find(|link| link.line == line) {
            let url = link.url.clone();
            model.link_picker_items.clear();
            self.follow_resolved_link(model, &url);
            return;
        }

        let Some(image) = model
            .document
            .images()
            .iter()
            .find(|img| line >= img.line_range.start && line < img.line_range.end)
        else {
            return;
        };
        let url = image.src.clone();
        model.link_picker_items.clear();
        self.follow_resolved_link(model, &url);
    }

    fn follow_resolved_link(&self, model: &mut Model, url: &str) {
        let _ = self;
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
            Err(err) => model.show_toast(ToastLevel::Error, format!("Open failed: {err}")),
        }
    }

    fn browse_activate_selected(&self, model: &mut Model) {
        let _ = self;
        let Some(sel) = model.toc_selected else {
            return;
        };
        let Some(entry) = model.browse_entries.get(sel).cloned() else {
            return;
        };
        let path = entry.path;
        if entry.is_dir {
            if let Err(err) = model.load_directory(&path) {
                model.show_toast(ToastLevel::Error, format!("Browse failed: {err}"));
            } else {
                model.toc_selected = Some(0);
                Self::browse_auto_load_first_file(model);
            }
        } else if let Err(err) = model.load_file(&path) {
            model.show_toast(ToastLevel::Error, format!("Open failed: {err}"));
        }
    }

    fn browse_navigate_parent(&self, model: &mut Model) {
        let _ = self;
        let parent = model
            .browse_dir
            .parent()
            .unwrap_or(&model.browse_dir)
            .to_path_buf();
        // Already at filesystem root — nothing to do.
        if parent == model.browse_dir {
            return;
        }
        let old_name = model
            .browse_dir
            .file_name()
            .map(|n| n.to_string_lossy().to_string());
        if let Err(err) = model.load_directory(&parent) {
            model.show_toast(ToastLevel::Error, format!("Browse failed: {err}"));
        } else {
            // Try to highlight the directory we came from
            if let Some(ref name) = old_name {
                if let Some(idx) = model.browse_entries.iter().position(|e| e.name == *name) {
                    model.toc_selected = Some(idx);
                } else {
                    model.toc_selected = Some(0);
                }
            } else {
                model.toc_selected = Some(0);
            }
            Self::browse_auto_load_first_file(model);
        }
    }

    fn browse_auto_load_first_file(model: &mut Model) {
        if let Some((idx, path)) = model.first_viewable_file_index() {
            if let Err(err) = model.load_file(&path) {
                model.show_toast(ToastLevel::Error, format!("Open failed: {err}"));
            } else {
                model.toc_selected = Some(idx);
            }
        }
    }

    fn copy_selection(&self, model: &mut Model) {
        let _ = self;
        let Some((text, lines)) = model.selected_text() else {
            return;
        };
        if text.is_empty() {
            return;
        }
        match copy_to_clipboard(&text) {
            Ok(()) => model.show_toast(ToastLevel::Info, format!("Copied {lines} line(s)")),
            Err(err) => model.show_toast(ToastLevel::Error, format!("Copy failed: {err}")),
        }
    }
}

fn open_external_link(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()?
            .wait()?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        use std::process::Stdio;
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()?;
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

fn copy_to_clipboard(text: &str) -> std::io::Result<()> {
    #[cfg(target_os = "macos")]
    {
        if copy_to_pbcopy(text).is_ok() {
            return Ok(());
        }
    }
    copy_to_clipboard_osc52(text)
}

#[cfg(target_os = "macos")]
fn copy_to_pbcopy(text: &str) -> std::io::Result<()> {
    use std::process::{Command, Stdio};

    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn()?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
    }
    let status = child.wait()?;
    if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other("pbcopy failed"))
    }
}

fn copy_to_clipboard_osc52(text: &str) -> std::io::Result<()> {
    let osc = osc52_sequence(text);
    let mut out = stdout();
    out.write_all(osc.as_bytes())?;
    out.flush()
}

fn osc52_sequence(text: &str) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    format!("\x1b]52;c;{encoded}\x07")
}

#[cfg(test)]
mod tests {
    use super::osc52_sequence;

    #[test]
    fn test_osc52_sequence_encodes_text() {
        let seq = osc52_sequence("hi");
        assert_eq!(seq, "\x1b]52;c;aGk=\x07");
    }
}
