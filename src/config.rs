//! Last workspace, theme, and fonts, under `$XDG_CONFIG_HOME/zarinotes`.

use std::path::{Path, PathBuf};

use crate::theme::{Appearance, ColorSlot, Colors, Customization, hex, parse_hex};

const LAST_WORKSPACE_FILE: &str = "last_workspace";
const APPEARANCE_FILE: &str = "theme";
const BACKLINKS_FILE: &str = "backlinks";
const COLORS_FILE: &str = "colors";
const FONTS_FILE: &str = "fonts";

/// `$XDG_CONFIG_HOME/zarinotes`, or `~/.config/zarinotes` when that variable
/// is unset.
fn config_dir() -> Option<PathBuf> {
    let base = match std::env::var_os("XDG_CONFIG_HOME") {
        Some(xdg) => PathBuf::from(xdg),
        None => PathBuf::from(std::env::var_os("HOME")?).join(".config"),
    };
    Some(base.join("zarinotes"))
}

fn read_config(name: &str) -> Option<String> {
    let path = config_dir()?.join(name);
    std::fs::read_to_string(path).ok()
}

/// Callers other than colors and fonts ignore failures: those files are
/// conveniences.
fn write_config(name: &str, bytes: &[u8]) -> std::io::Result<()> {
    let dir = config_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No configuration directory is available",
        )
    })?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join(name), bytes)
}

pub(crate) fn load_last_workspace() -> Option<PathBuf> {
    let text = read_config(LAST_WORKSPACE_FILE)?;
    Some(PathBuf::from(text.trim()))
}

pub(crate) fn save_last_workspace(dir: &Path) {
    let _ = write_config(LAST_WORKSPACE_FILE, dir.to_string_lossy().as_bytes());
}

pub(crate) fn load_appearance() -> Appearance {
    read_config(APPEARANCE_FILE)
        .and_then(|text| Appearance::parse(text.trim()))
        .unwrap_or(Appearance::Dark)
}

pub(crate) fn save_appearance(appearance: Appearance) {
    let _ = write_config(APPEARANCE_FILE, appearance.as_str().as_bytes());
}

/// Whether the backlink list under the editor starts open. Missing means open.
pub(crate) fn load_backlinks_open() -> bool {
    read_config(BACKLINKS_FILE).is_none_or(|text| text.trim() != "closed")
}

pub(crate) fn save_backlinks_open(open: bool) {
    let _ = write_config(BACKLINKS_FILE, if open { b"open" } else { b"closed" });
}

/// Overrides live separately from the appearance file, so resetting colors
/// keeps the chosen appearance.
pub(crate) fn load_customization() -> Customization {
    read_config(COLORS_FILE).map_or_else(Customization::default, |text| parse_customization(&text))
}

pub(crate) fn save_customization(customization: &Customization) -> std::io::Result<()> {
    write_config(
        COLORS_FILE,
        serialize_customization(customization)?.as_bytes(),
    )
}

/// Lines of `<appearance>.<slot>=#RRGGBB`. Unknown lines are skipped; an
/// appearance is used only when all four of its slots are present and valid.
fn parse_customization(text: &str) -> Customization {
    // Indexed by appearance, then slot.
    let mut fields = [[None; 4]; 2];
    let mut rejected = [false; 2];
    for line in text.lines() {
        let Some((appearance, slot, value)) = parse_field(line) else {
            continue;
        };
        let a = appearance as usize;
        let parsed = parse_hex(value.trim());
        // A malformed or duplicate known field invalidates only its appearance.
        rejected[a] |= parsed.is_none() || fields[a][slot.index()].is_some();
        fields[a][slot.index()] = parsed;
    }
    let mut customization = Customization::default();
    for appearance in [Appearance::Light, Appearance::Dark] {
        let a = appearance as usize;
        if rejected[a] {
            continue;
        }
        if let [Some(accent), Some(page), Some(sidebar), Some(text)] = fields[a] {
            let colors = Colors {
                accent,
                page,
                sidebar,
                text,
            };
            customization.set(appearance, colors);
        }
    }
    customization
}

/// The known appearance and slot of a `key=value` line, with its raw value.
fn parse_field(line: &str) -> Option<(Appearance, ColorSlot, &str)> {
    let (key, value) = line.split_once('=')?;
    let (appearance, slot) = key.trim().split_once('.')?;
    let appearance = Appearance::parse(appearance)?;
    let slot = ColorSlot::ALL.into_iter().find(|s| slot_key(*s) == slot)?;
    Some((appearance, slot, value))
}

/// The slot part of a key, e.g. `accent` in `dark.accent`.
fn slot_key(slot: ColorSlot) -> String {
    slot.label().to_ascii_lowercase()
}

