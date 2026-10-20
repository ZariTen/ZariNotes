//! Turn `[[links]]` into Markdown links the live preview can click.
//!
//! The preview parser does not know wikilinks. A link to `zari-wiki:Note`
//! round-trips through the viewer and is resolved on click. Code spans and
//! fenced blocks are left alone — a wikilink in a sample is not a link.

use super::wiki::{
    WIKI_SCHEME, Wiki, is_asset_embed, percent_encode, split_alias, split_heading, take_wikilink,
    wiki_target_blank,
};
use crate::highlight::is_fence;

/// Rewrite `[[links]]` into Markdown links the live preview can click.
pub fn rewrite_wikilinks(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_fence = false;
    for (i, line) in source.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        in_fence = rewrite_outside_fence(line, in_fence, &mut out);
    }
    out
}

/// Copy a fenced line as-is. Otherwise rewrite wikilinks, and report whether
/// the next line is inside a fence.
fn rewrite_outside_fence(line: &str, in_fence: bool, out: &mut String) -> bool {
    if is_fence(line) {
        out.push_str(line);
        return !in_fence;
    }
    if in_fence {
        out.push_str(line);
        return true;
    }
    rewrite_line(line, out);
    false
}

fn rewrite_line(line: &str, out: &mut String) {
    let mut rest = line;
    let mut inline_code = false;
    while !rest.is_empty() {
        if rest.starts_with('\\') {
            // A backslash hides the next character, including a `[` that would
            // otherwise start a wikilink. Copy both through unchanged.
            out.push('\\');
            rest = &rest[1..];
            if let Some(ch) = rest.chars().next() {
                out.push(ch);
                rest = &rest[ch.len_utf8()..];
            }
            continue;
        }
        if rest.starts_with('`') {
            inline_code = !inline_code;
            out.push('`');
            rest = &rest[1..];
            continue;
        }
        if !inline_code && let Some((link, after)) = take_wikilink(rest) {
            if is_asset_embed(&link) || wiki_target_blank(&link) {
                let taken = rest.len() - after.len();
                out.push_str(&rest[..taken]);
            } else {
                push_rewritten(out, &link);
            }
            rest = after;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        rest = &rest[ch.len_utf8()..];
    }
}

fn push_rewritten(out: &mut String, link: &Wiki<'_>) {
    let (target, alias) = split_alias(link.inner);
    let label = if alias.is_empty() {
        display_label(target)
    } else {
        alias.to_owned()
    };
    let label = if link.embed {
        format!("↗ {label}")
    } else {
        label
    };
    let url = format!("{WIKI_SCHEME}{}", percent_encode(link.inner));
    out.push('[');
    push_label(out, &label);
    out.push_str("](");
    out.push_str(&url);
    out.push(')');
}

/// `[[folder/Note|shown]]` displays `shown`. Without an alias, show the file name.
fn display_label(target: &str) -> String {
    let (path, heading) = split_heading(target);
    let name = path
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(path.as_str())
        .trim();
    let name = name.strip_suffix(".md").unwrap_or(name);
    if name.is_empty() {
        return heading.unwrap_or_else(|| target.to_owned());
    }
    match heading {
        Some(heading) => format!("{name} › {heading}"),
        None => name.to_owned(),
    }
}

fn push_label(out: &mut String, label: &str) {
    for ch in label.chars() {
        if matches!(ch, '[' | ']' | '(' | ')') {
            out.push('\\');
        }
        out.push(ch);
    }
}
