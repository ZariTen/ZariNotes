//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them with an
//! live preview (or as plain source).

/// Embedded typography keeps UI weights consistent across Linux desktops.
pub(crate) const UI_FONT: iced::Font = iced::Font::with_name("Inter");

mod app;
mod config;
mod highlight;
mod icons;
mod images;
mod links;
mod live;
mod spot;
mod theme;
mod tree;

fn main() -> iced::Result {
    app::run()
}
