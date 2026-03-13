use crate::color::Hsla;
use crate::curve::CurvePrimitive;
use crate::geometry::{Bounds, Point, Size};
pub use crate::vector::SvgQuad;
use crate::vector::{ImageQuad, VectorBatch};
use bytemuck::{Pod, Zeroable};
use std::error::Error;
use std::fmt;

/// GPU-ready image/SVG quad for rendering.
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct GpuImageQuad {
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub uv: [f32; 4],
    pub tint: [f32; 4],
    pub clip_origin: [f32; 2],
    pub clip_size: [f32; 2],
}

impl GpuImageQuad {
    pub fn from_image(image: &ImageQuad, clip: Option<Bounds>, scale_factor: f32) -> Self {
        let (clip_origin, clip_size) = clip_to_gpu(clip, scale_factor);
        let mut tint_color = image.tint.map_or([1.0, 1.0, 1.0, 1.0], |color| {
            #[cfg(not(target_arch = "wasm32"))]
            {
                color.to_linear_rgba()
            }
            #[cfg(target_arch = "wasm32")]
            {
                color.to_rgba()
            }
        });
        tint_color[3] *= image.opacity;

        Self {
            position: [
                image.bounds.origin.x * scale_factor,
                image.bounds.origin.y * scale_factor,
            ],
            size: [
                image.bounds.size.width * scale_factor,
                image.bounds.size.height * scale_factor,
            ],
            uv: image.uv,
            tint: tint_color,
            clip_origin,
            clip_size,
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
    pub clip_origin: [f32; 2],
    pub clip_size: [f32; 2],
}

impl GpuQuad {
    /// Create a GPU quad from a scene quad.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn from_quad(quad: &Quad, clip: Option<Bounds>, scale_factor: f32) -> Self {
        let (clip_origin, clip_size) = clip_to_gpu(clip, scale_factor);
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
            background: quad.background.map_or([0.0, 0.0, 0.0, 0.0], |color| {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    color.to_linear_rgba()
                }
                #[cfg(target_arch = "wasm32")]
                {
                    color.to_rgba()
                }
            }),
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
            clip_origin,
            clip_size,
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
    pub pad: f32,
    pub color: [f32; 4],
}

impl GpuLine {
    /// Create a GPU line from start/end points and styling.
    pub fn new(start: Point, end: Point, width: f32, color: Hsla, scale_factor: f32) -> Self {
        Self {
            start: [start.x * scale_factor, start.y * scale_factor],
            end: [end.x * scale_factor, end.y * scale_factor],
            width: width * scale_factor,
            pad: 0.0,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeshTopology {
    TriangleList,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

impl MeshVertex {
    pub fn new(position: [f32; 3], normal: [f32; 3], color: [f32; 4]) -> Self {
        Self {
            position,
            normal,
            color,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MeshEdge {
    pub start: u32,
    pub end: u32,
    pub flags: u32,
}

/// Optional edge flag hint for silhouette emphasis.
pub const MESH_EDGE_FLAG_SILHOUETTE: u32 = 1 << 3;
/// Optional edge flag hint for selected-entity outline emphasis.
pub const MESH_EDGE_FLAG_SELECTED: u32 = 1 << 4;

impl MeshEdge {
    pub fn new(start: u32, end: u32) -> Self {
        Self {
            start,
            end,
            flags: 0,
        }
    }

    pub fn with_flags(mut self, flags: u32) -> Self {
        self.flags = flags;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MeshPrimitive {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u32>,
    pub edges: Vec<MeshEdge>,
    pub topology: MeshTopology,
}

impl MeshPrimitive {
    pub fn new(vertices: Vec<MeshVertex>, indices: Vec<u32>) -> Self {
        Self {
            vertices,
            indices,
            edges: Vec::new(),
            topology: MeshTopology::TriangleList,
        }
    }

    pub fn with_edges(mut self, edges: Vec<MeshEdge>) -> Self {
        self.edges = edges;
        self
    }

    pub fn with_topology(mut self, topology: MeshTopology) -> Self {
        self.topology = topology;
        self
    }

    pub fn validate(&self) -> Result<(), MeshPrimitiveError> {
        if self.vertices.is_empty() {
            return Err(MeshPrimitiveError::EmptyVertices);
        }
        if self.indices.is_empty() {
            return Err(MeshPrimitiveError::EmptyIndices);
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(MeshPrimitiveError::InvalidTriangleIndexCount {
                count: self.indices.len(),
            });
        }

        let vertex_count = self.vertices.len() as u32;
        for (idx, index) in self.indices.iter().enumerate() {
            if *index >= vertex_count {
                return Err(MeshPrimitiveError::IndexOutOfRange {
                    index_position: idx,
                    index_value: *index,
                    vertex_count,
                });
            }
        }
        for (idx, edge) in self.edges.iter().enumerate() {
            if edge.start >= vertex_count || edge.end >= vertex_count {
                return Err(MeshPrimitiveError::EdgeIndexOutOfRange {
                    edge_position: idx,
                    start: edge.start,
                    end: edge.end,
                    vertex_count,
                });
            }
            if edge.start == edge.end {
                return Err(MeshPrimitiveError::DegenerateEdge { edge_position: idx });
            }
        }

        for (idx, vertex) in self.vertices.iter().enumerate() {
            if !vertex
                .position
                .iter()
                .chain(vertex.normal.iter())
                .chain(vertex.color.iter())
                .all(|value| value.is_finite())
            {
                return Err(MeshPrimitiveError::NonFiniteVertex {
                    vertex_position: idx,
                });
            }
        }

        Ok(())
    }

    pub fn bounds(&self) -> Option<Bounds> {
        let mut iter = self.vertices.iter();
        let first = iter.next()?;
        let mut min = first.position;
        let mut max = first.position;
        for vertex in iter {
            for axis in 0..3 {
                min[axis] = min[axis].min(vertex.position[axis]);
                max[axis] = max[axis].max(vertex.position[axis]);
            }
        }
        Some(Bounds::new(
            min[0],
            min[1],
            max[0] - min[0],
            max[1] - min[1],
        ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MeshPrimitiveError {
    EmptyVertices,
    EmptyIndices,
    InvalidTriangleIndexCount {
        count: usize,
    },
    IndexOutOfRange {
        index_position: usize,
        index_value: u32,
        vertex_count: u32,
    },
    EdgeIndexOutOfRange {
        edge_position: usize,
        start: u32,
        end: u32,
        vertex_count: u32,
    },
    DegenerateEdge {
        edge_position: usize,
    },
    NonFiniteVertex {
        vertex_position: usize,
    },
}

impl MeshPrimitiveError {
    pub fn remediation_hint(&self) -> &'static str {
        match self {
            Self::EmptyVertices => "Provide at least one mesh vertex before issuing draw_mesh.",
            Self::EmptyIndices => "Provide triangle indices for mesh rasterization.",
            Self::InvalidTriangleIndexCount { .. } => {
                "Triangle list indices must be a multiple of 3."
            }
            Self::IndexOutOfRange { .. } => {
                "Ensure all indices reference valid vertices within mesh bounds."
            }
            Self::EdgeIndexOutOfRange { .. } => "Ensure edge index pairs reference valid vertices.",
            Self::DegenerateEdge { .. } => "Ensure each edge pair uses distinct vertex indices.",
            Self::NonFiniteVertex { .. } => {
                "Normalize mesh vertex values to finite position/normal/color components."
            }
        }
    }
}

impl fmt::Display for MeshPrimitiveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyVertices => write!(f, "mesh primitive requires at least one vertex"),
            Self::EmptyIndices => write!(f, "mesh primitive requires triangle indices"),
            Self::InvalidTriangleIndexCount { count } => {
                write!(f, "mesh triangle index count {count} is not divisible by 3")
            }
            Self::IndexOutOfRange {
                index_position,
                index_value,
                vertex_count,
            } => write!(
                f,
                "mesh index {index_position} has value {index_value} which exceeds vertex_count {vertex_count}"
            ),
            Self::EdgeIndexOutOfRange {
                edge_position,
                start,
                end,
                vertex_count,
            } => write!(
                f,
                "mesh edge {edge_position} has out-of-range vertices {start}->{end}; vertex_count={vertex_count}"
            ),
            Self::DegenerateEdge { edge_position } => {
                write!(f, "mesh edge {edge_position} is degenerate")
            }
            Self::NonFiniteVertex { vertex_position } => {
                write!(f, "mesh vertex {vertex_position} has non-finite components")
            }
        }
    }
}

impl Error for MeshPrimitiveError {}

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
    pub clip_origin: [f32; 2],
    pub clip_size: [f32; 2],
}

impl GpuTextQuad {
    /// Create a GPU text quad from a glyph instance.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn from_glyph(
        glyph: &GlyphInstance,
        origin: Point,
        color: Hsla,
        clip: Option<Bounds>,
        scale_factor: f32,
    ) -> Self {
        let (clip_origin, clip_size) = clip_to_gpu(clip, scale_factor);
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
            clip_origin,
            clip_size,
        }
    }
}

#[derive(Default)]
pub struct Scene {
    pub quads: Vec<(u32, Quad, Option<Bounds>)>, // (layer, quad, clip)
    pub text_runs: Vec<(u32, TextRun, Option<Bounds>)>, // (layer, text_run, clip)
    pub curves: Vec<(u32, CurvePrimitive)>,      // (layer, curve)
    pub meshes: Vec<(u32, MeshPrimitive, Option<Bounds>)>, // (layer, mesh, clip)
    pub images: Vec<(u32, ImageQuad, Option<Bounds>)>, // (layer, image, clip)
    pub vector_batches: Vec<(u32, VectorBatch, Option<Bounds>)>, // (layer, batch, clip)
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
        self.meshes.clear();
        self.images.clear();
        self.vector_batches.clear();
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
                self.quads.push((self.current_layer, quad, Some(*clip)));
            }
        } else {
            self.quads.push((self.current_layer, quad, None));
        }
    }

    pub fn draw_text(&mut self, text_run: TextRun) {
        if let Some(clip) = self.clip_stack.last() {
            if text_run.bounds().intersects(clip) {
                self.text_runs
                    .push((self.current_layer, text_run, Some(*clip)));
            }
        } else {
            self.text_runs.push((self.current_layer, text_run, None));
        }
    }

    pub fn draw_image(&mut self, image: ImageQuad) {
        if let Some(clip) = self.clip_stack.last() {
            if image.bounds.intersects(clip) {
                self.images.push((self.current_layer, image, Some(*clip)));
            }
        } else {
            self.images.push((self.current_layer, image, None));
        }
    }

    /// Draw an SVG at the specified bounds.
    pub fn draw_svg(&mut self, svg: SvgQuad) {
        if let Some(clip) = self.clip_stack.last() {
            if svg.bounds.intersects(clip) {
                self.images
                    .push((self.current_layer, svg.clone().into(), Some(*clip)));
                self.svg_quads.push(svg);
            }
        } else {
            self.images
                .push((self.current_layer, svg.clone().into(), None));
            self.svg_quads.push(svg);
        }
    }

    pub fn draw_vector_batch(&mut self, batch: VectorBatch) {
        if let Some(clip) = self.clip_stack.last() {
            if batch.bounds.intersects(clip) {
                self.vector_batches
                    .push((self.current_layer, batch, Some(*clip)));
            }
        } else {
            self.vector_batches.push((self.current_layer, batch, None));
        }
    }

    /// Draw a bezier curve.
    pub fn draw_curve(&mut self, curve: CurvePrimitive) {
        // TODO: Add clipping support for curves
        self.curves.push((self.current_layer, curve));
    }

    /// Draw a generic mesh primitive.
    pub fn draw_mesh(&mut self, mesh: MeshPrimitive) -> Result<(), MeshPrimitiveError> {
        mesh.validate()?;
        if let Some(clip) = self.clip_stack.last() {
            let mesh_bounds = mesh.bounds().unwrap_or(Bounds::ZERO);
            if mesh_bounds.intersects(clip) {
                self.meshes.push((self.current_layer, mesh, Some(*clip)));
            }
        } else {
            self.meshes.push((self.current_layer, mesh, None));
        }
        Ok(())
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
            .filter(|(l, _, _)| *l == layer)
            .map(|(_, q, clip)| GpuQuad::from_quad(q, *clip, scale_factor))
            .collect()
    }

    /// Get GPU text quads for a specific layer.
    /// This is the GPU boundary where we scale from logical to physical pixels.
    pub fn gpu_text_quads_for_layer(&self, layer: u32, scale_factor: f32) -> Vec<GpuTextQuad> {
        let mut quads = Vec::new();
        for (l, run, clip) in &self.text_runs {
            if *l != layer {
                continue;
            }
            for glyph in &run.glyphs {
                quads.push(GpuTextQuad::from_glyph(
                    glyph,
                    run.origin,
                    run.color,
                    *clip,
                    scale_factor,
                ));
            }
        }
        quads
    }

    pub fn gpu_image_quads_for_layer(&self, layer: u32, scale_factor: f32) -> Vec<GpuImageQuad> {
        self.images
            .iter()
            .filter(|(l, _, _)| *l == layer)
            .map(|(_, image, clip)| GpuImageQuad::from_image(image, *clip, scale_factor))
            .collect()
    }

    /// Get all unique layers used in this scene, sorted.
    pub fn layers(&self) -> Vec<u32> {
        let mut layers: Vec<u32> = self
            .quads
            .iter()
            .map(|(l, _, _)| *l)
            .chain(self.text_runs.iter().map(|(l, _, _)| *l))
            .chain(self.curves.iter().map(|(l, _)| *l))
            .chain(self.meshes.iter().map(|(l, _, _)| *l))
            .chain(self.images.iter().map(|(l, _, _)| *l))
            .chain(self.vector_batches.iter().map(|(l, _, _)| *l))
            .collect();
        layers.sort_unstable();
        layers.dedup();
        layers
    }

    pub fn quads(&self) -> Vec<&Quad> {
        self.quads.iter().map(|(_, q, _)| q).collect()
    }

    pub fn text_runs(&self) -> Vec<&TextRun> {
        self.text_runs.iter().map(|(_, r, _)| r).collect()
    }

    pub fn images_for_layer(&self, layer: u32) -> Vec<&ImageQuad> {
        self.images
            .iter()
            .filter(|(l, _, _)| *l == layer)
            .map(|(_, image, _)| image)
            .collect()
    }

    pub fn svg_quads(&self) -> &[SvgQuad] {
        &self.svg_quads
    }

    pub fn vector_batches_for_layer(&self, layer: u32) -> Vec<&VectorBatch> {
        self.vector_batches
            .iter()
            .filter(|(l, _, _)| *l == layer)
            .map(|(_, batch, _)| batch)
            .collect()
    }

    pub fn mesh_primitives_for_layer(&self, layer: u32) -> Vec<&MeshPrimitive> {
        self.meshes
            .iter()
            .filter(|(l, _, _)| *l == layer)
            .map(|(_, mesh, _)| mesh)
            .collect()
    }

    pub fn meshes(&self) -> Vec<&MeshPrimitive> {
        self.meshes.iter().map(|(_, mesh, _)| mesh).collect()
    }
}

fn clip_to_gpu(clip: Option<Bounds>, scale_factor: f32) -> ([f32; 2], [f32; 2]) {
    match clip {
        Some(bounds) => (
            [
                bounds.origin.x * scale_factor,
                bounds.origin.y * scale_factor,
            ],
            [
                bounds.size.width * scale_factor,
                bounds.size.height * scale_factor,
            ],
        ),
        None => ([0.0, 0.0], [-1.0, -1.0]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::{ImageData, ImageSource};
    use std::sync::Arc;

    fn sample_mesh() -> MeshPrimitive {
        let vertices = vec![
            MeshVertex::new([0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.4, 0.7, 0.9, 1.0]),
            MeshVertex::new([10.0, 0.0, 0.0], [0.0, 0.0, 1.0], [0.4, 0.7, 0.9, 1.0]),
            MeshVertex::new([10.0, 10.0, 0.0], [0.0, 0.0, 1.0], [0.4, 0.7, 0.9, 1.0]),
            MeshVertex::new([0.0, 10.0, 0.0], [0.0, 0.0, 1.0], [0.4, 0.7, 0.9, 1.0]),
        ];
        MeshPrimitive::new(vertices, vec![0, 1, 2, 0, 2, 3])
            .with_edges(vec![MeshEdge::new(0, 1), MeshEdge::new(1, 2)])
    }

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
    fn test_layered_images_participate_in_scene_layers() {
        let mut scene = Scene::new();
        scene.set_layer(4);
        scene.draw_svg(SvgQuad::new(
            Bounds::new(16.0, 16.0, 24.0, 24.0),
            Arc::<[u8]>::from(&b"<svg></svg>"[..]),
        ));

        scene.set_layer(2);
        scene.draw_image(ImageQuad::new(
            Bounds::new(4.0, 4.0, 12.0, 12.0),
            ImageSource::Rgba8(
                ImageData::rgba8(1, 1, Arc::<[u8]>::from([255, 255, 255, 255]))
                    .expect("1x1 RGBA data should validate"),
            ),
        ));

        scene.set_layer(1);
        scene.draw_quad(Quad::new(Bounds::new(0.0, 0.0, 8.0, 8.0)));

        assert_eq!(scene.layers(), vec![1, 2, 4]);
        assert_eq!(scene.images_for_layer(2).len(), 1);
        assert_eq!(scene.images_for_layer(4).len(), 1);
    }

    #[test]
    fn test_scene_draw_image_is_clipped_like_other_drawables() {
        let mut scene = Scene::new();
        scene.push_clip(Bounds::new(0.0, 0.0, 20.0, 20.0));
        scene.draw_image(ImageQuad::new(
            Bounds::new(4.0, 4.0, 8.0, 8.0),
            ImageSource::Rgba8(
                ImageData::rgba8(1, 1, Arc::<[u8]>::from([255, 0, 0, 255]))
                    .expect("1x1 RGBA data should validate"),
            ),
        ));
        scene.draw_image(ImageQuad::new(
            Bounds::new(40.0, 40.0, 8.0, 8.0),
            ImageSource::Rgba8(
                ImageData::rgba8(1, 1, Arc::<[u8]>::from([0, 0, 255, 255]))
                    .expect("1x1 RGBA data should validate"),
            ),
        ));

        let images = scene.images_for_layer(0);
        assert_eq!(images.len(), 1);

        let gpu_images = scene.gpu_image_quads_for_layer(0, 2.0);
        assert_eq!(gpu_images.len(), 1);
        assert_eq!(gpu_images[0].clip_origin, [0.0, 0.0]);
        assert_eq!(gpu_images[0].clip_size, [40.0, 40.0]);
    }

    #[test]
    fn test_scene_draw_vector_batch_is_layered_and_clipped() {
        let mut scene = Scene::new();
        scene.set_layer(7);
        scene.draw_vector_batch(VectorBatch::new(Bounds::new(8.0, 8.0, 24.0, 24.0)));
        assert_eq!(scene.layers(), vec![7]);
        assert_eq!(scene.vector_batches_for_layer(7).len(), 1);

        scene.push_clip(Bounds::new(100.0, 100.0, 8.0, 8.0));
        scene.draw_vector_batch(VectorBatch::new(Bounds::new(0.0, 0.0, 20.0, 20.0)));
        assert_eq!(scene.vector_batches_for_layer(7).len(), 1);
    }

    #[test]
    fn test_mesh_validation_rejects_out_of_range_indices() {
        let mesh = MeshPrimitive::new(
            vec![
                MeshVertex::new([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
                MeshVertex::new([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 1.0, 1.0, 1.0]),
            ],
            vec![0, 1, 2],
        );
        let error = mesh.validate().expect_err("invalid index must fail");
        assert_eq!(
            error,
            MeshPrimitiveError::IndexOutOfRange {
                index_position: 2,
                index_value: 2,
                vertex_count: 2,
            }
        );
        assert_eq!(
            error.remediation_hint(),
            "Ensure all indices reference valid vertices within mesh bounds."
        );
    }

    #[test]
    fn test_scene_draw_mesh_is_layered_and_clipped() {
        let mut scene = Scene::new();
        scene.set_layer(3);
        scene
            .draw_mesh(sample_mesh())
            .expect("mesh should validate and draw");
        assert_eq!(scene.layers(), vec![3]);
        assert_eq!(scene.mesh_primitives_for_layer(3).len(), 1);

        scene.push_clip(Bounds::new(100.0, 100.0, 10.0, 10.0));
        scene
            .draw_mesh(sample_mesh())
            .expect("mesh validation should still succeed");
        assert_eq!(
            scene.mesh_primitives_for_layer(3).len(),
            1,
            "out-of-clip mesh should not be retained"
        );
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
        let gpu_quad = GpuQuad::from_quad(&quad, None, 1.0);

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
        let gpu_quad = GpuQuad::from_quad(&quad, None, 2.0);

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
        let gpu_quad =
            GpuTextQuad::from_glyph(&glyph, Point::new(10.0, 20.0), Hsla::white(), None, 1.0);

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
        let gpu_quad =
            GpuTextQuad::from_glyph(&glyph, Point::new(10.0, 20.0), Hsla::white(), None, 2.0);

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
        scene.draw_image(ImageQuad::new(
            Bounds::new(0.0, 0.0, 8.0, 8.0),
            ImageSource::Rgba8(
                ImageData::rgba8(1, 1, Arc::<[u8]>::from([255, 255, 255, 255]))
                    .expect("1x1 RGBA data should validate"),
            ),
        ));
        scene.draw_vector_batch(VectorBatch::new(Bounds::new(0.0, 0.0, 8.0, 8.0)));
        scene.push_clip(Bounds::new(0.0, 0.0, 50.0, 50.0));

        scene.clear();

        assert!(scene.quads().is_empty());
        assert!(scene.text_runs().is_empty());
        assert!(scene.images_for_layer(0).is_empty());
        assert!(scene.vector_batches_for_layer(0).is_empty());
        assert!(scene.current_clip().is_none());
    }
}
