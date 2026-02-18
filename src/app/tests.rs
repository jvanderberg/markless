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

/// Enter edit mode: runs pure update then side effects (which reads
/// the file from disk to populate the buffer).
fn enter_edit_mode(mut model: Model) -> Model {
    model = update(model, Message::EnterEditMode);
    let mut watcher = None;
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model
}

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
    let mut watcher = None;

    std::fs::write(&file_path, "# Updated\n\nbeta").unwrap();
    model = update(model, Message::ForceReload);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::ForceReload);

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
    let mut watcher = None;

    model = update(model, Message::StartSelection(2));
    model = update(model, Message::UpdateSelection(4));
    model = update(model, Message::EndSelection(4));
    assert!(model.selection.is_some());

    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EndSelection(4));

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

    let key = event::KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE);
    let msg = App::handle_key(key, &model);
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
fn test_ctrl_q_quits_in_normal_mode() {
    let model = create_test_model();
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        &model,
    );
    assert_eq!(msg, Some(Message::Quit));
}

#[test]
fn test_ctrl_q_quits_in_editor_mode() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        &model,
    );
    assert_eq!(msg, Some(Message::Quit));
}

#[test]
fn test_ctrl_q_quits_in_search_mode() {
    let model = create_test_model();
    let model = update(model, Message::StartSearch);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL),
        &model,
    );
    assert_eq!(msg, Some(Message::Quit));
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
    let mut model = create_test_model();
    model = update(model, Message::StartSearch);
    model = update(model, Message::SearchInput("a".to_string()));

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::SearchInput("an".to_string())));
}

#[test]
fn test_search_mode_enter_moves_to_next_match() {
    let mut model = create_test_model();
    model = update(model, Message::StartSearch);
    model = update(model, Message::SearchInput("test".to_string()));

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::NextMatch));
}

#[test]
fn test_toc_focus_space_pages_document() {
    let mut model = create_long_test_model();
    model.toc_visible = true;
    model.toc_focused = true;
    model.toc_selected = Some(0);

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &model,
    );
    // Space always pages the document body, even when TOC is focused
    assert_eq!(msg, Some(Message::PageDown));
}

#[test]
fn test_question_mark_opens_help_when_not_searching() {
    let model = create_test_model();

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('?'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::ToggleHelp));
}

#[test]
fn test_help_mode_esc_closes_help() {
    let mut model = create_test_model();
    model.help_visible = true;

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HideHelp));
}

#[test]
fn test_help_mode_unrecognized_key_ignored() {
    let mut model = create_test_model();
    model.help_visible = true;

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, None);
}

#[test]
fn test_mouse_click_on_doc_link_emits_follow_message() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(0, Some(0))));
}

#[test]
fn test_mouse_click_on_image_emits_follow_message() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(0, Some(0))));
}

#[test]
fn test_mouse_click_on_image_body_emits_follow_message() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(1, Some(0))));
}

#[test]
fn test_mouse_click_on_image_body_after_press_emits_follow_message() {
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
    let _ = App::handle_mouse(down, &model);
    let model = update(model, Message::StartSelection(1));
    let msg = App::handle_mouse(up, &model);
    assert_eq!(msg, Some(Message::FollowLinkAtLine(1, Some(0))));
}

#[test]
fn test_mouse_hover_on_doc_link_emits_hover_message() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(
        msg,
        Some(Message::HoverLink(Some("https://example.com".to_string())))
    );
}

#[test]
fn test_mouse_hover_on_image_body_emits_hover_message() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::HoverLink(Some("image.png".to_string()))));
}

#[test]
fn test_hover_prefers_link_at_column_when_multiple() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(
        msg,
        Some(Message::HoverLink(Some("https://github.com".to_string())))
    );
}

#[test]
fn test_o_key_triggers_open_visible_links_message() {
    let model = create_test_model();
    let msg = App::handle_key(
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
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_follow_local_markdown_link_loads_target_file() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let target_path = dir.path().join("next.md");
    let current_md = "[Next](next.md)";
    std::fs::write(&current_path, current_md).unwrap();
    std::fs::write(&target_path, "# Next\n\nLoaded").unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 8));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, target_path);
    assert!(model.document.source().contains("# Next"));
}

#[test]
fn test_follow_local_markdown_link_with_code_styled_label_loads_target_file() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let fixes_dir = dir.path().join("fixes");
    std::fs::create_dir_all(&fixes_dir).unwrap();
    let target_path = fixes_dir.join("README.md");
    let current_md = "See [`fixes/README.md`](fixes/README.md) for details.";
    std::fs::write(&current_path, current_md).unwrap();
    std::fs::write(&target_path, "# Fixes\n\nDetails").unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 8));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, target_path);
    assert!(model.document.source().contains("# Fixes"));
}

#[test]
fn test_follow_local_non_markdown_link_loads_readable_file() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let target_path = dir.path().join("notes.txt");
    let current_md = "[Notes](notes.txt)";
    let target_text = "alpha\nbeta";
    std::fs::write(&current_path, current_md).unwrap();
    std::fs::write(&target_path, target_text).unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 8));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, target_path);
    assert!(model.document.source().contains("alpha"));
    assert!(model.document.source().contains("beta"));
    let rendered: Vec<String> = (0..model.document.line_count())
        .filter_map(|idx| {
            model
                .document
                .line_at(idx)
                .map(|line| line.content().to_string())
        })
        .collect();
    assert!(rendered.iter().any(|line| line.contains("alpha")));
    assert!(rendered.iter().any(|line| line.contains("beta")));
}

#[test]
fn test_follow_file_url_with_anchor_loads_file_and_jumps() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let target_path = dir.path().join("target.md");
    std::fs::write(&target_path, "# Intro\n\n## Jump Here\n\nBody").unwrap();
    let link = format!(
        "[Jump](<file://{}#jump-here>)",
        target_path.to_string_lossy()
    );
    std::fs::write(&current_path, &link).unwrap();

    let doc = Document::parse_with_layout(&link, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 6));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    let target_line = model.document.resolve_internal_anchor("jump-here").unwrap();
    assert_eq!(model.file_path, target_path);
    assert!(model.viewport.offset() >= target_line.saturating_sub(1));
}

#[test]
fn test_follow_missing_local_link_keeps_current_file() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let current_md = "[Missing](does-not-exist.md)";
    std::fs::write(&current_path, current_md).unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path.clone(), doc, (80, 8));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, current_path);
    assert!(
        model
            .active_toast()
            .is_some_and(|(msg, level)| level == ToastLevel::Error && msg.contains("Open failed"))
    );
}

#[test]
fn test_follow_local_link_updates_browse_selection_same_directory() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let target_path = dir.path().join("next.md");
    let current_md = "[Next](next.md)";
    std::fs::write(&current_path, current_md).unwrap();
    std::fs::write(&target_path, "# Next").unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 8));
    model.browse_mode = true;
    model.toc_visible = true;
    model.load_directory(dir.path()).unwrap();
    model.toc_selected = model
        .browse_entries
        .iter()
        .position(|e| e.name == "current.md");
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, target_path);
    let selected = model.toc_selected.expect("selection should be set");
    assert_eq!(model.browse_entries[selected].name, "next.md");
}

