//! Appearance and font controls. Editing modes remain in the note header.

use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, space, stack, text,
    text_input,
};
use iced::{Border, Color, Element, Fill, Padding, Task};

use super::style::{
    field_style, icon_button, menu_card, picker_menu, picker_style, rounded_subtle, segment, switch,
};
use super::{App, LABEL, Message};
use crate::config::Fonts;
use crate::system_font;
use crate::theme::{self, Appearance, ColorSlot, Colors};

const SYSTEM_DEFAULT: &str = "System default";

#[derive(Debug, Clone)]
pub(super) enum Msg {
    Open,
    Close,
    KeepOpen,
    ColorInput(ColorSlot, String),
    PickColor(ColorSlot, Color),
    TogglePicker(ColorSlot),
    Reset,
    PickFont(FontTarget, String),
}

#[derive(Debug, Clone, Copy)]
pub(super) enum FontTarget {
    Interface,
    Note,
}

impl FontTarget {
    fn label(self) -> &'static str {
        match self {
            FontTarget::Interface => "Interface font",
            FontTarget::Note => "Note font",
        }
    }

    fn alias(self) -> &'static str {
        match self {
            FontTarget::Interface => system_font::UI_FAMILY,
            FontTarget::Note => system_font::NOTE_FAMILY,
        }
    }

    fn choice(self, fonts: &Fonts) -> Option<&String> {
        match self {
            FontTarget::Interface => fonts.interface.as_ref(),
            FontTarget::Note => fonts.note.as_ref(),
        }
    }

    fn set(self, fonts: &mut Fonts, family: Option<String>) {
        match self {
            FontTarget::Interface => fonts.interface = family,
            FontTarget::Note => fonts.note = family,
        }
    }
}

pub(super) struct State {
    pub open: bool,
    /// The slot whose preset swatches are showing.
    open_picker: Option<ColorSlot>,
    /// What each hex field shows, in [`ColorSlot::ALL`] order. May be invalid
    /// while the user is typing.
    hex_drafts: [String; 4],
    save_error: Option<String>,
    fonts: Fonts,
    /// [`SYSTEM_DEFAULT`], then the installed families. Filled on open.
    font_options: Vec<String>,
}

impl State {
    pub fn new(colors: Colors, fonts: Fonts) -> Self {
        Self {
            open: false,
            open_picker: None,
            hex_drafts: hex_drafts(colors),
            save_error: None,
            fonts,
            font_options: Vec::new(),
        }
    }

    /// Show `colors` in the hex fields and close any picker.
    pub fn sync(&mut self, colors: Colors) {
        self.hex_drafts = hex_drafts(colors);
        self.open_picker = None;
    }

    fn has_invalid_draft(&self) -> bool {
        self.hex_drafts
            .iter()
            .any(|value| theme::parse_hex(value).is_none())
    }
}

fn hex_drafts(colors: Colors) -> [String; 4] {
    std::array::from_fn(|i| theme::hex(ColorSlot::ALL[i].get(colors)))
}

impl App {
    pub(super) fn update_settings(&mut self, message: Msg) -> Task<Message> {
        match message {
            Msg::KeepOpen => {}
            Msg::Open => {
                self.create = None;
                self.settings
                    .sync(self.customization.colors(self.appearance));
                self.settings.font_options = std::iter::once(SYSTEM_DEFAULT.to_owned())
                    .chain(system_font::families())
                    .collect();
                self.settings.open = true;
            }
            Msg::Close => {
                self.settings.open = false;
            }
            Msg::TogglePicker(slot) => {
                self.settings.open_picker =
                    (self.settings.open_picker != Some(slot)).then_some(slot);
            }
            Msg::ColorInput(slot, value) => {
                let parsed = theme::parse_hex(&value);
                self.settings.hex_drafts[slot.index()] = value;
                if let Some(color) = parsed {
                    self.apply_color(slot, color);
                }
            }
            Msg::PickColor(slot, color) => {
                self.settings.hex_drafts[slot.index()] = theme::hex(color);
                self.apply_color(slot, color);
            }
            Msg::Reset => {
                self.customization.reset(self.appearance);
                self.settings
                    .sync(self.customization.colors(self.appearance));
                self.persist_colors();
            }
            Msg::PickFont(target, choice) => {
                let family = (choice != SYSTEM_DEFAULT).then_some(choice);
                system_font::apply(target.alias(), family.as_deref());
                target.set(&mut self.settings.fonts, family);
                let saved = crate::config::save_fonts(&self.settings.fonts);
                self.record_save(saved);
            }
        }
        Task::none()
    }

