use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::{Bounds, Hsla, InputEvent, MouseButton, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeedbackType {
    ThumbsUp,
    ThumbsDown,
}

pub struct FeedbackButton {
    id: Option<ComponentId>,
    feedback_type: FeedbackType,
    selected: bool,
    hovered: bool,
    size: f32,
    on_click: Option<Box<dyn FnMut(FeedbackType)>>,
}

impl FeedbackButton {
    pub fn new(feedback_type: FeedbackType) -> Self {
        Self {
            id: None,
            feedback_type,
            selected: false,
            hovered: false,
            size: 24.0,
            on_click: None,
        }
    }

    pub fn thumbs_up() -> Self {
        Self::new(FeedbackType::ThumbsUp)
    }

    pub fn thumbs_down() -> Self {
        Self::new(FeedbackType::ThumbsDown)
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }

    pub fn on_click<F>(mut self, f: F) -> Self
    where
        F: FnMut(FeedbackType) + 'static,
    {
        self.on_click = Some(Box::new(f));
        self
    }

    pub fn is_selected(&self) -> bool {
        self.selected
    }

    pub fn set_selected(&mut self, selected: bool) {
        self.selected = selected;
    }

    pub fn feedback_type(&self) -> FeedbackType {
        self.feedback_type
    }

    fn icon(&self) -> &'static str {
        match self.feedback_type {
            FeedbackType::ThumbsUp => "+",
            FeedbackType::ThumbsDown => "-",
        }
    }

    fn color(&self) -> Hsla {
        if self.selected {
            match self.feedback_type {
                FeedbackType::ThumbsUp => theme::status::SUCCESS,
                FeedbackType::ThumbsDown => theme::status::ERROR,
            }
        } else if self.hovered {
            theme::text::PRIMARY
        } else {
            theme::text::MUTED
        }
    }
}

impl Component for FeedbackButton {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let btn_bounds = Bounds::new(
            bounds.origin.x + (bounds.size.width - self.size) / 2.0,
            bounds.origin.y + (bounds.size.height - self.size) / 2.0,
            self.size,
            self.size,
        );

        let bg_color = if self.selected || self.hovered {
            self.color().with_alpha(0.2)
        } else {
            Hsla::transparent()
        };

        cx.scene.draw_quad(
            Quad::new(btn_bounds)
                .with_background(bg_color)
                .with_border(self.color(), 1.0),
        );

        let font_size = self.size * 0.6;
        let text_x = btn_bounds.origin.x + (self.size - font_size * 0.6) / 2.0;
        let text_y = btn_bounds.origin.y + self.size * 0.5 - font_size * 0.55;

        let text_run = cx.text.layout(
            self.icon(),
            Point::new(text_x, text_y),
            font_size,
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
                    self.selected = !self.selected;
                    if let Some(on_click) = &mut self.on_click {
                        on_click(self.feedback_type);
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
        (Some(self.size), Some(self.size))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feedback_button_new() {
        let btn = FeedbackButton::new(FeedbackType::ThumbsUp);
        assert_eq!(btn.feedback_type(), FeedbackType::ThumbsUp);
        assert!(!btn.is_selected());
    }

    #[test]
    fn test_thumbs_up_down() {
        let up = FeedbackButton::thumbs_up();
        let down = FeedbackButton::thumbs_down();
        assert_eq!(up.feedback_type(), FeedbackType::ThumbsUp);
        assert_eq!(down.feedback_type(), FeedbackType::ThumbsDown);
    }

    #[test]
    fn test_selected() {
        let mut btn = FeedbackButton::thumbs_up().selected(true);
        assert!(btn.is_selected());
        btn.set_selected(false);
        assert!(!btn.is_selected());
    }

    #[test]
    fn test_icons() {
        let up = FeedbackButton::thumbs_up();
        let down = FeedbackButton::thumbs_down();
        assert_eq!(up.icon(), "+");
        assert_eq!(down.icon(), "-");
    }
}
