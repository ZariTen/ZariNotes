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

use iced::font::Weight;
use iced::keyboard::{self, Key};
use iced::widget::text_editor::{Binding, Cursor, KeyPress, Position};
use iced::widget::{
    button, column, container, row, rule, scrollable, space, text, text_editor, text_input, tooltip,
};
use iced::{Element, Fill, Font, Padding, Subscription, Task, Theme, border};

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
    /// Sidebar filter. Empty shows the full tree.
    filter: String,
    /// Problem worth showing in the footer. Routine success is not stored.
    notice: Option<String>,
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
    NewNameChanged(String),
    FilterChanged(String),
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
            filter: String::new(),
            notice: None,
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
                        self.dirty = false;
                        self.load(&body, Position { line: 0, column: 0 })
                    }
                    Err(e) => {
                        self.notice = Some(format!("Failed to open {}: {e}", rel.display()));
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
            Message::NewNameChanged(name) => {
                self.new_name = name;
                Task::none()
            }
            Message::FilterChanged(filter) => {
                self.filter = filter;
                Task::none()
            }
            Message::CreateNote => self.create_note(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
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

        let main = column![
            container(editor).padding([8.0, 12.0]).height(Fill),
            self.footer(),
        ]
        .spacing(0)
        .width(Fill)
        .height(Fill);

        row![
            container(self.sidebar())
                .style(sidebar_panel)
                .width(SIDEBAR_WIDTH)
                .height(Fill),
            rule::vertical(1),
            main
        ]
        .height(Fill)
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

    /// Quiet writing context: the open note, save state, word count, cursor,
    /// and a Live / Source switch. Errors replace the note name.
    fn footer(&self) -> Element<'_, Message> {
        let palette = THEME.extended_palette();
        let ink = palette.background.base.text;
        let muted = ink.scale_alpha(0.75);

        let left: Element<'_, Message> = if let Some(err) = &self.notice {
            container(
                text(err)
                    .size(12)
                    .color(palette.danger.base.color)
                    .wrapping(text::Wrapping::None),
            )
            .width(Fill)
            .clip(true)
            .into()
        } else if let Some(rel) = &self.current {
            let state: Element<'_, Message> = if self.dirty {
                tooltip(
                    button(text("Unsaved").size(12))
                        .padding([2, 8])
                        .style(unsaved_style)
                        .on_press(Message::Save),
                    hint("Save (Ctrl+S)"),
                    tooltip::Position::Top,
                )
                .gap(6)
                .delay(iced::time::Duration::from_millis(350))
                .into()
            } else {
                text("Saved").size(12).color(muted).into()
            };
            row![
                state,
                line(rel.display().to_string(), 12.0, Font::DEFAULT, ink),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into()
        } else {
            text("No note open").size(12).color(muted).into()
        };

        let mut right: Vec<Element<'_, Message>> = Vec::new();
        if self.doc.is_some() {
            let words = self.text().as_deref().map(word_count).unwrap_or(0);
            right.push(text(words_label(words)).size(12).color(muted).into());
            if let Some(cursor) = self.cursor() {
                right.push(
                    text(format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1))
                        .size(12)
                        .color(muted)
                        .into(),
                );
            }
            right.push(self.mode_switch());
        }

        container(
            column![
                rule::horizontal(1),
                row![
                    container(left).width(Fill).clip(true),
                    row(right).spacing(16)
                ]
                .spacing(16)
                .padding([8, 14])
                .align_y(iced::Center),
            ]
            .width(Fill),
        )
        .style(sidebar_panel)
        .width(Fill)
        .into()
    }

    fn mode_switch(&self) -> Element<'_, Message> {
        container(
            row![
                mode_segment(
                    "Live",
                    self.mode == Mode::Live,
                    "Rendered notes, except the line under the cursor (Ctrl+E)",
                    Mode::Live,
                ),
                mode_segment(
                    "Source",
                    self.mode == Mode::Source,
                    "Raw Markdown, for editing across lines (Ctrl+E)",
                    Mode::Source,
                ),
            ]
            .spacing(2),
        )
        .padding(2)
        .style(segment_track)
        .into()
    }

    fn sidebar(&self) -> Element<'_, Message> {
        let palette = THEME.extended_palette();
        let ink = palette.background.base.text;
        let muted = ink.scale_alpha(0.75);
        let label = ink.scale_alpha(0.75);

        let body: Element<'_, Message> = if let Some(ws) = &self.workspace {
            let name = ws
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| ws.display().to_string());
            let mut title = column![
                text("Workspace").size(12).font(LABEL).color(label),
                line(name, 15.0, MEDIUM, ink),
            ]
            .spacing(2)
            .width(Fill);
            if let Some(parent) = ws.parent().and_then(|p| p.file_name()) {
                title = title.push(line(
                    format!("in {}", parent.to_string_lossy()),
                    12.0,
                    Font::DEFAULT,
                    muted,
                ));
            }

            let filtering = !self.filter.trim().is_empty();
            let filter_input = text_input("Find a note…", &self.filter)
                .on_input(Message::FilterChanged)
                .padding([8, 10])
                .size(14)
                .width(Fill)
                .style(field_style);
            let filter_row: Element<'_, Message> = if filtering {
                row![
                    filter_input,
                    button(text("Clear").size(13))
                        .padding([8, 10])
                        .style(rounded_subtle)
                        .on_press(Message::FilterChanged(String::new())),
                ]
                .spacing(6)
                .align_y(iced::Center)
                .into()
            } else {
                filter_input.into()
            };

            let can_add = !self.new_name.trim().is_empty();
            let total = self.tree.file_count();
            let shown = self.tree.visible_file_count(&self.filter);
            let count = if filtering {
                format!("{shown} of {total}")
            } else {
                match total {
                    0 => "None yet".into(),
                    1 => "1 note".into(),
                    n => format!("{n} notes"),
                }
            };

            let mut rows = Vec::new();
            self.tree_rows(&self.tree, Path::new(""), 0, &self.filter, false, &mut rows);
            if rows.is_empty() {
                let message = if filtering {
                    format!("No notes match \"{}\".", self.filter.trim())
                } else {
                    "No notes yet. Type a name above and press Add.".into()
                };
                rows.push(
                    container(text(message).size(13).color(muted))
                        .padding([8, 4])
                        .into(),
                );
            }

            column![
                row![
                    title,
                    icon_button(
                        icons::open_folder(),
                        "Open folder",
                        Some(Message::PickWorkspace)
                    ),
                    icon_button(icons::refresh(), "Refresh", Some(Message::Refresh)),
                ]
                .spacing(6)
                .align_y(iced::Center),
                column![text("Filter").size(12).font(LABEL).color(label), filter_row,].spacing(4),
                column![
                    text("New note").size(12).font(LABEL).color(label),
                    row![
                        text_input("Name or folder/name", &self.new_name)
                            .on_input(Message::NewNameChanged)
                            .on_submit(Message::CreateNote)
                            .padding([8, 10])
                            .size(14)
                            .width(Fill)
                            .style(field_style),
                        button(text("Add").size(13))
                            .padding([8, 12])
                            .style(rounded_primary)
                            .on_press_maybe(can_add.then_some(Message::CreateNote)),
                    ]
                    .spacing(6)
                    .align_y(iced::Center),
                ]
                .spacing(4),
                rule::horizontal(1),
                row![
                    text("Notes").size(12).font(LABEL).color(label),
                    space::horizontal(),
                    text(count).size(12).color(muted),
                ]
                .align_y(iced::Center),
                scrollable(column(rows).spacing(2)).height(Fill),
            ]
            .spacing(14)
            .into()
        } else {
            column![
                text("Notes").size(18).font(MEDIUM),
                text("No folder open").size(14).color(muted),
                text("Open a folder to list its Markdown notes.")
                    .size(13)
                    .color(muted)
                    .width(Fill),
                button(text("Open folder").size(14))
                    .width(Fill)
                    .padding([10, 12])
                    .style(rounded_primary)
                    .on_press(Message::PickWorkspace),
            ]
            .spacing(10)
            .into()
        };

        container(body).padding(14).width(Fill).height(Fill).into()
    }

    fn scan(&self) -> Task<Message> {
        match self.workspace.clone() {
            Some(ws) => Task::perform(async move { Dir::scan(&ws) }, Message::FilesScanned),
            None => Task::none(),
        }
    }

    /// Flatten the visible part of `dir` into indented sidebar rows.
    ///
    /// An empty `query` follows the expanded-folder set. A query shows only
    /// matching branches, and a folder whose own name matches reveals all of
    /// its children.
    fn tree_rows<'a>(
        &'a self,
        dir: &'a Dir,
        prefix: &Path,
        depth: u16,
        query: &str,
        reveal_all: bool,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        const INDENT: f32 = 16.0;
        let pad = Padding::from([6, 8]).left(8.0 + f32::from(depth) * INDENT);
        let filtering = !query.trim().is_empty() && !reveal_all;

        for (name, sub) in &dir.dirs {
            let path = prefix.join(name);
            let name_match = Dir::name_hit(name, query);
            if filtering && !name_match && !sub.contains_match(query) {
                continue;
            }
            let open = if query.trim().is_empty() {
                self.expanded.contains(&path)
            } else {
                !sub.is_empty()
            };
            let chevron: Element<'a, Message> = if sub.is_empty() {
                space().width(14).into()
            } else {
                icons::chevron(open).into()
            };
            rows.push(tree_button(
                row![chevron, icons::folder(open), row_label(name, MEDIUM)]
                    .spacing(8)
                    .align_y(iced::Center),
                pad,
                false,
                Message::ToggleDir(path.clone()),
            ));
            if open {
                self.tree_rows(sub, &path, depth + 1, query, reveal_all || name_match, rows);
            }
        }

        for name in &dir.files {
            if filtering && !Dir::file_hit(name, query) {
                continue;
            }
            let path = prefix.join(name);
            let selected = self.current.as_ref() == Some(&path);
            let label = name.strip_suffix(".md").unwrap_or(name);
            let mut parts = vec![
                space().width(14).into(),
                icons::file(selected).into(),
                row_label(label, if selected { MEDIUM } else { Font::DEFAULT }),
            ];
            if selected && self.dirty {
                parts.push(
                    text("●")
                        .size(8)
                        .color(THEME.extended_palette().warning.base.color)
                        .into(),
                );
            }
            rows.push(tree_button(
                row(parts).spacing(8).align_y(iced::Center),
                pad,
                selected,
                Message::Open(path),
            ));
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

    fn save(&mut self) {
        let (Some(ws), Some(rel), Some(text)) = (&self.workspace, &self.current, self.text())
        else {
            return;
        };
        match std::fs::write(ws.join(rel), text) {
            Ok(()) => {
                self.dirty = false;
                self.notice = None;
            }
            Err(e) => self.notice = Some(format!("Save failed: {e}")),
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

fn line<'a>(
    content: impl Into<String>,
    size: f32,
    font: Font,
    color: impl Into<iced::Color>,
) -> Element<'a, Message> {
    container(
        text(content.into())
            .size(size)
            .font(font)
            .color(color)
            .wrapping(text::Wrapping::None),
    )
    .width(Fill)
    .clip(true)
    .into()
}

fn row_label<'a>(label: &'a str, font: Font) -> Element<'a, Message> {
    text(label)
        .size(14)
        .font(font)
        .width(Fill)
        .wrapping(text::Wrapping::WordOrGlyph)
        .into()
}

