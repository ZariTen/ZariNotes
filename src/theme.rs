//! Paper (warm ivory and sage) and Ink (charcoal and muted jade).
//!
//! Quiet, layered surfaces keep the writing in focus. Teal and jade provide
//! restrained emphasis, while text and semantic tones retain WCAG AA contrast
//! across the writing surface, sidebar, canvas, and raised controls.

use iced::theme::Base;
use iced::theme::palette::{self, Extended, Pair};
use iced::{Color, Theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "light" => Some(Self::Light),
            "dark" => Some(Self::Dark),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            Self::Light => "Paper",
            Self::Dark => "Ink",
        }
    }
}

/// Shared surface, typography, interaction, and semantic color tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    pub canvas: Color,
    pub surface: Color,
    pub panel: Color,
    pub raised: Color,
    pub track: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_text: Color,
    pub accent_edge: Color,
    pub ink: Color,
    pub muted: Color,
    pub border: Color,
    pub border_strong: Color,
    pub lip: Color,
    pub link: Color,
    pub selection: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub focus: Color,
    pub dark: bool,
}

const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn tokens(appearance: Appearance) -> Tokens {
    match appearance {
        Appearance::Light => PAPER,
        Appearance::Dark => INK,
    }
}

/// Resolve tokens from the theme iced is currently painting with.
///
/// iced only gives us the name we registered in [`iced`].
pub fn tokens_of(theme: &Theme) -> Tokens {
    if theme.name() == Appearance::Dark.title() {
        return tokens(Appearance::Dark);
    }
    tokens(Appearance::Light)
}

pub fn iced(appearance: Appearance) -> Theme {
    let look = tokens(appearance);
    Theme::custom_with_fn(appearance.title(), base_palette(look), move |_| {
        extended(look)
    })
}

const PAPER: Tokens = Tokens {
    canvas: rgb(0xF3F1EA),
    surface: rgb(0xFAF8F2),
    panel: rgb(0xE7EBE5),
    raised: rgb(0xFFFFFB),
    track: rgb(0xE1E5DE),
    accent: rgb(0x245B53),
    accent_hover: rgb(0x1B4943),
    accent_text: rgb(0xFCFBF6),
    accent_edge: rgb(0x163F39),
    ink: rgb(0x242D2A),
    muted: rgb(0x59645E),
    border: rgb(0xDCE1D8),
    border_strong: rgb(0xB7C3B8),
    lip: rgb(0xD1D8CE),
    link: rgb(0x285F56),
    selection: rgb(0xC9DCCF),
    success: rgb(0x356342),
    warning: rgb(0x805B24),
    danger: rgb(0x9E4640),
    focus: rgb(0x245B53),
    dark: false,
};

const INK: Tokens = Tokens {
    canvas: rgb(0x191E1C),
    surface: rgb(0x202624),
    panel: rgb(0x1C2220),
    raised: rgb(0x29312D),
    track: rgb(0x303B35),
    accent: rgb(0x93B9A6),
    accent_hover: rgb(0xADCBBB),
    accent_text: rgb(0x18241E),
    accent_edge: rgb(0x618D78),
    ink: rgb(0xE9EDE5),
    muted: rgb(0xADB8AE),
    border: rgb(0x303B35),
    border_strong: rgb(0x4F6156),
    lip: rgb(0x101613),
    link: rgb(0xA0C9B4),
    selection: rgb(0x384F43),
    success: rgb(0xAAC696),
    warning: rgb(0xD9BB80),
    danger: rgb(0xE5A39A),
    focus: rgb(0x93B9A6),
    dark: true,
};

fn base_palette(look: Tokens) -> palette::Palette {
    palette::Palette {
        background: look.canvas,
        text: look.ink,
        primary: look.link,
        success: look.success,
        warning: look.warning,
        danger: look.danger,
    }
}

fn text_on(color: Color, text: Color) -> Pair {
    Pair { color, text }
}

/// Map editorial surfaces onto iced's slots so stock widgets (rules,
/// scrollbars, inputs, primary buttons) share the same visual hierarchy.
fn extended(look: Tokens) -> Extended {
    Extended {
        background: background_slots(look),
        primary: primary_slots(look),
        secondary: secondary_slots(look),
        success: success_slots(look),
        warning: warning_slots(look),
        danger: danger_slots(look),
        is_dark: look.dark,
    }
}

fn background_slots(look: Tokens) -> palette::Background {
    let ink = look.ink;
    palette::Background {
        base: text_on(look.surface, ink),
        weakest: text_on(look.panel, ink),
        weaker: text_on(look.raised, ink),
        weak: text_on(look.raised, ink),
        neutral: text_on(look.canvas, ink),
        strong: text_on(look.border_strong, ink),
        stronger: text_on(look.border, ink),
        strongest: text_on(strongest_surface(look), ink),
    }
}

