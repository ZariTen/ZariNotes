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
    /// Ctrl+V. The app checks the clipboard for an image before pasting text.
    Paste,
    /// Left button released. Ends a drag selection.
    DragEnd,
    /// Latest modifier state, so shift-click can extend a selection.
    Modifiers(Modifiers),
    /// Pointer moved over the raw editor while a drag is in progress.
    EditorDrag(Point),
    /// Left press on an image's corner handle. `x` is the window point.
    ResizeStart {
        line: usize,
        url: String,
        width: f32,
        x: f32,
    },
    /// Pointer moved while a corner drag is in progress. `x` is the window point.
    ResizeMove(f32),
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
    /// Ctrl+V. The app checks the clipboard for an image before pasting text.
    Paste,
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
    /// Corner drag in progress. The width is written into the note on release.
    resize: Option<Resize>,
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
            resize: None,
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
        let caret = self.editor.cursor().position;
        Position {
            line: self.editor_lines.start + caret.line,
            column: caret.column,
        }
    }

    pub fn has_selection(&self) -> bool {
        self.editor.selection().is_some()
    }

    pub fn is_resizing(&self) -> bool {
        self.resize.is_some()
    }

    pub fn focus(&self) -> Task<Msg> {
        Task::batch([
            iced::widget::operation::focus(Id::new(EDITOR_ID)),
            self.scroll_into_view(),
        ])
    }

    pub fn update(&mut self, msg: Msg) -> (Task<Msg>, Outcome) {
        // Consumed here so EditorDrag sees only the previous message's hit-test.
        let native_drag = self.native_drag;
        self.native_drag = false;
        match msg {
            Msg::Edit(action) => self.on_edit(action),
            Msg::Nav(nav) => (self.navigate(nav), Outcome::None),
            Msg::MergeUp => self.merge_up(),
            Msg::MergeDown => self.merge_down(),
            Msg::Activate(index) => self.activate(index),
            Msg::Hover(index, point) => self.hover(index, point),
            Msg::ToggleTask(line) => self.toggle_task(line),
            Msg::Link(url) => (Task::none(), Outcome::Link(url)),
            Msg::Save => (Task::none(), Outcome::Save),
            Msg::ToggleMode => (Task::none(), Outcome::ToggleMode),
            Msg::Undo => (Task::none(), Outcome::Undo),
            Msg::Redo => (Task::none(), Outcome::Redo),
            Msg::Paste => (Task::none(), Outcome::Paste),
            Msg::DragEnd => self.drag_end(),
            Msg::Modifiers(modifiers) => {
                self.shift = modifiers.shift();
                (Task::none(), Outcome::None)
            }
            Msg::EditorDrag(point) => self.on_editor_drag(point, native_drag),
            Msg::ResizeStart {
                line,
                url,
                width,
                x,
            } => self.start_resize(line, url, width, x),
            Msg::ResizeMove(x) => self.resize_to(x),
        }
    }

    fn on_edit(&mut self, action: Action) -> (Task<Msg>, Outcome) {
        if let Some(task) = self.redirected_selection(&action) {
            return (task, Outcome::None);
        }
        self.mark_drag(&action);
        if self.shift_click(&action) {
            return (self.extend_shift_click(action), Outcome::None);
        }
        let edited = action.is_edit();
        self.editor.perform(action);
        if !edited {
            return (self.after_cursor_change(), Outcome::None);
        }
        (self.sync_editor(), Outcome::Changed)
    }

    fn mark_drag(&mut self, action: &Action) {
        if matches!(action, Action::Click(_) | Action::Drag(_)) {
            self.dragging = true;
        }
        if matches!(action, Action::Drag(_)) {
            self.native_drag = true;
        }
    }

    fn shift_click(&self, action: &Action) -> bool {
        self.shift && matches!(action, Action::Click(_))
    }

    fn extend_shift_click(&mut self, action: Action) -> Task<Msg> {
        let anchor = self.selection_anchor();
        self.editor.perform(action);
        self.show_selection(anchor, self.cursor())
    }

    fn merge_up(&mut self) -> (Task<Msg>, Outcome) {
        let line = self.segments[self.active].lines.start;
        self.join_with_previous(line)
    }

    fn merge_down(&mut self) -> (Task<Msg>, Outcome) {
        let line = self.segments[self.active].lines.end;
        self.join_with_previous(line)
    }

    fn join_with_previous(&mut self, line: usize) -> (Task<Msg>, Outcome) {
        if line == 0 || line >= self.lines.len() {
            return (Task::none(), Outcome::None);
        }
        let column = self.lines[line - 1].len();
        let removed = self.lines.remove(line);
        self.lines[line - 1].push_str(&removed);
        self.rebuild();
        let task = self.set_cursor(Position {
            line: line - 1,
            column,
        });
        (task, Outcome::Changed)
    }

    fn activate(&mut self, index: usize) -> (Task<Msg>, Outcome) {
        let pos = self.click_position(index);
        self.dragging = true;
        if self.shift {
            return (self.extend_to(pos), Outcome::None);
        }
        (self.set_cursor(pos), Outcome::None)
    }

    fn hover(&mut self, index: usize, point: Point) -> (Task<Msg>, Outcome) {
        self.hover = Some((index, point));
        if !self.dragging {
            return (Task::none(), Outcome::None);
        }
        let pos = self.click_position(index);
        (self.extend_to(pos), Outcome::None)
    }

    fn toggle_task(&mut self, line: usize) -> (Task<Msg>, Outcome) {
        let Some(text) = self.lines.get_mut(line) else {
            return (Task::none(), Outcome::None);
        };
        flip_task_box(text);
        let cursor = self.cursor();
        self.rebuild();
        self.active = self.segment_at(cursor.line);
        (Task::none(), Outcome::Changed)
    }

    fn drag_end(&mut self) -> (Task<Msg>, Outcome) {
        if let Some(resize) = self.resize.take() {
            let changed = self.commit_resize(&resize);
            let outcome = if changed {
                Outcome::Changed
            } else {
                Outcome::None
            };
            return (Task::none(), outcome);
        }
        let was_dragging = self.dragging;
        self.dragging = false;
        if !was_dragging {
            return (Task::none(), Outcome::None);
        }
        // A click focuses the editor, then the release arrives. Keep that
        // focus so typing works and a one-line selection stays visible.
        (
            Task::batch([self.after_cursor_change(), self.focus()]),
            Outcome::None,
        )
    }

    fn on_editor_drag(&mut self, point: Point, native_drag: bool) -> (Task<Msg>, Outcome) {
        if native_drag || !self.dragging || !self.spans_extra() {
            return (Task::none(), Outcome::None);
        }
        (self.extend_to(self.point_in_editor(point)), Outcome::None)
    }

    fn start_resize(
        &mut self,
        line: usize,
        url: String,
        width: f32,
        x: f32,
    ) -> (Task<Msg>, Outcome) {
        self.resize = Some(Resize {
            line,
            url,
            start_x: x,
            start_width: width,
            width,
        });
        (Task::none(), Outcome::None)
    }

    fn resize_to(&mut self, x: f32) -> (Task<Msg>, Outcome) {
        let Some(resize) = &mut self.resize else {
            return (Task::none(), Outcome::None);
        };
        let next = resize.start_width + (x - resize.start_x);
        resize.width = next.clamp(
            crate::images::MIN_IMAGE_WIDTH,
            crate::images::MAX_IMAGE_WIDTH,
        );
        (Task::none(), Outcome::None)
    }

    /// Write the dragged width into the image link. Does not move the cursor.
    fn commit_resize(&mut self, resize: &Resize) -> bool {
        let width = resize.width.round() as u32;
        if (width as f32 - resize.start_width.round()).abs() < 0.5 {
            return false;
        }
        let Some(index) = self
            .segments
            .iter()
            .position(|seg| seg.lines.start == resize.line)
        else {
            return false;
        };
        let range = self.segments[index].lines.clone();
        let mut changed = false;
        for i in range {
            let Some(line) = self.lines.get(i) else {
                continue;
            };
            let Some(next) = crate::images::set_image_width(line, &resize.url, width) else {
                continue;
            };
            if next != *line {
                self.lines[i] = next;
                changed = true;
            }
        }
        if changed {
            self.rebuild();
        }
        changed
    }
}

/// A corner drag that has not been written into the note yet.
#[derive(Debug, Clone)]
struct Resize {
    line: usize,
    url: String,
    start_x: f32,
    start_width: f32,
    width: f32,
}

fn flip_task_box(line: &mut String) {
    if let Some(at) = line.find("[ ]") {
        line.replace_range(at..at + 3, "[x]");
        return;
    }
    if let Some(at) = checked_box(line) {
        line.replace_range(at..at + 3, "[ ]");
    }
}

fn checked_box(line: &str) -> Option<usize> {
    if let Some(at) = line.find("[x]") {
        return Some(at);
    }
    line.find("[X]")
}
