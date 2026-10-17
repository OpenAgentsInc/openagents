//! A Lab card's 3D model (#11948): a Model3d card's glTF binary, turned by
//! dragging and drawn through `visual-gpu`, in place of its poster.
//!
//! The model goes into a one-line visual scene (`s:model`), so it is parsed,
//! fitted and lit exactly as an agent's visual would be. Frames render on a
//! background thread, on the GPU when there is one and in software
//! otherwise; the view shows the newest frame meanwhile.

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use gpui::{
    Context, InteractiveElement, IntoElement, MouseButton, ParentElement, Pixels, Point, Render,
    RenderImage, SharedString, Styled, StyledImage as _, Window, canvas, div, img, px,
};
use openagents_visual::{Camera, Color, NodeKind, Scene, Token};

use crate::theme::Theme;

/// The model's name in the scene's assets.
const ASSET: &str = openagents_account::lab_cards::MODEL_ASSET;
/// Radians per pixel dragged.
const TURN: f32 = 0.01;
/// The steepest the camera looks up or down.
const MAX_PITCH: f32 = 1.5;

/// The scene a model is shown in.
pub(crate) fn script() -> String {
    openagents_account::lab_cards::model_script(240)
}

/// The camera a drag turns: yaw and pitch in radians.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    /// Where a drag started, and the angles then.
    drag: Option<(Point<Pixels>, f32, f32)>,
}

impl Orbit {
    pub fn new(yaw: f32, pitch: f32) -> Self {
        Self {
            yaw,
            pitch: pitch.clamp(-MAX_PITCH, MAX_PITCH),
            drag: None,
        }
    }

    pub fn press(&mut self, at: Point<Pixels>) {
        self.drag = Some((at, self.yaw, self.pitch));
    }

    /// Turns to follow the pointer; false when no drag is under way.
    pub fn drag_to(&mut self, at: Point<Pixels>) -> bool {
        let Some((start, yaw, pitch)) = self.drag else {
            return false;
        };
        let delta = at - start;
        self.yaw = yaw - f32::from(delta.x) * TURN;
        self.pitch = (pitch + f32::from(delta.y) * TURN).clamp(-MAX_PITCH, MAX_PITCH);
        true
    }

    pub fn release(&mut self) {
        self.drag = None;
    }

    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
}

/// What a frame was drawn for.
#[derive(Clone, Copy, PartialEq)]
struct Key {
    yaw: f32,
    pitch: f32,
    size: (f32, f32),
    scale: f32,
}

pub struct LabModel {
    id: SharedString,
    scene: Arc<Scene>,
    zoom: f32,
    orbit: Orbit,
    shown: Option<(Key, Arc<RenderImage>)>,
    rendering: bool,
    gpu: Option<openagents_visual_gpu::SceneState>,
    /// The canvas size and density, from the last paint.
    size: Rc<Cell<Option<((f32, f32), f32)>>>,
    released: bool,
}

/// A model ready to show: its scene and the camera it starts from.
pub struct Parsed {
    scene: Arc<Scene>,
    zoom: f32,
    yaw: f32,
    pitch: f32,
}

/// The model in `bytes` as a scene. `None` when it isn't a model the visual
/// runtime can draw.
pub fn parse(bytes: Vec<u8>) -> Option<Parsed> {
    let assets = openagents_visual::Assets::new(HashMap::from([(
        ASSET.to_owned(),
        Arc::<[u8]>::from(bytes),
    )]));
    let mut visual = openagents_visual::Visual::load(&script(), 1)
        .ok()?
        .with_assets(assets);
    let tree = visual.render().ok()?;
    let NodeKind::Scene(node) = &tree.kind else {
        return None;
    };
    let scene = visual.draw_scene(node.draw, 0.0).ok()?;
    (scene.face_count() > 0).then(|| Parsed {
        scene: Arc::new(scene),
        zoom: node.zoom,
        yaw: node.yaw,
        pitch: node.pitch,
    })
}

impl LabModel {
    pub fn new(id: SharedString, parsed: Parsed, cx: &mut Context<Self>) -> Self {
        cx.on_release(|this, cx| {
            if let Some((_, image)) = this.shown.take() {
                cx.drop_image(image, None);
            }
        })
        .detach();
        Self {
            id,
            scene: parsed.scene,
            zoom: parsed.zoom,
            orbit: Orbit::new(parsed.yaw, parsed.pitch),
            shown: None,
            rendering: false,
            gpu: None,
            size: Rc::default(),
            released: false,
        }
    }

