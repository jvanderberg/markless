use std::path::PathBuf;
use std::time::{Duration, Instant};

use crossterm::event::{self, KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;
use tempfile::tempdir;

use crate::document::Document;

use super::event_loop::{BrowseDebouncer, ResizeDebouncer};
use super::{App, Message, Model, ToastLevel, update};

fn create_test_model() -> Model {
    let doc = Document::parse("# Test\n\nHello world").unwrap();
    Model::new(PathBuf::from("test.md"), doc, (80, 24))
}

fn create_long_test_model() -> Model {
    // Create a document with 100 lines so we can test scrolling
    let mut md = String::from("# Test Document\n\n");
    for i in 1..=50 {
        md.push_str(&format!("Line {} of content.\n\n", i));
    }
    let doc = Document::parse(&md).unwrap();
    Model::new(PathBuf::from("test.md"), doc, (80, 24))
}

fn create_many_headings_model() -> Model {
    let mut md = String::new();
    for i in 1..=20 {
        md.push_str(&format!("## Heading {}\n\nBody {}\n\n", i, i));
    }
    let doc = Document::parse(&md).unwrap();
    Model::new(PathBuf::from("test.md"), doc, (80, 8))
}

#[test]
fn test_scroll_down_updates_viewport() {
    let model = create_long_test_model();
    let model = update(model, Message::ScrollDown(5));
    assert_eq!(model.viewport.offset(), 5);
}

#[test]
fn test_scroll_up_updates_viewport() {
    let mut model = create_long_test_model();
    model.viewport.scroll_down(10);
    let model = update(model, Message::ScrollUp(3));
    assert_eq!(model.viewport.offset(), 7);
}

#[test]
fn test_toggle_toc_changes_visibility() {
    let model = create_test_model();
    assert!(!model.toc_visible);

    let model = update(model, Message::ToggleToc);
    assert!(model.toc_visible);

    let model = update(model, Message::ToggleToc);
    assert!(!model.toc_visible);
}

#[test]
fn test_toggle_toc_selects_first_entry() {
    let model = create_test_model();
    assert!(model.toc_selected.is_none());

    let model = update(model, Message::ToggleToc);
    assert_eq!(model.toc_selected, Some(0));
}

#[test]
fn test_toggle_watch_changes_state() {
    let model = create_test_model();
    assert!(!model.watch_enabled);

    let model = update(model, Message::ToggleWatch);
    assert!(model.watch_enabled);
}

#[test]
fn test_force_reload_reloads_document_from_disk() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("doc.md");
    std::fs::write(&file_path, "# One\n\nalpha").unwrap();

    let doc = Document::parse_with_layout("# One\n\nalpha", 80).unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));

    std::fs::write(&file_path, "# Two\n\nbeta\n\nmore").unwrap();
    model.reload_from_disk().unwrap();

    assert!(model.document.source().contains("# Two"));
    assert!(model.document.line_count() >= 3);
}

#[test]
fn test_force_reload_message_triggers_reload_side_effect() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("doc.md");
    std::fs::write(&file_path, "# One\n\nalpha").unwrap();
    let doc = Document::parse_with_layout("# One\n\nalpha", 80).unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    let app = App::new(file_path.clone());
    let mut watcher = None;

    std::fs::write(&file_path, "# Updated\n\nbeta").unwrap();
    model = update(model, Message::ForceReload);
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::ForceReload);

    assert!(model.document.source().contains("# Updated"));
}

#[test]
fn test_toggle_help_changes_visibility() {
    let model = create_test_model();
    assert!(!model.help_visible);

    let model = update(model, Message::ToggleHelp);
    assert!(model.help_visible);

    let model = update(model, Message::HideHelp);
    assert!(!model.help_visible);
}

#[test]
fn test_selection_range_orders_lines() {
    let doc = Document::parse("# Title\n\nLine one\n\nLine two").unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    model = update(model, Message::StartSelection(4));
    model = update(model, Message::UpdateSelection(2));
    model = update(model, Message::EndSelection(2));

    let range = model.selection_range().unwrap();
    assert_eq!(*range.start(), 2);
    assert_eq!(*range.end(), 4);
}

#[test]
fn test_selected_text_returns_block() {
    let doc = Document::parse("# Title\n\nLine one\n\nLine two").unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    model = update(model, Message::StartSelection(4));
    model = update(model, Message::UpdateSelection(6));
    model = update(model, Message::EndSelection(6));

    let (text, lines) = model.selected_text().unwrap();
    assert_eq!(lines, 3);
    assert_eq!(text, "Line one\n\nLine two");
}

