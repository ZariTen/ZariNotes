//! Segment boundaries and click-to-column estimates.

use std::ops::Range;

use iced::widget::text_editor::{Binding, Motion};

use super::{Kind, Msg, TEXT_SIZE};
use crate::highlight;

/// Split lines into segments.
///
/// Front matter is only recognized at the start of the file. After that, a
/// fence is tried before a table, and a table before a single line.
pub(super) fn split(lines: &[String]) -> Vec<(Range<usize>, Kind)> {
    let mut out = Vec::new();
    let mut i = 0;
    if let Some(end) = front_matter_end(lines) {
        out.push((0..end, Kind::FrontMatter));
        i = end;
    }
    while i < lines.len() {
        if let Some(end) = fence_end(lines, i) {
            out.push((i..end, Kind::Fence));
            i = end;
            continue;
        }
        if let Some(end) = table_end(lines, i) {
            out.push((i..end, Kind::Table));
            i = end;
            continue;
        }
        out.push((i..i + 1, line_kind(&lines[i])));
        i += 1;
    }
    out
}

fn front_matter_end(lines: &[String]) -> Option<usize> {
    let first = lines.first()?;
    if first.trim_end() != "---" {
        return None;
    }
    let close = (1..lines.len()).find(|&j| highlight::is_front_matter_mark(&lines[j]))?;
    Some(close + 1)
}

fn fence_end(lines: &[String], i: usize) -> Option<usize> {
    let fence = fence_marker(&lines[i])?;
    let close = (i + 1..lines.len()).find(|&j| lines[j].trim_start().starts_with(fence));
    Some(close.map_or(lines.len(), |j| j + 1))
}

fn fence_marker(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    highlight::is_fence(trimmed).then(|| &trimmed[..3])
}

fn table_end(lines: &[String], i: usize) -> Option<usize> {
    if !lines[i].trim_start().starts_with('|') {
        return None;
    }
    let end = (i..lines.len())
        .find(|&j| !lines[j].trim_start().starts_with('|'))
        .unwrap_or(lines.len());
    Some(end)
}

fn line_kind(line: &str) -> Kind {
    if line.trim_start().is_empty() {
        Kind::Blank
    } else {
        Kind::Markdown
    }
}

/// On Enter at the end of a list item, continue the list (or end it if the item is empty).
pub(super) fn continue_list(line: &str) -> Option<Binding<Msg>> {
    let item = read_list_line(line)?;
    if item.body.trim().is_empty() {
        return Some(clear_list_marker());
    }
    let marker = next_ordered_marker(&item.marker);
    Some(insert_next_item(item.indent, &marker))
}

struct ListLine<'a> {
    indent: &'a str,
    marker: String,
    body: &'a str,
}

fn read_list_line(line: &str) -> Option<ListLine<'_>> {
    let indent_len = line.len() - line.trim_start().len();
    let rest = &line[indent_len..];
    let marker_len = highlight::list_marker(rest)?;
    let mut marker = rest[..marker_len].to_owned();
    let mut body = &rest[marker_len..];
    if let Some(task) = highlight::task_box(body) {
        body = &body[task..];
        marker.push_str("[ ] ");
    }
    Some(ListLine {
        indent: &line[..indent_len],
        marker,
        body,
    })
}

fn clear_list_marker() -> Binding<Msg> {
    Binding::Sequence(vec![Binding::Select(Motion::Home), Binding::Backspace])
}

fn next_ordered_marker(marker: &str) -> String {
    let digits = marker.bytes().take_while(u8::is_ascii_digit).count();
    if digits == 0 {
        return marker.to_owned();
    }
    let Ok(n) = marker[..digits].parse::<u64>() else {
        return marker.to_owned();
    };
    format!("{}{}", n + 1, &marker[digits..])
}

