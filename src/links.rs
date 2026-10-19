//! Wikilink index for a workspace.
//!
//! Notes are plain files. This module is the graph over them: every `[[note]]`
//! and every Markdown link to a `.md` file, resolved the way Obsidian does
//! (shortest unique path, then a unique file name), plus the reverse map that
//! answers "what links here?".
//!
//! The index is rebuilt from disk when the tree is scanned, and patched for the
//! open note when it is saved. Unsaved text is not in the graph — a backlink
//! panel that changed on every keystroke would thrash, and other notes can only
//! see what is on disk anyway.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};

use crate::tree::Dir;

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
    kind: LinkKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkKind {
    Wiki,
    Markdown,
}

/// A note that points at the note currently open.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backlink {
    /// Workspace-relative path of the note that contains the link.
    pub from: PathBuf,
    pub alias: Option<String>,
    pub heading: Option<String>,
    pub embed: bool,
    /// Line in `from`, so the panel can quote what was said.
    pub line: usize,
    /// The source line, trimmed, from the moment that note was indexed.
    pub excerpt: String,
}

/// Resolved target of a wikilink click, or why it could not be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// A single note. `heading` is the fragment after `#`, if the link had one.
    Note {
        path: PathBuf,
        heading: Option<String>,
    },
    /// Same note, a heading only (`[[#Heading]]`).
    Heading(String),
    /// More than one note shares that name or path suffix.
    Ambiguous {
        target: String,
        matches: Vec<PathBuf>,
    },
    /// Nothing in the vault matches.
    Missing { target: String },
}

/// Outgoing links and the reverse map, for one workspace.
#[derive(Debug, Clone, Default)]
pub struct Index {
    /// Every Markdown note, workspace-relative, in scan order.
    notes: Vec<PathBuf>,
    /// Lowercased lookup key → note indexes. A key is the full path without
    /// `.md`, and every suffix of that path (`folder/note`, `note`).
    by_key: HashMap<String, Vec<usize>>,
    /// Workspace-relative path → the links written in that note.
    outgoing: HashMap<PathBuf, Vec<Mention>>,
    /// Resolved target path → notes that link to it.
    incoming: HashMap<PathBuf, Vec<PathBuf>>,
}

/// Tree plus the link graph built from the same folder.
#[derive(Debug, Clone)]
pub struct Scanned {
    pub tree: Dir,
    pub index: Index,
}

impl Index {
    /// Read every note in `tree` under `workspace` and index its links.
    pub fn build(workspace: &Path, tree: &Dir) -> Self {
        let notes = tree.files();
        let mut index = Self {
            by_key: keys_for(&notes),
            notes,
            ..Self::default()
        };
        for rel in index.notes.clone() {
            let Ok(text) = std::fs::read_to_string(workspace.join(&rel)) else {
                continue;
            };
            index.set_note(&rel, &text);
        }
        index
    }

    /// Replace one note's links. Used after a save, so the panel matches disk
    /// without rereading the whole vault.
    pub fn set_note(&mut self, rel: &Path, text: &str) {
        self.forget(rel);
        let mentions = self.mentions_in(rel, text);
        self.remember(rel, mentions);
    }

    /// Drop a note from both maps. A deleted folder drops every note under it.
    ///
    /// Other notes are resolved again: a name that was ambiguous may now be
    /// unique, and a link to the deleted note is no longer a backlink.
    pub fn remove(&mut self, rel: &Path, folder: bool) {
        let doomed: Vec<PathBuf> = if folder {
            self.notes
                .iter()
                .filter(|note| note.starts_with(rel))
                .cloned()
                .collect()
        } else {
            vec![rel.to_path_buf()]
        };
        for note in &doomed {
            self.outgoing.remove(note);
            self.notes.retain(|path| path != note);
        }
        self.reindex();
    }

    /// A note was created empty. It can be linked to immediately; it links nowhere.
    ///
    /// Existing notes are resolved again, so a `[[New]]` written before the file
    /// existed becomes a backlink without waiting for the next scan.
    pub fn insert_empty(&mut self, rel: &Path) {
        if self.notes.iter().any(|note| note == rel) {
            return;
        }
        self.notes.push(rel.to_path_buf());
        self.reindex();
    }