#[test]
fn test_follow_local_link_updates_browse_directory_for_subdir_target() {
    let dir = tempdir().unwrap();
    let current_path = dir.path().join("current.md");
    let subdir = dir.path().join("fixes");
    let target_path = subdir.join("README.md");
    std::fs::create_dir_all(&subdir).unwrap();
    let current_md = "[Fixes](fixes/README.md)";
    std::fs::write(&current_path, current_md).unwrap();
    std::fs::write(&target_path, "# Fixes").unwrap();

    let doc = Document::parse_with_layout(current_md, 80).unwrap();
    let mut model = Model::new(current_path, doc, (80, 8));
    model.browse_mode = true;
    model.toc_visible = true;
    model.load_directory(dir.path()).unwrap();
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert_eq!(model.file_path, target_path);
    assert_eq!(model.browse_dir, subdir.canonicalize().unwrap());
    let selected = model.toc_selected.expect("selection should be set");
    assert_eq!(model.browse_entries[selected].name, "README.md");
}

#[test]
fn test_follow_link_jumps_to_footnote_definition() {
    let md = "Alpha[^1]\n\n[^1]: Footnote text";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 4));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);

    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_follow_link_on_image_line_uses_image_src() {
    let md = "![Alt](#target)\n\n## Target";
    let mut heights = std::collections::HashMap::new();
    heights.insert("#target".to_string(), 3);
    let doc = Document::parse_with_layout_and_image_heights(md, 80, &heights).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 6));
    let mut watcher = None;

    let line = 1; // inside image body
    model = update(model, Message::FollowLinkAtLine(line, None));
    App::handle_message_side_effects(
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
    let mut watcher = None;

    let link_line = model.document.links()[0].line;
    // Rendered text is "First Second"; "Second" starts at column 6
    let col = 6;

    let msg = Message::FollowLinkAtLine(link_line, Some(col));
    model = update(model, msg.clone());
    App::handle_message_side_effects(&mut model, &mut watcher, &msg);

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
fn test_wrapped_link_is_clickable_on_both_lines() {
    // A link whose text wraps to a second line should be clickable on both lines.
    let md = "Go [click here for more details](https://example.com) now.";
    let doc = Document::parse_with_layout(md, 25).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (25, 10));

    let links: Vec<_> = model
        .document
        .links()
        .iter()
        .filter(|l| l.url == "https://example.com")
        .collect();

    // Should have link refs on two different lines
    assert!(
        links.len() >= 2,
        "expected link refs on 2+ lines, got {}: {:?}",
        links.len(),
        links
    );

    // Both lines should be clickable at the column where the link text appears
    for link in &links {
        let line_content = model.document.line_at(link.line).unwrap().content();
        let byte_pos = line_content.find(&link.text).unwrap();
        let col = unicode_width::UnicodeWidthStr::width(&line_content[..byte_pos]);
        let found = App::link_at_column(&model, link.line, col);
        assert!(
            found.is_some(),
            "link should be clickable at line {} col {} (text {:?})",
            link.line,
            col,
            link.text
        );
        assert_eq!(found.unwrap().url, "https://example.com");
    }
}

#[test]
fn test_open_visible_links_shows_picker_when_multiple() {
    let md = "[A](#one)\n\n[B](#two)\n\n## One\n\n## Two";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 8));
    let mut watcher = None;

    model = update(model, Message::OpenVisibleLinks);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::OpenVisibleLinks);
    assert_eq!(model.link_picker_items.len(), 2);

    model = update(model, Message::SelectVisibleLink(2));
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::SelectVisibleLink(2));
    assert!(model.link_picker_items.is_empty());
    assert!(model.viewport.offset() > 0);
}

#[test]
fn test_mouse_click_in_link_picker_selects_item() {
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
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::SelectVisibleLink(1)));
}

#[test]
fn test_link_picker_key_other_than_number_cancels() {
    let mut model = create_test_model();
    model.link_picker_items = vec![crate::document::LinkRef {
        text: "Link".to_string(),
        url: "https://example.com".to_string(),
        line: 0,
    }];

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::CancelVisibleLinkPicker));
}

#[test]
fn test_link_picker_click_outside_cancels() {
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
    let msg = App::handle_mouse(mouse, &model);
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
fn test_toc_auto_sync_works_when_toc_focused() {
    let mut model = create_many_headings_model();
    model = update(model, Message::ToggleToc);
    model.toc_focused = true;

    let target_idx = 8;
    let target_line = model.document.headings()[target_idx].line;
    // Scrolling to a heading should still sync TOC selection even when TOC is focused
    model = update(model, Message::GoToLine(target_line));

    assert_eq!(
        model.toc_selected,
        Some(target_idx),
        "TOC should sync to viewport even when toc_focused is true"
    );
}

#[test]
fn test_b_key_sends_enter_browse_mode() {
    let model = create_test_model();
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('B'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::EnterBrowseMode));
}

#[test]
fn test_f_key_sends_enter_file_mode() {
    let model = create_test_model();
    let msg = App::handle_key(
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
    let mut model = create_test_model();
    model.browse_mode = true;
    model.toc_visible = true;
    model.toc_focused = true;
    model.toc_selected = Some(0);

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::TocCollapse));
}

#[test]
fn test_browse_navigate_parent_at_root_is_noop() {
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
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::TocCollapse);

    // Should stay at root without error
    assert_eq!(model.browse_dir, PathBuf::from("/"));
    assert!(
        model.active_toast().is_none(),
        "Should not show error toast"
    );
}

#[test]
fn test_mermaid_as_images_false_without_picker() {
    let model = create_test_model();
    // No picker set — should not render mermaid as images
    assert!(!model.should_render_mermaid_as_images());
}

#[test]
fn test_mermaid_as_images_false_with_halfblock_picker() {
    let picker = ratatui_image::picker::Picker::halfblocks();
    let model = create_test_model().with_picker(Some(picker));
    // Halfblock picker — should NOT render mermaid as images
    assert!(!model.should_render_mermaid_as_images());
}

#[test]
fn test_mermaid_as_images_false_when_images_disabled() {
    let picker = ratatui_image::picker::Picker::halfblocks();
    let mut model = create_test_model().with_picker(Some(picker));
    model.images_enabled = false;
    assert!(!model.should_render_mermaid_as_images());
}

#[test]
fn test_click_on_footnote_superscript_finds_link() {
    let md = "See this[^1] thing.\n\n[^1]: The footnote.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));

    // Find the footnote link
    let footnote_link = model
        .document
        .links()
        .iter()
        .find(|l| l.url.starts_with("footnote:"))
        .expect("should have a footnote link");
    let link_line = footnote_link.line;

    // The rendered line should contain the superscript "¹"
    let line_text = model.document.line_at(link_line).unwrap().content();
    assert!(
        line_text.contains('¹'),
        "rendered line should contain superscript: {line_text:?}"
    );

    // Find the column of the superscript character
    let col = line_text.find('¹').unwrap();
    let col_chars = line_text[..col].chars().count();

    // link_at_column should find the footnote link via exact column hit
    let found = App::link_at_column(&model, link_line, col_chars);
    assert!(
        found.is_some(),
        "link_at_column should find footnote link at column {col_chars} in line {line_text:?}"
    );
    let found = found.unwrap();
    assert!(found.url.starts_with("footnote:"));
    // Verify the link text matches what's actually in the rendered line
    assert_eq!(found.text, "¹");
    assert!(
        line_text.contains(&found.text),
        "link text {:?} must appear in rendered line {:?}",
        found.text,
        line_text
    );
}

