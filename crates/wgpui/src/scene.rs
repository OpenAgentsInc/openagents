use crate::color::Hsla;
use crate::geometry::{Bounds, Point, Size};
use bytemuck::{Pod, Zeroable};

#[derive(Clone, Debug)]
pub struct Quad {
    pub bounds: Bounds,
    pub background: Option<Hsla>,
    pub border_color: Hsla,
    pub border_width: f32,
    pub corner_radius: f32,
}

impl Default for Quad {
    fn default() -> Self {
        Self {
            bounds: Bounds::ZERO,
            background: None,
            border_color: Hsla::transparent(),
            border_width: 0.0,
            corner_radius: 0.0,
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

    pub fn with_corner_radius(mut self, radius: f32) -> Self {
        self.corner_radius = radius;
        self
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuQuad {
    pub origin: [f32; 2],
    pub size: [f32; 2],
    pub background: [f32; 4],
    pub border_color: [f32; 4],
    pub border_width: f32,
    pub corner_radius: f32,
    pub _padding: [f32; 2],
}

impl GpuQuad {
    pub fn from_quad(quad: &Quad) -> Self {
        Self {
            origin: [quad.bounds.origin.x, quad.bounds.origin.y],
            size: [quad.bounds.size.width, quad.bounds.size.height],
            background: quad
                .background
                .map(|c| {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        c.to_linear_rgba()
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        c.to_rgba()
                    }
                })
                .unwrap_or([0.0, 0.0, 0.0, 0.0]),
            border_color: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    quad.border_color.to_linear_rgba()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    quad.border_color.to_rgba()
                }
            },
            border_width: quad.border_width,
            corner_radius: quad.corner_radius,
            _padding: [0.0, 0.0],
        }
    }
}

#[derive(Clone, Debug)]
pub struct GlyphInstance {
    pub glyph_id: u16,
    pub offset: Point,
    pub size: Size,
    pub uv: [f32; 4],
}

#[derive(Clone, Debug)]
pub struct TextRun {
    pub glyphs: Vec<GlyphInstance>,
    pub origin: Point,
    pub color: Hsla,
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

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuTextQuad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub color: [f32; 4],
}

impl GpuTextQuad {
    pub fn from_glyph(glyph: &GlyphInstance, origin: Point, color: Hsla) -> Self {
        Self {
            position: [origin.x + glyph.offset.x, origin.y + glyph.offset.y],
            size: [glyph.size.width, glyph.size.height],
            uv: glyph.uv,
            color: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    color.to_linear_rgba()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    color.to_rgba()
                }
            },
        }
    }
}

#[derive(Default)]
pub struct Scene {
    pub quads: Vec<Quad>,
    pub text_runs: Vec<TextRun>,
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

