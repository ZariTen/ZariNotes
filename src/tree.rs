//! In-memory tree of a workspace's folders and Markdown files.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path};

/// A folder: its subfolders and the `.md` files directly inside it.
#[derive(Debug, Clone, Default)]
pub struct Dir {
    pub dirs: BTreeMap<String, Dir>,
    pub files: BTreeSet<String>,
}

impl Dir {
    /// Recursively scan `path`, keeping every non-hidden folder and every `.md` file.
    pub fn scan(path: &Path) -> Dir {
        let mut dir = Dir::default();
        let Ok(entries) = std::fs::read_dir(path) else {
            return dir;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                dir.dirs.insert(name, Dir::scan(&entry.path()));
            } else if kind.is_file() && name.ends_with(".md") {
                dir.files.insert(name);
            }
        }
        dir
    }

    /// Add a file at a workspace-relative path, creating intermediate folders.
    pub fn insert_file(&mut self, rel: &Path) {
        let mut names = normal_names(rel);
        let Some(file) = names.pop() else {
            return;
        };
        let mut dir = self;
        for name in names {
            dir = dir.dirs.entry(name).or_default();
        }
        dir.files.insert(file);
    }

    pub fn is_empty(&self) -> bool {
        self.dirs.is_empty() && self.files.is_empty()
    }

    pub fn file_count(&self) -> usize {
        self.files.len() + self.dirs.values().map(Self::file_count).sum::<usize>()
    }

    /// Case-insensitive substring match. An empty query matches nothing —
    /// callers treat "no query" before calling this.
    pub fn name_hit(name: &str, query: &str) -> bool {
        let query = query.trim();
        !query.is_empty() && name.to_lowercase().contains(&query.to_lowercase())
    }

    /// Match a note's display name, not the `.md` suffix, so "md" is not a hit.
    pub fn file_hit(filename: &str, query: &str) -> bool {
        Self::name_hit(filename.strip_suffix(".md").unwrap_or(filename), query)
    }

    /// Whether this folder's name, a note, or a descendant matches `query`.
    pub fn contains_match(&self, query: &str) -> bool {
        if query.trim().is_empty() {
            return true;
        }
        if self.files.iter().any(|file| Self::file_hit(file, query)) {
            return true;
        }
        self.dirs
            .iter()
            .any(|(name, sub)| Self::name_hit(name, query) || sub.contains_match(query))
    }

    /// Notes that a filtered tree would show.
    ///
    /// An empty query counts every note. A folder whose name matches contributes
    /// all of its notes, not only the ones whose names match.
    pub fn visible_file_count(&self, query: &str) -> usize {
        if query.trim().is_empty() {
            return self.file_count();
        }
        let mut count = self
            .files
            .iter()
            .filter(|file| Self::file_hit(file, query))
            .count();
        for (name, sub) in &self.dirs {
            count += notes_shown_in(name, sub, query);
        }
        count
    }
}

fn normal_names(path: &Path) -> Vec<String> {
    let mut names = Vec::new();
    for part in path.components() {
        if let Component::Normal(name) = part {
            names.push(name.to_string_lossy().into_owned());
        }
    }
    names
}

/// Notes a filtered tree shows under this folder.
///
/// A folder whose own name matches contributes every note inside it.
fn notes_shown_in(name: &str, folder: &Dir, query: &str) -> usize {
    if Dir::name_hit(name, query) {
        return folder.file_count();
    }
    if folder.contains_match(query) {
        return folder.visible_file_count(query);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn insert_builds_nested_folders() {
        let mut root = Dir::default();
        root.insert_file(Path::new("a.md"));
        root.insert_file(Path::new("journal/2026/sept.md"));
        root.insert_file(Path::new("journal/index.md"));

        assert!(root.files.contains("a.md"));
        let journal = &root.dirs["journal"];
        assert!(journal.files.contains("index.md"));
        assert!(journal.dirs["2026"].files.contains("sept.md"));
    }

    #[test]
    fn scan_keeps_folders_and_markdown_only() {
        let tmp = std::env::temp_dir().join(format!("zarinotes-tree-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("sub/deep")).unwrap();
        std::fs::create_dir_all(tmp.join(".hidden")).unwrap();
        std::fs::create_dir_all(tmp.join("empty")).unwrap();
        std::fs::write(tmp.join("root.md"), "").unwrap();
        std::fs::write(tmp.join("image.png"), "").unwrap();
        std::fs::write(tmp.join("sub/deep/note.md"), "").unwrap();
        std::fs::write(tmp.join(".hidden/secret.md"), "").unwrap();

        let tree = Dir::scan(&tmp);
        std::fs::remove_dir_all(&tmp).unwrap();

        assert_eq!(tree.files.iter().collect::<Vec<_>>(), ["root.md"]);
        assert_eq!(tree.dirs.keys().collect::<Vec<_>>(), ["empty", "sub"]);
        assert!(tree.dirs["sub"].dirs["deep"].files.contains("note.md"));
    }

    #[test]
    fn filter_counts_matching_notes_and_folder_names() {
        let mut root = Dir::default();
        root.insert_file(Path::new("a.md"));
        root.insert_file(Path::new("journal/2026/sept.md"));
        root.insert_file(Path::new("journal/index.md"));
        root.insert_file(Path::new("ideas/draft.md"));

        assert_eq!(root.file_count(), 4);
        assert_eq!(root.visible_file_count(""), 4);
        assert_eq!(root.visible_file_count("   "), 4);
        assert_eq!(root.visible_file_count("sept"), 1);
        assert_eq!(root.visible_file_count("JOURNAL"), 2);
        assert_eq!(root.visible_file_count("md"), 0);
        assert_eq!(root.visible_file_count("nope"), 0);
        assert!(root.contains_match("draft"));
        assert!(!root.contains_match("nope"));
    }
}
