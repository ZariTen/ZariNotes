//! Sidebar and editor chrome: buttons, fields, and surfaces.

use iced::overlay::menu;
use iced::widget::{
    button, column, container, pick_list, row, text, text_editor, text_input, tooltip,
};
use iced::{Border, Color, Element, Fill, Font, Padding, Shadow, Theme, Vector, border};

use super::{LABEL, Message};
use crate::theme;

pub(super) fn line<'a>(
    content: impl Into<String>,
    size: f32,
    font: Font,
    color: impl Into<Color>,
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

pub(super) fn row_label<'a>(label: &'a str, font: Font) -> Element<'a, Message> {
    text(label)
        .size(14)
        .font(font)
        .width(Fill)
        .wrapping(text::Wrapping::WordOrGlyph)
        .into()
}

pub(super) fn icon_button<'a>(
    icon: impl Into<Element<'a, Message>>,
    tip: &'a str,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    with_tip(
        button(icon)
            .padding(7)
            .width(30)
            .height(30)
            .style(icon_button_style)
            .on_press_maybe(on_press),
        tip,
        tooltip::Position::Bottom,
        350,
    )
}

/// `content` with a hint box that shows after `delay_ms` of hovering.
pub(super) fn with_tip<'a>(
    content: impl Into<Element<'a, Message>>,
    tip: &'a str,
    position: tooltip::Position,
    delay_ms: u64,
) -> Element<'a, Message> {
    let hint = container(text(tip).size(12))
        .padding([4, 8])
        .style(hint_box);
    tooltip(content, hint, position)
        .gap(6)
        .delay(iced::time::Duration::from_millis(delay_ms))
        .into()
}

pub(super) fn segment<'a>(
    label: &'a str,
    active: bool,
    tip: &'a str,
    message: Message,
) -> Element<'a, Message> {
    with_tip(
        button(
            text(label)
                .size(12)
                .font(if active { LABEL } else { crate::UI_FONT }),
        )
        .padding([4, 10])
        .style(segment_style(active))
        .on_press(message),
        tip,
        tooltip::Position::Top,
        400,
    )
}

/// Two [`segment`]s on one track.
pub(super) fn switch<'a>(segments: [Element<'a, Message>; 2]) -> Element<'a, Message> {
    container(row(segments).spacing(2))
        .padding(2)
        .style(segment_track)
        .into()
}

fn segment_track(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.track.into()),
        border: border::rounded(6),
        ..container::Style::default()
    }
}

fn segment_style(active: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let look = theme::tokens_of(theme);
        button::Style {
            background: segment_fill(look, active, status),
            text_color: if active { look.ink } else { look.muted },
            border: border::rounded(6),
            ..button::Style::default()
        }
    }
}

fn segment_fill(
    look: theme::Tokens,
    active: bool,
    status: button::Status,
) -> Option<iced::Background> {
    if active {
        return Some(look.raised.into());
    }
    match status {
        button::Status::Hovered | button::Status::Pressed => Some(look.surface.into()),
        _ => None,
    }
}

pub(super) fn unsaved_style(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    let mut style = button::Style {
        text_color: look.warning,
        border: border::rounded(6),
        ..button::Style::default()
    };
    if matches!(status, button::Status::Hovered | button::Status::Pressed) {
        style.background = Some(look.warning.scale_alpha(0.16).into());
    }
    style
}

/// Whitespace-separated words. Markdown marks count as words; this is a writing
/// glance, not a rendered-text count.
pub(super) fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

pub(super) fn words_label(n: usize) -> String {
    match n {
        0 => "Empty".into(),
        1 => "1 word".into(),
        n => format!("{n} words"),
    }
}

pub(super) fn menu_card(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.raised.into()),
        text_color: Some(look.ink),
        border: hairline(look.border_strong),
        shadow: card_shadow(look.dark),
        ..container::Style::default()
    }
}

pub(super) fn menu_danger_style(theme: &Theme, status: button::Status) -> button::Style {
    button::Style {
        text_color: theme::tokens_of(theme).danger,
        ..quiet_button(theme, status)
    }
}

