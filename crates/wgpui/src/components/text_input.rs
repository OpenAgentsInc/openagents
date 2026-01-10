use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::input::{Key, NamedKey};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};
use web_time::Instant;

pub struct TextInput {
    id: Option<ComponentId>,
    value: String,
    placeholder: String,
    cursor_pos: usize,
    selection_start: Option<usize>,
    focused: bool,
    hovered: bool,
    font_size: f32,
    padding: (f32, f32),
    background: Hsla,
    border_color: Hsla,
    border_color_focused: Hsla,
    text_color: Hsla,
    placeholder_color: Hsla,
    cursor_color: Hsla,
    cursor_blink_start: Instant,
    mono: bool,
    on_change: Option<Box<dyn FnMut(&str)>>,
    on_submit: Option<Box<dyn FnMut(&str)>>,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            id: None,
            value: String::new(),
            placeholder: String::new(),
            cursor_pos: 0,
            selection_start: None,
            focused: false,
            hovered: false,
            font_size: theme::font_size::SM,
            padding: (theme::spacing::SM, theme::spacing::XS),
            background: theme::bg::SURFACE,
            border_color: theme::border::DEFAULT,
            border_color_focused: theme::accent::PRIMARY,
            text_color: theme::text::PRIMARY,
            placeholder_color: theme::text::MUTED,
            cursor_color: theme::text::PRIMARY,
            cursor_blink_start: Instant::now(),
            mono: false,
            on_change: None,
            on_submit: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_pos = self.value.len();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn padding(mut self, horizontal: f32, vertical: f32) -> Self {
        self.padding = (horizontal, vertical);
        self
    }

    pub fn background(mut self, color: Hsla) -> Self {
        self.background = color;
        self
    }

    pub fn border_color(mut self, color: Hsla) -> Self {
        self.border_color = color;
        self
    }

    pub fn border_color_focused(mut self, color: Hsla) -> Self {
        self.border_color_focused = color;
        self
    }

    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    pub fn placeholder_color(mut self, color: Hsla) -> Self {
        self.placeholder_color = color;
        self
    }

    pub fn cursor_color(mut self, color: Hsla) -> Self {
        self.cursor_color = color;
        self
    }

    pub fn mono(mut self, mono: bool) -> Self {
        self.mono = mono;
        self
    }

