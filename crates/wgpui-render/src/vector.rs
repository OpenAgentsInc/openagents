use tiny_skia::{
    BlendMode as TinyBlendMode, Color, FillRule as TinyFillRule, FilterQuality, GradientStop,
    IntSize, LineCap, LineJoin, LinearGradient, Mask, Paint, Path, PathBuilder, Pattern, Pixmap,
    PixmapPaint, Point as TinyPoint, RadialGradient, SpreadMode, Stroke, Transform,
};
use wgpui_core::vector::{
    ImageData, VectorBatch, VectorBlendMode, VectorBrush, VectorCommand, VectorFillRule,
    VectorPaint as SceneVectorPaint, VectorPaintStyle, VectorPath, VectorPathElement,
    VectorStrokeCap, VectorStrokeJoin,
};

#[derive(Clone, Debug, PartialEq)]
pub struct RasterizedVectorBatch {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

#[derive(Debug)]
struct RasterState {
    transform: Transform,
    clip_mask: Option<Mask>,
}

impl Clone for RasterState {
    fn clone(&self) -> Self {
        Self {
            transform: self.transform,
            clip_mask: self.clip_mask.as_ref().and_then(clone_mask),
        }
    }
}

pub fn rasterize_vector_batch(
    batch: &VectorBatch,
    scale_factor: f32,
) -> Option<RasterizedVectorBatch> {
    let width = (batch.bounds.size.width * scale_factor).ceil().max(1.0) as u32;
    let height = (batch.bounds.size.height * scale_factor).ceil().max(1.0) as u32;
    let mut pixmap = Pixmap::new(width, height)?;
    let root_transform = Transform::from_row(
        scale_factor,
        0.0,
        0.0,
        scale_factor,
        -batch.bounds.origin.x * scale_factor,
        -batch.bounds.origin.y * scale_factor,
    );
    let mut states = vec![RasterState {
        transform: root_transform,
        clip_mask: None,
    }];

    let images = batch
        .images
        .iter()
        .filter_map(|image| decode_image(&image.data).map(|pixmap| (image.id, pixmap)))
        .collect::<std::collections::HashMap<_, _>>();

    for command in &batch.commands {
        match command {
            VectorCommand::StatePush => {
                let next_state = states.last().cloned().unwrap_or(RasterState {
                    transform: root_transform,
                    clip_mask: None,
                });
                states.push(next_state);
            }
            VectorCommand::StatePop => {
                if states.len() > 1 {
                    states.pop();
                }
            }
            VectorCommand::Transform(transform) => {
                if let Some(state) = states.last_mut() {
                    state.transform = state.transform.pre_concat(transform_from_scene(*transform));
                }
            }
            VectorCommand::SetClip(path) => {
                if let Some(state) = states.last_mut() {
                    state.clip_mask = rasterize_clip_mask(path, width, height, state.transform);
                }
            }
            VectorCommand::DrawPath { path, paint } => {
                let Some(state) = states.last() else {
                    continue;
                };
                let Some(tiny_path) = path_from_scene(path) else {
                    continue;
                };
                let Some(tiny_paint) = paint_from_scene(paint, state.transform) else {
                    continue;
                };
                match paint.style {
                    VectorPaintStyle::Fill => {
                        pixmap.fill_path(
                            &tiny_path,
                            &tiny_paint,
                            fill_rule_from_scene(path.fill_rule),
                            state.transform,
                            state.clip_mask.as_ref(),
                        );
                    }
                    VectorPaintStyle::Stroke => {
                        let stroke = stroke_from_scene(paint);
                        pixmap.stroke_path(
                            &tiny_path,
                            &tiny_paint,
                            &stroke,
                            state.transform,
                            state.clip_mask.as_ref(),
                        );
                    }
                }
            }
            VectorCommand::DrawImage {
                image_id,
                blend_mode,
                opacity,
            } => {
                let Some(state) = states.last() else {
                    continue;
                };
                let Some(image) = images.get(image_id) else {
                    continue;
                };
                let mut paint = PixmapPaint::default();
                paint.opacity = opacity.clamp(0.0, 1.0);
                paint.blend_mode = blend_mode_from_scene(*blend_mode);
                paint.quality = FilterQuality::Bilinear;
                let image_transform = state.transform.pre_translate(
                    -(image.width() as f32) * 0.5,
                    -(image.height() as f32) * 0.5,
                );
                pixmap.draw_pixmap(
                    0,
                    0,
                    image.as_ref(),
                    &paint,
                    image_transform,
                    state.clip_mask.as_ref(),
                );
            }
            VectorCommand::DrawImageMesh(mesh) => {
                let Some(state) = states.last() else {
                    continue;
                };
                let Some(image) = images.get(&mesh.image_id) else {
                    continue;
                };
                for triangle in mesh.indices.chunks_exact(3) {
                    let Some(points) = triangle_points(&mesh.vertices, triangle) else {
                        continue;
                    };
                    let Some(uvs) = triangle_points(&mesh.uvs, triangle) else {
                        continue;
                    };
                    let transformed_points = points.map(|point| map_point(state.transform, point));
                    let Some(triangle_path) = triangle_path(transformed_points) else {
                        continue;
                    };
                    let mut paint = Paint::default();
                    paint.anti_alias = true;
                    paint.blend_mode = blend_mode_from_scene(mesh.blend_mode);
                    paint.shader = Pattern::new(
                        image.as_ref(),
                        SpreadMode::Pad,
                        FilterQuality::Bilinear,
                        mesh.opacity.clamp(0.0, 1.0),
                        pattern_transform(transformed_points, uvs, image.width(), image.height()),
                    );
                    pixmap.fill_path(
                        &triangle_path,
                        &paint,
                        TinyFillRule::Winding,
                        Transform::identity(),
                        state.clip_mask.as_ref(),
                    );
                }
            }
        }
    }

    Some(RasterizedVectorBatch {
        width,
        height,
        pixels: pixmap.take(),
    })
}

fn clone_mask(mask: &Mask) -> Option<Mask> {
    let size = IntSize::from_wh(mask.width(), mask.height())?;
    Mask::from_vec(mask.data().to_vec(), size)
}

fn decode_image(image: &ImageData) -> Option<Pixmap> {
    let size = IntSize::from_wh(image.width, image.height)?;
    Pixmap::from_vec(image.rgba8.to_vec(), size)
}

fn rasterize_clip_mask(
    path: &VectorPath,
    width: u32,
    height: u32,
    transform: Transform,
) -> Option<Mask> {
    let tiny_path = path_from_scene(path)?;
    let mut mask = Mask::new(width, height)?;
    mask.fill_path(
        &tiny_path,
        fill_rule_from_scene(path.fill_rule),
        true,
        transform,
    );
    Some(mask)
}

fn path_from_scene(path: &VectorPath) -> Option<Path> {
    let mut builder = PathBuilder::new();
    for element in &path.elements {
        match element {
            VectorPathElement::MoveTo(point) => builder.move_to(point.x, point.y),
            VectorPathElement::LineTo(point) => builder.line_to(point.x, point.y),
            VectorPathElement::CubicTo {
                out_handle,
                in_handle,
                to,
            } => builder.cubic_to(
                out_handle.x,
                out_handle.y,
                in_handle.x,
                in_handle.y,
                to.x,
                to.y,
            ),
            VectorPathElement::Close => builder.close(),
        }
    }
    builder.finish()
}

fn paint_from_scene(paint: &SceneVectorPaint, transform: Transform) -> Option<Paint<'static>> {
    let mut tiny_paint = Paint::default();
    tiny_paint.anti_alias = true;
    tiny_paint.blend_mode = blend_mode_from_scene(paint.blend_mode);

