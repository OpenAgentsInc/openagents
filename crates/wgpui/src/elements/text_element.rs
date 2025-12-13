//! Text element - renders text content

use crate::color::Hsla;
use crate::element::{Element, LayoutContext, PaintContext};
use crate::layout::{Bounds, LayoutId};
use crate::theme;

/// Text element for rendering text content
pub struct Text {
    content: String,
    color: Hsla,
    font_size: f32,
}

impl Text {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            color: theme::text::PRIMARY,
            font_size: theme::FONT_SIZE,
        }
    }

    /// Set text color
    pub fn color(mut self, color: impl Into<Hsla>) -> Self {
        self.color = color.into();
        self
    }

    /// Set font size
    pub fn size(mut self, size: f32) -> Self {
        self.font_size = size;
        self
    }
}

/// Create a text element
pub fn text(content: impl Into<String>) -> Text {
    Text::new(content)
}

/// State for text during layout
#[derive(Default)]
pub struct TextState {
    measured_width: f32,
    measured_height: f32,
}

impl Element for Text {
    type State = TextState;

    fn request_layout(&mut self, cx: &mut LayoutContext) -> (LayoutId, Self::State) {
        // Measure text to determine intrinsic size
        let measured_width = cx.text_system.measure(&self.content, self.font_size);
        let measured_height = self.font_size * 1.2; // Approximate line height

        // Request layout with measured size as the preferred size
        let layout_id = cx
            .layout_engine
            .request_measured_layout(measured_width, measured_height);

        (
            layout_id,
            TextState {
                measured_width,
                measured_height,
            },
        )
    }

    fn paint(&mut self, bounds: Bounds, _state: &mut Self::State, cx: &mut PaintContext) {
        // Add text to scene for rendering
        cx.scene.add_text(
            &self.content,
            [bounds.origin.x, bounds.origin.y + self.font_size], // Baseline offset
            self.font_size,
            self.color,
        );
    }
}
