//! Rendered Markdown, with the cursor's segment shown as source.

use iced::advanced::widget::Id;
use iced::font::Weight;
use iced::keyboard::{Key, key::Named};
use iced::widget::text_editor::{Binding, KeyPress};
use iced::widget::{
    checkbox, column, container, markdown, mouse_area, row, scrollable, space, text, text_editor,
};
use iced::{Border, Color, Element, Fill, Font, Padding, Theme, mouse};

use super::split::continue_list;
use super::{EDITOR_ID, Kind, LINE_HEIGHT, Live, Msg, Nav, SCROLL_ID, Segment, TEXT_SIZE};
use crate::highlight;

impl Live {
    pub fn view<'a>(&'a self, theme: &Theme) -> Element<'a, Msg> {
        let mut settings = markdown::Settings::with_text_size(TEXT_SIZE, theme);
        settings.code_size = (TEXT_SIZE * 0.875).into();
        settings.spacing = (TEXT_SIZE * 0.5).into();
        let look = crate::theme::tokens_of(theme);
        settings.style.link_color = look.link;
        settings.style.inline_code_color = look.accent_text;
        settings.style.inline_code_highlight.background = look.accent.into();
        settings.style.inline_code_highlight.border.radius = 4.0.into();

        let mut blocks = Vec::with_capacity(self.segments.len());
        let mut i = 0;
        while i < self.segments.len() {
            let start = self.segments[i].lines.start;
            if self.editor_lines.contains(&start) {
                if start == self.editor_lines.start {
                    blocks.push(self.editor_view());
                }
                i += 1;
                continue;
            }
            blocks.push(self.rendered(i, &self.segments[i], settings));
            i += 1;
        }

        scrollable(
            container(column(blocks).max_width(820).padding([24, 32]))
                .center_x(Fill)
                .padding(Padding::ZERO.bottom(200)),
        )
        .id(Id::new(SCROLL_ID))
        .height(Fill)
        .into()
    }

    // ── views ────────────────────────────────────────────────────

    fn editor_view(&self) -> Element<'_, Msg> {
        let c = self.editor.cursor();
        let last = self.editor.line_count().saturating_sub(1);
        let line_text = self
            .editor
            .line(c.position.line)
            .map(|l| l.text.into_owned())
            .unwrap_or_default();
        let no_sel = c.selection.is_none();
        let at_top = c.position.line == 0;
        let at_bottom = c.position.line == last;
        let at_start = no_sel && at_top && c.position.column == 0;
        let at_end = no_sel && at_bottom && c.position.column >= line_text.len();
        let at_line_end = c.position.column >= line_text.len();

        let editor = text_editor(&self.editor)
            .id(Id::new(EDITOR_ID))
            .on_action(Msg::Edit)
            .size(TEXT_SIZE)
            .padding(0)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings { mono: false },
                highlight::to_format,
            )
            .key_binding(move |kp: KeyPress| {
                let m = kp.modifiers;
                if m.command() {
                    match kp.key.as_ref() {
                        Key::Character("s") => return Some(Binding::Custom(Msg::Save)),
                        Key::Character("e") => return Some(Binding::Custom(Msg::ToggleMode)),
                        _ => {}
                    }
                }
                let plain = !m.shift() && !m.command() && !m.alt();
                match kp.key.as_ref() {
                    Key::Named(Named::ArrowUp) if plain && at_top => {
                        Some(Binding::Custom(Msg::Nav(Nav::Up)))
                    }
                    Key::Named(Named::ArrowDown) if plain && at_bottom => {
                        Some(Binding::Custom(Msg::Nav(Nav::Down)))
                    }
                    Key::Named(Named::ArrowLeft) if plain && at_start => {
                        Some(Binding::Custom(Msg::Nav(Nav::Left)))
                    }
                    Key::Named(Named::ArrowRight) if plain && at_end => {
                        Some(Binding::Custom(Msg::Nav(Nav::Right)))
                    }
                    Key::Named(Named::Backspace) if at_start => Some(Binding::Custom(Msg::MergeUp)),
                    Key::Named(Named::Delete) if at_end => Some(Binding::Custom(Msg::MergeDown)),
                    Key::Named(Named::Tab) if m.is_empty() => {
                        Some(Binding::Sequence(vec![Binding::Insert(' '); 4]))
                    }
                    Key::Named(Named::Enter) if plain && no_sel && at_line_end => {
                        continue_list(&line_text).or_else(|| Binding::from_key_press(kp))
                    }
                    _ => Binding::from_key_press(kp),
                }
            })
            .style(|theme, status| iced::widget::text_editor::Style {
                background: Color::TRANSPARENT.into(),
                border: Border::default(),
                ..iced::widget::text_editor::default(theme, status)
            });
        // Always the same wrapper. Swapping it on and off when a drag starts
        // rebuilds the editor widget, which drops focus — the caret and the
        // selection highlight are only drawn while it is focused, and keys
        // stop landing.
        mouse_area(editor)
            .on_move(Msg::EditorDrag)
            .on_release(Msg::DragEnd)
            .into()
    }

    fn rendered<'a>(
        &'a self,
        i: usize,
        seg: &'a Segment,
        settings: markdown::Settings,
    ) -> Element<'a, Msg> {
        let dim = |t: &Theme| text::Style {
            color: Some(t.extended_palette().background.base.text.scale_alpha(0.5)),
        };

        let body: Element<'a, Msg> = match seg.kind {
            Kind::Blank => space().height(LINE_HEIGHT).into(),
            Kind::FrontMatter => container(column(self.lines[seg.lines.clone()].iter().map(|l| {
                text(l.as_str())
                    .font(Font::MONOSPACE)
                    .size(TEXT_SIZE * 0.8)
                    .style(dim)
                    .into()
            })))
            .padding(8)
            .width(Fill)
            .style(container::rounded_box)
            .into(),
            _ if seg.items.is_empty() => {
                text(seg.source.as_str()).size(TEXT_SIZE).style(dim).into()
            }
            _ => markdown::view_with(
                &seg.items,
                settings,
                &Viewer {
                    line: seg.lines.start,
                },
            ),
        };

        let indent = seg.indent as f32 * TEXT_SIZE * 0.5;
        mouse_area(
            container(body)
                .width(Fill)
                .padding(Padding::ZERO.left(indent)),
        )
        .on_press(Msg::Activate(i))
        .on_release(Msg::DragEnd)
        .on_move(move |p| Msg::Hover(i, p))
        .interaction(mouse::Interaction::Text)
        .into()
    }
}