fn icon_button<'a>(
    icon: impl Into<Element<'a, Message>>,
    tip: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    tooltip(
        button(icon)
            .padding(7)
            .width(30)
            .height(30)
            .style(icon_button_style)
            .on_press_maybe(on_press),
        container(text(tip).size(12))
            .padding([4, 8])
            .style(container::rounded_box),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .delay(iced::time::Duration::from_millis(350))
    .into()
}

fn hint<'a>(tip: &'a str) -> Element<'a, Message> {
    container(text(tip).size(12))
        .padding([4, 8])
        .style(container::rounded_box)
        .into()
}

fn mode_segment<'a>(
    label: &'a str,
    active: bool,
    tip: &'a str,
    mode: Mode,
) -> Element<'a, Message> {
    tooltip(
        button(text(label).size(12))
            .padding([4, 10])
            .style(segment_style(active))
            .on_press(Message::SetMode(mode)),
        hint(tip),
        tooltip::Position::Top,
    )
    .gap(6)
    .delay(iced::time::Duration::from_millis(400))
    .into()
}

fn segment_track(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weak.color.into()),
        border: border::rounded(8),
        ..container::Style::default()
    }
}

fn segment_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let background = if active {
            Some(palette.primary.base.color.scale_alpha(0.2).into())
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => {
                    Some(palette.background.strong.color.into())
                }
                _ => None,
            }
        };
        button::Style {
            background,
            text_color: if active {
                palette.background.base.text
            } else {
                palette.background.base.text.scale_alpha(0.7)
            },
            border: border::rounded(6),
            ..button::Style::default()
        }
    }
}

