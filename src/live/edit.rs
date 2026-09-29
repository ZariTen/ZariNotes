//! Cursor, selection, and writing the editor back into the document.

use std::collections::HashMap;
use std::ops::Range;

use iced::Task;
use iced::widget::markdown;
use iced::widget::text_editor::{Action, Cursor, Motion, Position};

use super::split::{estimate_column, floor_char_boundary, split};
use super::{Kind, LINE_HEIGHT, Live, Msg, Nav, Segment, TEXT_SIZE};

impl Live {
    // ── model ────────────────────────────────────────────────────

    /// Re-split `lines` into segments, reusing parsed Markdown where the source is unchanged.
    pub(super) fn rebuild(&mut self) {
        let mut cache: HashMap<String, Vec<markdown::Item>> = self
            .segments
            .drain(..)
            .map(|s| (s.source, s.items))
            .collect();

        self.segments = split(&self.lines)
            .into_iter()
            .map(|(range, kind)| {
                let first = &self.lines[range.start];
                let indent = first
                    .chars()
                    .take_while(|c| c.is_whitespace())
                    .map(|c| if c == '\t' { 4 } else { 1 })
                    .sum();
                let source = match kind {
                    Kind::Markdown => first.trim_start().to_owned(),
                    _ => self.lines[range.clone()].join("\n"),
                };
                let items = match kind {
                    Kind::Blank | Kind::FrontMatter => Vec::new(),
                    _ => cache
                        .remove(&source)
                        .unwrap_or_else(|| markdown::parse(&source).collect()),
                };
                Segment {
                    lines: range,
                    kind,
                    indent: if kind == Kind::Markdown { indent } else { 0 },
                    source,
                    items,
                }
            })
            .collect();
        self.active = self.active.min(self.segments.len() - 1);
    }

    pub(super) fn segment_at(&self, line: usize) -> usize {
        self.segments
            .partition_point(|s| s.lines.end <= line)
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
            self.scroll_into_view()
        } else {
            self.set_cursor(cursor)
        }
    }

    pub(super) fn navigate(&mut self, nav: Nav) -> Task<Msg> {
        let Range { start, end } = self.segments[self.active].lines;
        let col = self.editor.cursor().position.column;
        let target = match nav {
            Nav::Up if start > 0 => Position {
                line: start - 1,
                column: col,
            },
            Nav::Down if end < self.lines.len() => Position {
                line: end,
                column: col,
            },
            Nav::Left if start > 0 => Position {
                line: start - 1,
                column: self.lines[start - 1].len(),
            },
            Nav::Right if end < self.lines.len() => Position {
                line: end,
                column: 0,
            },
            _ => return Task::none(),
        };
        self.set_cursor(target)
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
        let c = self.editor.cursor().position;
        let range = &self.editor_lines;
        if range.is_empty() {
            return false;
        }
        let at_top = c.line == 0;
        let at_bottom = c.line + 1 >= range.len();
        let line_len = self.editor.line(c.line).map(|l| l.text.len()).unwrap_or(0);
        let at_start = at_top && c.column == 0;
        let at_end = at_bottom && c.column >= line_len;
        match motion {
            Motion::Up => at_top && range.start > 0,
            Motion::Down => at_bottom && range.end < self.lines.len(),
            Motion::Left | Motion::WordLeft => at_start && range.start > 0,
            Motion::Right | Motion::WordRight => at_end && range.end < self.lines.len(),
            Motion::PageUp | Motion::DocumentStart => range.start > 0,
            Motion::PageDown | Motion::DocumentEnd => range.end < self.lines.len(),
            Motion::Home | Motion::End => false,
        }
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
        let on = |line: usize, column: usize| Position {
            line,
            column: floor_char_boundary(&self.lines[line], column.min(self.lines[line].len())),
        };
        match motion {
            Motion::Up => on(cur.line.saturating_sub(1), cur.column),
            Motion::Down => on((cur.line + 1).min(last), cur.column),
            Motion::Left | Motion::WordLeft | Motion::Home => {
                if cur.line == 0 {
                    on(0, 0)
                } else {
                    on(cur.line - 1, self.lines[cur.line - 1].len())
                }
            }
            Motion::Right | Motion::WordRight | Motion::End => {
                if cur.line >= last {
                    on(last, self.lines[last].len())
                } else {
                    on(cur.line + 1, 0)
                }
            }
            Motion::PageUp | Motion::DocumentStart => on(0, 0),
            Motion::PageDown | Motion::DocumentEnd => on(last, self.lines[last].len()),
        }
    }

    pub(super) fn selection_anchor(&self) -> Position {
        let c = self.editor.cursor();
        let local = c.selection.unwrap_or(c.position);
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
        let lo = anchor.line.min(head.line);
        let hi = anchor.line.max(head.line);
        let start = self.segments[self.segment_at(lo)].lines.start;
        let end = self.segments[self.segment_at(hi)].lines.end;
        let local = |p: Position| Position {
            line: p.line - start,
            column: floor_char_boundary(
                &self.lines[p.line],
                p.column.min(self.lines[p.line].len()),
            ),
        };
        self.load_editor(
            start..end,
            Cursor {
                position: local(head),
                selection: Some(local(anchor)),
            },
        )
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
    pub(super) fn click_position(&self, i: usize) -> Position {
        let seg = &self.segments[i];
        let Range { start, end } = seg.lines.clone();
        let Some(p) = self.hover.filter(|(h, _)| *h == i).map(|(_, p)| p) else {
            return Position {
                line: start,
                column: self.lines[start].len(),
            };
        };

        let line = match seg.kind {
            Kind::Blank => {
                return Position {
                    line: start,
                    column: 0,
                };
            }
            Kind::Markdown => {
                let x = p.x - seg.indent as f32 * TEXT_SIZE * 0.5;
                return Position {
                    line: start,
                    column: estimate_column(&self.lines[start], x),
                };
            }
            Kind::Fence => {
                let row_h = TEXT_SIZE * 0.875 * 1.3;
                let k = ((p.y - TEXT_SIZE * 1.1).max(0.0) / row_h) as usize;
                (start + 1 + k).min(end.saturating_sub(2).max(start))
            }
            Kind::Table => {
                let k = (p.y / (LINE_HEIGHT + 10.0)) as usize;
                if k == 0 { start } else { start + 1 + k }
            }
            Kind::FrontMatter => start + (p.y / (TEXT_SIZE * 0.8 * 1.3)) as usize,
        };
        let line = line.min(end - 1);
        Position {
            line,
            column: self.lines[line].len(),
        }
    }
}
