//! HUD background with DotsGrid effect

use std::time::Instant;
use wgpui::{
    Bounds, Component, Easing, EventContext, EventResult, Hsla, InputEvent, PaintContext,
    components::hud::{DotShape, DotsGrid, DotsOrigin},
};

/// Background layer with subtle DotsGrid
pub struct HudBackground {
    start_time: Instant,
    animation_complete: bool,
    color: Hsla,
}

impl HudBackground {
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            animation_complete: false,
            color: Hsla::new(0.0, 0.0, 0.25, 0.2), // Subtle gray
        }
    }

    #[allow(dead_code)]
    pub fn dots_color(mut self, color: Hsla) -> Self {
        self.color = color;
        self
    }

    fn ease_out_cubic(t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        1.0 - (1.0 - t).powi(3)
    }
}

impl Default for HudBackground {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for HudBackground {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Calculate animation progress
        let progress = if self.animation_complete {
            1.0
        } else {
            let elapsed = self.start_time.elapsed().as_secs_f32();
            let p = Self::ease_out_cubic((elapsed / 1.5).min(1.0));
            if p >= 1.0 {
                self.animation_complete = true;
            }
            p
        };

        // Create and paint dots grid
        let mut dots = DotsGrid::new()
            .color(self.color)
            .shape(DotShape::Circle)
            .distance(48.0)
            .size(2.0)
            .origin(DotsOrigin::Center)
            .easing(Easing::EaseOut)
            .animation_progress(progress);

        dots.paint(bounds, cx);
    }

    fn event(&mut self, _: &InputEvent, _: Bounds, _: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}
