//! ZariNotes — a minimal Markdown notes app.
//!
//! Pick a workspace folder, browse its `.md` files, and edit them with an
//! live preview (or as plain source).

mod app;
mod config;
mod highlight;
mod icons;
mod images;
mod live;
mod spot;
mod theme;
mod tree;

fn main() -> iced::Result {
    app::run()
}
