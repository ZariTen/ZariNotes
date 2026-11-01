//! The window: editor surface and footer.

use iced::keyboard::{self, Key, Modifiers};
use iced::widget::text_editor::{Binding, Content, KeyPress};
use iced::widget::{
    button, column, container, row, rule, scrollable, space, text, text_editor, tooltip,
};
use iced::{Element, Fill, Font};

use super::style::{
    backlink_row, chassis, editor_style, line, panel, quiet_button, segment, switch, unsaved_style,
    with_tip, word_count, words_label, writing_surface,
};
use super::{App, Doc, MEDIUM, Message, Mode, SIDEBAR_WIDTH, SOURCE_EDITOR_ID};
use crate::highlight;
use crate::live::{self, Live};
use crate::theme::{self, Appearance};

impl App {
    pub(super) fn view(&self) -> Element<'_, Message> {
        let content = if self.doc.is_some() {
            column![
                self.editor_header(),
                rule::horizontal(1),
                self.editor_pane()
            ]
        } else {
            column![self.editor_pane()]
        };
        let page = container(content)
            .style(writing_surface)
            .clip(true)
            .width(Fill)
            .height(Fill);

        let mut main = column![page].spacing(8).width(Fill).height(Fill);
        if self.current.is_some() {
            main = main.push(self.backlinks_strip());
        }
        let main = main.push(self.footer());

        let body = row![
            container(self.sidebar())
                .style(panel)
                .clip(true)
                .width(SIDEBAR_WIDTH)
                .height(Fill),
            main
        ]
        .spacing(12)
        .height(Fill);

        self.layer_popup(
            container(body)
                .padding(12)
                .style(chassis)
                .width(Fill)
                .height(Fill),
        )
    }

    fn editor_header(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        let title = self
            .current
            .as_ref()
            .and_then(|path| path.file_stem())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let header = row![
            crate::icons::file(true),
            line(title, 13.0, MEDIUM, look.ink),
            self.mode_switch(),
        ]
        .spacing(10)
        .align_y(iced::Center);
        container(header).padding([14, 20]).width(Fill).into()
    }

    fn editor_pane(&self) -> Element<'_, Message> {
        match &self.doc {
            Some(Doc::Live(live)) => {
                let note_dir = self.note_dir();
                live_editor(live, self.appearance, note_dir.as_deref())
            }
            Some(Doc::Source(content)) => source_editor(content),
            None => self.empty_editor(),
        }
    }

    /// Save state, word count, cursor, and appearance. Editing mode lives in the header.
    fn footer(&self) -> Element<'_, Message> {
        row![
            container(self.footer_status()).width(Fill).clip(true),
            row(self.footer_stats()).spacing(16)
        ]
        .spacing(16)
        .padding([3, 8])
        .align_y(iced::Center)
        .into()
    }

    fn footer_status(&self) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        if let Some(err) = &self.notice {
            return line(err.clone(), 12.0, crate::UI_FONT, look.danger);
        }
        if let Some(rel) = &self.current {
            return row![
                self.save_state(),
                line(rel.display().to_string(), 12.0, crate::UI_FONT, look.ink),
            ]
            .spacing(8)
            .align_y(iced::Center)
            .into();
        }
        space().into()
    }

    /// Notes that link here. The header stays, so the list can be folded away
    /// without losing the count. Hidden until a note is open.
    fn backlinks_strip(&self) -> Element<'_, Message> {
        let Some(current) = &self.current else {
            return column![].into();
        };
        let links = self.links.backlinks(current);
        if links.is_empty() {
            return column![].into();
        }
        let header = self.backlinks_header(links.len());
        if links.is_empty() || !self.backlinks_open {
            return header;
        }
        let rows = links.into_iter().map(|link| backlink_row(link));
        // A popular note must not push the editor off the window.
        column![
            header,
            container(scrollable(column(rows).spacing(2)).style(theme::scrollbar))
                .max_height(112)
                .width(Fill),
        ]
        .spacing(4)
        .width(Fill)
        .into()
    }

    fn backlinks_header(&self, count: usize) -> Element<'_, Message> {
        let look = theme::tokens(self.appearance);
        let open = self.backlinks_open && count > 0;
        let color = if count == 0 { look.muted } else { look.ink };
        button(
            row![
                crate::icons::chevron(open),
                text(backlinks_label(count))
                    .size(12)
                    .font(super::LABEL)
                    .color(color),
            ]
            .spacing(6)
            .align_y(iced::Center),
        )
        .width(Fill)
        .padding([2, 4])
        .style(quiet_button)
        .on_press(Message::ToggleBacklinks)
        .into()
    }

    fn save_state(&self) -> Element<'_, Message> {
        let muted = theme::tokens(self.appearance).muted;
        if !self.dirty {
            return text("Saved").size(12).color(muted).into();
        }
        with_tip(
            button(text("Unsaved").size(12))
                .padding([2, 8])
                .style(unsaved_style)
                .on_press(Message::Save),
            "Save (Ctrl+S)",
            tooltip::Position::Top,
            350,
        )
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
        }
        stats.push(self.theme_switch());
        stats
    }

    fn theme_switch(&self) -> Element<'_, Message> {
        switch([
            segment(
                "Light",
                self.appearance == Appearance::Light,
                "Paper — warm ivory and deep teal",
                Message::SetAppearance(Appearance::Light),
            ),
            segment(
                "Dark",
                self.appearance == Appearance::Dark,
                "Ink — charcoal and soft jade",
                Message::SetAppearance(Appearance::Dark),
            ),
        ])
    }

    fn empty_editor(&self) -> Element<'_, Message> {
        let action: Element<'_, Message> = if self.workspace.is_some() {
            button(text("New note").size(13).font(super::LABEL))
                .padding([11, 20])
                .style(super::style::rounded_primary)
                .on_press(Message::NewNote)
                .into()
        } else {
            space().into()
        };
        container(action).center(Fill).into()
    }

    fn mode_switch(&self) -> Element<'_, Message> {
        switch([
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
        ])
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
        .padding(32)
        .height(Fill)
        .style(editor_style)
        .into()
}

fn backlinks_label(n: usize) -> String {
    match n {
        0 => "No notes link here".into(),
        1 => "1 backlink".into(),
        n => format!("{n} backlinks"),
    }
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

/// The same shortcuts as live preview, see [`live::command`].
pub(super) fn shortcut(key: keyboard::Key<&str>, modifiers: Modifiers) -> Option<Message> {
    match live::command(key, modifiers)? {
        live::Msg::Save => Some(Message::Save),
        live::Msg::ToggleMode => Some(Message::ToggleMode),
        live::Msg::Undo => Some(Message::Undo),
        live::Msg::Redo => Some(Message::Redo),
        live::Msg::Paste => Some(Message::Paste),
        _ => None,
    }
}
