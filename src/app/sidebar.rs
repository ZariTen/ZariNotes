//! Workspace sidebar: search, folder tree, and a right-click create menu.

use std::path::{Path, PathBuf};

use iced::widget::{
    button, column, container, float, mouse_area, row, rule, scrollable, space, stack, text,
    text_input,
};
use iced::{Element, Fill, Font, Padding, Point, Rectangle, Vector};

use super::spot::spot;
use super::style::{
    field_style, icon_button, line, menu_card, menu_item_style, rounded_primary, rounded_subtle,
    row_label, tree_button,
};
use super::{App, CREATE_NAME_ID, CreateKind, CreatePrompt, CreateStep, LABEL, MEDIUM, Message};
use crate::icons;
use crate::theme;
use crate::tree::Dir;

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let body = if self.workspace.is_some() {
            self.open_sidebar()
        } else {
            self.closed_sidebar()
        };
        container(body).padding(14).width(Fill).height(Fill).into()
    }

    /// Search bar and the note tree. A right-click opens a popup at the cursor.
    fn open_sidebar(&self) -> Element<'_, Message> {
        let tree = column![
            self.workspace_header(),
            self.search_bar(),
            rule::horizontal(1),
            self.notes_header(),
            scrollable(column(self.note_rows()).spacing(2)).height(Fill),
        ]
        .spacing(14)
        .width(Fill)
        .height(Fill);
        spot(tree, |at| Message::AskCreate {
            parent: PathBuf::new(),
            at,
        })
    }

    fn closed_sidebar(&self) -> Element<'_, Message> {
        let muted = theme::tokens(self.appearance).muted;
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
    }

    fn workspace_header(&self) -> Element<'_, Message> {
        let Some(workspace) = &self.workspace else {
            return column![].into();
        };
        row![
            workspace_title(workspace, self.appearance),
            icon_button(
                icons::open_folder(),
                "Open folder",
                Some(Message::PickWorkspace)
            ),
            icon_button(icons::refresh(), "Refresh", Some(Message::Refresh)),
        ]
        .spacing(6)
        .align_y(iced::Center)
        .into()
    }

    fn search_bar(&self) -> Element<'_, Message> {
        let input = text_input("Find a note…", &self.filter)
            .on_input(Message::FilterChanged)
            .padding([8, 10])
            .size(14)
            .width(Fill)
            .style(field_style);
        if self.filter.trim().is_empty() {
            return input.into();
        }
        row![
            input,
            button(text("Clear").size(13))
                .padding([8, 10])
                .style(rounded_subtle)
                .on_press(Message::FilterChanged(String::new())),
        ]
        .spacing(6)
        .align_y(iced::Center)
        .into()
    }

    /// Popup at the right-click, drawn above the rest of the window.
    pub(super) fn layer_popup<'a>(
        &'a self,
        window: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let window = window.into();
        let Some(prompt) = &self.create else {
            return window;
        };
        stack![
            window,
            mouse_area(space().width(Fill).height(Fill)).on_press(Message::DismissCreate),
            self.popup(prompt),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn popup(&self, prompt: &CreatePrompt) -> Element<'_, Message> {
        let at = prompt.at;
        let menu = container(self.popup_body(prompt))
            .padding(4)
            .width(180)
            .style(menu_card);
        float(menu)
            .translate(move |bounds, viewport| place_popup(at, bounds, viewport))
            .into()
    }

    fn popup_body(&self, prompt: &CreatePrompt) -> Element<'_, Message> {
        match prompt.step {
            CreateStep::Choose => column![
                popup_item("New Note", Message::PickCreate(CreateKind::Note)),
                popup_item("New Folder", Message::PickCreate(CreateKind::Folder)),
            ]
            .spacing(2)
            .into(),
            CreateStep::Name(kind) => self.create_name(kind, prompt),
        }
    }

    fn create_name(&self, kind: CreateKind, prompt: &CreatePrompt) -> Element<'_, Message> {
        let can_add = !prompt.name.trim().is_empty();
        row![
            text_input(name_placeholder(kind), &prompt.name)
                .id(CREATE_NAME_ID)
                .on_input(Message::CreateNameChanged)
                .on_submit(Message::SubmitCreate)
                .padding([8, 10])
                .size(14)
                .width(Fill)
                .style(field_style),
            button(text("Create").size(13))
                .padding([8, 12])
                .style(rounded_primary)
                .on_press_maybe(can_add.then_some(Message::SubmitCreate)),
        ]
        .spacing(6)
        .align_y(iced::Center)
        .into()
    }

    fn notes_header(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        row![
            text("Notes").size(12).font(LABEL).color(look.muted),
            space::horizontal(),
            text(note_count(&self.filter, &self.tree))
                .size(12)
                .color(look.muted),
        ]
        .align_y(iced::Center)
        .into()
    }

    fn note_rows(&self) -> Vec<Element<'_, Message>> {
        let mut rows = Vec::new();
        self.tree_rows(&self.tree, Path::new(""), 0, &self.filter, false, &mut rows);
        if rows.is_empty() {
            rows.push(empty_tree_message(&self.filter, self.appearance));
        }
        rows
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
        for (name, sub) in &dir.dirs {
            if hide_folder(name, sub, query, reveal_all) {
                continue;
            }
            let path = prefix.join(name);
            let open = self.folder_open(&path, sub, query);
            self.push_folder_row(name, sub, &path, depth, open, rows);
            if open {
                let name_match = Dir::name_hit(name, query);
                self.tree_rows(sub, &path, depth + 1, query, reveal_all || name_match, rows);
            }
        }

        for name in &dir.files {
            if hide_file(name, query, reveal_all) {
                continue;
            }
            self.push_file_row(name, prefix, depth, rows);
        }
    }

    fn folder_open(&self, path: &Path, folder: &Dir, query: &str) -> bool {
        if query.trim().is_empty() {
            return self.expanded.contains(path);
        }
        !folder.is_empty()
    }

    fn push_folder_row<'a>(
        &'a self,
        name: &'a str,
        folder: &'a Dir,
        path: &Path,
        depth: u16,
        open: bool,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
        let chevron: Element<'a, Message> = if folder.is_empty() {
            space().width(14).into()
        } else {
            icons::chevron(open).into()
        };
        let parent = path.to_path_buf();
        rows.push(spot(
            tree_button(
                row![chevron, icons::folder(open), row_label(name, MEDIUM)]
                    .spacing(8)
                    .align_y(iced::Center),
                row_pad(depth),
                false,
                Message::ToggleDir(path.to_path_buf()),
            ),
            move |at| Message::AskCreate {
                parent: parent.clone(),
                at,
            },
        ));
    }

    fn push_file_row<'a>(
        &'a self,
        name: &'a str,
        prefix: &Path,
        depth: u16,
        rows: &mut Vec<Element<'a, Message>>,
    ) {
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
                    .color(theme::tokens(self.appearance).warning)
                    .into(),
            );
        }
        let parent = prefix.to_path_buf();
        rows.push(spot(
            tree_button(
                row(parts).spacing(8).align_y(iced::Center),
                row_pad(depth),
                selected,
                Message::Open(path),
            ),
            move |at| Message::AskCreate {
                parent: parent.clone(),
                at,
            },
        ));
    }
}

