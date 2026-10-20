//! The sidebar's right-click menu: create a note or folder, or delete one.

use std::path::{Component, Path, PathBuf};

use iced::Task;

use super::{App, CREATE_NAME_ID, Clicked, CreateKind, CreatePrompt, CreateStep, Message};

impl App {
    pub(super) fn ask_create(
        &mut self,
        parent: PathBuf,
        at: iced::Point,
        clicked: Option<Clicked>,
    ) -> Task<Message> {
        if self.workspace.is_none() {
            return Task::none();
        }
        self.notice = None;
        self.create = Some(CreatePrompt {
            parent,
            at,
            step: CreateStep::Choose,
            name: String::new(),
            clicked,
        });
        Task::none()
    }

    pub(super) fn ask_delete(&mut self) -> Task<Message> {
        let Some(prompt) = &mut self.create else {
            return Task::none();
        };
        if prompt.clicked.is_none() {
            return Task::none();
        }
        prompt.step = CreateStep::ConfirmDelete;
        Task::none()
    }

    pub(super) fn confirm_delete(&mut self) -> Task<Message> {
        let Some(workspace) = self.workspace.clone() else {
            return Task::none();
        };
        let Some(prompt) = self.create.clone() else {
            return Task::none();
        };
        if prompt.step != CreateStep::ConfirmDelete {
            return Task::none();
        }
        let Some(clicked) = prompt.clicked else {
            return Task::none();
        };
        self.create = None;
        match remove_clicked(&workspace, &clicked) {
            Ok(()) => {
                self.drop_deleted(&clicked);
                self.notice = None;
            }
            Err(error) => self.notice = Some(error),
        }
        Task::none()
    }

    fn drop_deleted(&mut self, clicked: &Clicked) {
        self.close_if_deleted(clicked);
        self.drop_expanded(clicked);
        match clicked {
            Clicked::Note(path) => {
                self.tree.remove_file(path);
                self.links.remove(path, false);
            }
            Clicked::Folder(path) => {
                self.tree.remove_dir(path);
                self.links.remove(path, true);
            }
        }
    }

    fn close_if_deleted(&mut self, clicked: &Clicked) {
        if self
            .current
            .as_ref()
            .is_some_and(|current| clicked.deletes(current))
        {
            self.close_note();
        }
    }

    fn drop_expanded(&mut self, clicked: &Clicked) {
        let Clicked::Folder(folder) = clicked else {
            return;
        };
        self.expanded.retain(|path| !path.starts_with(folder));
    }

    pub(super) fn pick_create(&mut self, kind: CreateKind) -> Task<Message> {
        let Some(prompt) = &mut self.create else {
            return Task::none();
        };
        prompt.step = CreateStep::Name(kind);
        prompt.name.clear();
        iced::widget::operation::focus(CREATE_NAME_ID)
    }

    pub(super) fn set_create_name(&mut self, name: String) -> Task<Message> {
        if let Some(prompt) = &mut self.create {
            prompt.name = name;
        }
        Task::none()
    }

    pub(super) fn submit_create(&mut self) -> Task<Message> {
        let Some(workspace) = self.workspace.clone() else {
            return Task::none();
        };
        let Some(prompt) = self.create.clone() else {
            return Task::none();
        };
        let CreateStep::Name(kind) = prompt.step else {
            return Task::none();
        };
        let rel = match entry_path(&prompt.parent, &prompt.name, kind) {
            EntryPath::Blank => return Task::none(),
            EntryPath::Rejected => {
                self.notice = Some("Use a single name, without slashes or ..".into());
                return Task::none();
            }
            EntryPath::Ready(rel) => rel,
        };
        if let Err(error) = write_entry(&workspace, &rel, kind) {
            self.notice = Some(format!("Could not create {}: {error}", rel.display()));
            return Task::none();
        }
        self.create = None;
        self.filter.clear();
        self.notice = None;
        self.reveal_folder(rel.parent().unwrap_or(Path::new("")));
        match kind {
            CreateKind::Note => {
                self.tree.insert_file(&rel);
                self.links.insert_empty(&rel);
                Task::done(Message::Open(rel))
            }
            CreateKind::Folder => {
                self.tree.insert_dir(&rel);
                self.expanded.insert(rel);
                Task::none()
            }
        }
    }

    fn reveal_folder(&mut self, folder: &Path) {
        if !folder.as_os_str().is_empty() {
            self.expanded.insert(folder.to_path_buf());
        }
        self.expand_ancestors(folder);
    }
}

#[derive(Debug, PartialEq, Eq)]
enum EntryPath {
    Blank,
    Rejected,
    Ready(PathBuf),
}

/// A single name inside `parent`. Notes gain `.md` when they don't already have it.
fn entry_path(parent: &Path, name: &str, kind: CreateKind) -> EntryPath {
    let name = name.trim();
    if name.is_empty() {
        return EntryPath::Blank;
    }
    if rejected_name(name) {
        return EntryPath::Rejected;
    }
    let mut rel = parent.join(name);
    if kind == CreateKind::Note && needs_md_extension(&rel) {
        rel.set_extension("md");
    }
    if leaves_workspace(&rel) {
        return EntryPath::Rejected;
    }
    EntryPath::Ready(rel)
}

fn rejected_name(name: &str) -> bool {
    name == "." || name == ".." || name.starts_with('.') || name.contains(['/', '\\'])
}

fn leaves_workspace(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    path.components().any(|part| part == Component::ParentDir)
}

/// A click target may only name normal path pieces inside the workspace.
fn stays_in_workspace(rel: &Path) -> bool {
    let mut parts = rel.components();
    let Some(Component::Normal(_)) = parts.next() else {
        return false;
    };
    parts.all(|part| matches!(part, Component::Normal(_)))
}

