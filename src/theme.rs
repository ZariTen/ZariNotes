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

/// The four user-editable colors. Other theme colors are derived from these.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Colors {
    pub accent: Color,
    pub page: Color,
    pub sidebar: Color,
    pub text: Color,
}

impl Colors {
    /// Validate opaque RGB input. Incompatible surface lightness and unreadable
    /// text are adjusted automatically by `tokens_with`, not rejected.
    pub fn validate(self) -> Result<(), &'static str> {
        for slot in ColorSlot::ALL {
            let c = slot.get(self);
            if ![c.r, c.g, c.b]
                .into_iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
                || c.a != 1.0
            {
                return Err("Colors must be opaque RGB values.");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSlot {
    Accent,
    Page,
    Sidebar,
    Text,
}

impl ColorSlot {
    pub const ALL: [Self; 4] = [Self::Accent, Self::Page, Self::Sidebar, Self::Text];

    pub fn label(self) -> &'static str {
        match self {
            Self::Accent => "Accent",
            Self::Page => "Page",
            Self::Sidebar => "Sidebar",
            Self::Text => "Text",
        }
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn get(self, mut colors: Colors) -> Color {
        *self.get_mut(&mut colors)
    }

    pub fn get_mut(self, colors: &mut Colors) -> &mut Color {
        match self {
            Self::Accent => &mut colors.accent,
            Self::Page => &mut colors.page,
            Self::Sidebar => &mut colors.sidebar,
            Self::Text => &mut colors.text,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Customization {
    pub light: Option<Colors>,
    pub dark: Option<Colors>,
}

impl Customization {
    pub fn colors(self, appearance: Appearance) -> Colors {
        self.override_for(appearance).unwrap_or_else(|| {
            let t = tokens(appearance);
            Colors {
                accent: t.accent,
                page: t.surface,
                sidebar: t.panel,
                text: t.ink,
            }
        })
    }

    pub fn override_for(mut self, appearance: Appearance) -> Option<Colors> {
        *self.slot(appearance)
    }

    pub fn set(&mut self, appearance: Appearance, colors: Colors) {
        *self.slot(appearance) = Some(colors);
    }

    pub fn reset(&mut self, appearance: Appearance) {
        *self.slot(appearance) = None;
    }

    fn slot(&mut self, appearance: Appearance) -> &mut Option<Colors> {
        match appearance {
            Appearance::Light => &mut self.light,
            Appearance::Dark => &mut self.dark,
        }
    }
}

/// Exactly six ASCII hexadecimal digits, with an optional leading `#`.
pub fn parse_hex(value: &str) -> Option<Color> {
    let digits = value.strip_prefix('#').unwrap_or(value);
    if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u32::from_str_radix(digits, 16).ok().map(rgb)
}

pub fn hex(color: Color) -> String {
    let [r, g, b, _] = color.into_rgba8();
    format!("#{r:02X}{g:02X}{b:02X}")
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
    pub ink: Color,
    pub muted: Color,
    pub border: Color,
    pub border_strong: Color,
    pub link: Color,
    pub selection: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub focus: Color,
    pub dark: bool,
}

pub(crate) const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
}

pub fn tokens(appearance: Appearance) -> Tokens {
    match appearance {
        Appearance::Light => PAPER,
        Appearance::Dark => INK,
    }
}

/// Resolve custom colors from the actual palette, without process-global state.
/// Reconstructing with the already-corrected foreground is intentionally stable.
pub fn tokens_of(theme: &Theme) -> Tokens {
    let name = theme.name();
    let custom_dark = name.starts_with("Custom Ink ");
    let appearance = if custom_dark || name == Appearance::Dark.title() {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    if custom_dark || name.starts_with("Custom Paper ") {
        let p = theme.extended_palette();
        return tokens_with(
            appearance,
            Some(Colors {
                accent: p.primary.base.color,
                page: p.background.base.color,
                sidebar: p.background.weakest.color,
                text: p.background.base.text,
            }),
        );
    }
    tokens(appearance)
}

pub fn custom(appearance: Appearance, colors: Option<Colors>) -> Theme {
    let colors = colors.filter(|c| c.validate().is_ok());
    let look = tokens_with(appearance, colors);
    // Names also change with the palette: iced uses them for widget caches.
    let name = if let Some(colors) = colors {
        format!(
            "Custom {} {} {} {} {}",
            appearance.title(),
            hex(colors.accent),
            hex(colors.page),
            hex(colors.sidebar),
            hex(colors.text)
        )
    } else {
        appearance.title().to_owned()
    };
    Theme::custom_with_fn(name, base_palette(look), move |_| {
        if colors.is_some() {
            custom_extended(look)
        } else {
            extended(look)
        }
    })
}

/// Preserve the built-in palettes exactly until an override is supplied.
pub fn tokens_with(appearance: Appearance, colors: Option<Colors>) -> Tokens {
    match colors.filter(|c| c.validate().is_ok()) {
        Some(colors) => custom_tokens(appearance, colors),
        None => tokens(appearance),
    }
}

fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * amount,
        a.g + (b.g - a.g) * amount,
        a.b + (b.b - a.b) * amount,
    )
}

// Intersect the allowed foreground-luminance intervals for every surface.
// A tiny margin prevents f32 roundoff from dropping a corrected color below AA.
fn luminance_ranges(surfaces: &[Color]) -> Vec<(f32, f32)> {
    const RATIO: f32 = 4.5001;
    let mut ranges = vec![(0.0_f32, 1.0_f32)];
    for surface in surfaces {
        let lum = surface.relative_luminance();
        let allowed = [
            (0.0, (lum + 0.05) / RATIO - 0.05),
            (RATIO * (lum + 0.05) - 0.05, 1.0),
        ];
        ranges = ranges
            .into_iter()
            .flat_map(|(lo, hi)| {
                allowed.into_iter().filter_map(move |(a, b)| {
                    let lower = lo.max(a);
                    let upper = hi.min(b);
                    (lower <= upper).then_some((lower, upper))
                })
            })
            .collect();
    }
    ranges
}

/// Whichever of black or white reads better on `fill`.
pub fn black_or_white(fill: Color) -> Color {
    if Color::BLACK.relative_contrast(fill) >= Color::WHITE.relative_contrast(fill) {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

fn readable(color: Color, surfaces: &[Color]) -> bool {
    surfaces.iter().all(|s| color.relative_contrast(*s) >= 4.5)
}

/// Move only as far toward black/white as needed, retaining the requested hue.
fn foreground(color: Color, surfaces: &[Color]) -> Color {
    if readable(color, surfaces) {
        return color;
    }
    let lum = color.relative_luminance();
    let target = luminance_ranges(surfaces)
        .into_iter()
        .map(|(lo, hi)| lum.clamp(lo, hi))
        .min_by(|a, b| (a - lum).abs().total_cmp(&(b - lum).abs()))
        .expect("custom surfaces always have a shared foreground");
    let end = if target < lum {
        Color::BLACK
    } else {
        Color::WHITE
    };
    let mut lo = 0.0;
    let mut hi = 1.0;
    for _ in 0..30 {
        let mid = (lo + hi) / 2.0;
        let candidate = mix(color, end, mid);
        if (candidate.relative_luminance() < target) == (target < lum) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    mix(color, end, (lo + hi) / 2.0)
}

fn custom_tokens(appearance: Appearance, colors: Colors) -> Tokens {
    let base = tokens(appearance);
    let page = colors.page;
    let mut panel = colors.sidebar;
    // Some mixed-lightness surfaces cannot share any AA foreground. Preserve
    // the page and accent exactly, moving only the sidebar toward the page in
    // 1% increments until compatible. Stored choices remain untouched.
    if luminance_ranges(&[page, panel]).is_empty() {
        for step in 1..=100 {
            panel = mix(colors.sidebar, page, step as f32 / 100.0);
            if !luminance_ranges(&[page, panel]).is_empty() {
                break;
            }
        }
    }
    let mut canvas = mix(page, Color::BLACK, if base.dark { 0.16 } else { 0.035 });
    let mut raised = mix(page, Color::WHITE, if base.dark { 0.045 } else { 0.35 });
    // Very narrow feasible ranges (e.g. black beside white) leave no room for
    // surface layering. Readability takes precedence over decorative elevation.
    if luminance_ranges(&[page, panel, canvas, raised]).is_empty() {
        canvas = page;
        raised = page;
    }
    let surfaces = [page, panel, canvas, raised];
    let ink = foreground(colors.text, &surfaces);
    let accent_text = black_or_white(colors.accent);
    let accent_end = if accent_text == Color::BLACK {
        Color::WHITE
    } else {
        Color::BLACK
    };
    let mut selection = mix(page, colors.accent, 0.18);
    if !readable(ink, &[selection]) {
        selection = page;
    }
    let mut track = mix(panel, ink, 0.08);
    if !readable(ink, &[track]) {
        track = panel;
    }
    Tokens {
        canvas,
        surface: page,
        panel,
        raised,
        track,
        accent: colors.accent,
        accent_hover: mix(colors.accent, accent_end, 0.12),
        accent_text,
        ink,
        muted: foreground(mix(ink, page, 0.30), &surfaces),
        border: mix(page, ink, 0.16),
        border_strong: mix(page, ink, 0.34),
        link: foreground(colors.accent, &surfaces),
        selection,
        success: foreground(base.success, &surfaces),
        warning: foreground(base.warning, &surfaces),
        danger: foreground(base.danger, &surfaces),
        focus: foreground(colors.accent, &surfaces),
        dark: base.dark,
    }
}

/// Stock widgets can paint on semantic fills too; choose text per pair instead
/// of reusing the page foreground. Keep the legacy palette mapping unchanged.
fn custom_extended(look: Tokens) -> Extended {
    let mut p = extended(look);
    for pair in pairs_mut(&mut p) {
        pair.text = foreground(pair.text, &[pair.color]);
    }
    p
}

/// Every text-on-fill pair in an extended palette.
fn pairs_mut(p: &mut Extended) -> [&mut Pair; 23] {
    [
        &mut p.background.base,
        &mut p.background.weakest,
        &mut p.background.weaker,
        &mut p.background.weak,
        &mut p.background.neutral,
        &mut p.background.strong,
        &mut p.background.stronger,
        &mut p.background.strongest,
        &mut p.primary.base,
        &mut p.primary.weak,
        &mut p.primary.strong,
        &mut p.secondary.base,
        &mut p.secondary.weak,
        &mut p.secondary.strong,
        &mut p.success.base,
        &mut p.success.weak,
        &mut p.success.strong,
        &mut p.warning.base,
        &mut p.warning.weak,
        &mut p.warning.strong,
        &mut p.danger.base,
        &mut p.danger.weak,
        &mut p.danger.strong,
    ]
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
    ink: rgb(0x242D2A),
    muted: rgb(0x59645E),
    border: rgb(0xDCE1D8),
    border_strong: rgb(0xB7C3B8),
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
    ink: rgb(0xE9EDE5),
    muted: rgb(0xADB8AE),
    border: rgb(0x303B35),
    border_strong: rgb(0x4F6156),
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
    use super::*;
    use iced::theme::Base;

    #[test]
    fn hex_is_strict_and_round_trips() {
        for s in [
            "",
            "#",
            "fff",
            "#12345",
            "#1234567",
            "#12345678",
            "##123456",
            "#invalid",
            "12345g",
            " 123456",
            "123456\n",
            "+12345",
            "🦀ff",
        ] {
            assert_eq!(parse_hex(s), None, "{s:?}");
        }
        assert_eq!(parse_hex("aBcD01"), Some(rgb(0xABCD01)));
        assert_eq!(parse_hex("#abcdef"), Some(rgb(0xABCDEF)));
        for c in [Color::WHITE, Color::BLACK, rgb(0x1A2B3C)] {
            assert_eq!(parse_hex(&hex(c)), Some(c));
        }
        assert_eq!(hex(rgb(0x12AB03)), "#12AB03");
    }

    fn assert_custom_contrast(appearance: Appearance, colors: Colors) {
        let t = tokens_with(appearance, Some(colors));
        assert_eq!(t.surface, colors.page);
        assert_eq!(t.accent, colors.accent);
        for surface in [t.surface, t.panel, t.canvas, t.raised] {
            for text in [
                t.ink, t.muted, t.link, t.success, t.warning, t.danger, t.focus,
            ] {
                assert!(
                    text.relative_contrast(surface) >= 4.5,
                    "{appearance:?} {colors:?}: {text:?} on {surface:?} = {}",
                    text.relative_contrast(surface)
                );
            }
        }
        assert!(t.ink.relative_contrast(t.selection) >= 4.5);
        assert!(t.accent_text.relative_contrast(t.accent) >= 4.5);
        assert!(t.accent_text.relative_contrast(t.accent_hover) >= 4.5);
        let theme = custom(appearance, Some(colors));
        assert_eq!(
            tokens_of(&theme),
            t,
            "custom widget token resolution must be exact"
        );
        let mut p = *theme.extended_palette();
        for pair in pairs_mut(&mut p) {
            assert!(pair.text.relative_contrast(pair.color) >= 4.5);
        }
    }

    #[test]
    fn extremes_and_mixed_surfaces_are_adjusted_without_losing_page_or_accent() {
        for appearance in [Appearance::Light, Appearance::Dark] {
            for (page, sidebar, text, accent) in [
                (0xFFFFFF, 0xFFFFFF, 0xFFFFFF, 0xFFFFFF),
                (0x000000, 0x000000, 0x000000, 0x000000),
                (0x000000, 0xFFFFFF, 0xFFFFFF, 0xFF00FF),
                (0xFFFFFF, 0x000000, 0x000000, 0xFFFF00),
                (0x595959, 0xCCCCCC, 0xAAAAAA, 0x00FFFF),
                (0xCCCCCC, 0x595959, 0x777777, 0xFF0000),
                (0x757575, 0x757575, 0x757575, 0x00FF00),
            ] {
                assert_custom_contrast(
                    appearance,
                    Colors {
                        page: rgb(page),
                        sidebar: rgb(sidebar),
                        text: rgb(text),
                        accent: rgb(accent),
                    },
                );
            }
            let c = Colors {
                page: Color::WHITE,
                sidebar: Color::WHITE,
                text: Color::BLACK,
                accent: rgb(0x123456),
            };
            assert_eq!(tokens_with(appearance, Some(c)).ink, c.text);
        }
        let mixed = Colors {
            page: rgb(0x595959),
            sidebar: rgb(0xCCCCCC),
            text: Color::WHITE,
            accent: Color::BLACK,
        };
        assert_ne!(
            tokens_with(Appearance::Light, Some(mixed)).panel,
            mixed.sidebar
        );
    }

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
            let theme = custom(appearance, None);
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
