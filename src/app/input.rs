use crossterm::event::{
    self, Event, KeyCode, KeyEventKind, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::Frame;
use ratatui::layout::Rect;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Message, Model};
use crate::editor::Direction;

use super::event_loop::ResizeDebouncer;

impl App {
    pub(super) fn handle_event(
        event: &Event,
        model: &Model,
        now_ms: u64,
        resize_debouncer: &mut ResizeDebouncer,
    ) -> Option<Message> {
        match event {
            Event::Key(key) if key.kind == KeyEventKind::Press => Self::handle_key(*key, model),
            Event::Mouse(mouse) => Self::handle_mouse(*mouse, model),
            Event::Resize(w, h) => {
                crate::perf::log_event("event.resize.queue", format!("width={w} height={h}"));
                resize_debouncer.queue(*w, *h, now_ms);
                None
            }
            _ => None,
        }
    }

    pub(super) fn handle_mouse(mouse: MouseEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            return match mouse.kind {
                MouseEventKind::ScrollDown => Some(Message::HelpScrollDown(3)),
                MouseEventKind::ScrollUp => Some(Message::HelpScrollUp(3)),
                _ => None,
            };
        }

        if model.link_picker_active() {
            let area = Rect::new(
                0,
                0,
                model.viewport.width(),
                model.viewport.height().saturating_add(1),
            );
            let popup = crate::ui::link_picker_rect(area, model.link_picker_items.len());
            let in_popup = mouse.column >= popup.x
                && mouse.column < popup.x + popup.width
                && mouse.row >= popup.y
                && mouse.row < popup.y + popup.height;
            if in_popup && matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
                let content_top = crate::ui::link_picker_content_top(popup);
                if mouse.row >= content_top {
                    let rel = mouse.row - content_top;
                    let idx = (rel / 2) as usize;
                    if idx < model.link_picker_items.len() {
                        return Some(Message::SelectVisibleLink(
                            u8::try_from(idx + 1).unwrap_or(u8::MAX),
                        ));
                    }
                }
                return Some(Message::CancelVisibleLinkPicker);
            }
            if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
                return Some(Message::CancelVisibleLinkPicker);
            }
            if matches!(mouse.kind, MouseEventKind::Moved) {
                return None;
            }
        }

        // Editor mode: handle scroll wheel and mouse click
        if model.editor_mode {
            return match mouse.kind {
                MouseEventKind::ScrollDown => Some(Message::EditorScrollDown(3)),
                MouseEventKind::ScrollUp => Some(Message::EditorScrollUp(3)),
                MouseEventKind::Down(MouseButton::Left) => {
                    let buf = model.editor_buffer.as_ref()?;
                    let toast_active = model.active_toast().is_some();
                    let footer_rows = 1 + u16::from(toast_active);
                    let editor_area_height = model
                        .viewport
                        .height()
                        .saturating_add(1)
                        .saturating_sub(footer_rows);
                    let gutter_width = crate::ui::line_number_width(buf.line_count()) + 1;
                    let clicked_line = model.editor_scroll_offset + mouse.row as usize;
                    let clicked_col = (mouse.column as usize).saturating_sub(gutter_width as usize);
                    if mouse.row < editor_area_height {
                        Some(Message::EditorMoveTo(clicked_line, clicked_col))
                    } else {
                        None
                    }
                }
                _ => None,
            };
        }

        let doc_area = document_mouse_area(model);
        let in_doc = point_in_rect(mouse.column, mouse.row, doc_area);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if in_doc && let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false) {
                    return Some(Message::StartSelection(line));
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if model.selection.is_some()
                    && let Some(line) = doc_line_for_row(model, doc_area, mouse.row, true)
                {
                    return Some(Message::UpdateSelection(line));
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if model.selection.is_some() {
                    if let Some(line) = doc_line_for_row(model, doc_area, mouse.row, true) {
                        if model.selection_dragging() {
                            return Some(Message::EndSelection(line));
                        }
                        let content_col = mouse
                            .column
                            .saturating_sub(doc_area.x + crate::ui::DOCUMENT_LEFT_PADDING)
                            as usize;
                        if Self::link_at_column(model, line, content_col).is_some() {
                            return Some(Message::FollowLinkAtLine(line, Some(content_col)));
                        }
                        if image_at_line(model, line) {
                            return Some(Message::FollowLinkAtLine(line, None));
                        }
                        return Some(Message::ClearSelection);
                    }
                    return Some(Message::ClearSelection);
                }
            }
            _ => {}
        }

        if model.toc_visible {
            let total_area = Rect::new(
                0,
                0,
                model.viewport.width(),
                model.viewport.height().saturating_add(1),
            );
            let chunks = crate::ui::split_main_columns(total_area);
            let toc_area = chunks.first().copied().unwrap_or(total_area);
            let toc_hit = mouse.column >= toc_area.x
                && mouse.column < toc_area.x + toc_area.width
                && mouse.row >= toc_area.y
                && mouse.row < toc_area.y + toc_area.height;

            if toc_hit {
                match mouse.kind {
                    MouseEventKind::Up(MouseButton::Left) => {
                        let entry_count = model.toc_entry_count();
                        if entry_count == 0 {
                            return None;
                        }
                        if mouse.row <= toc_area.y
                            || mouse.row >= toc_area.y + toc_area.height.saturating_sub(1)
                        {
                            return None;
                        }
                        let inner_height = toc_area.height.saturating_sub(2) as usize;
                        if inner_height == 0 {
                            return None;
                        }
                        let max_start = entry_count.saturating_sub(inner_height);
                        let start = model.toc_scroll_offset.min(max_start);
                        let rel_row = (mouse.row - toc_area.y - 1) as usize;
                        let idx = start + rel_row;
                        if idx < entry_count {
                            return Some(Message::TocClick(idx));
                        }
                        return None;
                    }
                    MouseEventKind::ScrollDown => return Some(Message::TocScrollDown),
                    MouseEventKind::ScrollUp => return Some(Message::TocScrollUp),
                    MouseEventKind::Moved => return Some(Message::HoverLink(None)),
                    _ => {}
                }
            }

            if !model.selection_dragging()
                && in_doc
                && matches!(mouse.kind, MouseEventKind::Moved)
                && let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false)
            {
                let content_col = mouse
                    .column
                    .saturating_sub(doc_area.x + crate::ui::DOCUMENT_LEFT_PADDING)
                    as usize;
                let hovered = Self::link_at_column(model, line, content_col)
                    .map(|link| link.url)
                    .or_else(|| image_url_at_line(model, line));
                return Some(Message::HoverLink(hovered));
            }
            if !model.selection_dragging() && matches!(mouse.kind, MouseEventKind::Moved) {
                return Some(Message::HoverLink(None));
            }
        }

        if in_doc
            && model.selection.is_none()
            && matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left))
            && let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false)
        {
            let content_col = mouse
                .column
                .saturating_sub(doc_area.x + crate::ui::DOCUMENT_LEFT_PADDING)
                as usize;
            if Self::link_at_column(model, line, content_col).is_some() {
                return Some(Message::FollowLinkAtLine(line, Some(content_col)));
            }
            if image_at_line(model, line) {
                return Some(Message::FollowLinkAtLine(line, None));
            }
        }

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

    pub(super) fn handle_key(key: event::KeyEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            return match key.code {
                KeyCode::Esc | KeyCode::Char('?' | 'q') | KeyCode::F(1) => Some(Message::HideHelp),
                KeyCode::Char('j') | KeyCode::Down => Some(Message::HelpScrollDown(1)),
                KeyCode::Char('k') | KeyCode::Up => Some(Message::HelpScrollUp(1)),
                KeyCode::Char(' ') | KeyCode::PageDown => Some(Message::HelpScrollDown(10)),
                KeyCode::Char('b') | KeyCode::PageUp => Some(Message::HelpScrollUp(10)),
                KeyCode::Char('g') | KeyCode::Home => Some(Message::HelpScrollUp(usize::MAX)),
                KeyCode::Char('G') | KeyCode::End => Some(Message::HelpScrollDown(usize::MAX)),
                _ => None,
            };
        }

        if model.link_picker_active() {
            return match key.code {
                KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                    Some(Message::SelectVisibleLink((c as u8) - b'0'))
                }
                _ => Some(Message::CancelVisibleLinkPicker),
            };
        }

        // Ctrl+C / Ctrl+Q quit from any mode
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'q'))
        {
            return Some(Message::Quit);
        }

        // Ctrl+E toggles edit mode from any mode
        if key.modifiers.contains(KeyModifiers::CONTROL) && matches!(key.code, KeyCode::Char('e')) {
            return if model.editor_mode {
                Some(Message::ExitEditMode)
            } else {
                Some(Message::EnterEditMode)
            };
        }

        if model.editor_mode {
            return Self::handle_editor_key(key);
        }

        if let Some(active_query) = model.search_query.as_ref() {
            return match key.code {
                KeyCode::Esc => Some(Message::ClearSearch),
                KeyCode::Enter => Some(Message::NextMatch),
                KeyCode::Backspace => {
                    let mut next = active_query.clone();
                    next.pop();
                    Some(Message::SearchInput(next))
                }
                KeyCode::Char(c)
                    if !key.modifiers.contains(KeyModifiers::CONTROL)
                        && !key.modifiers.contains(KeyModifiers::ALT) =>
                {
                    let mut next = active_query.clone();
                    next.push(c);
                    Some(Message::SearchInput(next))
                }
                _ => None,
            };
        }

        // Global keys — always apply regardless of TOC focus
        match key.code {
            KeyCode::Char(' ') | KeyCode::PageDown => {
                if model.viewport.can_scroll_down() {
                    return Some(Message::PageDown);
                }
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                if model.viewport.can_scroll_up() {
                    return Some(Message::PageUp);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_down() {
                    return Some(Message::HalfPageDown);
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_up() {
                    return Some(Message::HalfPageUp);
                }
            }
            KeyCode::Char('g') | KeyCode::Home => return Some(Message::GoToTop),
            KeyCode::Char('G') | KeyCode::End => return Some(Message::GoToBottom),
            KeyCode::Char('/') => return Some(Message::StartSearch),
            KeyCode::Char('w') => return Some(Message::ToggleWatch),
            KeyCode::Char('R' | 'r') => return Some(Message::ForceReload),
            KeyCode::Char('o') => return Some(Message::OpenVisibleLinks),
            _ => {}
        }

        // Handle TOC-focused navigation
        if model.toc_focused && model.toc_visible {
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(Message::TocDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Message::TocUp),
                KeyCode::Enter => Some(Message::TocSelect),
                KeyCode::Char('h') | KeyCode::Left => Some(Message::TocCollapse),
                KeyCode::Backspace if model.browse_mode => Some(Message::TocCollapse),
                KeyCode::Char('l') | KeyCode::Right => Some(Message::TocExpand),
                KeyCode::Tab | KeyCode::Esc => Some(Message::SwitchFocus),
                KeyCode::Char('?') | KeyCode::F(1) => Some(Message::ToggleHelp),
                KeyCode::Char('t') => Some(Message::ToggleToc),
                KeyCode::Char('B') => Some(Message::EnterBrowseMode),
                KeyCode::Char('F') => Some(Message::EnterFileMode),
                KeyCode::Char('e') => Some(Message::EnterEditMode),
                KeyCode::Char('q') => Some(Message::Quit),
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
            // TOC
            KeyCode::Char('t') => Some(Message::ToggleToc),
            KeyCode::Char('T') => Some(Message::ToggleTocFocus),
            KeyCode::Tab if model.toc_visible => Some(Message::SwitchFocus),

            // Browse mode
            KeyCode::Char('B') => Some(Message::EnterBrowseMode),
            KeyCode::Char('F') => Some(Message::EnterFileMode),

            // Editor
            KeyCode::Char('e') => Some(Message::EnterEditMode),

            // File
            KeyCode::Char('?') | KeyCode::F(1) => Some(Message::ToggleHelp),

            // Search
            KeyCode::Esc => Some(Message::ClearSearch),

            // Quit
            KeyCode::Char('q') => Some(Message::Quit),

            _ => None,
        }
    }

    const fn handle_editor_key(key: event::KeyEvent) -> Option<Message> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // Exit edit mode
            KeyCode::Esc => Some(Message::ExitEditMode),

            // Save
            KeyCode::Char('s') if ctrl => Some(Message::EditorSave),

            // Navigation with Ctrl
            KeyCode::Left if ctrl => Some(Message::EditorMoveWordLeft),
            KeyCode::Right if ctrl => Some(Message::EditorMoveWordRight),
            KeyCode::Home if ctrl => Some(Message::EditorMoveToStart),
            KeyCode::End if ctrl => Some(Message::EditorMoveToEnd),

            // Basic navigation
            KeyCode::Left => Some(Message::EditorMoveCursor(Direction::Left)),
            KeyCode::Right => Some(Message::EditorMoveCursor(Direction::Right)),
            KeyCode::Up => Some(Message::EditorMoveCursor(Direction::Up)),
            KeyCode::Down => Some(Message::EditorMoveCursor(Direction::Down)),
            KeyCode::Home => Some(Message::EditorMoveHome),
            KeyCode::End => Some(Message::EditorMoveEnd),
            KeyCode::PageUp => Some(Message::EditorScrollUp(20)),
            KeyCode::PageDown => Some(Message::EditorScrollDown(20)),

            // Editing
            KeyCode::Enter => Some(Message::EditorSplitLine),
            KeyCode::Backspace => Some(Message::EditorDeleteBack),
            KeyCode::Delete => Some(Message::EditorDeleteForward),
            KeyCode::Tab => Some(Message::EditorInsertChar('\t')),
            KeyCode::Char(c) if !ctrl => Some(Message::EditorInsertChar(c)),

            _ => None,
        }
    }

    pub(super) fn view(model: &mut Model, frame: &mut Frame) {
        crate::ui::render(model, frame);
    }

    pub(super) fn link_at_column(
        model: &Model,
        line: usize,
        content_col: usize,
    ) -> Option<crate::document::LinkRef> {
        let line_text = model.document.line_at(line)?.content();
        let links_on_line: Vec<_> = model
            .document
            .links()
            .iter()
            .filter(|link| link.line == line)
            .cloned()
            .collect();
        if links_on_line.is_empty() {
            return None;
        }
        for link in links_on_line {
            if link.text.is_empty() {
                continue;
            }
            let mut search = 0usize;
            while search < line_text.len() {
                let Some(haystack) = line_text.get(search..) else {
                    break;
                };
                let Some(rel) = haystack.find(&link.text) else {
                    break;
                };
                let start_byte = search + rel;
                let Some(prefix) = line_text.get(..start_byte) else {
                    break;
                };
                let start_col = UnicodeWidthStr::width(prefix);
                let end_col = start_col + UnicodeWidthStr::width(link.text.as_str());
                if content_col >= start_col && content_col < end_col {
                    return Some(link);
                }
                search = start_byte + link.text.len();
            }
        }
        None
    }
}