fn popup_item(label: &'static str, message: Message) -> Element<'static, Message> {
    button(text(label).size(14))
        .width(Fill)
        .padding([6, 10])
        .style(menu_item_style)
        .on_press(message)
        .into()
}

fn place_popup(at: Point, bounds: Rectangle, viewport: Rectangle) -> Vector {
    let x =
        at.x.min(viewport.x + viewport.width - bounds.width)
            .max(viewport.x);
    let y =
        at.y.min(viewport.y + viewport.height - bounds.height)
            .max(viewport.y);
    Vector::new(x - bounds.x, y - bounds.y)
}

fn name_placeholder(kind: CreateKind) -> &'static str {
    match kind {
        CreateKind::Note => "Note name",
        CreateKind::Folder => "Folder name",
    }
}

fn workspace_title(workspace: &Path, appearance: theme::Appearance) -> Element<'static, Message> {
    let look = theme::tokens(appearance);
    let mut title = column![
        text("Workspace").size(12).font(LABEL).color(look.muted),
        line(folder_name(workspace), 15.0, MEDIUM, look.ink),
    ]
    .spacing(2)
    .width(Fill);
    if let Some(parent) = parent_folder_name(workspace) {
        title = title.push(line(
            format!("in {parent}"),
            12.0,
            Font::DEFAULT,
            look.muted,
        ));
    }
    title.into()
}

fn folder_name(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

fn parent_folder_name(path: &Path) -> Option<String> {
    let parent = path.parent()?.file_name()?;
    Some(parent.to_string_lossy().into_owned())
}

fn note_count(filter: &str, tree: &Dir) -> String {
    let total = tree.file_count();
    if !filter.trim().is_empty() {
        let shown = tree.visible_file_count(filter);
        return format!("{shown} of {total}");
    }
    match total {
        0 => "None yet".into(),
        1 => "1 note".into(),
        n => format!("{n} notes"),
    }
}

fn empty_tree_message(filter: &str, appearance: theme::Appearance) -> Element<'static, Message> {
    let muted = theme::tokens(appearance).muted;
    let message = if filter.trim().is_empty() {
        "No notes yet. Right-click to create a note or folder.".into()
    } else {
        format!("No notes match \"{}\".", filter.trim())
    };
    container(text(message).size(13).color(muted))
        .padding([8, 4])
        .into()
}

fn hide_folder(name: &str, folder: &Dir, query: &str, reveal_all: bool) -> bool {
    if query.trim().is_empty() || reveal_all {
        return false;
    }
    !Dir::name_hit(name, query) && !folder.contains_match(query)
}

fn hide_file(name: &str, query: &str, reveal_all: bool) -> bool {
    if query.trim().is_empty() || reveal_all {
        return false;
    }
    !Dir::file_hit(name, query)
}

fn row_pad(depth: u16) -> Padding {
    const INDENT: f32 = 16.0;
    Padding::from([6, 8]).left(8.0 + f32::from(depth) * INDENT)
}