fn unsaved_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::Style {
        text_color: palette.warning.base.color,
        border: border::rounded(6),
        ..button::Style::default()
    };
    if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(palette.warning.base.color.scale_alpha(0.16).into());
    }
    style
}

/// Whitespace-separated words. Markdown marks count as words; this is a writing
/// glance, not a rendered-text count.
fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn words_label(n: usize) -> String {
    match n {
        0 => "Empty".into(),
        1 => "1 word".into(),
        n => format!("{n} words"),
    }
}

fn tree_button<'a>(
    body: impl Into<Element<'a, Message>>,
    pad: Padding,
    selected: bool,
    message: Message,
) -> Element<'a, Message> {
    button(body)
        .width(Fill)
        .padding(pad)
        .style(tree_row_style(selected))
        .on_press(message)
        .into()
}

fn sidebar_panel(theme: &Theme) -> container::Style {
    let palette = theme.extended_palette();
    container::Style {
        background: Some(palette.background.weakest.color.into()),
        text_color: Some(palette.background.base.text),
        ..container::Style::default()
    }
}

fn field_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let mut style = text_input::default(theme, status);
    style.border.radius = 8.0.into();
    style
}

fn rounded_primary(theme: &Theme, status: button::Status) -> button::Style {
    let mut style = button::primary(theme, status);
    style.border = style.border.rounded(8);
    style
}