fn insert_next_item(indent: &str, marker: &str) -> Binding<Msg> {
    let mut seq = vec![Binding::Enter];
    seq.extend(indent.chars().map(Binding::Insert));
    seq.extend(marker.chars().map(Binding::Insert));
    Binding::Sequence(seq)
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
    let indent = raw.len() - raw.trim_start().len();
    let text = &raw[indent..];
    let prefix = drawn_prefix(text);
    let char_w = TEXT_SIZE * prefix.scale * 0.5;
    let target = ((x - prefix.extra_pixels).max(0.0) / char_w).round() as usize;
    let body = &text[prefix.hidden_bytes..];
    column_of_visible(body, target, indent + prefix.hidden_bytes, raw.len())
}

struct DrawnPrefix {
    hidden_bytes: usize,
    extra_pixels: f32,
    scale: f32,
}

/// Heading, then list, then quote. A heading marker is not a list marker.
fn drawn_prefix(text: &str) -> DrawnPrefix {
    heading_prefix(text)
        .or_else(|| list_prefix(text))
        .or_else(|| quote_prefix(text))
        .unwrap_or(DrawnPrefix {
            hidden_bytes: 0,
            extra_pixels: 0.0,
            scale: 1.0,
        })
}

fn heading_prefix(text: &str) -> Option<DrawnPrefix> {
    let hashes = text.bytes().take_while(|&b| b == b'#').count();
    if !(1..=6).contains(&hashes) {
        return None;
    }
    if !text[hashes..].starts_with(' ') {
        return None;
    }
    let scale = [2.0, 1.75, 1.5, 1.25, 1.0, 1.0][hashes - 1];
    Some(DrawnPrefix {
        hidden_bytes: hashes + 1,
        extra_pixels: 0.0,
        scale,
    })
}

fn list_prefix(text: &str) -> Option<DrawnPrefix> {
    let marker = highlight::list_marker(text)?;
    let task = highlight::task_box(&text[marker..]).is_some();
    let ordered = text.as_bytes()[0].is_ascii_digit();
    // An ordered item that is also a task still uses the ordered bullet width.
    let extra_pixels = if ordered {
        46.0
    } else if task {
        44.0
    } else {
        38.0
    };
    Some(DrawnPrefix {
        hidden_bytes: marker + if task { 4 } else { 0 },
        extra_pixels,
        scale: 1.0,
    })
}

fn quote_prefix(text: &str) -> Option<DrawnPrefix> {
    if !text.starts_with("> ") {
        return None;
    }
    Some(DrawnPrefix {
        hidden_bytes: 2,
        extra_pixels: 20.0,
        scale: 1.0,
    })
}

fn column_of_visible(body: &str, target: usize, base: usize, raw_len: usize) -> usize {
    let bytes = body.as_bytes();
    let mut visible = 0;
    let mut in_url = false;
    for (i, ch) in body.char_indices() {
        if in_url {
            in_url = ch != ')';
            continue;
        }
        match shown_char(ch, bytes, i) {
            Shown::Skip => continue,
            Shown::SkipUrl => {
                in_url = true;
                continue;
            }
            Shown::Count => {}
        }
        if visible >= target {
            return base + i;
        }
        visible += 1;
    }
    raw_len
}

enum Shown {
    Count,
    Skip,
    SkipUrl,
}

fn shown_char(ch: char, bytes: &[u8], index: usize) -> Shown {
    match ch {
        '*' | '`' | '~' | '[' | '\\' => Shown::Skip,
        ']' if bytes.get(index + 1) == Some(&b'(') => Shown::SkipUrl,
        ']' => Shown::Skip,
        '_' if underscore_is_marker(bytes, index) => Shown::Skip,
        _ => Shown::Count,
    }
}

/// An underscore between letters is part of a name, not emphasis.
fn underscore_is_marker(bytes: &[u8], index: usize) -> bool {
    let prev = index.checked_sub(1).map(|j| bytes[j]);
    let next = bytes.get(index + 1).copied();
    let in_name = prev.is_some_and(|b| b.is_ascii_alphanumeric())
        && next.is_some_and(|b| b.is_ascii_alphanumeric());
    !in_name
}

pub(super) fn floor_char_boundary(s: &str, i: usize) -> usize {
    let mut i = i.min(s.len());
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
