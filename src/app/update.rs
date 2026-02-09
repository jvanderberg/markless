use crate::app::Model;
use crate::app::model::{LineSelection, SelectionState};

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
        | Message::Redraw => {}

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
            model.should_quit = true;
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