    pub fn draw_quad(&mut self, quad: Quad) {
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

    pub fn draw_text(&mut self, text_run: TextRun) {
        self.text_runs.push(text_run);
    }

    pub fn push_clip(&mut self, bounds: Bounds) {
        let effective_clip = if let Some(parent) = self.clip_stack.last() {
            parent.intersection(&bounds).unwrap_or(Bounds::ZERO)
        } else {
            bounds
        };
        self.clip_stack.push(effective_clip);
    }

    pub fn pop_clip(&mut self) {
        self.clip_stack.pop();
    }

    pub fn current_clip(&self) -> Option<&Bounds> {
        self.clip_stack.last()
    }

    pub fn gpu_quads(&self) -> Vec<GpuQuad> {
        self.quads.iter().map(GpuQuad::from_quad).collect()
    }

    pub fn gpu_text_quads(&self) -> Vec<GpuTextQuad> {
        let mut quads = Vec::new();
        for run in &self.text_runs {
            for glyph in &run.glyphs {
                quads.push(GpuTextQuad::from_glyph(glyph, run.origin, run.color));
            }
        }
        quads
    }

    pub fn quads(&self) -> &[Quad] {
        &self.quads
    }

    pub fn text_runs(&self) -> &[TextRun] {
        &self.text_runs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quad_builder() {
        let bounds = Bounds::new(0.0, 0.0, 100.0, 50.0);
        let quad = Quad::new(bounds)
            .with_background(Hsla::white())
            .with_border(Hsla::black(), 1.0);

        assert!(quad.background.is_some());
        assert!((quad.border_width - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_scene_draw_quad() {
        let mut scene = Scene::new();
        let quad = Quad::new(Bounds::new(10.0, 10.0, 100.0, 50.0));
        scene.draw_quad(quad);

        assert_eq!(scene.quads().len(), 1);
    }

    #[test]
    fn test_scene_clipping() {
        let mut scene = Scene::new();

        scene.push_clip(Bounds::new(0.0, 0.0, 50.0, 50.0));

        let quad = Quad::new(Bounds::new(25.0, 25.0, 100.0, 100.0));
        scene.draw_quad(quad);

        assert_eq!(scene.quads().len(), 1);
        let clipped = &scene.quads()[0];
        assert!((clipped.bounds.width() - 25.0).abs() < 0.001);
        assert!((clipped.bounds.height() - 25.0).abs() < 0.001);

        scene.pop_clip();
        assert!(scene.current_clip().is_none());
    }

    #[test]
    fn test_scene_nested_clipping() {
        let mut scene = Scene::new();

        scene.push_clip(Bounds::new(0.0, 0.0, 100.0, 100.0));
        scene.push_clip(Bounds::new(50.0, 50.0, 100.0, 100.0));

        let clip = scene.current_clip().unwrap();
        assert!((clip.x() - 50.0).abs() < 0.001);
        assert!((clip.y() - 50.0).abs() < 0.001);
        assert!((clip.width() - 50.0).abs() < 0.001);
        assert!((clip.height() - 50.0).abs() < 0.001);

        scene.pop_clip();
        scene.pop_clip();
    }

    #[test]
    fn test_text_run() {
        let mut run = TextRun::new(Point::new(10.0, 20.0), Hsla::white(), 14.0);
        run.push_glyph(GlyphInstance {
            glyph_id: 65,
            offset: Point::new(0.0, 0.0),
            size: Size::new(8.0, 14.0),
            uv: [0.0, 0.0, 0.1, 0.1],
        });

        assert_eq!(run.glyphs.len(), 1);
        assert!((run.font_size - 14.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_quad_conversion() {
        let quad = Quad::new(Bounds::new(10.0, 20.0, 100.0, 50.0))
            .with_background(Hsla::from_hex(0xFF0000));

        let gpu_quad = GpuQuad::from_quad(&quad);

        assert!((gpu_quad.origin[0] - 10.0).abs() < 0.001);
        assert!((gpu_quad.origin[1] - 20.0).abs() < 0.001);
        assert!((gpu_quad.size[0] - 100.0).abs() < 0.001);
        assert!((gpu_quad.size[1] - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_text_quad_conversion() {
        let glyph = GlyphInstance {
            glyph_id: 65,
            offset: Point::new(5.0, 0.0),
            size: Size::new(8.0, 14.0),
            uv: [0.0, 0.0, 0.1, 0.1],
        };

        let gpu_quad = GpuTextQuad::from_glyph(&glyph, Point::new(10.0, 20.0), Hsla::white());

        assert!((gpu_quad.position[0] - 15.0).abs() < 0.001);
        assert!((gpu_quad.position[1] - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_scene_clear() {
        let mut scene = Scene::new();
        scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, 10.0, 10.0)));
        scene.draw_text(TextRun::new(Point::ZERO, Hsla::white(), 12.0));
        scene.push_clip(Bounds::new(0.0, 0.0, 50.0, 50.0));

        scene.clear();

        assert!(scene.quads().is_empty());
        assert!(scene.text_runs().is_empty());
        assert!(scene.current_clip().is_none());
    }
}
