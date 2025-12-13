//! Input field component (visual only, no text editing)

use super::{Bounds, Component, GpuQuad, Primitive, RenderContext, TextRun};
use crate::theme::{self, FONT_SIZE};

/// Input field component
pub struct Input {
    pub placeholder: String,
    pub value: String,
    pub origin: [f32; 2],
    pub width: f32,
    pub height: f32,
    pub disabled: bool,
}

impl Input {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            placeholder: placeholder.into(),
            value: String::new(),
            origin: [0.0, 0.0],
            width: 192.0,
            height: 32.0,
            disabled: false,
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.origin = [x, y];
        self
    }

    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    pub fn height(mut self, height: f32) -> Self {
        self.height = height;
        self
    }

    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

impl Component for Input {
    fn render(&self, _ctx: &mut RenderContext) -> Vec<Primitive> {
        let padding_x = 8.0;
        let corner_radius = 6.0;

        // Determine text to display
        let (display_text, text_color) = if self.value.is_empty() {
            (&self.placeholder, theme::input::PLACEHOLDER)
        } else {
            (&self.value, theme::input::TEXT)
        };

        let text_color = if self.disabled {
            theme::text::DISABLED
        } else {
            text_color
        };

        let mut primitives = Vec::new();

        // Background
        let quad = GpuQuad::new(self.origin[0], self.origin[1], self.width, self.height)
            .bg(theme::input::BG)
            .border(theme::input::BORDER, 1.0)
            .radius(corner_radius);

        primitives.push(Primitive::Quad(quad));

        // Text
        let text_x = self.origin[0] + padding_x;
        let text_y = self.origin[1] + (self.height - FONT_SIZE) / 2.0 + FONT_SIZE * 0.8;

        primitives.push(Primitive::Text(TextRun {
            text: display_text.clone(),
            position: [text_x, text_y],
            size: FONT_SIZE,
            color: text_color,
        }));

        primitives
    }

    fn bounds(&self) -> Bounds {
        Bounds {
            origin: self.origin,
            size: [self.width, self.height],
        }
    }
}
