//! The composer's text field: a GPUI input with selection, IME composition,
//! and clipboard, after GPUI's own `examples/input.rs`. It is one line
//! ([`TextInput::new`]) or, for a message, several
//! ([`TextInput::multiline`]): Shift+Enter starts a new line, long lines
//! wrap, and the field grows to [`MAX_ROWS`] rows, then scrolls.

use std::ops::Range;

use gpui::{
    App, Bounds, ClipboardItem, Context, CursorStyle, ElementId, ElementInputHandler, Entity,
    EntityInputHandler, EventEmitter, FocusHandle, Focusable, GlobalElementId, KeyBinding,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    ScrollWheelEvent, ShapedLine, SharedString, Style, TextRun, UTF16Selection, UnderlineStyle,
    Window, WrappedLine, actions, div, fill, point, prelude::*, px, relative, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::theme;

actions!(
    composer,
    [
        Backspace,
        Delete,
        Left,
        Right,
        SelectLeft,
        SelectRight,
        SelectAll,
        Home,
        End,
        Paste,
        Cut,
        Copy,
        Submit,
        Cancel,
        Newline,
        Up,
        Down,
        SelectUp,
        SelectDown,
    ]
);

/// The most rows a multi-line field shows before it scrolls.
pub const MAX_ROWS: usize = 8;
/// The field's line height.
const LINE_HEIGHT: f32 = 24.;

pub fn bind_keys(cx: &mut App) {
    let context = Some("Composer");
    cx.bind_keys([
        KeyBinding::new("backspace", Backspace, context),
        KeyBinding::new("delete", Delete, context),
        KeyBinding::new("left", Left, context),
        KeyBinding::new("right", Right, context),
        KeyBinding::new("shift-left", SelectLeft, context),
        KeyBinding::new("shift-right", SelectRight, context),
        KeyBinding::new("home", Home, context),
        KeyBinding::new("end", End, context),
        KeyBinding::new("enter", Submit, context),
        KeyBinding::new("shift-enter", Newline, context),
        KeyBinding::new("alt-enter", Newline, context),
        KeyBinding::new("up", Up, context),
        KeyBinding::new("down", Down, context),
        KeyBinding::new("shift-up", SelectUp, context),
        KeyBinding::new("shift-down", SelectDown, context),
        KeyBinding::new("escape", Cancel, context),
        KeyBinding::new("cmd-a", SelectAll, context),
        KeyBinding::new("cmd-v", Paste, context),
        KeyBinding::new("cmd-c", Copy, context),
        KeyBinding::new("cmd-x", Cut, context),
        KeyBinding::new("ctrl-a", SelectAll, context),
        KeyBinding::new("ctrl-v", Paste, context),
        KeyBinding::new("ctrl-c", Copy, context),
        KeyBinding::new("ctrl-x", Cut, context),
    ]);
}

/// Emitted when the person presses Enter with text in the field.
pub struct Submitted(pub String);

/// Emitted when the person presses Escape in the field.
pub struct Cancelled;

pub struct TextInput {
    focus_handle: FocusHandle,
    content: SharedString,
    placeholder: SharedString,
    selected_range: Range<usize>,
    selection_reversed: bool,
    marked_range: Option<Range<usize>>,
    last_layout: Option<ShapedLine>,
    last_bounds: Option<Bounds<Pixels>>,
    is_selecting: bool,
    /// Several lines, wrapped ([`TextInput::multiline`]).
    multiline: bool,
    /// A multi-line field's paragraphs as last drawn: each one's first byte,
    /// its wrapped layout, and its top within the text.
    last_rows: Vec<(usize, WrappedLine, Pixels)>,
    /// How far a multi-line field has scrolled down.
    scroll_y: Pixels,
}

impl EventEmitter<Submitted> for TextInput {}
impl EventEmitter<Cancelled> for TextInput {}

impl TextInput {
    pub fn new(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            content: SharedString::default(),
            placeholder: placeholder.into(),
            selected_range: 0..0,
            selection_reversed: false,
            marked_range: None,
            last_layout: None,
            last_bounds: None,
            is_selecting: false,
            multiline: false,
            last_rows: Vec::new(),
            scroll_y: px(0.),
        }
    }

    /// A field for a message: Shift+Enter starts a new line, long lines
    /// wrap, and it grows to [`MAX_ROWS`] rows.
    pub fn multiline(placeholder: impl Into<SharedString>, cx: &mut Context<Self>) -> Self {
        Self {
            multiline: true,
            ..Self::new(placeholder, cx)
        }
    }

    /// The field's text.
    pub fn text(&self) -> String {
        self.content.to_string()
    }

    pub fn is_empty(&self) -> bool {
        self.content.trim().is_empty()
    }

    pub fn set_placeholder(&mut self, placeholder: impl Into<SharedString>) {
        self.placeholder = placeholder.into();
    }

    /// Takes the text out of the field, leaving it empty.
    pub fn take(&mut self, cx: &mut Context<Self>) -> String {
        let text = self.content.to_string();
        self.content = SharedString::default();
        self.selected_range = 0..0;
        self.selection_reversed = false;
        self.marked_range = None;
        self.scroll_y = px(0.);
        cx.notify();
        text
    }

    /// Replaces the field's text, with the caret at its end and all of it
    /// selected.
    pub fn set_text(&mut self, text: impl Into<SharedString>, cx: &mut Context<Self>) {
        self.content = text.into();
        self.selected_range = 0..self.content.len();
        self.selection_reversed = false;
        self.marked_range = None;
        cx.notify();
    }

    fn cancel(&mut self, _: &Cancel, _: &mut Window, cx: &mut Context<Self>) {
        cx.emit(Cancelled);
    }

    fn submit(&mut self, _: &Submit, _: &mut Window, cx: &mut Context<Self>) {
        if self.marked_range.is_some() || self.is_empty() {
            return;
        }
        let text = self.take(cx);
        cx.emit(Submitted(text));
    }

    fn newline(&mut self, _: &Newline, window: &mut Window, cx: &mut Context<Self>) {
        if !self.multiline {
            return self.submit(&Submit, window, cx);
        }
        if self.marked_range.is_some() {
            return;
        }
        self.insert(None, "\n", cx);
    }

    fn up(&mut self, _: &Up, _: &mut Window, cx: &mut Context<Self>) {
        match self.vertical(self.cursor_offset(), -1) {
            Some(offset) => self.move_to(offset, cx),
            None => self.move_to(0, cx),
        }
    }

    fn down(&mut self, _: &Down, _: &mut Window, cx: &mut Context<Self>) {
        match self.vertical(self.cursor_offset(), 1) {
            Some(offset) => self.move_to(offset, cx),
            None => self.move_to(self.content.len(), cx),
        }
    }

    fn select_up(&mut self, _: &SelectUp, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.vertical(self.cursor_offset(), -1).unwrap_or(0);
        self.select_to(offset, cx);
    }

    fn select_down(&mut self, _: &SelectDown, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self
            .vertical(self.cursor_offset(), 1)
            .unwrap_or(self.content.len());
        self.select_to(offset, cx);
    }

    /// The offset one row above (`-1`) or below (`1`) `offset`, at the same
    /// x, or `None` past the first or last row.
    fn vertical(&self, offset: usize, rows: i32) -> Option<usize> {
        if !self.multiline {
            return None;
        }
        let at = self.position_of(offset)?;
        let line_height = px(LINE_HEIGHT);
        let y = at.y + line_height * rows as f32 + line_height / 2.;
        let bottom = self
            .last_rows
            .last()
            .map(|(_, line, top)| *top + line.size(line_height).height)?;
        if y < px(0.) || y >= bottom {
            return None;
        }
        Some(self.offset_at(point(at.x, y)))
    }

    /// Where `offset` is drawn in a multi-line field, from the top left of
    /// its text (before scrolling).
    fn position_of(&self, offset: usize) -> Option<Point<Pixels>> {
        let line_height = px(LINE_HEIGHT);
        let (start, line, top) = self
            .last_rows
            .iter()
            .rev()
            .find(|(start, _, _)| *start <= offset)?;
        let at = line.position_for_index(offset - start, line_height)?;
        Some(point(at.x, at.y + *top))
    }

    /// The offset nearest `position` in a multi-line field, from the top
    /// left of its text (before scrolling).
    fn offset_at(&self, position: Point<Pixels>) -> usize {
        let line_height = px(LINE_HEIGHT);
        let Some((start, line, top)) = self
            .last_rows
            .iter()
            .rev()
            .find(|(_, _, top)| *top <= position.y)
            .or_else(|| self.last_rows.first())
        else {
            return 0;
        };
        let local = point(position.x.max(px(0.)), (position.y - *top).max(px(0.)));
        let index = match line.closest_index_for_position(local, line_height) {
            Ok(index) | Err(index) => index,
        };
        (start + index).min(self.content.len())
    }

    /// Puts `text` in place of `range` (UTF-8), or of the marked text or the
    /// selection.
    fn insert(&mut self, range: Option<Range<usize>>, text: &str, cx: &mut Context<Self>) {
        let range = range
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + text + &self.content[range.end..]).into();
        self.selected_range = range.start + text.len()..range.start + text.len();
        self.marked_range.take();
        cx.notify();
    }

    fn left(&mut self, _: &Left, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.previous_boundary(self.cursor_offset()), cx);
        } else {
            self.move_to(self.selected_range.start, cx)
        }
    }

    fn right(&mut self, _: &Right, _: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            self.move_to(self.next_boundary(self.selected_range.end), cx);
        } else {
            self.move_to(self.selected_range.end, cx)
        }
    }

    fn select_left(&mut self, _: &SelectLeft, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.previous_boundary(self.cursor_offset()), cx);
    }

    fn select_right(&mut self, _: &SelectRight, _: &mut Window, cx: &mut Context<Self>) {
        self.select_to(self.next_boundary(self.cursor_offset()), cx);
    }

    fn select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.move_to(0, cx);
        self.select_to(self.content.len(), cx)
    }

    fn home(&mut self, _: &Home, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let start = match self.multiline.then(|| self.position_of(offset)).flatten() {
            Some(at) => self.offset_at(point(px(0.), at.y + px(LINE_HEIGHT) / 2.)),
            None => 0,
        };
        self.move_to(start, cx);
    }

    fn end(&mut self, _: &End, _: &mut Window, cx: &mut Context<Self>) {
        let offset = self.cursor_offset();
        let end = match self.multiline.then(|| self.position_of(offset)).flatten() {
            Some(at) => self.offset_at(point(px(100_000.), at.y + px(LINE_HEIGHT) / 2.)),
            None => self.content.len(),
        };
        self.move_to(end, cx);
    }

    fn backspace(&mut self, _: &Backspace, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let prev = self.previous_boundary(self.cursor_offset());
            if self.cursor_offset() == prev {
                return;
            }
            self.select_to(prev, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn delete(&mut self, _: &Delete, window: &mut Window, cx: &mut Context<Self>) {
        if self.selected_range.is_empty() {
            let next = self.next_boundary(self.cursor_offset());
            if self.cursor_offset() == next {
                return;
            }
            self.select_to(next, cx)
        }
        self.replace_text_in_range(None, "", window, cx)
    }

    fn on_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        window.focus(&self.focus_handle, cx);
        self.is_selecting = true;
        if event.modifiers.shift {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        } else {
            self.move_to(self.index_for_mouse_position(event.position), cx)
        }
    }

    fn on_mouse_up(&mut self, _: &MouseUpEvent, _window: &mut Window, _: &mut Context<Self>) {
        self.is_selecting = false;
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _: &mut Window, cx: &mut Context<Self>) {
        if self.is_selecting {
            self.select_to(self.index_for_mouse_position(event.position), cx);
        }
    }

    fn paste(&mut self, _: &Paste, window: &mut Window, cx: &mut Context<Self>) {
        match cx.read_from_clipboard().and_then(|item| item.text()) {
            Some(text) => self.replace_text_in_range(None, &text, window, cx),
            // A browser hands over pasted text only in its own `paste` event,
            // which arrives through the input handler; let the keystroke
            // through so that event fires.
            None => cx.propagate(),
        }
    }

    fn copy(&mut self, _: &Copy, _: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
        } else if !crate::selection::copy(cx) {
            // Nothing selected here or in the transcript: the page's own
            // copy goes on (#12305).
            cx.propagate();
        }
    }

    fn cut(&mut self, _: &Cut, window: &mut Window, cx: &mut Context<Self>) {
        if !self.selected_range.is_empty() {
            cx.write_to_clipboard(ClipboardItem::new_string(
                self.content[self.selected_range.clone()].to_string(),
            ));
            self.replace_text_in_range(None, "", window, cx)
        }
    }

    fn move_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        self.selected_range = offset..offset;
        cx.notify()
    }

    fn cursor_offset(&self) -> usize {
        if self.selection_reversed {
            self.selected_range.start
        } else {
            self.selected_range.end
        }
    }

    fn index_for_mouse_position(&self, position: Point<Pixels>) -> usize {
        if self.content.is_empty() {
            return 0;
        }
        if self.multiline {
            let Some(bounds) = self.last_bounds.as_ref() else {
                return 0;
            };
            return self.offset_at(point(
                position.x - bounds.left(),
                position.y - bounds.top() + self.scroll_y,
            ));
        }
        let (Some(bounds), Some(line)) = (self.last_bounds.as_ref(), self.last_layout.as_ref())
        else {
            return 0;
        };
        if position.y < bounds.top() {
            return 0;
        }
        if position.y > bounds.bottom() {
            return self.content.len();
        }
        line.closest_index_for_x(position.x - bounds.left())
    }

    fn select_to(&mut self, offset: usize, cx: &mut Context<Self>) {
        if self.selection_reversed {
            self.selected_range.start = offset
        } else {
            self.selected_range.end = offset
        };
        if self.selected_range.end < self.selected_range.start {
            self.selection_reversed = !self.selection_reversed;
            self.selected_range = self.selected_range.end..self.selected_range.start;
        }
        cx.notify()
    }

    fn offset_from_utf16(&self, offset: usize) -> usize {
        let mut utf8_offset = 0;
        let mut utf16_count = 0;
        for ch in self.content.chars() {
            if utf16_count >= offset {
                break;
            }
            utf16_count += ch.len_utf16();
            utf8_offset += ch.len_utf8();
        }
        utf8_offset
    }

    fn offset_to_utf16(&self, offset: usize) -> usize {
        let mut utf16_offset = 0;
        let mut utf8_count = 0;
        for ch in self.content.chars() {
            if utf8_count >= offset {
                break;
            }
            utf8_count += ch.len_utf8();
            utf16_offset += ch.len_utf16();
        }
        utf16_offset
    }

    fn range_to_utf16(&self, range: &Range<usize>) -> Range<usize> {
        self.offset_to_utf16(range.start)..self.offset_to_utf16(range.end)
    }

    fn range_from_utf16(&self, range_utf16: &Range<usize>) -> Range<usize> {
        self.offset_from_utf16(range_utf16.start)..self.offset_from_utf16(range_utf16.end)
    }

    fn previous_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .rev()
            .find_map(|(idx, _)| (idx < offset).then_some(idx))
            .unwrap_or(0)
    }

    fn next_boundary(&self, offset: usize) -> usize {
        self.content
            .grapheme_indices(true)
            .find_map(|(idx, _)| (idx > offset).then_some(idx))
            .unwrap_or(self.content.len())
    }
}

