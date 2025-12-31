use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, MouseButton, Point, Quad, theme};

pub struct ThinkingToggle {
    id: Option<ComponentId>,
    expanded: bool,
    hovered: bool,
    label_collapsed: String,
    label_expanded: String,
    font_size: f32,
    on_toggle: Option<Box<dyn FnMut(bool)>>,
}

impl ThinkingToggle {
    pub fn new() -> Self {
        Self {
            id: None,
            expanded: false,
            hovered: false,
            label_collapsed: "Show thinking".to_string(),
            label_expanded: "Hide thinking".to_string(),
            font_size: theme::font_size::XS,
            on_toggle: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn expanded(mut self, expanded: bool) -> Self {
        self.expanded = expanded;
        self
    }

    pub fn labels(mut self, collapsed: impl Into<String>, expanded: impl Into<String>) -> Self {
        self.label_collapsed = collapsed.into();
        self.label_expanded = expanded.into();
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn on_toggle<F>(mut self, f: F) -> Self
    where
        F: FnMut(bool) + 'static,
    {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
        if let Some(on_toggle) = &mut self.on_toggle {
            on_toggle(self.expanded);
        }
    }

    fn current_label(&self) -> &str {
        if self.expanded {
            &self.label_expanded
        } else {
            &self.label_collapsed
        }
    }
}

impl Default for ThinkingToggle {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ThinkingToggle {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let label = self.current_label();
        let text_color = if self.hovered {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        };

        let arrow = if self.expanded { "v" } else { ">" };
        let full_label = format!("{} {}", arrow, label);

        let text_y = bounds.origin.y + bounds.size.height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout(
            &full_label,
            Point::new(bounds.origin.x, text_y),
            self.font_size,
            text_color,
        );
        cx.scene.draw_text(text_run);

        if self.hovered {
            let underline_y = text_y + self.font_size + 2.0;
            let text_width = full_label.len() as f32 * self.font_size * 0.6;
            cx.scene.draw_quad(
                Quad::new(Bounds::new(bounds.origin.x, underline_y, text_width, 1.0))
                    .with_background(text_color),
            );
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y } => {
                let was_hovered = self.hovered;
                self.hovered = bounds.contains(Point::new(*x, *y));
                if was_hovered != self.hovered {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseDown { button, x, y } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    self.toggle();
                    return EventResult::Handled;
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
        let label = self.current_label();
        let text_width = (label.len() + 2) as f32 * self.font_size * 0.6;
        (Some(text_width), Some(self.font_size * 1.5))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_toggle_new() {
        let toggle = ThinkingToggle::new();
        assert!(!toggle.is_expanded());
    }

    #[test]
    fn test_thinking_toggle_builder() {
        let toggle = ThinkingToggle::new()
            .with_id(1)
            .expanded(true)
            .labels("Show", "Hide");

        assert_eq!(toggle.id, Some(1));
        assert!(toggle.is_expanded());
    }

    #[test]
    fn test_toggle() {
        let mut toggle = ThinkingToggle::new();
        assert!(!toggle.is_expanded());
        toggle.toggle();
        assert!(toggle.is_expanded());
        toggle.toggle();
        assert!(!toggle.is_expanded());
    }

    #[test]
    fn test_current_label() {
        let mut toggle = ThinkingToggle::new().labels("Collapsed", "Expanded");

        assert_eq!(toggle.current_label(), "Collapsed");
        toggle.set_expanded(true);
        assert_eq!(toggle.current_label(), "Expanded");
    }
}