#[test]
fn test_selected_text_strips_code_block_borders() {
    let md = "```rust\nlet x = 1;\nlet y = 2;\n```";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    let mut top = None;
    let mut bottom = None;
    for idx in 0..model.document.line_count() {
        let line = model.document.line_at(idx).expect("line missing");
        if line.content().starts_with('┌') {
            top = Some(idx);
        } else if line.content().starts_with('└') {
            bottom = Some(idx);
        }
    }
    let top = top.expect("top border missing");
    let bottom = bottom.expect("bottom border missing");

    model = update(model, Message::StartSelection(top));
    model = update(model, Message::UpdateSelection(bottom));
    model = update(model, Message::EndSelection(bottom));

    let (text, lines) = model.selected_text().unwrap();
    assert_eq!(lines, 2);
    assert_eq!(text, "let x = 1;\nlet y = 2;");
    assert!(!text.contains('┌'));
    assert!(!text.contains('└'));
    assert!(!text.contains("│ "));
}

#[test]
fn test_selection_clears_after_mouse_up() {
    let doc = Document::parse("# Title\n\nLine one\n\nLine two").unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    model = update(model, Message::StartSelection(2));
    model = update(model, Message::UpdateSelection(4));
    model = update(model, Message::EndSelection(4));
    assert!(model.selection.is_some());

    app.handle_message_side_effects(&mut model, &mut watcher, &Message::EndSelection(4));

    assert!(model.selection.is_none());
}

#[test]
fn test_selected_text_uses_link_urls_for_copy() {
    let md = "See [one](https://one.test) and [two](https://two.test).";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    model = update(model, Message::StartSelection(0));
    model = update(model, Message::UpdateSelection(0));
    model = update(model, Message::EndSelection(0));

    let (text, _) = model.selected_text().unwrap();
    assert_eq!(text, "See https://one.test and https://two.test.");
}

#[test]
fn test_help_toggle_works_when_toc_focused() {
    let model = create_test_model();
    let model = update(model, Message::ToggleTocFocus);
    assert!(model.toc_visible);
    assert!(model.toc_focused);

    let app = App::new(PathBuf::from("test.md"));
    let key = event::KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let msg = app.handle_key(key, &model);
    assert_eq!(msg, Some(Message::ToggleHelp));
}

#[test]
fn test_toast_lifecycle() {
    let mut model = create_test_model();
    model.show_toast(ToastLevel::Warning, "watch failed");
    let (msg, level) = model.active_toast().expect("toast should be set");
    assert_eq!(msg, "watch failed");
    assert_eq!(level, ToastLevel::Warning);
    assert!(!model.expire_toast(Instant::now()));
    assert!(model.expire_toast(Instant::now() + Duration::from_secs(5)));
    assert!(model.active_toast().is_none());
}

#[test]
fn test_toc_down_scrolls_when_selection_hits_bottom_row() {
    let mut model = create_many_headings_model();
    model.toc_visible = true;
    model.toc_selected = Some(5);
    model.toc_scroll_offset = 0;

    let model = update(model, Message::TocDown);
    assert_eq!(model.toc_selected, Some(6));
    assert_eq!(model.toc_scroll_offset, 1);
}

#[test]
fn test_toc_up_scrolls_when_selection_hits_top_row() {
    let mut model = create_many_headings_model();
    model.toc_visible = true;
    model.toc_selected = Some(3);
    model.toc_scroll_offset = 3;

    let model = update(model, Message::TocUp);
    assert_eq!(model.toc_selected, Some(2));
    assert_eq!(model.toc_scroll_offset, 2);
}

#[test]
fn test_toc_auto_sync_selects_heading_near_viewport_top() {
    let mut model = create_many_headings_model();
    model = update(model, Message::ToggleToc);

    let target_idx = 8;
    let target_line = model.document.headings()[target_idx].line;
    model = update(model, Message::GoToLine(target_line));

    assert_eq!(model.toc_selected, Some(target_idx));
    assert_eq!(
        model.toc_scroll_offset,
        target_idx.min(model.max_toc_scroll_offset())
    );
}

