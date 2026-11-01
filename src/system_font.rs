//! Interface and note text draw with alias families, each pointed at the
//! family chosen in settings or the user's default sans-serif font.
//!
//! cosmic-text uses a family only for weights it has an exact face for, and
//! counts a variable font as one regular face, so Medium or Bold text would
//! otherwise come from unrelated fonts. An alias gets a face for every weight
//! the app uses, each borrowing the nearest real face of its family.
//!
//! iced shapes all-ASCII text without font fallback, so a family missing a
//! printable ASCII character draws it as a box. Only families whose regular
//! face covers printable ASCII are offered or applied.

use std::borrow::Cow;

use iced::advanced::graphics::text::cosmic_text::FontSystem;
use iced::advanced::graphics::text::cosmic_text::fontdb::{
    FaceInfo, ID, Language, Stretch, Style, Weight,
};
use iced::advanced::graphics::text::font_system;

pub(crate) const UI_FAMILY: &str = "ZariNotes UI";
pub(crate) const NOTE_FAMILY: &str = "ZariNotes Note";
/// Always the system default, for text that must stay readable whatever
/// font is chosen, such as the font pickers themselves.
pub(crate) const DEFAULT_FAMILY: &str = "ZariNotes Default";
const ALIASES: [&str; 3] = [UI_FAMILY, NOTE_FAMILY, DEFAULT_FAMILY];
const WEIGHTS: [u16; 4] = [400, 500, 600, 700];

fn has_family(face: &FaceInfo, name: &str) -> bool {
    face.families.iter().any(|(family, _)| family == name)
}

/// The face an alias's regular weight borrows from `family`.
fn regular_face(fonts: &FontSystem, family: &str) -> Option<ID> {
    fonts
        .db()
        .faces()
        .filter(|face| {
            has_family(face, family)
                && face.style == Style::Normal
                && face.stretch == Stretch::Normal
        })
        .min_by_key(|face| face.weight.0.abs_diff(400))
        .map(|face| face.id)
}

/// Whether `family` can draw interface and note text by itself.
fn draws_ascii(fonts: &mut FontSystem, family: &str) -> bool {
    let Some(id) = regular_face(fonts, family) else {
        return false;
    };
    fonts.get_font(id, Weight::NORMAL).is_some_and(|font| {
        let charmap = font.as_swash().charmap();
        (' '..='~').all(|ch| charmap.map(ch) != 0)
    })
}

/// Points `alias` at `family`, or at the system default when `family` is
/// `None`, not installed, or cannot draw ASCII. Without any, text keeps
/// cosmic-text's fallback.
pub(crate) fn apply(alias: &str, family: Option<&str>) {
    let Ok(mut fonts) = font_system().write() else {
        return;
    };
    let raw = fonts.raw();
    let chosen = family
        .filter(|name| draws_ascii(raw, name))
        .map(str::to_owned)
        .into_iter()
        .chain(default_families())
        .find(|name| regular_face(raw, name).is_some());
    let db = raw.db_mut();
    let stale: Vec<_> = db
        .faces()
        .filter(|face| has_family(face, alias))
        .map(|face| face.id)
        .collect();
    for id in stale {
        db.remove_face(id);
    }
    if let Some(chosen) = chosen {
        let faces: Vec<FaceInfo> = db
            .faces()
            .filter(|face| has_family(face, &chosen) && face.stretch == Stretch::Normal)
            .cloned()
            .collect();
        for style in [Style::Normal, Style::Italic] {
            for weight in WEIGHTS {
                let nearest = faces
                    .iter()
                    .filter(|face| face.style == style)
                    .min_by_key(|face| face.weight.0.abs_diff(weight));
                if let Some(nearest) = nearest {
                    db.push_face_info(FaceInfo {
                        families: vec![(alias.to_owned(), Language::English_UnitedStates)],
                        weight: Weight(weight),
                        ..nearest.clone()
                    });
                }
            }
        }
    }
    // Loading nothing still bumps the font system's version, which makes iced
    // shape text that is already laid out again, now with the new faces.
    fonts.load_font(Cow::Owned(Vec::new()));
}

/// Installed families that can draw ASCII, sorted, for the font pickers.
pub(crate) fn families() -> Vec<String> {
    let Ok(mut fonts) = font_system().write() else {
        return Vec::new();
    };
    let raw = fonts.raw();
    let mut names: Vec<String> = raw
        .db()
        .faces()
        .filter_map(|face| Some(face.families.first()?.0.clone()))
        // Hidden system faces, the aliases, and iced's icon font are not text fonts.
        .filter(|name| {
            !name.starts_with('.') && !ALIASES.contains(&name.as_str()) && name != "Iced-Icons"
        })
        .collect();
    names.sort_by_key(|name| name.to_lowercase());
    names.dedup();
    names.retain(|name| draws_ascii(raw, name));
    names
}

/// The platform UI font, first choice first.
fn default_families() -> Vec<String> {
    if cfg!(target_os = "macos") {
        return vec![".SF NS".into(), "Helvetica".into()];
    }
    if cfg!(windows) {
        return vec!["Segoe UI".into()];
    }
    // fontconfig resolves the desktop's default, including user overrides.
    std::process::Command::new("fc-match")
        .args(["-f", "%{family[0]}", "sans-serif"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|name| name.trim().to_owned())
        .into_iter()
        .collect()
}