#[test]
fn test_mouse_click_on_footnote_superscript_emits_follow() {
    let md = "See this[^1] thing.\n\n[^1]: The footnote.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.toc_visible = true;

    // Simulate mouse Down (starts selection) then Up (triggers link follow)
    let chunks = crate::ui::split_main_columns(Rect::new(0, 0, 80, 24));
    let doc_x = chunks[1].x;

    // Find where the superscript is in the rendered line
    let footnote_link = model
        .document
        .links()
        .iter()
        .find(|l| l.url.starts_with("footnote:"))
        .unwrap();
    let link_line = footnote_link.line;
    let line_text = model.document.line_at(link_line).unwrap().content();
    let col_byte = line_text.find('¹').expect("superscript not found");
    let col_chars = line_text[..col_byte].chars().count();

    // Mouse Down
    let down = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING + col_chars as u16,
        row: link_line as u16,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(down, &model);
    assert_eq!(msg, Some(Message::StartSelection(link_line)));
    model = update(model, msg.unwrap());

    // Mouse Up at same position
    let up = MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        column: doc_x + crate::ui::DOCUMENT_LEFT_PADDING + col_chars as u16,
        row: link_line as u16,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(up, &model);
    assert_eq!(
        msg,
        Some(Message::FollowLinkAtLine(link_line, Some(col_chars))),
        "clicking on footnote superscript should emit FollowLinkAtLine"
    );
}

#[test]
fn test_footnote_link_ref_line_matches_rendered_line() {
    // Realistic document with heading, paragraph, footnote ref, and definition
    let md = "# Title\n\nSome text with a reference[^1] in it.\n\n[^1]: The footnote definition.";
    let doc = Document::parse_with_layout(md, 80).unwrap();

    let footnote_links: Vec<_> = doc
        .links()
        .iter()
        .filter(|l| l.url.starts_with("footnote:"))
        .collect();
    assert!(!footnote_links.is_empty(), "should have footnote link");

    for link in &footnote_links {
        let line_text = doc
            .line_at(link.line)
            .unwrap_or_else(|| panic!("no line at {}", link.line))
            .content();

        // The link text must actually appear in the rendered line
        assert!(
            line_text.contains(&link.text),
            "link text {:?} not found in line {} content {:?}",
            link.text,
            link.line,
            line_text
        );
    }
}

#[test]
fn test_footnote_link_at_column_with_example_file() {
    // Use the same content as the example footnotes file
    let md = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/09-footnotes.md"),
    )
    .unwrap();
    // Use a narrow width to trigger wrapping, more realistic
    let doc = Document::parse_with_layout(&md, 40).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (40, 40));

    // Dump all links for debugging
    let all_links = model.document.links();
    let footnote_links: Vec<_> = all_links
        .iter()
        .filter(|l| l.url.starts_with("footnote:"))
        .collect();

    assert!(
        !footnote_links.is_empty(),
        "should have footnote links, all links: {:?}",
        all_links
    );

    // For each footnote link, verify link_at_column can find it
    for link in &footnote_links {
        let line_text = model
            .document
            .line_at(link.line)
            .unwrap_or_else(|| panic!("no line at {}", link.line))
            .content();

        // The link text must appear in the line
        let found_in_line = line_text.contains(&link.text);
        assert!(
            found_in_line,
            "link {:?} (text={:?}) not found in line {} content {:?}. \
             All lines: {:?}",
            link.url,
            link.text,
            link.line,
            line_text,
            (0..model.document.line_count())
                .map(|i| (i, model.document.line_at(i).unwrap().content().to_string()))
                .collect::<Vec<_>>()
        );

        // Find the column position and check link_at_column
        let col_byte = line_text.find(&link.text).unwrap();
        let col_width = unicode_width::UnicodeWidthStr::width(&line_text[..col_byte]);

        let found = App::link_at_column(&model, link.line, col_width);
        assert!(
            found.is_some(),
            "link_at_column failed for {:?} at line {} col {}",
            link.url,
            link.line,
            col_width
        );
    }
}

// Issue 4: clicking far from a link should not activate it
#[test]
fn test_link_at_column_returns_none_far_from_link() {
    // A line with a short link at the start, click at the far right
    let md = "[Go](https://go.dev) and then some very long filler text stretching out.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let link = model
        .document
        .links()
        .iter()
        .find(|l| l.url == "https://go.dev")
        .unwrap()
        .clone();
    // Click at column 60, far from "Go" which is at column 0-1
    let found = App::link_at_column(&model, link.line, 60);
    assert!(
        found.is_none(),
        "clicking 60 columns away from a 2-char link should return None"
    );
}

#[test]
fn test_wrap_width_caps_layout_width() {
    let md = "This is a very long paragraph that should wrap at the configured wrap width rather than the full terminal width when wrap_width is set.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (120, 24));
    model.wrap_width = Some(60);
    model.reflow_layout();

    let paragraph_lines: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();

    assert!(
        paragraph_lines.len() > 1,
        "expected text to wrap with wrap_width=60"
    );
    for len in &paragraph_lines {
        assert!(*len <= 60, "line exceeds wrap_width: {} > 60", len);
    }
}

#[test]
fn test_wrap_width_none_uses_terminal_width() {
    let md = "Short text.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = None;

    // layout_width should equal document_content_width for 80-col terminal
    let expected = crate::ui::document_content_width(80, false);
    assert_eq!(model.layout_width(), expected);
}

#[test]
fn test_wrap_width_larger_than_terminal_uses_terminal() {
    let md = "Short text.";
    let doc = Document::parse_with_layout(md, 80).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = Some(200); // larger than terminal

    // Should still cap to terminal width
    let terminal_width = crate::ui::document_content_width(80, false);
    assert_eq!(model.layout_width(), terminal_width);
}

#[test]
fn test_wrap_width_preserved_after_resize() {
    let md = "This is a long paragraph that should always wrap at 40 columns regardless of how wide the terminal gets after a resize event.";
    let doc = Document::parse_with_layout(md, 40).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = Some(40);
    model.reflow_layout();

    // Verify initial wrapping at 40
    let lines_before: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();
    assert!(lines_before.len() > 1, "should wrap at 40");
    for len in &lines_before {
        assert!(
            *len <= 40,
            "line exceeds wrap_width before resize: {len} > 40"
        );
    }

    // Simulate resize to much wider terminal
    model = update(model, Message::Resize(200, 50));

    // wrap_width must still be 40 and lines must still respect it
    assert_eq!(model.wrap_width, Some(40), "wrap_width lost after resize");
    let lines_after: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();
    assert!(lines_after.len() > 1, "should still wrap after resize");
    for len in &lines_after {
        assert!(
            *len <= 40,
            "line exceeds wrap_width after resize: {len} > 40"
        );
    }
}

