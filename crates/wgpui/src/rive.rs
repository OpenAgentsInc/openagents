use crate::components::{Component, ComponentId, EventContext, EventResult, PaintContext};
use crate::input::{InputEvent, MouseButton};
use crate::{
    Bounds, Hsla, ImageData, Point, Size, VectorBatch, VectorBlendMode, VectorBrush, VectorCommand,
    VectorFillRule, VectorGradientStop, VectorImage, VectorImageMesh, VectorPaint,
    VectorPaintStyle, VectorPath, VectorPathElement, VectorStrokeCap, VectorStrokeJoin,
};
use image::ImageReader;
use rive_rs::renderer::{
    BlendMode as RuntimeBlendMode, Buffer as RuntimeBuffer, BufferFlags, BufferType,
    Color as RuntimeColor, Gradient as RuntimeGradient, Image as RuntimeImage,
    Paint as RuntimePaint, PaintStyle as RuntimePaintStyle, Path as RuntimePath,
    Renderer as RuntimeRenderer, StrokeCap as RuntimeStrokeCap, StrokeJoin as RuntimeStrokeJoin,
};
use rive_rs::scene::Scene as RuntimeScene;
use rive_rs::{Artboard, File, Handle, Instantiate, LinearAnimation, StateMachine, Viewport};
use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::Duration;
use web_time::Instant;

/// Host-facing Rive input values.
#[derive(Clone, Debug, PartialEq)]
pub enum RiveInputValue {
    Bool(bool),
    Number(f32),
    Trigger,
}

/// Artboard/state-machine selector.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum RiveHandle {
    #[default]
    Default,
    Index(usize),
    Name(String),
}

impl RiveHandle {
    fn to_runtime_handle(&self) -> Handle {
        match self {
            Self::Default => Handle::Default,
            Self::Index(index) => Handle::Index(*index),
            Self::Name(name) => Handle::Name(Cow::Owned(name.clone())),
        }
    }
}

/// Surface fit mode applied to the rendered artboard.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RiveFitMode {
    #[default]
    Contain,
    Cover,
    Fill,
}

/// Lightweight metrics from the last emitted frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RiveMetrics {
    pub scene_name: String,
    pub artboard_size: Size,
    pub command_count: usize,
    pub image_count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiveError {
    UnsupportedVersion,
    Malformed,
    MissingArtboard,
    MissingScene,
}

impl std::fmt::Display for RiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnsupportedVersion => f.write_str("unsupported Rive version"),
            Self::Malformed => f.write_str("malformed Rive file"),
            Self::MissingArtboard => f.write_str("requested artboard was not found"),
            Self::MissingScene => f.write_str("requested state machine or animation was not found"),
        }
    }
}

impl std::error::Error for RiveError {}

impl From<rive_rs::Error> for RiveError {
    fn from(value: rive_rs::Error) -> Self {
        match value {
            rive_rs::Error::UnsupportedVersion => Self::UnsupportedVersion,
            rive_rs::Error::Malformed => Self::Malformed,
        }
    }
}

/// WGPUI-owned controller for a `.riv` asset.
///
/// The controller owns the runtime state and emits `VectorBatch` output that
/// can be pushed into `PaintContext.scene` or any other WGPUI scene.
pub struct RiveController {
    bytes: Arc<[u8]>,
    artboard_handle: RiveHandle,
    scene_handle: RiveHandle,
    fit_mode: RiveFitMode,
    paused: bool,
    viewport: Viewport,
    scene: SceneInstance,
    last_metrics: RiveMetrics,
}

impl RiveController {
    pub fn from_bytes(bytes: impl Into<Arc<[u8]>>) -> Result<Self, RiveError> {
        Self::from_bytes_with_handles(bytes, RiveHandle::Default, RiveHandle::Default)
    }

    pub fn from_bytes_with_handles(
        bytes: impl Into<Arc<[u8]>>,
        artboard_handle: RiveHandle,
        scene_handle: RiveHandle,
    ) -> Result<Self, RiveError> {
        let bytes = bytes.into();
        let scene = instantiate_scene(
            bytes.as_ref(),
            &artboard_handle.to_runtime_handle(),
            &scene_handle,
        )?;
        Ok(Self {
            bytes,
            artboard_handle,
            scene_handle,
            fit_mode: RiveFitMode::Contain,
            paused: false,
            viewport: Viewport::default(),
            scene,
            last_metrics: RiveMetrics::default(),
        })
    }