#[test]
fn test_toc_auto_sync_picks_previous_on_equal_distance() {
    let mut model = create_many_headings_model();
    model = update(model, Message::ToggleToc);

    let headings = model.document.headings();
    let between = (headings[5].line + headings[6].line) / 2;
    model = update(model, Message::GoToLine(between));

    assert_eq!(model.toc_selected, Some(5));
}

#[test]
fn test_quit_sets_should_quit() {
    let model = create_test_model();
    let model = update(model, Message::Quit);
    assert!(model.should_quit);
}

#[test]
fn test_resize_updates_viewport() {
    let model = create_test_model();
    let model = update(model, Message::Resize(120, 40));
    assert_eq!(model.viewport.width(), 120);
    assert_eq!(model.viewport.height(), 39); // -1 for status bar
}

#[test]
fn test_resize_reflows_document_using_content_width() {
    let md = "This is a line that should wrap after resize because the content area is narrower.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = update(model, Message::Resize(20, 24));
    let lines = model.document.visible_lines(0, 20);
    let paragraph_lines: Vec<_> = lines
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .collect();
    assert!(
        paragraph_lines.len() > 1,
        "expected wrapped paragraph lines"
    );
    for line in paragraph_lines {
        assert!(
            line.content().len() <= 18,
            "line should wrap to content width (20 - 2 padding): {}",
            line.content()
        );
    }
}

#[test]
fn test_toggle_toc_reflows_document_to_narrower_width() {
    let md = "This is a long paragraph line used to verify that enabling TOC reduces wrapping width and forces narrower rendered lines in the document pane.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let before_max = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .max()
        .unwrap_or(0);

    let model = update(model, Message::ToggleToc);
    let after_max = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .max()
        .unwrap_or(0);
    assert!(
        after_max < before_max,
        "expected narrower lines when TOC is visible (before={}, after={})",
        before_max,
        after_max
    );
}

#[test]
fn test_resize_with_toc_visible_reflows_using_toc_width() {
    let md = "A long paragraph line used to verify wrapping width honors the visible TOC pane.";
    let doc = Document::parse_with_layout(md, 100).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (100, 24));
    model = update(model, Message::ToggleToc);
    model = update(model, Message::Resize(60, 24));

    let expected_max = crate::ui::document_content_width(60, true) as usize;
    for line in model.document.visible_lines(0, 50) {
        if *line.line_type() == crate::document::LineType::Paragraph {
            assert!(
                line.content().chars().count() <= expected_max,
                "line exceeds TOC-aware width: {} > {} ({})",
                line.content().chars().count(),
                expected_max,
                line.content()
            );
        }
    }
}

#[test]
fn test_go_to_top() {
    let mut model = create_test_model();
    model.viewport.scroll_down(100);
    let model = update(model, Message::GoToTop);
    assert_eq!(model.viewport.offset(), 0);
}

#[test]
fn test_switch_focus_toggles_between_toc_and_document() {
    let mut model = create_test_model();
    model.toc_visible = true;
    assert!(!model.toc_focused);

    let model = update(model, Message::SwitchFocus);
    assert!(model.toc_focused);

    let model = update(model, Message::SwitchFocus);
    assert!(!model.toc_focused);
}

#[test]
fn test_switch_focus_does_nothing_when_toc_hidden() {
    let model = create_test_model();
    assert!(!model.toc_visible);
    assert!(!model.toc_focused);

    let model = update(model, Message::SwitchFocus);
    assert!(!model.toc_focused);
}