    /// Notes that link to `rel`, with the excerpt captured when each was indexed.
    ///
    /// The open note's own outgoing links are not listed — that would just
    /// repeat the page you are reading.
    pub fn backlinks(&self, rel: &Path) -> Vec<Backlink> {
        let Some(froms) = self.incoming.get(rel) else {
            return Vec::new();
        };
        let mut links = Vec::new();
        for from in froms {
            if from == rel {
                continue;
            }
            let Some(mentions) = self.outgoing.get(from) else {
                continue;
            };
            for mention in mentions {
                if mention.dest.as_deref() != Some(rel) {
                    continue;
                }
                links.push(Backlink {
                    from: from.clone(),
                    alias: mention.alias.clone(),
                    heading: mention.heading.clone(),
                    embed: mention.embed,
                    line: mention.line,
                    excerpt: mention.excerpt.clone(),
                });
            }
        }
        links.sort_by(|a, b| a.from.cmp(&b.from).then(a.line.cmp(&b.line)));
        links
    }

    fn mentions_in(&self, from: &Path, text: &str) -> Vec<Mention> {
        let mut mentions = parse_mentions(text);
        for mention in &mut mentions {
            mention.dest = self.destination(from, mention);
        }
        mentions
    }

    fn destination(&self, from: &Path, mention: &Mention) -> Option<PathBuf> {
        match mention.kind {
            LinkKind::Wiki => match self.resolve(&mention.target) {
                Resolved::Note { path, .. } => Some(path),
                _ => None,
            },
            LinkKind::Markdown => self.relative_note(from, &mention.target),
        }
    }

    /// A Markdown link is relative to the note it is written in, same as a
    /// click in the preview. It counts only when that exact note exists.
    fn relative_note(&self, from: &Path, target: &str) -> Option<PathBuf> {
        let folder = from.parent().unwrap_or(Path::new(""));
        let key = normalize_key(&folder.join(target).to_string_lossy());
        if key.is_empty() {
            return None;
        }
        let matches = self.by_key.get(&key)?;
        let exact: Vec<&PathBuf> = matches
            .iter()
            .map(|index| &self.notes[*index])
            .filter(|path| note_key(path) == key)
            .collect();
        match exact.as_slice() {
            [path] => Some((*path).clone()),
            _ => None,
        }
    }

    fn remember(&mut self, rel: &Path, mentions: Vec<Mention>) {
        if mentions.is_empty() {
            return;
        }
        for target in unique_dests(&mentions) {
            self.incoming
                .entry(target)
                .or_default()
                .push(rel.to_path_buf());
        }
        self.outgoing.insert(rel.to_path_buf(), mentions);
    }

    fn reindex(&mut self) {
        self.by_key = keys_for(&self.notes);
        self.incoming.clear();
        let outgoing = std::mem::take(&mut self.outgoing);
        for (rel, mut mentions) in outgoing {
            for mention in &mut mentions {
                mention.dest = self.destination(&rel, mention);
            }
            self.remember(&rel, mentions);
        }
    }

    /// Resolve a wikilink target against the vault.
    ///
    /// `[[Note]]` matches a unique `Note.md` anywhere. `[[folder/Note]]` matches
    /// that suffix. Several matches stay ambiguous instead of guessing — a
    /// wrong note is worse than a notice.
    pub fn resolve(&self, target: &str) -> Resolved {
        let (path, heading) = split_heading(target);
        if path.is_empty() {
            return match heading {
                Some(heading) => Resolved::Heading(heading),
                None => Resolved::Missing {
                    target: target.to_owned(),
                },
            };
        }
        let key = normalize_key(&path);
        if key.is_empty() {
            return Resolved::Missing { target: path };
        }
        match self.by_key.get(&key).map(Vec::as_slice).unwrap_or(&[]) {
            [] => Resolved::Missing { target: path },
            [index] => Resolved::Note {
                path: self.notes[*index].clone(),
                heading,
            },
            matches => {
                let mut paths: Vec<PathBuf> =
                    matches.iter().map(|i| self.notes[*i].clone()).collect();
                paths.sort();
                Resolved::Ambiguous {
                    target: path,
                    matches: paths,
                }
            }
        }
    }

    /// Whether `url` is a wikilink written into Markdown as `zari-wiki:…`.
    pub fn is_wiki_url(url: &str) -> bool {
        url.starts_with(WIKI_SCHEME)
    }

    /// Turn a rewritten preview link back into a [`Resolved`] target.
    pub fn resolve_wiki_url(&self, url: &str) -> Option<Resolved> {
        let raw = url.strip_prefix(WIKI_SCHEME)?;
        let target = percent_decode(raw);
        Some(self.resolve(&target))
    }

    fn forget(&mut self, rel: &Path) {
        self.outgoing.remove(rel);
        for froms in self.incoming.values_mut() {
            froms.retain(|path| path != rel);
        }
        self.incoming.retain(|_, froms| !froms.is_empty());
    }
}

