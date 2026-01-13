use crate::color::Hsla;
use crate::curve::CurvePrimitive;
use crate::geometry::{Bounds, Point, Size};
use bytemuck::{Pod, Zeroable};

/// GPU-ready image/SVG quad for rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuImageQuad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub tint: [f32; 4],
}

impl GpuImageQuad {
    /// Create a GPU image quad from bounds and optional tint.
    /// UV is full texture (0,0 to 1,1).
    pub fn new(position: [f32; 2], size: [f32; 2], tint: Option<Hsla>) -> Self {
        let tint_color = tint
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
            .unwrap_or([1.0, 1.0, 1.0, 1.0]); // White = no tint

        Self {
            position,
            size,
            uv: [0.0, 0.0, 1.0, 1.0], // Full texture
            tint: tint_color,
        }
    }
}

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
    /// Create a GPU quad from a scene quad.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn from_quad(quad: &Quad, scale_factor: f32) -> Self {
        // Scale from LOGICAL to PHYSICAL pixels at GPU boundary.
        Self {
            origin: [
                quad.bounds.origin.x * scale_factor,
                quad.bounds.origin.y * scale_factor,
            ],
            size: [
                quad.bounds.size.width * scale_factor,
                quad.bounds.size.height * scale_factor,
            ],
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
            border_width: quad.border_width * scale_factor,
            corner_radius: quad.corner_radius * scale_factor,
            _padding: [0.0, 0.0],
        }
    }
}

/// GPU-ready line instance for rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuLine {
    pub start: [f32; 2],
    pub end: [f32; 2],
    pub width: f32,
    pub _pad: f32,
    pub color: [f32; 4],
}