pub(super) fn backlink_row(link: crate::links::Backlink) -> Element<'static, Message> {
    let name = link
        .from
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_else(|| link.from.display().to_string());
    let title = match &link.alias {
        Some(alias) => format!("{name} · {alias}"),
        None => name,
    };
    let title = if link.embed {
        format!("{title} · embed")
    } else {
        title
    };
    let body = if link.excerpt.is_empty() {
        column![text(title).size(13)]
    } else {
        column![
            text(title).size(13),
            text(link.excerpt).size(12).style(muted_ink),
        ]
        .spacing(2)
    };
    button(body)
        .width(Fill)
        .padding([4, 8])
        .style(quiet_button)
        .on_press(Message::Open(link.from))
        .into()
}

fn muted_ink(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(theme::tokens_of(theme).muted),
    }
}

/// A full-width row that only shows a fill when hovered: menu items,
/// backlinks, and the backlink header.
pub(super) fn quiet_button(theme: &Theme, status: button::Status) -> button::Style {
    flat_row(theme, status, false)
}

pub(super) fn tree_button<'a>(
    body: impl Into<Element<'a, Message>>,
    pad: Padding,
    selected: bool,
    message: Message,
) -> Element<'a, Message> {
    button(body)
        .width(Fill)
        .padding(pad)
        .style(move |theme, status| flat_row(theme, status, selected))
        .on_press(message)
        .into()
}

/// The window background behind the sidebar and page.
pub(super) fn canvas(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.canvas.into()),
        text_color: Some(look.ink),
        ..container::Style::default()
    }
}

pub(super) fn panel(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.panel.into()),
        text_color: Some(look.ink),
        border: border::rounded(12),
        ..container::Style::default()
    }
}

pub(super) fn brand_mark(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.accent.into()),
        border: border::rounded(10),
        ..container::Style::default()
    }
}

pub(super) fn writing_surface(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.surface.into()),
        text_color: Some(look.ink),
        border: Border {
            radius: 12.0.into(),
            ..hairline(look.border)
        },
        ..container::Style::default()
    }
}

fn hairline(color: Color) -> Border {
    Border {
        color,
        width: 1.0,
        radius: 8.0.into(),
    }
}

fn card_shadow(dark: bool) -> Shadow {
    if dark {
        return Shadow {
            color: Color::BLACK.scale_alpha(0.5),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        };
    }
    Shadow {
        color: Color::BLACK.scale_alpha(0.04),
        offset: Vector::new(0.0, 2.0),
        blur_radius: 4.0,
    }
}

fn hint_box(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.raised.into()),
        text_color: Some(look.ink),
        border: hairline(look.border),
        ..container::Style::default()
    }
}

pub(super) fn editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let look = theme::tokens_of(theme);
    text_editor::Style {
        // Let the rounded writing surface paint the page, including its corners.
        background: Color::TRANSPARENT.into(),
        border: Border::default(),
        placeholder: look.muted,
        value: look.ink,
        selection: look.selection,
    }
}

/// A compact dropdown that matches [`field_style`].
pub(super) fn picker_style(theme: &Theme, status: pick_list::Status) -> pick_list::Style {
    let look = theme::tokens_of(theme);
    let border = match status {
        pick_list::Status::Opened { .. } => look.focus,
        pick_list::Status::Hovered => look.border_strong,
        pick_list::Status::Active => look.border,
    };
    pick_list::Style {
        text_color: look.ink,
        placeholder_color: look.muted,
        handle_color: look.muted,
        background: look.surface.into(),
        border: hairline(border),
    }
}

pub(super) fn picker_menu(theme: &Theme) -> menu::Style {
    let look = theme::tokens_of(theme);
    menu::Style {
        background: look.raised.into(),
        border: hairline(look.border_strong),
        text_color: look.ink,
        selected_text_color: look.ink,
        selected_background: look.selection.into(),
        shadow: card_shadow(look.dark),
    }
}

pub(super) fn field_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let look = theme::tokens_of(theme);
    text_input::Style {
        background: look.surface.into(),
        border: hairline(field_border(look, status)),
        icon: look.muted,
        placeholder: look.muted,
        value: look.ink,
        selection: look.selection,
    }
}

fn field_border(look: theme::Tokens, status: text_input::Status) -> Color {
    match status {
        text_input::Status::Focused { .. } => look.focus,
        text_input::Status::Hovered => look.border_strong,
        _ => look.border,
    }
}