#[test]
fn test_wrap_width_stable_through_multiple_resizes() {
    let md = "This is a fairly long paragraph of text that should consistently wrap at exactly 40 columns no matter how many times the terminal is resized to various widths both smaller and larger.";
    let doc = Document::parse_with_layout(md, 40).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = Some(40);
    model.reflow_layout();

    // Resize through a series of widths: narrow, wide, narrow, wider
    for &(width, height) in &[(60, 24), (200, 50), (45, 24), (300, 60)] {
        model = update(model, Message::Resize(width, height));
        assert_eq!(
            model.wrap_width,
            Some(40),
            "wrap_width lost at {width}x{height}"
        );
        let lines: Vec<_> = model
            .document
            .visible_lines(0, 50)
            .iter()
            .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
            .map(|l| l.content().chars().count())
            .collect();
        assert!(lines.len() > 1, "should wrap at {width}x{height}");
        for len in &lines {
            assert!(
                *len <= 40,
                "line exceeds wrap_width at {width}x{height}: {len} > 40"
            );
        }
    }
}

#[test]
fn test_wrap_width_with_toc_visible_after_resize() {
    let md = "# Heading\n\nThis is a long paragraph that should always wrap at 40 columns even when the table of contents is visible and the terminal is resized.";
    let doc = Document::parse_with_layout(md, 40).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = Some(40);
    model.toc_visible = true;
    model.reflow_layout();

    // Resize wider
    model = update(model, Message::Resize(200, 50));

    assert_eq!(model.wrap_width, Some(40));
    let lines: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();
    for len in &lines {
        assert!(
            *len <= 40,
            "line exceeds wrap_width with TOC visible: {len} > 40"
        );
    }
}

#[test]
fn test_wrap_width_with_toc_toggle_after_resize() {
    let md = "# Heading\n\nThis is a long paragraph that should always wrap at 40 columns even when the table of contents is toggled on and off during resizes.";
    let doc = Document::parse_with_layout(md, 40).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (120, 24));
    model.wrap_width = Some(40);
    model.reflow_layout();

    // Resize wide, then toggle TOC on
    model = update(model, Message::Resize(200, 50));
    model = update(model, Message::ToggleToc);

    assert_eq!(model.wrap_width, Some(40));
    let lines: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();
    for len in &lines {
        assert!(
            *len <= 40,
            "line exceeds wrap_width after TOC toggle: {len} > 40"
        );
    }
}

/// The initial document load must respect wrap_width immediately, not only
/// after the first resize. This mimics the event_loop init path where the
/// document is parsed at terminal width and wrap_width is set afterward.
#[test]
fn test_wrap_width_applied_at_init_before_any_resize() {
    let md = "This paragraph is long enough to demonstrate that the initial document load must apply the wrap width immediately rather than waiting for the first resize event to trigger a reflow.";

    // Mimic event_loop: parse at terminal content width (ignoring wrap_width)
    let terminal_content_w = crate::ui::document_content_width(160, true);
    let doc = Document::parse_with_layout(md, terminal_content_w).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (160, 40));
    model.toc_visible = true;
    model.wrap_width = Some(80);

    // Immediately after init (no resize yet), text should respect wrap_width
    model.reflow_layout();

    let lines: Vec<_> = model
        .document
        .visible_lines(0, 50)
        .iter()
        .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
        .map(|l| l.content().chars().count())
        .collect();
    assert!(
        lines.len() > 1,
        "should wrap at 80, not {terminal_content_w}"
    );
    for len in &lines {
        assert!(*len <= 80, "line exceeds wrap_width on init: {len} > 80");
    }
}

/// Verify that layout_width stays exactly at wrap_width across all sizes
/// and that the longest paragraph line matches it, not something smaller.
#[test]
fn test_wrap_width_is_exact_target_not_smaller() {
    let md = "This is a sentence that is definitely much longer than forty characters so it must wrap at least once when the wrap width is set to forty columns.";
    let doc = Document::parse_with_layout(md, 40).unwrap();
    let mut model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    model.wrap_width = Some(40);
    model.toc_visible = true;
    model.browse_mode = true;

    // Check at several terminal widths: the wrap target should always be 40
    for &terminal_width in &[80u16, 120, 200, 300] {
        model = update(model, Message::Resize(terminal_width, 40));
        let lw = model.layout_width();
        assert_eq!(
            lw, 40,
            "layout_width should be 40 at terminal {terminal_width}, got {lw}"
        );

        let max_para_len = model
            .document
            .visible_lines(0, 50)
            .iter()
            .filter(|l| *l.line_type() == crate::document::LineType::Paragraph)
            .map(|l| l.content().chars().count())
            .max()
            .unwrap_or(0);

        // The longest line should be close to 40, not way below
        assert!(
            max_para_len > 30,
            "longest paragraph line is only {max_para_len} chars at terminal \
             {terminal_width} — text is wrapping too narrow (expected ~40)"
        );
        assert!(
            max_para_len <= 40,
            "longest paragraph line is {max_para_len} chars at terminal \
             {terminal_width} — exceeds wrap_width 40"
        );
    }
}

// --- Editor mode tests ---

#[test]
fn test_enter_edit_mode_populates_buffer() {
    let model = create_test_model();
    assert!(!model.editor_mode);
    assert!(model.editor_buffer.is_none());

    let model = enter_edit_mode(model);
    assert!(model.editor_mode);
    assert!(model.editor_buffer.is_some());
}

#[test]
fn test_enter_edit_mode_loads_source() {
    let model = create_test_model();
    let source = model.document.source().to_string();

    let model = enter_edit_mode(model);
    let buf = model.editor_buffer.as_ref().unwrap();
    assert_eq!(buf.text(), source);
}

#[test]
fn test_exit_edit_mode_clears_buffer() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    assert!(model.editor_mode);

    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);
    assert!(model.editor_buffer.is_none());
}

#[test]
fn test_exit_edit_mode_after_save_keeps_changes() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Test\n\nHello world").unwrap();

    let doc = Document::parse("# Test\n\nHello world").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Save via side effect (marks buffer clean)
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);
    assert!(!model.editor_buffer.as_ref().unwrap().is_dirty());

    // Now Esc exits immediately (buffer is clean) and re-parses
    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);
    assert!(model.document.source().contains('X'));
}

#[test]
fn test_exit_edit_mode_discard_reverts_document() {
    let model = create_test_model();
    let original_source = model.document.source().to_string();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    // Discard: Esc twice
    let model = update(model, Message::ExitEditMode);
    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);
    // Document should have original source, NOT the edited version
    assert_eq!(model.document.source(), original_source);
}

#[test]
fn test_editor_insert_char() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('Z'));

    let buf = model.editor_buffer.as_ref().unwrap();
    assert!(buf.is_dirty());
    assert!(buf.text().starts_with('Z'));
}

#[test]
fn test_editor_delete_back() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('A'));
    let model = update(model, Message::EditorInsertChar('B'));
    let model = update(model, Message::EditorDeleteBack);

    let buf = model.editor_buffer.as_ref().unwrap();
    assert!(buf.text().starts_with('A'));
    assert!(!buf.text().starts_with("AB"));
}

