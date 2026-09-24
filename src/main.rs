//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them as plain text.

use std::path::{Path, PathBuf};

use iced::keyboard::{self, Key};
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{
    button, column, container, row, rule, scrollable, space, text, text_editor, text_input,
};
use iced::{Element, Fill, Font, Subscription, Task, Theme};

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(|_: &App| Theme::TokyoNight)
        .window_size((1100.0, 720.0))
        .run()
}

struct App {
    workspace: Option<PathBuf>,
    /// Markdown files in the workspace, relative to its root, sorted.
    files: Vec<PathBuf>,
    /// Currently open file, relative to the workspace root.
    current: Option<PathBuf>,
    content: text_editor::Content,
    dirty: bool,
    new_name: String,
    status: String,
}

#[derive(Debug, Clone)]
enum Message {
    PickWorkspace,
    WorkspacePicked(Option<PathBuf>),
    Refresh,
    FilesScanned(Vec<PathBuf>),
    Open(PathBuf),
    Edit(text_editor::Action),
    Save,
    NewNameChanged(String),
    CreateNote,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            workspace: None,
            files: Vec::new(),
            current: None,
            content: text_editor::Content::new(),
            dirty: false,
            new_name: String::new(),
            status: "Open a workspace folder to begin.".into(),
        };

        // Reopen the last workspace, if it still exists.
        let task = match load_last_workspace() {
            Some(dir) if dir.is_dir() => Task::done(Message::WorkspacePicked(Some(dir))),
            _ => Task::none(),
        };
        (app, task)
    }

    fn title(&self) -> String {
        match &self.current {
            Some(file) => format!(
                "{}{} — ZariNotes",
                if self.dirty { "• " } else { "" },
                file.display()
            ),
            None => "ZariNotes".into(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickWorkspace => Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Select workspace folder")
                        .pick_folder()
                        .await
                        .map(|h| h.path().to_path_buf())
                },
                Message::WorkspacePicked,
            ),
            Message::WorkspacePicked(None) => Task::none(),
            Message::WorkspacePicked(Some(dir)) => {
                self.save_if_dirty();
                self.current = None;
                self.content = text_editor::Content::new();
                self.dirty = false;
                self.status = format!("Workspace: {}", dir.display());
                save_last_workspace(&dir);
                self.workspace = Some(dir);
                self.scan()
            }
            Message::Refresh => self.scan(),
            Message::FilesScanned(files) => {
                self.files = files;
                Task::none()
            }
            Message::Open(rel) => {
                if self.current.as_ref() == Some(&rel) {
                    return Task::none();
                }
                self.save_if_dirty();
                let Some(ws) = &self.workspace else {
                    return Task::none();
                };
                match std::fs::read_to_string(ws.join(&rel)) {
                    Ok(body) => {
                        self.content = text_editor::Content::with_text(&body);
                        self.status = format!("Opened {}", rel.display());
                        self.current = Some(rel);
                        self.dirty = false;
                    }
                    Err(e) => self.status = format!("Failed to open {}: {e}", rel.display()),
                }
                Task::none()
            }
            Message::Edit(action) => {
                if self.current.is_some() {
                    self.dirty |= action.is_edit();
                    self.content.perform(action);
                }
                Task::none()
            }
            Message::Save => {
                self.save();
                Task::none()
            }
            Message::NewNameChanged(name) => {
                self.new_name = name;
                Task::none()
            }
            Message::CreateNote => self.create_note(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        // ── Sidebar ──────────────────────────────────────────────
        let ws_label = self
            .workspace
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "No workspace".into());

        let file_list = column(self.files.iter().map(|rel| {
            let selected = self.current.as_ref() == Some(rel);
            button(text(rel.display().to_string()).size(14))
                .width(Fill)
                .style(if selected {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::Open(rel.clone()))
                .into()
        }))
        .spacing(2);

        let new_note = text_input("new-note.md", &self.new_name)
            .on_input_maybe(self.workspace.is_some().then_some(Message::NewNameChanged))
            .on_submit(Message::CreateNote)
            .size(14);

        let sidebar = column![
            row![
                button("Open…").on_press(Message::PickWorkspace),
                button("⟳")
                    .style(button::secondary)
                    .on_press_maybe(self.workspace.is_some().then_some(Message::Refresh)),
            ]
            .spacing(6),
            text(ws_label).size(16),
            new_note,
            rule::horizontal(1),
            scrollable(file_list).height(Fill),
        ]
        .spacing(10)
        .padding(10)
        .width(260);

        // ── Editor ──────────────────────────────────────────────
        let editor: Element<'_, Message> = if self.current.is_some() {
            text_editor(&self.content)
                .placeholder("Start writing Markdown…")
                .on_action(Message::Edit)
                .key_binding(editor_bindings)
                .font(Font::MONOSPACE)
                .size(15)
                .padding(16)
                .height(Fill)
                .into()
        } else {
            container(text("Select or create a note.").size(16))
                .center(Fill)
                .into()
        };

        let status_bar = row![
            text(&self.status).size(13),
            space::horizontal(),
            text(if self.current.is_some() {
                let c = self.content.cursor().position;
                format!("Ln {}, Col {}", c.line + 1, c.column + 1)
            } else {
                String::new()
            })
            .size(13),
            button(text("Save").size(13))
                .on_press_maybe((self.dirty && self.current.is_some()).then_some(Message::Save)),
        ]
        .spacing(12)
        .align_y(iced::Center);

        let main = column![editor, status_bar].spacing(8).padding(10);

        row![
            container(sidebar)
                .style(container::rounded_box)
                .height(Fill),
            main
        ]
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        // Ctrl+S when the editor isn't focused (the editor handles its own binding).
        keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. }
                if modifiers.command() && key == Key::Character("s".into()) =>
            {
                Some(Message::Save)
            }
            _ => None,
        })
    }

    // ── helpers ─────────────────────────────────────────────────

    fn scan(&self) -> Task<Message> {
        match self.workspace.clone() {
            Some(ws) => Task::perform(async move { scan_markdown(&ws) }, Message::FilesScanned),
            None => Task::none(),
        }
    }

    fn save(&mut self) {
        let (Some(ws), Some(rel)) = (&self.workspace, &self.current) else {
            return;
        };
        match std::fs::write(ws.join(rel), self.content.text()) {
            Ok(()) => {
                self.dirty = false;
                self.status = format!("Saved {}", rel.display());
            }
            Err(e) => self.status = format!("Save failed: {e}"),
        }
    }

    fn save_if_dirty(&mut self) {
        if self.dirty {
            self.save();
        }
    }

    fn create_note(&mut self) -> Task<Message> {
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
            self.status = "Note name must stay inside the workspace.".into();
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
                self.status = format!("Could not create {}: {e}", rel.display());
                return Task::none();
            }
        }

        self.new_name.clear();
        if !self.files.contains(&rel) {
            self.files.push(rel.clone());
            self.files.sort();
        }
        Task::done(Message::Open(rel))
    }
}

