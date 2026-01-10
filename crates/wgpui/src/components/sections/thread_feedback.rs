use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult, TextInput};
use crate::{Bounds, InputEvent, Point, Quad, theme};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FeedbackRating {
    #[default]
    None,
    Positive,
    Negative,
}

pub struct ThreadFeedback {
    id: Option<ComponentId>,
    rating: FeedbackRating,
    comment: String,
    comment_input: TextInput,
    show_comment_input: bool,
    submitted: bool,
    positive_hovered: bool,
    negative_hovered: bool,
    submit_hovered: bool,
    on_submit: Option<Box<dyn FnMut(FeedbackRating, String)>>,
    on_dismiss: Option<Box<dyn FnMut()>>,
}

impl ThreadFeedback {
    pub fn new() -> Self {
        Self {
            id: None,
            rating: FeedbackRating::None,
            comment: String::new(),
            comment_input: TextInput::new()
                .placeholder("Add a comment (optional)...")
                .background(theme::bg::SURFACE),
            show_comment_input: false,
            submitted: false,
            positive_hovered: false,
            negative_hovered: false,
            submit_hovered: false,
            on_submit: None,
            on_dismiss: None,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn on_submit<F>(mut self, f: F) -> Self
    where
        F: FnMut(FeedbackRating, String) + 'static,
    {
        self.on_submit = Some(Box::new(f));
        self
    }

    pub fn on_dismiss<F>(mut self, f: F) -> Self
    where
        F: FnMut() + 'static,
    {
        self.on_dismiss = Some(Box::new(f));
        self
    }

    pub fn rating(&self) -> FeedbackRating {
        self.rating
    }

    pub fn is_submitted(&self) -> bool {
        self.submitted
    }

    pub fn reset(&mut self) {
        self.rating = FeedbackRating::None;
        self.comment.clear();
        self.comment_input.set_value("");
        self.show_comment_input = false;
        self.submitted = false;
    }

    fn positive_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_size = 32.0;
        let center_x = bounds.origin.x + bounds.size.width / 2.0;
        let y = bounds.origin.y + theme::spacing::MD;

        Bounds::new(
            center_x - btn_size - theme::spacing::SM,
            y,
            btn_size,
            btn_size,
        )
    }

    fn negative_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_size = 32.0;
        let center_x = bounds.origin.x + bounds.size.width / 2.0;
        let y = bounds.origin.y + theme::spacing::MD;

        Bounds::new(center_x + theme::spacing::SM, y, btn_size, btn_size)
    }

    fn submit_bounds(&self, bounds: &Bounds) -> Bounds {
        let btn_width = 80.0;
        let btn_height = 32.0;
        let y = bounds.origin.y + bounds.size.height - theme::spacing::MD - btn_height;

        Bounds::new(
            bounds.origin.x + (bounds.size.width - btn_width) / 2.0,
            y,
            btn_width,
            btn_height,
        )
    }
}

impl Default for ThreadFeedback {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ThreadFeedback {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        cx.scene.draw_quad(
            Quad::new(bounds)
                .with_background(theme::bg::SURFACE)
                .with_border(theme::border::DEFAULT, 1.0),
        );

