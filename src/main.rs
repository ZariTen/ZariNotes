//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them with an
//! live preview (or as plain source).

/// The UI typeface, the system default sans-serif; see [`system_font`].
pub(crate) const UI_FONT: iced::Font = iced::Font::with_name(system_font::FAMILY);

mod app;
mod config;
mod highlight;
mod icons;
mod images;
mod links;
mod live;
mod spot;
mod system_font;
mod theme;
mod tree;

fn main() -> iced::Result {
    system_font::register();
    app::run()
}
