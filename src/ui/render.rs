use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::Model;
use crate::document::LineType;

use super::{
    DOC_WIDTH_PERCENT, DOCUMENT_LEFT_PADDING, TOC_WIDTH_PERCENT, images, overlays, status,
};

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

    if model.editor_mode {
        render_editor(model, frame, area);
        return;
    }

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
    if model.browse_mode {
        render_browse_toc(model, frame, area);
    } else {
        render_heading_toc(model, frame, area);
    }
}

fn render_heading_toc(model: &Model, frame: &mut Frame, area: Rect) {
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
            let marker = if model.toc_selected == Some(i) {
                ">"
            } else {
                " "
            };
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

fn render_browse_toc(model: &Model, frame: &mut Frame, area: Rect) {
    let entries = &model.browse_entries;
    let visible_rows = area.height.saturating_sub(2) as usize;
    let max_start = entries.len().saturating_sub(visible_rows);
    let start = model.toc_scroll_offset.min(max_start);
    let end = (start + visible_rows).min(entries.len());

    let items: Vec<Line> = entries
        .iter()
        .enumerate()
        .skip(start)
        .take(end.saturating_sub(start))
        .map(|(i, entry)| {
            let marker = if model.toc_selected == Some(i) {
                ">"
            } else {
                " "
            };
            let display_name = if entry.is_dir && entry.name != ".." {
                format!("{}/", entry.name)
            } else {
                entry.name.clone()
            };
            let style = if entry.is_dir {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let style = if model.toc_selected == Some(i) {
                style.reversed()
            } else {
                style
            };
            Line::styled(format!("{marker} {display_name}"), style)
        })
        .collect();

    let title = model.browse_dir.file_name().map_or_else(
        || model.browse_dir.display().to_string(),
        |n| n.to_string_lossy().to_string(),
    );

    let toc_block = Block::default()
        .title(title)
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
        y: area.y + area.height.saturating_sub(1 + u16::from(search_active)),
        height: 1,
        ..area
    };
    let toast_area = Rect {
        y: area.y
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
            if let Some(query) = model
                .search_query
                .as_deref()
                .filter(|q| q.chars().count() >= 3)
            {
                styled_spans = highlight_spans(&styled_spans, query);
            }
            if selected {
                styled_spans = apply_selection_bg(styled_spans, Color::DarkGray);
            }
            content.push(Line::from(styled_spans));
        } else {
            let mut styled_spans = vec![Span::styled(line.content().to_string(), line_style)];
            if let Some(query) = model
                .search_query
                .as_deref()
                .filter(|q| q.chars().count() >= 3)
            {
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

fn render_editor(model: &Model, frame: &mut Frame, area: Rect) {
    let Some(buf) = &model.editor_buffer else {
        return;
    };

    let toast_active = model.active_toast().is_some();
    let footer_rows = 1 + u16::from(toast_active);
    let editor_area = Rect {
        height: area.height.saturating_sub(footer_rows),
        ..area
    };
    let toast_area = Rect {
        y: area.y + area.height.saturating_sub(1 + u16::from(toast_active)),
        height: 1,
        ..area
    };
    let status_area = Rect {
        y: area.y + area.height.saturating_sub(1),
        height: 1,
        ..area
    };

    // Line number gutter width
    let total_lines = buf.line_count();
    let gutter_width = line_number_width(total_lines);

    let visible_height = editor_area.height as usize;
    let start = model.editor_scroll_offset;
    let end = (start + visible_height).min(total_lines);
    let cursor = buf.cursor();

    let mut content: Vec<Line> = Vec::new();
    for line_idx in start..end {
        let line_text = buf.line_at(line_idx).unwrap_or_default();
        let line_num = format!("{:>width$} ", line_idx + 1, width = gutter_width as usize);

        let mut spans = vec![Span::styled(line_num, Style::default().fg(Color::DarkGray))];

        if line_idx == cursor.line {
            // Split line at cursor position for cursor rendering
            let col = cursor.col.min(line_text.len());
            let before = &line_text[..col];
            let cursor_char = line_text.get(col..col + 1).unwrap_or(" ");
            let after = if col < line_text.len() {
                &line_text[col + 1..]
            } else {
                ""
            };

            if !before.is_empty() {
                spans.push(Span::raw(before.to_string()));
            }
            spans.push(Span::styled(
                cursor_char.to_string(),
                Style::default().bg(Color::White).fg(Color::Black),
            ));
            if !after.is_empty() {
                spans.push(Span::raw(after.to_string()));
            }
        } else {
            spans.push(Span::raw(line_text));
        }

        content.push(Line::from(spans));
    }

    let doc = Paragraph::new(content);
    frame.render_widget(Clear, editor_area);
    frame.render_widget(doc, editor_area);

    // Render toast if active
    if toast_active {
        status::render_toast_bar(model, frame, toast_area);
    }

    // Render editor status bar
    render_editor_status_bar(model, frame, status_area);
}

fn render_editor_status_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let filename = model.file_path.file_name().map_or_else(
        || "untitled".to_string(),
        |s| s.to_string_lossy().to_string(),
    );

    let dirty = model
        .editor_buffer
        .as_ref()
        .is_some_and(crate::editor::EditorBuffer::is_dirty);
    let dirty_indicator = if dirty { " [modified]" } else { "" };

    let cursor_info = model.editor_buffer.as_ref().map_or_else(String::new, |b| {
        let c = b.cursor();
        format!("  Ln {}, Col {}", c.line + 1, c.col + 1)
    });

    let status = format!(" EDIT  {filename}{dirty_indicator}{cursor_info}  Esc:view  Ctrl+S:save");

    let status_bar =
        Paragraph::new(status).style(Style::default().bg(Color::Magenta).fg(Color::White));

    frame.render_widget(status_bar, area);
}

/// Calculate the width needed for line numbers.
pub const fn line_number_width(total_lines: usize) -> u16 {
    if total_lines < 10 {
        1
    } else if total_lines < 100 {
        2
    } else if total_lines < 1_000 {
        3
    } else if total_lines < 10_000 {
        4
    } else if total_lines < 100_000 {
        5
    } else {
        6
    }
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
