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
    let popup_height = area.height.saturating_sub(10).max(12);
    let popup = centered_popup_rect(popup_width, popup_height, area);

    let left_text = "\
Navigation
  j/k or Up/Down      Scroll
  Space/PageDown      Page down
  b/PageUp            Page up
  Ctrl-d / Ctrl-u     Half page
  g / G               Top / bottom

Search
  /                   Start search
  Enter               Next match
  Esc                 Clear search";

    let global_cfg = model
        .config_global_path
        .as_ref()
        .map_or_else(|| "<unknown>".to_string(), |p| p.display().to_string());
    let local_cfg = model
        .config_local_path
        .as_ref()
        .map_or_else(|| "<none>".to_string(), |p| p.display().to_string());

    let right_text = "\
TOC
  t                   Toggle TOC
  T                   Toggle + focus TOC
  Tab                 Switch focus
  TOC: j/k, arrows, Enter/Space, mouse, click

Browse
  B                   Browse directory
  F                   Focus on file only
  Backspace           Parent directory (in TOC)

Editor
  e                   Enter edit mode
  Esc                 Return to view mode
  Ctrl-s              Save file

Other
  w                   Toggle watch
  r or R              Reload file
  o                   Open visible links (1-9)
                      title on first row, full URL on second
  q or Ctrl-c         Quit
  ? or F1             Toggle help (closes on any key)
  Mouse drag          Select lines + copy";

    let config_text = format!(
        "\
Config
  Global: {global_cfg}
  Local override: {local_cfg}"
    );

    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .padding(Padding::uniform(1))
        .style(Style::default().bg(Color::Black).fg(Color::White));

    frame.render_widget(Clear, popup);
    frame.render_widget(block, popup);

    let inner = Rect::new(
        popup.x + 2,
        popup.y + 2,
        popup.width.saturating_sub(4),
        popup.height.saturating_sub(4),
    );
    let config_rows = 3;
    let main_height = inner.height.saturating_sub(config_rows);
    let main_area = Rect::new(inner.x, inner.y, inner.width, main_height);
    let config_area = Rect::new(
        inner.x,
        inner.y + main_height,
        inner.width,
        inner.height.saturating_sub(main_height),
    );

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_area);
    if let (Some(&left_area), Some(&right_area)) = (cols.first(), cols.get(1)) {
        frame.render_widget(Paragraph::new(left_text), left_area);
        frame.render_widget(Paragraph::new(right_text), right_area);
    }
    frame.render_widget(Paragraph::new(config_text), config_area);
}

fn centered_popup_rect(width: u16, height: u16, area: Rect) -> Rect {
    let w = width.min(area.width);
    let h = height.min(area.height);
    let x = area.x + (area.width.saturating_sub(w) / 2);
    let y = area.y + (area.height.saturating_sub(h) / 2);
    Rect::new(x, y, w, h)
}
