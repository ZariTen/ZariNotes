//! Press that also reports where the cursor was.
//!
//! `mouse_area`'s press message has no position, so a popup or a corner drag
//! cannot start from the pointer without this wrapper.

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{Tree, Widget, tree};
use iced::advanced::{Clipboard, Shell, overlay};
use iced::mouse;
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector};

/// Right-click. The sidebar popup is placed at the reported point.
pub fn spot<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_right: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    at(content, mouse::Button::Right, None, on_right)
}

/// Left-click. Used by the image corner handle.
pub fn press_at<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    on_press: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    at(
        content,
        mouse::Button::Left,
        Some(mouse::Interaction::ResizingDiagonallyDown),
        on_press,
    )
}

fn at<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    button: mouse::Button,
    interaction: Option<mouse::Interaction>,
    on_press: impl Fn(Point) -> Message + 'a,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: renderer::Renderer + 'a,
{
    Element::new(Spot {
        content: content.into(),
        button,
        interaction,
        on_press: Box::new(on_press),
    })
}

struct Spot<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    button: mouse::Button,
    interaction: Option<mouse::Interaction>,
    on_press: Box<dyn Fn(Point) -> Message + 'a>,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Spot<'_, Message, Theme, Renderer>
where
    Renderer: renderer::Renderer,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::stateless()
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content));
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn iced::advanced::widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        if shell.is_event_captured() {
            return;
        }
        let Event::Mouse(mouse::Event::ButtonPressed(button)) = event else {
            return;
        };
        if *button != self.button || !cursor.is_over(layout.bounds()) {
            return;
        }
        let Some(at) = cursor.position() else {
            return;
        };
        shell.publish((self.on_press)(at));
        shell.capture_event();
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        if let Some(interaction) = self.interaction
            && cursor.is_over(layout.bounds())
        {
            return interaction;
        }
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}
