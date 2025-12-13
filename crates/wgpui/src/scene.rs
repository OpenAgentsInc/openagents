//! Scene primitives for GPU rendering

use crate::color::Hsla;
use crate::layout::Bounds;
use bytemuck::{Pod, Zeroable};
use std::rc::Rc;

/// GPU quad instance data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Quad {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub background: [f32; 4],     // HSLA
    pub border_color: [f32; 4],   // HSLA
    pub border_widths: [f32; 4],  // top, right, bottom, left
    pub corner_radii: [f32; 4],   // top_left, top_right, bottom_right, bottom_left
}

impl Quad {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            origin: [bounds.origin.x, bounds.origin.y],
            size: [bounds.size.width, bounds.size.height],
            background: [0.0, 0.0, 0.0, 0.0],
            border_color: [0.0, 0.0, 0.0, 0.0],
            border_widths: [0.0, 0.0, 0.0, 0.0],
            corner_radii: [0.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn with_background(mut self, color: Hsla) -> Self {
        self.background = color.to_array();
        self
    }

    pub fn with_border(mut self, color: Hsla, width: f32) -> Self {
        self.border_color = color.to_array();
        self.border_widths = [width, width, width, width];
        self
    }

    pub fn with_border_widths(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.border_widths = [top, right, bottom, left];
        self
    }

    pub fn with_corner_radii(mut self, radii: f32) -> Self {
        self.corner_radii = [radii, radii, radii, radii];
        self
    }

    pub fn with_corner_radii_each(
        mut self,
        top_left: f32,
        top_right: f32,
        bottom_right: f32,
        bottom_left: f32,
    ) -> Self {
        self.corner_radii = [top_left, top_right, bottom_right, bottom_left];
        self
    }
}

/// GPU text quad instance data
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct TextQuad {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub uv_origin: [f32; 2],
    pub uv_size: [f32; 2],
    pub color: [f32; 4], // HSLA
}

impl TextQuad {
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        uv_x: f32,
        uv_y: f32,
        uv_width: f32,
        uv_height: f32,
        color: Hsla,
    ) -> Self {
        Self {
            origin: [x, y],
            size: [width, height],
            uv_origin: [uv_x, uv_y],
            uv_size: [uv_width, uv_height],
            color: color.to_array(),
        }
    }
}

/// Text run for rendering
pub struct TextRun {
    pub text: String,
    pub position: [f32; 2],
    pub font_size: f32,
    pub color: Hsla,
}

/// Click handler type
pub type ClickHandler = Rc<dyn Fn()>;

/// A clickable region in the scene
pub struct ClickableRegion {
    pub bounds: Bounds,
    pub handler: ClickHandler,
}

/// Scene containing all rendering primitives for a frame
pub struct Scene {
    pub quads: Vec<Quad>,
    pub text_quads: Vec<TextQuad>,
    pub text_runs: Vec<TextRun>,
    pub clickable_regions: Vec<ClickableRegion>,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            quads: Vec::new(),
            text_quads: Vec::new(),
            text_runs: Vec::new(),
            clickable_regions: Vec::new(),
        }
    }
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a quad to the scene
    pub fn add_quad(&mut self, quad: Quad) {
        self.quads.push(quad);
    }

    /// Add a text quad to the scene
    pub fn add_text_quad(&mut self, text_quad: TextQuad) {
        self.text_quads.push(text_quad);
    }

    /// Add a text run to be laid out and rendered
    pub fn add_text(&mut self, text: impl Into<String>, position: [f32; 2], font_size: f32, color: Hsla) {
        self.text_runs.push(TextRun {
            text: text.into(),
            position,
            font_size,
            color,
        });
    }

    /// Add a clickable region to the scene
    pub fn add_clickable_region(&mut self, bounds: Bounds, handler: ClickHandler) {
        self.clickable_regions.push(ClickableRegion { bounds, handler });
    }

    /// Handle a click at the given position, returns true if a handler was called
    pub fn handle_click(&self, x: f32, y: f32) -> bool {
        use crate::layout::Point;
        let point = Point::new(x, y);
        // Iterate in reverse order so topmost elements get clicked first
        for region in self.clickable_regions.iter().rev() {
            if region.bounds.contains(point) {
                (region.handler)();
                return true;
            }
        }
        false
    }

    /// Clear all primitives for the next frame
    pub fn clear(&mut self) {
        self.quads.clear();
        self.text_quads.clear();
        self.text_runs.clear();
        self.clickable_regions.clear();
    }

    /// Check if scene is empty
    pub fn is_empty(&self) -> bool {
        self.quads.is_empty() && self.text_quads.is_empty() && self.text_runs.is_empty()
    }
}