#[test]
fn test_ensure_highlight_overscan_highlights_visible_code() {
    let doc = Document::parse_with_layout("```rust\nfn main() {}\n```", 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.ensure_highlight_overscan();

    let lines = model.document.visible_lines(0, 10);
    let code_line = lines
        .iter()
        .find(|l| l.content().contains("fn main"))
        .expect("code line missing");
    let spans = code_line.spans().expect("code spans missing");
    assert!(spans.iter().any(|s| s.style().fg.is_some()));
}

#[test]
fn test_ensure_highlight_overscan_still_highlights_while_scrolling() {
    let doc = Document::parse_with_layout("Lead line\n\n```rust\nfn main() {}\n```", 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    // Highlighting should still run while actively scrolling.
    model = update(model, Message::ScrollDown(1));

    model.ensure_highlight_overscan();
    let lines = model.document.visible_lines(model.viewport.offset(), 10);
    let code_line = lines
        .iter()
        .find(|l| l.content().contains("fn main"))
        .expect("code line missing");
    let spans = code_line.spans().expect("code spans missing");
    assert!(spans.iter().any(|s| s.style().fg.is_some()));
}

#[test]
fn test_search_input_finds_matches_and_jumps_to_first() {
    let doc = Document::parse("alpha\n\nbeta\n\nalpha again").unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    let model = update(model, Message::StartSearch);
    let model = update(model, Message::SearchInput("alpha".to_string()));

    assert_eq!(model.search_match_count(), 2);
    assert_eq!(model.current_search_match(), Some((1, 2)));
    assert_eq!(model.viewport.offset(), 0);
}

#[test]
fn test_next_match_wraps() {
    let doc = Document::parse("alpha\n\nbeta\n\nalpha again").unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = update(model, Message::StartSearch);
    let model = update(model, Message::SearchInput("alpha".to_string()));

    let model = update(model, Message::NextMatch);
    assert_eq!(model.current_search_match(), Some((2, 2)));

    let model = update(model, Message::NextMatch);
    assert_eq!(model.current_search_match(), Some((1, 2)));
}

#[test]
fn test_short_query_does_not_auto_search_until_enter() {
    let doc = Document::parse("alpha\n\nbeta\n\natom").unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = update(model, Message::StartSearch);
    let model = update(model, Message::SearchInput("a".to_string()));
    assert_eq!(model.search_match_count(), 0);

    let model = update(model, Message::NextMatch);
    assert!(model.search_match_count() > 0);
}

#[test]
fn test_search_mode_char_input_appends_query() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model = update(model, Message::StartSearch);
    model = update(model, Message::SearchInput("a".to_string()));

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::SearchInput("an".to_string())));
}

#[test]
fn test_search_mode_enter_moves_to_next_match() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model = update(model, Message::StartSearch);
    model = update(model, Message::SearchInput("test".to_string()));

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::NextMatch));
}

#[test]
fn test_toc_focus_space_selects_heading() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.toc_visible = true;
    model.toc_focused = true;
    model.toc_selected = Some(0);

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::TocSelect));
}

#[test]
fn test_question_mark_opens_help_when_not_searching() {
    let app = App::new(PathBuf::from("test.md"));
    let model = create_test_model();

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::ToggleHelp));
}

#[test]
fn test_help_mode_esc_closes_help() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.help_visible = true;

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HideHelp));
}

#[test]
fn test_help_mode_any_key_closes_help() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.help_visible = true;

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HideHelp));
}

#[test]
fn test_mouse_click_on_doc_link_emits_follow_message() {
    let app = App::new(PathBuf::from("test.md"));
    let doc = Document::parse_with_layout("[Link](https://example.com)", 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true; // mouse capture path

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(0, Some(0))));
}

#[test]
fn test_mouse_click_on_image_emits_follow_message() {
    let app = App::new(PathBuf::from("test.md"));
    let doc = Document::parse_with_layout("![Alt text](image.png)", 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(0, Some(0))));
}

#[test]
fn test_mouse_click_on_image_body_emits_follow_message() {
    let app = App::new(PathBuf::from("test.md"));
    let mut heights = std::collections::HashMap::new();
    heights.insert("image.png".to_string(), 3);
    let doc = Document::parse_with_layout_and_image_heights("![Alt text](image.png)", 80, &heights)
        .unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(1, Some(0))));
}

#[test]
fn test_mouse_click_on_image_body_after_press_emits_follow_message() {
    let app = App::new(PathBuf::from("test.md"));
    let mut heights = std::collections::HashMap::new();
    heights.insert("image.png".to_string(), 3);
    let doc = Document::parse_with_layout_and_image_heights("![Alt text](image.png)", 80, &heights)
        .unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let _ = app.handle_mouse(down, &model);
    let model = update(model, Message::StartSelection(1));
    let msg = app.handle_mouse(up, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(1, Some(0))));
}