fn document_mouse_area(model: &Model) -> Rect {
    let total_area = Rect::new(
        0,
        0,
        model.viewport.width(),
        model.viewport.height().saturating_add(1),
    );
    let content_area = if model.toc_visible {
        crate::ui::split_main_columns(total_area)
            .get(1)
            .copied()
            .unwrap_or(total_area)
    } else {
        total_area
    };
    let search_active = model.search_query.is_some();
    let toast_active = model.active_toast().is_some();
    let hover_active = model.hovered_link_url.is_some();
    let footer_rows =
        1 + u16::from(search_active) + u16::from(toast_active) + u16::from(hover_active);
    Rect {
        x: content_area.x,
        y: content_area.y,
        width: content_area.width,
        height: content_area.height.saturating_sub(footer_rows),
    }
}

const fn point_in_rect(col: u16, row: u16, rect: Rect) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn doc_line_for_row(model: &Model, doc_area: Rect, row: u16, clamp: bool) -> Option<usize> {
    if doc_area.height == 0 || model.document.line_count() == 0 {
        return None;
    }
    let max_row = doc_area.y + doc_area.height.saturating_sub(1);
    let row = if clamp {
        row.clamp(doc_area.y, max_row)
    } else if row < doc_area.y || row > max_row {
        return None;
    } else {
        row
    };
    let rel_row = row.saturating_sub(doc_area.y) as usize;
    let mut line = model.viewport.offset() + rel_row;
    let max_line = model.document.line_count().saturating_sub(1);
    if line > max_line {
        line = max_line;
    }
    Some(line)
}

fn image_at_line(model: &Model, line: usize) -> bool {
    model
        .document
        .images()
        .iter()
        .any(|img| line >= img.line_range.start && line < img.line_range.end)
}

fn image_url_at_line(model: &Model, line: usize) -> Option<String> {
    model
        .document
        .images()
        .iter()
        .find(|img| line >= img.line_range.start && line < img.line_range.end)
        .map(|img| img.src.clone())
}
