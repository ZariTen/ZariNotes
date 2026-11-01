//! The UI draws with the user's default sans-serif font, registered under an
//! alias family.
//!
//! cosmic-text uses a family only for weights it has an exact face for, and
//! counts a variable font as one regular face, so Medium or Bold text would
//! otherwise come from unrelated fonts. The alias gets a face for every weight
//! the app uses, each borrowing the nearest real face of the default family.

use iced::advanced::graphics::text::cosmic_text::fontdb::{
    FaceInfo, Language, Stretch, Style, Weight,
};
use iced::advanced::graphics::text::font_system;

pub(crate) const FAMILY: &str = "ZariNotes UI";
const WEIGHTS: [u16; 4] = [400, 500, 600, 700];

/// Registers [`FAMILY`]. Without a default family, text keeps cosmic-text's
/// fallback.
pub(crate) fn register() {
    let Ok(mut fonts) = font_system().write() else {
        return;
    };
    let db = fonts.raw().db_mut();
    let has_family =
        |face: &FaceInfo, name: &str| face.families.iter().any(|(family, _)| family == name);
    let Some(chosen) = default_families()
        .into_iter()
        .find(|name| db.faces().any(|face| has_family(face, name)))
    else {
        return;
    };
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
                    families: vec![(FAMILY.to_owned(), Language::English_UnitedStates)],
                    weight: Weight(weight),
                    ..nearest.clone()
                });
            }
        }
    }
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