    pub fn orbit(&self) -> Orbit {
        self.orbit
    }

    /// Whether a frame has been drawn.
    pub fn drawn(&self) -> bool {
        self.shown.is_some()
    }

    pub fn press(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        self.orbit.press(at);
        cx.notify();
    }

    pub fn drag_to(&mut self, at: Point<Pixels>, cx: &mut Context<Self>) {
        if self.orbit.drag_to(at) {
            cx.notify();
        }
    }

    pub fn release_drag(&mut self, cx: &mut Context<Self>) {
        self.orbit.release();
        cx.notify();
    }

    /// Drops the frame; the model draws nothing after this.
    pub fn release(&mut self, cx: &mut Context<Self>) {
        self.released = true;
        if let Some((_, image)) = self.shown.take() {
            cx.drop_image(image, None);
        }
    }

    /// Renders a frame for the current camera when the shown one is stale.
    fn render_frame(&mut self, cx: &mut Context<Self>) {
        let Some((size, scale)) = self.size.get() else {
            return;
        };
        let key = Key {
            yaw: self.orbit.yaw,
            pitch: self.orbit.pitch,
            size,
            scale,
        };
        if self.released || self.rendering || self.shown.as_ref().is_some_and(|(k, _)| *k == key) {
            return;
        }
        self.rendering = true;
        let (scene, mut gpu) = (self.scene.clone(), self.gpu.take());
        let camera = Camera {
            yaw: key.yaw,
            pitch: key.pitch,
            zoom: self.zoom,
        };
        let text = Theme::of(cx).text.to_rgb();
        let fallback = [text.r, text.g, text.b];
        let job = cx.background_executor().spawn(async move {
            let rgba = move |color: Color| rgba(color, fallback);
            let (width, height, bgra) = match openagents_visual_gpu::render_scene(
                &scene, camera, size.0, size.1, scale, &rgba, &mut gpu,
            ) {
                Some(frame) => (frame.width, frame.height, frame.bgra),
                None => {
                    let raster = scene.render(
                        camera,
                        size.0,
                        size.1,
                        scale,
                        openagents_visual::Quality::Final,
                        &rgba,
                    );
                    (raster.width, raster.height, raster.bgra)
                }
            };
            let image = image::RgbaImage::from_raw(width, height, bgra)
                .map(|b| Arc::new(RenderImage::new([image::Frame::new(b)])));
            (image, gpu)
        });
        cx.spawn(async move |this, cx| {
            let (image, gpu) = job.await;
            let _ = this.update(cx, |this, cx| {
                this.rendering = false;
                this.gpu = gpu;
                if let Some(image) = image {
                    if this.released {
                        cx.drop_image(image, None);
                        return;
                    }
                    if let Some((_, old)) = this.shown.replace((key, image)) {
                        cx.drop_image(old, None);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }
}

/// A scene color as straight sRGB components: a model's own paint, else
/// the theme's text color.
fn rgba(color: Color, fallback: [f32; 3]) -> [f32; 4] {
    let [r, g, b] = match color.token {
        Token::Rgb(rgb) => [
            ((rgb >> 16) & 0xff) as f32 / 255.0,
            ((rgb >> 8) & 0xff) as f32 / 255.0,
            (rgb & 0xff) as f32 / 255.0,
        ],
        _ => fallback,
    };
    [r, g, b, color.alpha]
}

impl Render for LabModel {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.render_frame(cx);
        let size = self.size.clone();
        let entity = cx.entity().downgrade();
        let frame = self.shown.as_ref().map(|(_, image)| image.clone());
        div()
            .id(SharedString::from(format!("{}:model", self.id)))
            .relative()
            .w_full()
            .aspect_ratio(16.0 / 9.0)
            .rounded(px(Theme::CONTROL_RADIUS))
            .overflow_hidden()
            .bg(gpui::black().opacity(0.35))
            .cursor_grab()
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    cx.stop_propagation();
                    this.press(event.position, cx);
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                if this.orbit.dragging() {
                    this.drag_to(event.position, cx);
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| this.release_drag(cx)),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _: &gpui::MouseUpEvent, _, cx| this.release_drag(cx)),
            )
            .child(
                canvas(
                    move |bounds, window, cx| {
                        let now = (
                            (f32::from(bounds.size.width), f32::from(bounds.size.height)),
                            window.scale_factor(),
                        );
                        if size.get() != Some(now) {
                            size.set(Some(now));
                            let _ = entity.update(cx, |_, cx| cx.notify());
                        }
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .when_some(frame, |d, frame| {
                d.child(img(frame).object_fit(gpui::ObjectFit::Contain).size_full())
            })
    }
}

use gpui::prelude::FluentBuilder as _;

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// One red triangle as a binary glTF.
    pub(crate) fn triangle_glb() -> Vec<u8> {
        let positions: [f32; 9] = [0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0];
        let bin: Vec<u8> = positions.iter().flat_map(|f| f.to_le_bytes()).collect();
        let mut json = format!(
            r#"{{"asset":{{"version":"2.0"}},"scene":0,"scenes":[{{"nodes":[0]}}],
            "nodes":[{{"mesh":0}}],
            "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"material":0}}]}}],
            "materials":[{{"doubleSided":true,"pbrMetallicRoughness":{{"baseColorFactor":[1,0,0,1]}}}}],
            "accessors":[{{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}}],
            "bufferViews":[{{"buffer":0,"byteLength":{len}}}],
            "buffers":[{{"byteLength":{len}}}]}}"#,
            len = bin.len()
        )
        .into_bytes();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let total = 12 + 8 + json.len() + 8 + bin.len();
        let mut glb = Vec::with_capacity(total);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"BIN\0");
        glb.extend_from_slice(&bin);
        glb
    }

    #[test]
    fn dragging_turns_the_camera_and_pitch_stops_at_the_poles() {
        let mut orbit = Orbit::new(0.5, 0.2);
        assert!(!orbit.drag_to(gpui::point(px(10.), px(10.))), "no drag yet");
        orbit.press(gpui::point(px(100.), px(100.)));
        assert!(orbit.drag_to(gpui::point(px(150.), px(120.))));
        assert!((orbit.yaw - (0.5 - 0.5)).abs() < 1e-5, "{}", orbit.yaw);
        assert!((orbit.pitch - 0.4).abs() < 1e-5, "{}", orbit.pitch);
        assert!(orbit.drag_to(gpui::point(px(100.), px(100_000.))));
        assert_eq!(orbit.pitch, MAX_PITCH);
        orbit.release();
        assert!(!orbit.dragging());
        assert!(!orbit.drag_to(gpui::point(px(0.), px(0.))));
        assert_eq!(orbit.pitch, MAX_PITCH, "a released camera stays put");
    }

    #[test]
    fn the_model_scene_passes_the_visual_check() {
        let assets = openagents_visual::Assets::new(HashMap::from([(
            ASSET.to_owned(),
            Arc::<[u8]>::from(triangle_glb()),
        )]));
        openagents_visual::check_with_assets(&script(), 1, assets).unwrap();
    }

    #[gpui::test]
    fn a_model_draws_turns_with_the_pointer_and_junk_is_refused(cx: &mut gpui::TestAppContext) {
        cx.update(|cx| cx.set_global(Theme::default()));
        let parsed = parse(triangle_glb()).expect("a model");
        let (model, cx) = cx.add_window_view(|_, cx| LabModel::new("lab-model".into(), parsed, cx));
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
        });
        cx.run_until_parked();
        cx.update(|window, cx| {
            window.refresh();
            window.draw(cx).clear();
        });
        cx.run_until_parked();
        assert!(model.read_with(cx, |m, _| m.drawn()), "a frame lands");
        let before = model.read_with(cx, |m, _| m.orbit());
        model.update(cx, |m, cx| {
            m.press(gpui::point(px(10.), px(10.)), cx);
            m.drag_to(gpui::point(px(60.), px(10.)), cx);
            m.release_drag(cx);
        });
        let after = model.read_with(cx, |m, _| m.orbit());
        assert!((after.yaw - (before.yaw - 0.5)).abs() < 1e-5);
        assert_eq!(after.pitch, before.pitch);
        model.update(cx, |m, cx| m.release(cx));
        assert!(!model.read_with(cx, |m, _| m.drawn()));
        assert!(parse(b"not a model".to_vec()).is_none());
    }
}