    fn apply_color(&mut self, slot: ColorSlot, color: Color) {
        let mut colors = self.customization.colors(self.appearance);
        *slot.get_mut(&mut colors) = color;
        self.customization.set(self.appearance, colors);
        self.persist_colors();
    }

    fn persist_colors(&mut self) {
        let saved = crate::config::save_customization(&self.customization);
        self.record_save(saved);
    }

    fn record_save(&mut self, saved: std::io::Result<()>) {
        self.settings.save_error = saved
            .err()
            .map(|error| format!("Could not save settings: {error}"));
    }

    pub(super) fn layer_settings<'a>(
        &'a self,
        window: impl Into<Element<'a, Message>>,
    ) -> Element<'a, Message> {
        let window = window.into();
        if !self.settings.open {
            return window;
        }
        // A compact overlay leaves the note's mode controls in their usual place.
        let card = container(scrollable(self.settings_panel()).style(theme::scrollbar))
            .padding(16)
            .width(360)
            .max_height(430)
            .style(|theme| container::Style {
                shadow: iced::Shadow::default(),
                ..menu_card(theme)
            });
        stack![
            window,
            mouse_area(space().width(Fill).height(Fill)).on_press(Message::Settings(Msg::Close)),
            container(mouse_area(card).on_press(Message::Settings(Msg::KeepOpen)))
                .align_right(Fill)
                .padding(Padding::ZERO.top(76).right(20)),
        ]
        .width(Fill)
        .height(Fill)
        .into()
    }

    fn settings_panel(&self) -> Element<'_, Message> {
        let look = self.look();
        let mut body = column![
            row![
                text("Settings").size(16).font(LABEL).width(Fill),
                icon_button(
                    crate::icons::close(),
                    "Close",
                    Some(Message::Settings(Msg::Close))
                ),
            ]
            .align_y(iced::Center),
            row![
                text("Appearance").size(13).width(Fill),
                switch([
                    segment(
                        "Light",
                        self.appearance == Appearance::Light,
                        "Light theme",
                        Message::SetAppearance(Appearance::Light)
                    ),
                    segment(
                        "Dark",
                        self.appearance == Appearance::Dark,
                        "Dark theme",
                        Message::SetAppearance(Appearance::Dark)
                    ),
                ]),
            ]
            .align_y(iced::Center),
            self.font_control(FontTarget::Interface),
            self.font_control(FontTarget::Note),
        ]
        .spacing(10)
        .width(Fill);
        for slot in ColorSlot::ALL {
            body = body.push(self.color_control(slot));
        }
        if self.settings.has_invalid_draft() {
            body = body.push(text("Use #RRGGBB.").size(12).color(look.danger));
        }
        // The accent is always used as chosen; the rest may be corrected.
        let chosen = self.customization.colors(self.appearance);
        if chosen.text != look.ink || chosen.page != look.surface || chosen.sidebar != look.panel {
            body = body.push(
                text("Colors adjusted for contrast.")
                    .size(11)
                    .color(look.muted),
            );
        }
        if let Some(error) = &self.settings.save_error {
            body = body.push(text(error).size(12).color(look.danger));
        }
        body.push(
            button(text("Reset colors").size(12))
                .padding([8, 12])
                .style(rounded_subtle)
                .on_press(Message::Settings(Msg::Reset)),
        )
        .into()
    }

    fn font_control(&self, target: FontTarget) -> Element<'_, Message> {
        let options = &self.settings.font_options;
        // A saved family that is no longer installed shows as the default it falls back to.
        let chosen = target
            .choice(&self.settings.fonts)
            .filter(|family| options.contains(family))
            .map_or(SYSTEM_DEFAULT, String::as_str);
        row![
            text(target.label()).size(13).width(Fill),
            pick_list(options.as_slice(), Some(chosen.to_owned()), move |choice| {
                Message::Settings(Msg::PickFont(target, choice))
            })
            // The list stays readable even when the chosen font is not.
            .font(crate::DEFAULT_FONT)
            .text_size(13)
            .padding([8, 10])
            .width(180)
            .menu_height(240)
            .style(picker_style)
            .menu_style(picker_menu),
        ]
        .spacing(10)
        .align_y(iced::Center)
        .into()
    }

    fn color_control(&self, slot: ColorSlot) -> Element<'_, Message> {
        let colors = self.customization.colors(self.appearance);
        let picker_open = self.settings.open_picker == Some(slot);
        let current = slot.get(colors);
        let draft = &self.settings.hex_drafts[slot.index()];
        let invalid = theme::parse_hex(draft).is_none();
        let field = text_input("#RRGGBB", draft)
            .on_input(move |value| Message::Settings(Msg::ColorInput(slot, value)))
            .padding([8, 10])
            .size(13)
            .width(110)
            .style(move |theme, status| {
                let mut style = field_style(theme, status);
                if invalid {
                    style.border.color = theme::tokens_of(theme).danger;
                }
                style
            });
        let mut control = column![
            row![
                swatch(
                    current,
                    picker_open,
                    Message::Settings(Msg::TogglePicker(slot))
                ),
                text(slot.label()).size(13).width(Fill),
                field,
            ]
            .spacing(10)
            .align_y(iced::Center),
        ]
        .spacing(8);
        if picker_open {
            let presets = preset_colors(slot, self.appearance).map(|color| {
                swatch(
                    color,
                    color == current,
                    Message::Settings(Msg::PickColor(slot, color)),
                )
            });
            control = control.push(row(presets).spacing(8));
        }
        control.into()
    }
}

