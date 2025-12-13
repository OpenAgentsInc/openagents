//! Card/container component

use super::{Bounds, Component, GpuQuad, Primitive, RenderContext};
use crate::theme;

/// Card container component
pub struct Card {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub fill: bool,
    pub border: bool,
    pub corner_radius: f32,
}

impl Card {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            origin: [0.0, 0.0],
            size: [width, height],
            fill: true,
            border: true,
            corner_radius: 4.0,
        }
    }

    pub fn at(mut self, x: f32, y: f32) -> Self {
        self.origin = [x, y];
        self
    }

    pub fn borderless(mut self) -> Self {
        self.border = false;
        self
    }

    pub fn unfilled(mut self) -> Self {
        self.fill = false;
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radius = r;
        self
    }
}

impl Component for Card {
    fn render(&self, _ctx: &mut RenderContext) -> Vec<Primitive> {
        let bg_color = if self.fill {
            theme::bg::CARD
        } else {
            [0.0, 0.0, 0.0, 0.0]
        };

        let mut quad =
            GpuQuad::new(self.origin[0], self.origin[1], self.size[0], self.size[1])
                .bg(bg_color)
                .radius(self.corner_radius);

        if self.border {
            quad = quad.border(theme::border::DEFAULT, 1.0);
        }

        vec![Primitive::Quad(quad)]
    }

    fn bounds(&self) -> Bounds {
        Bounds {
            origin: self.origin,
            size: self.size,
        }
    }
}