fn remove_clicked(workspace: &Path, clicked: &Clicked) -> Result<(), String> {
    let rel = clicked.path();
    let failed = |why: &dyn std::fmt::Display| format!("Could not delete {}: {why}", rel.display());
    let leaves = "that path leaves the workspace.";
    if !stays_in_workspace(rel) {
        return Err(failed(&leaves));
    }
    let path = workspace.join(rel);
    if !path.exists() {
        return Ok(());
    }
    match resolved_inside(workspace, &path) {
        Ok(true) => delete_path(&path, clicked).map_err(|error| failed(&error)),
        Ok(false) => Err(failed(&leaves)),
        Err(error) => Err(failed(&error)),
    }
}

fn resolved_inside(workspace: &Path, path: &Path) -> std::io::Result<bool> {
    let root = workspace.canonicalize()?;
    let target = path.canonicalize()?;
    Ok(target != root && target.starts_with(&root))
}

fn delete_path(path: &Path, clicked: &Clicked) -> std::io::Result<()> {
    match clicked {
        Clicked::Note(_) => std::fs::remove_file(path),
        Clicked::Folder(_) => std::fs::remove_dir_all(path),
    }
}

fn needs_md_extension(path: &Path) -> bool {
    path.extension().is_none_or(|ext| ext != "md")
}

fn write_entry(workspace: &Path, rel: &Path, kind: CreateKind) -> std::io::Result<()> {
    let path = workspace.join(rel);
    match kind {
        CreateKind::Note => create_empty_file(&path),
        CreateKind::Folder => {
            if path.is_file() {
                return Err(std::io::Error::other(
                    "a file with that name already exists",
                ));
            }
            std::fs::create_dir_all(path)
        }
    }
}

fn create_empty_file(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, "")
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use super::{Clicked, CreateKind, EntryPath, entry_path, remove_clicked, stays_in_workspace};

    #[test]
    fn entry_path_puts_a_note_in_the_chosen_folder() {
        assert_eq!(
            entry_path(Path::new("journal"), "ideas", CreateKind::Note),
            EntryPath::Ready(PathBuf::from("journal/ideas.md"))
        );
        assert_eq!(
            entry_path(Path::new(""), "ideas.md", CreateKind::Note),
            EntryPath::Ready(PathBuf::from("ideas.md"))
        );
    }

    #[test]
    fn entry_path_keeps_a_folder_name_as_typed() {
        assert_eq!(
            entry_path(Path::new("journal"), "2026", CreateKind::Folder),
            EntryPath::Ready(PathBuf::from("journal/2026"))
        );
    }

    #[test]
    fn entry_path_rejects_a_path_instead_of_a_name() {
        assert!(matches!(
            entry_path(Path::new(""), "  ", CreateKind::Note),
            EntryPath::Blank
        ));
        assert!(matches!(
            entry_path(Path::new("journal"), "a/b", CreateKind::Note),
            EntryPath::Rejected
        ));
        assert!(matches!(
            entry_path(Path::new(""), "..", CreateKind::Folder),
            EntryPath::Rejected
        ));
        assert!(matches!(
            entry_path(Path::new(""), ".hidden", CreateKind::Note),
            EntryPath::Rejected
        ));
    }

    #[test]
    fn stays_in_workspace_allows_only_a_normal_relative_path() {
        assert!(stays_in_workspace(Path::new("journal/ideas.md")));
        assert!(!stays_in_workspace(Path::new("")));
        assert!(!stays_in_workspace(Path::new(".")));
        assert!(!stays_in_workspace(Path::new("..")));
        assert!(!stays_in_workspace(Path::new("journal/../secret.md")));
        assert!(!stays_in_workspace(Path::new("/tmp/note.md")));
    }

    #[test]
    fn remove_clicked_deletes_a_note_or_folder_and_refuses_to_leave() {
        let root = std::env::temp_dir().join(format!("zarinotes-delete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("journal")).unwrap();
        std::fs::write(root.join("journal/ideas.md"), "hi").unwrap();
        std::fs::write(root.join("keep.md"), "stay").unwrap();

        remove_clicked(&root, &Clicked::Note(PathBuf::from("journal/ideas.md"))).unwrap();
        assert!(!root.join("journal/ideas.md").exists());
        assert!(root.join("journal").is_dir());
        assert!(root.join("keep.md").exists());

        let outside = Clicked::Note(PathBuf::from("../keep.md"));
        assert!(remove_clicked(&root, &outside).is_err());
        assert!(root.join("keep.md").exists());

        remove_clicked(&root, &Clicked::Folder(PathBuf::from("journal"))).unwrap();
        assert!(!root.join("journal").exists());
        assert!(root.join("keep.md").exists());

        std::fs::remove_dir_all(&root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn remove_clicked_refuses_a_symlink_that_leaves_the_workspace() {
        let root =
            std::env::temp_dir().join(format!("zarinotes-delete-link-{}", std::process::id()));
        let outside =
            std::env::temp_dir().join(format!("zarinotes-delete-out-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let _ = std::fs::remove_dir_all(&outside);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("safe.txt"), "no").unwrap();
        std::os::unix::fs::symlink(&outside, root.join("linked")).unwrap();

        let error = remove_clicked(&root, &Clicked::Folder(PathBuf::from("linked")));
        assert!(error.is_err());
        assert!(outside.join("safe.txt").exists());
        assert!(root.join("linked").exists());

        std::fs::remove_dir_all(&root).unwrap();
        std::fs::remove_dir_all(&outside).unwrap();
    }
}
