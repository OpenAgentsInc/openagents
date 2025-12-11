//! Avatar component for user/entity display

use gpui::prelude::*;
use gpui::*;
use theme_oa::{bg, text, border};

/// Avatar size variants
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum AvatarSize {
    Sm,
    #[default]
    Default,
    Lg,
}

impl AvatarSize {
    fn px_size(&self) -> f32 {
        match self {
            AvatarSize::Sm => 24.0,
            AvatarSize::Default => 40.0,
            AvatarSize::Lg => 56.0,
        }
    }

    fn text_size(&self) -> f32 {
        match self {
            AvatarSize::Sm => 10.0,
            AvatarSize::Default => 14.0,
            AvatarSize::Lg => 20.0,
        }
    }
}

/// An avatar component showing an image or fallback initials
///
/// # Example
/// ```
/// Avatar::new().fallback("JD")
/// Avatar::new().fallback("AB").size(AvatarSize::Lg)
/// ```
#[derive(IntoElement)]
pub struct Avatar {
    fallback: Option<SharedString>,
    size: AvatarSize,
}

impl Avatar {
    /// Create a new avatar
    pub fn new() -> Self {
        Self {
            fallback: None,
            size: AvatarSize::Default,
        }
    }

    /// Set the fallback text (usually initials)
    pub fn fallback(mut self, text: impl Into<SharedString>) -> Self {
        self.fallback = Some(text.into());
        self
    }

    /// Set the avatar size
    pub fn size(mut self, size: AvatarSize) -> Self {
        self.size = size;
        self
    }
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderOnce for Avatar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let size = self.size.px_size();
        let text_size = self.size.text_size();

        let mut el = div()
            .w(px(size))
            .h(px(size))
            .rounded_full()
            .bg(bg::ELEVATED)
            .border_1()
            .border_color(border::DEFAULT)
            .flex()
            .items_center()
            .justify_center()
            .overflow_hidden();

        // Show fallback initials
        if let Some(fallback) = self.fallback {
            el = el.child(
                div()
                    .text_color(text::PRIMARY)
                    .text_size(px(text_size))
                    .font_weight(FontWeight::MEDIUM)
                    .child(fallback)
            );
        }

        el
    }
}