    pub fn is_paused(&self) -> bool {
        self.paused
    }

    pub fn play(&mut self) {
        self.paused = false;
    }

    pub fn pause(&mut self) {
        self.paused = true;
    }

    pub fn restart(&mut self) -> Result<(), RiveError> {
        self.scene = instantiate_scene(
            self.bytes.as_ref(),
            &self.artboard_handle.to_runtime_handle(),
            &self.scene_handle,
        )?;
        self.viewport = Viewport::default();
        Ok(())
    }

    pub fn set_artboard(&mut self, handle: RiveHandle) -> Result<(), RiveError> {
        self.artboard_handle = handle;
        self.restart()
    }

    pub fn set_state_machine(&mut self, handle: RiveHandle) -> Result<(), RiveError> {
        self.scene_handle = handle;
        self.restart()
    }

    pub fn set_fit_mode(&mut self, fit_mode: RiveFitMode) {
        self.fit_mode = fit_mode;
    }

    pub fn fit_mode(&self) -> RiveFitMode {
        self.fit_mode
    }

    pub fn set_input(&mut self, name: &str, value: RiveInputValue) -> bool {
        self.scene.set_input(name, value)
    }

    pub fn advance(&mut self, elapsed: Duration) -> bool {
        if self.paused {
            return false;
        }
        self.scene.advance_and_apply(elapsed)
    }

    pub fn render_batch(&mut self, bounds: Bounds) -> VectorBatch {
        self.update_viewport(bounds);
        let mut renderer = RiveVectorRenderer::new(bounds);
        renderer.state_push();
        renderer.transform(&view_transform(
            self.fit_mode,
            self.scene.artboard_size(),
            bounds,
        ));
        self.scene.draw(&mut renderer);
        renderer.state_pop();
        let batch = renderer.into_batch();
        self.update_metrics(&batch);
        batch
    }

    pub fn paint_into_scene(&mut self, bounds: Bounds, scene: &mut crate::Scene) -> RiveMetrics {
        let batch = self.render_batch(bounds);
        scene.draw_vector_batch(batch);
        self.last_metrics.clone()
    }

    pub fn pointer_down(&mut self, x: f32, y: f32, bounds: Bounds) {
        self.update_viewport(bounds);
        self.scene.pointer_down(x, y, &self.viewport);
    }

    pub fn pointer_move(&mut self, x: f32, y: f32, bounds: Bounds) {
        self.update_viewport(bounds);
        self.scene.pointer_move(x, y, &self.viewport);
    }

    pub fn pointer_up(&mut self, x: f32, y: f32, bounds: Bounds) {
        self.update_viewport(bounds);
        self.scene.pointer_up(x, y, &self.viewport);
    }

    pub fn metrics(&self) -> &RiveMetrics {
        &self.last_metrics
    }

    fn update_viewport(&mut self, bounds: Bounds) {
        let width = bounds.size.width.max(1.0).round() as u32;
        let height = bounds.size.height.max(1.0).round() as u32;
        self.viewport.resize(width, height);
        set_viewport_inverse(
            &mut self.viewport,
            invert_transform(view_transform(
                self.fit_mode,
                self.scene.artboard_size(),
                bounds,
            )),
        );
    }

    fn update_metrics(&mut self, batch: &VectorBatch) {
        self.last_metrics = RiveMetrics {
            scene_name: scene_handle_label(&self.scene_handle),
            artboard_size: batch.bounds.size,
            command_count: batch.commands.len(),
            image_count: batch.images.len(),
        };
    }
}

fn scene_handle_label(handle: &RiveHandle) -> String {
    match handle {
        RiveHandle::Default => "default".to_string(),
        RiveHandle::Index(index) => format!("index:{index}"),
        RiveHandle::Name(name) => name.clone(),
    }
}

/// `Component` wrapper around `RiveController`.
pub struct RiveSurface {
    id: Option<ComponentId>,
    controller: RiveController,
    last_paint: Option<Instant>,
    redraw_pending: bool,
    carry_redraws: u8,
    animating: bool,
    pointer_captured: bool,
}

