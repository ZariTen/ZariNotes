//! Wikilink and Markdown-note syntax, shared by the index and the live preview.
//!
//! `[[note]]`, `[[note|alias]]`, `[[note#heading]]`, and `![[note]]` are one
//! syntax. A Markdown link counts as a note when its destination has no scheme
//! and no extension, or a `.md` extension. Code spans, fences, and front matter
//! are not links.

use std::path::{Component, Path, PathBuf};

use crate::highlight::{is_fence, is_front_matter_mark};

pub(super) const WIKI_SCHEME: &str = "zari-wiki:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinkKind {
    Wiki,
    Markdown,
}

/// One outgoing link, as written in a note.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mention {
    /// Path or note name inside the brackets or Markdown destination, without a
    /// heading. Empty when the link is only a heading in the same note.
    pub target: String,
    /// Display text after `|` in a wikilink. Markdown links leave this empty;
    /// their label is the link text, which is not what we match on.
    pub alias: Option<String>,
    /// Heading after `#`, so `[[Note#Heading]]` can be told apart from a link
    /// to the note itself. Not lowercased; matching headings is a later step.
    pub heading: Option<String>,
    /// `![[note]]` rather than `[[note]]`.
    pub embed: bool,
    /// Line the link was written on, for the excerpt in the backlink panel.
    pub line: usize,
    /// The source line, trimmed, captured when the note was indexed.
    pub excerpt: String,
    /// Where the link points, if the vault had exactly one match when indexed.
    /// Wikilinks match a unique name anywhere. Markdown links are relative to
    /// the note that contains them.
    pub dest: Option<PathBuf>,
    pub(super) kind: LinkKind,
}

pub(super) struct Wiki<'a> {
    pub(super) embed: bool,
    pub(super) inner: &'a str,
}

/// Every wikilink and every Markdown link to a note, outside code.
pub(super) fn parse_mentions(text: &str) -> Vec<Mention> {
    let mut mentions = Vec::new();
    let mut in_fence = false;
    // YAML between the opening `---` and its close is not prose. A `[[link]]`
    // there is a value, not a note reference.
    let mut in_front_matter = false;
    for (line_no, line) in text.lines().enumerate() {
        if line_no == 0 && is_front_matter_mark(line) {
            in_front_matter = true;
            continue;
        }
        if in_front_matter {
            if is_front_matter_mark(line) {
                in_front_matter = false;
            }
            continue;
        }
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        mentions.extend(mentions_in_line(line, line_no));
    }
    mentions
}

fn mentions_in_line(line: &str, line_no: usize) -> Vec<Mention> {
    let excerpt = clip(line.trim(), 96);
    let mut mentions = Vec::new();
    let mut rest = line;
    let mut inline_code = false;
    while !rest.is_empty() {
        if rest.starts_with('\\') {
            rest = skip_escaped(rest);
            continue;
        }
        if rest.starts_with('`') {
            inline_code = !inline_code;
            rest = &rest[1..];
            continue;
        }
        if inline_code {
            let ch = rest.chars().next().unwrap();
            rest = &rest[ch.len_utf8()..];
            continue;
        }
        if let Some((taken, after)) = take_wikilink(rest) {
            if let Some(mention) = mention_from_wiki(&taken, line_no, &excerpt) {
                mentions.push(mention);
            }
            rest = after;
            continue;
        }
        if let Some((mention, after)) = take_markdown_link(rest, line_no, &excerpt) {
            mentions.push(mention);
            rest = after;
            continue;
        }
        let ch = rest.chars().next().unwrap();
        rest = &rest[ch.len_utf8()..];
    }
    mentions
}

fn mention_from_wiki(link: &Wiki<'_>, line: usize, excerpt: &str) -> Option<Mention> {
    if is_asset_embed(link) {
        return None;
    }
    let (target, alias) = split_alias(link.inner);
    let (path, heading) = split_heading(target);
    if path.is_empty() && heading.is_none() {
        return None;
    }
    Some(Mention {
        target: path,
        alias: nonempty(alias),
        heading,
        embed: link.embed,
        line,
        excerpt: excerpt.to_owned(),
        dest: None,
        kind: LinkKind::Wiki,
    })
}

/// `![[pic.png]]` is an image, not a note. Leave it for the image renderer.
pub(super) fn is_asset_embed(link: &Wiki<'_>) -> bool {
    link.embed && looks_like_asset(split_alias(link.inner).0)
}