fn unique_dests(mentions: &[Mention]) -> Vec<PathBuf> {
    let mut targets = Vec::new();
    for mention in mentions {
        if let Some(path) = &mention.dest
            && !targets.contains(path)
        {
            targets.push(path.clone());
        }
    }
    targets
}

/// Scan the folder and index its links. Run off the UI thread.
pub fn scan(workspace: &Path) -> Scanned {
    let tree = Dir::scan(workspace);
    let index = Index::build(workspace, &tree);
    Scanned { tree, index }
}

const WIKI_SCHEME: &str = "zari-wiki:";

/// Rewrite `[[links]]` into Markdown links the live preview can click.
///
/// The preview parser does not know wikilinks. A link to `zari-wiki:Note`
/// round-trips through the viewer and is resolved on click. Code spans and
/// fenced blocks are left alone — a wikilink in a sample is not a link.
pub fn rewrite_wikilinks(source: &str) -> String {
    let mut out = String::with_capacity(source.len());
    let mut in_fence = false;
    let mut lines = source.split('\n');
    if let Some(first) = lines.next() {
        in_fence = rewrite_outside_fence(first, in_fence, &mut out);
    }
    for line in lines {
        out.push('\n');
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

struct Wiki<'a> {
    embed: bool,
    inner: &'a str,
}

fn take_wikilink(text: &str) -> Option<(Wiki<'_>, &str)> {
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

/// Every wikilink and every Markdown link to a note, outside code.
fn parse_mentions(text: &str) -> Vec<Mention> {
    let mut mentions = Vec::new();
    let mut in_fence = false;
    // YAML between the opening `---` and its close is not prose. A `[[link]]`
    // there is a value, not a note reference.
    let mut in_front_matter = false;
    let mut started = false;
    for (line_no, line) in text.lines().enumerate() {
        if !started {
            started = true;
            if is_front_matter_mark(line) {
                in_front_matter = true;
                continue;
            }
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

fn is_front_matter_mark(line: &str) -> bool {
    let line = line.trim_end();
    line == "---" || line == "..."
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
fn is_asset_embed(link: &Wiki<'_>) -> bool {
    link.embed && looks_like_asset(split_alias(link.inner).0)
}

/// `[[]]` and `[[|alias]]` name no note. Leave them as typed.
fn wiki_target_blank(link: &Wiki<'_>) -> bool {
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
    url.contains("://") || url.starts_with("mailto:") || url.starts_with(WIKI_SCHEME)
}

fn is_fence(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn skip_escaped(text: &str) -> &str {
    let mut chars = text.chars();
    chars.next();
    chars.next();
    chars.as_str()
}

fn split_alias(inner: &str) -> (&str, &str) {
    match inner.split_once('|') {
        Some((target, alias)) => (target.trim(), alias.trim()),
        None => (inner.trim(), ""),
    }
}

fn split_heading(target: &str) -> (String, Option<String>) {
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

fn note_key(path: &Path) -> String {
    normalize_key(&path.to_string_lossy())
}

fn normalize_key(target: &str) -> String {
    let mut out = PathBuf::new();
    let bare = target.strip_suffix(".md").unwrap_or(target);
    for part in Path::new(bare).components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    out.components()
        .filter_map(|part| match part {
            Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn keys_for(notes: &[PathBuf]) -> HashMap<String, Vec<usize>> {
    let mut by_key: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, note) in notes.iter().enumerate() {
        let full = normalize_key(&note.to_string_lossy());
        if full.is_empty() {
            continue;
        }
        let mut suffix = full.as_str();
        loop {
            by_key.entry(suffix.to_owned()).or_default().push(index);
            match suffix.split_once('/') {
                Some((_, rest)) => suffix = rest,
                None => break,
            }
        }
    }
    by_key
}

fn percent_encode(text: &str) -> String {
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

fn percent_decode(text: &str) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn index_of(notes: &[&str], bodies: &[(&str, &str)]) -> Index {
        let notes: Vec<PathBuf> = notes.iter().map(PathBuf::from).collect();
        let mut index = Index {
            by_key: keys_for(&notes),
            notes,
            ..Index::default()
        };
        for (path, text) in bodies {
            index.set_note(Path::new(path), text);
        }
        index
    }

    #[test]
    fn parse_skips_code_and_keeps_alias_heading_and_embed() {
        let text = "\
see [[Note|alias]] and [[folder/Other#Heading]]
![[Embed]]
`[[not a link]]`
```
[[fenced]]
```
and [md](folder/Note.md) plus [web](https://example.com) and ![](pic.png)
";
        let mentions = parse_mentions(text);
        assert_eq!(mentions.len(), 4);
        assert_eq!(mentions[0].target, "Note");
        assert_eq!(mentions[0].alias.as_deref(), Some("alias"));
        assert_eq!(mentions[1].target, "folder/Other");
        assert_eq!(mentions[1].heading.as_deref(), Some("Heading"));
        assert!(mentions[2].embed);
        assert_eq!(mentions[2].target, "Embed");
        assert_eq!(mentions[3].target, "folder/Note");
    }

    #[test]
    fn resolve_uses_unique_name_and_refuses_duplicates() {
        let index = index_of(
            &["journal/Ideas.md", "archive/ideas.md", "journal/Other.md"],
            &[],
        );
        assert_eq!(
            index.resolve("journal/Other"),
            Resolved::Note {
                path: PathBuf::from("journal/Other.md"),
                heading: None,
            }
        );
        assert_eq!(
            index.resolve("Other#Today"),
            Resolved::Note {
                path: PathBuf::from("journal/Other.md"),
                heading: Some("Today".into()),
            }
        );
        assert!(matches!(index.resolve("ideas"), Resolved::Ambiguous { .. }));
        assert!(matches!(index.resolve("missing"), Resolved::Missing { .. }));
        assert_eq!(index.resolve("#Here"), Resolved::Heading("Here".into()));
    }

    #[test]
    fn backlinks_follow_wikilinks_and_markdown_links() {
        let index = index_of(
            &["Ideas.md", "journal/Day.md", "Alone.md"],
            &[
                ("journal/Day.md", "Wrote about [[Ideas|the idea]] today."),
                (
                    "Alone.md",
                    "No mention, but [elsewhere](../Ideas.md) waits.",
                ),
                (
                    "Ideas.md",
                    "[[Alone]] is outgoing, not a backlink of itself.",
                ),
            ],
        );
        let links = index.backlinks(Path::new("Ideas.md"));
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].from, PathBuf::from("Alone.md"));
        assert_eq!(links[1].from, PathBuf::from("journal/Day.md"));
        assert_eq!(links[1].alias.as_deref(), Some("the idea"));
        assert!(links[1].excerpt.contains("[[Ideas|the idea]]"));
    }

    #[test]
    fn markdown_links_are_relative_to_the_note() {
        let index = index_of(
            &["Ideas.md", "journal/Ideas.md", "journal/Day.md"],
            &[(
                "journal/Day.md",
                "See [ideas](Ideas.md) and [root](../Ideas.md).",
            )],
        );
        let links = index.backlinks(Path::new("journal/Ideas.md"));
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].from, PathBuf::from("journal/Day.md"));
        let root = index.backlinks(Path::new("Ideas.md"));
        assert_eq!(root.len(), 1);
        assert!(root[0].excerpt.contains("../Ideas.md"));
    }

    #[test]
    fn creating_a_note_makes_an_existing_wikilink_resolve() {
        let mut index = index_of(&["Day.md"], &[("Day.md", "[[Ideas]]")]);
        assert!(index.backlinks(Path::new("Ideas.md")).is_empty());
        index.insert_empty(Path::new("Ideas.md"));
        assert_eq!(index.backlinks(Path::new("Ideas.md")).len(), 1);
    }

    #[test]
    fn rewrite_makes_a_clickable_link_and_leaves_code() {
        let rewritten = rewrite_wikilinks("see [[Note|Hi]] and `[[no]]`");
        assert!(rewritten.contains("[Hi](zari-wiki:"));
        assert!(rewritten.contains("`[[no]]`"));
        assert!(!rewritten.contains("[[Note"));
    }

    #[test]
    fn front_matter_and_image_embeds_are_not_notes() {
        let text = "---\ntitle: [[Ideas]]\n---\nsee [[Ideas]] and ![[pic.png]]\n";
        let mentions = parse_mentions(text);
        assert_eq!(mentions.len(), 1);
        assert_eq!(mentions[0].target, "Ideas");
        assert!(!mentions[0].embed);

        let rewritten = rewrite_wikilinks("![[pic.png]] and [[]]");
        assert_eq!(rewritten, "![[pic.png]] and [[]]");
    }

    #[test]
    fn remove_drops_backlinks_from_a_deleted_note() {
        let mut index = index_of(&["Ideas.md", "Day.md"], &[("Day.md", "[[Ideas]]")]);
        assert_eq!(index.backlinks(Path::new("Ideas.md")).len(), 1);
        index.remove(Path::new("Day.md"), false);
        assert!(index.backlinks(Path::new("Ideas.md")).is_empty());
        assert!(matches!(index.resolve("Day"), Resolved::Missing { .. }));
    }
}
