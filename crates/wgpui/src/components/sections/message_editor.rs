use crate::components::atoms::{Mode, ModeBadge};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Button, ButtonVariant, Component, ComponentId, EventResult, TextInput};
use crate::input::{Key, NamedKey};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct MessageEditor {
    id: Option<ComponentId>,
    input: TextInput,
    mode: Mode,
    placeholder: String,
    is_streaming: bool,
    show_mode_badge: bool,
    show_keybinding_hint: bool,
    send_hovered: bool,
    on_send: Option<Box<dyn FnMut(String)>>,
    on_cancel: Option<Box<dyn FnMut()>>,
}

impl MessageEditor {
    pub fn new() -> Self {
        Self {
            id: None,
            input: TextInput::new()
                .placeholder("Type a message...")
                .background(theme::bg::SURFACE),
            mode: Mode::Normal,
            placeholder: "Type a message...".to_string(),
            is_streaming: false,
            show_mode_badge: true,
            show_keybinding_hint: true,
            send_hovered: false,
            on_send: None,
            on_cancel: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self.input = self.input.placeholder(&self.placeholder);
        self
    }

    pub fn mode(mut self, mode: Mode) -> Self {
        self.mode = mode;
        self
    }

    pub fn streaming(mut self, streaming: bool) -> Self {
        self.is_streaming = streaming;
        self
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        self.is_streaming = streaming;
    }

    pub fn show_mode_badge(mut self, show: bool) -> Self {
        self.show_mode_badge = show;
        self
    }

    pub fn show_keybinding_hint(mut self, show: bool) -> Self {
        self.show_keybinding_hint = show;
        self
    }

    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_send = Some(Box::new(f));
        self
    }

    pub fn on_cancel<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_cancel = Some(Box::new(f));
        self
    }

    pub fn value(&self) -> &str {
        self.input.get_value()
    }

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.input.set_value(value);
    }

    pub fn clear(&mut self) {
        self.input.set_value("");
    }

    pub fn focus(&mut self) {
        self.input.set_focused(true);
    }

    pub fn is_focused(&self) -> bool {
        self.input.is_focused()
    }

    pub fn is_streaming(&self) -> bool {
        self.is_streaming
    }

    fn send_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_width = if self.is_streaming { 60.0 } else { 50.0 };
        let btn_height = 32.0;
        let padding = theme::spacing::SM;

        Bounds::new(
            bounds.origin.x + bounds.size.width - padding - btn_width,
            bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
            btn_width,
            btn_height,
        )
    }

    fn trigger_send(&mut self) {
        let value = self.input.get_value().to_string();
        if !value.trim().is_empty() {
            if let Some(callback) = &mut self.on_send {
                callback(value);
            }
            self.input.set_value("");
        }
    }
}

impl Default for MessageEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for MessageEditor {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        let padding = theme::spacing::SM;
        let mut left_x = bounds.origin.x + padding;

        if self.show_mode_badge {
            let mut badge = ModeBadge::new(self.mode);
            let (badge_w, badge_h) = badge.size_hint();
            let badge_bounds = Bounds::new(
                left_x,
                bounds.origin.y + (bounds.size.height - badge_h.unwrap_or(24.0)) / 2.0,
                badge_w.unwrap_or(60.0),
                badge_h.unwrap_or(24.0),
            );
            badge.paint(badge_bounds, cx);
            left_x += badge_w.unwrap_or(60.0) + padding;
        }

        let send_bounds = self.send_button_bounds(&bounds);
        let input_width = send_bounds.origin.x - left_x - padding;
        let input_height = 36.0;

        let input_bounds = Bounds::new(
            left_x,
            bounds.origin.y + (bounds.size.height - input_height) / 2.0,
            input_width,
            input_height,
        );
        self.input.paint(input_bounds, cx);