        if self.submitted {
            let thank_you = "Thank you for your feedback!";
            let font_size = theme::font_size::SM;
            let text_width = thank_you.len() as f32 * font_size * 0.6;

            let thank_run = cx.text.layout_mono(
                thank_you,
                Point::new(
                    bounds.origin.x + (bounds.size.width - text_width) / 2.0,
                    bounds.origin.y + bounds.size.height / 2.0 - font_size / 2.0,
                ),
                font_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(thank_run);
            return;
        }

        let title = "Was this response helpful?";
        let title_size = theme::font_size::SM;
        let title_width = title.len() as f32 * title_size * 0.5;

        let title_run = cx.text.layout_mono(
            title,
            Point::new(
                bounds.origin.x + (bounds.size.width - title_width) / 2.0,
                bounds.origin.y + theme::spacing::SM,
            ),
            title_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(title_run);

        let pos_bounds = self.positive_bounds(&bounds);
        let neg_bounds = self.negative_bounds(&bounds);

        let pos_bg = if self.rating == FeedbackRating::Positive {
            theme::status::SUCCESS.with_alpha(0.3)
        } else if self.positive_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::MUTED
        };

        let neg_bg = if self.rating == FeedbackRating::Negative {
            theme::status::ERROR.with_alpha(0.3)
        } else if self.negative_hovered {
            theme::bg::HOVER
        } else {
            theme::bg::MUTED
        };

        cx.scene
            .draw_quad(Quad::new(pos_bounds).with_background(pos_bg));
        cx.scene
            .draw_quad(Quad::new(neg_bounds).with_background(neg_bg));

        let thumb_up = "\u{1F44D}";
        let thumb_down = "\u{1F44E}";
        let emoji_size = theme::font_size::LG;

        let up_run = cx.text.layout_mono(
            thumb_up,
            Point::new(
                pos_bounds.origin.x + (pos_bounds.size.width - emoji_size * 0.8) / 2.0,
                pos_bounds.origin.y + (pos_bounds.size.height - emoji_size) / 2.0,
            ),
            emoji_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(up_run);

        let down_run = cx.text.layout_mono(
            thumb_down,
            Point::new(
                neg_bounds.origin.x + (neg_bounds.size.width - emoji_size * 0.8) / 2.0,
                neg_bounds.origin.y + (neg_bounds.size.height - emoji_size) / 2.0,
            ),
            emoji_size,
            theme::text::PRIMARY,
        );
        cx.scene.draw_text(down_run);

        if self.show_comment_input && self.rating != FeedbackRating::None {
            let input_y = pos_bounds.origin.y + pos_bounds.size.height + theme::spacing::MD;
            let input_bounds = Bounds::new(
                bounds.origin.x + theme::spacing::MD,
                input_y,
                bounds.size.width - theme::spacing::MD * 2.0,
                36.0,
            );
            self.comment_input.paint(input_bounds, cx);

            let submit_bounds = self.submit_bounds(&bounds);
            let submit_bg = if self.submit_hovered {
                theme::accent::PRIMARY.lighten(0.1)
            } else {
                theme::accent::PRIMARY
            };

            cx.scene
                .draw_quad(Quad::new(submit_bounds).with_background(submit_bg));

            let submit_text = "Submit";
            let submit_size = theme::font_size::SM;
            let submit_run = cx.text.layout_mono(
                submit_text,
                Point::new(
                    submit_bounds.origin.x
                        + (submit_bounds.size.width - submit_text.len() as f32 * submit_size * 0.5)
                            / 2.0,
                    submit_bounds.origin.y + (submit_bounds.size.height - submit_size) / 2.0,
                ),
                submit_size,
                theme::text::PRIMARY,
            );
            cx.scene.draw_text(submit_run);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        if self.submitted {
            return EventResult::Ignored;
        }

        match event {
            InputEvent::MouseMove { x, y } => {
                let point = Point::new(*x, *y);
                let was_pos = self.positive_hovered;
                let was_neg = self.negative_hovered;
                let was_submit = self.submit_hovered;

                self.positive_hovered = self.positive_bounds(&bounds).contains(point);
                self.negative_hovered = self.negative_bounds(&bounds).contains(point);
                self.submit_hovered =
                    self.show_comment_input && self.submit_bounds(&bounds).contains(point);

                if was_pos != self.positive_hovered
                    || was_neg != self.negative_hovered
                    || was_submit != self.submit_hovered
                {
                    return EventResult::Handled;
                }
            }
            InputEvent::MouseUp { x, y, .. } => {
                let point = Point::new(*x, *y);

                if self.positive_bounds(&bounds).contains(point) {
                    self.rating = FeedbackRating::Positive;
                    self.show_comment_input = true;
                    return EventResult::Handled;
                }

                if self.negative_bounds(&bounds).contains(point) {
                    self.rating = FeedbackRating::Negative;
                    self.show_comment_input = true;
                    return EventResult::Handled;
                }

                if self.show_comment_input && self.submit_bounds(&bounds).contains(point) {
                    self.comment = self.comment_input.get_value().to_string();
                    self.submitted = true;
                    if let Some(callback) = &mut self.on_submit {
                        callback(self.rating, self.comment.clone());
                    }
                    return EventResult::Handled;
                }
            }
            _ => {}
        }

        if self.show_comment_input && self.rating != FeedbackRating::None {
            let pos_bounds = self.positive_bounds(&bounds);
            let input_y = pos_bounds.origin.y + pos_bounds.size.height + theme::spacing::MD;
            let input_bounds = Bounds::new(
                bounds.origin.x + theme::spacing::MD,
                input_y,
                bounds.size.width - theme::spacing::MD * 2.0,
                36.0,
            );

            let result = self.comment_input.event(event, input_bounds, cx);
            if result == EventResult::Handled {
                return result;
            }
        }

        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let height = if self.show_comment_input { 160.0 } else { 80.0 };
        (None, Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thread_feedback_new() {
        let feedback = ThreadFeedback::new();
        assert_eq!(feedback.rating(), FeedbackRating::None);
        assert!(!feedback.is_submitted());
    }

    #[test]
    fn test_thread_feedback_reset() {
        let mut feedback = ThreadFeedback::new();
        feedback.rating = FeedbackRating::Positive;
        feedback.submitted = true;

        feedback.reset();

        assert_eq!(feedback.rating(), FeedbackRating::None);
        assert!(!feedback.is_submitted());
    }

    #[test]
    fn test_feedback_rating_default() {
        let rating = FeedbackRating::default();
        assert_eq!(rating, FeedbackRating::None);
    }

    #[test]
    fn test_thread_feedback_builder() {
        let feedback = ThreadFeedback::new().with_id(1);
        assert_eq!(feedback.id, Some(1));
    }

    #[test]
    fn test_thread_feedback_size_hint() {
        let feedback = ThreadFeedback::new();
        let (w, h) = feedback.size_hint();
        assert!(w.is_none());
        assert_eq!(h, Some(80.0));
    }

    #[test]
    fn test_thread_feedback_expanded_size() {
        let mut feedback = ThreadFeedback::new();
        feedback.show_comment_input = true;

        let (_, h) = feedback.size_hint();
        assert_eq!(h, Some(160.0));
    }
}