#[test]
fn test_editor_split_line() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    let line_count_before = model.editor_buffer.as_ref().unwrap().line_count();

    let model = update(model, Message::EditorSplitLine);

    let buf = model.editor_buffer.as_ref().unwrap();
    assert_eq!(buf.line_count(), line_count_before + 1);
}

#[test]
fn test_editor_cursor_movement() {
    use crate::editor::Direction;

    let model = create_test_model();
    let model = enter_edit_mode(model);

    // Move right, then check cursor moved
    let model = update(model, Message::EditorMoveCursor(Direction::Right));
    let cursor = model.editor_buffer.as_ref().unwrap().cursor();
    assert_eq!(cursor.col, 1);
}

#[test]
fn test_editor_move_home_end() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    let model = update(model, Message::EditorMoveEnd);
    let end_col = model.editor_buffer.as_ref().unwrap().cursor().col;
    assert!(end_col > 0);

    let model = update(model, Message::EditorMoveHome);
    let col = model.editor_buffer.as_ref().unwrap().cursor().col;
    assert_eq!(col, 0);
}

#[test]
fn test_editor_scroll_keeps_cursor_visible() {
    let mut md = String::new();
    for i in 1..=50 {
        md.push_str(&format!("Line {i}\n"));
    }
    let doc = Document::parse(&md).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 10));
    let model = enter_edit_mode(model);
    assert_eq!(model.editor_scroll_offset, 0);

    // Move cursor down many lines
    let mut model = model;
    for _ in 0..20 {
        model = update(
            model,
            Message::EditorMoveCursor(crate::editor::Direction::Down),
        );
    }
    // Cursor should be visible — scroll offset should have adjusted
    let cursor_line = model.editor_buffer.as_ref().unwrap().cursor().line;
    assert_eq!(cursor_line, 20);
    assert!(model.editor_scroll_offset > 0);
    assert!(cursor_line >= model.editor_scroll_offset);
}

#[test]
fn test_editor_scroll_up_down() {
    // Use a long document so scrolling isn't clamped too early
    let model = create_long_test_model();
    let model = enter_edit_mode(model);

    let model = update(model, Message::EditorScrollDown(5));
    assert_eq!(model.editor_scroll_offset, 5);

    let model = update(model, Message::EditorScrollUp(3));
    assert_eq!(model.editor_scroll_offset, 2);

    let model = update(model, Message::EditorScrollUp(10));
    assert_eq!(model.editor_scroll_offset, 0);
}

#[test]
fn test_enter_edit_mode_is_idempotent() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    // Entering edit mode again should not reset the buffer
    let model = update(model, Message::EnterEditMode);
    let buf = model.editor_buffer.as_ref().unwrap();
    assert!(buf.text().starts_with('X'));
}

// --- Editor input mapping tests ---

#[test]
fn test_e_key_enters_edit_mode() {
    let model = create_test_model();
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EnterEditMode));
}

#[test]
fn test_e_key_enters_edit_mode_when_toc_focused() {
    let model = create_test_model();
    let model = update(model, Message::ToggleTocFocus);
    assert!(model.toc_visible);
    assert!(model.toc_focused);

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EnterEditMode));
}

#[test]
fn test_editor_mode_esc_exits() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::ExitEditMode));
}

#[test]
fn test_editor_mode_char_inserts() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorInsertChar('a')));
}

#[test]
fn test_editor_mode_arrows_move_cursor() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Left, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(
        msg,
        Some(Message::EditorMoveCursor(crate::editor::Direction::Left))
    );

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Right, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(
        msg,
        Some(Message::EditorMoveCursor(crate::editor::Direction::Right))
    );
}

#[test]
fn test_editor_mode_ctrl_s_saves() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorSave));
}

#[test]
fn test_editor_mode_enter_splits_line() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorSplitLine));
}

#[test]
fn test_editor_mode_backspace_deletes_back() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorDeleteBack));
}

#[test]
fn test_editor_mode_ctrl_left_moves_word_left() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Left, KeyModifiers::CONTROL),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorMoveWordLeft));
}

#[test]
fn test_editor_mode_home_end() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Home, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorMoveHome));

    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::End, KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::EditorMoveEnd));
}

#[test]
fn test_editor_mode_mouse_scroll() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    let msg = App::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        },
        &model,
    );
    assert_eq!(msg, Some(Message::EditorScrollDown(3)));

    let msg = App::handle_mouse(
        MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 10,
            row: 10,
            modifiers: KeyModifiers::NONE,
        },
        &model,
    );
    assert_eq!(msg, Some(Message::EditorScrollUp(3)));
}

#[test]
fn test_editor_save_writes_file() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Hello\n\nOriginal content").unwrap();

    let doc = Document::parse("# Hello\n\nOriginal content").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    // Enter edit mode
    model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);

    // Type some text at the beginning
    model = update(model, Message::EditorInsertChar('X'));
    assert!(model.editor_buffer.as_ref().unwrap().is_dirty());

    // Save via side effect
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);

    // Buffer should be marked clean
    assert!(!model.editor_buffer.as_ref().unwrap().is_dirty());

    // File should contain the modified content
    let saved = std::fs::read_to_string(&file_path).unwrap();
    assert!(saved.starts_with('X'));
}

#[test]
fn test_editor_save_no_changes_shows_toast() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Hello").unwrap();

    let doc = Document::parse("# Hello").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    model = enter_edit_mode(model);

    // Save without any edits
    let mut watcher = None;
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);

    // Should show "No changes" toast
    let toast = model.active_toast();
    assert!(toast.is_some());
    assert!(toast.unwrap().0.contains("No changes"));
}

// --- Dirty buffer protection tests ---

#[test]
fn test_quit_with_dirty_editor_shows_warning() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    // First quit should warn, not quit
    let model = update(model, Message::Quit);
    assert!(!model.should_quit);
    assert!(model.quit_confirmed);
}

#[test]
fn test_quit_dirty_editor_toast_says_ctrl_q() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    let model = update(model, Message::Quit);
    let toast = model.active_toast().map(|(msg, _)| msg.to_string());
    assert!(
        toast
            .as_deref()
            .is_some_and(|t| t.contains("Ctrl+Q") || t.contains("ctrl+q")),
        "dirty quit toast should mention Ctrl+Q, got: {:?}",
        toast
    );
}

#[test]
fn test_ctrl_e_toggles_edit_mode() {
    let model = create_test_model();
    // Ctrl+E in normal mode should enter edit mode
    let ctrl_e = event::KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
    let msg = App::handle_key(ctrl_e, &model);
    assert_eq!(msg, Some(Message::EnterEditMode));

    // Ctrl+E in editor mode should exit edit mode
    let model = enter_edit_mode(model);
    assert!(model.editor_mode);
    let msg = App::handle_key(ctrl_e, &model);
    assert_eq!(msg, Some(Message::ExitEditMode));
}

#[test]
fn test_quit_twice_with_dirty_editor_quits() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    let model = update(model, Message::Quit);
    assert!(!model.should_quit);

    let model = update(model, Message::Quit);
    assert!(model.should_quit);
}

#[test]
fn test_quit_clean_editor_quits_immediately() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    // No edits — quit should work immediately
    let model = update(model, Message::Quit);
    assert!(model.should_quit);
}

