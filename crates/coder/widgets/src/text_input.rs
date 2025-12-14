//! TextInput widget - single-line text input field.
//!
//! The TextInput widget handles keyboard input, cursor positioning,
//! and text selection for user input.

use crate::context::{EventContext, PaintContext};
use crate::widget::{EventResult, Widget, WidgetId};
use wgpui::{Bounds, Hsla, InputEvent, Key, KeyCode, MouseButton, NamedKey, Point, Quad};

/// Callback for when the input is submitted (Enter pressed).
pub type OnSubmit = Box<dyn FnMut(&str)>;

/// Callback for when the value changes.
pub type OnChange = Box<dyn FnMut(&str)>;

/// A single-line text input widget.
pub struct TextInput {
    /// Unique ID for this widget.
    id: Option<WidgetId>,
    /// Current text content.
    value: String,
    /// Placeholder text shown when empty.
    placeholder: String,
    /// Cursor position (character index).
    cursor: usize,
    /// Selection start (if selecting).
    selection_start: Option<usize>,
    /// Whether the input has focus.
    focused: bool,
    /// Font size.
    font_size: f32,
    /// Text color.
    text_color: Hsla,
    /// Placeholder color.
    placeholder_color: Hsla,
    /// Background color.
    background: Hsla,
    /// Border color.
    border_color: Hsla,
    /// Focused border color.
    focused_border_color: Hsla,
    /// Corner radius.
    corner_radius: f32,
    /// Padding.
    padding: f32,
    /// Callback on submit.
    on_submit: Option<OnSubmit>,
    /// Callback on change.
    on_change: Option<OnChange>,
    /// Cursor blink state (frame counter).
    cursor_blink_frame: u64,
}

impl TextInput {
    /// Create a new text input.
    pub fn new() -> Self {
        Self {
            id: None,
            value: String::new(),
            placeholder: String::new(),
            cursor: 0,
            selection_start: None,
            focused: false,
            font_size: 14.0,
            text_color: wgpui::theme::text::PRIMARY,
            placeholder_color: wgpui::theme::text::MUTED,
            background: wgpui::theme::bg::SURFACE,
            border_color: wgpui::theme::border::DEFAULT,
            focused_border_color: wgpui::theme::accent::PRIMARY,
            corner_radius: 4.0,
            padding: 8.0,
            on_submit: None,
            on_change: None,
            cursor_blink_frame: 0,
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set the initial value.
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor = self.value.len();
        self
    }

    /// Set the placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// Set the font size.
    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    /// Set the text color.
    pub fn text_color(mut self, color: Hsla) -> Self {
        self.text_color = color;
        self
    }

    /// Set the background color.
    pub fn background(mut self, color: Hsla) -> Self {
        self.background = color;
        self
    }

    /// Set the corner radius.
    pub fn corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }

    /// Set the padding.
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = padding;
        self
    }

    /// Set the on_submit callback.
    pub fn on_submit<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_submit = Some(Box::new(f));
        self
    }

    /// Set the on_change callback.
    pub fn on_change<F>(mut self, f: F) -> Self
    where
        F: FnMut(&str) + 'static,
    {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Get the current value.
    pub fn get_value(&self) -> &str {
        &self.value
    }

    /// Set the value programmatically.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.cursor.min(self.value.len());
    }

    /// Check if focused.
    pub fn is_focused(&self) -> bool {
        self.focused
    }

    /// Set focus state.
    pub fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
        if focused {
            self.cursor_blink_frame = 0;
        }
    }

    /// Handle character insertion.
    fn insert_char(&mut self, c: char) {
        // Delete selection if any
        if let Some(start) = self.selection_start.take() {
            let (from, to) = if start < self.cursor {
                (start, self.cursor)
            } else {
                (self.cursor, start)
            };
            self.value.drain(from..to);
            self.cursor = from;
        }

        self.value.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.notify_change();
    }

    /// Handle text insertion.
    fn insert_text(&mut self, text: &str) {
        // Delete selection if any
        if let Some(start) = self.selection_start.take() {
            let (from, to) = if start < self.cursor {
                (start, self.cursor)
            } else {
                (self.cursor, start)
            };
            self.value.drain(from..to);
            self.cursor = from;
        }

        self.value.insert_str(self.cursor, text);
        self.cursor += text.len();
        self.notify_change();
    }

    /// Handle backspace.
    fn backspace(&mut self) {
        if let Some(start) = self.selection_start.take() {
            // Delete selection
            let (from, to) = if start < self.cursor {
                (start, self.cursor)
            } else {
                (self.cursor, start)
            };
            self.value.drain(from..to);
            self.cursor = from;
            self.notify_change();
        } else if self.cursor > 0 {
            // Delete character before cursor
            let prev_char_len = self.value[..self.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor -= prev_char_len;
            self.value.drain(self.cursor..self.cursor + prev_char_len);
            self.notify_change();
        }
    }

    /// Handle delete.
    fn delete(&mut self) {
        if let Some(start) = self.selection_start.take() {
            // Delete selection
            let (from, to) = if start < self.cursor {
                (start, self.cursor)
            } else {
                (self.cursor, start)
            };
            self.value.drain(from..to);
            self.cursor = from;
            self.notify_change();
        } else if self.cursor < self.value.len() {
            // Delete character after cursor
            let next_char_len = self.value[self.cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.value.drain(self.cursor..self.cursor + next_char_len);
            self.notify_change();
        }
    }

    /// Move cursor left.
    fn move_left(&mut self, selecting: bool) {
        if !selecting {
            self.selection_start = None;
        } else if self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        }

        if self.cursor > 0 {
            let prev_char_len = self.value[..self.cursor]
                .chars()
                .last()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor -= prev_char_len;
        }
    }

    /// Move cursor right.
    fn move_right(&mut self, selecting: bool) {
        if !selecting {
            self.selection_start = None;
        } else if self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        }

        if self.cursor < self.value.len() {
            let next_char_len = self.value[self.cursor..]
                .chars()
                .next()
                .map(|c| c.len_utf8())
                .unwrap_or(0);
            self.cursor += next_char_len;
        }
    }

    /// Move cursor to start.
    fn move_home(&mut self, selecting: bool) {
        if !selecting {
            self.selection_start = None;
        } else if self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        }
        self.cursor = 0;
    }

    /// Move cursor to end.
    fn move_end(&mut self, selecting: bool) {
        if !selecting {
            self.selection_start = None;
        } else if self.selection_start.is_none() {
            self.selection_start = Some(self.cursor);
        }
        self.cursor = self.value.len();
    }

    /// Select all text.
    fn select_all(&mut self) {
        self.selection_start = Some(0);
        self.cursor = self.value.len();
    }

    /// Get selected text.
    fn selected_text(&self) -> Option<&str> {
        self.selection_start.map(|start| {
            let (from, to) = if start < self.cursor {
                (start, self.cursor)
            } else {
                (self.cursor, start)
            };
            &self.value[from..to]
        })
    }

    /// Notify change callback.
    fn notify_change(&mut self) {
        if let Some(on_change) = &mut self.on_change {
            on_change(&self.value);
        }
    }

    /// Submit the input.
    fn submit(&mut self) {
        if let Some(on_submit) = &mut self.on_submit {
            on_submit(&self.value);
        }
    }

    /// Calculate cursor X position within text.
    fn cursor_x_offset(&self) -> f32 {
        // Approximate width based on character count
        // In a real implementation, this would use the text system
        let text_before_cursor = &self.value[..self.cursor];
        text_before_cursor.chars().count() as f32 * self.font_size * 0.6
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for TextInput {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Increment blink counter
        self.cursor_blink_frame = self.cursor_blink_frame.wrapping_add(1);

        // Draw background
        let border_color = if self.focused {
            self.focused_border_color
        } else {
            self.border_color
        };

        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(self.background)
                .with_border(border_color, 1.0)
                .with_uniform_radius(self.corner_radius),
        );

        // Calculate text position
        let text_x = bounds.origin.x + self.padding;
        let text_y = bounds.origin.y + (bounds.size.height - self.font_size) / 2.0;

        // Draw selection background if selecting
        if let Some(start) = self.selection_start {
            if start != self.cursor {
                let (from, to) = if start < self.cursor {
                    (start, self.cursor)
                } else {
                    (self.cursor, start)
                };
                let from_x = text_x + self.value[..from].chars().count() as f32 * self.font_size * 0.6;
                let to_x = text_x + self.value[..to].chars().count() as f32 * self.font_size * 0.6;

                cx.scene.draw_quad(
                    Quad::new(Bounds::new(
                        from_x,
                        bounds.origin.y + 4.0,
                        to_x - from_x,
                        bounds.size.height - 8.0,
                    ))
                    .with_background(wgpui::theme::accent::PRIMARY.with_alpha(0.3)),
                );
            }
        }

        // Draw text or placeholder
        let (text, color) = if self.value.is_empty() {
            (&self.placeholder, self.placeholder_color)
        } else {
            (&self.value, self.text_color)
        };

        if !text.is_empty() {
            let text_run = cx.text.layout(
                text,
                Point::new(text_x, text_y),
                self.font_size,
                color,
            );
            cx.scene.draw_text(text_run);
        }

        // Draw cursor (blink every 30 frames ~= 500ms at 60fps)
        if self.focused && (self.cursor_blink_frame / 30) % 2 == 0 {
            let cursor_x = text_x + self.cursor_x_offset();
            cx.scene.draw_quad(
                Quad::new(Bounds::new(
                    cursor_x,
                    bounds.origin.y + 4.0,
                    2.0,
                    bounds.size.height - 8.0,
                ))
                .with_background(self.text_color),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseDown { position, button, .. } => {
                if *button == MouseButton::Left && bounds.contains(*position) {
                    self.focused = true;
                    self.cursor_blink_frame = 0;
                    self.selection_start = None;
                    // TODO: Calculate cursor position from click
                    return EventResult::Handled;
                } else if *button == MouseButton::Left {
                    self.focused = false;
                }
            }

            InputEvent::TextInput { text } if self.focused => {
                self.insert_text(text);
                return EventResult::Handled;
            }

            InputEvent::KeyDown { key, code, modifiers, .. } if self.focused => {
                match key {
                    Key::Named(NamedKey::Enter) => {
                        self.submit();
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::Backspace) => {
                        self.backspace();
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::Delete) => {
                        self.delete();
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::ArrowLeft) => {
                        self.move_left(modifiers.shift);
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::ArrowRight) => {
                        self.move_right(modifiers.shift);
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::Home) => {
                        self.move_home(modifiers.shift);
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::End) => {
                        self.move_end(modifiers.shift);
                        return EventResult::Handled;
                    }
                    Key::Named(NamedKey::Escape) => {
                        self.focused = false;
                        return EventResult::Handled;
                    }
                    Key::Character(c) if modifiers.command() => {
                        match c.as_str() {
                            "a" => {
                                self.select_all();
                                return EventResult::Handled;
                            }
                            "c" => {
                                // Copy - would need clipboard access
                                if let Some(_selected) = self.selected_text() {
                                    // TODO: Copy to clipboard
                                }
                                return EventResult::Handled;
                            }
                            "v" => {
                                // Paste - would need clipboard access
                                // TODO: Paste from clipboard
                                return EventResult::Handled;
                            }
                            "x" => {
                                // Cut
                                if let Some(start) = self.selection_start.take() {
                                    let (from, to) = if start < self.cursor {
                                        (start, self.cursor)
                                    } else {
                                        (self.cursor, start)
                                    };
                                    // TODO: Copy to clipboard first
                                    self.value.drain(from..to);
                                    self.cursor = from;
                                    self.notify_change();
                                }
                                return EventResult::Handled;
                            }
                            _ => {}
                        }
                    }
                    // Direct character input for non-command keys
                    Key::Character(c) if !modifiers.command() && !modifiers.ctrl => {
                        // Only insert printable characters
                        if c.chars().all(|ch| !ch.is_control()) {
                            for ch in c.chars() {
                                self.insert_char(ch);
                            }
                            return EventResult::Handled;
                        }
                    }
                    _ => {}
                }

                // Check for printable ASCII via key code
                if !modifiers.command() && !modifiers.ctrl && !modifiers.alt {
                    if let Some(c) = keycode_to_char(*code, modifiers.shift) {
                        self.insert_char(c);
                        return EventResult::Handled;
                    }
                }
            }

            InputEvent::FocusIn if bounds.contains(Point::new(bounds.origin.x, bounds.origin.y)) => {
                self.focused = true;
                self.cursor_blink_frame = 0;
                return EventResult::Handled;
            }

            InputEvent::FocusOut => {
                self.focused = false;
            }

            _ => {}
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = self.font_size * 1.4 + self.padding * 2.0;
        (None, Some(height))
    }
}

/// Convert a key code to a character (basic ASCII).
fn keycode_to_char(code: KeyCode, shift: bool) -> Option<char> {
    match code {
        KeyCode::KeyA => Some(if shift { 'A' } else { 'a' }),
        KeyCode::KeyB => Some(if shift { 'B' } else { 'b' }),
        KeyCode::KeyC => Some(if shift { 'C' } else { 'c' }),
        KeyCode::KeyD => Some(if shift { 'D' } else { 'd' }),
        KeyCode::KeyE => Some(if shift { 'E' } else { 'e' }),
        KeyCode::KeyF => Some(if shift { 'F' } else { 'f' }),
        KeyCode::KeyG => Some(if shift { 'G' } else { 'g' }),
        KeyCode::KeyH => Some(if shift { 'H' } else { 'h' }),
        KeyCode::KeyI => Some(if shift { 'I' } else { 'i' }),
        KeyCode::KeyJ => Some(if shift { 'J' } else { 'j' }),
        KeyCode::KeyK => Some(if shift { 'K' } else { 'k' }),
        KeyCode::KeyL => Some(if shift { 'L' } else { 'l' }),
        KeyCode::KeyM => Some(if shift { 'M' } else { 'm' }),
        KeyCode::KeyN => Some(if shift { 'N' } else { 'n' }),
        KeyCode::KeyO => Some(if shift { 'O' } else { 'o' }),
        KeyCode::KeyP => Some(if shift { 'P' } else { 'p' }),
        KeyCode::KeyQ => Some(if shift { 'Q' } else { 'q' }),
        KeyCode::KeyR => Some(if shift { 'R' } else { 'r' }),
        KeyCode::KeyS => Some(if shift { 'S' } else { 's' }),
        KeyCode::KeyT => Some(if shift { 'T' } else { 't' }),
        KeyCode::KeyU => Some(if shift { 'U' } else { 'u' }),
        KeyCode::KeyV => Some(if shift { 'V' } else { 'v' }),
        KeyCode::KeyW => Some(if shift { 'W' } else { 'w' }),
        KeyCode::KeyX => Some(if shift { 'X' } else { 'x' }),
        KeyCode::KeyY => Some(if shift { 'Y' } else { 'y' }),
        KeyCode::KeyZ => Some(if shift { 'Z' } else { 'z' }),
        KeyCode::Digit0 => Some(if shift { ')' } else { '0' }),
        KeyCode::Digit1 => Some(if shift { '!' } else { '1' }),
        KeyCode::Digit2 => Some(if shift { '@' } else { '2' }),
        KeyCode::Digit3 => Some(if shift { '#' } else { '3' }),
        KeyCode::Digit4 => Some(if shift { '$' } else { '4' }),
        KeyCode::Digit5 => Some(if shift { '%' } else { '5' }),
        KeyCode::Digit6 => Some(if shift { '^' } else { '6' }),
        KeyCode::Digit7 => Some(if shift { '&' } else { '7' }),
        KeyCode::Digit8 => Some(if shift { '*' } else { '8' }),
        KeyCode::Digit9 => Some(if shift { '(' } else { '9' }),
        KeyCode::Space => Some(' '),
        KeyCode::Minus => Some(if shift { '_' } else { '-' }),
        KeyCode::Equal => Some(if shift { '+' } else { '=' }),
        KeyCode::BracketLeft => Some(if shift { '{' } else { '[' }),
        KeyCode::BracketRight => Some(if shift { '}' } else { ']' }),
        KeyCode::Backslash => Some(if shift { '|' } else { '\\' }),
        KeyCode::Semicolon => Some(if shift { ':' } else { ';' }),
        KeyCode::Quote => Some(if shift { '"' } else { '\'' }),
        KeyCode::Backquote => Some(if shift { '~' } else { '`' }),
        KeyCode::Comma => Some(if shift { '<' } else { ',' }),
        KeyCode::Period => Some(if shift { '>' } else { '.' }),
        KeyCode::Slash => Some(if shift { '?' } else { '/' }),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_input_creation() {
        let input = TextInput::new()
            .id(1)
            .value("Hello")
            .placeholder("Enter text...");

        assert_eq!(input.id, Some(1));
        assert_eq!(input.value, "Hello");
        assert_eq!(input.placeholder, "Enter text...");
        assert_eq!(input.cursor, 5); // At end of text
    }

    #[test]
    fn test_text_input_insertion() {
        let mut input = TextInput::new().value("Hello");
        input.cursor = 5;
        input.insert_char('!');

        assert_eq!(input.value, "Hello!");
        assert_eq!(input.cursor, 6);
    }

    #[test]
    fn test_text_input_backspace() {
        let mut input = TextInput::new().value("Hello!");
        input.cursor = 6;
        input.backspace();

        assert_eq!(input.value, "Hello");
        assert_eq!(input.cursor, 5);
    }

    #[test]
    fn test_text_input_selection() {
        let mut input = TextInput::new().value("Hello World");
        input.cursor = 0;
        input.move_right(true); // Select 'H'
        input.move_right(true); // Select 'e'
        input.move_right(true); // Select 'l'

        assert_eq!(input.selection_start, Some(0));
        assert_eq!(input.cursor, 3);
        assert_eq!(input.selected_text(), Some("Hel"));
    }

    #[test]
    fn test_text_input_select_all() {
        let mut input = TextInput::new().value("Hello World");
        input.select_all();

        assert_eq!(input.selection_start, Some(0));
        assert_eq!(input.cursor, 11);
        assert_eq!(input.selected_text(), Some("Hello World"));
    }

    #[test]
    fn test_text_input_replace_selection() {
        let mut input = TextInput::new().value("Hello World");
        input.selection_start = Some(0);
        input.cursor = 5;
        input.insert_text("Hi");

        assert_eq!(input.value, "Hi World");
    }
}
