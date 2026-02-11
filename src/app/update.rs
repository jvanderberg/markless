use crate::app::Model;
use crate::app::model::{LineSelection, SelectionState};
use crate::editor::{Direction, EditorBuffer};

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
    /// Open visible-link picker (or follow directly when single link)
    OpenVisibleLinks,
    /// Follow link on an exact rendered line, optionally at a specific column
    FollowLinkAtLine(usize, Option<usize>),
    /// Follow numbered link in the picker
    SelectVisibleLink(u8),
    /// Close visible-link picker
    CancelVisibleLinkPicker,
    /// Update hovered link URL (or clear when none)
    HoverLink(Option<String>),
    /// Start a line selection (mouse down)
    StartSelection(usize),
    /// Update a line selection (mouse drag)
    UpdateSelection(usize),
    /// Finish a line selection (mouse up)
    EndSelection(usize),
    /// Clear current selection
    ClearSelection,

    // Browse mode
    /// Switch to file-only mode (TOC shows headings)
    EnterFileMode,
    /// Switch to browse mode (TOC shows directory listing)
    EnterBrowseMode,

    // Editor
    /// Enter edit mode (load source into editor buffer)
    EnterEditMode,
    /// Exit edit mode (return to view mode)
    ExitEditMode,
    /// Insert a character at the cursor
    EditorInsertChar(char),
    /// Delete character before cursor (Backspace)
    EditorDeleteBack,
    /// Delete character at cursor (Delete)
    EditorDeleteForward,
    /// Split line at cursor (Enter)
    EditorSplitLine,
    /// Move cursor in a direction
    EditorMoveCursor(Direction),
    /// Move cursor to beginning of line (Home)
    EditorMoveHome,
    /// Move cursor to end of line (End)
    EditorMoveEnd,
    /// Move cursor one word left (Ctrl+Left)
    EditorMoveWordLeft,
    /// Move cursor one word right (Ctrl+Right)
    EditorMoveWordRight,
    /// Move cursor to start of buffer (Ctrl+Home)
    EditorMoveToStart,
    /// Move cursor to end of buffer (Ctrl+End)
    EditorMoveToEnd,
    /// Save editor buffer to file
    EditorSave,
    /// Scroll editor viewport up by n lines
    EditorScrollUp(usize),
    /// Move cursor to absolute position (line, col) — e.g. from mouse click
    EditorMoveTo(usize, usize),
    /// Scroll editor viewport down by n lines
    EditorScrollDown(usize),

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
            | Message::HoverLink(_)
    );
    // Reset confirmation flags on any action other than the confirmed one.
    // EditorSave preserves quit/exit flags so Ctrl+S can complete a pending quit/exit.
    if !matches!(msg, Message::Quit | Message::EditorSave) {
        model.quit_confirmed = false;
    }
    if !matches!(msg, Message::ExitEditMode | Message::EditorSave) {
        model.exit_confirmed = false;
    }
    if !matches!(msg, Message::EditorSave) {
        model.save_confirmed = false;
    }

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
                let max = model.toc_entry_count().saturating_sub(1);
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
            if !model.browse_mode
                && let Some(sel) = model.toc_selected
                && let Some(heading) = model.document.headings().get(sel)
            {
                model.viewport.go_to_line(heading.line);
            }
            // Browse mode selection handled in effects
        }
        Message::TocClick(idx) => {
            model.toc_selected = Some(idx);
            if !model.browse_mode
                && let Some(heading) = model.document.headings().get(idx)
            {
                model.viewport.go_to_line(heading.line);
            }
            // Browse mode click handled in effects
        }
        Message::TocScrollUp => {
            model.toc_scroll_offset = model.toc_scroll_offset.saturating_sub(1);
        }
        Message::TocScrollDown => {
            model.toc_scroll_offset =
                (model.toc_scroll_offset + 1).min(model.max_toc_scroll_offset());
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
        // TocCollapse/TocExpand: handled in effects (browse mode navigation)
        // FileChanged/ForceReload: handled in event loop (side effect)
        // Redraw: no state change needed
        Message::TocCollapse
        | Message::TocExpand
        | Message::FileChanged
        | Message::ForceReload
        | Message::Redraw
        | Message::EditorSave => {}

        // Search
        Message::StartSearch => {
            model.search_query = Some(String::new());
            model.search_matches.clear();
            model.search_match_index = None;
            model.search_allow_short = false;
        }
        Message::StartSearchWith(query) | Message::SearchInput(query) => {
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
        Message::OpenVisibleLinks
        | Message::FollowLinkAtLine(_, _)
        | Message::SelectVisibleLink(_) => {
            model.clear_selection();
            // side effect in event loop
        }
        Message::CancelVisibleLinkPicker => {
            model.link_picker_items.clear();
        }
        Message::HoverLink(url) => {
            model.hovered_link_url = url;
        }
        Message::StartSelection(line) => {
            model.selection = Some(LineSelection {
                anchor: line,
                active: line,
                state: SelectionState::Pending,
            });
        }
        Message::UpdateSelection(line) => {
            if let Some(selection) = model.selection {
                model.selection = Some(LineSelection {
                    anchor: selection.anchor,
                    active: line,
                    state: SelectionState::Dragging,
                });
            }
        }
        Message::EndSelection(line) => {
            if let Some(selection) = model.selection {
                model.selection = Some(LineSelection {
                    anchor: selection.anchor,
                    active: line,
                    state: SelectionState::Finalized,
                });
            }
        }
        Message::ClearSelection => {
            model.clear_selection();
        }

        // Editor
        Message::EnterEditMode => {
            if !model.editor_mode {
                let source = model.document.source().to_string();
                let mut buf = EditorBuffer::from_text(&source);

                // Approximate editor scroll from viewport position
                let vp_offset = model.viewport.offset();
                let rendered_total = model.document.line_count().max(1);
                let source_lines = buf.line_count();
                // Map rendered-line offset to source-line offset proportionally
                let target_line = if rendered_total > 1 && vp_offset > 0 {
                    (vp_offset * source_lines.saturating_sub(1)) / rendered_total.saturating_sub(1)
                } else {
                    0
                };
                buf.move_to(target_line, 0);
                model.editor_scroll_offset = target_line;

                model.editor_buffer = Some(buf);
                model.editor_mode = true;
            }
        }
        Message::ExitEditMode => {
            if model.editor_mode {
                // Warn about unsaved changes on first Esc press
                if model.editor_is_dirty() && !model.exit_confirmed {
                    model.show_toast(
                        crate::app::ToastLevel::Warning,
                        "Unsaved changes! Press Esc again to discard, or Ctrl+S to save",
                    );
                    model.exit_confirmed = true;
                    return model;
                }

                // Remember scroll position ratio before dropping the buffer
                let scroll_ratio = model.editor_buffer.as_ref().map(|buf| {
                    let source_total = buf.line_count().saturating_sub(1).max(1);
                    (model.editor_scroll_offset, source_total)
                });

                // Only update the document if the buffer was saved (clean).
                // If dirty, the user is discarding — keep the original document.
                let is_clean = model.editor_buffer.as_ref().is_some_and(|b| !b.is_dirty());
                if is_clean && let Some(buf) = &model.editor_buffer {
                    let text = buf.text();
                    let is_md = super::model::is_markdown_ext(&model.file_path.to_string_lossy());
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
                }
                model.editor_mode = false;
                model.editor_buffer = None;
                model.editor_scroll_offset = 0;
                model.editor_disk_hash = None;
                model.editor_disk_conflict = false;
                model.save_confirmed = false;

                // Map source scroll position to rendered line position
                if let Some((src_offset, src_total)) = scroll_ratio
                    && src_offset > 0
                {
                    let rendered_total = model.document.line_count().saturating_sub(1).max(1);
                    let target = (src_offset * rendered_total) / src_total;
                    model.viewport.go_to_line(target);
                }
            }
        }
        Message::EditorInsertChar(ch) => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.insert_char(ch);
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorDeleteBack => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.delete_back();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorDeleteForward => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.delete_forward();
            }
        }
        Message::EditorSplitLine => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.split_line();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveCursor(dir) => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_cursor(dir);
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveHome => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_home();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveEnd => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_end();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveWordLeft => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_word_left();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveWordRight => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_word_right();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveToStart => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_to_start();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveToEnd => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_to_end();
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorMoveTo(line, col) => {
            if let Some(buf) = &mut model.editor_buffer {
                buf.move_to(line, col);
                editor_ensure_cursor_visible(&mut model);
            }
        }
        Message::EditorScrollUp(n) => {
            model.editor_scroll_offset = model.editor_scroll_offset.saturating_sub(n);
        }
        Message::EditorScrollDown(n) => {
            let max = model
                .editor_buffer
                .as_ref()
                .map_or(0, |buf| buf.line_count().saturating_sub(1));
            model.editor_scroll_offset = (model.editor_scroll_offset + n).min(max);
        }

        // Browse mode
        Message::EnterFileMode => {
            model.browse_mode = false;
            // Sync TOC selection to the current viewport so it points at valid
            // heading indices rather than stale browse-entry indices.
            model.sync_toc_to_viewport();
        }
        Message::EnterBrowseMode => {
            model.browse_mode = true;
            model.toc_visible = true;
            if model.toc_selected.is_none() {
                model.toc_selected = Some(0);
            }
        }
        // Window
        Message::Resize(width, height) => {
            model.viewport.resize(width, height.saturating_sub(1));
            model.reflow_layout();
        }
        // Application
        Message::Quit => {
            if model.editor_is_dirty() && !model.quit_confirmed {
                model.show_toast(
                    crate::app::ToastLevel::Warning,
                    "Unsaved changes! Press Ctrl+Q again to quit, or Ctrl+S to save",
                );
                model.quit_confirmed = true;
            } else {
                model.should_quit = true;
            }
        }
    }
    if should_sync_toc && model.toc_visible && !model.browse_mode {
        model.sync_toc_to_viewport();
    }
    model
}

pub(super) fn closest_heading_to_line(
    headings: &[crate::document::HeadingRef],
    line: usize,
) -> Option<usize> {
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

/// Ensure the editor cursor line is visible in the viewport.
fn editor_ensure_cursor_visible(model: &mut Model) {
    let Some(buf) = &model.editor_buffer else {
        return;
    };
    let cursor_line = buf.cursor().line;
    let visible_height = usize::from(model.viewport.height().saturating_sub(1));
    if visible_height == 0 {
        model.editor_scroll_offset = cursor_line;
        return;
    }

    if cursor_line < model.editor_scroll_offset {
        model.editor_scroll_offset = cursor_line;
    } else if cursor_line >= model.editor_scroll_offset + visible_height {
        model.editor_scroll_offset = cursor_line + 1 - visible_height;
    }
}

pub(super) fn refresh_search_matches(model: &mut Model, jump_to_first: bool, allow_short: bool) {
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

    // Large documents: defer search until explicit Enter to avoid hanging
    // on every keystroke. 10k lines ≈ 160KB binary or a very long text file.
    if !allow_short && model.document.line_count() > 10_000 {
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
