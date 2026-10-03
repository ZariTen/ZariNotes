//! Syntax highlighting for raw Markdown inside a `text_editor`.
//!
//! Used for the line under the cursor in live preview and for the whole
//! document in source mode. Syntax markers are dimmed, emphasis is shown with
//! real bold/italic fonts, code is monospaced.

use std::ops::Range;

use iced::advanced::text::highlighter::{self, Format};
use iced::font::{Style, Weight};
use iced::{Color, Font, Theme};

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Use a monospace base font (source mode) instead of the proportional one.
    pub mono: bool,
}

type Spans = Vec<(Range<usize>, Kind)>;

struct Found {
    end: usize,
    spans: Spans,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Kind {
    Marker,
    Heading,
    Bold,
    Italic,
    Code,
    Link,
    Quote,
    Strike,
    CodeLine,
}

#[derive(Debug, Clone, Copy)]
pub struct Highlight {
    kind: Kind,
    mono: bool,
}

pub struct Highlighter {
    mono: bool,
    current: usize,
    /// `in_fence[i]` = whether line `i` starts inside a fenced code block.
    in_fence: Vec<bool>,
}

impl highlighter::Highlighter for Highlighter {
    type Settings = Settings;
    type Highlight = Highlight;
    type Iterator<'a> = std::vec::IntoIter<(Range<usize>, Highlight)>;

    fn new(settings: &Settings) -> Self {
        Self {
            mono: settings.mono,
            current: 0,
            in_fence: vec![false],
        }
    }

    fn update(&mut self, settings: &Settings) {
        *self = Self::new(settings);
    }

    fn change_line(&mut self, line: usize) {
        self.current = self.current.min(line);
        self.in_fence.truncate(self.current + 1);
    }

    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let fenced = self.in_fence.get(self.current).copied().unwrap_or(false);
        let (spans, fenced_after) = spans(line, fenced);

        self.in_fence.truncate(self.current + 1);
        self.in_fence.push(fenced_after);
        self.current += 1;

        let mono = self.mono;
        spans
            .into_iter()
            .map(|(r, kind)| (r, Highlight { kind, mono }))
            .collect::<Vec<_>>()
            .into_iter()
    }

    fn current_line(&self) -> usize {
        self.current
    }
}

pub fn to_format(highlight: &Highlight, theme: &Theme) -> Format<Font> {
    let base = if highlight.mono {
        Font::MONOSPACE
    } else {
        Font::DEFAULT
    };
    let (color, font) = look_of(highlight.kind, base, theme);
    Format { color, font }
}

fn look_of(kind: Kind, base: Font, theme: &Theme) -> (Option<Color>, Option<Font>) {
    let ink = theme.extended_palette().background.base.text;
    match kind {
        Kind::Marker => (Some(ink.scale_alpha(0.4)), None),
        Kind::Heading | Kind::Bold => (None, Some(bold_font(base))),
        Kind::Italic => (None, Some(italic_font(base))),
        Kind::Code | Kind::CodeLine => (Some(theme.palette().success), Some(Font::MONOSPACE)),
        Kind::Link => (Some(theme.palette().primary), None),
        Kind::Quote => (Some(ink.scale_alpha(0.75)), Some(italic_font(base))),
        Kind::Strike => (Some(ink.scale_alpha(0.55)), None),
    }
}

fn bold_font(base: Font) -> Font {
    Font {
        weight: Weight::Bold,
        ..base
    }
}

fn italic_font(base: Font) -> Font {
    Font {
        style: Style::Italic,
        ..base
    }
}

/// Compute highlight spans for one line. Returns the spans and whether the
/// following line starts inside a fenced code block.
pub fn spans(line: &str, fenced: bool) -> (Vec<(Range<usize>, Kind)>, bool) {
    // A closing fence is still a fence, so this runs before the inside-fence check.
    if let Some(done) = fence_line(line, fenced) {
        return done;
    }
    if fenced {
        return code_line(line);
    }
    if let Some(done) = horizontal_rule(line) {
        return done;
    }
    if let Some(done) = heading(line) {
        return done;
    }

    let mut out = Vec::new();
    let mut pos = indent_len(line);
    let fill = block_quote(line, &mut pos, &mut out);
    list_item(line, &mut pos, &mut out);
    inline(line, pos, fill, &mut out);
    (out, false)
}