#[test]
fn test_mouse_hover_on_doc_link_emits_hover_message() {
    let app = App::new(PathBuf::from("test.md"));
    let doc = Document::parse_with_layout("[Link](https://example.com)", 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let mouse = MouseEvent {
        kind: MouseEventKind::Moved,
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(
        msg,
        Some(Message::HoverLink(Some("https://example.com".to_string())))
    );
}

#[test]
fn test_mouse_hover_on_image_body_emits_hover_message() {
    let app = App::new(PathBuf::from("test.md"));
    let mut heights = std::collections::HashMap::new();
    heights.insert("image.png".to_string(), 3);
    let doc = Document::parse_with_layout_and_image_heights("![Alt text](image.png)", 80, &heights)
        .unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;
    let mouse = MouseEvent {
        kind: MouseEventKind::Moved,
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::HoverLink(Some("image.png".to_string()))));
}

#[test]
fn test_hover_prefers_link_at_column_when_multiple() {
    let app = App::new(PathBuf::from("test.md"));
    let md = "[Rust](https://rust-lang.org) and [GitHub](https://github.com)";
    let doc = Document::parse_with_layout(md, 120).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (120, 24));
    model.toc_visible = true;

    let line_text = model.document.line_at(0).unwrap().content();
    let github_pos = line_text.find("GitHub").unwrap();
    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 120, 24));
    let doc_x = chunks[1].x;
    let column = doc_x + crate::ui::DOCUMENT_LEFT_PADDING + github_pos as u16;

    let mouse = MouseEvent {
        kind: MouseEventKind::Moved,
        column,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(
        msg,
        Some(Message::HoverLink(Some("https://github.com".to_string())))
    );
}

#[test]
fn test_o_key_triggers_open_visible_links_message() {
    let app = App::new(PathBuf::from("test.md"));
    let model = create_test_model();
    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::OpenVisibleLinks));
}

#[test]
fn test_follow_link_jumps_to_internal_anchor() {
    let md = "[Go](#target)\n\n## Target";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 4));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_follow_link_jumps_to_footnote_definition() {
    let md = "Alpha[^1]\n\n[^1]: Footnote text";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 4));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_follow_link_on_image_line_uses_image_src() {
    let md = "![Alt](#target)\n\n## Target";
    let mut heights = std::collections::HashMap::new();
    heights.insert("#target".to_string(), 3);
    let doc = Document::parse_with_layout_and_image_heights(md, 80, &heights).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 6));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    let line = 1; // inside image body
    model = update(model, Message::FollowLinkAtLine(line, None));
    app.handle_message_side_effects(
        &mut model,
        &mut watcher,
        &Message::FollowLinkAtLine(line, None),
    );

    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_follow_link_at_column_picks_correct_link_on_same_line() {
    // Two links on the same rendered line pointing to different anchors.
    // Clicking on the second link's column should jump to #second, not #first.
    let md = "[First](#first) [Second](#second)\n\n\
              filler\n\nfiller\n\nfiller\n\nfiller\n\nfiller\n\n\
              ## First\n\n## Second";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 4));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    let link_line = model.document.links()[0].line;
    // Rendered text is "First Second"; "Second" starts at column 6
    let col = 6;

    let msg = Message::FollowLinkAtLine(link_line, Some(col));
    model = update(model, msg.clone());
    app.handle_message_side_effects(&mut model, &mut watcher, &msg);

    let first_line = model.document.resolve_internal_anchor("first").unwrap();
    let second_line = model.document.resolve_internal_anchor("second").unwrap();
    assert!(
        model.viewport.offset() >= second_line.saturating_sub(1),
        "Should jump near #second (line {}), not #first (line {}), got offset {}",
        second_line,
        first_line,
        model.viewport.offset()
    );
    assert!(
        model.viewport.offset() > first_line,
        "Should have scrolled past #first heading"
    );
}

#[test]
fn test_open_visible_links_shows_picker_when_multiple() {
    let md = "[A](#one)\n\n[B](#two)\n\n## One\n\n## Two";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 8));
    let app = App::new(PathBuf::from("test.md"));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);
    assert_eq!(model.link_picker_items.len(), 2);

    model = update(model, Message::SelectVisibleLink(2));
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::SelectVisibleLink(2));
    assert!(model.link_picker_items.is_empty());
    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_mouse_click_in_link_picker_selects_item() {
    let app = App::new(PathBuf::from("test.md"));
    let md = "[A](#one)\n\n[B](#two)\n\n## One\n\n## Two";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.link_picker_items = model.document.links().iter().take(2).cloned().collect();

    let area = Rect::new(0, 0, 80, 24);
    let popup = crate::ui::link_picker_rect(area, model.link_picker_items.len());
    let content_top = crate::ui::link_picker_content_top(popup);
    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: popup.x + 2,
        row: content_top,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::SelectVisibleLink(1)));
}

