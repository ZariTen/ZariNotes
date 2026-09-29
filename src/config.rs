//! Last workspace and theme, under `$XDG_CONFIG_HOME/zarinotes`.

use std::path::{Path, PathBuf};

use crate::theme::Appearance;

fn config_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("zarinotes"))
}

fn config_file() -> Option<PathBuf> {
    Some(config_dir()?.join("last_workspace"))
}

pub(crate) fn load_last_workspace() -> Option<PathBuf> {
    let s = std::fs::read_to_string(config_file()?).ok()?;
    Some(PathBuf::from(s.trim()))
}

pub(crate) fn save_last_workspace(dir: &Path) {
    if let Some(file) = config_file() {
        let _ = file.parent().map(std::fs::create_dir_all);
        let _ = std::fs::write(file, dir.to_string_lossy().as_bytes());
    }
}

pub(crate) fn load_appearance() -> Appearance {
    let Some(path) = config_dir().map(|d| d.join("theme")) else {
        return Appearance::Dark;
    };
    let Ok(text) = std::fs::read_to_string(path) else {
        return Appearance::Dark;
    };
    Appearance::parse(text.trim()).unwrap_or(Appearance::Dark)
}

pub(crate) fn save_appearance(appearance: Appearance) {
    if let Some(dir) = config_dir() {
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::fs::write(dir.join("theme"), appearance.as_str());
    }
}
