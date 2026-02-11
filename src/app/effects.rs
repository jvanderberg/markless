use std::io::{Write, stdout};
use std::path::PathBuf;
use std::time::Duration;

use crate::app::{App, Message, Model, ToastLevel};
use crate::watcher::FileWatcher;
use base64::Engine;

impl App {
    pub(super) fn make_file_watcher(path: &std::path::Path) -> notify::Result<FileWatcher> {
        FileWatcher::new(path, Duration::from_millis(200))
    }

    pub(super) fn handle_message_side_effects(
        model: &mut Model,
        file_watcher: &mut Option<FileWatcher>,
        msg: &Message,
    ) {
        match msg {
            Message::ToggleWatch => {
                if model.watch_enabled {
                    match Self::make_file_watcher(&model.file_path) {
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
                if model.editor_mode {
                    // Don't reload while editing — check for conflict instead
                    if let Some(new_hash) = model.file_disk_hash()
                        && model.editor_disk_hash != Some(new_hash)
                    {
                        model.editor_disk_conflict = true;
                        model.show_toast(ToastLevel::Warning, "File changed on disk while editing");
                    }
                } else if let Err(err) = model.reload_from_disk() {
                    model.show_toast(ToastLevel::Error, format!("Reload failed: {err}"));
                    crate::perf::log_event(
                        "reload.error",
                        format!("failed path={} err={err}", model.file_path.display()),
                    );
                } else if matches!(msg, Message::ForceReload) {
                    model.show_toast(ToastLevel::Info, "Reloaded");
                } else {
                    model.show_toast(ToastLevel::Info, "File changed, reloaded");
                }
            }
            Message::OpenVisibleLinks => {
                Self::open_visible_links(model);
            }
            Message::FollowLinkAtLine(line, col) => {
                Self::follow_link_on_line(model, *line, *col);
            }
            Message::SelectVisibleLink(index) => {
                Self::follow_link_picker_index(model, *index);
            }
            Message::EndSelection(_) => {
                Self::copy_selection(model);
                model.clear_selection();
            }
            Message::TocSelect | Message::TocClick(_) | Message::TocExpand if model.browse_mode => {
                Self::browse_activate_selected(model);
            }
            Message::TocCollapse if model.browse_mode => {
                Self::browse_navigate_parent(model);
            }
            Message::EnterEditMode => {
                model.editor_disk_hash = model.file_disk_hash();
            }
            Message::EditorSave => {
                Self::save_editor_buffer(model);
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

    fn open_visible_links(model: &mut Model) {
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
            1 => Self::follow_resolved_link(model, &visible[0].url),
            _ => {
                model.link_picker_items = visible;
                model.show_toast(ToastLevel::Info, "Select link: 1-9 (Esc to cancel)");
            }
        }
    }

    fn follow_link_picker_index(model: &mut Model, index: u8) {
        if index == 0 {
            return;
        }
        let idx = (index - 1) as usize;
        let Some(link) = model.link_picker_items.get(idx) else {
            return;
        };
        let url = link.url.clone();
        model.link_picker_items.clear();
        Self::follow_resolved_link(model, &url);
    }

    fn follow_link_on_line(model: &mut Model, line: usize, col: Option<usize>) {
        if let Some(col) = col
            && let Some(link) = Self::link_at_column(model, line, col)
        {
            let url = link.url;
            model.link_picker_items.clear();
            Self::follow_resolved_link(model, &url);
            return;
        }
        if let Some(link) = model.document.links().iter().find(|link| link.line == line) {
            let url = link.url.clone();
            model.link_picker_items.clear();
            Self::follow_resolved_link(model, &url);
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
        Self::follow_resolved_link(model, &url);
    }

    fn follow_resolved_link(model: &mut Model, url: &str) {
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

        if url.starts_with("mermaid://") {
            Self::open_mermaid_svg(model, url);
            return;
        }

        match open_external_link(url) {
            Ok(()) => model.show_toast(ToastLevel::Info, format!("Opened {url}")),
            Err(err) => model.show_toast(ToastLevel::Error, format!("Open failed: {err}")),
        }
    }

    fn open_mermaid_svg(model: &mut Model, mermaid_url: &str) {
        use std::hash::{DefaultHasher, Hash, Hasher};

        let Some(source) = model.document.mermaid_sources().get(mermaid_url).cloned() else {
            model.show_toast(ToastLevel::Warning, "Mermaid source not found");
            return;
        };
        let svg = match crate::mermaid::render_to_svg(&source) {
            Ok(s) => s,
            Err(err) => {
                model.show_toast(ToastLevel::Error, format!("Mermaid render failed: {err}"));
                return;
            }
        };
        // Use a content hash so different diagrams get distinct files and
        // different documents don't silently overwrite each other's SVGs.
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        let hash = hasher.finish();
        let path = std::env::temp_dir().join(format!("markless-mermaid-{hash:016x}.svg"));
        if let Err(err) = std::fs::write(&path, &svg) {
            model.show_toast(ToastLevel::Error, format!("Write SVG failed: {err}"));
            return;
        }
        let path_str = path.to_string_lossy();
        match open_external_link(&path_str) {
            Ok(()) => model.show_toast(ToastLevel::Info, "Opened mermaid SVG"),
            Err(err) => model.show_toast(ToastLevel::Error, format!("Open failed: {err}")),
        }
    }

    fn browse_activate_selected(model: &mut Model) {
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

    fn browse_navigate_parent(model: &mut Model) {
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

    fn save_editor_buffer(model: &mut Model) {
        let is_dirty = model
            .editor_buffer
            .as_ref()
            .is_some_and(crate::editor::EditorBuffer::is_dirty);
        if model.editor_buffer.is_none() {
            return;
        }
        if !is_dirty {
            model.show_toast(ToastLevel::Info, "No changes to save");
            return;
        }

        // Check if the file changed on disk since we started editing
        if !model.save_confirmed
            && let Some(current_hash) = model.file_disk_hash()
            && model.editor_disk_hash != Some(current_hash)
        {
            model.editor_disk_conflict = true;
            model.save_confirmed = true;
            model.show_toast(
                ToastLevel::Warning,
                "File changed on disk! Press Ctrl+S again to overwrite",
            );
            return;
        }

        let text = model
            .editor_buffer
            .as_ref()
            .map(crate::editor::EditorBuffer::text)
            .unwrap_or_default();
        let path = model.file_path.clone();
        match std::fs::write(&path, &text) {
            Ok(()) => {
                if let Some(buf) = &mut model.editor_buffer {
                    buf.mark_clean();
                }
                model.editor_disk_hash = model.file_disk_hash();
                model.editor_disk_conflict = false;
                model.save_confirmed = false;

                // If save was triggered during a quit/exit warning, complete that action
                if model.quit_confirmed {
                    model.should_quit = true;
                } else if model.exit_confirmed {
                    // Re-parse saved text into the document before leaving edit mode
                    let is_md =
                        crate::app::model::is_markdown_ext(&model.file_path.to_string_lossy());
                    let doc = if is_md {
                        crate::document::Document::parse_with_all_options(
                            &text,
                            model.layout_width(),
                            &std::collections::HashMap::new(),
                            model.should_render_mermaid_as_images(),
                        )
                        .ok()
                    } else {
                        Some(crate::document::Document::from_plain_text(&text))
                    };
                    if let Some(doc) = doc {
                        model.document = doc;
                        model.viewport.set_total_lines(model.document.line_count());
                    }

                    // Exit edit mode now that the buffer is saved
                    model.editor_mode = false;
                    model.editor_buffer = None;
                    model.editor_scroll_offset = 0;
                    model.editor_disk_hash = None;
                    model.exit_confirmed = false;
                    model.show_toast(
                        ToastLevel::Info,
                        format!("Saved {} and exited editor", path.display()),
                    );
                } else {
                    model.show_toast(ToastLevel::Info, format!("Saved {}", path.display()));
                }
            }
            Err(err) => {
                model.show_toast(ToastLevel::Error, format!("Save failed: {err}"));
            }
        }
    }

    fn copy_selection(model: &mut Model) {
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