        if self.is_streaming {
            let mut cancel_btn = Button::new("Cancel").variant(ButtonVariant::Danger);
            cancel_btn.paint(send_bounds, cx);
        } else {
            let bg = if self.send_hovered {
                theme::accent::PRIMARY.lighten(0.1)
            } else {
                theme::accent::PRIMARY
            };

            cx.scene
                .draw_quad(Quad::new(send_bounds).with_background(bg));

            let arrow = "\u{2191}";
            let arrow_size = theme::font_size::LG;
            let arrow_run = cx.text.layout(
                arrow,
                Point::new(
                    send_bounds.origin.x + (send_bounds.size.width - arrow_size * 0.5) / 2.0,
                    send_bounds.origin.y + (send_bounds.size.height - arrow_size) / 2.0,
                ),
                arrow_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(arrow_run);
        }

        if self.show_keybinding_hint && !self.is_streaming {
            let hint_text = "Enter to send";
            let hint_font_size = theme::font_size::XS;
            let hint_y = bounds.origin.y + bounds.size.height - padding - hint_font_size;

            let hint_run = cx.text.layout(
                hint_text,
                Point::new(left_x, hint_y),
                hint_font_size,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(hint_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_hovered = self.send_hovered;
                self.send_hovered = self.send_button_bounds(&bounds).contains(point);
                if was_hovered != self.send_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);
                if self.send_button_bounds(&bounds).contains(point) {
                    if self.is_streaming {
                        if let Some(callback) = &mut self.on_cancel {
                            callback();
                        }
                    } else {
                        self.trigger_send();
                    }
                    return EventResult::Handled;
                }
            }
            InputEvent::KeyDown { key, .. } => {
                if let Key::Named(NamedKey::Enter) = key
                    && self.input.is_focused()
                    && !self.is_streaming
                {
                    self.trigger_send();
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        let padding = theme::spacing::SM;
        let mut left_x = bounds.origin.x + padding;

        if self.show_mode_badge {
            left_x += 60.0 + padding;
        }

        let send_bounds = self.send_button_bounds(&bounds);
        let input_width = send_bounds.origin.x - left_x - padding;
        let input_height = 36.0;

        let input_bounds = Bounds::new(
            left_x,
            bounds.origin.y + (bounds.size.height - input_height) / 2.0,
            input_width,
            input_height,
        );

        self.input.event(event, input_bounds, cx)
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(64.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Modifiers;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[test]
    fn test_message_editor_new() {
        let editor = MessageEditor::new();
        assert!(!editor.is_streaming());
        assert!(!editor.is_focused());
    }

    #[test]
    fn test_message_editor_builder() {
        let editor = MessageEditor::new()
            .with_id(1)
            .placeholder("Ask anything...")
            .mode(Mode::Plan)
            .streaming(true)
            .show_mode_badge(false);

        assert_eq!(editor.id, Some(1));
        assert!(editor.is_streaming());
        assert!(!editor.show_mode_badge);
    }

    #[test]
    fn test_message_editor_set_streaming() {
        let mut editor = MessageEditor::new();
        assert!(!editor.is_streaming());
        editor.set_streaming(true);
        assert!(editor.is_streaming());
    }

    #[test]
    fn test_message_editor_value() {
        let mut editor = MessageEditor::new();
        editor.set_value("Hello");
        assert_eq!(editor.value(), "Hello");

        editor.clear();
        assert_eq!(editor.value(), "");
    }

    #[test]
    fn test_message_editor_focus() {
        let mut editor = MessageEditor::new();
        assert!(!editor.is_focused());

        editor.focus();
        assert!(editor.is_focused());
    }

    #[test]
    fn test_message_editor_size_hint() {
        let editor = MessageEditor::new();
        let (w, h) = editor.size_hint();
        assert!(w.is_none());
        assert_eq!(h, Some(64.0));
    }

    #[test]
    fn test_message_editor_send_on_enter() {
        let sent = Rc::new(RefCell::new(None));
        let sent_clone = sent.clone();
        let mut editor = MessageEditor::new().on_send(move |value| {
            *sent_clone.borrow_mut() = Some(value);
        });

        editor.set_value("Hello");
        editor.focus();

        let mut cx = EventContext::new();
        let event = InputEvent::KeyDown {
            key: Key::Named(NamedKey::Enter),
            modifiers: Modifiers::default(),
        };
        let bounds = Bounds::new(0.0, 0.0, 400.0, 64.0);

        let result = editor.event(&event, bounds, &mut cx);

        assert_eq!(result, EventResult::Handled);
        assert_eq!(sent.borrow().as_deref(), Some("Hello"));
        assert_eq!(editor.value(), "");
    }
}
