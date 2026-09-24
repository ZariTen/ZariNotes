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

pub fn to_format(h: &Highlight, theme: &Theme) -> Format<Font> {
    let base = if h.mono {
        Font::MONOSPACE
    } else {
        Font::DEFAULT
    };
    let p = theme.extended_palette();
    let dim = p.background.base.text.scale_alpha(0.4);

    let (color, font): (Option<Color>, Option<Font>) = match h.kind {
        Kind::Marker => (Some(dim), None),
        Kind::Heading => (
            None,
            Some(Font {
                weight: Weight::Bold,
                ..base
            }),
        ),
        Kind::Bold => (
            None,
            Some(Font {
                weight: Weight::Bold,
                ..base
            }),
        ),
        Kind::Italic => (
            None,
            Some(Font {
                style: Style::Italic,
                ..base
            }),
        ),
        Kind::Code | Kind::CodeLine => (Some(theme.palette().success), Some(Font::MONOSPACE)),
        Kind::Link => (Some(theme.palette().primary), None),
        Kind::Quote => (
            Some(p.background.base.text.scale_alpha(0.75)),
            Some(Font {
                style: Style::Italic,
                ..base
            }),
        ),
        Kind::Strike => (Some(p.background.base.text.scale_alpha(0.55)), None),
    };
    Format { color, font }
}

/// Compute highlight spans for one line. Returns the spans and whether the
/// following line starts inside a fenced code block.
pub fn spans(line: &str, fenced: bool) -> (Vec<(Range<usize>, Kind)>, bool) {
    let mut out = Vec::new();
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();

    if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
        out.push((0..line.len(), Kind::Marker));
        return (out, !fenced);
    }
    if fenced {
        out.push((0..line.len(), Kind::CodeLine));
        return (out, true);
    }

    // Horizontal rule.
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() >= 3
        && (compact.chars().all(|c| c == '-')
            || compact.chars().all(|c| c == '*')
            || compact.chars().all(|c| c == '_'))
    {
        out.push((0..line.len(), Kind::Marker));
        return (out, false);
    }

    // Heading.
    let hashes = trimmed.bytes().take_while(|&b| b == b'#').count();
    if (1..=6).contains(&hashes) && trimmed[hashes..].is_empty()
        || (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ')
    {
        let marker_end = indent + hashes + usize::from(trimmed.len() > hashes);
        out.push((0..marker_end, Kind::Marker));
        inline(line, marker_end, Some(Kind::Heading), &mut out);
        return (out, false);
    }

    let mut pos = indent;
    let mut fill = None;

    // Block quotes (possibly nested).
    while line[pos..].starts_with('>') {
        let end = pos + 1 + usize::from(line[pos + 1..].starts_with(' '));
        out.push((pos..end, Kind::Marker));
        pos = end;
        fill = Some(Kind::Quote);
    }

    // List marker, optionally followed by a task box.
    if let Some(len) = list_marker(&line[pos..]) {
        out.push((pos..pos + len, Kind::Marker));
        pos += len;
        for task in ["[ ] ", "[x] ", "[X] "] {
            if line[pos..].starts_with(task) {
                out.push((pos..pos + task.len(), Kind::Marker));
                pos += task.len();
                break;
            }
        }
    }

    inline(line, pos, fill, &mut out);
    (out, false)
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
        let rest = &line[i..];
        let found: Option<(usize, Spans)> = if let Some(after) = rest.strip_prefix('`') {
            after.find('`').map(|j| {
                let end = i + 1 + j + 1;
                (
                    end,
                    vec![
                        (i..i + 1, Kind::Marker),
                        (i + 1..end - 1, Kind::Code),
                        (end - 1..end, Kind::Marker),
                    ],
                )
            })
        } else if rest.starts_with("**") || rest.starts_with("__") {
            delimited(line, i, &rest[..2], Kind::Bold)
        } else if rest.starts_with("~~") {
            delimited(line, i, "~~", Kind::Strike)
        } else if rest.starts_with("==") {
            delimited(line, i, "==", Kind::Link)
        } else if (rest.starts_with('*') || rest.starts_with('_'))
            && !(rest.starts_with('_') && i > 0 && b[i - 1].is_ascii_alphanumeric())
        {
            delimited(line, i, &rest[..1], Kind::Italic)
        } else if rest.starts_with("[[") {
            rest.find("]]").map(|j| {
                let end = i + j + 2;
                (
                    end,
                    vec![
                        (i..i + 2, Kind::Marker),
                        (i + 2..end - 2, Kind::Link),
                        (end - 2..end, Kind::Marker),
                    ],
                )
            })
        } else if rest.starts_with('[') || rest.starts_with("![") {
            let open = i + usize::from(rest.starts_with('!'));
            line[open..].find("](").and_then(|j| {
                let close_text = open + j;
                line[close_text..].find(')').map(|k| {
                    let end = close_text + k + 1;
                    (
                        end,
                        vec![
                            (i..open + 1, Kind::Marker),
                            (open + 1..close_text, Kind::Link),
                            (close_text..end, Kind::Marker),
                        ],
                    )
                })
            })
        } else {
            None
        };

        match found {
            Some((end, spans)) => {
                flush(out, gap, i);
                out.extend(spans);
                i = end;
                gap = end;
            }
            None => i += rest.chars().next().map_or(1, char::len_utf8),
        }
    }
    flush(out, gap, b.len());
}

fn delimited(line: &str, i: usize, delim: &str, kind: Kind) -> Option<(usize, Spans)> {
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
    Some((
        inner_end + d,
        vec![
            (i..inner_start, Kind::Marker),
            (inner_start..inner_end, kind),
            (inner_end..inner_end + d, Kind::Marker),
        ],
    ))
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
