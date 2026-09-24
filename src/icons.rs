//! Small inline SVG icons (from Lucide, ISC license), tinted to the theme.

use std::sync::LazyLock;

use iced::widget::svg::{self, Svg};
use iced::{Color, Theme};

const FOLDER: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z"/></svg>"#;

const FOLDER_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2"/></svg>"#;

const FILE_TEXT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/></svg>"#;

static FOLDER_H: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(FOLDER.as_bytes()));
static FOLDER_OPEN_H: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(FOLDER_OPEN.as_bytes()));
static FILE_H: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(FILE_TEXT.as_bytes()));

const SIZE: f32 = 15.0;

fn icon<'a>(handle: &svg::Handle, color: impl Fn(&Theme) -> Color + 'a) -> Svg<'a> {
    Svg::new(handle.clone())
        .width(SIZE)
        .height(SIZE)
        .style(move |theme, _status| svg::Style {
            color: Some(color(theme)),
        })
}

/// Folder icon in the theme's accent color.
pub fn folder<'a>(open: bool) -> Svg<'a> {
    let handle = if open { &*FOLDER_OPEN_H } else { &*FOLDER_H };
    icon(handle, |t| t.extended_palette().primary.base.color)
}

/// Note icon; uses the on-accent color when the row is selected.
pub fn file<'a>(selected: bool) -> Svg<'a> {
    icon(&FILE_H, move |t| {
        let p = t.extended_palette();
        if selected {
            p.primary.base.text
        } else {
            p.background.base.text.scale_alpha(0.7)
        }
    })
}
