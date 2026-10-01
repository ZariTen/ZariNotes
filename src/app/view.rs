//! The window: editor surface and footer.

use iced::keyboard::{self, Key};
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{button, column, container, row, text, text_editor, tooltip};
use iced::{Element, Fill, Font};

use super::style::{
    chassis, editor_style, hint, line, panel, segment, segment_track, unsaved_style, word_count,
    words_label, writing_surface,
};
use super::{App, Doc, Message, Mode, SIDEBAR_WIDTH, SOURCE_EDITOR_ID};
use crate::highlight;
use crate::theme::{self, Appearance};

impl App {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let theme = theme::iced(self.appearance);
        let editor: Element<'_, Message> = match &self.doc {
            Some(Doc::Live(live)) => live.view(&theme).map(Message::Live),
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
                .style(editor_style)
                .into(),
            None => container(text("Select or create a note.").size(16))
                .center(Fill)
                .into(),
        };

        let page = container(editor)
            .padding([8.0, 12.0])
            .style(writing_surface)
            .clip(true)
            .width(Fill)
            .height(Fill);

        let main = column![page, self.footer()]
            .spacing(8)
            .width(Fill)
            .height(Fill);

        let body = row![
            container(self.sidebar())
                .style(panel)
                .clip(true)
                .width(SIDEBAR_WIDTH)
                .height(Fill),
            main
        ]
        .spacing(8)
        .height(Fill);

        container(body)
            .padding(8)
            .style(chassis)
            .width(Fill)
            .height(Fill)
            .into()
    }

    // ── helpers ─────────────────────────────────────────────────

    /// Quiet writing context: the open note, save state, word count, cursor,
    /// a Live / Source switch, and the light / dark theme switch.
    fn footer(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        let ink = look.ink;
        let muted = look.muted;

        let left: Element<'_, Message> = if let Some(err) = &self.notice {
            container(
                text(err)
                    .size(12)
                    .color(look.danger)
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
        right.push(self.theme_switch());

        row![
            container(left).width(Fill).clip(true),
            row(right).spacing(16)
        ]
        .spacing(16)
        .padding([2, 4])
        .align_y(iced::Center)
        .into()
    }

    fn theme_switch(&self) -> Element<'_, Message> {
        container(
            row![
                segment(
                    "Light",
                    self.appearance == Appearance::Light,
                    "Retro Classic — vintage beige and slate",
                    Message::SetAppearance(Appearance::Light),
                ),
                segment(
                    "Dark",
                    self.appearance == Appearance::Dark,
                    "Dolch Noir — charcoal and signal steel",
                    Message::SetAppearance(Appearance::Dark),
                ),
            ]
            .spacing(2),
        )
        .padding(2)
        .style(segment_track)
        .into()
    }

    fn mode_switch(&self) -> Element<'_, Message> {
        container(
            row![
                segment(
                    "Live",
                    self.mode == Mode::Live,
                    "Rendered notes. A selection shows source so you can copy it (Ctrl+E)",
                    Message::SetMode(Mode::Live),
                ),
                segment(
                    "Source",
                    self.mode == Mode::Source,
                    "Raw Markdown for the whole note (Ctrl+E)",
                    Message::SetMode(Mode::Source),
                ),
            ]
            .spacing(2),
        )
        .padding(2)
        .style(segment_track)
        .into()
    }
}

/// Ctrl+S saves, Ctrl+Z undoes, Ctrl+Shift+Z / Ctrl+Y redoes, Ctrl+E toggles
/// live preview, Tab inserts spaces.
fn editor_bindings(kp: KeyPress) -> Option<Binding<Message>> {
    if let Some(message) = shortcut(kp.key.as_ref(), kp.modifiers) {
        return Some(Binding::Custom(message));
    }
    if matches!(kp.key, Key::Named(keyboard::key::Named::Tab)) && kp.modifiers.is_empty() {
        return Some(Binding::Sequence(vec![Binding::Insert(' '); 4]));
    }
    Binding::from_key_press(kp)
}

pub(super) fn shortcut(
    key: iced::keyboard::Key<&str>,
    modifiers: iced::keyboard::Modifiers,
) -> Option<Message> {
    if !modifiers.command() {
        return None;
    }
    match key {
        Key::Character("s") => Some(Message::Save),
        Key::Character("e") => Some(Message::ToggleMode),
        Key::Character("z" | "Z") if modifiers.shift() => Some(Message::Redo),
        Key::Character("y" | "Y") if !modifiers.shift() => Some(Message::Redo),
        Key::Character("z" | "Z") => Some(Message::Undo),
        _ => None,
    }
}
