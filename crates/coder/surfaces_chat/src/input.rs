//! ChatInput widget - message composition input.
//!
//! Provides a text input area for composing and sending messages.

use coder_widgets::context::{EventContext, PaintContext};
use coder_widgets::{Button, ButtonVariant, EventResult, TextInput, Widget, WidgetId};
use wgpui::{Bounds, Hsla, InputEvent};

/// Callback for when a message is sent.
pub type OnSendMessage = Box<dyn FnMut(String)>;

/// Max width for the input container.
const MAX_WIDTH: f32 = 768.0;

/// Chat input widget.
pub struct ChatInput {
    /// Widget ID.
    id: Option<WidgetId>,

    /// Text input widget.
    text_input: TextInput,

    /// Send button.
    send_button: Button,

    /// Whether the input is enabled.
    enabled: bool,

    /// Placeholder text.
    placeholder: String,

    /// Send callback.
    on_send: Option<OnSendMessage>,

    /// Current value (synced with text_input).
    current_value: String,
}

impl ChatInput {
    /// Create a new chat input.
    pub fn new() -> Self {
        // White with 10% opacity for button background
        let button_bg = Hsla::new(0.0, 0.0, 1.0, 0.1);
        // White text
        let button_text = Hsla::new(0.0, 0.0, 1.0, 1.0);

        Self {
            id: None,
            text_input: TextInput::new()
                .placeholder("Type a message...")
                .font_size(13.0)
                .border_color(Hsla::transparent())
                .focused_border_color(Hsla::transparent()),
            send_button: Button::new("Send")
                .variant(ButtonVariant::Ghost)
                .background(button_bg)
                .text_color(button_text)
                .font_size(12.0)
                .corner_radius(6.0),
            enabled: true,
            placeholder: "Type a message...".to_string(),
            on_send: None,
            current_value: String::new(),
        }
    }

    /// Set the widget ID.
    pub fn id(mut self, id: WidgetId) -> Self {
        self.id = Some(id);
        self
    }

    /// Set enabled state.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self.send_button = self.send_button.disabled(!enabled);
        self
    }

    /// Set placeholder text.
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self.text_input = TextInput::new()
            .placeholder(&self.placeholder)
            .font_size(13.0)
            .border_color(Hsla::transparent())
            .focused_border_color(Hsla::transparent());
        self
    }

    /// Set the on_send callback.
    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_send = Some(Box::new(f));
        self
    }

    /// Get the current input value.
    pub fn value(&self) -> &str {
        &self.current_value
    }

    /// Set the input value.
    pub fn set_value(&mut self, value: impl Into<String>) {
        self.current_value = value.into();
        self.text_input.set_value(&self.current_value);
    }

    /// Clear the input.
    pub fn clear(&mut self) {
        self.current_value.clear();
        self.text_input.set_value("");
    }

    /// Check if input is empty.
    pub fn is_empty(&self) -> bool {
        self.current_value.trim().is_empty()
    }

    /// Send the current message.
    fn send(&mut self) {
        if self.is_empty() {
            return;
        }

        let message = std::mem::take(&mut self.current_value);

        if let Some(on_send) = &mut self.on_send {
            on_send(message);
        }

        self.clear();
    }
}

impl Default for ChatInput {
    fn default() -> Self {
        Self::new()
    }
}

impl Widget for ChatInput {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Center content with max width
        let content_width = bounds.size.width.min(MAX_WIDTH);
        let offset_x = (bounds.size.width - content_width) / 2.0;
        let centered = Bounds::new(
            bounds.origin.x + offset_x,
            bounds.origin.y,
            content_width,
            bounds.size.height,
        );

        // Calculate layout: [input field] [send button]
        let button_width = 60.0;
        let gap = 8.0;

        let input_bounds = Bounds::new(
            centered.origin.x,
            centered.origin.y,
            centered.size.width - button_width - gap,
            centered.size.height,
        );

        let button_bounds = Bounds::new(
            centered.origin.x + centered.size.width - button_width,
            centered.origin.y,
            button_width,
            centered.size.height,
        );

        // Paint input
        self.text_input.paint(input_bounds, cx);

        // Paint button
        self.send_button.paint(button_bounds, cx);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if !self.enabled {
            return EventResult::Ignored;
        }

        // Center content with max width (must match paint)
        let content_width = bounds.size.width.min(MAX_WIDTH);
        let offset_x = (bounds.size.width - content_width) / 2.0;
        let centered = Bounds::new(
            bounds.origin.x + offset_x,
            bounds.origin.y,
            content_width,
            bounds.size.height,
        );

        // Calculate layout (must match paint)
        let button_width = 60.0;
        let gap = 8.0;

        let input_bounds = Bounds::new(
            centered.origin.x,
            centered.origin.y,
            centered.size.width - button_width - gap,
            centered.size.height,
        );

        let button_bounds = Bounds::new(
            centered.origin.x + centered.size.width - button_width,
            centered.origin.y,
            button_width,
            centered.size.height,
        );

        // Check for Enter key to send
        if let InputEvent::KeyDown { key, .. } = event {
            if let wgpui::Key::Named(wgpui::NamedKey::Enter) = key {
                if !self.is_empty() {
                    self.send();
                    return EventResult::Handled;
                }
            }
        }

        // Check for button click (MouseUp on button)
        if let InputEvent::MouseUp { position, button, .. } = event {
            if *button == wgpui::MouseButton::Left && button_bounds.contains(*position) {
                self.send();
                return EventResult::Handled;
            }
        }

        // Check if this is a mouse event on the button area - don't forward to text_input
        // to avoid unfocusing when clicking the send button
        let is_button_mouse_event = match event {
            InputEvent::MouseDown { position, .. } |
            InputEvent::MouseUp { position, .. } |
            InputEvent::MouseMove { position, .. } => button_bounds.contains(*position),
            _ => false,
        };

        if is_button_mouse_event {
            // Let button handle hover/press states
            let _ = self.send_button.event(event, button_bounds, cx);
            return EventResult::Handled;
        }

        // Handle click anywhere in the input area to focus the text input
        if let InputEvent::MouseDown { position, button, .. } = event {
            if *button == wgpui::MouseButton::Left && bounds.contains(*position) {
                self.text_input.set_focused(true);
                return EventResult::Handled;
            }
        }

        // Handle input events (for typing)
        let result = self.text_input.event(event, input_bounds, cx);
        if result.is_handled() {
            // Sync value from text input
            self.current_value = self.text_input.get_value().to_string();
            return result;
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<WidgetId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(40.0)) // Fixed height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chat_input_creation() {
        let input = ChatInput::new()
            .id(1)
            .placeholder("Enter message...")
            .enabled(true);

        assert_eq!(input.id, Some(1));
        assert!(input.enabled);
    }

    #[test]
    fn test_chat_input_value() {
        let mut input = ChatInput::new();

        assert!(input.is_empty());

        input.set_value("Hello!");
        assert!(!input.is_empty());
        assert_eq!(input.value(), "Hello!");

        input.clear();
        assert!(input.is_empty());
    }

    #[test]
    fn test_chat_input_empty_whitespace() {
        let mut input = ChatInput::new();

        input.set_value("   ");
        assert!(input.is_empty()); // Whitespace-only counts as empty
    }

    #[test]
    fn test_chat_input_disabled() {
        let input = ChatInput::new().enabled(false);
        assert!(!input.enabled);
    }
}