pub(super) fn rounded_primary(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    solid_button(
        look.accent,
        look.accent_hover,
        look.accent_text,
        look.accent,
        status,
    )
}

pub(super) fn rounded_danger(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    solid_button(
        look.danger,
        look.danger,
        theme::black_or_white(look.danger),
        look.danger,
        status,
    )
}

pub(super) fn rounded_subtle(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    solid_button(look.surface, look.raised, look.ink, look.border, status)
}

/// Flat, softly rounded actions; interaction is expressed through tone.
fn solid_button(
    fill: Color,
    hover_fill: Color,
    text: Color,
    border: Color,
    status: button::Status,
) -> button::Style {
    let fill = if status == button::Status::Hovered {
        hover_fill
    } else {
        fill
    };
    let mut style = button::Style {
        background: Some(fill.into()),
        text_color: text,
        border: hairline(border),
        ..button::Style::default()
    };
    if status == button::Status::Disabled {
        style.text_color = style.text_color.scale_alpha(0.45);
        style.background = style.background.map(|bg| bg.scale_alpha(0.5));
    }
    style
}

fn icon_button_style(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    let mut style = button::Style {
        text_color: look.ink,
        border: border::rounded(6),
        ..button::Style::default()
    };
    match status {
        button::Status::Hovered => {
            style.background = Some(look.raised.into());
        }
        button::Status::Pressed => {
            style.background = Some(look.surface.into());
        }
        button::Status::Disabled => {
            style.text_color = style.text_color.scale_alpha(0.35);
        }
        button::Status::Active => {}
    }
    style
}

fn flat_row(theme: &Theme, status: button::Status, selected: bool) -> button::Style {
    let look = theme::tokens_of(theme);
    button::Style {
        background: row_fill(look, selected, status),
        text_color: look.ink,
        border: Border {
            color: if selected {
                look.accent.scale_alpha(0.18)
            } else {
                Color::TRANSPARENT
            },
            width: 1.0,
            radius: 8.0.into(),
        },
        ..button::Style::default()
    }
}

fn row_fill(
    look: theme::Tokens,
    selected: bool,
    status: button::Status,
) -> Option<iced::Background> {
    if selected {
        return Some(look.selection.into());
    }
    match status {
        button::Status::Hovered => Some(look.raised.into()),
        button::Status::Pressed => Some(look.surface.into()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{word_count, words_label};

    #[test]
    fn chrome_stays_flat_and_selected_notes_stay_distinct() {
        for appearance in [
            crate::theme::Appearance::Light,
            crate::theme::Appearance::Dark,
        ] {
            let theme = crate::theme::custom(appearance, None);
            assert_eq!(super::panel(&theme).shadow, iced::Shadow::default());
            assert_eq!(
                super::writing_surface(&theme).shadow,
                iced::Shadow::default()
            );
            for status in [
                iced::widget::button::Status::Active,
                iced::widget::button::Status::Hovered,
                iced::widget::button::Status::Pressed,
                iced::widget::button::Status::Disabled,
            ] {
                let action = super::rounded_primary(&theme, status);
                assert_eq!(action.shadow, iced::Shadow::default());
                assert_ne!(
                    super::flat_row(&theme, status, true).background,
                    super::flat_row(&theme, status, false).background
                );
            }
        }
    }

    #[test]
    fn custom_selection_and_destructive_actions_keep_readable_legends() {
        for appearance in [
            crate::theme::Appearance::Light,
            crate::theme::Appearance::Dark,
        ] {
            let theme = crate::theme::custom(
                appearance,
                Some(crate::theme::Colors {
                    accent: iced::Color::WHITE,
                    page: iced::Color::WHITE,
                    sidebar: iced::Color::BLACK,
                    text: iced::Color::WHITE,
                }),
            );
            for style in [
                super::flat_row(&theme, iced::widget::button::Status::Active, true),
                super::rounded_danger(&theme, iced::widget::button::Status::Active),
            ] {
                let Some(iced::Background::Color(fill)) = style.background else {
                    panic!("solid fill expected")
                };
                assert!(style.text_color.relative_contrast(fill) >= 4.5);
            }
        }
    }

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
