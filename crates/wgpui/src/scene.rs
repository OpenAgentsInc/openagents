//! Scene primitives: Quad, TextRun, and Scene accumulator.

use crate::color::Hsla;
use crate::geometry::{Bounds, CornerRadii, Point, Size};
use bytemuck::{Pod, Zeroable};

/// A quad primitive for GPU rendering.
/// Supports background color, border, and rounded corners.
#[derive(Clone, Debug)]
pub struct Quad {
    pub bounds: Bounds,
    pub background: Option<Hsla>,
    pub border_color: Hsla,
    pub border_width: f32,
    pub corner_radii: CornerRadii,
}

impl Default for Quad {
    fn default() -> Self {
        Self {
            bounds: Bounds::ZERO,
            background: None,
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radii: CornerRadii::ZERO,
        }
    }
}

impl Quad {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            ..Default::default()
        }
    }

    pub fn with_background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_border(mut self, color: Hsla, width: f32) -> Self {
        self.border_color = color;
        self.border_width = width;
        self
    }

    pub fn with_corner_radii(mut self, radii: CornerRadii) -> Self {
        self.corner_radii = radii;
        self
    }

    pub fn with_uniform_radius(mut self, radius: f32) -> Self {
        self.corner_radii = CornerRadii::uniform(radius);
        self
    }
}

/// GPU-ready quad instance data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuQuad {
    /// Origin (x, y) in logical pixels
    pub origin: [f32; 2],
    /// Size (width, height) in logical pixels
    pub size: [f32; 2],
    /// Background color (RGBA)
    pub background: [f32; 4],
    /// Border color (RGBA)
    pub border_color: [f32; 4],
    /// Border width
    pub border_width: f32,
    /// Corner radii (TL, TR, BR, BL)
    pub corner_radii: [f32; 4],
    /// Padding for alignment
    pub _padding: [f32; 2],
}

impl GpuQuad {
    pub fn from_quad(quad: &Quad) -> Self {
        Self {
            origin: [quad.bounds.origin.x, quad.bounds.origin.y],
            size: [quad.bounds.size.width, quad.bounds.size.height],
            background: quad
                .background
                .map(|c| c.to_rgba())
                .unwrap_or([0.0, 0.0, 0.0, 0.0]),
            border_color: quad.border_color.to_rgba(),
            border_width: quad.border_width,
            corner_radii: quad.corner_radii.to_array(),
            _padding: [0.0, 0.0],
        }
    }
}

/// A positioned glyph for text rendering.
#[derive(Clone, Debug)]
pub struct GlyphInstance {
    /// Glyph ID from the font
    pub glyph_id: u16,
    /// Position relative to text origin
    pub offset: Point,
    /// Size of the glyph
    pub size: Size,
    /// UV coordinates in the glyph atlas (min_u, min_v, max_u, max_v)
    pub uv: [f32; 4],
}

/// A run of text with consistent styling.
#[derive(Clone, Debug)]
pub struct TextRun {
    /// Glyphs in this run
    pub glyphs: Vec<GlyphInstance>,
    /// Origin of the text run
    pub origin: Point,
    /// Text color
    pub color: Hsla,
    /// Font size used
    pub font_size: f32,
}

impl TextRun {
    pub fn new(origin: Point, color: Hsla, font_size: f32) -> Self {
        Self {
            glyphs: Vec::new(),
            origin,
            color,
            font_size,
        }
    }

    pub fn push_glyph(&mut self, glyph: GlyphInstance) {
        self.glyphs.push(glyph);
    }
}

/// GPU-ready text quad instance data.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuTextQuad {
    /// Position in logical pixels
    pub position: [f32; 2],
    /// Size in logical pixels
    pub size: [f32; 2],
    /// UV coordinates (min_u, min_v, max_u, max_v)
    pub uv: [f32; 4],
    /// Text color (RGBA)
    pub color: [f32; 4],
}

impl GpuTextQuad {
    pub fn from_glyph(glyph: &GlyphInstance, origin: Point, color: Hsla) -> Self {
        Self {
            position: [origin.x + glyph.offset.x, origin.y + glyph.offset.y],
            size: [glyph.size.width, glyph.size.height],
            uv: glyph.uv,
            color: color.to_rgba(),
        }
    }
}

/// Accumulated scene primitives for a frame.
#[derive(Default)]
pub struct Scene {
    /// Quads to render (back to front)
    pub quads: Vec<Quad>,
    /// Text runs to render
    pub text_runs: Vec<TextRun>,
    /// Clip stack for nested clipping
    clip_stack: Vec<Bounds>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.quads.clear();
        self.text_runs.clear();
        self.clip_stack.clear();
    }

    /// Draw a quad
    pub fn draw_quad(&mut self, quad: Quad) {
        // Apply clipping if active
        if let Some(clip) = self.clip_stack.last() {
            if let Some(clipped) = quad.bounds.intersection(clip) {
                let mut clipped_quad = quad;
                clipped_quad.bounds = clipped;
                self.quads.push(clipped_quad);
            }
        } else {
            self.quads.push(quad);
        }
    }

    /// Draw a text run
    pub fn draw_text(&mut self, text_run: TextRun) {
        // TODO: Apply clipping to text runs
        self.text_runs.push(text_run);
    }

    /// Push a clipping region
    pub fn push_clip(&mut self, bounds: Bounds) {
        let effective_clip = if let Some(parent) = self.clip_stack.last() {
            parent.intersection(&bounds).unwrap_or(Bounds::ZERO)
        } else {
            bounds
        };
        self.clip_stack.push(effective_clip);
    }

    /// Pop the current clipping region
    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    /// Get current clip bounds (if any)
    pub fn current_clip(&self) -> Option<&Bounds> {
        self.clip_stack.last()
    }

    /// Convert quads to GPU format
    pub fn gpu_quads(&self) -> Vec<GpuQuad> {
        self.quads.iter().map(GpuQuad::from_quad).collect()
    }

    /// Convert text runs to GPU format
    pub fn gpu_text_quads(&self) -> Vec<GpuTextQuad> {
        let mut quads = Vec::new();
        for run in &self.text_runs {
            for glyph in &run.glyphs {
                quads.push(GpuTextQuad::from_glyph(glyph, run.origin, run.color));
            }
        }
        quads
    }
}