    pub fn set_mono(&mut self, mono: bool) {
        self.mono = mono;
    }

    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn on_submit<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_submit = Some(Box::new(f));
        self
    }

    pub fn get_value(&self) -> &str {
        &self.value
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor_pos = self.cursor_pos.min(self.value.len());
    }

    pub fn insert_text(&mut self, text: &str) {
        self.insert_str(text);
    }

    pub fn is_focused(&self) -> bool {
        self.focused
    }

    pub fn focus(&mut self) {
        self.focused = true;
        self.cursor_blink_start = Instant::now();
    }

    /// Reset the cursor blink timer (call on user interaction)
    fn reset_cursor_blink(&mut self) {
        self.cursor_blink_start = Instant::now();
    }

    pub fn blur(&mut self) {
        self.focused = false;
        self.selection_start = None;
    }

    pub fn set_focused(&mut self, focused: bool) {
        if focused {
            self.focus();
        } else {
            self.blur();
        }
    }

    /// Calculate the current height based on line count
    pub fn current_height(&self) -> f32 {
        let line_height = self.font_size * 1.4;
        let line_count = self.value.lines().count().max(1);
        // Handle trailing newline (lines() doesn't count it as an extra line)
        let line_count = if self.value.ends_with('\n') {
            line_count + 1
        } else {
            line_count
        };
        line_height * line_count as f32 + self.padding.1 * 2.0
    }

    fn insert_str(&mut self, s: &str) {
        if self.cursor_pos <= self.value.len() {
            self.value.insert_str(self.cursor_pos, s);
            self.cursor_pos += s.len();
            self.notify_change();
        }
    }

    fn insert_char(&mut self, c: char) {
        if self.cursor_pos <= self.value.len() {
            self.value.insert(self.cursor_pos, c);
            self.cursor_pos += c.len_utf8();
            self.notify_change();
        }
    }

    fn delete_backward(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
            self.value.remove(self.cursor_pos);
            self.notify_change();
        }
    }

    fn delete_forward(&mut self) {
        if self.cursor_pos < self.value.len() {
            self.value.remove(self.cursor_pos);
            self.notify_change();
        }
    }

    /// Delete selected text (if any selection exists)
    fn delete_selection(&mut self) -> bool {
        if let Some(start) = self.selection_start {
            let (from, to) = if start < self.cursor_pos {
                (start, self.cursor_pos)
            } else {
                (self.cursor_pos, start)
            };
            if from != to {
                self.value.replace_range(from..to, "");
                self.cursor_pos = from;
                self.selection_start = None;
                self.notify_change();
                return true;
            }
        }
        self.selection_start = None;
        false
    }

    /// Get the selection range as (start, end) where start < end
    fn get_selection(&self) -> Option<(usize, usize)> {
        self.selection_start.map(|start| {
            if start < self.cursor_pos {
                (start, self.cursor_pos)
            } else {
                (self.cursor_pos, start)
            }
        }).filter(|(start, end)| start != end)
    }

    /// Select all text
    fn select_all(&mut self) {
        if !self.value.is_empty() {
            self.selection_start = Some(0);
            self.cursor_pos = self.value.len();
        }
    }

    fn move_cursor_left(&mut self) {
        if self.cursor_pos > 0 {
            self.cursor_pos -= 1;
        }
    }

    fn move_cursor_right(&mut self) {
        if self.cursor_pos < self.value.len() {
            self.cursor_pos += 1;
        }
    }

    fn move_cursor_to_start(&mut self) {
        self.cursor_pos = 0;
    }

    fn move_cursor_to_end(&mut self) {
        self.cursor_pos = self.value.len();
    }

    fn notify_change(&mut self) {
        if let Some(on_change) = &mut self.on_change {
            on_change(&self.value);
        }
    }

    fn notify_submit(&mut self) {
        if let Some(on_submit) = &mut self.on_submit {
            on_submit(&self.value);
        }
    }

    /// Get the line number and column for the current cursor position
    fn cursor_line_col(&self) -> (usize, usize) {
        let text_before = &self.value[..self.cursor_pos.min(self.value.len())];
        let line = text_before.matches('\n').count();
        let col = text_before
            .rfind('\n')
            .map(|pos| self.cursor_pos - pos - 1)
            .unwrap_or(self.cursor_pos);
        (line, col)
    }

    /// Get the text on the current line before the cursor
    fn text_before_cursor_on_line(&self) -> &str {
        let text_before = &self.value[..self.cursor_pos.min(self.value.len())];
        text_before.rfind('\n').map_or(text_before, |pos| &text_before[pos + 1..])
    }

    /// Get character index at a specific (x, y) position for multiline text
    fn char_index_at_xy(&self, x: f32, y: f32, text_start_x: f32, text_start_y: f32) -> usize {
        let line_height = self.font_size * 1.4;
        let relative_y = (y - text_start_y).max(0.0);
        let line_index = (relative_y / line_height) as usize;

        let lines: Vec<&str> = self.value.split('\n').collect();
        let target_line = line_index.min(lines.len().saturating_sub(1));

        // Calculate byte offset to the start of target line
        let mut byte_offset = 0;
        for (i, line) in lines.iter().enumerate() {
            if i == target_line {
                break;
            }
            byte_offset += line.len() + 1; // +1 for the newline
        }

        // Calculate column within the line
        let relative_x = (x - text_start_x).max(0.0);
        let char_width = self.font_size * 0.6;
        let col = (relative_x / char_width).round() as usize;
        let line_len = lines.get(target_line).map_or(0, |l| l.len());
        let col = col.min(line_len);

        (byte_offset + col).min(self.value.len())
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for TextInput {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let border = if self.focused {
            self.border_color_focused
        } else {
            self.border_color
        };

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.background)
                .with_border(border, 1.0),
        );

        let text_x = bounds.origin.x + self.padding.0;
        let line_height = self.font_size * 1.4;

        let display_text = if self.value.is_empty() {
            &self.placeholder
        } else {
            &self.value
        };

        let text_color = if self.value.is_empty() {
            self.placeholder_color
        } else {
            self.text_color
        };

        // Render each line separately for multiline support
        if !display_text.is_empty() {
            let lines: Vec<&str> = display_text.split('\n').collect();
            for (i, line) in lines.iter().enumerate() {
                let line_y = bounds.origin.y + self.padding.1 + line_height * i as f32 + self.font_size * 0.35;
                if !line.is_empty() {
                    let text_run = if self.mono {
                        cx.text.layout_styled_mono(
                            line,
                            Point::new(text_x, line_y),
                            self.font_size,
                            text_color,
                            FontStyle::default(),
                        )
                    } else {
                        cx.text.layout_mono(
                            line,
                            Point::new(text_x, line_y),
                            self.font_size,
                            text_color,
                        )
                    };
                    cx.scene.draw_text(text_run);
                }
            }
        }

        if self.focused {
            // Blink cursor: 500ms on, 500ms off
            let elapsed = self.cursor_blink_start.elapsed().as_millis();
            let cursor_visible = (elapsed / 500) % 2 == 0;

            if cursor_visible {
                let (cursor_line, _cursor_col) = self.cursor_line_col();
                let text_before = self.text_before_cursor_on_line();
                let cursor_x = text_x + text_before.chars().count() as f32 * self.font_size * 0.6;
                let cursor_y = bounds.origin.y + self.padding.1 + line_height * cursor_line as f32;
                let cursor_height = line_height;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(cursor_x, cursor_y, 2.0, cursor_height))
                        .with_background(self.cursor_color),
                );
            }
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }

            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let clicked_inside = bounds.contains(Point::new(*x, *y));

                    if clicked_inside {
                        self.focused = true;
                        self.reset_cursor_blink();

                        let text_x = bounds.origin.x + self.padding.0;
                        let text_y = bounds.origin.y + self.padding.1;
                        self.cursor_pos = self.char_index_at_xy(*x, *y, text_x, text_y);

                        if let Some(id) = self.id {
                            cx.set_focus(id);
                        }

                        return EventResult::Handled;
                    } else if self.focused {
                        self.focused = false;
                        cx.clear_focus();
                        return EventResult::Handled;
                    }
                }
            }

            InputEvent::KeyDown { key, modifiers } => {
                if !self.focused {
                    return EventResult::Ignored;
                }

                // Reset cursor blink on any key press for immediate visibility
                self.reset_cursor_blink();

                match key {
                    Key::Character(c) => {
                        if modifiers.ctrl || modifiers.meta {
                            match c.as_str() {
                                "a" | "A" => {
                                    self.select_all();
                                }
                                "c" | "C" => {
                                    // Copy: selection or entire value
                                    if let Some((start, end)) = self.get_selection() {
                                        cx.write_clipboard(&self.value[start..end]);
                                    } else {
                                        cx.write_clipboard(&self.value);
                                    }
                                }
                                "x" | "X" => {
                                    // Cut: copy selection then delete it
                                    if let Some((start, end)) = self.get_selection() {
                                        cx.write_clipboard(&self.value[start..end]);
                                        self.delete_selection();
                                    }
                                }
                                "v" | "V" => {
                                    // Paste: delete selection first, then insert
                                    if let Some(text) = cx.read_clipboard() {
                                        self.delete_selection();
                                        self.insert_str(&text);
                                    }
                                }
                                _ => {}
                            }
                        } else {
                            // Regular character input - delete selection first
                            self.delete_selection();
                            self.insert_str(c);
                        }
                        return EventResult::Handled;
                    }

                    Key::Named(named) => {
                        match named {
                            NamedKey::Space => {
                                // Space key - delete selection first, then insert space
                                self.delete_selection();
                                self.insert_str(" ");
                            }
                            NamedKey::Backspace => {
                                if !self.delete_selection() {
                                    self.delete_backward();
                                }
                            }
                            NamedKey::Delete => {
                                if !self.delete_selection() {
                                    self.delete_forward();
                                }
                            }
                            NamedKey::Enter => {
                                if modifiers.shift {
                                    // Shift+Enter: insert newline
                                    self.delete_selection();
                                    self.insert_char('\n');
                                } else {
                                    // Plain Enter: submit
                                    self.notify_submit();
                                }
                            }
                            NamedKey::Escape => {
                                self.focused = false;
                                cx.clear_focus();
                            }
                            NamedKey::Home => {
                                self.selection_start = None;
                                self.move_cursor_to_start();
                            }
                            NamedKey::End => {
                                self.selection_start = None;
                                self.move_cursor_to_end();
                            }
                            NamedKey::ArrowLeft => {
                                self.selection_start = None;
                                if modifiers.ctrl || modifiers.meta {
                                    self.move_cursor_to_start();
                                } else {
                                    self.move_cursor_left();
                                }
                            }
                            NamedKey::ArrowRight => {
                                self.selection_start = None;
                                if modifiers.ctrl || modifiers.meta {
                                    self.move_cursor_to_end();
                                } else {
                                    self.move_cursor_right();
                                }
                            }
                            NamedKey::Tab => {
                                return EventResult::Ignored;
                            }
                            _ => {}
                        }
                        return EventResult::Handled;
                    }
                }
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(self.current_height()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_new() {
        let input = TextInput::new();
        assert!(input.value.is_empty());
        assert!(input.placeholder.is_empty());
        assert_eq!(input.cursor_pos, 0);
        assert!(!input.focused);
    }

    #[test]
    fn test_text_input_builder() {
        let input = TextInput::new()
            .with_id(42)
            .value("hello")
            .placeholder("Enter text...");

        assert_eq!(input.id, Some(42));
        assert_eq!(input.value, "hello");
        assert_eq!(input.cursor_pos, 5);
        assert_eq!(input.placeholder, "Enter text...");
    }

    #[test]
    fn test_insert_char() {
        let mut input = TextInput::new();
        input.insert_char('a');
        assert_eq!(input.value, "a");
        assert_eq!(input.cursor_pos, 1);

        input.insert_char('b');
        assert_eq!(input.value, "ab");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn test_insert_str() {
        let mut input = TextInput::new();
        input.insert_str("hello");
        assert_eq!(input.value, "hello");
        assert_eq!(input.cursor_pos, 5);
    }

    #[test]
    fn test_delete_backward() {
        let mut input = TextInput::new().value("hello");
        input.delete_backward();
        assert_eq!(input.value, "hell");
        assert_eq!(input.cursor_pos, 4);
    }

    #[test]
    fn test_delete_backward_at_start() {
        let mut input = TextInput::new().value("hello");
        input.cursor_pos = 0;
        input.delete_backward();
        assert_eq!(input.value, "hello");
        assert_eq!(input.cursor_pos, 0);
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = TextInput::new().value("hello");

        input.move_cursor_left();
        assert_eq!(input.cursor_pos, 4);

        input.move_cursor_right();
        assert_eq!(input.cursor_pos, 5);

        input.move_cursor_to_start();
        assert_eq!(input.cursor_pos, 0);

        input.move_cursor_to_end();
        assert_eq!(input.cursor_pos, 5);
    }

    #[test]
    fn test_focus_blur() {
        let mut input = TextInput::new();
        assert!(!input.is_focused());

        input.focus();
        assert!(input.is_focused());

        input.blur();
        assert!(!input.is_focused());
    }

    #[test]
    fn test_set_value() {
        let mut input = TextInput::new().value("hello");
        assert_eq!(input.cursor_pos, 5);

        input.set_value("hi");
        assert_eq!(input.value, "hi");
        assert_eq!(input.cursor_pos, 2);
    }

    #[test]
    fn test_size_hint() {
        let input = TextInput::new().font_size(14.0).padding(8.0, 4.0);
        let (width, height) = input.size_hint();

        assert!(width.is_none());
        assert!(height.is_some());
        assert!(height.unwrap() > 0.0);
    }
}
