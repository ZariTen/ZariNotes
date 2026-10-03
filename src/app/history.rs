//! Undo and redo for the open note.
//!
//! iced's text editor has no history, and live preview rebuilds the editor
//! whenever the cursor changes segment, so a widget-local undo would forget
//! everything outside the current line. Snapshots are the whole note.
//!
//! Consecutive inserts coalesce into a word (a trailing space stays with the
//! word). Consecutive backspaces coalesce, and so do consecutive deletes.
//! Enter, paste, and any edit that replaces a selection are their own step.

use iced::widget::text_editor::{Action, Edit, Position};

const LIMIT: usize = 100;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Group {
    Insert,
    /// Whitespace that closed an insert, or a run of spaces on its own (a tab).
    Space,
    Backspace,
    Delete,
}

/// What an edit does, for grouping.
#[derive(Clone, Copy)]
pub(super) enum EditKind {
    Insert(char),
    Backspace,
    Delete,
    /// Paste, enter, a selection replacement, a task toggle. Never coalesced.
    Other,
}

/// How a message should affect the history.
#[derive(Clone, Copy)]
pub(super) enum Input {
    Edit(EditKind),
    /// The caret moved. The next edit starts a new step.
    Moved,
    /// Scroll, modifiers, and the like.
    Ignore,
}

pub(super) fn input_action(action: &Action, selected: bool) -> Input {
    match action {
        Action::Edit(edit) => Input::Edit(edit_kind(edit, selected)),
        Action::Scroll { .. } => Input::Ignore,
        _ => Input::Moved,
    }
}

fn edit_kind(edit: &Edit, selected: bool) -> EditKind {
    if selected {
        return EditKind::Other;
    }
    match edit {
        Edit::Insert(c) => EditKind::Insert(*c),
        Edit::Backspace => EditKind::Backspace,
        Edit::Delete => EditKind::Delete,
        _ => EditKind::Other,
    }
}

struct Planned {
    starts_new: bool,
    group: Option<Group>,
}

fn plan(open: Option<Group>, kind: EditKind) -> Planned {
    match kind {
        EditKind::Insert(c) => plan_insert(open, c),
        EditKind::Backspace => same_group(open, Group::Backspace),
        EditKind::Delete => same_group(open, Group::Delete),
        EditKind::Other => Planned {
            starts_new: true,
            group: None,
        },
    }
}

fn plan_insert(open: Option<Group>, c: char) -> Planned {
    if c.is_whitespace() {
        return plan_space(open);
    }
    same_group(open, Group::Insert)
}

/// A space stays with the word it closed, or with a run of spaces (a tab).
fn plan_space(open: Option<Group>) -> Planned {
    let continues_word = matches!(open, Some(Group::Insert | Group::Space));
    Planned {
        starts_new: !continues_word,
        group: Some(Group::Space),
    }
}

fn same_group(open: Option<Group>, group: Group) -> Planned {
    let continues = open == Some(group);
    Planned {
        starts_new: !continues,
        group: Some(group),
    }
}

#[derive(Clone)]
pub(super) struct Snapshot {
    pub text: String,
    pub cursor: Position,
}

/// Previous grouping, kept so a no-op edit can put it back.
struct Armed {
    previous: Option<Group>,
}

#[derive(Default)]
pub(super) struct History {
    undo: Vec<Snapshot>,
    redo: Vec<Snapshot>,
    grouping: Option<Group>,
    armed: Option<Armed>,
}

impl History {
    pub(super) fn begin(&mut self, kind: EditKind, current: Snapshot) -> bool {
        let planned = plan(self.grouping, kind);
        if !planned.starts_new {
            self.grouping = planned.group;
            return false;
        }
        self.armed = Some(Armed {
            previous: self.grouping,
        });
        self.undo.push(current);
        if self.undo.len() > LIMIT {
            self.undo.drain(0..self.undo.len() - LIMIT);
        }
        self.grouping = planned.group;
        true
    }

    /// Drop the snapshot from [`begin`] when the edit was a no-op.
    /// A real edit discards the redo stack.
    pub(super) fn finish(&mut self, changed: bool) {
        let Some(armed) = self.armed.take() else {
            return;
        };
        if changed {
            self.redo.clear();
            return;
        }
        self.undo.pop();
        self.grouping = armed.previous;
    }

