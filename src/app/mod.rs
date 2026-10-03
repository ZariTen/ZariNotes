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
use iced::widget::text_editor::{self, Action};
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
    Edit(Action),
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
            Message::PickWorkspace => self.pick_workspace(),
            Message::WorkspacePicked(None) => Task::none(),
            Message::WorkspacePicked(Some(dir)) => self.open_workspace(dir),
            Message::Refresh => self.scan(),
            Message::FilesScanned(tree) => {
                self.tree = tree;
                Task::none()
            }
            Message::ToggleDir(dir) => self.toggle_dir(dir),
            Message::Open(rel) => self.open_file(rel),
            Message::Edit(action) => self.edit_source(action),
            Message::Live(msg) => self.edit_live(msg),
            Message::ToggleMode => self.toggle_mode(),
            Message::SetMode(mode) => self.set_mode(mode),
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
            Message::SetAppearance(appearance) => self.set_appearance(appearance),
        }
    }

    fn pick_workspace(&self) -> Task<Message> {
        Task::perform(
            async {
                rfd::AsyncFileDialog::new()
                    .set_title("Select workspace folder")
                    .pick_folder()
                    .await
                    .map(|handle| handle.path().to_path_buf())
            },
            Message::WorkspacePicked,
        )
    }

    fn open_workspace(&mut self, dir: PathBuf) -> Task<Message> {
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

    fn toggle_dir(&mut self, dir: PathBuf) -> Task<Message> {
        if !self.expanded.remove(&dir) {
            self.expanded.insert(dir);
        }
        Task::none()
    }

    fn set_mode(&mut self, mode: Mode) -> Task<Message> {
        if self.mode == mode || self.doc.is_none() {
            return Task::none();
        }
        self.toggle_mode()
    }

    fn set_appearance(&mut self, appearance: Appearance) -> Task<Message> {
        if self.appearance == appearance {
            return Task::none();
        }
        self.appearance = appearance;
        save_appearance(appearance);
        Task::none()
    }

    fn edit_source(&mut self, action: Action) -> Task<Message> {
        match history::input_action(&action, self.source_has_selection()) {
            history::Input::Edit(kind) => {
                self.record_edit(kind, |app| app.perform_source(action));
            }
            history::Input::Moved => {
                self.history.close();
                self.perform_source(action);
            }
            history::Input::Ignore => self.perform_source(action),
        }
        Task::none()
    }

    fn perform_source(&mut self, action: Action) {
        let Some(Doc::Source(content)) = &mut self.doc else {
            return;
        };
        content.perform(action);
    }

    fn source_has_selection(&self) -> bool {
        match &self.doc {
            Some(Doc::Source(content)) => content.selection().is_some(),
            _ => false,
        }
    }

    fn edit_live(&mut self, msg: live::Msg) -> Task<Message> {
        let input = live_input(&msg, self.live_has_selection());
        let (task, outcome) = self.run_live(msg, input);
        self.finish_live(task, outcome)
    }

    fn run_live(
        &mut self,
        msg: live::Msg,
        input: history::Input,
    ) -> (Task<live::Msg>, live::Outcome) {
        match input {
            history::Input::Edit(kind) => self.record_edit(kind, |app| app.update_live(msg)),
            history::Input::Moved => {
                self.history.close();
                self.update_live(msg)
            }
            history::Input::Ignore => self.update_live(msg),
        }
    }

    fn finish_live(&mut self, task: Task<live::Msg>, outcome: live::Outcome) -> Task<Message> {
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

    fn live_has_selection(&self) -> bool {
        match &self.doc {
            Some(Doc::Live(live)) => live.has_selection(),
            _ => false,
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