    match &paint.brush {
        VectorBrush::Solid(color) => {
            tiny_paint.set_color(hsla_to_tiny_color(*color, paint.opacity));
        }
        VectorBrush::LinearGradient { start, end, stops } => {
            let tiny_stops = gradient_stops(stops, paint.opacity);
            tiny_paint.shader = LinearGradient::new(
                TinyPoint::from_xy(start.x, start.y),
                TinyPoint::from_xy(end.x, end.y),
                tiny_stops,
                SpreadMode::Pad,
                transform,
            )?;
        }
        VectorBrush::RadialGradient {
            center,
            radius,
            stops,
        } => {
            let tiny_stops = gradient_stops(stops, paint.opacity);
            tiny_paint.shader = RadialGradient::new(
                TinyPoint::from_xy(center.x, center.y),
                TinyPoint::from_xy(center.x, center.y),
                *radius,
                tiny_stops,
                SpreadMode::Pad,
                transform,
            )?;
        }
    }

    Some(tiny_paint)
}

fn gradient_stops(
    stops: &[wgpui_core::vector::VectorGradientStop],
    opacity: f32,
) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|stop| GradientStop::new(stop.position, hsla_to_tiny_color(stop.color, opacity)))
        .collect()
}

fn stroke_from_scene(paint: &SceneVectorPaint) -> Stroke {
    let mut stroke = Stroke::default();
    stroke.width = paint.thickness.max(0.0);
    stroke.line_cap = match paint.cap {
        VectorStrokeCap::Butt => LineCap::Butt,
        VectorStrokeCap::Round => LineCap::Round,
        VectorStrokeCap::Square => LineCap::Square,
    };
    stroke.line_join = match paint.join {
        VectorStrokeJoin::Miter => LineJoin::Miter,
        VectorStrokeJoin::Round => LineJoin::Round,
        VectorStrokeJoin::Bevel => LineJoin::Bevel,
    };
    stroke
}

