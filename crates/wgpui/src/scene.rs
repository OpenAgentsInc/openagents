use crate::color::Hsla;
use crate::geometry::Bounds;

#[derive(Clone, Debug)]
pub struct Quad {
    pub bounds: Bounds,
    pub background: Option<Hsla>,
    pub border_color: Option<Hsla>,
    pub border_width: f32,
}

impl Quad {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            background: None,
            border_color: None,
            border_width: 0.0,
        }
    }

    pub fn with_background(mut self, color: Hsla) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_border(mut self, color: Hsla, width: f32) -> Self {
        self.border_color = Some(color);
        self.border_width = width;
        self
    }
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub glyphs: Vec<GlyphInstance>,
}

#[derive(Clone, Copy, Debug)]
pub struct GlyphInstance {
    pub x: f32,
    pub y: f32,
    pub glyph_id: u32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct GpuQuad {
    pub bounds: [f32; 4],
    pub background: [f32; 4],
    pub border_color: [f32; 4],
    pub border_width: f32,
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct GpuTextQuad {
    pub bounds: [f32; 4],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

#[derive(Default)]
pub struct Scene {
    quads: Vec<Quad>,
    text_runs: Vec<TextRun>,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn draw_quad(&mut self, quad: Quad) {
        self.quads.push(quad);
    }

    pub fn draw_text(&mut self, run: TextRun) {
        self.text_runs.push(run);
    }

    pub fn quads(&self) -> &[Quad] {
        &self.quads
    }

    pub fn text_runs(&self) -> &[TextRun] {
        &self.text_runs
    }

    pub fn clear(&mut self) {
        self.quads.clear();
        self.text_runs.clear();
    }
}
