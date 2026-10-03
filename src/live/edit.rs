//! Cursor, selection, and writing the editor back into the document.

use std::collections::HashMap;
use std::ops::Range;

use iced::widget::markdown;
use iced::widget::text_editor::{Action, Cursor, Motion, Position};
use iced::{Point, Task};

use super::split::{estimate_column, estimate_raw_column, floor_char_boundary, split};
use super::{Kind, LINE_HEIGHT, Live, Msg, Nav, Segment, TEXT_SIZE};

impl Live {
    /// Re-split `lines` into segments, reusing parsed Markdown where the source is unchanged.
    pub(super) fn rebuild(&mut self) {
        let mut cache = take_parsed(&mut self.segments);
        self.segments = split(&self.lines)
            .into_iter()
            .map(|(range, kind)| make_segment(&self.lines, range, kind, &mut cache))
            .collect();
        self.active = self.active.min(self.segments.len() - 1);
    }

    pub(super) fn segment_at(&self, line: usize) -> usize {
        self.segments
            .partition_point(|seg| seg.lines.end <= line)
            .min(self.segments.len() - 1)
    }

    /// Put the cursor at a document position, loading its segment into the editor.
    pub(super) fn set_cursor(&mut self, pos: Position) -> Task<Msg> {
        let line = pos.line.min(self.lines.len() - 1);
        self.active = self.segment_at(line);
        let range = self.segments[self.active].lines.clone();
        self.editor_lines = range.clone();
        self.editor = iced::widget::text_editor::Content::with_text(&self.lines[range].join("\n"));
        self.editor.move_to(Cursor {
            position: Position {
                line: line - self.editor_lines.start,
                column: floor_char_boundary(&self.lines[line], pos.column),
            },
            selection: None,
        });
        self.focus()
    }

    /// Write the editor's text back into `lines` and re-segment around the cursor.
    pub(super) fn sync_editor(&mut self) -> Task<Msg> {
        let old = self.editor_lines.clone();
        let cursor = self.cursor();
        let new: Vec<String> = self.editor.text().split('\n').map(str::to_owned).collect();
        let new_len = new.len();
        self.lines.splice(old.clone(), new);
        self.rebuild();

        let idx = self.segment_at(cursor.line.min(self.lines.len() - 1));
        let range = self.segments[idx].lines.clone();
        if range.start == old.start && range.len() == new_len {
            // Same segment, editor already holds exactly its text.
            self.active = idx;
            self.editor_lines = range;
            return self.scroll_into_view();
        }
        self.set_cursor(cursor)
    }

    pub(super) fn navigate(&mut self, nav: Nav) -> Task<Msg> {
        let Some(target) = self.nav_target(nav) else {
            return Task::none();
        };
        self.set_cursor(target)
    }

    fn nav_target(&self, nav: Nav) -> Option<Position> {
        let Range { start, end } = self.segments[self.active].lines;
        let column = self.editor.cursor().position.column;
        match nav {
            Nav::Up if start > 0 => Some(Position {
                line: start - 1,
                column,
            }),
            Nav::Down if end < self.lines.len() => Some(Position { line: end, column }),
            Nav::Left if start > 0 => Some(Position {
                line: start - 1,
                column: self.lines[start - 1].len(),
            }),
            Nav::Right if end < self.lines.len() => Some(Position {
                line: end,
                column: 0,
            }),
            _ => None,
        }
    }

    /// A select-all or a select-motion that would leave the editor buffer.
    /// Those are redirected so the selection can span segments.
    pub(super) fn redirected_selection(&mut self, action: &Action) -> Option<Task<Msg>> {
        match action {
            Action::SelectAll => Some(self.select_all()),
            Action::Select(motion) if self.selection_leaves(*motion) => {
                Some(self.extend_by(*motion))
            }
            _ => None,
        }
    }

    fn selection_leaves(&self, motion: Motion) -> bool {
        let Some(edge) = self.editor_edge() else {
            return false;
        };
        match motion {
            Motion::Up => edge.at_top && edge.range_start > 0,
            Motion::Down => edge.at_bottom && edge.range_end < self.lines.len(),
            Motion::Left | Motion::WordLeft => edge.at_start && edge.range_start > 0,
            Motion::Right | Motion::WordRight => edge.at_end && edge.range_end < self.lines.len(),
            Motion::PageUp | Motion::DocumentStart => edge.range_start > 0,
            Motion::PageDown | Motion::DocumentEnd => edge.range_end < self.lines.len(),
            Motion::Home | Motion::End => false,
        }
    }

