use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PermissionAction {
    #[default]
    Allow,
    Deny,
    AllowOnce,
    AllowAlways,
}

impl PermissionAction {
    fn label(&self) -> &'static str {
        match self {
            PermissionAction::Allow => "Allow",
            PermissionAction::Deny => "Deny",
            PermissionAction::AllowOnce => "Allow once",
            PermissionAction::AllowAlways => "Allow always",
        }
    }

    fn color(&self) -> Hsla {
        match self {
            PermissionAction::Allow
            | PermissionAction::AllowOnce
            | PermissionAction::AllowAlways => theme::status::SUCCESS,
            PermissionAction::Deny => theme::status::ERROR,
        }
    }
}

pub struct PermissionButton {
    id: Option<ComponentId>,
    action: PermissionAction,
    hovered: bool,
    pressed: bool,
    font_size: f32,
    on_click: Option<Box<dyn FnMut(PermissionAction)>>,
}

impl PermissionButton {
    pub fn new(action: PermissionAction) -> Self {
        Self {
            id: None,
            action,
            hovered: false,
            pressed: false,
            font_size: theme::font_size::SM,
            on_click: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(PermissionAction) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn action(&self) -> PermissionAction {
        self.action
    }
}

impl Default for PermissionButton {
    fn default() -> Self {
        Self::new(PermissionAction::default())
    }
}

impl Component for PermissionButton {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let padding_h = theme::spacing::MD;
        let padding_v = theme::spacing::SM;

        let label = self.action.label();
        let text_width = label.len() as f32 * self.font_size * 0.6;
        let btn_width = text_width + padding_h * 2.0;
        let btn_height = self.font_size + padding_v * 2.0;

        let btn_bounds = Bounds::new(
            bounds.origin.x,
            bounds.origin.y + (bounds.size.height - btn_height) / 2.0,
            btn_width,
            btn_height,
        );

        let bg_color = if self.pressed {
            self.action.color().darken(0.1)
        } else if self.hovered {
            self.action.color().lighten(0.1)
        } else {
            self.action.color()
        };

        cx.scene
            .draw_quad(Quad::new(btn_bounds).with_background(bg_color));

        let text_x = btn_bounds.origin.x + padding_h;
        let text_y = btn_bounds.origin.y + btn_height * 0.5 - self.font_size * 0.55;

        let text_run = cx.text.layout_mono(
            label,
            Point::new(text_x, text_y),
            self.font_size,
            theme::text::PRIMARY,
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
                    self.pressed = true;
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { button, x, y } => {
                if *button == MouseButton::Left && self.pressed {
                    self.pressed = false;
                    if bounds.contains(Point::new(*x, *y))
                        && let Some(on_click) = &mut self.on_click
                    {
                        on_click(self.action);
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
        let padding_h = theme::spacing::MD;
        let padding_v = theme::spacing::SM;
        let label = self.action.label();
        let text_width = label.len() as f32 * self.font_size * 0.6;
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
    fn test_permission_button_new() {
        let btn = PermissionButton::new(PermissionAction::Allow);
        assert_eq!(btn.action(), PermissionAction::Allow);
    }

    #[test]
    fn test_action_labels() {
        assert_eq!(PermissionAction::Allow.label(), "Allow");
        assert_eq!(PermissionAction::Deny.label(), "Deny");
        assert_eq!(PermissionAction::AllowAlways.label(), "Allow always");
    }

    #[test]
    fn test_action_colors() {
        assert_eq!(PermissionAction::Allow.color(), theme::status::SUCCESS);
        assert_eq!(PermissionAction::Deny.color(), theme::status::ERROR);
    }
}
