//! Keep the cursor line inside the document scrollport.

use iced::advanced::widget::{self as advanced_widget, Id, Operation, operation};
use iced::widget::scrollable::AbsoluteOffset;
use iced::{Rectangle, Task, Vector};

use super::{EDITOR_ID, LINE_HEIGHT, Live, Msg, SCROLL_ID};

impl Live {
    pub(super) fn scroll_into_view(&self) -> Task<Msg> {
        let cursor_offset = self.editor.cursor().position.line as f32 * LINE_HEIGHT;
        advanced_widget::operate(ScrollIntoView {
            cursor_offset,
            editor: None,
            scroll: None,
        })
        .then(|y| {
            iced::widget::operation::scroll_to(
                Id::new(SCROLL_ID),
                AbsoluteOffset {
                    x: None,
                    y: Some(y),
                },
            )
        })
    }
}

/// Finds the live editor and the document scrollable, then reports the scroll
/// offset that brings the cursor line into view.
struct ScrollIntoView {
    cursor_offset: f32,
    editor: Option<Rectangle>,
    scroll: Option<(Rectangle, Rectangle, Vector)>,
}

impl Operation<f32> for ScrollIntoView {
    fn traverse(&mut self, operate: &mut dyn FnMut(&mut dyn Operation<f32>)) {
        operate(self);
    }

    fn scrollable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        content_bounds: Rectangle,
        translation: Vector,
        _state: &mut dyn operation::Scrollable,
    ) {
        if id == Some(&Id::new(SCROLL_ID)) {
            self.scroll = Some((bounds, content_bounds, translation));
        }
    }

    fn focusable(
        &mut self,
        id: Option<&Id>,
        bounds: Rectangle,
        _state: &mut dyn operation::Focusable,
    ) {
        if id == Some(&Id::new(EDITOR_ID)) {
            self.editor = Some(bounds);
        }
    }

    fn finish(&self) -> operation::Outcome<f32> {
        let (Some(editor), Some((viewport, content, translation))) = (self.editor, self.scroll)
        else {
            return operation::Outcome::None;
        };
        let top = editor.y - content.y + self.cursor_offset;
        match scroll_target(top, translation.y, viewport.height) {
            Some(y) => operation::Outcome::Some(y),
            None => operation::Outcome::None,
        }
    }
}

/// Scroll offset that keeps the cursor line, plus two lines of margin, inside
/// the viewport. `None` means it is already visible.
fn scroll_target(top: f32, current_y: f32, viewport_height: f32) -> Option<f32> {
    let bottom = top + LINE_HEIGHT;
    let margin = LINE_HEIGHT * 2.0;
    if top - margin < current_y {
        return Some((top - margin).max(0.0));
    }
    if bottom + margin > current_y + viewport_height {
        return Some(bottom + margin - viewport_height);
    }
    None
}
