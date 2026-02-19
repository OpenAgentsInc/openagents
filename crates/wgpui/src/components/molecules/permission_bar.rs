use crate::components::atoms::{PermissionAction, PermissionButton};
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, InputEvent, Point, Quad, theme};

pub struct PermissionBar {
    id: Option<ComponentId>,
    message: String,
    actions: Vec<PermissionAction>,
    on_action: Option<Box<dyn FnMut(PermissionAction)>>,
}

impl PermissionBar {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            id: None,
            message: message.into(),
            actions: vec![PermissionAction::Allow, PermissionAction::Deny],
            on_action: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn actions(mut self, actions: Vec<PermissionAction>) -> Self {
        self.actions = actions;
        self
    }

    pub fn on_action<F>(mut self, f: F) -> Self
    where
        F: FnMut(PermissionAction) + 'static,
    {
        self.on_action = Some(Box::new(f));
        self
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Default for PermissionBar {
    fn default() -> Self {
        Self::new("Permission required")
    }
}

impl Component for PermissionBar {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::status::WARNING.with_alpha(0.5), 1.0),
        );

        let padding = theme::spacing::MD;
        let font_size = theme::font_size::SM;
        let text_y = bounds.origin.y + bounds.size.height * 0.5 - font_size * 0.55;

        let text_run = cx.text.layout_mono(
            &self.message,
            Point::new(bounds.origin.x + padding, text_y),
            font_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(text_run);

        let mut x = bounds.origin.x + bounds.size.width - padding;
        for action in self.actions.iter().rev() {
            let btn = PermissionButton::new(*action);
            let (btn_w, _) = btn.size_hint();
            let btn_width = btn_w.unwrap_or(80.0);
            x -= btn_width;

            let mut btn = PermissionButton::new(*action);
            btn.paint(
                Bounds::new(
                    x,
                    bounds.origin.y + 4.0,
                    btn_width,
                    bounds.size.height - 8.0,
                ),
                cx,
            );
            x -= theme::spacing::SM;
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        let padding = theme::spacing::MD;
        let mut x = bounds.origin.x + bounds.size.width - padding;
        let mut clicked_action: Option<PermissionAction> = None;

        for action in self.actions.iter().rev() {
            let btn = PermissionButton::new(*action);
            let (btn_w, _) = btn.size_hint();
            let btn_width = btn_w.unwrap_or(80.0);
            x -= btn_width;

            let btn_bounds = Bounds::new(
                x,
                bounds.origin.y + 4.0,
                btn_width,
                bounds.size.height - 8.0,
            );

            let mut btn = PermissionButton::new(*action);
            let result = btn.event(event, btn_bounds, cx);
            if result == EventResult::Handled {
                clicked_action = Some(*action);
                break;
            }

            x -= theme::spacing::SM;
        }

        if let Some(action) = clicked_action {
            if let Some(callback) = &mut self.on_action {
                callback(action);
            }
            return EventResult::Handled;
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        (None, Some(40.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_permission_bar_new() {
        let bar = PermissionBar::new("Allow file access?");
        assert_eq!(bar.message(), "Allow file access?");
    }

    #[test]
    fn test_permission_bar_builder() {
        let bar = PermissionBar::new("Permission")
            .with_id(1)
            .actions(vec![PermissionAction::AllowOnce, PermissionAction::Deny]);

        assert_eq!(bar.id, Some(1));
        assert_eq!(bar.actions.len(), 2);
    }
}