// ── Markdown viewer ───────────────────────────────────────────────

/// Renders Markdown with bold headings and clickable task checkboxes.
struct Viewer {
    /// Document line this segment starts at (for toggling tasks).
    line: usize,
}

impl<'a> markdown::Viewer<'a, Msg> for Viewer {
    fn on_link_click(url: markdown::Uri) -> Msg {
        Msg::Link(url)
    }

    fn heading(
        &self,
        mut settings: markdown::Settings,
        level: &'a markdown::HeadingLevel,
        text: &'a markdown::Text,
        index: usize,
    ) -> Element<'a, Msg> {
        settings.style.font = Font {
            weight: Weight::Bold,
            ..settings.style.font
        };
        markdown::heading(settings, level, text, index, Self::on_link_click)
    }

    fn unordered_list(
        &self,
        settings: markdown::Settings,
        bullets: &'a [markdown::Bullet],
    ) -> Element<'a, Msg> {
        let line = self.line;
        column(bullets.iter().map(|bullet| {
            let (marker, items): (Element<'a, Msg>, _) = match bullet {
                markdown::Bullet::Point { items } => {
                    (text("•").size(settings.text_size).into(), items)
                }
                markdown::Bullet::Task { items, done } => (
                    container(
                        checkbox(*done)
                            .size(settings.text_size)
                            .on_toggle(move |_| Msg::ToggleTask(line)),
                    )
                    .center_y(
                        iced::widget::text::LineHeight::default().to_absolute(settings.text_size),
                    )
                    .into(),
                    items,
                ),
            };
            row![marker, markdown::view_with(items, settings, self)]
                .spacing(settings.spacing)
                .into()
        }))
        .spacing(settings.spacing * 0.75)
        .padding([0.0, settings.spacing.0])
        .into()
    }
}
