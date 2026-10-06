//! Open, save, create, and delete notes.

use std::path::{Component, Path, PathBuf};

use iced::Task;
use iced::widget::text_editor::{self, Cursor, Position};

use super::history::{EditKind, Snapshot};
use super::{
    App, CREATE_NAME_ID, Clicked, CreateKind, CreatePrompt, CreateStep, Doc, Message, Mode,
    SOURCE_EDITOR_ID,
};
use crate::live::Live;
use crate::tree::Dir;

impl App {
    pub(super) fn scan(&self) -> Task<Message> {
        let Some(workspace) = self.workspace.clone() else {
            return Task::none();
        };
        Task::perform(async move { Dir::scan(&workspace) }, Message::FilesScanned)
    }

    pub(super) fn expand_ancestors(&mut self, rel: &Path) {
        let mut dir = rel.parent();
        while let Some(folder) = dir.filter(|folder| !folder.as_os_str().is_empty()) {
            self.expanded.insert(folder.to_path_buf());
            dir = folder.parent();
        }
    }

    pub(super) fn open_file(&mut self, rel: PathBuf) -> Task<Message> {
        if self.current.as_ref() == Some(&rel) {
            return Task::none();
        }
        self.save_if_dirty();
        let Some(workspace) = &self.workspace else {
            return Task::none();
        };
        match std::fs::read_to_string(workspace.join(&rel)) {
            Ok(body) => self.show_opened(rel, &body),
            Err(error) => {
                self.notice = Some(format!("Failed to open {}: {error}", rel.display()));
                Task::none()
            }
        }
    }

    fn show_opened(&mut self, rel: PathBuf, body: &str) -> Task<Message> {
        self.notice = None;
        self.expand_ancestors(&rel);
        self.current = Some(rel);
        self.history.clear();
        self.dirty = false;
        let task = self.load(body, Position { line: 0, column: 0 });
        // Compare undo against the editor's text, not the raw file, so a
        // normalized newline doesn't look unsaved.
        self.saved = self.text();
        task
    }

    /// Load `text` into an editor for the current mode.
    pub(super) fn load(&mut self, text: &str, cursor: Position) -> Task<Message> {
        match self.mode {
            Mode::Live => self.load_live(text, cursor),
            Mode::Source => self.load_source(text, cursor),
        }
    }

    fn load_live(&mut self, text: &str, cursor: Position) -> Task<Message> {
        let (live, task) = Live::new(text, cursor);
        self.doc = Some(Doc::Live(live));
        task.map(Message::Live)
    }

    fn load_source(&mut self, text: &str, cursor: Position) -> Task<Message> {
        let mut content = text_editor::Content::with_text(text);
        content.move_to(Cursor {
            position: cursor,
            selection: None,
        });
        self.doc = Some(Doc::Source(content));
        iced::widget::operation::focus(SOURCE_EDITOR_ID)
    }

    pub(super) fn text(&self) -> Option<String> {
        match self.doc.as_ref()? {
            Doc::Live(live) => Some(live.text()),
            Doc::Source(content) => Some(content.text()),
        }
    }

    pub(super) fn cursor(&self) -> Option<Position> {
        match self.doc.as_ref()? {
            Doc::Live(live) => Some(live.cursor()),
            Doc::Source(content) => Some(content.cursor().position),
        }
    }

    fn capture(&self) -> Option<Snapshot> {
        Some(Snapshot {
            text: self.text()?,
            cursor: self.cursor()?,
        })
    }

    pub(super) fn sync_dirty(&mut self) {
        self.dirty = self.text().as_ref() != self.saved.as_ref();
    }

    /// Run `apply`, snapshotting the note first when `kind` starts a new undo step.
    pub(super) fn record_edit<T>(
        &mut self,
        kind: EditKind,
        apply: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let before = self.text_before_new_step(kind);
        let result = apply(self);
        if let Some(before) = before {
            let changed = self.text().as_deref() != Some(before.as_str());
            self.history.finish(changed);
        }
        self.sync_dirty();
        result
    }

    fn text_before_new_step(&mut self, kind: EditKind) -> Option<String> {
        let snap = self.capture()?;
        let text = snap.text.clone();
        if self.history.begin(kind, snap) {
            return Some(text);
        }
        None
    }

    pub(super) fn update_live(
        &mut self,
        msg: crate::live::Msg,
    ) -> (Task<crate::live::Msg>, crate::live::Outcome) {
        let Some(Doc::Live(live)) = &mut self.doc else {
            return (Task::none(), crate::live::Outcome::None);
        };
        live.update(msg)
    }

    pub(super) fn undo(&mut self) -> Task<Message> {
        self.step(true)
    }

    pub(super) fn redo(&mut self) -> Task<Message> {
        self.step(false)
    }

    fn step(&mut self, undo: bool) -> Task<Message> {
        let Some(current) = self.capture() else {
            return Task::none();
        };
        let snap = if undo {
            self.history.undo(current)
        } else {
            self.history.redo(current)
        };
        let Some(snap) = snap else {
            return Task::none();
        };
        let task = self.load(&snap.text, snap.cursor);
        self.sync_dirty();
        task
    }

