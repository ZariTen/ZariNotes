//! Paste. An image is saved beside the note; anything else is inserted as text.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use iced::Task;
use iced::widget::text_editor::{Action, Edit};

use super::{App, Doc, Message};
use crate::images::{self, ClipboardOffer, ImageBytes};
use crate::live;

impl App {
    pub(super) fn paste(&self) -> Task<Message> {
        if self.doc.is_none() {
            return Task::none();
        }
        Task::perform(async { images::read_clipboard() }, Message::ClipboardOffer)
    }

    pub(super) fn clipboard_offer(&mut self, offer: ClipboardOffer) -> Task<Message> {
        match offer {
            ClipboardOffer::Image(image) => self.insert_image(image),
            ClipboardOffer::None => iced::clipboard::read().map(|text| Message::PasteText {
                text,
                image_tool_missing: false,
            }),
            ClipboardOffer::ToolMissing => iced::clipboard::read().map(|text| Message::PasteText {
                text,
                image_tool_missing: true,
            }),
        }
    }

    pub(super) fn paste_text(
        &mut self,
        text: Option<String>,
        image_tool_missing: bool,
    ) -> Task<Message> {
        let Some(text) = text.filter(|text| !text.is_empty()) else {
            if image_tool_missing {
                self.notice = Some(
                    "Could not read an image from the clipboard. Install wl-clipboard on Wayland, or xclip on X11.".into(),
                );
            }
            return Task::none();
        };
        if let Some(path) = images::copied_image_file(&text) {
            return self.insert_copied_file(&path);
        }
        self.insert_text(text)
    }

    fn insert_copied_file(&mut self, path: &Path) -> Task<Message> {
        let Some(extension) = path.extension().and_then(|ext| ext.to_str()) else {
            return self.insert_text(path.display().to_string());
        };
        match std::fs::read(path) {
            Ok(bytes) => self.insert_image(ImageBytes {
                bytes,
                extension: extension.to_owned(),
            }),
            Err(error) => {
                self.notice = Some(format!("Could not read {}: {error}", path.display()));
                Task::none()
            }
        }
    }

    fn insert_image(&mut self, image: ImageBytes) -> Task<Message> {
        let Some(dir) = self.note_dir() else {
            self.notice = Some("Open a note before pasting an image.".into());
            return Task::none();
        };
        match images::save_image(&dir, &image) {
            Ok(filename) => {
                self.notice = None;
                let column = self.cursor().map(|cursor| cursor.column).unwrap_or(0);
                self.insert_text(images::paste_snippet(column, &filename))
            }
            Err(error) => {
                self.notice = Some(format!("Could not save the image: {error}"));
                Task::none()
            }
        }
    }

    fn insert_text(&mut self, text: String) -> Task<Message> {
        let action = Action::Edit(Edit::Paste(Arc::new(text)));
        match &self.doc {
            Some(Doc::Live(_)) => self.edit_live(live::Msg::Edit(action)),
            Some(Doc::Source(_)) => self.edit_source(action),
            None => Task::none(),
        }
    }

    pub(super) fn note_dir(&self) -> Option<PathBuf> {
        let workspace = self.workspace.as_ref()?;
        let note = self.current.as_ref()?;
        Some(workspace.join(note.parent().unwrap_or(Path::new(""))))
    }
}