fn swatch(color: Color, selected: bool, message: Message) -> Element<'static, Message> {
    button(space().width(24).height(24))
        .padding(3)
        .style(move |theme, status| {
            let look = theme::tokens_of(theme);
            let active =
                selected || matches!(status, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(color.into()),
                border: Border {
                    color: if active { look.ink } else { look.border_strong },
                    width: if active { 2.0 } else { 1.0 },
                    radius: 7.0.into(),
                },
                ..button::Style::default()
            }
        })
        .on_press(message)
        .into()
}

/// The swatches offered under a slot's picker.
fn preset_colors(slot: ColorSlot, appearance: Appearance) -> [Color; 8] {
    let presets = match slot {
        ColorSlot::Accent => [
            0x245B53, 0x93B9A6, 0x5686C4, 0x9C7CC2, 0xC47587, 0xC38C50, 0x667786, 0x343A38,
        ],
        ColorSlot::Page | ColorSlot::Sidebar if appearance == Appearance::Light => [
            0xFFFFFF, 0xFAF8F2, 0xF2EBDE, 0xE7EBE5, 0xE5EDF3, 0xEEE8F0, 0xF1E5E4, 0xE7E9E8,
        ],
        ColorSlot::Page | ColorSlot::Sidebar => [
            0x151515, 0x202624, 0x1C2220, 0x242832, 0x29252E, 0x30272A, 0x302C25, 0x303534,
        ],
        ColorSlot::Text => [
            0x151515, 0x242D2A, 0x3B414B, 0x564A47, 0xE9EDE5, 0xFAF8F2, 0xD7DFED, 0xFFFFFF,
        ],
    };
    presets.map(theme::rgb)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_settings_preserves_the_note_mode_and_unsaved_text() {
        let (mut app, _) = App::new();
        app.doc = Some(super::super::Doc::Source(
            iced::widget::text_editor::Content::with_text("Unsaved note"),
        ));
        app.mode = super::super::Mode::Source;
        app.dirty = true;
        let _ = app.update_settings(Msg::Open);
        assert!(app.settings.open);
        assert!(app.create.is_none());
        assert_eq!(app.mode, super::super::Mode::Source);
        assert_eq!(app.text().as_deref(), Some("Unsaved note"));
        assert!(app.dirty);
        let _ = app.update_settings(Msg::Close);
        assert!(!app.settings.open);
        assert!(app.doc.is_some());
    }

    #[test]
    fn invalid_input_does_not_change_the_active_palette() {
        let (mut app, _) = App::new();
        let before = app.customization;
        for value in ["#12", "#xx"] {
            let _ = app.update_settings(Msg::ColorInput(ColorSlot::Accent, value.into()));
            assert_eq!(app.customization, before);
            assert!(app.settings.has_invalid_draft());
        }
    }
}
