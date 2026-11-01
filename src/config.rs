//! Last workspace and theme, under `$XDG_CONFIG_HOME/zarinotes`.

use std::path::{Path, PathBuf};

use crate::theme::{Appearance, ColorSlot, Colors, Customization, hex, parse_hex};

fn config_dir() -> Option<PathBuf> {
    Some(config_base()?.join("zarinotes"))
}

/// `$XDG_CONFIG_HOME`, or `~/.config` when that variable is unset.
fn config_base() -> Option<PathBuf> {
    if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg));
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config"))
}

fn read_config(name: &str) -> Option<String> {
    let path = config_dir()?.join(name);
    std::fs::read_to_string(path).ok()
}

fn write_config(name: &str, bytes: &[u8]) {
    let Some(dir) = config_dir() else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join(name), bytes);
}

pub(crate) fn load_last_workspace() -> Option<PathBuf> {
    let text = read_config("last_workspace")?;
    Some(PathBuf::from(text.trim()))
}

pub(crate) fn save_last_workspace(dir: &Path) {
    write_config("last_workspace", dir.to_string_lossy().as_bytes());
}

pub(crate) fn load_appearance() -> Appearance {
    let Some(text) = read_config("theme") else {
        return Appearance::Dark;
    };
    Appearance::parse(text.trim()).unwrap_or(Appearance::Dark)
}

pub(crate) fn save_appearance(appearance: Appearance) {
    write_config("theme", appearance.as_str().as_bytes());
}

/// Whether the backlink list under the editor starts open. Missing means open.
pub(crate) fn load_backlinks_open() -> bool {
    read_config("backlinks").is_none_or(|text| text.trim() != "closed")
}

pub(crate) fn save_backlinks_open(open: bool) {
    write_config("backlinks", if open { b"open" } else { b"closed" });
}

/// Overrides live separately from `theme`, preserving the selected appearance.
pub(crate) fn load_customization() -> Customization {
    read_config("colors").map_or_else(Customization::default, |text| parse_customization(&text))
}

pub(crate) fn save_customization(customization: &Customization) -> std::io::Result<()> {
    let text = serialize_customization(customization)?;
    let dir = config_dir().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "No configuration directory is available",
        )
    })?;
    std::fs::create_dir_all(&dir)?;
    std::fs::write(dir.join("colors"), text)
}

fn parse_customization(text: &str) -> Customization {
    let mut values = [[None; 4]; 2];
    let mut invalid = [false; 2];
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        let Some((appearance, slot)) = key.trim().split_once('.') else {
            continue;
        };
        let index = match appearance {
            "light" => 0,
            "dark" => 1,
            _ => continue,
        };
        let slot = match slot {
            "accent" => ColorSlot::Accent,
            "page" => ColorSlot::Page,
            "sidebar" => ColorSlot::Sidebar,
            "text" => ColorSlot::Text,
            _ => continue,
        };
        let parsed = parse_hex(value.trim());
        // A malformed or duplicate known field invalidates only its appearance.
        invalid[index] |= parsed.is_none() || values[index][slot.index()].is_some();
        values[index][slot.index()] = parsed;
    }
    let mut customization = Customization::default();
    for (index, appearance) in [Appearance::Light, Appearance::Dark]
        .into_iter()
        .enumerate()
    {
        if invalid[index] {
            continue;
        }
        if let [Some(accent), Some(page), Some(sidebar), Some(text)] = values[index] {
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
                    slot.label().to_ascii_lowercase(),
                    hex(slot.get(colors))
                );
            }
        }
    }
    Ok(text)
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
}
