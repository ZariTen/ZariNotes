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

// ── scrolling ─────────────────────────────────────────────────────

/// Finds the live editor and the document scrollable, and computes the
/// scroll offset needed to bring the cursor line into view (if any).
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
        let bottom = top + LINE_HEIGHT;
        let margin = LINE_HEIGHT * 2.0;

        if top - margin < translation.y {
            operation::Outcome::Some((top - margin).max(0.0))
        } else if bottom + margin > translation.y + viewport.height {
            operation::Outcome::Some(bottom + margin - viewport.height)
        } else {
            operation::Outcome::None
        }
    }
}
