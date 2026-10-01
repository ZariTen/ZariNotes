//! Live preview.
//!
//! The document is split into *segments*: usually one line each, except for
//! multi-line constructs (fenced code blocks, tables, front matter). Every
//! segment is rendered as Markdown, except the one holding the cursor, which
//! is shown as raw, syntax-highlighted Markdown inside a `text_editor`.
//!
//! The full document lives in `lines`; the editor only holds the active
//! segment and is written back into `lines` after every edit. Keys that would
//! leave the segment (arrows at its edges, Backspace at its start, …) are
//! intercepted and move the cursor into the neighbouring segment.
//!
//! A selection is different: shift-arrows, drag, shift-click and select-all
//! expand the editor across every spanned segment (shown as raw source) so
//! the selection can be copied. It shrinks back to one segment when the
//! selection collapses.

mod edit;
mod scroll;
mod split;
mod view;

#[cfg(test)]
mod tests;

use std::ops::Range;

use iced::advanced::widget::Id;
use iced::keyboard::Modifiers;
use iced::widget::markdown;
use iced::widget::text_editor::{Action, Position};
use iced::{Point, Task};

use split::estimate_raw_column;

pub const TEXT_SIZE: f32 = 16.0;
const LINE_HEIGHT: f32 = TEXT_SIZE * 1.3;
const EDITOR_ID: &str = "live-editor";
const SCROLL_ID: &str = "live-scroll";

#[derive(Debug, Clone)]
pub enum Msg {
    Edit(Action),
    Nav(Nav),
    MergeUp,
    MergeDown,
    Activate(usize),
    Hover(usize, Point),
    ToggleTask(usize),
    Link(String),
    Save,
    ToggleMode,
    Undo,
    Redo,
    /// Left button released. Ends a drag selection.
    DragEnd,
    /// Latest modifier state, so shift-click can extend a selection.
    Modifiers(Modifiers),
    /// Pointer moved over the raw editor while a drag is in progress.
    EditorDrag(Point),
}

#[derive(Debug, Clone, Copy)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
}