impl RiveSurface {
    pub fn from_bytes(
        bytes: impl Into<Arc<[u8]>>,
        id: Option<ComponentId>,
    ) -> Result<Self, RiveError> {
        Self::from_bytes_with_handles(bytes, RiveHandle::Default, RiveHandle::Default, id)
    }

    pub fn from_bytes_with_handles(
        bytes: impl Into<Arc<[u8]>>,
        artboard_handle: RiveHandle,
        scene_handle: RiveHandle,
        id: Option<ComponentId>,
    ) -> Result<Self, RiveError> {
        Ok(Self {
            id,
            controller: RiveController::from_bytes_with_handles(
                bytes,
                artboard_handle,
                scene_handle,
            )?,
            last_paint: None,
            redraw_pending: true,
            carry_redraws: 0,
            animating: false,
            pointer_captured: false,
        })
    }

    pub fn controller(&self) -> &RiveController {
        &self.controller
    }

    pub fn controller_mut(&mut self) -> &mut RiveController {
        &mut self.controller
    }

    pub fn needs_redraw(&self) -> bool {
        self.redraw_pending || self.animating || self.carry_redraws > 0
    }

    pub fn is_animating(&self) -> bool {
        self.animating
    }

    pub fn is_settled(&self) -> bool {
        !self.needs_redraw()
    }

    pub fn has_pointer_capture(&self) -> bool {
        self.pointer_captured
    }

    pub fn mark_dirty(&mut self) {
        self.redraw_pending = true;
    }
}

impl Component for RiveSurface {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        let now = Instant::now();
        let first_paint = self.last_paint.is_none();
        let was_dirty = self.redraw_pending;
        let elapsed = self
            .last_paint
            .replace(now)
            .map_or(Duration::ZERO, |previous| now - previous);
        if self.carry_redraws > 0 {
            self.carry_redraws = self.carry_redraws.saturating_sub(1);
        }
        self.animating = self.controller.advance(elapsed);
        self.controller.paint_into_scene(bounds, cx.scene);
        self.redraw_pending = false;
        if !self.controller.is_paused() && !self.animating && (first_paint || was_dirty) {
            self.carry_redraws = self.carry_redraws.max(1);
        }
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        match event {
            InputEvent::MouseMove { x, y }
                if self.pointer_captured || bounds.contains(Point::new(*x, *y)) =>
            {
                self.controller.pointer_move(*x, *y, bounds);
                self.redraw_pending = true;
                EventResult::Handled
            }
            InputEvent::MouseDown {
                button: MouseButton::Left,
                x,
                y,
                ..
            } if bounds.contains(Point::new(*x, *y)) => {
                self.pointer_captured = true;
                self.controller.pointer_down(*x, *y, bounds);
                self.redraw_pending = true;
                EventResult::Handled
            }
            InputEvent::MouseUp {
                button: MouseButton::Left,
                x,
                y,
            } if self.pointer_captured || bounds.contains(Point::new(*x, *y)) => {
                self.pointer_captured = false;
                self.controller.pointer_up(*x, *y, bounds);
                self.redraw_pending = true;
                EventResult::Handled
            }
            _ => EventResult::Ignored,
        }
    }

    fn id(&self) -> Option<ComponentId> {
        self.id
    }
}

type RuntimeArtboard = Artboard<RiveVectorRenderer>;
type RuntimeFile = File<RiveVectorRenderer>;
type RuntimeStateMachine = StateMachine<RiveVectorRenderer>;
type RuntimeLinearAnimation = LinearAnimation<RiveVectorRenderer>;

enum SceneInstance {
    StateMachine(RuntimeStateMachine),
    LinearAnimation(RuntimeLinearAnimation),
}

impl SceneInstance {
    fn instantiate(artboard: &RuntimeArtboard, handle: &RiveHandle) -> Option<Self> {
        let runtime_handle = handle.to_runtime_handle();
        RuntimeStateMachine::instantiate(artboard, runtime_handle.clone())
            .map(Self::StateMachine)
            .or_else(|| {
                RuntimeLinearAnimation::instantiate(artboard, runtime_handle)
                    .map(Self::LinearAnimation)
            })
    }

