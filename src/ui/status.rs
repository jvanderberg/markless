use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::app::Model;

pub fn render_hover_link_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let Some(url) = model.hovered_link_url.as_deref() else {
        return;
    };
    let bar = Paragraph::new(format!("link: {url}"))
        .style(Style::default().bg(Color::Blue).fg(Color::White));
    frame.render_widget(bar, area);
}

pub fn render_search_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let query = model.search_query.as_deref().unwrap_or_default();
    let large_file = model.document.line_count() > 10_000;
    let deferred = large_file && model.search_match_count() == 0 && !query.trim().is_empty();
    let match_info = if query.trim().is_empty() {
        String::new()
    } else if deferred {
        "  [Enter to search]".to_string()
    } else if let Some((current, total)) = model.current_search_match() {
        format!("  [{current}/{total}]")
    } else {
        String::new()
    };
    let text = format!("/{}{}  Enter: next  Esc: clear", query, match_info);
    let bar = Paragraph::new(text).style(Style::default().bg(Color::Blue).fg(Color::White));
    frame.render_widget(bar, area);
}

pub fn render_status_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let filename = model
        .file_path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "untitled".to_string());

    let percent = model.viewport.scroll_percent();
    let line_info = format!(
        "Line {}/{}",
        model.viewport.offset() + 1,
        model.viewport.total_lines()
    );

    let watch_indicator = if model.watch_enabled {
        " [watching]"
    } else {
        ""
    };
    let toc_indicator = if model.toc_visible { " [TOC]" } else { "" };

    let status = format!(
        " {}  [{}%]  {}{}{}  ?:help",
        filename, percent, line_info, watch_indicator, toc_indicator
    );

    let status_bar =
        Paragraph::new(status).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    frame.render_widget(status_bar, area);
}

pub fn render_toast_bar(model: &Model, frame: &mut Frame, area: Rect) {
    let Some((message, level)) = model.active_toast() else {
        return;
    };
    let (prefix, style) = match level {
        crate::app::ToastLevel::Info => (
            "[info]",
            Style::default().bg(Color::DarkGray).fg(Color::White),
        ),
        crate::app::ToastLevel::Warning => (
            "[warn]",
            Style::default().bg(Color::Yellow).fg(Color::Black),
        ),
        crate::app::ToastLevel::Error => {
            ("[error]", Style::default().bg(Color::Red).fg(Color::White))
        }
    };
    let toast = Paragraph::new(format!("{} {}", prefix, message)).style(style);
    frame.render_widget(toast, area);
}