#[test]
fn test_quit_confirmation_resets_on_other_action() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    // First quit warns
    let model = update(model, Message::Quit);
    assert!(model.quit_confirmed);

    // Typing resets the confirmation
    let model = update(model, Message::EditorInsertChar('Y'));
    assert!(!model.quit_confirmed);

    // Must press q twice again
    let model = update(model, Message::Quit);
    assert!(!model.should_quit);
}

#[test]
fn test_exit_edit_mode_dirty_warns_first() {
    let model = create_test_model();
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('X'));

    // First Esc warns
    let model = update(model, Message::ExitEditMode);
    assert!(model.editor_mode);
    assert!(model.exit_confirmed);

    // Second Esc actually exits
    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);
}

#[test]
fn test_exit_edit_mode_clean_exits_immediately() {
    let model = create_test_model();
    let model = enter_edit_mode(model);

    // No edits — Esc should exit immediately
    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);
}

#[test]
fn test_save_during_quit_warning_quits_after_save() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path;

    let mut watcher = None;
    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Quit warns about dirty buffer
    model = update(model, Message::Quit);
    assert!(!model.should_quit);
    assert!(model.quit_confirmed);

    // Ctrl+S saves and should auto-quit
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);
    assert!(
        model.should_quit,
        "save during quit warning should auto-quit"
    );
}

#[test]
fn test_save_during_exit_warning_exits_edit_mode() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path;

    let mut watcher = None;
    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Esc warns about dirty buffer
    model = update(model, Message::ExitEditMode);
    assert!(model.editor_mode);
    assert!(model.exit_confirmed);

    // Ctrl+S saves and should auto-exit edit mode
    let file_path = model.file_path.clone();
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);
    assert!(
        !model.editor_mode,
        "save during exit warning should auto-exit edit mode"
    );

    // File must actually be saved to disk
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        on_disk.starts_with('X'),
        "file should be saved to disk, got: {:?}",
        &on_disk[..on_disk.len().min(40)]
    );

    // Document must reflect the saved content
    assert!(
        model.document.source().starts_with('X'),
        "document should be updated with saved content, got: {:?}",
        &model.document.source()[..model.document.source().len().min(40)]
    );
}

// --- Scroll position sync tests ---

#[test]
fn test_enter_edit_mode_from_scrolled_position() {
    let model = create_long_test_model();
    // Scroll to middle
    let model = update(model, Message::GoToBottom);
    let vp_offset = model.viewport.offset();
    assert!(vp_offset > 0, "Should be scrolled");

    let model = enter_edit_mode(model);
    // Editor should not start at line 0
    assert!(
        model.editor_scroll_offset > 0,
        "Editor should scroll to approximate position, got {}",
        model.editor_scroll_offset
    );
}

#[test]
fn test_enter_edit_mode_from_top_stays_at_top() {
    let model = create_test_model();
    assert_eq!(model.viewport.offset(), 0);

    let model = enter_edit_mode(model);
    assert_eq!(model.editor_scroll_offset, 0);
    assert_eq!(model.editor_buffer.as_ref().unwrap().cursor().line, 0);
}

#[test]
fn test_exit_edit_mode_restores_scroll_position() {
    let model = create_long_test_model();
    let model = enter_edit_mode(model);

    // Scroll editor to the middle
    let mut model = model;
    for _ in 0..30 {
        model = update(
            model,
            Message::EditorMoveCursor(crate::editor::Direction::Down),
        );
    }
    assert!(model.editor_scroll_offset > 0);

    // Exit edit mode
    let model = update(model, Message::ExitEditMode);
    // View should be scrolled to approximately the same position
    assert!(
        model.viewport.offset() > 0,
        "Viewport should be scrolled after exiting editor"
    );
}

#[test]
fn test_discard_edit_then_reenter_shows_original() {
    let model = create_test_model();
    let original_source = model.document.source().to_string();

    // Enter edit, make a change
    let model = enter_edit_mode(model);
    let model = update(model, Message::EditorInsertChar('Z'));

    // Discard: Esc twice (first warns, second discards)
    let model = update(model, Message::ExitEditMode);
    let model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode);

    // Document should have the ORIGINAL source, not the edited version
    assert_eq!(model.document.source(), original_source);

    // Re-enter edit mode — buffer should also have the original source
    let model = enter_edit_mode(model);
    let buf = model.editor_buffer.as_ref().unwrap();
    assert!(
        !buf.text().starts_with('Z'),
        "Re-entered editor should show original text, not discarded edits"
    );
    assert_eq!(buf.text(), original_source);
}

// --- Editor disk conflict tests ---

#[test]
fn test_enter_edit_mode_stores_disk_hash() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Hello").unwrap();

    let doc = Document::parse("# Hello").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();
    assert!(model.editor_disk_hash.is_none());

    let mut model = update(model, Message::EnterEditMode);
    let mut watcher = None;
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);

    assert!(
        model.editor_disk_hash.is_some(),
        "disk hash should be set on entering edit mode"
    );
}

#[test]
fn test_file_changed_during_edit_sets_conflict() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    assert!(!model.editor_disk_conflict);

    // Simulate external file change
    std::fs::write(&file_path, "# Changed externally").unwrap();

    App::handle_message_side_effects(&mut model, &mut watcher, &Message::FileChanged);

    assert!(
        model.editor_disk_conflict,
        "conflict should be detected when file changes during edit"
    );
    assert!(model.editor_mode, "should remain in editor mode");
}

#[test]
fn test_file_changed_during_edit_skips_reload() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    let original_doc_source = model.document.source().to_string();

    // Simulate external file change
    std::fs::write(&file_path, "# Changed externally").unwrap();

    App::handle_message_side_effects(&mut model, &mut watcher, &Message::FileChanged);

    // Document should NOT have been reloaded
    assert_eq!(
        model.document.source(),
        original_doc_source,
        "document should not reload during edit"
    );
}

#[test]
fn test_save_with_disk_conflict_warns_first() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Change file on disk behind our back
    std::fs::write(&file_path, "# Changed externally").unwrap();

    // First save should detect conflict and warn
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);

    assert!(
        model.save_confirmed,
        "first save should set save_confirmed on conflict"
    );
    // File on disk should still be the external change
    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(
        on_disk, "# Changed externally",
        "file should not be overwritten on first save"
    );
}

#[test]
fn test_save_with_disk_conflict_second_save_forces() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Change file on disk behind our back
    std::fs::write(&file_path, "# Changed externally").unwrap();

    // First save: warns
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);
    assert!(model.save_confirmed);

    // Second save: forces overwrite
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);

    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        on_disk.starts_with('X'),
        "second save should force overwrite"
    );
    assert!(
        !model.save_confirmed,
        "save_confirmed should reset after successful save"
    );
}

#[test]
fn test_save_without_conflict_succeeds_immediately() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    model = update(model, Message::EditorInsertChar('X'));

    // Save without any external change — should succeed immediately
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);

    let on_disk = std::fs::read_to_string(&file_path).unwrap();
    assert!(
        on_disk.starts_with('X'),
        "save should succeed immediately without conflict"
    );
    assert!(!model.save_confirmed);
}