    fn artboard_size(&self) -> Size {
        match self {
            Self::StateMachine(scene) => Size::new(scene.width(), scene.height()),
            Self::LinearAnimation(scene) => Size::new(scene.width(), scene.height()),
        }
    }

    fn advance_and_apply(&mut self, elapsed: Duration) -> bool {
        match self {
            Self::StateMachine(scene) => scene.advance_and_apply(elapsed),
            Self::LinearAnimation(scene) => scene.advance_and_apply(elapsed),
        }
    }

    fn draw(&self, renderer: &mut RiveVectorRenderer) {
        match self {
            Self::StateMachine(scene) => scene.draw(renderer),
            Self::LinearAnimation(scene) => scene.draw(renderer),
        }
    }

    fn pointer_down(&mut self, x: f32, y: f32, viewport: &Viewport) {
        match self {
            Self::StateMachine(scene) => scene.pointer_down(x, y, viewport),
            Self::LinearAnimation(scene) => scene.pointer_down(x, y, viewport),
        }
    }

    fn pointer_move(&mut self, x: f32, y: f32, viewport: &Viewport) {
        match self {
            Self::StateMachine(scene) => scene.pointer_move(x, y, viewport),
            Self::LinearAnimation(scene) => scene.pointer_move(x, y, viewport),
        }
    }

    fn pointer_up(&mut self, x: f32, y: f32, viewport: &Viewport) {
        match self {
            Self::StateMachine(scene) => scene.pointer_up(x, y, viewport),
            Self::LinearAnimation(scene) => scene.pointer_up(x, y, viewport),
        }
    }

    fn set_input(&mut self, name: &str, value: RiveInputValue) -> bool {
        match self {
            Self::StateMachine(scene) => match value {
                RiveInputValue::Bool(next) => scene
                    .get_bool(name)
                    .map(|mut input| {
                        input.set(next);
                        true
                    })
                    .unwrap_or(false),
                RiveInputValue::Number(next) => scene
                    .get_number(name)
                    .map(|mut input| {
                        input.set(next);
                        true
                    })
                    .unwrap_or(false),
                RiveInputValue::Trigger => scene
                    .get_trigger(name)
                    .map(|mut input| {
                        input.fire();
                        true
                    })
                    .unwrap_or(false),
            },
            Self::LinearAnimation(_) => false,
        }
    }
}

fn instantiate_scene(
    bytes: &[u8],
    artboard_handle: &Handle,
    scene_handle: &RiveHandle,
) -> Result<SceneInstance, RiveError> {
    let file = RuntimeFile::new(bytes)?;
    let artboard = RuntimeArtboard::instantiate(&file, artboard_handle.clone())
        .ok_or(RiveError::MissingArtboard)?;
    SceneInstance::instantiate(&artboard, scene_handle).ok_or(RiveError::MissingScene)
}

#[repr(C)]
struct ViewportRepr {
    width: u32,
    height: u32,
    inverse_view_transform: [f32; 6],
}

fn set_viewport_inverse(viewport: &mut Viewport, inverse_view_transform: [f32; 6]) {
    let repr = viewport as *mut Viewport as *mut ViewportRepr;
    unsafe {
        (*repr).inverse_view_transform = inverse_view_transform;
    }
}

fn view_transform(fit: RiveFitMode, artboard_size: Size, bounds: Bounds) -> [f32; 6] {
    let artboard_width = artboard_size.width.max(1.0);
    let artboard_height = artboard_size.height.max(1.0);
    let width = bounds.size.width.max(1.0);
    let height = bounds.size.height.max(1.0);
    match fit {
        RiveFitMode::Contain => {
            let scale = (width / artboard_width).min(height / artboard_height);
            let offset_x = bounds.origin.x + (width - artboard_width * scale) * 0.5;
            let offset_y = bounds.origin.y + (height - artboard_height * scale) * 0.5;
            [scale, 0.0, 0.0, scale, offset_x, offset_y]
        }
        RiveFitMode::Cover => {
            let scale = (width / artboard_width).max(height / artboard_height);
            let offset_x = bounds.origin.x + (width - artboard_width * scale) * 0.5;
            let offset_y = bounds.origin.y + (height - artboard_height * scale) * 0.5;
            [scale, 0.0, 0.0, scale, offset_x, offset_y]
        }
        RiveFitMode::Fill => {
            let scale_x = width / artboard_width;
            let scale_y = height / artboard_height;
            [scale_x, 0.0, 0.0, scale_y, bounds.origin.x, bounds.origin.y]
        }
    }
}

