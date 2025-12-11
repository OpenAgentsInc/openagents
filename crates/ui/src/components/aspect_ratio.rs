//! AspectRatio component for maintaining proportional dimensions

use gpui::prelude::*;
use gpui::*;

/// Common aspect ratio presets
#[derive(Clone, Copy)]
pub enum AspectRatioPreset {
    /// 1:1 square
    Square,
    /// 16:9 widescreen
    Widescreen,
    /// 4:3 standard
    Standard,
    /// 21:9 ultrawide
    Ultrawide,
    /// 3:2 photo
    Photo,
    /// Custom ratio (width/height)
    Custom(f32),
}

impl AspectRatioPreset {
    fn ratio(&self) -> f32 {
        match self {
            AspectRatioPreset::Square => 1.0,
            AspectRatioPreset::Widescreen => 16.0 / 9.0,
            AspectRatioPreset::Standard => 4.0 / 3.0,
            AspectRatioPreset::Ultrawide => 21.0 / 9.0,
            AspectRatioPreset::Photo => 3.0 / 2.0,
            AspectRatioPreset::Custom(r) => *r,
        }
    }
}

/// A container that maintains a specific aspect ratio
///
/// # Example
/// ```
/// AspectRatio::new(AspectRatioPreset::Widescreen)
///     .child(img("video-thumbnail.jpg"))
/// ```
#[derive(IntoElement)]
pub struct AspectRatio {
    ratio: f32,
    content: Option<AnyElement>,
    width: Option<f32>,
}

impl AspectRatio {
    /// Create a new aspect ratio container with a preset
    pub fn new(preset: AspectRatioPreset) -> Self {
        Self {
            ratio: preset.ratio(),
            content: None,
            width: None,
        }
    }

    /// Create with a custom ratio (width/height)
    pub fn custom(ratio: f32) -> Self {
        Self {
            ratio,
            content: None,
            width: None,
        }
    }

    /// Set the content to display
    pub fn child(mut self, content: impl IntoElement) -> Self {
        self.content = Some(content.into_any_element());
        self
    }

    /// Set a fixed width (height will be calculated from ratio)
    pub fn width(mut self, w: f32) -> Self {
        self.width = Some(w);
        self
    }
}

impl RenderOnce for AspectRatio {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // Use padding-bottom trick for aspect ratio (percentage based on width)
        // This is a common CSS technique: padding-bottom = (1/ratio) * 100%
        let padding_percent = (1.0 / self.ratio) * 100.0;

        let mut container = div()
            .relative()
            .w_full()
            .overflow_hidden();

        // If width is specified, use it
        if let Some(w) = self.width {
            container = container.w(px(w));
        }

        // Spacer div that creates the aspect ratio via padding
        let spacer = div()
            .w_full()
            .pb(Rems(padding_percent / 100.0 * 1.0)); // Convert to approximate padding

        // Content positioned absolutely to fill the container
        let mut content_wrapper = div()
            .absolute()
            .top_0()
            .left_0()
            .w_full()
            .h_full();

        if let Some(content) = self.content {
            content_wrapper = content_wrapper.child(content);
        }

        container
            .child(spacer)
            .child(content_wrapper)
    }
}