impl EntityInputHandler for TextInput {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let range = self.range_from_utf16(&range_utf16);
        actual_range.replace(self.range_to_utf16(&range));
        Some(self.content[range].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        Some(UTF16Selection {
            range: self.range_to_utf16(&self.selected_range),
            reversed: self.selection_reversed,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_range
            .as_ref()
            .map(|range| self.range_to_utf16(range))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Enter arrives as text on some platforms; the Submit binding owns it.
        if new_text == "\n" || new_text == "\r" {
            return;
        }
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16));
        // Pasted text keeps its line breaks in a multi-line field and
        // becomes one line in a single-line one.
        let text = new_text.replace("\r\n", "\n").replace('\r', "\n");
        let text = if self.multiline {
            text
        } else {
            text.replace('\n', " ")
        };
        self.insert(range, &text, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range = range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .or(self.marked_range.clone())
            .unwrap_or(self.selected_range.clone());
        self.content =
            (self.content[0..range.start].to_owned() + new_text + &self.content[range.end..])
                .into();
        self.marked_range =
            (!new_text.is_empty()).then(|| range.start..range.start + new_text.len());
        self.selected_range = new_selected_range_utf16
            .as_ref()
            .map(|range_utf16| self.range_from_utf16(range_utf16))
            .map(|new_range| new_range.start + range.start..new_range.end + range.start)
            .unwrap_or_else(|| range.start + new_text.len()..range.start + new_text.len());
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let range = self.range_from_utf16(&range_utf16);
        if self.multiline {
            let start = self.position_of(range.start)?;
            let end = self.position_of(range.end).unwrap_or(start);
            let top = bounds.top() - self.scroll_y;
            return Some(Bounds::from_corners(
                point(bounds.left() + start.x, top + start.y),
                point(
                    bounds.left() + end.x.max(start.x),
                    top + end.y + px(LINE_HEIGHT),
                ),
            ));
        }
        let last_layout = self.last_layout.as_ref()?;
        Some(Bounds::from_corners(
            point(
                bounds.left() + last_layout.x_for_index(range.start),
                bounds.top(),
            ),
            point(
                bounds.left() + last_layout.x_for_index(range.end),
                bounds.bottom(),
            ),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        if self.multiline {
            let bounds = self.last_bounds?;
            let local = bounds.localize(&point)?;
            let offset = self.offset_at(gpui::point(local.x, local.y + self.scroll_y));
            return Some(self.offset_to_utf16(offset));
        }
        let line_point = self.last_bounds?.localize(&point)?;
        let last_layout = self.last_layout.as_ref()?;
        let utf8_index = last_layout.index_for_x(point.x - line_point.x)?;
        Some(self.offset_to_utf16(utf8_index))
    }
}

struct TextElement {
    input: Entity<TextInput>,
}

struct PrepaintState {
    line: Option<ShapedLine>,
    cursor: Option<PaintQuad>,
    selection: Option<PaintQuad>,
}

impl IntoElement for TextElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for TextElement {
    type RequestLayoutState = ();
    type PrepaintState = PrepaintState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        style.size.height = window.line_height().into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let input = self.input.read(cx);
        let content = input.content.clone();
        let selected_range = input.selected_range.clone();
        let cursor = input.cursor_offset();
        let style = window.text_style();
        let (display_text, text_color) = if content.is_empty() {
            (input.placeholder.clone(), theme::text_faint())
        } else {
            (content, style.color)
        };
        let run = TextRun {
            len: display_text.len(),
            font: style.font(),
            color: text_color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = if let Some(marked_range) = input.marked_range.as_ref() {
            vec![
                TextRun {
                    len: marked_range.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked_range.end - marked_range.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: display_text.len() - marked_range.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect()
        } else {
            vec![run]
        };
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line = window
            .text_system()
            .shape_line(display_text, font_size, &runs, None);
        let cursor_pos = if input.content.is_empty() {
            px(0.)
        } else {
            line.x_for_index(cursor)
        };
        let (selection, cursor) = if selected_range.is_empty() {
            (
                None,
                Some(fill(
                    Bounds::new(
                        point(bounds.left() + cursor_pos, bounds.top() + px(2.)),
                        size(px(1.5), bounds.bottom() - bounds.top() - px(4.)),
                    ),
                    theme::accent(),
                )),
            )
        } else {
            (
                Some(fill(
                    Bounds::from_corners(
                        point(
                            bounds.left() + line.x_for_index(selected_range.start),
                            bounds.top(),
                        ),
                        point(
                            bounds.left() + line.x_for_index(selected_range.end),
                            bounds.bottom(),
                        ),
                    ),
                    theme::selection(),
                )),
                None,
            )
        };
        PrepaintState {
            line: Some(line),
            cursor,
            selection,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        if let Some(selection) = prepaint.selection.take() {
            window.paint_quad(selection)
        }
        let Some(line) = prepaint.line.take() else {
            return;
        };
        line.paint(
            bounds.origin,
            window.line_height(),
            gpui::TextAlign::Left,
            None,
            window,
            cx,
        )
        .ok();
        if focus_handle.is_focused(window)
            && let Some(cursor) = prepaint.cursor.take()
        {
            window.paint_quad(cursor);
        }
        self.input.update(cx, |input, _cx| {
            input.last_layout = Some(line);
            input.last_bounds = Some(bounds);
        });
    }
}

impl Render for TextInput {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_1()
            .min_w_0()
            .key_context("Composer")
            .track_focus(&self.focus_handle(cx))
            .cursor(CursorStyle::IBeam)
            .on_action(cx.listener(Self::submit))
            .on_action(cx.listener(Self::cancel))
            .on_action(cx.listener(Self::backspace))
            .on_action(cx.listener(Self::delete))
            .on_action(cx.listener(Self::left))
            .on_action(cx.listener(Self::right))
            .on_action(cx.listener(Self::select_left))
            .on_action(cx.listener(Self::select_right))
            .on_action(cx.listener(Self::select_all))
            .on_action(cx.listener(Self::home))
            .on_action(cx.listener(Self::end))
            .on_action(cx.listener(Self::paste))
            .on_action(cx.listener(Self::cut))
            .on_action(cx.listener(Self::copy))
            .on_action(cx.listener(Self::newline))
            .on_action(cx.listener(Self::up))
            .on_action(cx.listener(Self::down))
            .on_action(cx.listener(Self::select_up))
            .on_action(cx.listener(Self::select_down))
            .when(self.multiline, |field| {
                field.on_scroll_wheel(cx.listener(Self::on_scroll))
            })
            .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_up_out(MouseButton::Left, cx.listener(Self::on_mouse_up))
            .on_mouse_move(cx.listener(Self::on_mouse_move))
            .text_size(px(15.))
            .line_height(px(LINE_HEIGHT))
            .text_color(theme::text())
            .overflow_hidden()
            .map(|field| {
                if self.multiline {
                    field.child(AreaElement { input: cx.entity() })
                } else {
                    field.child(TextElement { input: cx.entity() })
                }
            })
    }
}

impl TextInput {
    fn on_scroll(&mut self, event: &ScrollWheelEvent, _: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(px(LINE_HEIGHT)).y;
        let total = self.text_height();
        let visible = self.visible_height();
        let next = (self.scroll_y - delta)
            .max(px(0.))
            .min((total - visible).max(px(0.)));
        if next != self.scroll_y {
            self.scroll_y = next;
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// The height of a multi-line field's whole text, as last drawn.
    fn text_height(&self) -> Pixels {
        self.last_rows
            .last()
            .map(|(_, line, top)| *top + line.size(px(LINE_HEIGHT)).height)
            .unwrap_or(px(LINE_HEIGHT))
    }

    /// The height a multi-line field shows: its text, up to [`MAX_ROWS`].
    fn visible_height(&self) -> Pixels {
        self.text_height()
            .min(px(LINE_HEIGHT) * MAX_ROWS as f32)
            .max(px(LINE_HEIGHT))
    }
}

/// Shapes a multi-line field's text (or its placeholder) to `width`: each
/// paragraph's first byte, its wrapped layout, and its top.
fn shape_rows(
    text: &SharedString,
    runs: &[TextRun],
    font_size: Pixels,
    width: Pixels,
    window: &Window,
) -> Vec<(usize, WrappedLine, Pixels)> {
    let line_height = px(LINE_HEIGHT);
    let lines = window
        .text_system()
        .shape_text(text.clone(), font_size, runs, Some(width), None)
        .map(|lines| lines.into_vec())
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(lines.len());
    let (mut start, mut top) = (0, px(0.));
    for line in lines {
        let height = line.size(line_height).height.max(line_height);
        let len = line.len();
        rows.push((start, line, top));
        start += len + 1;
        top += height;
    }
    rows
}

/// A multi-line field's text: wrapped paragraphs, the caret, and the
/// selection, scrolled to keep the caret in view.
struct AreaElement {
    input: Entity<TextInput>,
}

struct AreaState {
    rows: Vec<(usize, WrappedLine, Pixels)>,
    scroll_y: Pixels,
    cursor: Option<PaintQuad>,
    selection: Vec<PaintQuad>,
    placeholder: bool,
}

impl IntoElement for AreaElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl AreaElement {
    fn runs(input: &TextInput, window: &Window) -> (SharedString, Vec<TextRun>, bool) {
        let style = window.text_style();
        let placeholder = input.content.is_empty();
        let (text, color) = if placeholder {
            (input.placeholder.clone(), theme::text_faint())
        } else {
            (input.content.clone(), style.color)
        };
        let run = TextRun {
            len: text.len(),
            font: style.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let runs = match input.marked_range.as_ref().filter(|_| !placeholder) {
            Some(marked) => [
                TextRun {
                    len: marked.start,
                    ..run.clone()
                },
                TextRun {
                    len: marked.end - marked.start,
                    underline: Some(UnderlineStyle {
                        color: Some(run.color),
                        thickness: px(1.0),
                        wavy: false,
                    }),
                    ..run.clone()
                },
                TextRun {
                    len: text.len() - marked.end,
                    ..run
                },
            ]
            .into_iter()
            .filter(|run| run.len > 0)
            .collect(),
            None => vec![run],
        };
        (text, runs, placeholder)
    }
}

impl Element for AreaElement {
    type RequestLayoutState = ();
    type PrepaintState = AreaState;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.).into();
        let input = self.input.clone();
        let layout = window.request_measured_layout(style, move |known, available, window, cx| {
            let width = known.width.unwrap_or(match available.width {
                gpui::AvailableSpace::Definite(width) => width,
                _ => px(600.),
            });
            let input = input.read(cx);
            let (text, runs, _) = AreaElement::runs(input, window);
            let font_size = window.text_style().font_size.to_pixels(window.rem_size());
            let rows = shape_rows(&text, &runs, font_size, width, window);
            let total = rows
                .last()
                .map(|(_, line, top)| *top + line.size(px(LINE_HEIGHT)).height)
                .unwrap_or(px(LINE_HEIGHT));
            let height = total
                .min(px(LINE_HEIGHT) * MAX_ROWS as f32)
                .max(px(LINE_HEIGHT));
            size(width, height)
        });
        (layout, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let line_height = px(LINE_HEIGHT);
        let input = self.input.read(cx);
        let (text, runs, placeholder) = AreaElement::runs(input, window);
        let font_size = window.text_style().font_size.to_pixels(window.rem_size());
        let rows = shape_rows(&text, &runs, font_size, bounds.size.width, window);
        let position = |offset: usize| -> Point<Pixels> {
            rows.iter()
                .rev()
                .find(|(start, _, _)| *start <= offset)
                .and_then(|(start, line, top)| {
                    line.position_for_index(offset - start, line_height)
                        .map(|at| point(at.x, at.y + *top))
                })
                .unwrap_or_default()
        };
        let total = rows
            .last()
            .map(|(_, line, top)| *top + line.size(line_height).height)
            .unwrap_or(line_height);
        let visible = bounds.size.height;
        // Keep the caret in view.
        let caret = if placeholder {
            point(px(0.), px(0.))
        } else {
            position(input.cursor_offset())
        };
        let mut scroll_y = input.scroll_y.min((total - visible).max(px(0.)));
        if caret.y < scroll_y {
            scroll_y = caret.y;
        } else if caret.y + line_height > scroll_y + visible {
            scroll_y = caret.y + line_height - visible;
        }
        let origin = point(bounds.left(), bounds.top() - scroll_y);
        let selected = input.selected_range.clone();
        let (cursor, selection) = if selected.is_empty() || placeholder {
            (
                Some(fill(
                    Bounds::new(
                        point(origin.x + caret.x, origin.y + caret.y + px(2.)),
                        size(px(1.5), line_height - px(4.)),
                    ),
                    theme::accent(),
                )),
                Vec::new(),
            )
        } else {
            let (from, to) = (position(selected.start), position(selected.end));
            let right = bounds.size.width;
            let mut quads = Vec::new();
            let mut row_y = from.y;
            while row_y <= to.y {
                let left = if row_y == from.y { from.x } else { px(0.) };
                let end = if row_y == to.y { to.x } else { right };
                if end > left {
                    quads.push(fill(
                        Bounds::from_corners(
                            point(origin.x + left, origin.y + row_y),
                            point(origin.x + end, origin.y + row_y + line_height),
                        ),
                        theme::selection(),
                    ));
                }
                row_y += line_height;
            }
            (None, quads)
        };
        AreaState {
            rows,
            scroll_y,
            cursor,
            selection,
            placeholder,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let line_height = px(LINE_HEIGHT);
        let focus_handle = self.input.read(cx).focus_handle.clone();
        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.input.clone()),
            cx,
        );
        let scroll_y = prepaint.scroll_y;
        let rows = std::mem::take(&mut prepaint.rows);
        let selection = std::mem::take(&mut prepaint.selection);
        let cursor = prepaint.cursor.take();
        window.with_content_mask(Some(gpui::ContentMask { bounds }), |window| {
            for quad in selection {
                window.paint_quad(quad);
            }
            for (_, line, top) in &rows {
                let origin = point(bounds.left(), bounds.top() + *top - scroll_y);
                line.paint(origin, line_height, gpui::TextAlign::Left, None, window, cx)
                    .ok();
            }
            if focus_handle.is_focused(window)
                && let Some(cursor) = cursor
            {
                window.paint_quad(cursor);
            }
        });
        let placeholder = prepaint.placeholder;
        self.input.update(cx, |input, _cx| {
            // The placeholder's layout says nothing about the text's.
            input.last_rows = if placeholder { Vec::new() } else { rows };
            input.last_bounds = Some(bounds);
            input.scroll_y = scroll_y;
        });
    }
}

impl Focusable for TextInput {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}