fn fence_line(line: &str, fenced: bool) -> Option<(Spans, bool)> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        Some((vec![(0..line.len(), Kind::Marker)], !fenced))
    } else {
        None
    }
}

fn code_line(line: &str) -> (Spans, bool) {
    (vec![(0..line.len(), Kind::CodeLine)], true)
}

fn horizontal_rule(line: &str) -> Option<(Spans, bool)> {
    let compact: String = line
        .trim_start()
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    if compact.len() < 3 {
        return None;
    }
    let marker = compact.chars().next()?;
    if marker != '-' && marker != '*' && marker != '_' {
        return None;
    }
    if compact.chars().all(|c| c == marker) {
        Some((vec![(0..line.len(), Kind::Marker)], false))
    } else {
        None
    }
}

fn heading(line: &str) -> Option<(Spans, bool)> {
    let trimmed = line.trim_start();
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    let rest = &trimmed[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let marker_end = indent_len(line) + hashes + usize::from(!rest.is_empty());
    let mut out = Vec::new();
    out.push((0..marker_end, Kind::Marker));
    inline(line, marker_end, Some(Kind::Heading), &mut out);
    Some((out, false))
}

fn block_quote(line: &str, pos: &mut usize, out: &mut Spans) -> Option<Kind> {
    let mut fill = None;
    while line[*pos..].starts_with('>') {
        let end = *pos + 1 + usize::from(line[*pos + 1..].starts_with(' '));
        out.push((*pos..end, Kind::Marker));
        *pos = end;
        fill = Some(Kind::Quote);
    }
    fill
}

fn list_item(line: &str, pos: &mut usize, out: &mut Spans) {
    let Some(len) = list_marker(&line[*pos..]) else {
        return;
    };
    out.push((*pos..*pos + len, Kind::Marker));
    *pos += len;
    let Some(task_len) = task_box(&line[*pos..]) else {
        return;
    };
    out.push((*pos..*pos + task_len, Kind::Marker));
    *pos += task_len;
}

fn task_box(s: &str) -> Option<usize> {
    ["[ ] ", "[x] ", "[X] "]
        .into_iter()
        .find(|task| s.starts_with(task))
        .map(|task| task.len())
}

fn indent_len(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

/// Length of a list marker (`- `, `* `, `+ `, `12. `, `3) `) at the start of `s`.
pub fn list_marker(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() >= 2 && matches!(b[0], b'-' | b'*' | b'+') && b[1] == b' ' {
        return Some(2);
    }
    let digits = b.iter().take_while(|c| c.is_ascii_digit()).count();
    if (1..=9).contains(&digits)
        && b.len() > digits + 1
        && matches!(b[digits], b'.' | b')')
        && b[digits + 1] == b' '
    {
        return Some(digits + 2);
    }
    None
}

/// Highlight inline syntax in `line[start..]`. Gaps get `fill` (if any).
fn inline(line: &str, start: usize, fill: Option<Kind>, out: &mut Vec<(Range<usize>, Kind)>) {
    let b = line.as_bytes();
    let mut i = start;
    let mut gap = start;

    let flush = |out: &mut Vec<_>, from: usize, to: usize| {
        if let Some(k) = fill
            && from < to
        {
            out.push((from..to, k));
        }
    };

    while i < b.len() {
        if let Some(found) = first_syntax(line, i) {
            flush(out, gap, i);
            out.extend(found.spans);
            i = found.end;
            gap = found.end;
        } else {
            // One character, not one byte. A multibyte char must not be split.
            let step = line[i..].chars().next().map_or(1, char::len_utf8);
            i += step;
        }
    }
    flush(out, gap, b.len());
}

/// Order matters. `**` must be tried before `*`, and `[[` before `[`,
/// or the shorter marker would win.
fn first_syntax(line: &str, at: usize) -> Option<Found> {
    if let Some(found) = inline_code(line, at) {
        return Some(found);
    }
    if let Some(found) = bold(line, at) {
        return Some(found);
    }
    if let Some(found) = strike(line, at) {
        return Some(found);
    }
    if let Some(found) = double_equals(line, at) {
        return Some(found);
    }
    if let Some(found) = italic(line, at) {
        return Some(found);
    }
    if let Some(found) = wiki_link(line, at) {
        return Some(found);
    }
    markdown_link(line, at)
}

/// `` `code` `` with a closing tick on this line.
fn inline_code(line: &str, at: usize) -> Option<Found> {
    let after_open = line[at..].strip_prefix('`')?;
    let inner_len = after_open.find('`')?;
    let inner_start = at + 1;
    let inner_end = inner_start + inner_len;
    let end = inner_end + 1;
    Some(marked(at, inner_start..inner_end, end, Kind::Code))
}

/// `**bold**` or `__bold__`.
fn bold(line: &str, at: usize) -> Option<Found> {
    let rest = &line[at..];
    let delim = if rest.starts_with("**") {
        "**"
    } else if rest.starts_with("__") {
        "__"
    } else {
        return None;
    };
    delimited(line, at, delim, Kind::Bold)
}

/// `~~strike~~`.
fn strike(line: &str, at: usize) -> Option<Found> {
    if line[at..].starts_with("~~") {
        delimited(line, at, "~~", Kind::Strike)
    } else {
        None
    }
}

/// `==marked==`. This is painted as a link, same as before.
fn double_equals(line: &str, at: usize) -> Option<Found> {
    if line[at..].starts_with("==") {
        delimited(line, at, "==", Kind::Link)
    } else {
        None
    }
}

/// `*italic*` or `_italic_`. An underscore after a letter is a name, not emphasis.
fn italic(line: &str, at: usize) -> Option<Found> {
    let rest = &line[at..];
    let star = rest.starts_with('*');
    let underscore = rest.starts_with('_');
    if !star && !underscore {
        return None;
    }
    if underscore && at > 0 && line.as_bytes()[at - 1].is_ascii_alphanumeric() {
        return None;
    }
    delimited(line, at, &rest[..1], Kind::Italic)
}

/// `[[note]]`.
fn wiki_link(line: &str, at: usize) -> Option<Found> {
    let rest = &line[at..];
    if !rest.starts_with("[[") {
        return None;
    }
    let close = rest.find("]]")?;
    let inner_start = at + 2;
    let inner_end = at + close;
    let end = inner_end + 2;
    Some(marked(at, inner_start..inner_end, end, Kind::Link))
}

/// `[text](url)` or `![alt](url)`.
fn markdown_link(line: &str, at: usize) -> Option<Found> {
    let rest = &line[at..];
    let image = rest.starts_with("![");
    if !image && !rest.starts_with('[') {
        return None;
    }
    let open = at + usize::from(image);
    let text_end = open + line[open..].find("](")?;
    let end = text_end + line[text_end..].find(')')? + 1;
    Some(marked(at, open + 1..text_end, end, Kind::Link))
}

fn marked(start: usize, inner: Range<usize>, end: usize, kind: Kind) -> Found {
    Found {
        end,
        spans: vec![
            (start..inner.start, Kind::Marker),
            (inner.start..inner.end, kind),
            (inner.end..end, Kind::Marker),
        ],
    }
}

fn delimited(line: &str, i: usize, delim: &str, kind: Kind) -> Option<Found> {
    let d = delim.len();
    let inner_start = i + d;
    // Opening delimiter must be followed by non-space content.
    if line[inner_start..].starts_with(' ') || inner_start >= line.len() {
        return None;
    }
    let j = line[inner_start..].find(delim)?;
    if j == 0 {
        return None;
    }
    let inner_end = inner_start + j;
    Some(marked(i, inner_start..inner_end, inner_end + d, kind))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(line: &str) -> Vec<(&str, Kind)> {
        spans(line, false)
            .0
            .into_iter()
            .map(|(r, k)| (&line[r], k))
            .collect()
    }

    #[test]
    fn heading_and_bold() {
        assert_eq!(
            kinds("## Hi **there**"),
            [
                ("## ", Kind::Marker),
                ("Hi ", Kind::Heading),
                ("**", Kind::Marker),
                ("there", Kind::Bold),
                ("**", Kind::Marker),
            ]
        );
    }

    #[test]
    fn list_task_and_link() {
        assert_eq!(
            kinds("- [x] see [docs](http://x)"),
            [
                ("- ", Kind::Marker),
                ("[x] ", Kind::Marker),
                ("[", Kind::Marker),
                ("docs", Kind::Link),
                ("](http://x)", Kind::Marker),
            ]
        );
    }

    #[test]
    fn snake_case_is_not_italic() {
        assert!(kinds("my_var_name").is_empty());
    }

    #[test]
    fn fences_toggle() {
        assert!(spans("```rust", false).1);
        assert!(spans("let x = 1;", true).1);
        assert!(!spans("```", true).1);
    }
}
