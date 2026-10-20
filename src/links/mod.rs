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

mod preview;
mod wiki;

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::tree::Dir;

use wiki::{LinkKind, normalize_key, note_key, percent_decode, split_heading};

pub use preview::rewrite_wikilinks;
pub use wiki::Mention;

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
        let mut mentions = wiki::parse_mentions(text);
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
        url.starts_with(wiki::WIKI_SCHEME)
    }

    /// Turn a rewritten preview link back into a [`Resolved`] target.
    pub fn resolve_wiki_url(&self, url: &str) -> Option<Resolved> {
        let raw = url.strip_prefix(wiki::WIKI_SCHEME)?;
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

/// Scan the folder and index its links. Run off the UI thread.
pub fn scan(workspace: &Path) -> Scanned {
    let tree = Dir::scan(workspace);
    let index = Index::build(workspace, &tree);
    Scanned { tree, index }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

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
        let mentions = wiki::parse_mentions(text);
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
        let mentions = wiki::parse_mentions(text);
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
