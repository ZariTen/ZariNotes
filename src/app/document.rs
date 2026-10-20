//! Open, save, undo, and follow links in the open note.

use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::text_editor::{self, Cursor, Position};

use super::history::{EditKind, Input, Snapshot};
use super::{App, Doc, Message, Mode, SOURCE_EDITOR_ID};
use crate::links::{self, Resolved};
use crate::live::Live;

impl App {
    pub(super) fn scan(&self) -> Task<Message> {
        let Some(workspace) = self.workspace.clone() else {
            return Task::none();
        };
        Task::perform(
            async move { links::scan(&workspace) },
            Message::FilesScanned,
        )
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
        self.history.begin(kind, snap).then_some(text)
    }

    /// Run `apply` as `input` says: a recorded edit, a caret move that ends
    /// the current undo group, or neither.
    pub(super) fn apply_input<T>(&mut self, input: Input, apply: impl FnOnce(&mut Self) -> T) -> T {
        match input {
            Input::Edit(kind) => self.record_edit(kind, apply),
            Input::Moved => {
                self.history.close();
                apply(self)
            }
            Input::Ignore => apply(self),
        }
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
        if links::Index::is_wiki_url(url) {
            return self.open_wiki(url);
        }
        if links::is_external_url(url) {
            return self.open_in_system(url);
        }
        let Some(rel) = note_path(self.current.as_deref(), url) else {
            return Task::none();
        };
        self.open_workspace_note(rel)
    }

    /// A `[[wikilink]]` rewritten for the live preview.
    ///
    /// A heading fragment is not scrolled to yet, so it is dropped rather than
    /// flashed in the footer — opening the note clears that footer anyway.
    fn open_wiki(&mut self, url: &str) -> Task<Message> {
        let Some(resolved) = self.links.resolve_wiki_url(url) else {
            return Task::none();
        };
        match resolved {
            Resolved::Note { path, .. } => self.open_workspace_note(path),
            Resolved::Heading(_) => Task::none(),
            Resolved::Ambiguous { target, matches } => {
                let names = matches
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.notice = Some(format!("“{target}” matches more than one note: {names}"));
                Task::none()
            }
            Resolved::Missing { target } => {
                self.notice = Some(format!("Note not found: {target}"));
                Task::none()
            }
        }
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
                self.links.set_note(rel, &text);
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

    /// Forget the open note without saving it.
    pub(super) fn close_note(&mut self) {
        self.current = None;
        self.doc = None;
        self.dirty = false;
        self.saved = None;
        self.history.clear();
    }
}

fn note_path(current: Option<&Path>, url: &str) -> Option<PathBuf> {
    let target = link_target(url)?;
    let mut rel = links::normalize(&note_folder(current).join(target));
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
    (!target.is_empty()).then_some(target)
}

fn note_folder(current: Option<&Path>) -> PathBuf {
    current
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_default()
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