    fn editor_edge(&self) -> Option<EditorEdge> {
        let caret = self.editor.cursor().position;
        let range = &self.editor_lines;
        if range.is_empty() {
            return None;
        }
        let at_top = caret.line == 0;
        let at_bottom = caret.line + 1 >= range.len();
        let line_len = self
            .editor
            .line(caret.line)
            .map(|l| l.text.len())
            .unwrap_or(0);
        Some(EditorEdge {
            at_top,
            at_bottom,
            at_start: at_top && caret.column == 0,
            at_end: at_bottom && caret.column >= line_len,
            range_start: range.start,
            range_end: range.end,
        })
    }

    fn extend_by(&mut self, motion: Motion) -> Task<Msg> {
        let anchor = self.selection_anchor();
        let head = self.motion_target(motion);
        self.show_selection(anchor, head)
    }

    pub(super) fn extend_to(&mut self, head: Position) -> Task<Msg> {
        let anchor = self.selection_anchor();
        self.show_selection(anchor, head)
    }

    fn select_all(&mut self) -> Task<Msg> {
        let last = self.lines.len() - 1;
        self.show_selection(
            Position { line: 0, column: 0 },
            Position {
                line: last,
                column: self.lines[last].len(),
            },
        )
    }

    /// Where `motion` lands, in document coordinates, stepping out of the editor if needed.
    fn motion_target(&self, motion: Motion) -> Position {
        let cur = self.cursor();
        let last = self.lines.len() - 1;
        match motion {
            Motion::Up => self.place(cur.line.saturating_sub(1), cur.column),
            Motion::Down => self.place((cur.line + 1).min(last), cur.column),
            Motion::Left | Motion::WordLeft | Motion::Home => {
                if cur.line == 0 {
                    self.place(0, 0)
                } else {
                    self.place(cur.line - 1, self.lines[cur.line - 1].len())
                }
            }
            Motion::Right | Motion::WordRight | Motion::End => {
                if cur.line >= last {
                    self.place(last, self.lines[last].len())
                } else {
                    self.place(cur.line + 1, 0)
                }
            }
            Motion::PageUp | Motion::DocumentStart => self.place(0, 0),
            Motion::PageDown | Motion::DocumentEnd => self.place(last, self.lines[last].len()),
        }
    }

    fn place(&self, line: usize, column: usize) -> Position {
        Position {
            line,
            column: floor_char_boundary(&self.lines[line], column.min(self.lines[line].len())),
        }
    }

    pub(super) fn selection_anchor(&self) -> Position {
        let caret = self.editor.cursor();
        let local = caret.selection.unwrap_or(caret.position);
        Position {
            line: self.editor_lines.start + local.line,
            column: local.column,
        }
    }

    /// Show `anchor..=head` as one raw editor selection, covering whole segments.
    pub(super) fn show_selection(&mut self, anchor: Position, head: Position) -> Task<Msg> {
        let anchor = self.clamp_pos(anchor);
        let head = self.clamp_pos(head);
        if anchor == head {
            return self.set_cursor(head);
        }
        let range = self.covered_lines(anchor, head);
        self.load_editor(
            range.clone(),
            Cursor {
                position: self.to_local(range.start, head),
                selection: Some(self.to_local(range.start, anchor)),
            },
        )
    }

    fn covered_lines(&self, anchor: Position, head: Position) -> Range<usize> {
        let lo = anchor.line.min(head.line);
        let hi = anchor.line.max(head.line);
        let start = self.segments[self.segment_at(lo)].lines.start;
        let end = self.segments[self.segment_at(hi)].lines.end;
        start..end
    }

    fn to_local(&self, start: usize, pos: Position) -> Position {
        Position {
            line: pos.line - start,
            column: floor_char_boundary(
                &self.lines[pos.line],
                pos.column.min(self.lines[pos.line].len()),
            ),
        }
    }

    fn load_editor(&mut self, range: Range<usize>, cursor: Cursor) -> Task<Msg> {
        if self.editor_lines != range {
            self.editor = iced::widget::text_editor::Content::with_text(
                &self.lines[range.clone()].join("\n"),
            );
            self.editor_lines = range;
        }
        self.editor.move_to(cursor);
        let head = self.editor_lines.start + self.editor.cursor().position.line;
        self.active = self.segment_at(head.min(self.lines.len() - 1));
        self.focus()
    }

    pub(super) fn spans_extra(&self) -> bool {
        self.editor_lines != self.segments[self.active].lines
    }

    fn clamp_pos(&self, pos: Position) -> Position {
        let line = pos.line.min(self.lines.len() - 1);
        Position {
            line,
            column: floor_char_boundary(&self.lines[line], pos.column.min(self.lines[line].len())),
        }
    }