fn rounded_subtle(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::Style {
        text_color: palette.background.base.text,
        background: Some(palette.background.weaker.color.into()),
        border: border::rounded(8),
        ..button::Style::default()
    };
    if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(palette.background.strong.color.into());
    }
    if status == button::Status::Disabled {
        style.text_color = style.text_color.scale_alpha(0.4);
    }
    style
}

fn icon_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let palette = theme.extended_palette();
    let mut style = button::Style {
        text_color: palette.background.base.text,
        border: border::rounded(8),
        ..button::Style::default()
    };
    match status {
        button::Status::Hovered => {
            style.background = Some(palette.background.weak.color.into());
        }
        button::Status::Pressed => {
            style.background = Some(palette.background.strong.color.into());
        }
        button::Status::Disabled => {
            style.text_color = style.text_color.scale_alpha(0.35);
        }
        button::Status::Active => {}
    }
    style
}

fn tree_row_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let palette = theme.extended_palette();
        let background = if selected {
            let alpha = match status {
                button::Status::Hovered | button::Status::Pressed => 0.28,
                _ => 0.16,
            };
            Some(palette.primary.base.color.scale_alpha(alpha).into())
        } else {
            match status {
                button::Status::Hovered => Some(palette.background.weak.color.into()),
                button::Status::Pressed => Some(palette.background.strong.color.into()),
                _ => None,
            }
        };
        button::Style {
            background,
            text_color: palette.background.base.text,
            border: border::rounded(6),
            ..button::Style::default()
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::{word_count, words_label};

    #[test]
    fn word_count_splits_on_whitespace() {
        assert_eq!(word_count(""), 0);
        assert_eq!(word_count("  hello   world\n"), 2);
        assert_eq!(word_count("one"), 1);
        assert_eq!(words_label(0), "Empty");
        assert_eq!(words_label(1), "1 word");
        assert_eq!(words_label(3), "3 words");
    }
}
