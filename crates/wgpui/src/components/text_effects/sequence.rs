use std::time::Duration;

use crate::animation::AnimatorState;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::text::FontStyle;
use crate::{Bounds, Hsla, InputEvent, Point, theme};

use super::{CursorBlink, TextEffectAnimator, TextEffectFrame, TextEffectTiming};

pub struct TextSequence {
    id: Option<ComponentId>,
    text: String,
    font_size: f32,
    color: Hsla,
    style: FontStyle,
    show_cursor: bool,
    cursor_char: char,
    cursor_blink: CursorBlink,
    animator: TextEffectAnimator,
    frame: TextEffectFrame,
    state: AnimatorState,
}

impl TextSequence {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut animator = TextEffectAnimator::new();
        let frame = animator.update_with_delta(
            AnimatorState::Entered,
            text.chars().count(),
            Duration::ZERO,
        );
        Self {
            id: None,
            text,
            font_size: theme::font_size::SM,
            color: theme::text::PRIMARY,
            style: FontStyle::normal(),
            show_cursor: true,
            cursor_char: '|',
            cursor_blink: CursorBlink::new(),
            animator,
            frame,
            state: AnimatorState::Entered,
        }
    }

    pub fn with_id(mut self, id: ComponentId) -> Self {
        self.id = Some(id);
        self
    }

    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.set_text(text);
        self
    }

    pub fn font_size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }

    pub fn color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    pub fn bold(mut self) -> Self {
        self.style = FontStyle::bold();
        self
    }

    pub fn italic(mut self) -> Self {
        self.style = FontStyle::italic();
        self
    }

    pub fn bold_italic(mut self) -> Self {
        self.style = FontStyle::bold_italic();
        self
    }

    pub fn style(mut self, style: FontStyle) -> Self {
        self.style = style;
        self
    }

    pub fn show_cursor(mut self, show: bool) -> Self {
        self.show_cursor = show;
        self
    }

    pub fn cursor_char(mut self, cursor_char: char) -> Self {
        self.cursor_char = cursor_char;
        self
    }

    pub fn cursor_blink_period(mut self, period: Duration) -> Self {
        self.cursor_blink.set_period(period);
        self
    }

    pub fn timing(mut self, timing: TextEffectTiming) -> Self {
        self.animator.set_timing(timing);
        self.frame = self
            .animator
            .update_with_delta(self.state, self.text_len(), Duration::ZERO);
        self
    }

    pub fn set_timing(&mut self, timing: TextEffectTiming) {
        self.animator.set_timing(timing);
        self.frame = self
            .animator
            .update_with_delta(self.state, self.text_len(), Duration::ZERO);
    }

    pub fn easing(mut self, easing: crate::animation::Easing) -> Self {
        self.animator.set_easing(easing);
        self
    }

    pub fn set_easing(&mut self, easing: crate::animation::Easing) {
        self.animator.set_easing(easing);
    }

    pub fn state(&self) -> AnimatorState {
        self.state
    }

    pub fn set_state(&mut self, state: AnimatorState) {
        self.frame = self
            .animator
            .update_with_delta(state, self.text_len(), Duration::ZERO);
        self.cursor_blink
            .update(Duration::ZERO, state, self.show_cursor);
        self.state = state;
    }

    pub fn frame(&self) -> TextEffectFrame {
        self.frame
    }

    pub fn progress(&self) -> f32 {
        self.frame.progress()
    }

    pub fn update(&mut self, state: AnimatorState) -> TextEffectFrame {
        let frame = self.animator.update(state, self.text_len());
        let delta = self.animator.last_delta();
        self.cursor_blink.update(delta, state, self.show_cursor);
        self.state = state;
        self.frame = frame;
        frame
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> TextEffectFrame {
        let frame = self
            .animator
            .update_with_delta(state, self.text_len(), delta);
        self.cursor_blink.update(delta, state, self.show_cursor);
        self.state = state;
        self.frame = frame;
        frame
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.frame = self
            .animator
            .update_with_delta(self.state, self.text_len(), Duration::ZERO);
    }

    pub fn text_value(&self) -> &str {
        &self.text
    }

    fn text_len(&self) -> usize {
        self.text.chars().count()
    }

    fn build_visible_text(&self, frame: &TextEffectFrame) -> String {
        match frame.state {
            AnimatorState::Entered => return self.text.clone(),
            AnimatorState::Exited => return String::new(),
            _ => {}
        }

        let mut visible = String::with_capacity(self.text.len());
        for (index, ch) in self.text.chars().enumerate() {
            if frame.is_visible(index) {
                visible.push(ch);
            } else {
                break;
            }
        }
        visible
    }
}

impl Component for TextSequence {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let text_origin = Point::new(bounds.origin.x, bounds.origin.y + self.font_size);
        let visible_text = self.build_visible_text(&self.frame);

        if !visible_text.is_empty() {
            let text_run = cx.text.layout_styled(
                &visible_text,
                text_origin,
                self.font_size,
                self.color,
                self.style,
            );
            cx.scene.draw_text(text_run);
        }

        if self.show_cursor && self.cursor_blink.visible() {
            let cursor_x = text_origin.x + cx.text.measure(&visible_text, self.font_size);
            let cursor_origin = Point::new(cursor_x, text_origin.y);
            let cursor_color = self.color.with_alpha(self.color.a * 0.8);
            let cursor_text = self.cursor_char.to_string();
            let cursor_run = cx.text.layout_styled(
                &cursor_text,
                cursor_origin,
                self.font_size,
                cursor_color,
                self.style,
            );
            cx.scene.draw_text(cursor_run);
        }
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }

    fn size_hint(&self) -> (Option<f32>, Option<f32>) {
        let char_count = self.text_len() as f32;
        let width = char_count * self.font_size * 0.6;
        let height = self.font_size * 1.4;
        (Some(width), Some(height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_reveal_with_delay() {
        let mut seq = TextSequence::new("ABC").timing(TextEffectTiming::new(
            Duration::from_secs(2),
            Duration::from_secs(1),
        ));

        let frame = seq.update_with_delta(AnimatorState::Entering, Duration::from_secs(1));
        assert_eq!(seq.build_visible_text(&frame), "A");

        let frame = seq.update_with_delta(AnimatorState::Entering, Duration::from_secs(2));
        assert_eq!(seq.build_visible_text(&frame), "AB");
    }
}
