//! Rendered Markdown, with the cursor's segment shown as source.

use iced::advanced::widget::Id;
use iced::font::Weight;
use iced::keyboard::{Key, Modifiers, key::Named};
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
        let settings = markdown_settings(theme);
        scrollable(
            container(
                column(self.blocks(settings))
                    .max_width(820)
                    .padding([24, 32]),
            )
            .center_x(Fill)
            .padding(Padding::ZERO.bottom(200)),
        )
        .id(Id::new(SCROLL_ID))
        .height(Fill)
        .into()
    }

    fn blocks<'a>(&'a self, settings: markdown::Settings) -> Vec<Element<'a, Msg>> {
        let mut blocks = Vec::with_capacity(self.segments.len());
        let mut i = 0;
        while i < self.segments.len() {
            let start = self.segments[i].lines.start;
            // The editor is one widget for the whole span. Later lines in that
            // span are already inside it, so they must not render again.
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
        blocks
    }

    fn editor_view(&self) -> Element<'_, Msg> {
        let caret = self.caret_edges();
        let line_text = self.editor_line_text();
        let editor = text_editor(&self.editor)
            .id(Id::new(EDITOR_ID))
            .on_action(Msg::Edit)
            .size(TEXT_SIZE)
            .padding(0)
            .highlight_with::<highlight::Highlighter>(
                highlight::Settings { mono: false },
                highlight::to_format,
            )
            .key_binding(move |key_press: KeyPress| editor_key(&caret, &line_text, key_press))
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

    fn caret_edges(&self) -> CaretEdges {
        let caret = self.editor.cursor();
        let last = self.editor.line_count().saturating_sub(1);
        let line_len = self
            .editor
            .line(caret.position.line)
            .map(|line| line.text.len())
            .unwrap_or(0);
        let no_selection = caret.selection.is_none();
        let at_top = caret.position.line == 0;
        let at_bottom = caret.position.line == last;
        CaretEdges {
            at_top,
            at_bottom,
            at_start: no_selection && at_top && caret.position.column == 0,
            at_end: no_selection && at_bottom && caret.position.column >= line_len,
            at_line_end: caret.position.column >= line_len,
            no_selection,
        }
    }

    fn editor_line_text(&self) -> String {
        let line = self.editor.cursor().position.line;
        self.editor
            .line(line)
            .map(|line| line.text.into_owned())
            .unwrap_or_default()
    }

    fn rendered<'a>(
        &'a self,
        index: usize,
        seg: &'a Segment,
        settings: markdown::Settings,
    ) -> Element<'a, Msg> {
        let body = self.segment_body(seg, settings);
        let indent = seg.indent as f32 * TEXT_SIZE * 0.5;
        mouse_area(
            container(body)
                .width(Fill)
                .padding(Padding::ZERO.left(indent)),
        )
        .on_press(Msg::Activate(index))
        .on_release(Msg::DragEnd)
        .on_move(move |point| Msg::Hover(index, point))
        .interaction(mouse::Interaction::Text)
        .into()
    }

    fn segment_body<'a>(
        &'a self,
        seg: &'a Segment,
        settings: markdown::Settings,
    ) -> Element<'a, Msg> {
        if let Some(body) = blank_body(seg) {
            return body;
        }
        if let Some(body) = self.front_matter_body(seg) {
            return body;
        }
        if seg.items.is_empty() {
            return dim_text(seg.source.as_str(), TEXT_SIZE);
        }
        markdown::view_with(
            &seg.items,
            settings,
            &Viewer {
                line: seg.lines.start,
            },
        )
    }

    fn front_matter_body<'a>(&'a self, seg: &'a Segment) -> Option<Element<'a, Msg>> {
        if seg.kind != Kind::FrontMatter {
            return None;
        }
        let lines = self.lines[seg.lines.clone()].iter().map(|line| {
            text(line.as_str())
                .font(Font::MONOSPACE)
                .size(TEXT_SIZE * 0.8)
                .style(dim_ink)
                .into()
        });
        Some(
            container(column(lines))
                .padding(8)
                .width(Fill)
                .style(container::rounded_box)
                .into(),
        )
    }
}

#[derive(Clone, Copy)]
struct CaretEdges {
    at_top: bool,
    at_bottom: bool,
    at_start: bool,
    at_end: bool,
    at_line_end: bool,
    no_selection: bool,
}

fn editor_key(caret: &CaretEdges, line: &str, key_press: KeyPress) -> Option<Binding<Msg>> {
    if let Some(binding) = command_binding(&key_press) {
        return Some(binding);
    }
    if let Some(binding) = edge_binding(caret, &key_press) {
        return Some(binding);
    }
    if let Some(binding) = tab_binding(&key_press) {
        return Some(binding);
    }
    if should_continue_list(caret, &key_press)
        && let Some(binding) = continue_list(line)
    {
        return Some(binding);
    }
    Binding::from_key_press(key_press)
}

