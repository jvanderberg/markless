use crossterm::event::{self, Event, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use ratatui::Frame;

use crate::app::{App, Message, Model};

use super::event_loop::ResizeDebouncer;

impl App {
    pub(super) fn handle_event(
        &self,
        event: Event,
        model: &Model,
        now_ms: u64,
        resize_debouncer: &mut ResizeDebouncer,
    ) -> Option<Message> {
        match event {
            Event::Key(key) => self.handle_key(key, model),
            Event::Mouse(mouse) => self.handle_mouse(mouse, model),
            Event::Resize(w, h) => {
                crate::perf::log_event("event.resize.queue", format!("width={} height={}", w, h));
                resize_debouncer.queue(w, h, now_ms);
                None
            }
            _ => None,
        }
    }

    pub(super) fn handle_mouse(&self, mouse: MouseEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            return None;
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
                        return Some(Message::SelectVisibleLink((idx + 1) as u8));
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

        let doc_area = document_mouse_area(model);
        let in_doc = point_in_rect(mouse.column, mouse.row, doc_area);

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                if in_doc {
                    if let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false) {
                        return Some(Message::StartSelection(line));
                    }
                }
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if model.selection.is_some() {
                    if let Some(line) = doc_line_for_row(model, doc_area, mouse.row, true) {
                        return Some(Message::UpdateSelection(line));
                    }
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
                        if self.link_at_column(model, line, content_col).is_some() {
                            return Some(Message::FollowLinkAtLine(line));
                        }
                        if image_at_line(model, line) {
                            return Some(Message::FollowLinkAtLine(line));
                        }
                        return Some(Message::EndSelection(line));
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
            let toc_area = chunks[0];
            let in_toc = mouse.column >= toc_area.x
                && mouse.column < toc_area.x + toc_area.width
                && mouse.row >= toc_area.y
                && mouse.row < toc_area.y + toc_area.height;

            if in_toc {
                match mouse.kind {
                    MouseEventKind::Up(MouseButton::Left) => {
                        let headings_len = model.document.headings().len();
                        if headings_len == 0 {
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
                        let max_start = headings_len.saturating_sub(inner_height);
                        let start = model.toc_scroll_offset.min(max_start);
                        let rel_row = (mouse.row - toc_area.y - 1) as usize;
                        let idx = start + rel_row;
                        if idx < headings_len {
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

            if !model.selection_dragging() && in_doc && matches!(mouse.kind, MouseEventKind::Moved) {
                if let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false) {
                    let content_col = mouse
                        .column
                        .saturating_sub(doc_area.x + crate::ui::DOCUMENT_LEFT_PADDING)
                        as usize;
                    let hovered = self
                        .link_at_column(model, line, content_col)
                        .map(|link| link.url)
                        .or_else(|| image_url_at_line(model, line));
                    return Some(Message::HoverLink(hovered));
                }
            }
            if !model.selection_dragging() && matches!(mouse.kind, MouseEventKind::Moved) {
                return Some(Message::HoverLink(None));
            }
        }

        if in_doc
            && model.selection.is_none()
            && matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left))
        {
            if let Some(line) = doc_line_for_row(model, doc_area, mouse.row, false) {
                let content_col = mouse
                    .column
                    .saturating_sub(doc_area.x + crate::ui::DOCUMENT_LEFT_PADDING)
                    as usize;
                if self.link_at_column(model, line, content_col).is_some() {
                    return Some(Message::FollowLinkAtLine(line));
                }
                if image_at_line(model, line) {
                    return Some(Message::FollowLinkAtLine(line));
                }
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

    pub(super) fn handle_key(&self, key: event::KeyEvent, model: &Model) -> Option<Message> {
        if model.help_visible {
            let _ = key;
            return Some(Message::HideHelp);
        }

        if model.link_picker_active() {
            return match key.code {
                KeyCode::Char(c) if ('1'..='9').contains(&c) => {
                    Some(Message::SelectVisibleLink((c as u8) - b'0'))
                }
                _ => Some(Message::CancelVisibleLinkPicker),
            };
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

        // Handle TOC-focused navigation
        if model.toc_focused && model.toc_visible {
            return match key.code {
                KeyCode::Char('j') | KeyCode::Down => Some(Message::TocDown),
                KeyCode::Char('k') | KeyCode::Up => Some(Message::TocUp),
                KeyCode::Enter | KeyCode::Char(' ') => Some(Message::TocSelect),
                KeyCode::Char('h') | KeyCode::Left => Some(Message::TocCollapse),
                KeyCode::Char('l') | KeyCode::Right => Some(Message::TocExpand),
                KeyCode::Tab => Some(Message::SwitchFocus),
                KeyCode::Char('?') | KeyCode::F(1) => Some(Message::ToggleHelp),
                KeyCode::Char('t') => Some(Message::ToggleToc),
                KeyCode::Char('q') => Some(Message::Quit),
                KeyCode::Esc => Some(Message::SwitchFocus),
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
            KeyCode::Char(' ') | KeyCode::PageDown => {
                if model.viewport.can_scroll_down() {
                    Some(Message::PageDown)
                } else {
                    None
                }
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                if model.viewport.can_scroll_up() {
                    Some(Message::PageUp)
                } else {
                    None
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_down() {
                    Some(Message::HalfPageDown)
                } else {
                    None
                }
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if model.viewport.can_scroll_up() {
                    Some(Message::HalfPageUp)
                } else {
                    None
                }
            }
            KeyCode::Char('g') | KeyCode::Home => Some(Message::GoToTop),
            KeyCode::Char('G') | KeyCode::End => Some(Message::GoToBottom),

            // TOC
            KeyCode::Char('t') => Some(Message::ToggleToc),
            KeyCode::Char('T') => Some(Message::ToggleTocFocus),
            KeyCode::Tab if model.toc_visible => Some(Message::SwitchFocus),

            // File
            KeyCode::Char('w') => Some(Message::ToggleWatch),
            KeyCode::Char('R') => Some(Message::ForceReload),
            KeyCode::Char('r') => Some(Message::ForceReload),
            KeyCode::Char('o') => Some(Message::OpenVisibleLinks),
            KeyCode::Char('?') | KeyCode::F(1) => Some(Message::ToggleHelp),

            // Search
            KeyCode::Char('/') => Some(Message::StartSearch),
            KeyCode::Esc => Some(Message::ClearSearch),

            // Quit
            KeyCode::Char('q') => Some(Message::Quit),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                Some(Message::Quit)
            }

            _ => None,
        }
    }

    pub(super) fn view(&self, model: &mut Model, frame: &mut Frame) {
        crate::ui::render(model, frame);
    }

    fn link_at_column(
        &self,
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
        let mut best: Option<(usize, crate::document::LinkRef)> = None;
        for link in links_on_line {
            let mut search = 0usize;
            while search < line_text.len() {
                let Some(rel) = line_text[search..].find(&link.text) else {
                    break;
                };
                let start_byte = search + rel;
                let start_char = line_text[..start_byte].chars().count();
                let end_char = start_char + link.text.chars().count();
                if content_col >= start_char && content_col < end_char {
                    return Some(link);
                }
                let dist = if content_col >= start_char {
                    content_col - start_char
                } else {
                    start_char - content_col
                };
                if best.as_ref().is_none_or(|(best_dist, _)| dist < *best_dist) {
                    best = Some((dist, link.clone()));
                }
                search = start_byte + 1;
            }
        }
        best.map(|(_, link)| link)
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
        crate::ui::split_main_columns(total_area)[1]
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

fn point_in_rect(col: u16, row: u16, rect: Rect) -> bool {
    col >= rect.x
        && col < rect.x + rect.width
        && row >= rect.y
        && row < rect.y + rect.height
}

fn doc_line_for_row(
    model: &Model,
    doc_area: Rect,
    row: u16,
    clamp: bool,
) -> Option<usize> {
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