#[test]
fn test_link_picker_key_other_than_number_cancels() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.link_picker_items = vec![crate::document::LinkRef {
        text: "Link".to_string(),
        url: "https://example.com".to_string(),
        line: 0,
    }];

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::CancelVisibleLinkPicker));
}

#[test]
fn test_link_picker_click_outside_cancels() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.link_picker_items = vec![crate::document::LinkRef {
        text: "Link".to_string(),
        url: "https://example.com".to_string(),
        line: 0,
    }];

    let mouse = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: 0,
        row: 0,
        modifiers: KeyModifiers::NONE,
    };
    let msg = app.handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::CancelVisibleLinkPicker));
}

#[test]
fn test_resize_debouncer_waits_for_quiet_period() {
    let mut debouncer = ResizeDebouncer::new(100);
    debouncer.queue(120, 40, 0);

    assert!(debouncer.take_ready(50).is_none());
    assert_eq!(debouncer.take_ready(100), Some((120, 40)));
}

#[test]
fn test_browse_debouncer_waits_for_quiet_period() {
    let mut debouncer = BrowseDebouncer::new(150);
    debouncer.queue(3, 0);

    assert!(debouncer.take_ready(100).is_none());
    assert_eq!(debouncer.take_ready(150), Some(3));
}

#[test]
fn test_browse_debouncer_uses_latest_index() {
    let mut debouncer = BrowseDebouncer::new(150);
    debouncer.queue(3, 0);
    debouncer.queue(5, 50);

    assert!(debouncer.take_ready(100).is_none());
    assert_eq!(debouncer.take_ready(200), Some(5));
}

#[test]
fn test_browse_debouncer_cancel_clears_pending() {
    let mut debouncer = BrowseDebouncer::new(150);
    debouncer.queue(3, 0);
    debouncer.cancel();

    assert!(debouncer.take_ready(200).is_none());
    assert!(!debouncer.is_pending());
}

#[test]
fn test_resize_debouncer_uses_latest_size() {
    let mut debouncer = ResizeDebouncer::new(100);
    debouncer.queue(120, 40, 0);
    debouncer.queue(140, 50, 20);

    assert!(debouncer.take_ready(80).is_none());
    assert_eq!(debouncer.take_ready(120), Some((140, 50)));
}

#[test]
#[ignore = "performance test; run with cargo test paging_perf_test_rendering -- --ignored --nocapture"]
fn paging_perf_test_rendering() {
    let md = include_str!("../../examples/test-rendering.md");
    let doc = Document::parse_with_layout(md, 120).unwrap();
    let mut model = Model::new(PathBuf::from("test-rendering.md"), doc, (120, 40));
    model.ensure_highlight_overscan();

    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();

    let mut frame_times = Vec::<Duration>::new();
    let mut pages = 0usize;

    // Warm-up first frame; measure paging renders only.
    terminal
        .draw(|frame| crate::ui::render(&mut model, frame))
        .unwrap();

    while model.viewport.can_scroll_down() {
        model = update(model, Message::PageDown);
        model.ensure_highlight_overscan();

        let t0 = Instant::now();
        terminal
            .draw(|frame| crate::ui::render(&mut model, frame))
            .unwrap();
        frame_times.push(t0.elapsed());

        pages += 1;
        assert!(pages < 10_000, "paging loop runaway");
    }

    assert!(!frame_times.is_empty());
    let total = frame_times.iter().copied().sum::<Duration>();
    let max = frame_times.iter().copied().max().unwrap_or_default();

    let total_ms = total.as_secs_f64() * 1000.0;
    let max_ms = max.as_secs_f64() * 1000.0;

    eprintln!(
        "[perf:paging] frames={} total={:.2}ms max={:.2}ms",
        frame_times.len(),
        total_ms,
        max_ms
    );

    let max_limit_ms = std::env::var("MARKLESS_PERF_MAX_FRAME_MS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50.0);
    let total_limit_ms = std::env::var("MARKLESS_PERF_TOTAL_MS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1500.0);

    assert!(
        max_ms <= max_limit_ms,
        "max frame time {:.2}ms exceeded {:.2}ms",
        max_ms,
        max_limit_ms
    );
    assert!(
        total_ms <= total_limit_ms,
        "total frame time {:.2}ms exceeded {:.2}ms",
        total_ms,
        total_limit_ms
    );
}

// ---- Browse Mode Tests ----

#[test]
fn test_browse_mode_default_is_false() {
    let model = create_test_model();
    assert!(!model.browse_mode);
    assert!(model.browse_entries.is_empty());
}

#[test]
fn test_load_directory_populates_entries() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("subdir");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(dir.path().join("beta.md"), "# Beta").unwrap();
    std::fs::write(dir.path().join("alpha.md"), "# Alpha").unwrap();
    std::fs::write(dir.path().join(".hidden"), "secret").unwrap();

    let mut model = create_test_model();
    model.load_directory(dir.path()).unwrap();

    // canonicalize to handle /private symlink on macOS
    let expected_dir = dir.path().canonicalize().unwrap();
    assert_eq!(model.browse_dir, expected_dir);
    // Should have: "..", "subdir/", "alpha.md", "beta.md" (sorted, hidden skipped)
    assert!(model.browse_entries.len() >= 3);
    assert_eq!(model.browse_entries[0].name, "..");
    assert!(model.browse_entries[0].is_dir);
    // Dirs before files
    let first_file_idx = model.browse_entries.iter().position(|e| !e.is_dir).unwrap();
    assert!(first_file_idx > 1); // ".." + subdir before files
    // Files are sorted
    let file_names: Vec<&str> = model
        .browse_entries
        .iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(file_names, vec!["alpha.md", "beta.md"]);
}

#[test]
fn test_load_directory_skips_hidden_files() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join(".hidden"), "secret").unwrap();
    std::fs::write(dir.path().join("visible.md"), "# Hi").unwrap();

    let mut model = create_test_model();
    model.load_directory(dir.path()).unwrap();

    let names: Vec<&str> = model
        .browse_entries
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert!(!names.contains(&".hidden"));
    assert!(names.contains(&"visible.md"));
}