fn fill_rule_from_scene(fill_rule: VectorFillRule) -> TinyFillRule {
    match fill_rule {
        VectorFillRule::NonZero => TinyFillRule::Winding,
        VectorFillRule::EvenOdd => TinyFillRule::EvenOdd,
    }
}

fn blend_mode_from_scene(blend_mode: VectorBlendMode) -> TinyBlendMode {
    match blend_mode {
        VectorBlendMode::SrcOver => TinyBlendMode::SourceOver,
        VectorBlendMode::Screen => TinyBlendMode::Screen,
        VectorBlendMode::Overlay => TinyBlendMode::Overlay,
        VectorBlendMode::Darken => TinyBlendMode::Darken,
        VectorBlendMode::Lighten => TinyBlendMode::Lighten,
        VectorBlendMode::ColorDodge => TinyBlendMode::ColorDodge,
        VectorBlendMode::ColorBurn => TinyBlendMode::ColorBurn,
        VectorBlendMode::HardLight => TinyBlendMode::HardLight,
        VectorBlendMode::SoftLight => TinyBlendMode::SoftLight,
        VectorBlendMode::Difference => TinyBlendMode::Difference,
        VectorBlendMode::Exclusion => TinyBlendMode::Exclusion,
        VectorBlendMode::Multiply => TinyBlendMode::Multiply,
        VectorBlendMode::Hue => TinyBlendMode::Hue,
        VectorBlendMode::Saturation => TinyBlendMode::Saturation,
        VectorBlendMode::Color => TinyBlendMode::Color,
        VectorBlendMode::Luminosity => TinyBlendMode::Luminosity,
    }
}

fn transform_from_scene(transform: [f32; 6]) -> Transform {
    Transform::from_row(
        transform[0],
        transform[1],
        transform[2],
        transform[3],
        transform[4],
        transform[5],
    )
}

