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

use std::collections::HashMap;
use std::ops::Range;

use iced::advanced::widget::{self as advanced_widget, Id, Operation, operation};
use iced::font::Weight;
use iced::keyboard::{Key, key::Named};
use iced::widget::scrollable::AbsoluteOffset;
use iced::widget::text_editor::{Action, Binding, Cursor, KeyPress, Position};
use iced::widget::{
    checkbox, column, container, markdown, mouse_area, row, scrollable, space, text, text_editor,
};
use iced::{
    Border, Color, Element, Fill, Font, Padding, Point, Rectangle, Task, Theme, Vector, mouse,
};

use crate::highlight;

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
    editor: iced::widget::text_editor::Content,
    hover: Option<(usize, Point)>,
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
            editor: iced::widget::text_editor::Content::new(),
            hover: None,
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
            line: self.segments[self.active].lines.start + c.line,
            column: c.column,
        }
    }

    pub fn focus(&self) -> Task<Msg> {
        Task::batch([
            iced::widget::operation::focus(Id::new(EDITOR_ID)),
            self.scroll_into_view(),
        ])
    }

    pub fn update(&mut self, msg: Msg) -> (Task<Msg>, Outcome) {
        match msg {
            Msg::Edit(action) => {
                let is_edit = action.is_edit();
                self.editor.perform(action);
                if !is_edit {
                    return (self.scroll_into_view(), Outcome::None);
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
                (self.set_cursor(pos), Outcome::None)
            }
            Msg::Hover(i, p) => {
                self.hover = Some((i, p));
                (Task::none(), Outcome::None)
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
        }
    }

    pub fn view<'a>(&'a self, theme: &Theme) -> Element<'a, Msg> {
        let mut settings = markdown::Settings::with_text_size(TEXT_SIZE, theme);
        settings.code_size = (TEXT_SIZE * 0.875).into();
        settings.spacing = (TEXT_SIZE * 0.5).into();

        let blocks = self.segments.iter().enumerate().map(|(i, seg)| {
            if i == self.active {
                self.editor_view()
            } else {
                self.rendered(i, seg, settings)
            }
        });

        scrollable(
            container(column(blocks).max_width(820).padding([24, 32]))
                .center_x(Fill)
                .padding(Padding::ZERO.bottom(200)),
        )
        .id(Id::new(SCROLL_ID))
        .height(Fill)
        .into()
    }

    // ── views ────────────────────────────────────────────────────

    fn editor_view(&self) -> Element<'_, Msg> {
        let c = self.editor.cursor();
        let last = self.editor.line_count().saturating_sub(1);
        let line_text = self
            .editor
            .line(c.position.line)
            .map(|l| l.text.into_owned())
            .unwrap_or_default();
        let no_sel = c.selection.is_none();
        let at_top = c.position.line == 0;
        let at_bottom = c.position.line == last;
        let at_start = no_sel && at_top && c.position.column == 0;
        let at_end = no_sel && at_bottom && c.position.column >= line_text.len();
        let at_line_end = c.position.column >= line_text.len();

        text_editor(&self.editor)
            .id(Id::new(EDITOR_ID))
            .on_action(Msg::Edit)
            .size(TEXT_SIZE)
            .padding(0)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings { mono: false },
                highlight::to_format,
            )
            .key_binding(move |kp: KeyPress| {
                let m = kp.modifiers;
                if m.command() {
                    match kp.key.as_ref() {
                        Key::Character("s") => return Some(Binding::Custom(Msg::Save)),
                        Key::Character("e") => return Some(Binding::Custom(Msg::ToggleMode)),
                        _ => {}
                    }
                }
                let plain = !m.shift() && !m.command() && !m.alt();
                match kp.key.as_ref() {
                    Key::Named(Named::ArrowUp) if plain && at_top => {
                        Some(Binding::Custom(Msg::Nav(Nav::Up)))
                    }
                    Key::Named(Named::ArrowDown) if plain && at_bottom => {
                        Some(Binding::Custom(Msg::Nav(Nav::Down)))
                    }
                    Key::Named(Named::ArrowLeft) if plain && at_start => {
                        Some(Binding::Custom(Msg::Nav(Nav::Left)))
                    }
                    Key::Named(Named::ArrowRight) if plain && at_end => {
                        Some(Binding::Custom(Msg::Nav(Nav::Right)))
                    }
                    Key::Named(Named::Backspace) if at_start => Some(Binding::Custom(Msg::MergeUp)),
                    Key::Named(Named::Delete) if at_end => Some(Binding::Custom(Msg::MergeDown)),
                    Key::Named(Named::Tab) if m.is_empty() => {
                        Some(Binding::Sequence(vec![Binding::Insert(' '); 4]))
                    }
                    Key::Named(Named::Enter) if plain && no_sel && at_line_end => {
                        continue_list(&line_text).or_else(|| Binding::from_key_press(kp))
                    }
                    _ => Binding::from_key_press(kp),
                }
            })
            .style(|theme, status| iced::widget::text_editor::Style {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                ..iced::widget::text_editor::default(theme, status)
            })
            .into()
    }

    fn rendered<'a>(
        &'a self,
        i: usize,
        seg: &'a Segment,
        settings: markdown::Settings,
    ) -> Element<'a, Msg> {
        let dim = |t: &Theme| text::Style {
            color: Some(t.extended_palette().background.base.text.scale_alpha(0.5)),
        };

        let body: Element<'a, Msg> = match seg.kind {
            Kind::Blank => space().height(LINE_HEIGHT).into(),
            Kind::FrontMatter => container(column(self.lines[seg.lines.clone()].iter().map(|l| {
                text(l.as_str())
                    .font(Font::MONOSPACE)
                    .size(TEXT_SIZE * 0.8)
                    .style(dim)
                    .into()
            })))
            .padding(8)
            .width(Fill)
            .style(container::rounded_box)
            .into(),
            _ if seg.items.is_empty() => {
                text(seg.source.as_str()).size(TEXT_SIZE).style(dim).into()
            }
            _ => markdown::view_with(
                &seg.items,
                settings,
                &Viewer {
                    line: seg.lines.start,
                },
            ),
        };

        let indent = seg.indent as f32 * TEXT_SIZE * 0.5;
        mouse_area(
            container(body)
                .width(Fill)
                .padding(Padding::ZERO.left(indent)),
        )
        .on_press(Msg::Activate(i))
        .on_move(move |p| Msg::Hover(i, p))
        .interaction(mouse::Interaction::Text)
        .into()
    }

    // ── model ────────────────────────────────────────────────────

    /// Re-split `lines` into segments, reusing parsed Markdown where the source is unchanged.
    fn rebuild(&mut self) {
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

    fn segment_at(&self, line: usize) -> usize {
        self.segments
            .partition_point(|s| s.lines.end <= line)
            .min(self.segments.len() - 1)
    }

    /// Put the cursor at a document position, loading its segment into the editor.
    fn set_cursor(&mut self, pos: Position) -> Task<Msg> {
        let line = pos.line.min(self.lines.len() - 1);
        self.active = self.segment_at(line);
        let range = self.segments[self.active].lines.clone();
        self.editor =
            iced::widget::text_editor::Content::with_text(&self.lines[range.clone()].join("\n"));
        self.editor.move_to(Cursor {
            position: Position {
                line: line - range.start,
                column: floor_char_boundary(&self.lines[line], pos.column),
            },
            selection: None,
        });
        self.focus()
    }

    /// Write the editor's text back into `lines` and re-segment around the cursor.
    fn sync_editor(&mut self) -> Task<Msg> {
        let old = self.segments[self.active].lines.clone();
        let cursor = self.cursor();
        let new: Vec<String> = self.editor.text().split('\n').map(str::to_owned).collect();
        let new_len = new.len();
        self.lines.splice(old.clone(), new);
        self.rebuild();

        let idx = self.segment_at(cursor.line);
        let range = &self.segments[idx].lines;
        if range.start == old.start && range.len() == new_len {
            // Same segment, editor already holds exactly its text.
            self.active = idx;
            self.scroll_into_view()
        } else {
            self.set_cursor(cursor)
        }
    }

    fn navigate(&mut self, nav: Nav) -> Task<Msg> {
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

    /// Best-effort mapping from a click on rendered Markdown to a source position.
    fn click_position(&self, i: usize) -> Position {
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

    fn scroll_into_view(&self) -> Task<Msg> {
        let cursor_offset = self.editor.cursor().position.line as f32 * LINE_HEIGHT;
        advanced_widget::operate(ScrollIntoView {
            cursor_offset,
            editor: None,
            scroll: None,
        })
        .then(|y| {
            iced::widget::operation::scroll_to(
                Id::new(SCROLL_ID),
                AbsoluteOffset {
                    x: None,
                    y: Some(y),
                },
            )
        })
    }
}

/// Split lines into segments.
fn split(lines: &[String]) -> Vec<(Range<usize>, Kind)> {
    let n = lines.len();
    let mut out = Vec::new();
    let mut i = 0;

    // YAML front matter.
    if lines.first().is_some_and(|l| l.trim_end() == "---")
        && let Some(j) = (1..n).find(|&j| matches!(lines[j].trim_end(), "---" | "..."))
    {
        out.push((0..j + 1, Kind::FrontMatter));
        i = j + 1;
    }

    while i < n {
        let t = lines[i].trim_start();
        if t.starts_with("```") || t.starts_with("~~~") {
            let fence = &t[..3];
            let close = (i + 1..n).find(|&j| lines[j].trim_start().starts_with(fence));
            let end = close.map_or(n, |j| j + 1);
            out.push((i..end, Kind::Fence));
            i = end;
        } else if t.starts_with('|') {
            let end = (i..n)
                .find(|&j| !lines[j].trim_start().starts_with('|'))
                .unwrap_or(n);
            out.push((i..end, Kind::Table));
            i = end;
        } else {
            let kind = if t.is_empty() {
                Kind::Blank
            } else {
                Kind::Markdown
            };
            out.push((i..i + 1, kind));
            i += 1;
        }
    }
    out
}

/// On Enter at the end of a list item, continue the list (or end it if the item is empty).
fn continue_list(line: &str) -> Option<Binding<Msg>> {
    let indent_len = line.len() - line.trim_start().len();
    let rest = &line[indent_len..];
    let marker_len = highlight::list_marker(rest)?;
    let mut marker = rest[..marker_len].to_owned();
    let mut body = &rest[marker_len..];
    for task in ["[ ] ", "[x] ", "[X] "] {
        if let Some(b) = body.strip_prefix(task) {
            body = b;
            marker.push_str("[ ] ");
            break;
        }
    }

    if body.trim().is_empty() {
        // Empty item: remove the marker instead of adding another one.
        return Some(Binding::Sequence(vec![
            Binding::Select(iced::widget::text_editor::Motion::Home),
            Binding::Backspace,
        ]));
    }

    // Increment ordered-list numbers.
    let digits = marker.bytes().take_while(u8::is_ascii_digit).count();
    if digits > 0
        && let Ok(n) = marker[..digits].parse::<u64>()
    {
        marker = format!("{}{}", n + 1, &marker[digits..]);
    }

    let mut seq = vec![Binding::Enter];
    seq.extend(line[..indent_len].chars().map(Binding::Insert));
    seq.extend(marker.chars().map(Binding::Insert));
    Some(Binding::Sequence(seq))
}

/// Estimate which source column a click at `x` pixels into a rendered line maps to.
fn estimate_column(raw: &str, x: f32) -> usize {
    let ws = raw.len() - raw.trim_start().len();
    let t = &raw[ws..];

    // (prefix bytes hidden by rendering, pixels the renderer adds in front, font scale)
    let hashes = t.bytes().take_while(|&b| b == b'#').count();
    let (prefix, offset, scale) = if (1..=6).contains(&hashes) && t[hashes..].starts_with(' ') {
        let scale = [2.0, 1.75, 1.5, 1.25, 1.0, 1.0][hashes - 1];
        (hashes + 1, 0.0, scale)
    } else if let Some(m) = highlight::list_marker(t) {
        let task = ["[ ] ", "[x] ", "[X] "]
            .iter()
            .any(|p| t[m..].starts_with(p));
        let ordered = t.as_bytes()[0].is_ascii_digit();
        let offset = if ordered {
            46.0
        } else if task {
            44.0
        } else {
            38.0
        };
        (m + if task { 4 } else { 0 }, offset, 1.0)
    } else if t.starts_with("> ") {
        (2, 20.0, 1.0)
    } else {
        (0, 0.0, 1.0)
    };

    let char_w = TEXT_SIZE * scale * 0.5;
    let target = ((x - offset).max(0.0) / char_w).round() as usize;

    let body = &t[prefix..];
    let base = ws + prefix;
    let bytes = body.as_bytes();
    let mut visible = 0;
    let mut in_url = false;
    for (i, ch) in body.char_indices() {
        if in_url {
            in_url = ch != ')';
            continue;
        }
        let hidden = match ch {
            '*' | '`' | '~' | '[' | '\\' => true,
            ']' => {
                in_url = bytes.get(i + 1) == Some(&b'(');
                true
            }
            '_' => {
                let prev = i.checked_sub(1).map(|j| bytes[j]);
                let next = bytes.get(i + 1).copied();
                !(prev.is_some_and(|b| b.is_ascii_alphanumeric())
                    && next.is_some_and(|b| b.is_ascii_alphanumeric()))
            }
            _ => false,
        };
        if hidden {
            continue;
        }
        if visible >= target {
            return base + i;
        }
        visible += 1;
    }
    raw.len()
}

fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

// ── Markdown viewer ───────────────────────────────────────────────

/// Renders Markdown with bold headings and clickable task checkboxes.
struct Viewer {
    /// Document line this segment starts at (for toggling tasks).
    line: usize,
}

impl<'a> markdown::Viewer<'a, Msg> for Viewer {
    fn on_link_click(url: markdown::Uri) -> Msg {
        Msg::Link(url)
    }

    fn heading(
        &self,
        mut settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        text: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Msg> {
        settings.style.font = Font {
            weight: Weight::Bold,
            ..settings.style.font
        };
        markdown::heading(settings, level, text, index, Self::on_link_click)
    }

    fn unordered_list(
        &self,
        settings: markdown::Settings,
        bullets: &'a [markdown::Bullet],
    ) -> Element<'a, Msg> {
        let line = self.line;
        column(bullets.iter().map(|bullet| {
            let (marker, items): (Element<'a, Msg>, _) = match bullet {
                markdown::Bullet::Point { items } => {
                    (text("•").size(settings.text_size).into(), items)
                }
                markdown::Bullet::Task { items, done } => (
                    container(
                        checkbox(*done)
                            .size(settings.text_size)
                            .on_toggle(move |_| Msg::ToggleTask(line)),
                    )
                    .center_y(
                        iced::widget::text::LineHeight::default().to_absolute(settings.text_size),
                    )
                    .into(),
                    items,
                ),
            };
            row![marker, markdown::view_with(items, settings, self)]
                .spacing(settings.spacing)
                .into()
        }))
        .spacing(settings.spacing * 0.75)
        .padding([0.0, settings.spacing.0])
        .into()
    }
}

// ── scrolling ─────────────────────────────────────────────────────

/// Finds the live editor and the document scrollable, and computes the
/// scroll offset needed to bring the cursor line into view (if any).
struct ScrollIntoView {
    cursor_offset: f32,
    editor: Option<Rectangle>,
    scroll: Option<(Rectangle, Rectangle, Vector)>,
}

impl Operation<f32> for ScrollIntoView {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<f32>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        if id == Some(&Id::new(SCROLL_ID)) {
            self.scroll = Some((bounds, content_bounds, translation));
        }
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        if id == Some(&Id::new(EDITOR_ID)) {
            self.editor = Some(bounds);
        }
    }

    fn finish(&self) -> operation::Outcome<f32> {
        let (Some(editor), Some((viewport, content, translation))) = (self.editor, self.scroll)
        else {
            return operation::Outcome::None;
        };
        let top = editor.y - content.y + self.cursor_offset;
        let bottom = top + LINE_HEIGHT;
        let margin = LINE_HEIGHT * 2.0;

        if top - margin < translation.y {
            operation::Outcome::Some((top - margin).max(0.0))
        } else if bottom + margin > translation.y + viewport.height {
            operation::Outcome::Some(bottom + margin - viewport.height)
        } else {
            operation::Outcome::None
        }
    }
}

#[cfg(test)]
mod tests {
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
    fn click_column_skips_markup() {
        // "a **bold** word": visible "a bold word"; 3rd visible char ('o' of bold)
        let raw = "a **bold** word";
        let col = estimate_column(raw, TEXT_SIZE * 0.5 * 3.0);
        assert_eq!(&raw[col..col + 1], "o");
    }
}