#[test]
fn test_toc_entry_count_uses_headings_in_file_mode() {
    let model = create_test_model();
    assert!(!model.browse_mode);
    assert_eq!(model.toc_entry_count(), model.document.headings().len());
}

#[test]
fn test_toc_entry_count_uses_browse_entries_in_browse_mode() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# A").unwrap();
    std::fs::write(dir.path().join("b.md"), "# B").unwrap();

    let mut model = create_test_model();
    model.browse_mode = true;
    model.load_directory(dir.path()).unwrap();

    assert_eq!(model.toc_entry_count(), model.browse_entries.len());
}

#[test]
fn test_load_file_updates_document() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("doc.md");
    std::fs::write(&file_path, "# Hello\n\nWorld").unwrap();

    let mut model = create_test_model();
    model.load_file(&file_path).unwrap();

    assert!(model.document.source().contains("# Hello"));
    assert_eq!(model.file_path, file_path);
}

#[test]
fn test_enter_browse_mode_message() {
    let mut model = create_test_model();
    model.browse_mode = false;
    let model = update(model, Message::EnterBrowseMode);
    assert!(model.browse_mode);
    assert!(model.toc_visible);
}

#[test]
fn test_enter_file_mode_message() {
    let mut model = create_test_model();
    model.browse_mode = true;
    let model = update(model, Message::EnterFileMode);
    assert!(!model.browse_mode);
}

#[test]
fn test_enter_file_mode_syncs_toc_to_headings() {
    let mut model = create_many_headings_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.toc_focused = true; // typical for browse mode
    // Simulate browse mode with toc_selected pointing at a browse entry index
    // that would be out of range for the headings list
    model.toc_selected = Some(50);
    model.toc_scroll_offset = 40;

    let model = update(model, Message::EnterFileMode);

    assert!(!model.browse_mode);
    // toc_selected should now be valid for headings (synced to viewport position)
    if let Some(sel) = model.toc_selected {
        assert!(
            sel < model.document.headings().len(),
            "toc_selected {} should be < headings count {}",
            sel,
            model.document.headings().len()
        );
    }
    assert!(
        model.toc_scroll_offset <= model.max_toc_scroll_offset(),
        "toc_scroll_offset {} should be <= max {}",
        model.toc_scroll_offset,
        model.max_toc_scroll_offset()
    );
}

#[test]
fn test_toc_down_in_browse_mode_uses_browse_entries_len() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# A").unwrap();
    std::fs::write(dir.path().join("b.md"), "# B").unwrap();

    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.load_directory(dir.path()).unwrap();
    model.toc_selected = Some(0);

    let model = update(model, Message::TocDown);
    assert_eq!(model.toc_selected, Some(1));
}