    /// Drop back to the caret's segment once a cross-segment selection is gone.
    pub(super) fn after_cursor_change(&mut self) -> Task<Msg> {
        if self.dragging || self.editor.cursor().selection.is_some() {
            return self.scroll_into_view();
        }
        let line = self.cursor().line.min(self.lines.len() - 1);
        let seg = self.segments[self.segment_at(line)].lines.clone();
        if seg == self.editor_lines {
            return self.scroll_into_view();
        }
        self.set_cursor(self.cursor())
    }

    /// Best-effort mapping from a click on rendered Markdown to a source position.
    pub(super) fn click_position(&self, index: usize) -> Position {
        let seg = &self.segments[index];
        let start = seg.lines.start;
        let Some(point) = self.hover_on(index) else {
            return end_of_line(&self.lines, start);
        };
        if let Some(pos) = blank_click(seg) {
            return pos;
        }
        if let Some(pos) = markdown_click(seg, &self.lines[start], point) {
            return pos;
        }
        let line = block_line(seg, point.y);
        end_of_line(&self.lines, line)
    }

    fn hover_on(&self, index: usize) -> Option<Point> {
        self.hover
            .filter(|(hit, _)| *hit == index)
            .map(|(_, point)| point)
    }

    pub(super) fn point_in_editor(&self, point: Point) -> Position {
        let last_local = self.editor_lines.len().saturating_sub(1);
        let local = ((point.y / LINE_HEIGHT) as usize).min(last_local);
        let line = (self.editor_lines.start + local).min(self.lines.len() - 1);
        let column = estimate_raw_column(&self.lines[line], point.x);
        Position { line, column }
    }
}

struct EditorEdge {
    at_top: bool,
    at_bottom: bool,
    at_start: bool,
    at_end: bool,
    range_start: usize,
    range_end: usize,
}

fn take_parsed(segments: &mut Vec<Segment>) -> HashMap<String, Vec<markdown::Item>> {
    segments
        .drain(..)
        .map(|seg| (seg.source, seg.items))
        .collect()
}

fn make_segment(
    lines: &[String],
    range: Range<usize>,
    kind: Kind,
    cache: &mut HashMap<String, Vec<markdown::Item>>,
) -> Segment {
    let first = &lines[range.start];
    let indent = indent_columns(first);
    let source = segment_source(lines, &range, kind);
    let items = cached_items(kind, &source, cache);
    Segment {
        lines: range,
        kind,
        indent: if kind == Kind::Markdown { indent } else { 0 },
        source,
        items,
    }
}

fn indent_columns(line: &str) -> usize {
    line.chars()
        .take_while(|c| c.is_whitespace())
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

fn segment_source(lines: &[String], range: &Range<usize>, kind: Kind) -> String {
    if kind == Kind::Markdown {
        return lines[range.start].trim_start().to_owned();
    }
    lines[range.clone()].join("\n")
}

fn cached_items(
    kind: Kind,
    source: &str,
    cache: &mut HashMap<String, Vec<markdown::Item>>,
) -> Vec<markdown::Item> {
    if kind == Kind::Blank || kind == Kind::FrontMatter {
        return Vec::new();
    }
    cache
        .remove(source)
        .unwrap_or_else(|| markdown::parse(source).collect())
}

fn end_of_line(lines: &[String], line: usize) -> Position {
    Position {
        line,
        column: lines[line].len(),
    }
}

fn blank_click(seg: &Segment) -> Option<Position> {
    if seg.kind != Kind::Blank {
        return None;
    }
    Some(Position {
        line: seg.lines.start,
        column: 0,
    })
}

fn markdown_click(seg: &Segment, line: &str, point: Point) -> Option<Position> {
    if seg.kind != Kind::Markdown {
        return None;
    }
    let x = point.x - seg.indent as f32 * TEXT_SIZE * 0.5;
    Some(Position {
        line: seg.lines.start,
        column: estimate_column(line, x),
    })
}

fn block_line(seg: &Segment, y: f32) -> usize {
    let start = seg.lines.start;
    let end = seg.lines.end;
    let line = match seg.kind {
        Kind::Fence => fence_line(start, end, y),
        Kind::Table => table_line(start, y),
        Kind::FrontMatter => front_matter_line(start, y),
        _ => start,
    };
    line.min(end - 1)
}

fn fence_line(start: usize, end: usize, y: f32) -> usize {
    let row_h = TEXT_SIZE * 0.875 * 1.3;
    let row = ((y - TEXT_SIZE * 1.1).max(0.0) / row_h) as usize;
    (start + 1 + row).min(end.saturating_sub(2).max(start))
}

fn table_line(start: usize, y: f32) -> usize {
    let row = (y / (LINE_HEIGHT + 10.0)) as usize;
    if row == 0 { start } else { start + 1 + row }
}

fn front_matter_line(start: usize, y: f32) -> usize {
    start + (y / (TEXT_SIZE * 0.8 * 1.3)) as usize
}
