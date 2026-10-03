//! Retro Classic (light) and Dolch Noir (dark).
//!
//! Surfaces follow the keyboard specs: canvas is the case, the panel is the
//! modifier cluster, the writing surface is an alpha keycap, and accent
//! buttons are the Esc/Enter keys. Success, warning, danger, and link colors
//! are not in those specs; they are vintage-family tones chosen so they stay
//! readable on the writing surface.

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
            Self::Light => "Retro Classic",
            Self::Dark => "Dolch Noir",
        }
    }
}

/// Exact spec tokens, plus the few semantic colors the specs leave out.
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
        Appearance::Light => RETRO,
        Appearance::Dark => DOLCH,
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

const RETRO: Tokens = Tokens {
    canvas: rgb(0xE2DDD1),
    surface: rgb(0xECE7DC),
    panel: rgb(0xB8C4CE),
    raised: rgb(0xF7F5EE),
    track: rgb(0xD5CEBF),
    accent: rgb(0x121417),
    accent_hover: rgb(0x2A3038),
    accent_text: rgb(0xECE7DC),
    accent_edge: rgb(0x050607),
    ink: rgb(0x1E2024),
    muted: rgb(0x4A5056),
    border: rgb(0xD5CEBF),
    border_strong: rgb(0x9BAAB6),
    lip: rgb(0xC4BCAC),
    // Deep slate steel: the spec accent is near-black, which would vanish as a link.
    link: rgb(0x3E5A68),
    selection: rgb(0xC5D0D8),
    success: rgb(0x3F5C38),
    warning: rgb(0x8A5520),
    danger: rgb(0x9C3B38),
    focus: rgb(0x121417),
    dark: false,
};

const DOLCH: Tokens = Tokens {
    canvas: rgb(0x131417),
    surface: rgb(0x2B2E33),
    panel: rgb(0x1E2024),
    raised: rgb(0x393D44),
    track: rgb(0x24272C),
    accent: rgb(0x528296),
    accent_hover: rgb(0x6C9FB5),
    // Spec allows white or inverse. Inverse is the higher-contrast of the two
    // on this mid cyan (about 4.4:1; neither allowed legend clears 4.5).
    accent_text: rgb(0x131417),
    accent_edge: rgb(0x335564),
    ink: rgb(0xEDEAE2),
    muted: rgb(0x9AA0A6),
    border: rgb(0x24272C),
    border_strong: rgb(0x3D424A),
    lip: rgb(0x15171A),
    link: rgb(0x8EC3D4),
    selection: rgb(0x3E5C6C),
    success: rgb(0xB7C7A4),
    warning: rgb(0xE4C07A),
    danger: rgb(0xE7A29C),
    focus: rgb(0x6C9FB5),
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

/// Map spec surfaces onto iced's generated slots so stock widgets (rules,
/// scrollbars, inputs, primary buttons) land on the keyboard colors.
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

#[cfg(test)]
mod tests {
    use super::{Appearance, iced, tokens, tokens_of};
    use iced::theme::Base;

    #[test]
    fn appearance_round_trips() {
        assert_eq!(Appearance::parse("light"), Some(Appearance::Light));
        assert_eq!(Appearance::parse("dark"), Some(Appearance::Dark));
        assert_eq!(Appearance::parse("tokyo"), None);
        assert_eq!(
            Appearance::parse(Appearance::Light.as_str()),
            Some(Appearance::Light)
        );
    }

    #[test]
    fn themes_keep_spec_colors_and_readable_text() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            let theme = iced(appearance);
            let t = tokens_of(&theme);
            assert_eq!(theme.name(), appearance.title());
            assert_eq!(t.ink, tokens(appearance).ink);
            assert!(
                t.ink.relative_contrast(t.surface) >= 4.5,
                "{appearance:?} ink/surface"
            );
            assert!(
                t.ink.relative_contrast(t.panel) >= 4.5,
                "{appearance:?} ink/panel"
            );
            assert!(
                t.ink.relative_contrast(t.canvas) >= 4.5,
                "{appearance:?} ink/canvas"
            );
            // Dolch's accent cyan is mid-luminance; the spec's own legend colors top out near 4.4:1.
            let accent_floor = if t.dark { 4.3 } else { 4.5 };
            assert!(
                t.accent_text.relative_contrast(t.accent) >= accent_floor,
                "{appearance:?} accent"
            );
            assert!(
                t.link.relative_contrast(t.surface) >= 4.5,
                "{appearance:?} link"
            );
            assert!(
                t.success.relative_contrast(t.surface) >= 4.5,
                "{appearance:?} success"
            );
            assert!(
                t.warning.relative_contrast(t.surface) >= 4.5,
                "{appearance:?} warning"
            );
            assert!(
                t.danger.relative_contrast(t.surface) >= 4.5,
                "{appearance:?} danger"
            );
            assert!(
                t.selection.relative_contrast(t.surface) >= 1.15,
                "{appearance:?} selection"
            );
            assert_ne!(t.panel, t.raised);
            assert_ne!(t.track, t.canvas);
        }
    }
}