/// Ctrl+S saves from inside the editor; Tab inserts spaces; everything else is default.
fn editor_bindings(kp: KeyPress) -> Option<Binding<Message>> {
    if kp.modifiers.command() && kp.key == Key::Character("s".into()) {
        return Some(Binding::Custom(Message::Save));
    }
    if matches!(kp.key, Key::Named(keyboard::key::Named::Tab)) && kp.modifiers.is_empty() {
        return Some(Binding::Sequence(vec![Binding::Insert(' '); 4]));
    }
    Binding::from_key_press(kp)
}

/// Recursively collect `.md` files (skipping hidden entries), relative to `root`.
fn scan_markdown(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            let Ok(ft) = entry.file_type() else { continue };
            if ft.is_dir() {
                stack.push(path);
            } else if ft.is_file()
                && path.extension().is_some_and(|e| e == "md")
                && let Ok(rel) = path.strip_prefix(root)
            {
                out.push(rel.to_path_buf());
            }
        }
    }
    out.sort();
    out
}

fn config_file() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("zarinotes").join("last_workspace"))
}

fn load_last_workspace() -> Option<PathBuf> {
    let s = std::fs::read_to_string(config_file()?).ok()?;
    Some(PathBuf::from(s.trim()))
}

fn save_last_workspace(dir: &Path) {
    if let Some(file) = config_file() {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(file, dir.to_string_lossy().as_bytes());
    }
}