impl GpuLine {
    /// Create a GPU line from start/end points and styling.
    pub fn new(start: Point, end: Point, width: f32, color: Hsla, scale_factor: f32) -> Self {
        Self {
            start: [start.x * scale_factor, start.y * scale_factor],
            end: [end.x * scale_factor, end.y * scale_factor],
            width: width * scale_factor,
            _pad: 0.0,
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

    /// Compute the bounding box of this text run
    pub fn bounds(&self) -> Bounds {
        if self.glyphs.is_empty() {
            return Bounds::new(self.origin.x, self.origin.y, 0.0, 0.0);
        }

        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for glyph in &self.glyphs {
            let x = self.origin.x + glyph.offset.x;
            let y = self.origin.y + glyph.offset.y;
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x + glyph.size.width);
            max_y = max_y.max(y + glyph.size.height);
        }

        Bounds::new(min_x, min_y, max_x - min_x, max_y - min_y)
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
    /// Create a GPU text quad from a glyph instance.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn from_glyph(
        glyph: &GlyphInstance,
        origin: Point,
        color: Hsla,
        scale_factor: f32,
    ) -> Self {
        // Scale from LOGICAL to PHYSICAL pixels at GPU boundary.
        // - origin: logical position where text run starts
        // - glyph.offset: logical offset from origin (already divided by scale_factor in text.rs)
        // - glyph.size: logical glyph size (already divided by scale_factor in text.rs)
        // Multiply by scale_factor to get physical pixels for the shader.
        Self {
            position: [
                (origin.x + glyph.offset.x) * scale_factor,
                (origin.y + glyph.offset.y) * scale_factor,
            ],
            size: [
                glyph.size.width * scale_factor,
                glyph.size.height * scale_factor,
            ],
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

/// An SVG to be rendered as a textured quad.
#[derive(Clone, Debug)]
pub struct SvgQuad {
    /// Bounds to render the SVG within (logical pixels)
    pub bounds: Bounds,
    /// Raw SVG bytes
    pub svg_data: std::sync::Arc<[u8]>,
    /// Optional tint color (for monochrome icons)
    pub tint: Option<Hsla>,
}

impl SvgQuad {
    /// Create a new SVG quad.
    pub fn new(bounds: Bounds, svg_data: std::sync::Arc<[u8]>) -> Self {
        Self {
            bounds,
            svg_data,
            tint: None,
        }
    }

    /// Set a tint color for the SVG.
    pub fn with_tint(mut self, color: Hsla) -> Self {
        self.tint = Some(color);
        self
    }
}

#[derive(Default)]
pub struct Scene {
    pub quads: Vec<(u32, Quad)>,           // (layer, quad)
    pub text_runs: Vec<(u32, TextRun)>,    // (layer, text_run)
    pub curves: Vec<(u32, CurvePrimitive)>, // (layer, curve)
    pub svg_quads: Vec<SvgQuad>,
    clip_stack: Vec<Bounds>,
    current_layer: u32,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.quads.clear();
        self.text_runs.clear();
        self.curves.clear();
        self.svg_quads.clear();
        self.clip_stack.clear();
        self.current_layer = 0;
    }

    /// Set the current layer for subsequent draw calls.
    /// Higher layers are rendered on top of lower layers.
    pub fn set_layer(&mut self, layer: u32) {
        self.current_layer = layer;
    }

    /// Get the current layer.
    pub fn layer(&self) -> u32 {
        self.current_layer
    }

    pub fn draw_quad(&mut self, quad: Quad) {
        if let Some(clip) = self.clip_stack.last() {
            if quad.bounds.intersects(clip) {
                self.quads.push((self.current_layer, quad));
            }
        } else {
            self.quads.push((self.current_layer, quad));
        }
    }

    pub fn draw_text(&mut self, text_run: TextRun) {
        if let Some(clip) = self.clip_stack.last() {
            if text_run.bounds().intersects(clip) {
                self.text_runs.push((self.current_layer, text_run));
            }
        } else {
            self.text_runs.push((self.current_layer, text_run));
        }
    }

    /// Draw an SVG at the specified bounds.
    pub fn draw_svg(&mut self, svg: SvgQuad) {
        if let Some(clip) = self.clip_stack.last() {
            if svg.bounds.intersects(clip) {
                self.svg_quads.push(svg);
            }
        } else {
            self.svg_quads.push(svg);
        }
    }

    /// Draw a bezier curve.
    pub fn draw_curve(&mut self, curve: CurvePrimitive) {
        // TODO: Add clipping support for curves
        self.curves.push((self.current_layer, curve));
    }

    /// Convert curves in a layer to GPU lines for rendering.
    /// Tessellates curves into line segments with adaptive subdivision.
    pub fn curve_lines_for_layer(&self, layer: u32, scale_factor: f32) -> Vec<GpuLine> {
        let mut lines = Vec::new();

        let curves_in_layer: Vec<_> = self.curves.iter().filter(|(l, _)| *l == layer).collect();

        for (_l, curve) in curves_in_layer {
            // Use adaptive tessellation for smooth curves
            let segments = curve.tessellate_adaptive(0.5);

            for seg in segments {
                lines.push(GpuLine::new(
                    seg.start,
                    seg.end,
                    curve.stroke_width,
                    curve.color,
                    scale_factor,
                ));
            }
        }

        lines
    }

    /// Check if a layer has curves.
    pub fn has_curves_in_layer(&self, layer: u32) -> bool {
        self.curves.iter().any(|(l, _)| *l == layer)
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

    /// Get GPU quads for a specific layer.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn gpu_quads_for_layer(&self, layer: u32, scale_factor: f32) -> Vec<GpuQuad> {
        self.quads
            .iter()
            .filter(|(l, _)| *l == layer)
            .map(|(_, q)| GpuQuad::from_quad(q, scale_factor))
            .collect()
    }

    /// Get GPU text quads for a specific layer.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn gpu_text_quads_for_layer(&self, layer: u32, scale_factor: f32) -> Vec<GpuTextQuad> {
        let mut quads = Vec::new();
        for (l, run) in &self.text_runs {
            if *l != layer {
                continue;
            }
            for glyph in &run.glyphs {
                quads.push(GpuTextQuad::from_glyph(
                    glyph,
                    run.origin,
                    run.color,
                    scale_factor,
                ));
            }
        }
        quads
    }

    /// Get all unique layers used in this scene, sorted.
    pub fn layers(&self) -> Vec<u32> {
        let mut layers: Vec<u32> = self
            .quads
            .iter()
            .map(|(l, _)| *l)
            .chain(self.text_runs.iter().map(|(l, _)| *l))
            .chain(self.curves.iter().map(|(l, _)| *l))
            .collect();
        layers.sort();
        layers.dedup();
        layers
    }

    pub fn quads(&self) -> Vec<&Quad> {
        self.quads.iter().map(|(_, q)| q).collect()
    }

    pub fn text_runs(&self) -> Vec<&TextRun> {
        self.text_runs.iter().map(|(_, r)| r).collect()
    }

    pub fn svg_quads(&self) -> &[SvgQuad] {
        &self.svg_quads
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
        let drawn = &scene.quads()[0];
        assert!((drawn.bounds.width() - 100.0).abs() < 0.001);
        assert!((drawn.bounds.height() - 100.0).abs() < 0.001);

        let non_intersecting = Quad::new(Bounds::new(100.0, 100.0, 50.0, 50.0));
        scene.draw_quad(non_intersecting);
        assert_eq!(scene.quads().len(), 1);

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

        // Test with scale_factor 1.0 (no scaling)
        let gpu_quad = GpuQuad::from_quad(&quad, 1.0);

        assert!((gpu_quad.origin[0] - 10.0).abs() < 0.001);
        assert!((gpu_quad.origin[1] - 20.0).abs() < 0.001);
        assert!((gpu_quad.size[0] - 100.0).abs() < 0.001);
        assert!((gpu_quad.size[1] - 50.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_quad_scaling() {
        let quad = Quad::new(Bounds::new(10.0, 20.0, 100.0, 50.0))
            .with_background(Hsla::from_hex(0xFF0000))
            .with_border(Hsla::white(), 2.0);

        // Test with scale_factor 2.0 (2x scaling)
        let gpu_quad = GpuQuad::from_quad(&quad, 2.0);

        // Position and size should be scaled by 2x
        assert!((gpu_quad.origin[0] - 20.0).abs() < 0.001); // 10 * 2 = 20
        assert!((gpu_quad.origin[1] - 40.0).abs() < 0.001); // 20 * 2 = 40
        assert!((gpu_quad.size[0] - 200.0).abs() < 0.001); // 100 * 2 = 200
        assert!((gpu_quad.size[1] - 100.0).abs() < 0.001); // 50 * 2 = 100
        assert!((gpu_quad.border_width - 4.0).abs() < 0.001); // 2 * 2 = 4
    }

    #[test]
    fn test_gpu_text_quad_conversion() {
        let glyph = GlyphInstance {
            glyph_id: 65,
            offset: Point::new(5.0, 0.0),
            size: Size::new(8.0, 14.0),
            uv: [0.0, 0.0, 0.1, 0.1],
        };

        // Test with scale_factor 1.0 (no scaling)
        let gpu_quad = GpuTextQuad::from_glyph(&glyph, Point::new(10.0, 20.0), Hsla::white(), 1.0);

        assert!((gpu_quad.position[0] - 15.0).abs() < 0.001);
        assert!((gpu_quad.position[1] - 20.0).abs() < 0.001);
    }

    #[test]
    fn test_gpu_text_quad_scaling() {
        let glyph = GlyphInstance {
            glyph_id: 65,
            offset: Point::new(5.0, 0.0),
            size: Size::new(8.0, 14.0),
            uv: [0.0, 0.0, 0.1, 0.1],
        };

        // Test with scale_factor 2.0 (2x scaling)
        let gpu_quad = GpuTextQuad::from_glyph(&glyph, Point::new(10.0, 20.0), Hsla::white(), 2.0);

        // Position and size should be scaled by 2x
        assert!((gpu_quad.position[0] - 30.0).abs() < 0.001); // (10 + 5) * 2 = 30
        assert!((gpu_quad.position[1] - 40.0).abs() < 0.001); // (20 + 0) * 2 = 40
        assert!((gpu_quad.size[0] - 16.0).abs() < 0.001); // 8 * 2 = 16
        assert!((gpu_quad.size[1] - 28.0).abs() < 0.001); // 14 * 2 = 28
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
