use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph};

use crate::app::Model;

pub fn link_picker_rect(area: Rect, items_len: usize) -> Rect {
    let popup_width = area.width.saturating_sub(16).max(44);
    // Link picker has at most a handful of items
    #[allow(clippy::cast_possible_truncation)]
    let needed_rows = (items_len as u16 * 2) + 4;
    let popup_height = needed_rows.min(area.height.saturating_sub(4).max(8));
    centered_popup_rect(popup_width, popup_height, area)
}

pub const fn link_picker_content_top(popup: Rect) -> u16 {
    // 1 row for border + 1 row for padding
    popup.y + 2
}

pub fn render_link_picker_overlay(model: &Model, frame: &mut Frame, area: Rect) {
    let items = &model.link_picker_items;
    if items.is_empty() {
        return;
    }
    let popup = link_picker_rect(area, items.len());

    let mut lines: Vec<Line> = Vec::new();
    for (idx, link) in items.iter().enumerate() {
        let title = if link.text.trim().is_empty() {
            "(untitled link)"
        } else {
            link.text.as_str()
        };
        let left_margin = "   ";
        let number = format!("{}: ", idx + 1);
        lines.push(Line::from(vec![
            Span::raw(left_margin),
            Span::styled(
                number,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                title.to_string(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
        ]));
        lines.push(Line::from(vec![
            Span::raw(left_margin),
            Span::raw("   "),
            Span::styled(link.url.clone(), Style::default().fg(Color::Cyan)),
        ]));
    }
    lines.push(Line::raw(" "));
    lines.push(Line::from(vec![
        Span::raw("   "),
        Span::styled(
            "1-9 open · any key or click outside cancels",
            Style::default().fg(Color::Indexed(245)),
        ),
    ]));

    let block = Block::default()
        .title("Open Link")
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Black).fg(Color::White));
    frame.render_widget(Clear, popup);
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

pub fn render_help_overlay(model: &Model, frame: &mut Frame, area: Rect) {
    let popup_width = area.width.saturating_sub(12).max(48);
    let popup_height = area.height.saturating_sub(6).max(12);
    let popup = centered_popup_rect(popup_width, popup_height, area);

    let global_cfg = model
        .config_global_path
        .as_ref()
        .map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string());
    let local_cfg = model
        .config_local_path
        .as_ref()
        .map_or_else(|| "<none>".to_string(), |p| p.display().to_string());

    let section_style = Style::default()
        .fg(Color::Yellow)
        .add_modifier(Modifier::BOLD);
    let dim_style = Style::default().fg(Color::Indexed(245));

    let mut all_lines: Vec<Line> = Vec::new();

    // Navigation
    all_lines.push(Line::styled("Navigation", section_style));
    all_lines.push(Line::raw("  j/k or Up/Down      Scroll"));
    all_lines.push(Line::raw("  Space/PageDown      Page down"));
    all_lines.push(Line::raw("  b/PageUp            Page up"));
    all_lines.push(Line::raw("  Ctrl-d / Ctrl-u     Half page"));
    all_lines.push(Line::raw("  g / G               Top / bottom"));
    all_lines.push(Line::raw(""));

    // Search
    all_lines.push(Line::styled("Search", section_style));
    all_lines.push(Line::raw("  /                   Start search"));
    all_lines.push(Line::raw("  Enter               Next match"));
    all_lines.push(Line::raw("  Esc                 Clear search"));
    all_lines.push(Line::raw(""));

    // TOC
    all_lines.push(Line::styled("TOC", section_style));
    all_lines.push(Line::raw("  t                   Toggle TOC"));
    all_lines.push(Line::raw("  T                   Toggle + focus TOC"));
    all_lines.push(Line::raw("  Tab                 Switch focus"));
    all_lines.push(Line::raw("  j/k, arrows, Enter/Space, mouse, click"));
    all_lines.push(Line::raw("  h / Left            Collapse / parent dir"));
    all_lines.push(Line::raw("  l / Right           Expand / enter dir"));
    all_lines.push(Line::raw(""));

    // Browse
    all_lines.push(Line::styled("Browse", section_style));
    all_lines.push(Line::raw("  B                   Browse directory"));
    all_lines.push(Line::raw("  F                   Focus on file only"));
    all_lines.push(Line::raw("  Backspace           Parent directory (in TOC)"));
    all_lines.push(Line::raw(""));

    // Editor
    all_lines.push(Line::styled("Editor", section_style));
    all_lines.push(Line::raw("  e                   Enter edit mode"));
    all_lines.push(Line::raw("  Ctrl-e              Toggle edit mode"));
    all_lines.push(Line::raw("  Esc                 Return to view mode"));
    all_lines.push(Line::raw("  Ctrl-s              Save file"));
    all_lines.push(Line::raw("  Arrows, Home/End    Navigate"));
    all_lines.push(Line::raw("  Ctrl+Left/Right     Word movement"));
    all_lines.push(Line::raw("  Ctrl+Home/End       Buffer start / end"));
    all_lines.push(Line::raw("  PageUp/PageDown     Scroll editor"));
    all_lines.push(Line::raw(""));

    // Other
    all_lines.push(Line::styled("Other", section_style));
    all_lines.push(Line::raw("  w                   Toggle watch"));
    all_lines.push(Line::raw("  r / R               Reload file"));
    all_lines.push(Line::raw("  o                   Open visible links (1-9)"));
    all_lines.push(Line::raw("  q / Ctrl-c / Ctrl-q Quit"));
    all_lines.push(Line::raw("  ? / F1              Toggle help"));
    all_lines.push(Line::raw("  Mouse drag          Select lines + copy"));
    all_lines.push(Line::raw(""));

    // Config
    all_lines.push(Line::styled("Config", section_style));
    all_lines.push(Line::raw(format!("  Global: {global_cfg}")));
    all_lines.push(Line::raw(format!("  Local override: {local_cfg}")));

    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Black).fg(Color::White));

    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);

    // Inner area: border(1) + padding(1) on each side = 4
    let inner = Rect::new(
        popup.x + 2,
        popup.y + 2,
        popup.width.saturating_sub(4),
        popup.height.saturating_sub(4),
    );

    // Reserve 1 row at bottom for footer hint
    let content_height_u16 = inner.height.saturating_sub(1);
    let content_height = content_height_u16 as usize;
    let max_scroll = all_lines.len().saturating_sub(content_height);
    let scroll = model.help_scroll_offset.min(max_scroll);

    let end = (scroll + content_height).min(all_lines.len());
    let visible: Vec<Line> = all_lines[scroll..end].to_vec();

    let content_area = Rect::new(inner.x, inner.y, inner.width, content_height_u16);
    frame.render_widget(Paragraph::new(visible), content_area);

    // Footer hint
    let footer_area = Rect::new(inner.x, inner.y + content_height_u16, inner.width, 1);
    let footer = Line::styled("j/k scroll \u{2502} Esc closes", dim_style);
    frame.render_widget(Paragraph::new(footer), footer_area);
}

fn centered_popup_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w) / 2);
    let y = area.y + (area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}
