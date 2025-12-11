//! Slider component for range selection

use gpui::prelude::*;
use gpui::*;
use theme_oa::ui::progress;

/// A slider for selecting a value within a range
///
/// Note: This is a visual-only component. Full interactivity requires
/// mouse tracking which would need a stateful Render implementation.
///
/// # Example
/// ```
/// Slider::new().value(0.5)
/// Slider::new().value(0.75).disabled(true)
/// ```
#[derive(IntoElement)]
pub struct Slider {
    value: f32,       // 0.0 to 1.0
    min: f32,
    max: f32,
    step: Option<f32>,
    disabled: bool,
}

impl Slider {
    /// Create a new slider
    pub fn new() -> Self {
        Self {
            value: 0.0,
            min: 0.0,
            max: 1.0,
            step: None,
            disabled: false,
        }
    }

    /// Set the current value (0.0 to 1.0 by default)
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(self.min, self.max);
        self
    }

    /// Set the minimum value
    pub fn min(mut self, min: f32) -> Self {
        self.min = min;
        self
    }

    /// Set the maximum value
    pub fn max(mut self, max: f32) -> Self {
        self.max = max;
        self
    }

    /// Set the step increment
    pub fn step(mut self, step: f32) -> Self {
        self.step = Some(step);
        self
    }

    /// Set disabled state
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Default for Slider {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Slider {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Calculate percentage for positioning
        let range = self.max - self.min;
        let percent = if range > 0.0 {
            ((self.value - self.min) / range).clamp(0.0, 1.0)
        } else {
            0.0
        };

        let track_width = 200.0; // Default width
        let thumb_size = 16.0;
        let track_height = 6.0;

        // Track background - reuse progress bar colors
        let track = div()
            .w(px(track_width))
            .h(px(track_height))
            .rounded_full()
            .bg(progress::TRACK_BG)
            .relative()
            // Filled range
            .child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .h_full()
                    .w(px(track_width * percent))
                    .rounded_full()
                    .bg(progress::INDICATOR)
            );

        // Thumb - same color as indicator
        let thumb_left = (track_width - thumb_size) * percent;
        let thumb = div()
            .absolute()
            .top(px(-((thumb_size - track_height) / 2.0)))
            .left(px(thumb_left))
            .w(px(thumb_size))
            .h(px(thumb_size))
            .rounded_full()
            .bg(progress::INDICATOR)
            .border_2()
            .border_color(progress::INDICATOR);

        let mut container = div()
            .relative()
            .w(px(track_width))
            .h(px(thumb_size))
            .flex()
            .items_center()
            .child(track)
            .child(thumb);

        if self.disabled {
            container = container.opacity(0.5).cursor_not_allowed();
        } else {
            container = container.cursor_pointer();
        }

        container
    }
}
