//! Segment boundaries and click-to-column estimates.

use std::ops::Range;

use iced::widget::text_editor::Binding;

use super::{Kind, Msg, TEXT_SIZE};
use crate::highlight;

/// Split lines into segments.
pub(super) fn split(lines: &[String]) -> Vec<(Range<usize>, Kind)> {
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
pub(super) fn continue_list(line: &str) -> Option<Binding<Msg>> {
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

/// Estimate which source column a click at `x` pixels into raw editor text maps to.
pub(super) fn estimate_raw_column(raw: &str, x: f32) -> usize {
    let char_w = TEXT_SIZE * 0.5;
    let target = (x.max(0.0) / char_w).round() as usize;
    raw.char_indices()
        .nth(target)
        .map(|(i, _)| i)
        .unwrap_or(raw.len())
}

/// Estimate which source column a click at `x` pixels into a rendered line maps to.
pub(super) fn estimate_column(raw: &str, x: f32) -> usize {
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

pub(super) fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