fn serialize_customization(customization: &Customization) -> std::io::Result<String> {
    use std::fmt::Write;
    let mut text = String::new();
    for appearance in [Appearance::Light, Appearance::Dark] {
        if let Some(colors) = customization.override_for(appearance) {
            colors.validate().map_err(|message| {
                std::io::Error::new(std::io::ErrorKind::InvalidInput, message)
            })?;
            for slot in ColorSlot::ALL {
                // Formatting into a String cannot fail.
                let _ = writeln!(
                    text,
                    "{}.{}={}",
                    appearance.as_str(),
                    slot_key(slot),
                    hex(slot.get(colors))
                );
            }
        }
    }
    Ok(text)
}

/// Chosen font families. `None` means the system default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Fonts {
    pub interface: Option<String>,
    pub note: Option<String>,
}

pub(crate) fn load_fonts() -> Fonts {
    read_config(FONTS_FILE).map_or_else(Fonts::default, |text| parse_fonts(&text))
}

pub(crate) fn save_fonts(fonts: &Fonts) -> std::io::Result<()> {
    write_config(FONTS_FILE, serialize_fonts(fonts).as_bytes())
}

/// Lines of `interface=<family>` and `note=<family>`. A missing line means
/// the system default; unknown lines are skipped.
fn parse_fonts(text: &str) -> Fonts {
    let mut fonts = Fonts::default();
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let family = Some(value.trim().to_owned()).filter(|family| !family.is_empty());
        match key.trim() {
            "interface" => fonts.interface = family,
            "note" => fonts.note = family,
            _ => {}
        }
    }
    fonts
}

fn serialize_fonts(fonts: &Fonts) -> String {
    [("interface", &fonts.interface), ("note", &fonts.note)]
        .into_iter()
        .filter_map(|(key, family)| Some(format!("{key}={}\n", family.as_ref()?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_overrides_round_trip_and_reset_independently() {
        let mut c = Customization::default();
        let light = c.colors(Appearance::Light);
        let dark = c.colors(Appearance::Dark);
        c.set(Appearance::Light, light);
        c.set(Appearance::Dark, dark);
        let text = serialize_customization(&c).unwrap();
        assert_eq!(text.lines().count(), 8);
        assert_eq!(parse_customization(&text), c);
        c.reset(Appearance::Light);
        let text = serialize_customization(&c).unwrap();
        assert!(!text.contains("light."));
        assert_eq!(parse_customization(&text), c);
        assert_eq!(c.override_for(Appearance::Dark), Some(dark));
        c.reset(Appearance::Dark);
        assert_eq!(serialize_customization(&c).unwrap(), "");
        assert_eq!(parse_customization(""), c);
    }

    #[test]
    fn malformed_or_partial_overrides_do_not_replace_defaults() {
        let mut c = Customization::default();
        c.set(Appearance::Dark, c.colors(Appearance::Dark));
        let valid = serialize_customization(&c).unwrap();
        for malformed in [
            "light.accent=#123456\n",
            "light.accent=#invalid\nlight.page=#ffffff\nlight.sidebar=#eeeeee\nlight.text=#000000\n",
            "light.accent=#123456\nlight.accent=#abcdef\nlight.page=#ffffff\nlight.sidebar=#eeeeee\nlight.text=#000000\n",
            "# comment\nunknown=value\nlight.page\nlight.text=🦀\n",
        ] {
            assert_eq!(parse_customization(&format!("{valid}{malformed}")), c);
        }
        assert_eq!(
            parse_customization(&valid.replace("dark.text=", "dark.unknown=")),
            Customization::default()
        );
    }

    #[test]
    fn font_choices_round_trip() {
        let fonts = Fonts {
            interface: Some("Noto Sans".into()),
            note: Some("DejaVu Serif".into()),
        };
        let text = serialize_fonts(&fonts);
        assert_eq!(text, "interface=Noto Sans\nnote=DejaVu Serif\n");
        assert_eq!(parse_fonts(&text), fonts);
        let interface_only = Fonts {
            note: None,
            ..fonts
        };
        assert_eq!(
            parse_fonts(&serialize_fonts(&interface_only)),
            interface_only
        );
        assert_eq!(serialize_fonts(&Fonts::default()), "");
    }

    #[test]
    fn missing_or_malformed_font_lines_mean_the_system_default() {
        for text in ["", "note=\n", "# comment\nunknown=Inter\ninterface\n"] {
            assert_eq!(parse_fonts(text), Fonts::default());
        }
        assert_eq!(
            parse_fonts(" note = Cantarell \n"),
            Fonts {
                interface: None,
                note: Some("Cantarell".into()),
            }
        );
    }
}
