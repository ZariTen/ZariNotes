//! Workspace sidebar: folder tree, filter, and new-note field.

use std::path::Path;

use iced::widget::{button, column, container, row, rule, scrollable, space, text, text_input};
use iced::{Element, Fill, Font, Padding};

use super::style::{
    field_style, icon_button, line, rounded_primary, rounded_subtle, row_label, tree_button,
};
use super::{App, LABEL, MEDIUM, Message};
use crate::icons;
use crate::theme;
use crate::tree::Dir;

impl App {
    pub(super) fn sidebar(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        let ink = look.ink;
        let muted = look.muted;
        let label = look.muted;

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
                        .color(theme::tokens(self.appearance).warning)
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
}