#[test]
fn test_sync_toc_skipped_in_browse_mode() {
    let mut model = create_many_headings_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.toc_focused = false;
    model.toc_selected = Some(0);

    // Scrolling should NOT auto-sync TOC selection in browse mode
    let model = update(model, Message::ScrollDown(5));
    assert_eq!(model.toc_selected, Some(0));
}

#[test]
fn test_b_key_sends_enter_browse_mode() {
    let app = App::new(PathBuf::from("test.md"));
    let model = create_test_model();
    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::EnterBrowseMode));
}

#[test]
fn test_f_key_sends_enter_file_mode() {
    let app = App::new(PathBuf::from("test.md"));
    let model = create_test_model();
    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Char('F'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::EnterFileMode));
}

#[test]
fn test_load_file_handles_image_file() {
    let dir = tempdir().unwrap();
    let img_path = dir.path().join("photo.png");
    // Write a minimal valid PNG (1x1 pixel)
    let png_bytes: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
        0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1x1
        0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, 0xde, // color type, etc
        0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, // IDAT chunk
        0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, 0x00, // compressed data
        0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, 0x33, // CRC
        0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, // IEND chunk
        0xae, 0x42, 0x60, 0x82, // IEND CRC
    ];
    std::fs::write(&img_path, png_bytes).unwrap();

    let mut model = create_test_model();
    model.load_file(&img_path).unwrap();

    // Document should contain the image reference
    assert!(
        model.document.source().contains("![photo.png]"),
        "Image file should be wrapped as markdown image ref"
    );
    assert!(
        !model.document.images().is_empty(),
        "Should have an ImageRef"
    );
    assert_eq!(model.file_path, img_path);
}

#[test]
fn test_browse_auto_load_selects_file_in_listing() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("alpha.md"), "# Alpha").unwrap();
    std::fs::write(dir.path().join("beta.md"), "# Beta").unwrap();

    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.load_directory(dir.path()).unwrap();

    // Simulate auto-loading first file (what browse_auto_load_first_file does)
    let first_file = model
        .browse_entries
        .iter()
        .find(|e| !e.is_dir)
        .map(|e| e.path.clone())
        .unwrap();
    model.load_file(&first_file).unwrap();

    // The loaded file should be selectable by filename comparison
    let loaded_name = model.file_path.file_name().unwrap().to_string_lossy();
    let idx = model
        .browse_entries
        .iter()
        .position(|e| e.name == loaded_name);
    assert!(
        idx.is_some(),
        "Should find loaded file '{}' in browse entries by name",
        loaded_name
    );
}

#[test]
fn test_browse_auto_load_prefers_markdown() {
    let dir = tempdir().unwrap();
    std::fs::write(dir.path().join("aaa.txt"), "text file").unwrap();
    std::fs::write(dir.path().join("readme.md"), "# Readme").unwrap();
    std::fs::write(dir.path().join("zzz.rs"), "fn main() {}").unwrap();

    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.load_directory(dir.path()).unwrap();

    // Find the first preferred file (should prefer .md over .txt alphabetically earlier)
    let preferred = model.first_viewable_file_index();
    assert!(preferred.is_some(), "Should find a viewable file");
    let (idx, _) = preferred.unwrap();
    assert_eq!(
        model.browse_entries[idx].name, "readme.md",
        "Should prefer markdown file over alphabetically-earlier txt"
    );
}

#[test]
fn test_browse_toc_backspace_sends_toc_collapse() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.toc_focused = true;
    model.toc_selected = Some(0);

    let msg = app.handle_key(
        event::KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::TocCollapse));
}

#[test]
fn test_browse_navigate_parent_at_root_is_noop() {
    let app = App::new(PathBuf::from("test.md"));
    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.browse_dir = PathBuf::from("/");
    model.browse_entries = vec![super::model::DirEntry {
        name: "..".to_string(),
        path: PathBuf::from("/"),
        is_dir: true,
    }];
    model.toc_selected = Some(0);

    // TocCollapse in browse mode triggers browse_navigate_parent
    let mut watcher: Option<crate::watcher::FileWatcher> = None;
    app.handle_message_side_effects(&mut model, &mut watcher, &Message::TocCollapse);

    // Should stay at root without error
    assert_eq!(model.browse_dir, PathBuf::from("/"));
    assert!(
        model.active_toast().is_none(),
        "Should not show error toast"
    );
}
