//! Progress bar component

use gpui::*;
use theme::ui::progress;

/// A progress bar showing completion percentage
///
/// # Example
/// ```
/// Progress::new().value(0.5)   // 50%
/// Progress::new().value(0.75).h(px(8.0))
/// ```
pub struct Progress {
    value: f32, // 0.0 to 1.0
    height: Pixels,
}

impl Progress {
    /// Create a new progress bar (default 0%)
    pub fn new() -> Self {
        Self {
            value: 0.0,
            height: px(8.0),
        }
    }

    /// Set the progress value (0.0 to 1.0)
    pub fn value(mut self, value: f32) -> Self {
        self.value = value.clamp(0.0, 1.0);
        self
    }

    /// Set the height
    pub fn h(mut self, height: Pixels) -> Self {
        self.height = height;
        self
    }
}

impl Default for Progress {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Progress {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let percent = (self.value * 100.0) as i32;

        div()
            .w_full()
            .h(self.height)
            .bg(progress::TRACK_BG)
            .rounded_full()
            .overflow_hidden()
            .child(
                div()
                    .h_full()
                    .w(relative(self.value))
                    .bg(progress::INDICATOR)
                    .rounded_full()
            )
    }
}

impl IntoElement for Progress {
    type Element = <Self as RenderOnce>::Element;

    fn into_element(self) -> Self::Element {
        self.render_once()
    }
}
