//! Sidebar and editor chrome: buttons, fields, and surfaces.

use iced::widget::{button, container, text, text_editor, text_input, tooltip};
use iced::{Border, Color, Element, Fill, Font, Padding, Shadow, Theme, Vector, border};

use super::{LABEL, Message};
use crate::theme;

pub(super) fn line<'a>(
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
    tooltip(
        button(icon)
            .padding(7)
            .width(30)
            .height(30)
            .style(icon_button_style)
            .on_press_maybe(on_press),
        container(text(tip).size(12))
            .padding([4, 8])
            .style(hint_box),
        tooltip::Position::Bottom,
    )
    .gap(6)
    .delay(iced::time::Duration::from_millis(350))
    .into()
}

pub(super) fn hint<'a>(tip: &'a str) -> Element<'a, Message> {
    container(text(tip).size(12))
        .padding([4, 8])
        .style(hint_box)
        .into()
}

pub(super) fn segment<'a>(
    label: &'a str,
    active: bool,
    tip: &'a str,
    message: Message,
) -> Element<'a, Message> {
    tooltip(
        button(
            text(label)
                .size(12)
                .font(if active { LABEL } else { Font::DEFAULT }),
        )
        .padding([4, 10])
        .style(segment_style(active))
        .on_press(message),
        hint(tip),
        tooltip::Position::Top,
    )
    .gap(6)
    .delay(iced::time::Duration::from_millis(400))
    .into()
}

pub(super) fn segment_track(theme: &Theme) -> container::Style {
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
        let background = if active {
            Some(look.raised.into())
        } else {
            match status {
                button::Status::Hovered | button::Status::Pressed => Some(look.surface.into()),
                _ => None,
            }
        };
        button::Style {
            background,
            text_color: if active { look.ink } else { look.muted },
            border: border::rounded(6),
            ..button::Style::default()
        }
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

pub(super) fn tree_button<'a>(
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

pub(super) fn chassis(theme: &Theme) -> container::Style {
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
        border: Border {
            color: look.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: card_shadow(look.dark),
        ..container::Style::default()
    }
}

pub(super) fn writing_surface(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.surface.into()),
        text_color: Some(look.ink),
        border: Border {
            color: look.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: card_shadow(look.dark),
        ..container::Style::default()
    }
}

fn card_shadow(dark: bool) -> Shadow {
    if dark {
        Shadow {
            color: Color::BLACK.scale_alpha(0.5),
            offset: Vector::new(0.0, 4.0),
            blur_radius: 12.0,
        }
    } else {
        Shadow {
            color: Color::BLACK.scale_alpha(0.04),
            offset: Vector::new(0.0, 2.0),
            blur_radius: 4.0,
        }
    }
}

fn hint_box(theme: &Theme) -> container::Style {
    let look = theme::tokens_of(theme);
    container::Style {
        background: Some(look.raised.into()),
        text_color: Some(look.ink),
        border: Border {
            color: look.border,
            width: 1.0,
            radius: 6.0.into(),
        },
        ..container::Style::default()
    }
}

pub(super) fn editor_style(theme: &Theme, _status: text_editor::Status) -> text_editor::Style {
    let look = theme::tokens_of(theme);
    text_editor::Style {
        background: look.surface.into(),
        border: Border::default(),
        placeholder: look.muted,
        value: look.ink,
        selection: look.selection,
    }
}

pub(super) fn field_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let look = theme::tokens_of(theme);
    let border_color = match status {
        text_input::Status::Focused { .. } => look.focus,
        text_input::Status::Hovered => look.border_strong,
        _ => look.border,
    };
    text_input::Style {
        background: look.surface.into(),
        border: Border {
            color: border_color,
            width: 1.0,
            radius: 6.0.into(),
        },
        icon: look.muted,
        placeholder: look.muted,
        value: look.ink,
        selection: look.selection,
    }
}

pub(super) fn rounded_primary(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    keycap(
        look.accent,
        look.accent_hover,
        look.accent_text,
        look.accent_edge,
        look.accent,
        status,
    )
}

pub(super) fn rounded_subtle(theme: &Theme, status: button::Status) -> button::Style {
    let look = theme::tokens_of(theme);
    keycap(
        look.surface,
        look.raised,
        look.ink,
        look.lip,
        look.border,
        status,
    )
}

/// Alpha or accent key: fill, a 2px lip, and no lip once the key is down.
fn keycap(
    rest: Color,
    hover: Color,
    text: Color,
    lip: Color,
    edge: Color,
    status: button::Status,
) -> button::Style {
    let pressed = status == button::Status::Pressed;
    let disabled = status == button::Status::Disabled;
    let fill = if status == button::Status::Hovered {
        hover
    } else {
        rest
    };
    let mut style = button::Style {
        background: Some(fill.into()),
        text_color: text,
        border: Border {
            color: edge,
            width: 1.0,
            radius: 6.0.into(),
        },
        shadow: if pressed || disabled {
            Shadow::default()
        } else {
            Shadow {
                color: lip,
                offset: Vector::new(0.0, 2.0),
                blur_radius: 0.0,
            }
        },
        ..button::Style::default()
    };
    if disabled {
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

fn tree_row_style(selected: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |theme, status| {
        let look = theme::tokens_of(theme);
        let background = if selected {
            let alpha = match status {
                button::Status::Hovered | button::Status::Pressed => 0.28,
                _ => 0.16,
            };
            Some(look.accent.scale_alpha(alpha).into())
        } else {
            match status {
                button::Status::Hovered => Some(look.raised.into()),
                button::Status::Pressed => Some(look.surface.into()),
                _ => None,
            }
        };
        button::Style {
            background,
            text_color: look.ink,
            border: border::rounded(6),
            ..button::Style::default()
        }
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
