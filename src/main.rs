//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them with an
//! live preview (or as plain source).

/// The interface typeface, chosen in settings; see [`system_font`].
pub(crate) const UI_FONT: iced::Font = iced::Font::with_name(system_font::UI_FAMILY);
/// The typeface of note prose. Code stays monospace.
pub(crate) const NOTE_FONT: iced::Font = iced::Font::with_name(system_font::NOTE_FAMILY);
/// The system default, whatever is chosen in settings.
pub(crate) const DEFAULT_FONT: iced::Font = iced::Font::with_name(system_font::DEFAULT_FAMILY);

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
    let fonts = config::load_fonts();
    system_font::apply(system_font::UI_FAMILY, fonts.interface.as_deref());
    system_font::apply(system_font::NOTE_FAMILY, fonts.note.as_deref());
    system_font::apply(system_font::DEFAULT_FAMILY, None);
    app::run()
}