fn command_binding(key_press: &KeyPress) -> Option<Binding<Msg>> {
    if !key_press.modifiers.command() {
        return None;
    }
    // Shift+Z is redo. It has to be tried before plain Z, or undo would steal it.
    match key_press.key.as_ref() {
        Key::Character("s") => Some(Binding::Custom(Msg::Save)),
        Key::Character("e") => Some(Binding::Custom(Msg::ToggleMode)),
        Key::Character("z" | "Z") if key_press.modifiers.shift() => {
            Some(Binding::Custom(Msg::Redo))
        }
        Key::Character("y" | "Y") if !key_press.modifiers.shift() => {
            Some(Binding::Custom(Msg::Redo))
        }
        Key::Character("z" | "Z") => Some(Binding::Custom(Msg::Undo)),
        _ => None,
    }
}

fn edge_binding(caret: &CaretEdges, key_press: &KeyPress) -> Option<Binding<Msg>> {
    let plain = modifiers_are_plain(key_press.modifiers);
    match key_press.key.as_ref() {
        Key::Named(Named::ArrowUp) if plain && caret.at_top => {
            Some(Binding::Custom(Msg::Nav(Nav::Up)))
        }
        Key::Named(Named::ArrowDown) if plain && caret.at_bottom => {
            Some(Binding::Custom(Msg::Nav(Nav::Down)))
        }
        Key::Named(Named::ArrowLeft) if plain && caret.at_start => {
            Some(Binding::Custom(Msg::Nav(Nav::Left)))
        }
        Key::Named(Named::ArrowRight) if plain && caret.at_end => {
            Some(Binding::Custom(Msg::Nav(Nav::Right)))
        }
        Key::Named(Named::Backspace) if caret.at_start => Some(Binding::Custom(Msg::MergeUp)),
        Key::Named(Named::Delete) if caret.at_end => Some(Binding::Custom(Msg::MergeDown)),
        _ => None,
    }
}

fn modifiers_are_plain(modifiers: Modifiers) -> bool {
    !modifiers.shift() && !modifiers.command() && !modifiers.alt()
}

fn tab_binding(key_press: &KeyPress) -> Option<Binding<Msg>> {
    if !matches!(key_press.key, Key::Named(Named::Tab)) || !key_press.modifiers.is_empty() {
        return None;
    }
    Some(Binding::Sequence(vec![Binding::Insert(' '); 4]))
}

fn should_continue_list(caret: &CaretEdges, key_press: &KeyPress) -> bool {
    modifiers_are_plain(key_press.modifiers)
        && caret.no_selection
        && caret.at_line_end
        && matches!(key_press.key, Key::Named(Named::Enter))
}

fn markdown_settings(theme: &Theme) -> markdown::Settings {
    let mut settings = markdown::Settings::with_text_size(TEXT_SIZE, theme);
    settings.code_size = (TEXT_SIZE * 0.875).into();
    settings.spacing = (TEXT_SIZE * 0.5).into();
    let look = crate::theme::tokens_of(theme);
    settings.style.link_color = look.link;
    // Stock markdown paints inline code white on black. Keep it on the theme.
    settings.style.inline_code_color = look.accent_text;
    settings.style.inline_code_highlight.background = look.accent.into();
    settings.style.inline_code_highlight.border.radius = 4.0.into();
    settings
}

fn blank_body(seg: &Segment) -> Option<Element<'static, Msg>> {
    if seg.kind != Kind::Blank {
        return None;
    }
    Some(space().height(LINE_HEIGHT).into())
}

fn dim_text(line: &str, size: f32) -> Element<'_, Msg> {
    text(line).size(size).style(dim_ink).into()
}

fn dim_ink(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(
            theme
                .extended_palette()
                .background
                .base
                .text
                .scale_alpha(0.5),
        ),
    }
}

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
                markdown::Bullet::Point { items } => (point_marker(settings), items),
                markdown::Bullet::Task { items, done } => {
                    (task_marker(line, settings, *done), items)
                }
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

fn point_marker(settings: markdown::Settings) -> Element<'static, Msg> {
    text("•").size(settings.text_size).into()
}

fn task_marker(line: usize, settings: markdown::Settings, done: bool) -> Element<'static, Msg> {
    container(
        checkbox(done)
            .size(settings.text_size)
            .on_toggle(move |_| Msg::ToggleTask(line)),
    )
    .center_y(iced::widget::text::LineHeight::default().to_absolute(settings.text_size))
    .into()
}