/// What the app should do after an update.
pub enum Outcome {
    None,
    Changed,
    Save,
    ToggleMode,
    Link(String),
    Undo,
    Redo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Kind {
    Blank,
    Markdown,
    Fence,
    Table,
    FrontMatter,
}

struct Segment {
    lines: Range<usize>,
    kind: Kind,
    /// Leading whitespace of the (first) line, in columns.
    indent: usize,
    /// Source handed to the Markdown parser (also the parse-cache key).
    source: String,
    items: Vec<markdown::Item>,
}

pub struct Live {
    lines: Vec<String>,
    segments: Vec<Segment>,
    active: usize,
    /// Document lines currently loaded in `editor`. One segment, unless a
    /// selection spans several — those lines are raw source so they can be
    /// selected and copied.
    editor_lines: Range<usize>,
    editor: iced::widget::text_editor::Content,
    hover: Option<(usize, Point)>,
    /// Left button is down; moves over other segments extend the selection.
    dragging: bool,
    /// Shift is held. A click then extends the selection instead of moving.
    shift: bool,
    /// The editor just hit-tested a drag. The fallback mapper should yield.
    native_drag: bool,
}

impl Live {
    pub fn new(text: &str, cursor: Position) -> (Self, Task<Msg>) {
        let lines = text
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l).to_owned())
            .collect();
        let mut live = Self {
            lines,
            segments: Vec::new(),
            active: 0,
            editor_lines: 0..0,
            editor: iced::widget::text_editor::Content::new(),
            hover: None,
            dragging: false,
            shift: false,
            native_drag: false,
        };
        live.rebuild();
        let task = live.set_cursor(cursor);
        (live, task)
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    /// Cursor position in document coordinates.
    pub fn cursor(&self) -> Position {
        let c = self.editor.cursor().position;
        Position {
            line: self.editor_lines.start + c.line,
            column: c.column,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.editor.selection().is_some()
    }

    pub fn focus(&self) -> Task<Msg> {
        Task::batch([
            iced::widget::operation::focus(Id::new(EDITOR_ID)),
            self.scroll_into_view(),
        ])
    }

    pub fn update(&mut self, msg: Msg) -> (Task<Msg>, Outcome) {
        let native_drag = self.native_drag;
        self.native_drag = false;
        match msg {
            Msg::Edit(action) => {
                if let Some(task) = self.redirected_selection(&action) {
                    return (task, Outcome::None);
                }
                let is_edit = action.is_edit();
                let click = matches!(action, Action::Click(_));
                if click || matches!(action, Action::Drag(_)) {
                    self.dragging = true;
                }
                if matches!(action, Action::Drag(_)) {
                    self.native_drag = true;
                }
                if click && self.shift {
                    let anchor = self.selection_anchor();
                    self.editor.perform(action);
                    return (self.show_selection(anchor, self.cursor()), Outcome::None);
                }
                self.editor.perform(action);
                if !is_edit {
                    return (self.after_cursor_change(), Outcome::None);
                }
                (self.sync_editor(), Outcome::Changed)
            }
            Msg::Nav(nav) => (self.navigate(nav), Outcome::None),
            Msg::MergeUp => {
                let s = self.segments[self.active].lines.start;
                if s == 0 {
                    return (Task::none(), Outcome::None);
                }
                let column = self.lines[s - 1].len();
                let cur = self.lines.remove(s);
                self.lines[s - 1].push_str(&cur);
                self.rebuild();
                let task = self.set_cursor(Position {
                    line: s - 1,
                    column,
                });
                (task, Outcome::Changed)
            }
            Msg::MergeDown => {
                let e = self.segments[self.active].lines.end;
                if e >= self.lines.len() {
                    return (Task::none(), Outcome::None);
                }
                let column = self.lines[e - 1].len();
                let next = self.lines.remove(e);
                self.lines[e - 1].push_str(&next);
                self.rebuild();
                let task = self.set_cursor(Position {
                    line: e - 1,
                    column,
                });
                (task, Outcome::Changed)
            }
            Msg::Activate(i) => {
                let pos = self.click_position(i);
                self.dragging = true;
                if self.shift {
                    (self.extend_to(pos), Outcome::None)
                } else {
                    (self.set_cursor(pos), Outcome::None)
                }
            }
            Msg::Hover(i, p) => {
                self.hover = Some((i, p));
                if !self.dragging {
                    return (Task::none(), Outcome::None);
                }
                let pos = self.click_position(i);
                (self.extend_to(pos), Outcome::None)
            }
            Msg::ToggleTask(line) => {
                let Some(l) = self.lines.get_mut(line) else {
                    return (Task::none(), Outcome::None);
                };
                if let Some(i) = l.find("[ ]") {
                    l.replace_range(i..i + 3, "[x]");
                } else if let Some(i) = l.find("[x]").or_else(|| l.find("[X]")) {
                    l.replace_range(i..i + 3, "[ ]");
                }
                let cursor = self.cursor();
                self.rebuild();
                self.active = self.segment_at(cursor.line);
                (Task::none(), Outcome::Changed)
            }
            Msg::Link(url) => (Task::none(), Outcome::Link(url)),
            Msg::Save => (Task::none(), Outcome::Save),
            Msg::ToggleMode => (Task::none(), Outcome::ToggleMode),
            Msg::Undo => (Task::none(), Outcome::Undo),
            Msg::Redo => (Task::none(), Outcome::Redo),
            Msg::DragEnd => {
                let was_dragging = self.dragging;
                self.dragging = false;
                if !was_dragging {
                    return (Task::none(), Outcome::None);
                }
                // A click focuses the editor, then the release arrives. Keep
                // that focus so typing works and a one-line selection stays
                // visible.
                (
                    Task::batch([self.after_cursor_change(), self.focus()]),
                    Outcome::None,
                )
            }
            Msg::Modifiers(modifiers) => {
                self.shift = modifiers.shift();
                (Task::none(), Outcome::None)
            }
            Msg::EditorDrag(p) => {
                if native_drag || !self.dragging || !self.spans_extra() {
                    return (Task::none(), Outcome::None);
                }
                let local =
                    ((p.y / LINE_HEIGHT) as usize).min(self.editor_lines.len().saturating_sub(1));
                let line = (self.editor_lines.start + local).min(self.lines.len() - 1);
                let column = estimate_raw_column(&self.lines[line], p.x);
                (self.extend_to(Position { line, column }), Outcome::None)
            }
        }
    }
}