fn invert_transform(transform: [f32; 6]) -> [f32; 6] {
    let determinant = transform[0] * transform[3] - transform[1] * transform[2];
    if determinant.abs() <= f32::EPSILON {
        return [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
    }
    let inverse = determinant.recip();
    let a = transform[3] * inverse;
    let b = -transform[1] * inverse;
    let c = -transform[2] * inverse;
    let d = transform[0] * inverse;
    let tx = -(a * transform[4] + c * transform[5]);
    let ty = -(b * transform[4] + d * transform[5]);
    [a, b, c, d, tx, ty]
}

struct RiveBuffer {
    bytes: Vec<u8>,
}

impl RiveBuffer {
    fn f32_points(&self) -> Vec<Point> {
        self.bytes
            .chunks_exact(8)
            .map(|chunk| {
                let x = f32::from_ne_bytes(chunk[0..4].try_into().unwrap_or([0; 4]));
                let y = f32::from_ne_bytes(chunk[4..8].try_into().unwrap_or([0; 4]));
                Point::new(x, y)
            })
            .collect()
    }

    fn u16_indices(&self) -> Vec<u32> {
        self.bytes
            .chunks_exact(2)
            .map(|chunk| u16::from_ne_bytes(chunk.try_into().unwrap_or([0; 2])) as u32)
            .collect()
    }
}

impl RuntimeBuffer for RiveBuffer {
    fn new(_type: BufferType, _flags: BufferFlags, len_in_bytes: usize) -> Self {
        Self {
            bytes: vec![0; len_in_bytes],
        }
    }

    fn map(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    fn unmap(&mut self) {}
}

#[derive(Default)]
struct RivePathRecorder {
    path: VectorPath,
}

impl RuntimePath for RivePathRecorder {
    fn new(commands: &mut rive_rs::path::Commands, fill_rule: rive_rs::path::FillRule) -> Self {
        let mut recorder = Self {
            path: VectorPath::new(fill_rule_from_runtime(fill_rule)),
        };
        for (verb, points) in commands {
            match verb {
                rive_rs::path::Verb::Move => recorder.move_to(points[0].x, points[0].y),
                rive_rs::path::Verb::Line => recorder.line_to(points[0].x, points[0].y),
                rive_rs::path::Verb::Cubic => recorder.cubic_to(
                    points[0].x,
                    points[0].y,
                    points[1].x,
                    points[1].y,
                    points[2].x,
                    points[2].y,
                ),
                rive_rs::path::Verb::Close => recorder.close(),
            }
        }
        recorder
    }

    fn reset(&mut self) {
        self.path.elements.clear();
    }

    fn extend(&mut self, from: &Self, transform: &[f32; 6]) {
        for element in &from.path.elements {
            match element {
                VectorPathElement::MoveTo(point) => {
                    self.path.move_to(transform_point(*point, transform));
                }
                VectorPathElement::LineTo(point) => {
                    self.path.line_to(transform_point(*point, transform));
                }
                VectorPathElement::CubicTo {
                    out_handle,
                    in_handle,
                    to,
                } => {
                    self.path.cubic_to(
                        transform_point(*out_handle, transform),
                        transform_point(*in_handle, transform),
                        transform_point(*to, transform),
                    );
                }
                VectorPathElement::Close => self.path.close(),
            }
        }
    }

    fn set_fill_rule(&mut self, fill_rule: rive_rs::path::FillRule) {
        self.path.fill_rule = fill_rule_from_runtime(fill_rule);
    }

    fn move_to(&mut self, x: f32, y: f32) {
        self.path.move_to(Point::new(x, y));
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.path.line_to(Point::new(x, y));
    }

    fn cubic_to(&mut self, ox: f32, oy: f32, ix: f32, iy: f32, x: f32, y: f32) {
        self.path
            .cubic_to(Point::new(ox, oy), Point::new(ix, iy), Point::new(x, y));
    }

    fn close(&mut self) {
        self.path.close();
    }
}

#[derive(Clone)]
enum RiveGradient {
    Linear {
        start: Point,
        end: Point,
        stops: Vec<VectorGradientStop>,
    },
    Radial {
        center: Point,
        radius: f32,
        stops: Vec<VectorGradientStop>,
    },
}

impl RuntimeGradient for RiveGradient {
    fn new_linear(
        sx: f32,
        sy: f32,
        ex: f32,
        ey: f32,
        colors: &[RuntimeColor],
        stops: &[f32],
    ) -> Self {
        Self::Linear {
            start: Point::new(sx, sy),
            end: Point::new(ex, ey),
            stops: gradient_stops(colors, stops),
        }
    }

    fn new_radial(cx: f32, cy: f32, radius: f32, colors: &[RuntimeColor], stops: &[f32]) -> Self {
        Self::Radial {
            center: Point::new(cx, cy),
            radius,
            stops: gradient_stops(colors, stops),
        }
    }
}

#[derive(Default)]
struct RivePaintRecorder {
    paint: VectorPaint,
}

impl RuntimePaint for RivePaintRecorder {
    type Gradient = RiveGradient;

    fn set_style(&mut self, style: RuntimePaintStyle) {
        self.paint.style = match style {
            RuntimePaintStyle::Stroke => VectorPaintStyle::Stroke,
            RuntimePaintStyle::Fill => VectorPaintStyle::Fill,
        };
    }

    fn set_color(&mut self, color: RuntimeColor) {
        self.paint.brush = VectorBrush::Solid(hsla_from_runtime_color(color));
    }

    fn set_thickness(&mut self, thickness: f32) {
        self.paint.thickness = thickness;
    }

    fn set_join(&mut self, join: RuntimeStrokeJoin) {
        self.paint.join = match join {
            RuntimeStrokeJoin::Miter => VectorStrokeJoin::Miter,
            RuntimeStrokeJoin::Round => VectorStrokeJoin::Round,
            RuntimeStrokeJoin::Bevel => VectorStrokeJoin::Bevel,
        };
    }

    fn set_cap(&mut self, cap: RuntimeStrokeCap) {
        self.paint.cap = match cap {
            RuntimeStrokeCap::Butt => VectorStrokeCap::Butt,
            RuntimeStrokeCap::Round => VectorStrokeCap::Round,
            RuntimeStrokeCap::Square => VectorStrokeCap::Square,
        };
    }

    fn set_blend_mode(&mut self, blend_mode: RuntimeBlendMode) {
        self.paint.blend_mode = blend_mode_from_runtime(blend_mode);
    }

    fn set_gradient(&mut self, gradient: &Self::Gradient) {
        self.paint.brush = match gradient {
            RiveGradient::Linear { start, end, stops } => VectorBrush::LinearGradient {
                start: *start,
                end: *end,
                stops: stops.clone(),
            },
            RiveGradient::Radial {
                center,
                radius,
                stops,
            } => VectorBrush::RadialGradient {
                center: *center,
                radius: *radius,
                stops: stops.clone(),
            },
        };
    }

    fn invalidate_stroke(&mut self) {}
}

#[derive(Clone)]
struct RiveImageData {
    id: u64,
    data: ImageData,
}

impl RuntimeImage for RiveImageData {
    fn decode(data: &[u8]) -> Option<Self> {
        let image = ImageReader::new(std::io::Cursor::new(data))
            .with_guessed_format()
            .ok()?
            .decode()
            .ok()?
            .to_rgba8();
        let (width, height) = image.dimensions();
        let rgba = Arc::<[u8]>::from(image.into_raw());
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        data.hash(&mut hasher);
        Some(Self {
            id: hasher.finish(),
            data: ImageData::rgba8(width, height, rgba)?,
        })
    }
}

struct RiveVectorRenderer {
    batch: VectorBatch,
    image_ids: HashSet<u64>,
}

impl RiveVectorRenderer {
    fn new(bounds: Bounds) -> Self {
        Self {
            batch: VectorBatch::new(bounds),
            image_ids: HashSet::new(),
        }
    }

    fn into_batch(self) -> VectorBatch {
        self.batch
    }

    fn ensure_image(&mut self, image: &RiveImageData) {
        if self.image_ids.insert(image.id) {
            self.batch.push_image(VectorImage {
                id: image.id,
                data: image.data.clone(),
            });
        }
    }
}

impl RuntimeRenderer for RiveVectorRenderer {
    type Buffer = RiveBuffer;
    type Path = RivePathRecorder;
    type Paint = RivePaintRecorder;
    type Gradient = RiveGradient;
    type Image = RiveImageData;

    fn state_push(&mut self) {
        self.batch.push_command(VectorCommand::StatePush);
    }

    fn state_pop(&mut self) {
        self.batch.push_command(VectorCommand::StatePop);
    }

    fn transform(&mut self, transform: &[f32; 6]) {
        self.batch
            .push_command(VectorCommand::Transform(*transform));
    }

    fn set_clip(&mut self, path: &Self::Path) {
        self.batch
            .push_command(VectorCommand::SetClip(path.path.clone()));
    }

    fn draw_path(&mut self, path: &Self::Path, paint: &Self::Paint) {
        self.batch.push_command(VectorCommand::DrawPath {
            path: path.path.clone(),
            paint: paint.paint.clone(),
        });
    }

    fn draw_image(&mut self, image: &Self::Image, blend_mode: RuntimeBlendMode, opacity: f32) {
        self.ensure_image(image);
        self.batch.push_command(VectorCommand::DrawImage {
            image_id: image.id,
            blend_mode: blend_mode_from_runtime(blend_mode),
            opacity,
        });
    }

    fn draw_image_mesh(
        &mut self,
        image: &Self::Image,
        vertices: &Self::Buffer,
        uvs: &Self::Buffer,
        indices: &Self::Buffer,
        blend_mode: RuntimeBlendMode,
        opacity: f32,
    ) {
        self.ensure_image(image);
        self.batch
            .push_command(VectorCommand::DrawImageMesh(VectorImageMesh {
                image_id: image.id,
                vertices: vertices.f32_points(),
                uvs: uvs.f32_points(),
                indices: indices.u16_indices(),
                blend_mode: blend_mode_from_runtime(blend_mode),
                opacity,
            }));
    }
}

fn fill_rule_from_runtime(fill_rule: rive_rs::path::FillRule) -> VectorFillRule {
    match fill_rule {
        rive_rs::path::FillRule::NonZero => VectorFillRule::NonZero,
        rive_rs::path::FillRule::EvenOdd => VectorFillRule::EvenOdd,
    }
}

fn gradient_stops(colors: &[RuntimeColor], stops: &[f32]) -> Vec<VectorGradientStop> {
    colors
        .iter()
        .zip(stops.iter().copied())
        .map(|(color, position)| VectorGradientStop {
            position,
            color: hsla_from_runtime_color(*color),
        })
        .collect()
}

fn hsla_from_runtime_color(color: RuntimeColor) -> Hsla {
    Hsla::from_rgb(
        f32::from(color.r) / 255.0,
        f32::from(color.g) / 255.0,
        f32::from(color.b) / 255.0,
    )
    .with_alpha(f32::from(color.a) / 255.0)
}

fn blend_mode_from_runtime(blend_mode: RuntimeBlendMode) -> VectorBlendMode {
    match blend_mode {
        RuntimeBlendMode::SrcOver => VectorBlendMode::SrcOver,
        RuntimeBlendMode::Screen => VectorBlendMode::Screen,
        RuntimeBlendMode::Overlay => VectorBlendMode::Overlay,
        RuntimeBlendMode::Darken => VectorBlendMode::Darken,
        RuntimeBlendMode::Lighten => VectorBlendMode::Lighten,
        RuntimeBlendMode::ColorDodge => VectorBlendMode::ColorDodge,
        RuntimeBlendMode::ColorBurn => VectorBlendMode::ColorBurn,
        RuntimeBlendMode::HardLight => VectorBlendMode::HardLight,
        RuntimeBlendMode::SoftLight => VectorBlendMode::SoftLight,
        RuntimeBlendMode::Difference => VectorBlendMode::Difference,
        RuntimeBlendMode::Exclusion => VectorBlendMode::Exclusion,
        RuntimeBlendMode::Multiply => VectorBlendMode::Multiply,
        RuntimeBlendMode::Hue => VectorBlendMode::Hue,
        RuntimeBlendMode::Saturation => VectorBlendMode::Saturation,
        RuntimeBlendMode::Color => VectorBlendMode::Color,
        RuntimeBlendMode::Luminosity => VectorBlendMode::Luminosity,
    }
}

fn transform_point(point: Point, transform: &[f32; 6]) -> Point {
    Point::new(
        transform[0] * point.x + transform[2] * point.y + transform[4],
        transform[1] * point.x + transform[3] * point.y + transform[5],
    )
}

#[cfg(test)]
mod tests {
    use super::RiveHandle;
    use super::{RiveFitMode, invert_transform, scene_handle_label, view_transform};
    use crate::{
        Bounds, Component, EventContext, InputEvent, MouseButton, PaintContext, Scene, Size,
        TextSystem,
    };

    fn packaged_hud_bytes() -> &'static [u8] {
        include_bytes!("../../../apps/autopilot-desktop/resources/rive/simple-fui-hud.riv")
    }

    #[test]
    fn contain_fit_centers_artboard() {
        let transform = view_transform(
            RiveFitMode::Contain,
            Size::new(100.0, 50.0),
            Bounds::new(20.0, 10.0, 200.0, 200.0),
        );
        assert_eq!(transform[0], 2.0);
        assert_eq!(transform[3], 2.0);
        assert_eq!(transform[4], 20.0);
        assert_eq!(transform[5], 60.0);
    }

    #[test]
    fn invert_transform_round_trips_translation_and_scale() {
        let transform = [2.0, 0.0, 0.0, 3.0, 20.0, 12.0];
        let inverse = invert_transform(transform);
        assert!((inverse[0] - 0.5).abs() < 0.0001);
        assert!((inverse[3] - (1.0 / 3.0)).abs() < 0.0001);
        assert!((inverse[4] + 10.0).abs() < 0.0001);
        assert!((inverse[5] + 4.0).abs() < 0.0001);
    }

    #[test]
    fn scene_handle_label_formats_supported_handles() {
        assert_eq!(scene_handle_label(&RiveHandle::Default), "default");
        assert_eq!(scene_handle_label(&RiveHandle::Index(7)), "index:7");
        assert_eq!(
            scene_handle_label(&RiveHandle::Name("hud".to_string())),
            "hud"
        );
    }

    #[test]
    fn surface_event_capture_persists_until_mouse_up() {
        let mut surface =
            crate::RiveSurface::from_bytes(packaged_hud_bytes(), None).expect("packaged HUD asset");
        let bounds = Bounds::new(0.0, 0.0, 320.0, 200.0);
        let mut event_context = EventContext::new();

        assert!(
            surface
                .event(
                    &InputEvent::MouseDown {
                        button: MouseButton::Left,
                        x: 24.0,
                        y: 32.0,
                        modifiers: crate::Modifiers::default(),
                    },
                    bounds,
                    &mut event_context,
                )
                .is_handled()
        );
        assert!(surface.has_pointer_capture());
        assert!(surface.needs_redraw());
        assert!(
            surface
                .event(
                    &InputEvent::MouseMove { x: 420.0, y: 260.0 },
                    bounds,
                    &mut event_context,
                )
                .is_handled(),
            "captured move should continue forwarding beyond the canvas bounds"
        );
        assert!(
            surface
                .event(
                    &InputEvent::MouseUp {
                        button: MouseButton::Left,
                        x: 420.0,
                        y: 260.0,
                    },
                    bounds,
                    &mut event_context,
                )
                .is_handled(),
            "captured release should not be dropped outside the canvas bounds"
        );
        assert!(!surface.has_pointer_capture());
    }

    #[test]
    fn surface_settles_after_a_paused_paint() {
        let mut surface =
            crate::RiveSurface::from_bytes(packaged_hud_bytes(), None).expect("packaged HUD asset");
        surface.controller_mut().pause();
        let mut scene = Scene::new();
        let mut text_system = TextSystem::new(1.0);
        let mut paint_context = PaintContext::new(&mut scene, &mut text_system, 1.0);

        surface.paint(Bounds::new(0.0, 0.0, 320.0, 200.0), &mut paint_context);

        assert!(!surface.is_animating());
        assert!(surface.is_settled());
        assert!(!surface.needs_redraw());
    }
}
