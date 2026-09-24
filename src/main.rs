//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them with an
//! live preview (or as plain source).

mod highlight;
mod icons;
mod live;
mod tree;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use iced::keyboard::{self, Key};
use iced::widget::text_editor::{Binding, Cursor, KeyPress, Position};
use iced::widget::{
    button, column, container, row, rule, scrollable, space, text, text_editor, text_input,
};
use iced::{Element, Fill, Font, Padding, Subscription, Task, Theme};

use live::Live;
use tree::Dir;

const THEME: Theme = Theme::TokyoNight;

fn main() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(|_: &App| THEME)
        .window_size((1100.0, 720.0))
        .run()
}

struct App {
    workspace: Option<PathBuf>,
    /// Folder/file tree of the workspace.
    tree: Dir,
    /// Workspace-relative folders currently expanded in the sidebar.
    expanded: HashSet<PathBuf>,
    /// Currently open file, relative to the workspace root.
    current: Option<PathBuf>,
    /// The open note's editor, if any.
    doc: Option<Doc>,
    /// Preferred editing mode, kept across notes.
    mode: Mode,
    dirty: bool,
    new_name: String,
    status: String,
}

enum Doc {
    Live(Live),
    Source(text_editor::Content),
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Mode {
    Live,
    Source,
}

#[derive(Debug, Clone)]
enum Message {
    PickWorkspace,
    WorkspacePicked(Option<PathBuf>),
    Refresh,
    FilesScanned(Dir),
    ToggleDir(PathBuf),
    Open(PathBuf),
    Edit(text_editor::Action),
    Live(live::Msg),
    ToggleMode,
    Save,
    NewNameChanged(String),
    CreateNote,
}

impl App {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            workspace: None,
            tree: Dir::default(),
            expanded: HashSet::new(),
            current: None,
            doc: None,
            mode: Mode::Live,
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
                self.doc = None;
                self.dirty = false;
                self.tree = Dir::default();
                self.expanded.clear();
                self.status = format!("Workspace: {}", dir.display());
                save_last_workspace(&dir);
                self.workspace = Some(dir);
                self.scan()
            }
            Message::Refresh => self.scan(),
            Message::FilesScanned(tree) => {
                self.tree = tree;
                Task::none()
            }
            Message::ToggleDir(dir) => {
                if !self.expanded.remove(&dir) {
                    self.expanded.insert(dir);
                }
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
                        self.status = format!("Opened {}", rel.display());
                        self.expand_ancestors(&rel);
                        self.current = Some(rel);
                        self.dirty = false;
                        self.load(&body, Position { line: 0, column: 0 })
                    }
                    Err(e) => {
                        self.status = format!("Failed to open {}: {e}", rel.display());
                        Task::none()
                    }
                }
            }
            Message::Edit(action) => {
                if let Some(Doc::Source(content)) = &mut self.doc {
                    self.dirty |= action.is_edit();
                    content.perform(action);
                }
                Task::none()
            }
            Message::Live(msg) => {
                let Some(Doc::Live(live)) = &mut self.doc else {
                    return Task::none();
                };
                let (task, outcome) = live.update(msg);
                let task = task.map(Message::Live);
                match outcome {
                    live::Outcome::None => task,
                    live::Outcome::Changed => {
                        self.dirty = true;
                        task
                    }
                    live::Outcome::Save => {
                        self.save();
                        task
                    }
                    live::Outcome::ToggleMode => self.toggle_mode(),
                    live::Outcome::Link(url) => Task::batch([task, self.open_link(&url)]),
                }
            }
            Message::ToggleMode => self.toggle_mode(),
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

        let mut rows = Vec::new();
        self.tree_rows(&self.tree, Path::new(""), 0, &mut rows);
        if rows.is_empty() && self.workspace.is_some() {
            rows.push(text("No notes yet.").size(13).into());
        }
        let file_list = column(rows).spacing(1);

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
        let editor: Element<'_, Message> = match &self.doc {
            Some(Doc::Live(live)) => live.view(&THEME).map(Message::Live),
            Some(Doc::Source(content)) => text_editor(content)
                .id(SOURCE_EDITOR_ID)
                .placeholder("Start writing Markdown…")
                .on_action(Message::Edit)
                .highlight_with::<highlight::Highlighter>(
                    highlight::Settings { mono: true },
                    highlight::to_format,
                )
                .key_binding(editor_bindings)
                .font(Font::MONOSPACE)
                .size(15)
                .padding(16)
                .height(Fill)
                .into(),
            None => container(text("Select or create a note.").size(16))
                .center(Fill)
                .into(),
        };

        let status_bar = row![
            text(&self.status).size(13),
            space::horizontal(),
            text(match self.cursor() {
                Some(c) => format!("Ln {}, Col {}", c.line + 1, c.column + 1),
                None => String::new(),
            })
            .size(13),
            button(
                text(match self.mode {
                    Mode::Live => "Live preview",
                    Mode::Source => "Source",
                })
                .size(13)
            )
            .style(button::secondary)
            .on_press_maybe(self.doc.is_some().then_some(Message::ToggleMode)),
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
        // Shortcuts when no editor is focused (the editors handle their own bindings).
        keyboard::listen().filter_map(|event| match event {
            keyboard::Event::KeyPressed { key, modifiers, .. } if modifiers.command() => {
                match key.as_ref() {
                    Key::Character("s") => Some(Message::Save),
                    Key::Character("e") => Some(Message::ToggleMode),
                    _ => None,
                }
            }
            _ => None,
        })
    }

    // ── helpers ─────────────────────────────────────────────────

    fn scan(&self) -> Task<Message> {
        match self.workspace.clone() {
            Some(ws) => Task::perform(async move { Dir::scan(&ws) }, Message::FilesScanned),
            None => Task::none(),
        }
    }

    /// Flatten the visible part of `dir` into indented sidebar rows.
    fn tree_rows<'a>(
        &'a self,
        dir: &'a Dir,
        prefix: &Path,
        depth: u16,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        const INDENT: f32 = 14.0;
        let pad = Padding::from([3, 6]).left(6.0 + f32::from(depth) * INDENT);

        for (name, sub) in &dir.dirs {
            let path = prefix.join(name);
            let open = self.expanded.contains(&path);
            let arrow = if sub.is_empty() {
                " "
            } else if open {
                "▾"
            } else {
                "▸"
            };
            rows.push(
                button(
                    row![
                        text(arrow).size(13).width(12),
                        icons::folder(open),
                        text(name.as_str()).size(14)
                    ]
                    .spacing(6)
                    .align_y(iced::Center),
                )
                .width(Fill)
                .padding(pad)
                .style(button::text)
                .on_press(Message::ToggleDir(path.clone()))
                .into(),
            );
            if open {
                self.tree_rows(sub, &path, depth + 1, rows);
            }
        }

        for name in &dir.files {
            let path = prefix.join(name);
            let selected = self.current.as_ref() == Some(&path);
            let label = name.strip_suffix(".md").unwrap_or(name);
            rows.push(
                button(
                    row![
                        space().width(12),
                        icons::file(selected),
                        text(label).size(14)
                    ]
                    .spacing(6)
                    .align_y(iced::Center),
                )
                .width(Fill)
                .padding(pad)
                .style(if selected {
                    button::primary
                } else {
                    button::text
                })
                .on_press(Message::Open(path))
                .into(),
            );
        }
    }

    fn expand_ancestors(&mut self, rel: &Path) {
        let mut dir = rel.parent();
        while let Some(d) = dir.filter(|d| !d.as_os_str().is_empty()) {
            self.expanded.insert(d.to_path_buf());
            dir = d.parent();
        }
    }

    /// Load `text` into an editor for the current mode.
    fn load(&mut self, text: &str, cursor: Position) -> Task<Message> {
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

    fn text(&self) -> Option<String> {
        match self.doc.as_ref()? {
            Doc::Live(live) => Some(live.text()),
            Doc::Source(content) => Some(content.text()),
        }
    }

    fn cursor(&self) -> Option<Position> {
        match self.doc.as_ref()? {
            Doc::Live(live) => Some(live.cursor()),
            Doc::Source(content) => Some(content.cursor().position),
        }
    }

    fn toggle_mode(&mut self) -> Task<Message> {
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
    fn open_link(&mut self, url: &str) -> Task<Message> {
        if url.contains("://") || url.starts_with("mailto:") {
            if let Err(e) = open_external(url) {
                self.status = format!("Could not open {url}: {e}");
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
                self.status = format!("Note not found: {}", rel.display());
                Task::none()
            }
        }
    }

    fn save(&mut self) {
        let (Some(ws), Some(rel), Some(text)) = (&self.workspace, &self.current, self.text())
        else {
            return;
        };
        match std::fs::write(ws.join(rel), text) {
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
        self.tree.insert_file(&rel);
        Task::done(Message::Open(rel))
    }
}

const SOURCE_EDITOR_ID: &str = "source-editor";

/// Ctrl+S saves, Ctrl+E toggles live preview, Tab inserts spaces.
fn editor_bindings(kp: KeyPress) -> Option<Binding<Message>> {
    if kp.modifiers.command() {
        match kp.key.as_ref() {
            Key::Character("s") => return Some(Binding::Custom(Message::Save)),
            Key::Character("e") => return Some(Binding::Custom(Message::ToggleMode)),
            _ => {}
        }
    }
    if matches!(kp.key, Key::Named(keyboard::key::Named::Tab)) && kp.modifiers.is_empty() {
        return Some(Binding::Sequence(vec![Binding::Insert(' '); 4]));
    }
    Binding::from_key_press(kp)
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