/// `[[]]` and `[[|alias]]` name no note. Leave them as typed.
pub(super) fn wiki_target_blank(link: &Wiki<'_>) -> bool {
    let (target, _) = split_alias(link.inner);
    let (path, heading) = split_heading(target);
    path.is_empty() && heading.is_none()
}

/// `[label](destination)` when `destination` looks like another note.
///
/// External URLs and image files are not notes. A destination with no scheme
/// and no extension, or a `.md` extension, is.
fn take_markdown_link<'a>(text: &'a str, line: usize, excerpt: &str) -> Option<(Mention, &'a str)> {
    if text.starts_with('!') || !text.starts_with('[') {
        return None;
    }
    let label_end = text.find("](")?;
    let dest_start = label_end + 2;
    let dest_end = text[dest_start..].find(')')? + dest_start;
    let dest = text[dest_start..dest_end].trim();
    let after = &text[dest_end + 1..];
    let (target, heading) = markdown_note_target(dest)?;
    Some((
        Mention {
            target,
            alias: None,
            heading,
            embed: false,
            line,
            excerpt: excerpt.to_owned(),
            dest: None,
            kind: LinkKind::Markdown,
        },
        after,
    ))
}

fn markdown_note_target(dest: &str) -> Option<(String, Option<String>)> {
    let dest = dest.split_whitespace().next().unwrap_or(dest);
    if dest.is_empty() || is_external(dest) {
        return None;
    }
    let (path, heading) = split_heading(dest);
    let path = path.split('?').next().unwrap_or(&path);
    let path = percent_decode(path);
    if path.is_empty() || looks_like_asset(&path) {
        return None;
    }
    let bare = path
        .strip_suffix(".md")
        .unwrap_or(&path)
        .trim_end_matches(['/', '\\']);
    if bare.is_empty() {
        None
    } else {
        Some((bare.to_owned(), heading))
    }
}

fn looks_like_asset(path: &str) -> bool {
    match Path::new(path).extension().and_then(|ext| ext.to_str()) {
        Some("md") | None => false,
        Some(_) => true,
    }
}

fn is_external(url: &str) -> bool {
    super::is_external_url(url) || url.starts_with(WIKI_SCHEME)
}

fn skip_escaped(text: &str) -> &str {
    let mut chars = text.chars();
    chars.next();
    chars.next();
    chars.as_str()
}

pub(super) fn take_wikilink(text: &str) -> Option<(Wiki<'_>, &str)> {
    let embed = text.starts_with("![[");
    if !embed && !text.starts_with("[[") {
        return None;
    }
    let open = if embed { 3 } else { 2 };
    let close = text[open..].find("]]")?;
    let inner = &text[open..open + close];
    if inner.contains('\n') || inner.contains("[[") {
        return None;
    }
    Some((Wiki { embed, inner }, &text[open + close + 2..]))
}

pub(super) fn split_alias(inner: &str) -> (&str, &str) {
    match inner.split_once('|') {
        Some((target, alias)) => (target.trim(), alias.trim()),
        None => (inner.trim(), ""),
    }
}

pub(super) fn split_heading(target: &str) -> (String, Option<String>) {
    match target.split_once('#') {
        Some((path, heading)) => (path.trim().to_owned(), nonempty(heading.trim())),
        None => (target.trim().to_owned(), None),
    }
}

fn nonempty(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.to_owned())
    }
}

pub(super) fn note_key(path: &Path) -> String {
    normalize_key(&path.to_string_lossy())
}

pub(super) fn normalize_key(target: &str) -> String {
    let bare = target.strip_suffix(".md").unwrap_or(target);
    super::normalize(Path::new(bare))
        .components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn percent_encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(super) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && i + 2 < bytes.len()
            && let Ok(byte) =
                u8::from_str_radix(std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or(""), 16)
        {
            out.push(byte);
            i += 3;
            continue;
        }
        if bytes[i] == b'+' {
            out.push(b' ');
        } else {
            out.push(bytes[i]);
        }
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| text.to_owned())
}

fn clip(text: &str, max: usize) -> String {
    let end = text
        .char_indices()
        .nth(max)
        .map(|(i, _)| i)
        .unwrap_or(text.len());
    if end < text.len() {
        format!("{}…", &text[..end])
    } else {
        text.to_owned()
    }
}