fn strongest_surface(look: Tokens) -> Color {
    if look.dark {
        look.muted
    } else {
        look.border_strong
    }
}

fn primary_slots(look: Tokens) -> palette::Primary {
    palette::Primary {
        base: text_on(look.accent, look.accent_text),
        weak: text_on(look.selection, look.ink),
        strong: text_on(look.accent_hover, look.accent_text),
    }
}

fn secondary_slots(look: Tokens) -> palette::Secondary {
    palette::Secondary {
        base: text_on(look.muted, look.ink),
        weak: text_on(look.muted, look.ink),
        strong: text_on(look.link, look.ink),
    }
}

fn success_slots(look: Tokens) -> palette::Success {
    let tone = text_on(look.success, look.ink);
    palette::Success {
        base: tone,
        weak: tone,
        strong: tone,
    }
}

fn warning_slots(look: Tokens) -> palette::Warning {
    let tone = text_on(look.warning, look.ink);
    palette::Warning {
        base: tone,
        weak: tone,
        strong: tone,
    }
}

fn danger_slots(look: Tokens) -> palette::Danger {
    let tone = text_on(look.danger, look.ink);
    palette::Danger {
        base: tone,
        weak: tone,
        strong: tone,
    }
}

/// Quiet rails keep navigation visible without competing with the page.
pub fn scrollbar(
    theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{self, Status};
    let look = tokens_of(theme);
    let mut style = scrollable::default(theme, status);
    let alpha = if matches!(status, Status::Dragged { .. }) {
        0.6
    } else {
        0.28
    };
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = None;
        rail.scroller.background = look.muted.scale_alpha(alpha).into();
        rail.scroller.border = iced::border::rounded(8);
    }
    match status {
        Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } => {
            style.vertical_rail.scroller.background = look.muted.scale_alpha(0.55).into();
        }
        Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } => {
            style.horizontal_rail.scroller.background = look.muted.scale_alpha(0.55).into();
        }
        _ => {}
    }
    style
}

#[cfg(test)]
mod tests {
    use super::{Appearance, iced, tokens, tokens_of};
    use iced::theme::Base;

    #[test]
    fn appearance_round_trips() {
        assert_eq!(Appearance::parse("light"), Some(Appearance::Light));
        assert_eq!(Appearance::parse("dark"), Some(Appearance::Dark));
        assert_eq!(Appearance::parse("tokyo"), None);
        for appearance in [Appearance::Light, Appearance::Dark] {
            assert_eq!(Appearance::parse(appearance.as_str()), Some(appearance));
        }
        assert_eq!(Appearance::Light.as_str(), "light");
        assert_eq!(Appearance::Dark.as_str(), "dark");
        assert_eq!(Appearance::Light.title(), "Paper");
        assert_eq!(Appearance::Dark.title(), "Ink");
    }

    #[test]
    fn themes_keep_editorial_colors_and_readable_text() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = iced(appearance);
            let t = tokens_of(&theme);
            assert_eq!(theme.name(), appearance.title());
            assert_eq!(t, tokens(appearance));
            for (surface_name, surface) in [
                ("surface", t.surface),
                ("panel", t.panel),
                ("canvas", t.canvas),
                ("raised", t.raised),
            ] {
                for (text_name, text) in [
                    ("ink", t.ink),
                    ("muted", t.muted),
                    ("link", t.link),
                    ("success", t.success),
                    ("warning", t.warning),
                    ("danger", t.danger),
                ] {
                    assert!(
                        text.relative_contrast(surface) >= 4.5,
                        "{appearance:?} {text_name}/{surface_name} must meet WCAG AA"
                    );
                }
            }
            for (name, fill) in [("accent", t.accent), ("accent_hover", t.accent_hover)] {
                assert!(
                    t.accent_text.relative_contrast(fill) >= 4.5,
                    "{appearance:?} accent_text/{name} must meet WCAG AA"
                );
            }
            for (name, surface) in [
                ("surface", t.surface),
                ("panel", t.panel),
                ("canvas", t.canvas),
            ] {
                assert!(
                    t.selection.relative_contrast(surface) >= 1.15,
                    "{appearance:?} selection/{name} must remain visible"
                );
            }
            assert!(
                t.ink.relative_contrast(t.selection) >= 4.5,
                "{appearance:?} ink/selection must meet WCAG AA"
            );
            assert_ne!(t.panel, t.raised);
            assert_ne!(t.track, t.canvas);
        }
    }
}
