//! Paper (warm ivory and sage) and Ink (charcoal and muted jade).
//!
//! Quiet, layered surfaces keep the writing in focus. Teal and jade provide
//! restrained emphasis, while text and semantic tones retain WCAG AA contrast
//! across the writing surface, sidebar, canvas, and raised controls.

use iced::theme::Base;
use iced::theme::palette::{self, Extended, Pair};
use iced::{Color, Theme};

/// WCAG AA minimum contrast for body text.
const AA_CONTRAST: f32 = 4.5;

/// [`AA_CONTRAST`] plus a margin, so f32 roundoff cannot leave a corrected
/// color just below AA.
const AA_CONTRAST_WITH_MARGIN: f32 = 4.5001;

/// Start of every [`custom`] theme name: `Custom <title> <hex colors>`.
const CUSTOM_NAME_PREFIX: &str = "Custom ";

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
    /// Accept only opaque RGB values. Clashing surfaces and unreadable text are
    /// adjusted later by [`tokens_with`], not rejected here.
    pub fn validate(self) -> Result<(), &'static str> {
        let opaque_rgb =
            |c: Color| c.a == 1.0 && [c.r, c.g, c.b].iter().all(|v| (0.0..=1.0).contains(v));
        if ColorSlot::ALL
            .into_iter()
            .all(|slot| opaque_rgb(slot.get(self)))
        {
            Ok(())
        } else {
            Err("Colors must be opaque RGB values.")
        }
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

    /// Position in [`Self::ALL`].
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

/// The user's color overrides, one optional set per appearance.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Customization {
    pub light: Option<Colors>,
    pub dark: Option<Colors>,
}

impl Customization {
    /// The override for `appearance`, or the built-in theme's colors.
    pub fn colors(self, appearance: Appearance) -> Colors {
        self.override_for(appearance).unwrap_or_else(|| {
            let defaults = tokens(appearance);
            Colors {
                accent: defaults.accent,
                page: defaults.surface,
                sidebar: defaults.panel,
                text: defaults.ink,
            }
        })
    }

    pub fn override_for(mut self, appearance: Appearance) -> Option<Colors> {
        *self.override_mut(appearance)
    }

    pub fn set(&mut self, appearance: Appearance, colors: Colors) {
        *self.override_mut(appearance) = Some(colors);
    }

    pub fn reset(&mut self, appearance: Appearance) {
        *self.override_mut(appearance) = None;
    }

    fn override_mut(&mut self, appearance: Appearance) -> &mut Option<Colors> {
        match appearance {
            Appearance::Light => &mut self.light,
            Appearance::Dark => &mut self.dark,
        }
    }
}

/// Shared surface, typography, interaction, and semantic color tokens.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tokens {
    /// Window background around the sidebar and page.
    pub canvas: Color,
    /// The writing page.
    pub surface: Color,
    /// The sidebar.
    pub panel: Color,
    /// Menus, tooltips, and hovered rows.
    pub raised: Color,
    /// Background of a segmented switch.
    pub track: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_text: Color,
    /// Body text.
    pub ink: Color,
    /// Secondary text.
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

pub fn tokens(appearance: Appearance) -> Tokens {
    match appearance {
        Appearance::Light => PAPER,
        Appearance::Dark => INK,
    }
}

/// The built-in tokens for `appearance` until valid overrides are supplied.
pub fn tokens_with(appearance: Appearance, colors: Option<Colors>) -> Tokens {
    match colors.filter(|c| c.validate().is_ok()) {
        Some(colors) => custom_tokens(appearance, colors),
        None => tokens(appearance),
    }
}

/// The tokens behind a theme made by [`custom`], read back from its name and
/// palette so styles need no global state. Re-deriving from already-corrected
/// colors gives the same tokens.
pub fn tokens_of(theme: &Theme) -> Tokens {
    let name = theme.name();
    let appearance = if name == Appearance::Dark.title() || is_custom_name(name, Appearance::Dark) {
        Appearance::Dark
    } else {
        Appearance::Light
    };
    if !is_custom_name(name, appearance) {
        return tokens(appearance);
    }
    let palette = theme.extended_palette();
    tokens_with(
        appearance,
        Some(Colors {
            accent: palette.primary.base.color,
            page: palette.background.base.color,
            sidebar: palette.background.weakest.color,
            text: palette.background.base.text,
        }),
    )
}

