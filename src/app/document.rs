//! Open, save, and create notes.

use std::path::{Path, PathBuf};

use iced::Task;
use iced::widget::text_editor::{self, Cursor, Position};

use super::{App, Doc, Message, Mode, SOURCE_EDITOR_ID};
use crate::live::Live;
use crate::tree::Dir;

impl App {
    pub(super) fn scan(&self) -> Task<Message> {
        match self.workspace.clone() {
            Some(ws) => Task::perform(async move { Dir::scan(&ws) }, Message::FilesScanned),
            None => Task::none(),
        }
    }

    pub(super) fn expand_ancestors(&mut self, rel: &Path) {
        let mut dir = rel.parent();
        while let Some(d) = dir.filter(|d| !d.as_os_str().is_empty()) {
            self.expanded.insert(d.to_path_buf());
            dir = d.parent();
        }
    }

    /// Load `text` into an editor for the current mode.
    pub(super) fn load(&mut self, text: &str, cursor: Position) -> Task<Message> {
        match self.mode {
            Mode::Live => {
                let (live, task) = Live::new(text, cursor);
                self.doc = Some(Doc::Live(live));
                task.map(Message::Live)
            }
            Mode::Source => {
                let mut content = text_editor::Content::with_text(text);
                content.move_to(Cursor {
                    position: cursor,
                    selection: None,
                });
                self.doc = Some(Doc::Source(content));
                iced::widget::operation::focus(SOURCE_EDITOR_ID)
            }
        }
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

    fn capture(&self) -> Option<super::history::Snapshot> {
        Some(super::history::Snapshot {
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
        kind: super::history::EditKind,
        apply: impl FnOnce(&mut Self) -> T,
    ) -> T {
        let before = self.capture().and_then(|snap| {
            let text = snap.text.clone();
            self.history.begin(kind, snap).then_some(text)
        });
        let result = apply(self);
        if let Some(before) = before {
            self.history
                .finish(self.text().as_deref() != Some(before.as_str()));
        }
        self.sync_dirty();
        result
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
        let Some(snap) = (if undo {
            self.history.undo(current)
        } else {
            self.history.redo(current)
        }) else {
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
        match (self.text(), self.cursor()) {
            (Some(text), Some(cursor)) => self.load(&text, cursor),
            _ => Task::none(),
        }
    }

    /// Follow a link clicked in the preview: other notes open in the app,
    /// everything else goes to the system handler.
    pub(super) fn open_link(&mut self, url: &str) -> Task<Message> {
        if url.contains("://") || url.starts_with("mailto:") {
            if let Err(e) = open_external(url) {
                self.notice = Some(format!("Could not open {url}: {e}"));
            } else {
                self.notice = None;
            }
            return Task::none();
        }

        let target = url
            .split('#')
            .next()
            .unwrap_or_default()
            .replace("%20", " ");
        if target.is_empty() {
            return Task::none();
        }
        let base = self
            .current
            .as_ref()
            .and_then(|c| c.parent())
            .map(Path::to_path_buf)
            .unwrap_or_default();
        let mut rel = normalize(&base.join(&target));
        if rel.extension().is_none() {
            rel.set_extension("md");
        }
        match &self.workspace {
            Some(ws) if ws.join(&rel).is_file() => Task::done(Message::Open(rel)),
            _ => {
                self.notice = Some(format!("Note not found: {}", rel.display()));
                Task::none()
            }
        }
    }

    pub(super) fn save(&mut self) {
        let (Some(ws), Some(rel), Some(text)) = (&self.workspace, &self.current, self.text())
        else {
            return;
        };
        match std::fs::write(ws.join(rel), &text) {
            Ok(()) => {
                self.saved = Some(text);
                self.dirty = false;
                self.notice = None;
            }
            Err(e) => self.notice = Some(format!("Save failed: {e}")),
        }
    }

    pub(super) fn save_if_dirty(&mut self) {
        if self.dirty {
            self.save();
        }
    }

    pub(super) fn create_note(&mut self) -> Task<Message> {
        let Some(ws) = self.workspace.clone() else {
            return Task::none();
        };
        let name = self.new_name.trim();
        if name.is_empty() {
            return Task::none();
        }
        let mut rel = PathBuf::from(name);
        if rel.is_absolute()
            || rel
                .components()
                .any(|c| c == std::path::Component::ParentDir)
        {
            self.notice = Some("Note name must stay inside the workspace.".into());
            return Task::none();
        }
        if rel.extension().is_none_or(|e| e != "md") {
            rel.set_extension("md");
        }

        let path = ws.join(&rel);
        if !path.exists() {
            let result = path
                .parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&path, ""));
            if let Err(e) = result {
                self.notice = Some(format!("Could not create {}: {e}", rel.display()));
                return Task::none();
            }
        }

        self.new_name.clear();
        self.filter.clear();
        self.tree.insert_file(&rel);
        Task::done(Message::Open(rel))
    }
}

/// Resolve `.` and `..` components without touching the filesystem.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::Normal(p) => out.push(p),
            _ => {}
        }
    }
    out
}

fn open_external(url: &str) -> std::io::Result<()> {
    let program = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::process::Command::new(program)
        .arg(url)
        .spawn()
        .map(drop)
}
