//! Application state and the update loop.

mod document;
mod history;
mod sidebar;
mod style;
mod view;

use std::collections::HashSet;
use std::path::PathBuf;

use iced::event::{self, Event};
use iced::font::Weight;
use iced::keyboard;
use iced::mouse;
use iced::widget::text_editor::{self, Position};
use iced::{Font, Subscription, Task};

use crate::config::{load_appearance, load_last_workspace, save_appearance, save_last_workspace};
use crate::live::{self, Live};
use crate::theme::{self, Appearance};
use crate::tree::Dir;

pub(crate) fn run() -> iced::Result {
    iced::application(App::new, App::update, App::view)
        .title(App::title)
        .subscription(App::subscription)
        .theme(|app: &App| theme::iced(app.appearance))
        .window(window_settings())
        .run()
}

/// Matches `zarinotes.desktop` so the launcher icon attaches to the window.
fn window_settings() -> iced::window::Settings {
    iced::window::Settings {
        size: iced::Size::new(1100.0, 720.0),
        #[cfg(target_os = "linux")]
        platform_specific: iced::window::settings::PlatformSpecific {
            application_id: "zarinotes".into(),
            ..Default::default()
        },
        ..Default::default()
    }
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
    /// Text last loaded or saved. Undo compares against it, so reverting a
    /// change clears the unsaved mark instead of leaving it stuck on.
    saved: Option<String>,
    history: history::History,
    new_name: String,
    /// Sidebar filter. Empty shows the full tree.
    filter: String,
    /// Problem worth showing in the footer. Routine success is not stored.
    notice: Option<String>,
    appearance: Appearance,
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
    SetMode(Mode),
    Save,
    Undo,
    Redo,
    NewNameChanged(String),
    FilterChanged(String),
    CreateNote,
    SetAppearance(Appearance),
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
            saved: None,
            history: history::History::default(),
            new_name: String::new(),
            filter: String::new(),
            notice: None,
            appearance: load_appearance(),
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
                self.saved = None;
                self.history.clear();
                self.tree = Dir::default();
                self.expanded.clear();
                self.filter.clear();
                self.notice = None;
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
                        self.notice = None;
                        self.expand_ancestors(&rel);
                        self.current = Some(rel);
                        self.history.clear();
                        self.dirty = false;
                        let task = self.load(&body, Position { line: 0, column: 0 });
                        // Compare undo against the editor's text, not the raw
                        // file, so a normalized newline doesn't look unsaved.
                        self.saved = self.text();
                        task
                    }
                    Err(e) => {
                        self.notice = Some(format!("Failed to open {}: {e}", rel.display()));
                        Task::none()
                    }
                }
            }
            Message::Edit(action) => {
                let selected = matches!(&self.doc, Some(Doc::Source(content)) if content.selection().is_some());
                match history::input_action(&action, selected) {
                    history::Input::Edit(kind) => {
                        self.record_edit(kind, |app| {
                            if let Some(Doc::Source(content)) = &mut app.doc {
                                content.perform(action);
                            }
                        });
                    }
                    history::Input::Moved => {
                        self.history.close();
                        if let Some(Doc::Source(content)) = &mut self.doc {
                            content.perform(action);
                        }
                    }
                    history::Input::Ignore => {
                        if let Some(Doc::Source(content)) = &mut self.doc {
                            content.perform(action);
                        }
                    }
                }
                Task::none()
            }
            Message::Live(msg) => {
                let selected = matches!(&self.doc, Some(Doc::Live(live)) if live.has_selection());
                let input = live_input(&msg, selected);
                let (task, outcome) = match input {
                    history::Input::Edit(kind) => self.record_edit(kind, |app| {
                        let Some(Doc::Live(live)) = &mut app.doc else {
                            return (Task::none(), live::Outcome::None);
                        };
                        live.update(msg)
                    }),
                    history::Input::Moved => {
                        self.history.close();
                        self.update_live(msg)
                    }
                    history::Input::Ignore => self.update_live(msg),
                };
                let task = task.map(Message::Live);
                match outcome {
                    live::Outcome::None => task,
                    live::Outcome::Changed => {
                        self.sync_dirty();
                        task
                    }
                    live::Outcome::Save => {
                        self.save();
                        task
                    }
                    live::Outcome::ToggleMode => self.toggle_mode(),
                    live::Outcome::Link(url) => Task::batch([task, self.open_link(&url)]),
                    live::Outcome::Undo => self.undo(),
                    live::Outcome::Redo => self.redo(),
                }
            }
            Message::ToggleMode => self.toggle_mode(),
            Message::SetMode(mode) => {
                if self.mode == mode || self.doc.is_none() {
                    return Task::none();
                }
                self.toggle_mode()
            }
            Message::Save => {
                self.save();
                Task::none()
            }
            Message::Undo => self.undo(),
            Message::Redo => self.redo(),
            Message::NewNameChanged(name) => {
                self.new_name = name;
                Task::none()
            }
            Message::FilterChanged(filter) => {
                self.filter = filter;
                Task::none()
            }
            Message::CreateNote => self.create_note(),
            Message::SetAppearance(appearance) => {
                if self.appearance != appearance {
                    self.appearance = appearance;
                    save_appearance(appearance);
                }
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        // Shortcuts when no editor is focused (the editors handle their own bindings).
        // Pointer events let live preview extend a selection past the active line.
        Subscription::batch([
            keyboard::listen().filter_map(|event| match event {
                keyboard::Event::KeyPressed { key, modifiers, .. } => {
                    view::shortcut(key.as_ref(), modifiers)
                }
                _ => None,
            }),
            event::listen_with(live_pointer),
        ])
    }
}

const SOURCE_EDITOR_ID: &str = "source-editor";
const SIDEBAR_WIDTH: f32 = 300.0;

const MEDIUM: Font = Font {
    weight: Weight::Medium,
    ..Font::DEFAULT
};

const LABEL: Font = Font {
    weight: Weight::Semibold,
    ..Font::DEFAULT
};

fn live_input(msg: &live::Msg, selected: bool) -> history::Input {
    match msg {
        live::Msg::Edit(action) => history::input_action(action, selected),
        // Joining a line is the backspace/delete that crossed a segment.
        live::Msg::MergeUp => history::Input::Edit(history::EditKind::Backspace),
        live::Msg::MergeDown => history::Input::Edit(history::EditKind::Delete),
        live::Msg::ToggleTask(_) => history::Input::Edit(history::EditKind::Other),
        live::Msg::Nav(_) | live::Msg::Activate(_) | live::Msg::DragEnd => history::Input::Moved,
        _ => history::Input::Ignore,
    }
}

fn live_pointer(
    event: Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
            Some(Message::Live(live::Msg::DragEnd))
        }
        Event::Keyboard(
            keyboard::Event::KeyPressed { modifiers, .. }
            | keyboard::Event::KeyReleased { modifiers, .. },
        ) => Some(Message::Live(live::Msg::Modifiers(modifiers))),
        _ => None,
    }
}
