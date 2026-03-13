use crate::color::Hsla;
use crate::geometry::{Bounds, Point};
use std::sync::Arc;

#[derive(Clone, Debug, PartialEq)]
pub struct ImageData {
    pub width: u32,
    pub height: u32,
    pub rgba8: Arc<[u8]>,
}

impl ImageData {
    pub fn rgba8(width: u32, height: u32, rgba8: Arc<[u8]>) -> Option<Self> {
        let expected_len = width.checked_mul(height)?.checked_mul(4)? as usize;
        if rgba8.len() != expected_len {
            return None;
        }
        Some(Self {
            width,
            height,
            rgba8,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ImageSource {
    SvgBytes(Arc<[u8]>),
    Rgba8(ImageData),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ImageQuad {
    pub bounds: Bounds,
    pub source: ImageSource,
    pub uv: [f32; 4],
    pub tint: Option<Hsla>,
    pub opacity: f32,
}

impl ImageQuad {
    pub fn new(bounds: Bounds, source: ImageSource) -> Self {
        Self {
            bounds,
            source,
            uv: [0.0, 0.0, 1.0, 1.0],
            tint: None,
            opacity: 1.0,
        }
    }

    pub fn with_tint(mut self, color: Hsla) -> Self {
        self.tint = Some(color);
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    pub fn with_uv(mut self, uv: [f32; 4]) -> Self {
        self.uv = uv;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SvgQuad {
    pub bounds: Bounds,
    pub svg_data: Arc<[u8]>,
    pub tint: Option<Hsla>,
    pub opacity: f32,
}

impl SvgQuad {
    pub fn new(bounds: Bounds, svg_data: Arc<[u8]>) -> Self {
        Self {
            bounds,
            svg_data,
            tint: None,
            opacity: 1.0,
        }
    }

    pub fn with_tint(mut self, color: Hsla) -> Self {
        self.tint = Some(color);
        self
    }

    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }
}

impl From<SvgQuad> for ImageQuad {
    fn from(value: SvgQuad) -> Self {
        let mut image = Self::new(value.bounds, ImageSource::SvgBytes(value.svg_data));
        image.tint = value.tint;
        image.opacity = value.opacity;
        image
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorFillRule {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorStrokeJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorStrokeCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorBlendMode {
    SrcOver,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Multiply,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VectorPathElement {
    MoveTo(Point),
    LineTo(Point),
    CubicTo {
        out_handle: Point,
        in_handle: Point,
        to: Point,
    },
    Close,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorPath {
    pub fill_rule: VectorFillRule,
    pub elements: Vec<VectorPathElement>,
}

impl Default for VectorPath {
    fn default() -> Self {
        Self {
            fill_rule: VectorFillRule::NonZero,
            elements: Vec::new(),
        }
    }
}

impl VectorPath {
    pub fn new(fill_rule: VectorFillRule) -> Self {
        Self {
            fill_rule,
            elements: Vec::new(),
        }
    }

    pub fn move_to(&mut self, point: Point) {
        self.elements.push(VectorPathElement::MoveTo(point));
    }

    pub fn line_to(&mut self, point: Point) {
        self.elements.push(VectorPathElement::LineTo(point));
    }

    pub fn cubic_to(&mut self, out_handle: Point, in_handle: Point, to: Point) {
        self.elements.push(VectorPathElement::CubicTo {
            out_handle,
            in_handle,
            to,
        });
    }

    pub fn close(&mut self) {
        self.elements.push(VectorPathElement::Close);
    }

    pub fn bounds(&self) -> Option<Bounds> {
        let mut points = self.elements.iter().flat_map(|element| match element {
            VectorPathElement::MoveTo(point) | VectorPathElement::LineTo(point) => {
                vec![*point]
            }
            VectorPathElement::CubicTo {
                out_handle,
                in_handle,
                to,
            } => vec![*out_handle, *in_handle, *to],
            VectorPathElement::Close => Vec::new(),
        });

        let first = points.next()?;

        let mut min_x = first.x;
        let mut min_y = first.y;
        let mut max_x = first.x;
        let mut max_y = first.y;

        for point in points {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }

        Some(Bounds::new(min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorGradientStop {
    pub position: f32,
    pub color: Hsla,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VectorBrush {
    Solid(Hsla),
    LinearGradient {
        start: Point,
        end: Point,
        stops: Vec<VectorGradientStop>,
    },
    RadialGradient {
        center: Point,
        radius: f32,
        stops: Vec<VectorGradientStop>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VectorPaintStyle {
    Fill,
    Stroke,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorPaint {
    pub style: VectorPaintStyle,
    pub brush: VectorBrush,
    pub thickness: f32,
    pub join: VectorStrokeJoin,
    pub cap: VectorStrokeCap,
    pub blend_mode: VectorBlendMode,
    pub opacity: f32,
}

impl Default for VectorPaint {
    fn default() -> Self {
        Self {
            style: VectorPaintStyle::Fill,
            brush: VectorBrush::Solid(Hsla::white()),
            thickness: 1.0,
            join: VectorStrokeJoin::Miter,
            cap: VectorStrokeCap::Butt,
            blend_mode: VectorBlendMode::SrcOver,
            opacity: 1.0,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorImage {
    pub id: u64,
    pub data: ImageData,
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorImageMesh {
    pub image_id: u64,
    pub vertices: Vec<Point>,
    pub uvs: Vec<Point>,
    pub indices: Vec<u32>,
    pub blend_mode: VectorBlendMode,
    pub opacity: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum VectorCommand {
    StatePush,
    StatePop,
    Transform([f32; 6]),
    SetClip(VectorPath),
    DrawPath {
        path: VectorPath,
        paint: VectorPaint,
    },
    DrawImage {
        image_id: u64,
        blend_mode: VectorBlendMode,
        opacity: f32,
    },
    DrawImageMesh(VectorImageMesh),
}

#[derive(Clone, Debug, PartialEq)]
pub struct VectorBatch {
    pub bounds: Bounds,
    pub images: Vec<VectorImage>,
    pub commands: Vec<VectorCommand>,
}

impl VectorBatch {
    pub fn new(bounds: Bounds) -> Self {
        Self {
            bounds,
            images: Vec::new(),
            commands: Vec::new(),
        }
    }

    pub fn push_image(&mut self, image: VectorImage) {
        self.images.push(image);
    }

    pub fn push_command(&mut self, command: VectorCommand) {
        self.commands.push(command);
    }
}
