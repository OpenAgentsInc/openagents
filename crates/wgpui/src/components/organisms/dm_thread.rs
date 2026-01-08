//! DM thread organism for displaying direct message conversations.
//!
//! Shows a scrollable conversation with message bubbles and input.

use crate::components::context::{EventContext, PaintContext};
use crate::components::molecules::{DmBubble, DmMessage};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

/// DM thread organism
pub struct DmThread {
    id: Option<ComponentId>,
    messages: Vec<DmMessage>,
    contact_name: String,
    contact_npub: String,
    input_text: String,
    scroll_offset: f32,
    input_focused: bool,
    send_hovered: bool,
    on_send: Option<Box<dyn FnMut(String)>>,
    on_scroll: Option<Box<dyn FnMut(f32)>>,
}

impl DmThread {
    pub fn new(contact_name: impl Into<String>, contact_npub: impl Into<String>) -> Self {
        Self {
            id: None,
            messages: Vec::new(),
            contact_name: contact_name.into(),
            contact_npub: contact_npub.into(),
            input_text: String::new(),
            scroll_offset: 0.0,
            input_focused: false,
            send_hovered: false,
            on_send: None,
            on_scroll: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn messages(mut self, messages: Vec<DmMessage>) -> Self {
        self.messages = messages;
        self
    }

    pub fn on_send<F>(mut self, f: F) -> Self
    where
        F: FnMut(String) + 'static,
    {
        self.on_send = Some(Box::new(f));
        self
    }

    pub fn on_scroll<F>(mut self, f: F) -> Self
    where
        F: FnMut(f32) + 'static,
    {
        self.on_scroll = Some(Box::new(f));
        self
    }

    fn header_bounds(&self, bounds: &Bounds) -> Bounds {
        Bounds::new(bounds.origin.x, bounds.origin.y, bounds.size.width, 56.0)
    }

    fn messages_bounds(&self, bounds: &Bounds) -> Bounds {
        let header_height = 56.0;
        let input_height = 60.0;
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + header_height,
            bounds.size.width,
            bounds.size.height - header_height - input_height,
        )
    }

    fn input_bounds(&self, bounds: &Bounds) -> Bounds {
        let input_height = 60.0;
        Bounds::new(
            bounds.origin.x,
            bounds.origin.y + bounds.size.height - input_height,
            bounds.size.width,
            input_height,
        )
    }

    fn send_button_bounds(&self, bounds: &Bounds) -> Bounds {
        let input = self.input_bounds(bounds);
        let padding = 12.0;
        Bounds::new(
            input.origin.x + input.size.width - padding - 60.0,
            input.origin.y + padding,
            50.0,
            36.0,
        )
    }

    fn short_npub(&self) -> String {
        if self.contact_npub.len() > 16 {
            format!("{}...", &self.contact_npub[..12])
        } else {
            self.contact_npub.clone()
        }
    }
}

impl Component for DmThread {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding = 12.0;

        // Background
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        // Header
        let header = self.header_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(header).with_background(theme::bg::MUTED));