#[test]
fn test_file_changed_shows_reload_toast() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "# Original").unwrap();

    let doc = Document::parse("# Original").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    let mut model = update(model, Message::FileChanged);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::FileChanged);

    let toast_msg = model.active_toast().map(|(msg, _)| msg.to_string());
    assert!(
        toast_msg.as_deref().is_some_and(|t| t.contains("reloaded")),
        "FileChanged should show a 'reloaded' toast: got {:?}",
        toast_msg
    );
}

#[test]
fn test_toggle_watch_watches_model_file_path_not_app_path() {
    let dir = tempdir().unwrap();
    let canonical_dir = dir.path().canonicalize().unwrap();
    let file_path = canonical_dir.join("doc.md");
    std::fs::write(&file_path, "# Hello").unwrap();

    // Model points at the actual file (as if browse mode navigated to it)
    let doc = Document::parse("# Hello").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.watch_enabled = true;

    let mut watcher: Option<crate::watcher::FileWatcher> = None;

    // ToggleWatch should create a watcher for model.file_path, not some other path
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::ToggleWatch);

    let w = watcher.as_ref().expect("watcher should be created");
    assert_eq!(
        w.target_path(),
        &file_path,
        "watcher must target model.file_path, not the app's original CLI path"
    );
}

// --- Editor mouse click tests ---

#[test]
fn test_editor_mouse_click_positions_cursor() {
    let md = "Hello world\nSecond line\nThird line\n";
    let doc = Document::parse(md).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = enter_edit_mode(model);

    let buf = model.editor_buffer.as_ref().unwrap();
    let gutter_width = crate::ui::line_number_width(buf.line_count()) as u16 + 1; // +1 space

    // Click on line 1 (0-indexed), col 3
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: gutter_width + 3,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::EditorMoveTo(1, 3)));

    // Apply and verify cursor position
    let model = update(model, Message::EditorMoveTo(1, 3));
    let cursor = model.editor_buffer.as_ref().unwrap().cursor();
    assert_eq!(cursor.line, 1);
    assert_eq!(cursor.col, 3);
}

#[test]
fn test_editor_mouse_click_with_scroll_offset() {
    let mut md = String::new();
    for i in 1..=50 {
        md.push_str(&format!("Line {i}\n"));
    }
    let doc = Document::parse(&md).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = enter_edit_mode(model);

    // Scroll down 10 lines
    let model = update(model, Message::EditorScrollDown(10));
    assert_eq!(model.editor_scroll_offset, 10);

    let buf = model.editor_buffer.as_ref().unwrap();
    let gutter_width = crate::ui::line_number_width(buf.line_count()) as u16 + 1;

    // Click on row 5 of the viewport — should map to line 15 (scroll_offset=10 + row=5)
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: gutter_width + 2,
        row: 5,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::EditorMoveTo(15, 2)));
}

#[test]
fn test_editor_mouse_click_clamps_past_end() {
    let md = "Short\n";
    let doc = Document::parse(md).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = enter_edit_mode(model);

    // Click at a column past the end of the line — move_to should clamp
    let model = update(model, Message::EditorMoveTo(0, 999));
    let cursor = model.editor_buffer.as_ref().unwrap().cursor();
    assert_eq!(cursor.line, 0);
    // Should be clamped to line length
    assert!(cursor.col <= 5);

    // Click past the last line — move_to should clamp
    let model = update(model, Message::EditorMoveTo(999, 0));
    let cursor = model.editor_buffer.as_ref().unwrap().cursor();
    assert!(cursor.line < 999);
}

#[test]
fn test_editor_mouse_click_in_gutter() {
    let md = "Hello world\nSecond line\n";
    let doc = Document::parse(md).unwrap();
    let model = Model::new(PathBuf::from("test.md"), doc, (80, 24));
    let model = enter_edit_mode(model);

    // Click in the gutter area (column 0) — should set col to 0
    let mouse = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: 0,
        row: 1,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::EditorMoveTo(1, 0)));
}

// ---- Help scroll tests ----

#[test]
fn test_help_scroll_offset_initial_is_zero() {
    let model = create_test_model();
    assert_eq!(model.help_scroll_offset, 0);
}

#[test]
fn test_toggle_help_resets_scroll_offset() {
    let mut model = create_test_model();
    model.help_scroll_offset = 5;
    let model = update(model, Message::ToggleHelp);
    assert!(model.help_visible);
    assert_eq!(model.help_scroll_offset, 0);
}

#[test]
fn test_help_scroll_down_increments_offset() {
    let mut model = create_test_model();
    model.help_visible = true;
    let model = update(model, Message::HelpScrollDown(1));
    assert_eq!(model.help_scroll_offset, 1);
}

#[test]
fn test_help_scroll_up_floors_at_zero() {
    let mut model = create_test_model();
    model.help_visible = true;
    model.help_scroll_offset = 0;
    let model = update(model, Message::HelpScrollUp(1));
    assert_eq!(model.help_scroll_offset, 0);
}

#[test]
fn test_help_scroll_up_decrements_offset() {
    let mut model = create_test_model();
    model.help_visible = true;
    model.help_scroll_offset = 5;
    let model = update(model, Message::HelpScrollUp(2));
    assert_eq!(model.help_scroll_offset, 3);
}

#[test]
fn test_help_mode_j_scrolls_down() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HelpScrollDown(1)));
}

#[test]
fn test_help_mode_k_scrolls_up() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HelpScrollUp(1)));
}

#[test]
fn test_help_mode_space_page_down() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HelpScrollDown(10)));
}

#[test]
fn test_help_mode_q_closes() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HideHelp));
}

#[test]
fn test_help_mode_arbitrary_key_ignored() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, None);
}

#[test]
fn test_help_mode_g_goes_to_top() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE),
        &model,
    );
    assert_eq!(msg, Some(Message::HelpScrollUp(usize::MAX)));
}

#[test]
fn test_help_mode_shift_g_goes_to_bottom() {
    let mut model = create_test_model();
    model.help_visible = true;
    let msg = App::handle_key(
        event::KeyEvent::new(KeyCode::Char('G'), KeyModifiers::SHIFT),
        &model,
    );
    assert_eq!(msg, Some(Message::HelpScrollDown(usize::MAX)));
}

#[test]
fn test_help_mode_mouse_scroll_down() {
    let mut model = create_test_model();
    model.help_visible = true;
    let mouse = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::HelpScrollDown(3)));
}

#[test]
fn test_help_mode_mouse_scroll_up() {
    let mut model = create_test_model();
    model.help_visible = true;
    let mouse = MouseEvent {
        kind: MouseEventKind::ScrollUp,
        column: 10,
        row: 10,
        modifiers: KeyModifiers::NONE,
    };
    let msg = App::handle_mouse(mouse, &model);
    assert_eq!(msg, Some(Message::HelpScrollUp(3)));
}

