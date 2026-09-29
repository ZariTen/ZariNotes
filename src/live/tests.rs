use iced::Point;
use iced::keyboard::Modifiers;
use iced::widget::text_editor::{Action, Edit, Motion, Position};

use super::split::{estimate_column, split};
use super::*;

fn lines(s: &str) -> Vec<String> {
    s.split('\n').map(str::to_owned).collect()
}

#[test]
fn split_groups_multiline_constructs() {
    let doc = lines(
        "---\ntitle: x\n---\n# H\n\n```rs\nfn a() {}\n```\n| a | b |\n|---|---|\n| 1 | 2 |\ntext",
    );
    let kinds = split(&doc);
    assert_eq!(
        kinds,
        [
            (0..3, Kind::FrontMatter),
            (3..4, Kind::Markdown),
            (4..5, Kind::Blank),
            (5..8, Kind::Fence),
            (8..11, Kind::Table),
            (11..12, Kind::Markdown),
        ]
    );
}

#[test]
fn typing_enter_moves_active_segment() {
    let (mut live, _) = Live::new("hello", Position { line: 0, column: 5 });
    let _ = live.update(Msg::Edit(Action::Edit(
        iced::widget::text_editor::Edit::Enter,
    )));
    let _ = live.update(Msg::Edit(Action::Edit(
        iced::widget::text_editor::Edit::Insert('x'),
    )));
    assert_eq!(live.text(), "hello\nx");
    assert_eq!(live.cursor(), Position { line: 1, column: 1 });
    assert_eq!(live.active, 1);
}

#[test]
fn merge_up_joins_lines() {
    let (mut live, _) = Live::new("ab\ncd", Position { line: 1, column: 0 });
    let _ = live.update(Msg::MergeUp);
    assert_eq!(live.text(), "abcd");
    assert_eq!(live.cursor(), Position { line: 0, column: 2 });
}

#[test]
fn toggle_task() {
    let (mut live, _) = Live::new("- [ ] a\nb", Position { line: 1, column: 0 });
    let _ = live.update(Msg::ToggleTask(0));
    assert_eq!(live.text(), "- [x] a\nb");
}

#[test]
fn shift_down_selects_into_the_next_line() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(
        iced::widget::text_editor::Motion::Down,
    )));
    assert_eq!(live.cursor(), Position { line: 1, column: 0 });
    assert_eq!(live.editor.selection().as_deref(), Some("one\n"));
}

#[test]
fn shift_down_can_cross_more_than_one_line() {
    let (mut live, _) = Live::new("a\nb\nc", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::Down)));
    let _ = live.update(Msg::Edit(Action::Select(Motion::Down)));
    assert_eq!(live.cursor(), Position { line: 2, column: 0 });
    assert_eq!(live.editor.selection().as_deref(), Some("a\nb\n"));
}

#[test]
fn shift_up_selects_the_previous_line() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 1, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::Up)));
    assert_eq!(live.cursor(), Position { line: 0, column: 0 });
    assert_eq!(live.editor.selection().as_deref(), Some("one\n"));
}

#[test]
fn shift_down_from_end_of_line_includes_the_next_line() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 0, column: 3 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::Down)));
    assert_eq!(live.editor.selection().as_deref(), Some("\ntwo"));
}

#[test]
fn select_all_covers_every_line() {
    let (mut live, _) = Live::new("one\ntwo\nthree", Position { line: 1, column: 1 });
    let _ = live.update(Msg::Edit(Action::SelectAll));
    assert_eq!(live.editor.selection().as_deref(), Some("one\ntwo\nthree"));
}

#[test]
fn deleting_a_cross_line_selection_removes_those_lines() {
    let (mut live, _) = Live::new("one\ntwo\nthree", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::Down)));
    let _ = live.update(Msg::Edit(Action::Edit(Edit::Delete)));
    assert_eq!(live.text(), "two\nthree");
    assert_eq!(live.cursor(), Position { line: 0, column: 0 });
}

#[test]
fn dragging_onto_the_next_line_selects_it() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 0, column: 3 });
    let _ = live.update(Msg::Edit(Action::Click(Point::ORIGIN)));
    let _ = live.update(Msg::Hover(1, Point::new(10_000.0, 0.0)));
    let sel = live.editor.selection().expect("selection");
    assert!(sel.contains('\n'), "{sel:?}");
    assert!(sel.contains("two"), "{sel:?}");
}

#[test]
fn shift_click_selects_from_the_cursor() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Modifiers(Modifiers::SHIFT));
    live.hover = Some((1, Point::new(10_000.0, 0.0)));
    let _ = live.update(Msg::Activate(1));
    assert_eq!(live.editor.selection().as_deref(), Some("one\ntwo"));
}

#[test]
fn moving_collapses_a_cross_line_selection() {
    let (mut live, _) = Live::new("one\ntwo", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::Down)));
    assert!(live.editor.line_count() > 1);
    let _ = live.update(Msg::Edit(Action::Move(Motion::DocumentStart)));
    assert_eq!(live.editor.line_count(), 1);
    assert!(live.editor.selection().is_none());
    assert_eq!(live.text(), "one\ntwo");
}

#[test]
fn selecting_words_on_one_line_keeps_the_selection() {
    let (mut live, _) = Live::new("alpha beta gamma", Position { line: 0, column: 0 });
    let _ = live.update(Msg::Edit(Action::Select(Motion::WordRight)));
    assert_eq!(live.editor.line_count(), 1);
    assert_eq!(live.editor.selection().as_deref(), Some("alpha"));
    // A click-drag releases. That must not wipe a one-line selection or
    // the buffer the user is typing into.
    let _ = live.update(Msg::Edit(Action::Click(Point::ORIGIN)));
    let _ = live.update(Msg::DragEnd);
    let _ = live.update(Msg::Edit(Action::Edit(Edit::Insert('Z'))));
    assert!(live.text().contains('Z'), "{}", live.text());
    assert_eq!(live.editor.line_count(), 1);
}

#[test]
fn click_column_skips_markup() {
    // "a **bold** word": visible "a bold word"; 3rd visible char ('o' of bold)
    let raw = "a **bold** word";
    let col = estimate_column(raw, TEXT_SIZE * 0.5 * 3.0);
    assert_eq!(&raw[col..col + 1], "o");
}
