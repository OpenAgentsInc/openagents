use std::time::Duration;

use crate::animation::AnimatorState;
use crate::components::context::{EventContext, PaintContext};
use crate::components::{Component, ComponentId, EventResult};
use crate::text::FontStyle;
use crate::tools::randomize_list;
use crate::{Bounds, Hsla, InputEvent, Point, theme};

use super::{TextEffectAnimator, TextEffectFrame, TextEffectTiming};

const DEFAULT_CHARACTERS: &str =
    "    ----____abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";

pub struct TextDecipher {
    id: Option<ComponentId>,
    text: String,
    font_size: f32,
    color: Hsla,
    style: FontStyle,
    characters: Vec<char>,
    scramble: ScrambleState,
    animator: TextEffectAnimator,
    frame: TextEffectFrame,
    state: AnimatorState,
    reveal_rank: Vec<usize>,
}

impl TextDecipher {
    pub fn new(text: impl Into<String>) -> Self {
        let text = text.into();
        let mut animator = TextEffectAnimator::new();
        let frame = animator.update_with_delta(
            AnimatorState::Entered,
            text.chars().count(),
            Duration::ZERO,
        );
        let mut decipher = Self {
            id: None,
            text,
            font_size: theme::font_size::SM,
            color: theme::text::PRIMARY,
            style: FontStyle::normal(),
            characters: DEFAULT_CHARACTERS.chars().collect(),
            scramble: ScrambleState::new(),
            animator,
            frame,
            state: AnimatorState::Entered,
            reveal_rank: Vec::new(),
        };
        decipher.rebuild_reveal_order();
        decipher
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

    pub fn characters(mut self, characters: impl AsRef<str>) -> Self {
        self.set_characters(characters);
        self
    }

    pub fn set_characters(&mut self, characters: impl AsRef<str>) {
        let chars = characters.as_ref();
        if chars.is_empty() {
            self.characters = DEFAULT_CHARACTERS.chars().collect();
        } else {
            self.characters = chars.chars().collect();
        }
    }

    pub fn seed(mut self, seed: u32) -> Self {
        self.set_seed(seed);
        self
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.scramble.seed = seed;
        self.rebuild_reveal_order();
    }

    pub fn scramble_interval(mut self, interval: Duration) -> Self {
        self.scramble.set_interval(interval);
        self
    }

    pub fn set_scramble_interval(&mut self, interval: Duration) {
        self.scramble.set_interval(interval);
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
        self.scramble.update(Duration::ZERO, state);
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
        self.scramble.update(delta, state);
        self.state = state;
        self.frame = frame;
        frame
    }

    pub fn update_with_delta(&mut self, state: AnimatorState, delta: Duration) -> TextEffectFrame {
        let frame = self
            .animator
            .update_with_delta(state, self.text_len(), delta);
        self.scramble.update(delta, state);
        self.state = state;
        self.frame = frame;
        frame
    }

    pub fn set_text(&mut self, text: impl Into<String>) {
        self.text = text.into();
        self.frame = self
            .animator
            .update_with_delta(self.state, self.text_len(), Duration::ZERO);
        self.rebuild_reveal_order();
    }

    pub fn text_value(&self) -> &str {
        &self.text
    }

    fn text_len(&self) -> usize {
        self.text.chars().count()
    }

    fn scramble_char(&self, index: usize) -> char {
        if self.characters.is_empty() {
            return '?';
        }
        let idx = self.scramble.scramble_index(index, self.characters.len());
        self.characters[idx]
    }

    fn build_display_text(&self, frame: &TextEffectFrame) -> String {
        if matches!(frame.state, AnimatorState::Exited) {
            return String::new();
        }

        let len = self.reveal_rank.len();
        let reveal_count = ((frame.progress() * len as f32).round() as usize).min(len);

        let mut display = String::with_capacity(self.text.len());
        for (index, ch) in self.text.chars().enumerate() {
            if ch.is_whitespace() {
                display.push(ch);
                continue;
            }

            if matches!(frame.state, AnimatorState::Entered)
                || (index < len && self.reveal_rank[index] < reveal_count)
            {
                display.push(ch);
            } else {
                display.push(self.scramble_char(index));
            }
        }
        display
    }

    fn rebuild_reveal_order(&mut self) {
        let length = self.text_len();
        if length == 0 {
            self.reveal_rank.clear();
            return;
        }

        let indices: Vec<usize> = (0..length).collect();
        let shuffled = randomize_list(&indices, Some(self.scramble.seed as u64));
        let mut rank = vec![0usize; length];
        for (pos, index) in shuffled.iter().enumerate() {
            if let Some(entry) = rank.get_mut(*index) {
                *entry = pos;
            }
        }
        self.reveal_rank = rank;
    }
}

impl Component for TextDecipher {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let text_origin = Point::new(bounds.origin.x, bounds.origin.y + self.font_size);
        let display = self.build_display_text(&self.frame);