#[test]
fn test_discard_after_save_shows_saved_content() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.md");
    std::fs::write(&file_path, "Original content").unwrap();

    let doc = Document::parse("Original content").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path.clone();

    let mut watcher = None;

    // Enter edit mode
    model = update(model, Message::EnterEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);

    // Type a character and save
    model = update(model, Message::EditorInsertChar('X'));
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EditorSave);
    assert!(!model.editor_buffer.as_ref().unwrap().is_dirty());

    // Type another character (buffer is dirty again)
    model = update(model, Message::EditorInsertChar('Y'));
    assert!(model.editor_buffer.as_ref().unwrap().is_dirty());

    // Discard: Esc twice (first warns, second discards)
    model = update(model, Message::ExitEditMode);
    assert!(model.editor_mode, "first Esc should warn, not exit");
    model = update(model, Message::ExitEditMode);
    assert!(!model.editor_mode, "second Esc should exit");

    // Document should reflect the SAVED state (with 'X'), not the original
    assert!(
        model.document.source().contains('X'),
        "Document should contain saved content, got: {}",
        model.document.source()
    );
    // And should NOT contain the unsaved 'Y'
    assert!(
        !model.document.source().contains('Y'),
        "Document should not contain unsaved edits"
    );
}

// --- External editor tests ---

#[test]
fn test_enter_edit_mode_with_external_editor_skips_builtin() {
    let mut model = create_test_model();
    model.external_editor = Some("hx".to_string());
    let model = update(model, Message::EnterEditMode);
    // External editor path: model should NOT enter built-in editor mode
    assert!(!model.editor_mode);
    assert!(model.editor_buffer.is_none());
}

#[test]
fn test_enter_edit_mode_with_external_editor_preserves_external_editor_field() {
    let mut model = create_test_model();
    model.external_editor = Some("hx".to_string());
    let model = update(model, Message::EnterEditMode);
    // The external_editor field should be preserved through update
    assert_eq!(model.external_editor, Some("hx".to_string()));
}

#[test]
fn test_enter_edit_mode_without_external_editor_enters_builtin() {
    let mut model = create_test_model();
    model.external_editor = None;
    let model = update(model, Message::EnterEditMode);
    // No external editor: should enter built-in editor as usual
    assert!(model.editor_mode);
    // Buffer is populated by effects, not by update
}

#[test]
fn test_builtin_editor_loads_raw_file_not_wrapped_source() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.py");
    std::fs::write(&file_path, "print('hello')").unwrap();

    // Load the file — markless wraps .py files in code fences
    let doc = Document::parse("```python\nprint('hello')\n```").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path;

    // Enter edit mode + run side effects
    let mut model = update(model, Message::EnterEditMode);
    let mut watcher = None;
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);

    // The editor buffer should contain the RAW file content, not the wrapped source
    let buf_text = model
        .editor_buffer
        .as_ref()
        .map(|b| b.text())
        .unwrap_or_default();
    assert!(
        !buf_text.contains("```python"),
        "Editor buffer should not contain code fences, got: {buf_text}"
    );
    assert!(
        buf_text.contains("print('hello')"),
        "Editor buffer should contain raw file content, got: {buf_text}"
    );
}

#[test]
fn test_exit_edit_mode_preserves_wrapping_for_non_md_files() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().canonicalize().unwrap().join("test.py");
    std::fs::write(&file_path, "print('hello')").unwrap();

    // Load the file — markless wraps .py files in code fences
    let doc = Document::parse("```python\nprint('hello')\n```").unwrap();
    let mut model = Model::new(file_path.clone(), doc, (80, 24));
    model.file_path = file_path;

    // Enter edit mode
    let mut model = update(model, Message::EnterEditMode);
    let mut watcher = None;
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::EnterEditMode);
    assert!(model.editor_mode);

    // Exit edit mode (no changes — buffer is clean)
    model = update(model, Message::ExitEditMode);
    App::handle_message_side_effects(&mut model, &mut watcher, &Message::ExitEditMode);
    assert!(!model.editor_mode);

    // Document should still have the code fence wrapping for syntax highlighting
    let source = model.document.source().to_string();
    assert!(
        source.contains("```"),
        "Document should preserve code fence wrapping after exit, got: {source}"
    );
}

// --- Edit restriction tests ---

#[test]
fn test_can_edit_returns_true_for_markdown_file() {
    let model = create_test_model();
    assert!(model.can_edit());
}

#[test]
fn test_can_edit_returns_false_for_image_file() {
    let doc = Document::parse("![photo](photo.png)").unwrap();
    let model = Model::new(PathBuf::from("photo.png"), doc, (80, 24));
    assert!(!model.can_edit());
}

#[test]
fn test_can_edit_returns_false_for_binary_file() {
    let bytes = vec![0x00, 0x01, 0x02, 0xff];
    let doc =
        crate::document::prepare_document_from_bytes(std::path::Path::new("data.bin"), bytes, 80);
    let model = Model::new(PathBuf::from("data.bin"), doc, (80, 24));
    assert!(!model.can_edit());
}

#[test]
fn test_enter_edit_mode_blocked_for_image_file() {
    let doc = Document::parse("![photo](photo.png)").unwrap();
    let model = Model::new(PathBuf::from("photo.png"), doc, (80, 24));

    let model = update(model, Message::EnterEditMode);
    assert!(
        !model.editor_mode,
        "should not enter edit mode for image files"
    );
    assert!(
        model.active_toast().is_some(),
        "should show a toast explaining why editing is blocked"
    );
}

#[test]
fn test_enter_edit_mode_blocked_for_binary_file() {
    let bytes = vec![0x00, 0x01, 0x02, 0xff];
    let doc =
        crate::document::prepare_document_from_bytes(std::path::Path::new("data.bin"), bytes, 80);
    let model = Model::new(PathBuf::from("data.bin"), doc, (80, 24));

    let model = update(model, Message::EnterEditMode);
    assert!(
        !model.editor_mode,
        "should not enter edit mode for binary files"
    );
    assert!(
        model.active_toast().is_some(),
        "should show a toast explaining why editing is blocked"
    );
}

#[test]
fn test_can_edit_returns_true_for_svg_file() {
    // SVG is XML text — editable despite being rendered as an image
    let doc = Document::parse("![logo](logo.svg)").unwrap();
    let model = Model::new(PathBuf::from("logo.svg"), doc, (80, 24));
    assert!(model.can_edit(), "SVG files should be editable");
}

#[test]
fn test_can_edit_returns_false_for_unknown_extension() {
    // Unknown extensions should not be editable (whitelist approach)
    let doc = Document::parse("some content").unwrap();
    let model = Model::new(PathBuf::from("data.xyz"), doc, (80, 24));
    assert!(
        !model.can_edit(),
        "unknown extensions should not be editable"
    );
}

#[test]
fn test_enter_edit_mode_blocked_toast_includes_extension() {
    let doc = Document::parse("![photo](photo.png)").unwrap();
    let model = Model::new(PathBuf::from("photo.png"), doc, (80, 24));

    let model = update(model, Message::EnterEditMode);
    let (message, _level) = model.active_toast().expect("should show toast");
    assert!(
        message.contains(".png"),
        "toast should mention the file extension, got: {message}"
    );
}

#[test]
fn test_enter_edit_mode_allowed_for_text_file() {
    let model = create_test_model();
    let model = update(model, Message::EnterEditMode);
    assert!(model.editor_mode, "should enter edit mode for text files");
}
