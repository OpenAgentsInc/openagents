//! UI Components for the web demo
//!
//! Components render to primitives (quads and text) that are then
//! drawn by the GPU renderer.

pub mod button;
pub mod card;
pub mod input;

pub use button::{Button, ButtonSize, ButtonStyle};
pub use card::Card;
pub use input::Input;

use crate::text::TextSystem;
use crate::theme::Color;
use bytemuck::{Pod, Zeroable};

/// GPU-ready quad instance (same as in lib.rs)
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuQuad {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub background: [f32; 4],    // HSLA
    pub border_color: [f32; 4],  // HSLA
    pub border_widths: [f32; 4], // top, right, bottom, left
    pub corner_radii: [f32; 4],  // per-corner
}

impl GpuQuad {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            origin: [x, y],
            size: [w, h],
            background: [0.0, 0.0, 0.0, 0.0],
            border_color: [0.0, 0.0, 0.0, 0.0],
            border_widths: [0.0; 4],
            corner_radii: [0.0; 4],
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.background = color;
        self
    }

    pub fn border(mut self, color: Color, width: f32) -> Self {
        self.border_color = color;
        self.border_widths = [width; 4];
        self
    }

    pub fn radius(mut self, r: f32) -> Self {
        self.corner_radii = [r; 4];
        self
    }
}

/// Text rendering request
#[derive(Clone, Debug)]
pub struct TextRun {
    pub text: String,
    pub position: [f32; 2],
    pub size: f32,
    pub color: Color,
}

/// Rendering primitive
pub enum Primitive {
    Quad(GpuQuad),
    Text(TextRun),
}

/// Bounds of a component
#[derive(Clone, Copy, Debug)]
pub struct Bounds {
    pub origin: [f32; 2],
    pub size: [f32; 2],
}

/// Component rendering context
pub struct RenderContext<'a> {
    pub text_system: &'a mut TextSystem,
    pub viewport_size: [f32; 2],
}

/// Trait for UI components
pub trait Component {
    /// Render component to primitives
    fn render(&self, ctx: &mut RenderContext) -> Vec<Primitive>;

    /// Get component bounds
    fn bounds(&self) -> Bounds;
}
