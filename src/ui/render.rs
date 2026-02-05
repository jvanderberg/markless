use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::Model;
use crate::document::LineType;

use super::{images, overlays, status, DOCUMENT_LEFT_PADDING, DOC_WIDTH_PERCENT, TOC_WIDTH_PERCENT};

pub fn split_main_columns(area: Rect) -> std::rc::Rc<[Rect]> {
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(TOC_WIDTH_PERCENT),
            Constraint::Percentage(DOC_WIDTH_PERCENT),
        ])
        .split(area)
}

pub fn document_content_width(total_width: u16, toc_visible: bool) -> u16 {
    let area = Rect::new(0, 0, total_width, 1);
    let doc_width = if toc_visible {
        split_main_columns(area)[1].width
    } else {
        total_width
    };
    doc_width.saturating_sub(DOCUMENT_LEFT_PADDING).max(1)
}

/// Render the complete UI.
pub fn render(model: &mut Model, frame: &mut Frame) {
    let area = frame.area();

    if model.toc_visible {
        // Split into TOC and document
        let chunks = split_main_columns(area);
        render_toc(model, frame, chunks[0]);
        render_document(model, frame, chunks[1]);
    } else {
        render_document(model, frame, area);
    }

    if model.help_visible {
        overlays::render_help_overlay(model, frame, area);
    } else if model.link_picker_active() {
        overlays::render_link_picker_overlay(model, frame, area);
    }
}

fn render_toc(model: &Model, frame: &mut Frame, area: Rect) {
    let headings = model.document.headings();
    let visible_rows = area.height.saturating_sub(2) as usize;
    let max_start = headings.len().saturating_sub(visible_rows);
    let start = model.toc_scroll_offset.min(max_start);
    let end = (start + visible_rows).min(headings.len());

    let items: Vec<Line> = headings
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(i, h)| {
            let indent = "  ".repeat(h.level.saturating_sub(1) as usize);
            let marker = if model.toc_selected == Some(i) { ">" } else { " " };
            let base_style = super::style::style_for_line_type(&LineType::Heading(h.level));
            let style = if model.toc_selected == Some(i) {
                base_style.reversed()
            } else {
                base_style
            };
            Line::styled(format!("{}{} {}", marker, indent, h.text), style)
        })
        .collect();

    let toc_block = Block::default()
        .title("Table of Contents")
        .borders(Borders::ALL)
        .border_style(if model.toc_focused {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        });

    let toc = Paragraph::new(items).block(toc_block);
    frame.render_widget(toc, area);
}

fn render_document(model: &mut Model, frame: &mut Frame, area: Rect) {
    let search_active = model.search_query.is_some();
    let toast_active = model.active_toast().is_some();
    let hover_active = model.hovered_link_url.is_some();
    let footer_rows =
        1 + u16::from(search_active) + u16::from(toast_active) + u16::from(hover_active);
    // Reserve last line for status bar (+ one search bar line when active).
    let doc_outer_area = Rect {
        height: area.height.saturating_sub(footer_rows),
        ..area
    };
    let search_area = Rect {
        y: area
            .y
            + area
                .height
                .saturating_sub(1 + u16::from(search_active)),
        height: 1,
        ..area
    };
    let toast_area = Rect {
        y: area
            .y
            + area
                .height
                .saturating_sub(1 + u16::from(search_active) + u16::from(toast_active)),
        height: 1,
        ..area
    };
    let hover_area = Rect {
        y: area.y
            + area.height.saturating_sub(
                1 + u16::from(search_active) + u16::from(toast_active) + u16::from(hover_active),
            ),
        height: 1,
        ..area
    };
    let status_area = Rect {
        y: area.y + area.height.saturating_sub(1),
        height: 1,
        ..area
    };

    // Render document content with styling
    let visible_lines = model
        .document
        .visible_lines(model.viewport.offset(), model.viewport.height() as usize);
    let selection = model.selection_range();

    // Build text content
    let mut content: Vec<Line> = Vec::new();
    for (idx, line) in visible_lines.iter().enumerate() {
        let line_idx = model.viewport.offset() + idx;
        let selected = selection
            .as_ref()
            .is_some_and(|range| range.contains(&line_idx));
        let line_style = super::style::style_for_line_type(line.line_type());
        if let Some(spans) = line.spans() {
            let mut styled_spans = spans
                .iter()
                .map(|span| {
                    Span::styled(
                        span.text().to_string(),
                        super::style::style_for_inline(line_style, span.style()),
                    )
                })
                .collect::<Vec<_>>();
            if let Some(query) = model.search_query.as_deref().filter(|q| q.chars().count() >= 3) {
                styled_spans = highlight_spans(&styled_spans, query);
            }
            if selected {
                styled_spans = apply_selection_bg(styled_spans, Color::DarkGray);
            }
            content.push(Line::from(styled_spans));
        } else {
            let mut styled_spans = vec![Span::styled(line.content().to_string(), line_style)];
            if let Some(query) = model.search_query.as_deref().filter(|q| q.chars().count() >= 3) {
                styled_spans = highlight_spans(&styled_spans, query);
            }
            if selected {
                styled_spans = apply_selection_bg(styled_spans, Color::DarkGray);
            }
            content.push(Line::from(styled_spans));
        }
    }

    let doc_block = Block::default()
        .borders(Borders::NONE)
        .padding(Padding::left(DOCUMENT_LEFT_PADDING));
    let doc_area = doc_block.inner(doc_outer_area);
    let doc = Paragraph::new(content).block(doc_block);
    // Clear doc area first so placeholder/image background styles from previous frames do not leak.
    frame.render_widget(Clear, doc_outer_area);
    frame.render_widget(doc, doc_outer_area);

    if model.images_enabled {
        images::render_images(model, frame, doc_area);
    }

    // Render status bar
    if hover_active {
        status::render_hover_link_bar(model, frame, hover_area);
    }
    if toast_active {
        status::render_toast_bar(model, frame, toast_area);
    }
    if search_active {
        status::render_search_bar(model, frame, search_area);
    }
    status::render_status_bar(model, frame, status_area);
}

fn highlight_spans(spans: &[Span<'_>], query: &str) -> Vec<Span<'static>> {
    let needle = query.trim();
    if needle.is_empty() {
        return spans
            .iter()
            .map(|s| Span::styled(s.content.to_string(), s.style))
            .collect();
    }
    let needle_lower = needle.to_ascii_lowercase();
    let mut out = Vec::new();

    for span in spans {
        let text = span.content.to_string();
        let text_lower = text.to_ascii_lowercase();
        let mut cursor = 0usize;

        while let Some(rel_idx) = text_lower[cursor..].find(&needle_lower) {
            let start = cursor + rel_idx;
            let end = start + needle_lower.len();

            if start > cursor {
                out.push(Span::styled(text[cursor..start].to_string(), span.style));
            }
            out.push(Span::styled(
                text[start..end].to_string(),
                span.style.bg(Color::Yellow).fg(Color::Black),
            ));
            cursor = end;
        }

        if cursor < text.len() {
            out.push(Span::styled(text[cursor..].to_string(), span.style));
        }
    }

    out
}

fn apply_selection_bg(spans: Vec<Span<'static>>, bg: Color) -> Vec<Span<'static>> {
    spans
        .into_iter()
        .map(|span| {
            let mut style = span.style;
            if style.bg.is_none() || style.bg == Some(Color::Reset) {
                style = style.bg(bg);
            }
            Span::styled(span.content.to_string(), style)
        })
        .collect()
}