fn is_custom_name(name: &str, appearance: Appearance) -> bool {
    name.strip_prefix(CUSTOM_NAME_PREFIX)
        .and_then(|rest| rest.strip_prefix(appearance.title()))
        .is_some_and(|rest| rest.starts_with(' '))
}

pub fn custom(appearance: Appearance, colors: Option<Colors>) -> Theme {
    let colors = colors.filter(|c| c.validate().is_ok());
    let look = tokens_with(appearance, colors);
    // Names also change with the palette: iced uses them for widget caches.
    let name = match colors {
        Some(colors) => format!(
            "{CUSTOM_NAME_PREFIX}{} {} {} {} {}",
            appearance.title(),
            hex(colors.accent),
            hex(colors.page),
            hex(colors.sidebar),
            hex(colors.text)
        ),
        None => appearance.title().to_owned(),
    };
    Theme::custom_with_fn(name, base_palette(look), move |_| {
        if colors.is_some() {
            custom_extended(look)
        } else {
            extended(look)
        }
    })
}

fn custom_tokens(appearance: Appearance, colors: Colors) -> Tokens {
    let base = tokens(appearance);
    let page = colors.page;
    let shares_foreground = |panel: Color| !luminance_ranges(&[page, panel]).is_empty();
    // Some mixed-lightness surfaces cannot share any AA foreground. Preserve
    // the page and accent exactly, moving only the sidebar toward the page in
    // 1% increments until compatible. Stored choices remain untouched.
    let mut panel = colors.sidebar;
    if !shares_foreground(panel) {
        for step in 1..=100 {
            panel = mix(colors.sidebar, page, step as f32 / 100.0);
            if shares_foreground(panel) {
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
    // Hovering moves the accent away from its text color.
    let hover_toward = if accent_text == Color::BLACK {
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
        accent_hover: mix(colors.accent, hover_toward, 0.12),
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

/// [`extended`], with each pair's text made readable on its own fill: stock
/// widgets also paint text on accent and semantic fills, not just the page.
fn custom_extended(look: Tokens) -> Extended {
    let mut slots = extended(look);
    for pair in text_pairs_mut(&mut slots) {
        pair.text = foreground(pair.text, &[pair.color]);
    }
    slots
}

/// Every text-on-fill pair in an extended palette.
fn text_pairs_mut(slots: &mut Extended) -> [&mut Pair; 23] {
    [
        &mut slots.background.base,
        &mut slots.background.weakest,
        &mut slots.background.weaker,
        &mut slots.background.weak,
        &mut slots.background.neutral,
        &mut slots.background.strong,
        &mut slots.background.stronger,
        &mut slots.background.strongest,
        &mut slots.primary.base,
        &mut slots.primary.weak,
        &mut slots.primary.strong,
        &mut slots.secondary.base,
        &mut slots.secondary.weak,
        &mut slots.secondary.strong,
        &mut slots.success.base,
        &mut slots.success.weak,
        &mut slots.success.strong,
        &mut slots.warning.base,
        &mut slots.warning.weak,
        &mut slots.warning.strong,
        &mut slots.danger.base,
        &mut slots.danger.weak,
        &mut slots.danger.strong,
    ]
}

fn text_on(color: Color, text: Color) -> Pair {
    Pair { color, text }
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
        strongest: text_on(
            if look.dark {
                look.muted
            } else {
                look.border_strong
            },
            ink,
        ),
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

fn readable(color: Color, surfaces: &[Color]) -> bool {
    surfaces
        .iter()
        .all(|s| color.relative_contrast(*s) >= AA_CONTRAST)
}

/// `color` moved only as far toward black or white as AA contrast on every
/// surface needs, keeping its hue.
fn foreground(color: Color, surfaces: &[Color]) -> Color {
    const BISECTION_STEPS: usize = 30;
    if readable(color, surfaces) {
        return color;
    }
    let luminance = color.relative_luminance();
    let target = luminance_ranges(surfaces)
        .into_iter()
        .map(|(lower, upper)| luminance.clamp(lower, upper))
        .min_by(|a, b| (a - luminance).abs().total_cmp(&(b - luminance).abs()))
        .expect("custom surfaces always have a shared foreground");
    let darken = target < luminance;
    let extreme = if darken { Color::BLACK } else { Color::WHITE };
    // Bisect for the mix toward `extreme` whose luminance reaches `target`.
    let mut low = 0.0;
    let mut high = 1.0;
    for _ in 0..BISECTION_STEPS {
        let mid = (low + high) / 2.0;
        if (mix(color, extreme, mid).relative_luminance() < target) == darken {
            high = mid;
        } else {
            low = mid;
        }
    }
    mix(color, extreme, (low + high) / 2.0)
}

/// The text luminance intervals that reach AA contrast on every surface;
/// empty when no single text color is readable on all of them.
fn luminance_ranges(surfaces: &[Color]) -> Vec<(f32, f32)> {
    let mut ranges = vec![(0.0_f32, 1.0_f32)];
    for surface in surfaces {
        let luminance = surface.relative_luminance();
        // Text must be darker or lighter than the surface by the AA ratio.
        let allowed = [
            (0.0, (luminance + 0.05) / AA_CONTRAST_WITH_MARGIN - 0.05),
            (AA_CONTRAST_WITH_MARGIN * (luminance + 0.05) - 0.05, 1.0),
        ];
        ranges = ranges
            .into_iter()
            .flat_map(|(lower, upper)| {
                allowed.into_iter().filter_map(move |(min, max)| {
                    let lower = lower.max(min);
                    let upper = upper.min(max);
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

/// `a` blended toward `b` by `amount` (0 keeps `a`, 1 gives `b`).
fn mix(a: Color, b: Color, amount: f32) -> Color {
    Color::from_rgb(
        a.r + (b.r - a.r) * amount,
        a.g + (b.g - a.g) * amount,
        a.b + (b.b - a.b) * amount,
    )
}

pub(crate) const fn rgb(hex: u32) -> Color {
    Color::from_rgb8((hex >> 16) as u8, (hex >> 8) as u8, hex as u8)
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

/// Quiet rails keep navigation visible without competing with the page.
pub fn scrollbar(
    theme: &Theme,
    status: iced::widget::scrollable::Status,
) -> iced::widget::scrollable::Style {
    use iced::widget::scrollable::{self, Status};
    const IDLE_ALPHA: f32 = 0.28;
    const HOVERED_ALPHA: f32 = 0.55;
    const DRAGGED_ALPHA: f32 = 0.6;
    let look = tokens_of(theme);
    let mut style = scrollable::default(theme, status);
    let alpha = if matches!(status, Status::Dragged { .. }) {
        DRAGGED_ALPHA
    } else {
        IDLE_ALPHA
    };
    for rail in [&mut style.vertical_rail, &mut style.horizontal_rail] {
        rail.background = None;
        rail.scroller.background = look.muted.scale_alpha(alpha).into();
        rail.scroller.border = iced::border::rounded(8);
    }
    let hovered_rail = match status {
        Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } => Some(&mut style.vertical_rail),
        Status::Hovered {
            is_horizontal_scrollbar_hovered: true,
            ..
        } => Some(&mut style.horizontal_rail),
        _ => None,
    };
    if let Some(rail) = hovered_rail {
        rail.scroller.background = look.muted.scale_alpha(HOVERED_ALPHA).into();
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
                    text.relative_contrast(surface) >= AA_CONTRAST,
                    "{appearance:?} {colors:?}: {text:?} on {surface:?} = {}",
                    text.relative_contrast(surface)
                );
            }
        }
        assert!(t.ink.relative_contrast(t.selection) >= AA_CONTRAST);
        assert!(t.accent_text.relative_contrast(t.accent) >= AA_CONTRAST);
        assert!(t.accent_text.relative_contrast(t.accent_hover) >= AA_CONTRAST);
        let theme = custom(appearance, Some(colors));
        assert_eq!(
            tokens_of(&theme),
            t,
            "custom widget token resolution must be exact"
        );
        let mut p = *theme.extended_palette();
        for pair in text_pairs_mut(&mut p) {
            assert!(pair.text.relative_contrast(pair.color) >= AA_CONTRAST);
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
                        text.relative_contrast(surface) >= AA_CONTRAST,
                        "{appearance:?} {text_name}/{surface_name} must meet WCAG AA"
                    );
                }
            }
            for (name, fill) in [("accent", t.accent), ("accent_hover", t.accent_hover)] {
                assert!(
                    t.accent_text.relative_contrast(fill) >= AA_CONTRAST,
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
                t.ink.relative_contrast(t.selection) >= AA_CONTRAST,
                "{appearance:?} ink/selection must meet WCAG AA"
            );
            assert_ne!(t.panel, t.raised);
            assert_ne!(t.track, t.canvas);
        }
    }
}