    pub(super) fn close(&mut self) {
        self.grouping = None;
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    pub(super) fn undo(&mut self, current: Snapshot) -> Option<Snapshot> {
        self.grouping = None;
        self.armed = None;
        let prev = self.undo.pop()?;
        self.redo.push(current);
        Some(prev)
    }

    pub(super) fn redo(&mut self, current: Snapshot) -> Option<Snapshot> {
        self.grouping = None;
        self.armed = None;
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(column: usize) -> Position {
        Position { line: 0, column }
    }

    fn snap(text: &str, column: usize) -> Snapshot {
        Snapshot {
            text: text.into(),
            cursor: at(column),
        }
    }

    /// Apply `kind` the way the editor does, against a stand-in document.
    fn apply(history: &mut History, kind: EditKind, text: &mut String, next: &str) {
        let before = text.clone();
        let stored = history.begin(kind, snap(&before, before.len()));
        *text = next.into();
        if stored {
            history.finish(text != &before);
        }
    }

    #[test]
    fn a_word_and_its_trailing_space_is_one_step() {
        let mut history = History::default();
        let mut text = String::new();
        for c in "hi there".chars() {
            let mut next = text.clone();
            next.push(c);
            apply(&mut history, EditKind::Insert(c), &mut text, &next);
        }
        assert_eq!(text, "hi there");
        let prev = history.undo(snap(&text, text.len())).unwrap();
        assert_eq!(prev.text, "hi ");
        let prev = history.undo(snap(&prev.text, prev.text.len())).unwrap();
        assert_eq!(prev.text, "");
        assert!(history.undo(snap("", 0)).is_none());
    }

    #[test]
    fn tab_is_one_step() {
        let mut history = History::default();
        let mut text = String::new();
        for _ in 0..4 {
            let mut next = text.clone();
            next.push(' ');
            apply(&mut history, EditKind::Insert(' '), &mut text, &next);
        }
        let prev = history.undo(snap(&text, 4)).unwrap();
        assert_eq!(prev.text, "");
        assert!(history.undo(snap("", 0)).is_none());
    }

    #[test]
    fn moving_the_caret_splits_the_group() {
        let mut history = History::default();
        let mut text = String::new();
        apply(&mut history, EditKind::Insert('a'), &mut text, "a");
        history.close();
        apply(&mut history, EditKind::Insert('b'), &mut text, "ab");
        let prev = history.undo(snap("ab", 2)).unwrap();
        assert_eq!(prev.text, "a");
    }

    #[test]
    fn backspace_does_not_join_an_insert() {
        let mut history = History::default();
        let mut text = String::new();
        apply(&mut history, EditKind::Insert('a'), &mut text, "a");
        apply(&mut history, EditKind::Insert('b'), &mut text, "ab");
        apply(&mut history, EditKind::Backspace, &mut text, "a");
        let prev = history.undo(snap("a", 1)).unwrap();
        assert_eq!(prev.text, "ab");
    }

    #[test]
    fn a_noop_does_not_consume_redo() {
        let mut history = History::default();
        history.begin(EditKind::Insert('a'), snap("", 0));
        history.finish(true);
        let restored = history.undo(snap("a", 1)).unwrap();
        assert_eq!(restored.text, "");
        // Backspace at the start of an empty document changes nothing.
        let stored = history.begin(EditKind::Backspace, snap("", 0));
        history.finish(false);
        assert!(stored);
        assert_eq!(history.redo(snap("", 0)).map(|s| s.text), Some("a".into()));
    }

    #[test]
    fn an_edit_after_undo_drops_redo() {
        let mut history = History::default();
        history.begin(EditKind::Insert('a'), snap("", 0));
        history.finish(true);
        history.undo(snap("a", 1));
        history.begin(EditKind::Insert('b'), snap("", 0));
        history.finish(true);
        assert!(history.redo(snap("b", 1)).is_none());
    }

    #[test]
    fn redo_restores_what_undo_removed() {
        let mut history = History::default();
        history.begin(EditKind::Other, snap("one", 3));
        history.finish(true);
        let prev = history.undo(snap("two", 3)).unwrap();
        assert_eq!(prev.text, "one");
        let next = history.redo(snap(&prev.text, 3)).unwrap();
        assert_eq!(next.text, "two");
    }
}