        if display.is_empty() {
            return;
        }

        let text_run = cx.text.layout_styled(
            &display,
            text_origin,
            self.font_size,
            self.color,
            self.style,
        );
        cx.scene.draw_text(text_run);
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

struct ScrambleState {
    seed: u32,
    step: u32,
    interval: Duration,
    elapsed: Duration,
}

impl ScrambleState {
    fn new() -> Self {
        Self {
            seed: 12345,
            step: 0,
            interval: Duration::from_millis(50),
            elapsed: Duration::ZERO,
        }
    }

    fn set_interval(&mut self, interval: Duration) {
        self.interval = if interval.is_zero() {
            Duration::from_millis(1)
        } else {
            interval
        };
    }

    fn update(&mut self, delta: Duration, state: AnimatorState) {
        if matches!(state, AnimatorState::Entering | AnimatorState::Exiting) {
            self.elapsed += delta;
            while self.elapsed >= self.interval {
                self.elapsed -= self.interval;
                self.step = self.step.wrapping_add(1);
            }
        } else {
            self.elapsed = Duration::ZERO;
        }
    }

    fn scramble_index(&self, index: usize, len: usize) -> usize {
        if len == 0 {
            return 0;
        }

        let mut value = self.seed ^ (index as u32).wrapping_mul(0x9e3779b9);
        value = value.wrapping_add(self.step.wrapping_mul(0x7f4a7c15));
        value = value.wrapping_mul(1664525).wrapping_add(1013904223);
        (value as usize) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decipher_updates_text_and_frame_length() {
        let mut decipher = TextDecipher::new("Hello");
        assert_eq!(decipher.text_value(), "Hello");
        assert_eq!(decipher.frame().length, 5);

        decipher.set_text("World!");
        assert_eq!(decipher.text_value(), "World!");
        assert_eq!(decipher.frame().length, 6);

        decipher.set_state(AnimatorState::Entering);
        let frame = decipher.update_with_delta(AnimatorState::Entering, Duration::from_millis(100));
        assert!(frame.progress() >= 0.0 && frame.progress() <= 1.0);
    }

    #[test]
    fn test_decipher_custom_charset() {
        let mut decipher =
            TextDecipher::new("A B")
                .characters("01")
                .seed(42)
                .timing(TextEffectTiming::new(
                    Duration::from_secs(1),
                    Duration::ZERO,
                ));

        let frame = decipher.update_with_delta(AnimatorState::Entering, Duration::from_millis(400));
        let display = decipher.build_display_text(&frame);
        let chars: Vec<char> = display.chars().collect();

        assert_eq!(chars.len(), 3);
        assert_eq!(chars[1], ' ');
        assert!(
            chars[0] == 'A' || chars[0] == '0' || chars[0] == '1',
            "unexpected char: {}",
            chars[0]
        );
        assert!(
            chars[2] == 'B' || chars[2] == '0' || chars[2] == '1',
            "unexpected char: {}",
            chars[2]
        );
    }
}