fn hsla_to_tiny_color(color: wgpui_core::Hsla, opacity: f32) -> Color {
    let rgba = color.to_rgba();
    let r = (rgba[0] * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (rgba[1] * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (rgba[2] * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (rgba[3] * opacity.clamp(0.0, 1.0) * 255.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    Color::from_rgba8(r, g, b, a)
}

fn triangle_points(
    points: &[wgpui_core::Point],
    triangle: &[u32],
) -> Option<[wgpui_core::Point; 3]> {
    Some([
        *points.get(*triangle.first()? as usize)?,
        *points.get(*triangle.get(1)? as usize)?,
        *points.get(*triangle.get(2)? as usize)?,
    ])
}

fn triangle_path(points: [wgpui_core::Point; 3]) -> Option<Path> {
    let mut builder = PathBuilder::new();
    builder.move_to(points[0].x, points[0].y);
    builder.line_to(points[1].x, points[1].y);
    builder.line_to(points[2].x, points[2].y);
    builder.close();
    builder.finish()
}

fn map_point(transform: Transform, point: wgpui_core::Point) -> wgpui_core::Point {
    let mut tiny_point = TinyPoint::from_xy(point.x, point.y);
    transform.map_point(&mut tiny_point);
    wgpui_core::Point::new(tiny_point.x, tiny_point.y)
}

fn pattern_transform(
    points: [wgpui_core::Point; 3],
    uvs: [wgpui_core::Point; 3],
    width: u32,
    height: u32,
) -> Transform {
    simplex_affine_mapping(
        uvs.map(|uv| Vector2 {
            x: uv.x * width as f32,
            y: uv.y * height as f32,
        }),
        points.map(|point| Vector2 {
            x: point.x,
            y: point.y,
        }),
    )
}

#[derive(Clone, Copy, Debug)]
struct Vector2 {
    x: f32,
    y: f32,
}

impl std::ops::Add for Vector2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl std::ops::Sub for Vector2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl std::ops::Mul<f32> for Vector2 {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

fn simplex_affine_mapping(from: [Vector2; 3], to: [Vector2; 3]) -> Transform {
    let [a, b, c] = from;
    let [d, e, f] = to;

    let det_recip = (a.x * b.y + b.x * c.y + c.x * a.y - a.x * c.y - b.x * a.y - c.x * b.y).recip();

    let p = (d * (b.y - c.y) - e * (a.y - c.y) + f * (a.y - b.y)) * det_recip;
    let q = (e * (a.x - c.x) - d * (b.x - c.x) - f * (a.x - b.x)) * det_recip;
    let t = (d * (b.x * c.y - b.y * c.x) - e * (a.x * c.y - a.y * c.x)
        + f * (a.x * b.y - a.y * b.x))
        * det_recip;

    Transform::from_row(p.x, p.y, q.x, q.y, t.x, t.y)
}

#[cfg(test)]
mod tests {
    use super::rasterize_vector_batch;
    use std::sync::Arc;
    use wgpui_core::vector::{
        ImageData, VectorBatch, VectorBlendMode, VectorBrush, VectorCommand, VectorFillRule,
        VectorGradientStop, VectorImage, VectorImageMesh, VectorPaint, VectorPaintStyle,
        VectorPath,
    };
    use wgpui_core::{Bounds, Hsla, Point};

    #[test]
    fn rasterize_vector_batch_renders_gradient_fill() {
        let mut path = VectorPath::new(VectorFillRule::NonZero);
        path.move_to(Point::new(10.0, 10.0));
        path.line_to(Point::new(110.0, 10.0));
        path.line_to(Point::new(110.0, 110.0));
        path.line_to(Point::new(10.0, 110.0));
        path.close();

        let mut batch = VectorBatch::new(Bounds::new(0.0, 0.0, 120.0, 120.0));
        batch.push_command(VectorCommand::DrawPath {
            path,
            paint: VectorPaint {
                style: VectorPaintStyle::Fill,
                brush: VectorBrush::LinearGradient {
                    start: Point::new(10.0, 10.0),
                    end: Point::new(110.0, 110.0),
                    stops: vec![
                        VectorGradientStop {
                            position: 0.0,
                            color: Hsla::from_hex(0xFF0000),
                        },
                        VectorGradientStop {
                            position: 1.0,
                            color: Hsla::from_hex(0x00FF00),
                        },
                    ],
                },
                ..VectorPaint::default()
            },
        });

        let rasterized = rasterize_vector_batch(&batch, 1.0).expect("batch should rasterize");
        assert_eq!(rasterized.width, 120);
        assert_eq!(rasterized.height, 120);
        assert!(
            rasterized.pixels.iter().any(|channel| *channel != 0),
            "gradient fill should produce visible pixels"
        );
    }

    #[test]
    fn rasterize_vector_batch_applies_clip_and_image_mesh() {
        let mut clip = VectorPath::new(VectorFillRule::NonZero);
        clip.move_to(Point::new(0.0, 0.0));
        clip.line_to(Point::new(64.0, 0.0));
        clip.line_to(Point::new(64.0, 64.0));
        clip.line_to(Point::new(0.0, 64.0));
        clip.close();

        let image = ImageData::rgba8(2, 2, Arc::<[u8]>::from(vec![255; 16]))
            .expect("2x2 RGBA data should validate");
        let mesh = VectorImageMesh {
            image_id: 7,
            vertices: vec![
                Point::new(8.0, 8.0),
                Point::new(56.0, 8.0),
                Point::new(56.0, 56.0),
                Point::new(8.0, 56.0),
            ],
            uvs: vec![
                Point::new(0.0, 0.0),
                Point::new(1.0, 0.0),
                Point::new(1.0, 1.0),
                Point::new(0.0, 1.0),
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
            blend_mode: VectorBlendMode::SrcOver,
            opacity: 1.0,
        };

        let mut batch = VectorBatch::new(Bounds::new(0.0, 0.0, 64.0, 64.0));
        batch.push_image(VectorImage { id: 7, data: image });
        batch.push_command(VectorCommand::SetClip(clip));
        batch.push_command(VectorCommand::DrawImageMesh(mesh));

        let rasterized = rasterize_vector_batch(&batch, 1.0).expect("batch should rasterize");
        assert_eq!(rasterized.width, 64);
        assert_eq!(rasterized.height, 64);
        assert!(
            rasterized.pixels.iter().any(|channel| *channel != 0),
            "image mesh should produce visible pixels"
        );
    }

    #[test]
    fn rasterize_vector_batch_honors_state_stack_transform_order() {
        let mut path = VectorPath::new(VectorFillRule::NonZero);
        path.move_to(Point::new(0.0, 0.0));
        path.line_to(Point::new(12.0, 0.0));
        path.line_to(Point::new(12.0, 12.0));
        path.line_to(Point::new(0.0, 12.0));
        path.close();

        let mut batch = VectorBatch::new(Bounds::new(0.0, 0.0, 64.0, 64.0));
        batch.push_command(VectorCommand::StatePush);
        batch.push_command(VectorCommand::Transform([1.0, 0.0, 0.0, 1.0, 28.0, 20.0]));
        batch.push_command(VectorCommand::DrawPath {
            path,
            paint: VectorPaint {
                style: VectorPaintStyle::Fill,
                brush: VectorBrush::Solid(Hsla::from_hex(0x00FF00)),
                ..VectorPaint::default()
            },
        });
        batch.push_command(VectorCommand::StatePop);

        let rasterized = rasterize_vector_batch(&batch, 1.0).expect("batch should rasterize");
        let translated_pixel = (32usize + 24usize * rasterized.width as usize) * 4;
        let original_pixel = (4usize + 4usize * rasterized.width as usize) * 4;

        assert!(
            rasterized.pixels[translated_pixel + 1] > 0,
            "translated draw should land inside the transformed region"
        );
        assert_eq!(
            rasterized.pixels[original_pixel + 1],
            0,
            "original region should remain empty after the transform stack is popped"
        );
    }
}