        // Contact avatar placeholder
        let avatar_bounds = Bounds::new(
            header.origin.x + padding,
            header.origin.y + 10.0,
            36.0,
            36.0,
        );
        cx.scene.draw_quad(
            Quad::new(avatar_bounds)
                .with_background(theme::accent::PRIMARY.with_alpha(0.3))
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let initial = self
            .contact_name
            .chars()
            .next()
            .unwrap_or('?')
            .to_uppercase()
            .to_string();
        let initial_run = cx.text.layout(
            &initial,
            Point::new(avatar_bounds.origin.x + 12.0, avatar_bounds.origin.y + 10.0),
            theme::font_size::SM,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(initial_run);

        // Contact name
        let name_run = cx.text.layout(
            &self.contact_name,
            Point::new(header.origin.x + padding + 48.0, header.origin.y + 12.0),
            theme::font_size::SM,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(name_run);

        // Contact npub
        let npub_run = cx.text.layout(
            &self.short_npub(),
            Point::new(header.origin.x + padding + 48.0, header.origin.y + 32.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(npub_run);

        // Encryption indicator
        let enc_run = cx.text.layout(
            "\u{1F512} Encrypted",
            Point::new(
                header.origin.x + header.size.width - padding - 80.0,
                header.origin.y + 20.0,
            ),
            theme::font_size::XS,
            Hsla::new(120.0, 0.6, 0.45, 1.0),
        );
        cx.scene.draw_text(enc_run);

        // Messages area
        let messages_area = self.messages_bounds(&bounds);
        cx.scene.push_clip(messages_area);

        if self.messages.is_empty() {
            // Empty state
            let empty_y = messages_area.origin.y + messages_area.size.height / 2.0 - 20.0;
            let empty_run = cx.text.layout(
                "No messages yet",
                Point::new(
                    messages_area.origin.x + messages_area.size.width / 2.0 - 60.0,
                    empty_y,
                ),
                theme::font_size::SM,
                theme::text::MUTED,
            );
            cx.scene.draw_text(empty_run);

            let hint_run = cx.text.layout(
                "Send a message to start the conversation",
                Point::new(
                    messages_area.origin.x + messages_area.size.width / 2.0 - 130.0,
                    empty_y + 24.0,
                ),
                theme::font_size::XS,
                theme::text::DISABLED,
            );
            cx.scene.draw_text(hint_run);
        } else {
            // Render messages
            let mut y = messages_area.origin.y + padding - self.scroll_offset;
            for msg in &self.messages {
                let msg_height = 75.0;
                if y + msg_height > messages_area.origin.y
                    && y < messages_area.origin.y + messages_area.size.height
                {
                    let mut bubble = DmBubble::new(msg.clone());
                    bubble.paint(
                        Bounds::new(
                            messages_area.origin.x,
                            y,
                            messages_area.size.width,
                            msg_height,
                        ),
                        cx,
                    );
                }
                y += msg_height + 8.0;
            }
        }

        cx.scene.pop_clip();

        // Input area
        let input_area = self.input_bounds(&bounds);
        cx.scene
            .draw_quad(Quad::new(input_area).with_background(theme::bg::MUTED));

        // Input field
        let input_field = Bounds::new(
            input_area.origin.x + padding,
            input_area.origin.y + padding,
            input_area.size.width - padding * 3.0 - 60.0,
            36.0,
        );
        let field_bg = if self.input_focused {
            theme::bg::SURFACE
        } else {
            theme::bg::APP
        };
        let field_border = if self.input_focused {
            theme::accent::PRIMARY
        } else {
            theme::border::DEFAULT
        };
        cx.scene.draw_quad(
            Quad::new(input_field)
                .with_background(field_bg)
                .with_border(field_border, 1.0),
        );

        // Placeholder or text
        let input_display = if self.input_text.is_empty() {
            "Type a message..."
        } else {
            &self.input_text
        };
        let text_color = if self.input_text.is_empty() {
            theme::text::DISABLED
        } else {
            theme::text::PRIMARY
        };
        let input_run = cx.text.layout(
            input_display,
            Point::new(input_field.origin.x + 8.0, input_field.origin.y + 10.0),
            theme::font_size::SM,
            text_color,
        );
        cx.scene.draw_text(input_run);

        // Send button
        let send_bounds = self.send_button_bounds(&bounds);
        let send_bg = if self.send_hovered {
            theme::accent::PRIMARY.with_alpha(0.3)
        } else {
            theme::accent::PRIMARY.with_alpha(0.2)
        };
        cx.scene.draw_quad(
            Quad::new(send_bounds)
                .with_background(send_bg)
                .with_border(theme::accent::PRIMARY, 1.0),
        );
        let send_run = cx.text.layout(
            "Send",
            Point::new(send_bounds.origin.x + 10.0, send_bounds.origin.y + 10.0),
            theme::font_size::XS,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(send_run);
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        let send_bounds = self.send_button_bounds(&bounds);
        let input_field = Bounds::new(
            bounds.origin.x + 12.0,
            bounds.origin.y + bounds.size.height - 48.0,
            bounds.size.width - 96.0,
            36.0,
        );

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_send_hovered = self.send_hovered;

                self.send_hovered = send_bounds.contains(point);

                if was_send_hovered != self.send_hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left {
                    let point = Point::new(*x, *y);

                    if send_bounds.contains(point) && !self.input_text.is_empty() {
                        if let Some(callback) = &mut self.on_send {
                            let text = self.input_text.clone();
                            callback(text);
                            self.input_text.clear();
                        }
                        return EventResult::Handled;
                    }

                    self.input_focused = input_field.contains(point);
                    return EventResult::Handled;
                }
            }
            InputEvent::Scroll { dy, .. } => {
                let messages_area = self.messages_bounds(&bounds);
                let total_height = self.messages.len() as f32 * 83.0;
                let max_scroll = (total_height - messages_area.size.height).max(0.0);

                self.scroll_offset = (self.scroll_offset - dy).clamp(0.0, max_scroll);

                if let Some(callback) = &mut self.on_scroll {
                    callback(self.scroll_offset);
                }
                return EventResult::Handled;
            }
            _ => {}
        }
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(400.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::molecules::{DmDirection, EncryptionStatus};

    #[test]
    fn test_dm_thread() {
        let thread = DmThread::new("Alice", "npub1abc...").messages(vec![
            DmMessage::new("1", "Hello!", DmDirection::Incoming)
                .encryption(EncryptionStatus::Encrypted),
        ]);
        assert_eq!(thread.contact_name, "Alice");
        assert_eq!(thread.messages.len(), 1);
    }

    #[test]
    fn test_short_npub() {
        let thread = DmThread::new("Bob", "npub1qwertyuiopasdfghjklzxcvbnm123456789");
        assert!(thread.short_npub().contains("..."));
    }
}
