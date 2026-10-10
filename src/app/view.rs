//! The window: editor surface and footer.

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::text_editor::{Binding, Content, KeyPress};
use iced::widget::{button, column, container, row, text, text_editor, tooltip};
use iced::{Element, Fill, Font};

use super::style::{
    chassis, editor_style, hint, line, panel, segment, segment_track, unsaved_style, word_count,
    words_label, writing_surface,
};
use super::{App, Doc, Message, Mode, SIDEBAR_WIDTH, SOURCE_EDITOR_ID};
use crate::highlight;
use crate::live::Live;
use crate::theme::{self, Appearance};

impl App {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let page = container(self.editor_pane())
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

        self.layer_popup(
            container(body)
                .padding(8)
                .style(chassis)
                .width(Fill)
                .height(Fill),
        )
    }

    fn editor_pane(&self) -> Element<'_, Message> {
        match &self.doc {
            Some(Doc::Live(live)) => {
                let note_dir = self.note_dir();
                live_editor(live, self.appearance, note_dir.as_deref())
            }
            Some(Doc::Source(content)) => source_editor(content),
            None => empty_editor(),
        }
    }

    /// Quiet writing context: the open note, save state, word count, cursor,
    /// a Live / Source switch, and the light / dark theme switch.
    fn footer(&self) -> Element<'_, Message> {
        row![
            container(self.footer_status()).width(Fill).clip(true),
            row(self.footer_stats()).spacing(16)
        ]
        .spacing(16)
        .padding([2, 4])
        .align_y(iced::Center)
        .into()
    }

    fn footer_status(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        if let Some(err) = &self.notice {
            return line(err.clone(), 12.0, Font::DEFAULT, look.danger);
        }
        if let Some(rel) = &self.current {
            return row![
                self.save_state(),
                line(rel.display().to_string(), 12.0, Font::DEFAULT, look.ink),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into();
        }
        text("No note open").size(12).color(look.muted).into()
    }

    fn save_state(&self) -> Element<'_, Message> {
        let muted = theme::tokens(self.appearance).muted;
        if !self.dirty {
            return text("Saved").size(12).color(muted).into();
        }
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
    }

    fn footer_stats(&self) -> Vec<Element<'_, Message>> {
        let muted = theme::tokens(self.appearance).muted;
        let mut stats = Vec::new();
        if self.doc.is_some() {
            let words = self.text().as_deref().map(word_count).unwrap_or(0);
            stats.push(text(words_label(words)).size(12).color(muted).into());
            if let Some(cursor) = self.cursor() {
                stats.push(
                    text(format!("Ln {}, Col {}", cursor.line + 1, cursor.column + 1))
                        .size(12)
                        .color(muted)
                        .into(),
                );
            }
            stats.push(self.mode_switch());
        }
        stats.push(self.theme_switch());
        stats
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

fn live_editor<'a>(
    live: &'a Live,
    appearance: Appearance,
    note_dir: Option<&std::path::Path>,
) -> Element<'a, Message> {
    let theme = theme::iced(appearance);
    live.view(&theme, note_dir).map(Message::Live)
}

fn source_editor(content: &Content) -> Element<'_, Message> {
    text_editor(content)
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
        .into()
}

fn empty_editor() -> Element<'static, Message> {
    container(text("Select or create a note.").size(16))
        .center(Fill)
        .into()
}

/// Ctrl+S saves, Ctrl+Z undoes, Ctrl+Shift+Z / Ctrl+Y redoes, Ctrl+E toggles
/// live preview, Tab inserts spaces.
fn editor_bindings(key_press: KeyPress) -> Option<Binding<Message>> {
    if let Some(message) = shortcut(key_press.key.as_ref(), key_press.modifiers) {
        return Some(Binding::Custom(message));
    }
    if matches!(key_press.key, Key::Named(keyboard::key::Named::Tab))
        && key_press.modifiers.is_empty()
    {
        return Some(Binding::Sequence(vec![Binding::Insert(' '); 4]));
    }
    Binding::from_key_press(key_press)
}

pub(super) fn shortcut(key: keyboard::Key<&str>, modifiers: Modifiers) -> Option<Message> {
    if !modifiers.command() {
        return None;
    }
    match key {
        Key::Character("s") => Some(Message::Save),
        Key::Character("e") => Some(Message::ToggleMode),
        // Shift+Z is redo. It has to be tried before plain Z, or undo would steal it.
        Key::Character("z" | "Z") if modifiers.shift() => Some(Message::Redo),
        Key::Character("y" | "Y") if !modifiers.shift() => Some(Message::Redo),
        Key::Character("z" | "Z") => Some(Message::Undo),
        Key::Character("v") if !modifiers.alt() => Some(Message::Paste),
        _ => None,
    }
}
