use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

pub struct CheckpointBadge {
    id: Option<ComponentId>,
    label: String,
    active: bool,
    hovered: bool,
    font_size: f32,
    on_click: Option<Box<dyn FnMut()>>,
}

impl CheckpointBadge {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: None,
            label: label.into(),
            active: false,
            hovered: false,
            font_size: theme::font_size::XS,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    fn color(&self) -> Hsla {
        if self.active {
            theme::accent::PRIMARY
        } else if self.hovered {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        }
    }
}

impl Default for CheckpointBadge {
    fn default() -> Self {
        Self::new("checkpoint")
    }
}

impl Component for CheckpointBadge {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;

        let icon = if self.active { "◆" } else { "◇" };
        let full_label = format!("{} {}", icon, self.label);
        let text_width = full_label.len() as f32 * self.font_size * 0.6;
        let badge_width = text_width + padding_h * 2.0;
        let badge_height = self.font_size + padding_v * 2.0;

        let badge_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - badge_height) / 2.0,
            badge_width,
            badge_height,
        );

        let bg_color = if self.active || self.hovered {
            self.color().with_alpha(0.15)
        } else {
            Hsla::transparent()
        };

        cx.scene.draw_quad(
            Quad::new(badge_bounds)
                .with_background(bg_color)
                .with_border(self.color(), 1.0),
        );

        let text_x = badge_bounds.origin.x + padding_h;
        let text_y = badge_bounds.origin.y + badge_height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout(
            &full_label,
            Point::new(text_x, text_y),
            self.font_size,
            self.color(),
        );
        cx.scene.draw_text(text_run);
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
            InputEvent::MouseDown { button, x, y, .. } => {
                if *button == MouseButton::Left && bounds.contains(Point::new(*x, *y)) {
                    if let Some(on_click) = &mut self.on_click {
                        on_click();
                    }
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
        let padding_h = theme::spacing::SM;
        let padding_v = theme::spacing::XS;
        let icon_len = 2;
        let text_width = (self.label.len() + icon_len) as f32 * self.font_size * 0.6;
        (
            Some(text_width + padding_h * 2.0),
            Some(self.font_size + padding_v * 2.0),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_badge_new() {
        let badge = CheckpointBadge::new("v1.0");
        assert_eq!(badge.label(), "v1.0");
        assert!(!badge.is_active());
    }

    #[test]
    fn test_checkpoint_badge_builder() {
        let badge = CheckpointBadge::new("save")
            .with_id(1)
            .active(true)
            .font_size(14.0);

        assert_eq!(badge.id, Some(1));
        assert!(badge.is_active());
        assert_eq!(badge.font_size, 14.0);
    }

    #[test]
    fn test_set_active() {
        let mut badge = CheckpointBadge::new("test");
        badge.set_active(true);
        assert!(badge.is_active());
    }
}
