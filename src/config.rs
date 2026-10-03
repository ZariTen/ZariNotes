//! Last workspace and theme, under `$XDG_CONFIG_HOME/zarinotes`.

use std::path::{Path, PathBuf};

use crate::theme::Appearance;

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