    pub(super) fn toggle_mode(&mut self) -> Task<Message> {
        self.mode = match self.mode {
            Mode::Live => Mode::Source,
            Mode::Source => Mode::Live,
        };
        let (Some(text), Some(cursor)) = (self.text(), self.cursor()) else {
            return Task::none();
        };
        self.load(&text, cursor)
    }

    /// Follow a link clicked in the preview: other notes open in the app,
    /// everything else goes to the system handler.
    pub(super) fn open_link(&mut self, url: &str) -> Task<Message> {
        if is_external_url(url) {
            return self.open_in_system(url);
        }
        let Some(rel) = note_path(self.current.as_deref(), url) else {
            return Task::none();
        };
        self.open_workspace_note(rel)
    }

    fn open_in_system(&mut self, url: &str) -> Task<Message> {
        if let Err(error) = open_external(url) {
            self.notice = Some(format!("Could not open {url}: {error}"));
        } else {
            self.notice = None;
        }
        Task::none()
    }

    fn open_workspace_note(&mut self, rel: PathBuf) -> Task<Message> {
        let found = self
            .workspace
            .as_ref()
            .is_some_and(|workspace| workspace.join(&rel).is_file());
        if found {
            return Task::done(Message::Open(rel));
        }
        self.notice = Some(format!("Note not found: {}", rel.display()));
        Task::none()
    }

    pub(super) fn save(&mut self) {
        let (Some(workspace), Some(rel), Some(text)) =
            (&self.workspace, &self.current, self.text())
        else {
            return;
        };
        match std::fs::write(workspace.join(rel), &text) {
            Ok(()) => {
                self.saved = Some(text);
                self.dirty = false;
                self.notice = None;
            }
            Err(error) => self.notice = Some(format!("Save failed: {error}")),
        }
    }

    pub(super) fn save_if_dirty(&mut self) {
        if self.dirty {
            self.save();
        }
    }

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
        if let Err(error) = remove_clicked(&workspace, &clicked) {
            self.notice = Some(error);
            self.create = None;
            return Task::none();
        }
        self.drop_deleted(&clicked);
        self.create = None;
        self.notice = None;
        Task::none()
    }

    fn drop_deleted(&mut self, clicked: &Clicked) {
        self.close_if_deleted(clicked);
        self.drop_expanded(clicked);
        match clicked {
            Clicked::Note(path) => {
                self.tree.remove_file(path);
            }
            Clicked::Folder(path) => {
                self.tree.remove_dir(path);
            }
        }
    }

    fn close_if_deleted(&mut self, clicked: &Clicked) {
        let Some(current) = self.current.clone() else {
            return;
        };
        if !clicked.deletes(&current) {
            return;
        }
        self.current = None;
        self.doc = None;
        self.dirty = false;
        self.saved = None;
        self.history.clear();
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
    if !stays_in_workspace(rel) {
        return Err(format!(
            "Could not delete {}: that path leaves the workspace.",
            rel.display()
        ));
    }
    let path = workspace.join(rel);
    if !path.exists() {
        return Ok(());
    }
    match resolved_inside(workspace, &path) {
        Ok(true) => {}
        Ok(false) => {
            return Err(format!(
                "Could not delete {}: that path leaves the workspace.",
                rel.display()
            ));
        }
        Err(error) => return Err(format!("Could not delete {}: {error}", rel.display())),
    }
    delete_path(&path, clicked)
        .map_err(|error| format!("Could not delete {}: {error}", rel.display()))
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
    match path.extension() {
        Some(ext) => ext != "md",
        None => true,
    }
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

fn is_external_url(url: &str) -> bool {
    url.contains("://") || url.starts_with("mailto:")
}

fn note_path(current: Option<&Path>, url: &str) -> Option<PathBuf> {
    let target = link_target(url)?;
    let mut rel = normalize(&note_folder(current).join(target));
    if rel.extension().is_none() {
        rel.set_extension("md");
    }
    Some(rel)
}

fn link_target(url: &str) -> Option<String> {
    let target = url
        .split('#')
        .next()
        .unwrap_or_default()
        .replace("%20", " ");
    if target.is_empty() {
        None
    } else {
        Some(target)
    }
}

fn note_folder(current: Option<&Path>) -> PathBuf {
    match current.and_then(|path| path.parent()) {
        Some(parent) => parent.to_path_buf(),
        None => PathBuf::new(),
    }
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::Normal(name) => out.push(name),
            _ => {}
        }
    }
    out
}

fn open_external(url: &str) -> std::io::Result<()> {
    std::process::Command::new(open_program())
        .arg(url)
        .spawn()
        .map(drop)
}

fn open_program() -> &'static str {
    if cfg!(target_os = "macos") {
        return "open";
    }
    if cfg!(target_os = "windows") {
        return "explorer";
    }
    "xdg-open"
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
