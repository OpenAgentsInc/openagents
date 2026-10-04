//! Native mounting for an already authenticated chamber transport.
use super::{Renderer, WindowPresenter, chamber, controls, overlay};
use crate::{render, ui::Atlas};
use glam::Vec3;
use std::{
    collections::HashSet,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, oneshot};
use verse_engine::{assets::Pack, director::Scene};
use verse_world::{
    Intent,
    play::Ability,
    service::{
        client::Client,
        event_cursor::Cursor,
        view::{Camera, View},
        worker::{self, Input, Update},
    },
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

/// Runs on the main thread. The supplied runtime must own the client's transport.
/// Callers admit the scene and pack before mounting; this does not fetch assets.
pub fn run(
    client: Client,
    runtime: tokio::runtime::Runtime,
    pack: Pack,
    atlas: Atlas,
    scene: Scene,
    dir: PathBuf,
) -> Result<(), String> {
    scene.validate()?;
    let instance = client.instance();
    let view = View::new(instance, 10., 0)?;
    let cursor = Cursor::new(instance);
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let (input, inputs, updates, output) = worker::channels();
    let (stop, stopping) = oneshot::channel();
    let thread = std::thread::spawn(move || {
        runtime.block_on(worker::run(
            client,
            cursor,
            Duration::from_millis(33),
            inputs,
            updates,
            stopping,
        ))
    });
    let mut app = App::new(pack, atlas, scene, dir, view, input, output);
    let result = event_loop.run_app(&mut app).map_err(|e| e.to_string());
    let _ = stop.send(());
    let worker_result = thread
        .join()
        .map_err(|_| "Remote chamber worker panicked".to_string())?;
    result?;
    if let Some(error) = app.error {
        return Err(error);
    }
    worker_result
}
struct App {
    pack: Pack,
    atlas: Atlas,
    scene: Scene,
    dir: PathBuf,
    view: View,
    input: mpsc::Sender<Input>,
    output: mpsc::Receiver<Update>,
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    presenter: Option<WindowPresenter>,
    controls: controls::ClassicControls,
    camera: controls::Camera,
    yaw: f32,
    keys: HashSet<KeyCode>,
    pointer: [f32; 2],
    last: Instant,
    next_move: Instant,
    received_at: Instant,
    owned_life: Option<verse_engine::core::LifeId>,
    status: String,
    error: Option<String>,
}
impl App {
    fn new(
        pack: Pack,
        atlas: Atlas,
        scene: Scene,
        dir: PathBuf,
        view: View,
        input: mpsc::Sender<Input>,
        output: mpsc::Receiver<Update>,
    ) -> Self {
        Self {
            pack,
            atlas,
            scene,
            dir,
            view,
            input,
            output,
            window: None,
            renderer: None,
            presenter: None,
            controls: controls::ClassicControls::default(),
            camera: controls::Camera::default(),
            yaw: std::f32::consts::PI,
            keys: HashSet::new(),
            pointer: [0.; 2],
            last: Instant::now(),
            next_move: Instant::now(),
            received_at: Instant::now(),
            owned_life: None,
            status: String::new(),
            error: None,
        }
    }

    fn fail(&mut self, event_loop: &ActiveEventLoop, error: String) {
        self.error = Some(error);
        event_loop.exit();
    }
    fn unlocked(&self) -> bool {
        self.view.camera_handoff()
            || self
                .view
                .replica()
                .latest()
                .is_some_and(|s| s.presentation.time >= self.scene.cut_at)
    }
    fn controlled(&self) -> bool {
        self.unlocked()
            && self
                .view
                .replica()
                .latest()
                .and_then(|s| s.hud.as_ref())
                .is_some_and(|h| h.resources.hp > 0)
    }
    fn send(&mut self, input: Input) {
        match self.input.try_send(input) {
            Ok(()) => self.status.clear(),
            Err(mpsc::error::TrySendError::Full(_)) => self.status = "Input queue is busy".into(),
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.status = "Chamber connection stopped".into()
            }
        }
    }
    fn cast(&mut self, ability: Ability) {
        if !self.controlled() {
            return;
        }
        let target = self.view.target();
        let aim = target
            .and_then(|life| {
                self.view
                    .replica()
                    .latest()?
                    .presentation
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == life)
                    .map(|p| p.actor.position + Vec3::Y)
            })
            .unwrap_or_else(|| {
                self.view
                    .replica()
                    .latest()
                    .and_then(|s| s.hud.as_ref())
                    .and_then(|h| {
                        self.view
                            .replica()
                            .latest()?
                            .presentation
                            .actors
                            .iter()
                            .find(|p| verse_engine::core::LifeId::from(p.life) == h.life)
                    })
                    .map_or(Vec3::ZERO, |p| {
                        p.actor.position + self.camera.direction() * 20.
                    })
            });
        self.send(Input::Command(Intent::Cast {
            ability,
            target,
            aim: aim.to_array(),
        }));
    }
    fn consume(&mut self) -> Result<(), String> {
        for _ in 0..worker::UPDATE_CAPACITY {
            match self.output.try_recv() {
                Ok(Update::Snapshot(r)) => {
                    self.view.push_snapshot(&r)?;
                    self.received_at = Instant::now();
                    let life = self
                        .view
                        .replica()
                        .latest()
                        .and_then(|s| s.hud.as_ref())
                        .map(|h| h.life);
                    if life != self.owned_life {
                        self.owned_life = life;
                        if let Some(pose) = self.view.replica().latest().and_then(|s| {
                            s.presentation
                                .actors
                                .iter()
                                .find(|p| Some(verse_engine::core::LifeId::from(p.life)) == life)
                        }) {
                            self.yaw = pose.actor.yaw;
                            self.camera.yaw = self.yaw;
                        }
                        self.release_pointer();
                    }
                }
                Ok(Update::Events { delivery, .. }) => self.view.push_events(&delivery)?,
                Ok(Update::Outcome(r)) => {
                    if let verse_world::service::wire::Reply::Refused { message, .. } = r.body {
                        self.status = message;
                    }
                }
                Err(mpsc::error::TryRecvError::Empty) => break,
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    return Err("Chamber update stream stopped".into());
                }
            }
        }
        Ok(())
    }
    fn redraw(&mut self) -> Result<(), String> {
        self.consume()?;
        if !self.controlled()
            && (self.controls.looking() || self.controls.autorun || !self.keys.is_empty())
        {
            self.release_pointer();
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        let held = controls::Held {
            forward: self.keys.contains(&KeyCode::KeyW),
            backward: self.keys.contains(&KeyCode::KeyS),
            turn_left: self.keys.contains(&KeyCode::KeyA),
            turn_right: self.keys.contains(&KeyCode::KeyD),
            strafe_left: self.keys.contains(&KeyCode::KeyQ),
            strafe_right: self.keys.contains(&KeyCode::KeyE),
        };
        let axes = self
            .controls
            .step(held, dt, &mut self.yaw, &mut self.camera);
        if self.controlled()
            && now >= self.next_move
            && self.input.capacity() == worker::INPUT_CAPACITY
        {
            self.send(Input::Command(Intent::Move {
                axes,
                yaw: self.yaw,
            }));
            self.next_move = now + Duration::from_millis(33);
        }
        let size = self.window.as_ref().unwrap().inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let width = 720. * size.width as f32 / size.height as f32;
        let origin = verse_engine::source_position(self.scene.origin_wow);
        let time = self
            .view
            .replica()
            .latest()
            .map_or(0., |s| s.presentation.time);
        let alpha = (now.duration_since(self.received_at).as_secs_f32() / 0.033).clamp(0., 1.);
        let sampled = self.view.replica().sample(alpha)?;
        let time = sampled.as_ref().map_or(time, |s| s.time);
        let cinematic = self.scene.frame(time);
        let mut camera = Camera {
            eye: cinematic.eye,
            target: cinematic.target,
            fov: cinematic.fov,
        };
        let focus = self
            .view
            .replica()
            .latest()
            .and_then(|s| s.hud.as_ref())
            .and_then(|h| {
                sampled
                    .as_ref()?
                    .actors
                    .iter()
                    .find(|p| verse_engine::core::LifeId::from(p.life) == h.life)
            })
            .map_or(cinematic.target, |p| p.actor.position);
        if self.unlocked()
            && self
                .view
                .replica()
                .latest()
                .is_some_and(|s| s.hud.is_some())
        {
            camera.target = focus + Vec3::Y * 1.5;
            camera.eye = camera.target - self.camera.direction() * self.camera.distance.max(0.01);
            camera.fov = 60.;
        }
        let rendered = chamber::remote_scene(
            &self.pack,
            &self.view,
            alpha,
            camera,
            origin,
            self.scene.collision_profile.as_deref() == Some(verse_world::playground::PROFILE),
            focus,
        )?;
        let mut ui = crate::ui::UiBatch::default();
        let mut instances = vec![];
        let mut lighting = chamber::lighting(origin);
        let projection = glam::Mat4::perspective_rh(
            camera.fov.to_radians(),
            size.width as f32 / size.height as f32,
            0.1,
            1000.,
        ) * glam::Mat4::look_at_rh(camera.eye, camera.target, Vec3::Y);
        if let Some(rendered) = rendered {
            let heights = self
                .pack
                .models
                .iter()
                .map(|(id, m)| (id.clone(), m.height))
                .collect();
            ui = overlay::cinematic(
                &self.atlas,
                &rendered.frame,
                &heights,
                projection,
                width,
                720.,
            );
            overlay::damage_numbers_from_values(
                &mut ui,
                &self.atlas,
                &self.view.damage_numbers(alpha)?,
                rendered.frame.time,
                &rendered.frame,
                &heights,
                projection,
                width,
                720.,
            );
            if let Some(hud) = self.view.replica().latest().and_then(|s| s.hud.as_ref()) {
                overlay::owned_hud(
                    &mut ui,
                    &self.atlas,
                    hud,
                    &rendered.frame,
                    self.unlocked(),
                    width,
                    720.,
                )?;
            }
            if let Some(target) = self.view.target() {
                overlay::target_hud(&mut ui, &self.atlas, &rendered.frame, target, width, 720.)?;
            }
            instances = rendered.instances;
            lighting = rendered.lighting;
        } else {
            ui.text(&self.atlas, 20., 20., "Waiting for chamber state", [1.; 4]);
        }
        if !self.status.is_empty() {
            ui.text(&self.atlas, 20., 126., &self.status, [1., 0.8, 0.4, 1.]);
        }
        let renderer = self.renderer.as_mut().unwrap();
        renderer.resize(size.width, size.height)?;
        renderer.set_overlay_size(width, 720.);
        renderer.draw_live(
            render::View {
                view_proj: projection,
                eye: camera.eye,
            },
            &instances,
            &ui,
            &lighting,
        )?;
        renderer.present_window(self.presenter.as_mut().unwrap(), [size.width, size.height])
    }
    fn release_pointer(&mut self) {
        self.keys.clear();
        self.controls.clear();
        if let Some(window) = &self.window {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
        }
        if self.controlled() {
            self.send(Input::Command(Intent::Move {
                axes: [0.; 2],
                yaw: self.yaw,
            }));
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Verse Engine — Remote chamber")
                            .with_inner_size(winit::dpi::LogicalSize::new(1280., 720.)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let size = window.inner_size();
            let renderer = Renderer::new(
                self.pack.clone(),
                &self.dir,
                size.width.max(1),
                size.height.max(1),
                &self.atlas,
                &chamber::static_instances(
                    &self.pack,
                    verse_engine::source_position(self.scene.origin_wow),
                ),
            )?;
            let presenter = renderer.attach_window(window.clone())?;
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.presenter = Some(presenter);
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            self.fail(event_loop, error);
        }
    }
    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if self.controlled() {
                self.controls
                    .motion([delta.0, delta.1], &mut self.yaw, &mut self.camera);
            }
        }
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) => self.release_pointer(),
            WindowEvent::CursorMoved { position, .. } => {
                let size = self.window.as_ref().unwrap().inner_size();
                self.pointer = [
                    position.x as f32 * 720. / size.height.max(1) as f32,
                    position.y as f32 * 720. / size.height.max(1) as f32,
                ];
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    if event.state == ElementState::Released {
                        self.keys.remove(&key);
                    } else if !event.repeat {
                        self.keys.insert(key);
                        match key {
                            KeyCode::Escape => event_loop.exit(),
                            KeyCode::Tab => {
                                self.view.cycle_target();
                            }
                            KeyCode::Space if self.controlled() => {
                                self.send(Input::Command(Intent::Jump))
                            }
                            KeyCode::NumLock if self.controlled() => {
                                self.controls.autorun = !self.controls.autorun
                            }
                            _ => {
                                if let Some(ability) = key_ability(key) {
                                    self.cast(ability);
                                }
                            }
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.unlocked() => {
                self.camera.zoom(match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.,
                });
            }
            WindowEvent::MouseInput { state, button, .. } if self.unlocked() => {
                let down = state == ElementState::Pressed;
                if button == MouseButton::Left && down {
                    if let Some(hud) = self.view.replica().latest().and_then(|s| s.hud.as_ref()) {
                        let size = self.window.as_ref().unwrap().inner_size();
                        let width = 720. * size.width as f32 / size.height.max(1) as f32;
                        if overlay::owned_respawn_at(
                            hud,
                            true,
                            self.pointer[0],
                            self.pointer[1],
                            width,
                            720.,
                        ) {
                            self.send(Input::Respawn);
                            return;
                        }
                        if let Some(ability) = overlay::owned_action_at(
                            hud,
                            true,
                            self.pointer[0],
                            self.pointer[1],
                            width,
                            720.,
                        ) {
                            self.cast(ability);
                            return;
                        }
                    }
                }
                if self.controlled() && matches!(button, MouseButton::Left | MouseButton::Right) {
                    self.controls.button(
                        button == MouseButton::Right,
                        down,
                        &mut self.yaw,
                        &self.camera,
                    );
                    let window = self.window.as_ref().unwrap();
                    if self.controls.looking() {
                        let _ = window
                            .set_cursor_grab(CursorGrabMode::Locked)
                            .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
                        window.set_cursor_visible(false);
                    } else {
                        let _ = window.set_cursor_grab(CursorGrabMode::None);
                        window.set_cursor_visible(true);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(error) = self.redraw() {
                    self.fail(event_loop, error);
                }
            }
            _ => {}
        }
    }
    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
fn key_ability(key: KeyCode) -> Option<Ability> {
    let index = match key {
        KeyCode::Digit1 => 0,
        KeyCode::Digit2 => 1,
        KeyCode::Digit3 => 2,
        KeyCode::Digit4 => 3,
        KeyCode::Digit5 => 4,
        KeyCode::Digit6 => 5,
        KeyCode::Digit7 => 6,
        KeyCode::Digit8 => 7,
        KeyCode::Digit9 => 8,
        KeyCode::Digit0 => 9,
        _ => return None,
    };
    Some(Ability::ALL[index])
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_native_input_reports_pressure_and_closed_update_streams() {
        let dir = tempfile::tempdir().unwrap();
        let pack = super::super::original::generate(dir.path()).unwrap();
        let atlas = super::super::original::atlas().unwrap();
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let view = View::new(160, 10., 0).unwrap();
        let (input, mut inputs, updates, output) = worker::channels();
        let mut app = App::new(pack, atlas, scene, dir.path().into(), view, input, output);
        assert!(!app.controlled());
        app.cast(Ability::Fireball);
        assert!(inputs.try_recv().is_err());
        for _ in 0..worker::INPUT_CAPACITY {
            app.send(Input::Command(Intent::Jump));
        }
        app.send(Input::Command(Intent::Jump));
        assert_eq!(app.status, "Input queue is busy");
        for _ in 0..worker::INPUT_CAPACITY {
            assert!(matches!(
                inputs.try_recv(),
                Ok(Input::Command(Intent::Jump))
            ));
        }
        assert!(inputs.try_recv().is_err());
        drop(inputs);
        app.send(Input::Respawn);
        assert_eq!(app.status, "Chamber connection stopped");
        assert!(app.consume().is_ok());
        drop(updates);
        assert!(app.consume().is_err());
    }
    #[test]
    fn native_keys_cover_only_the_shared_host_kit() {
        let keys = [
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            KeyCode::Digit0,
        ];
        for (key, ability) in keys.into_iter().zip(Ability::ALL) {
            assert_eq!(key_ability(key), Some(ability));
        }
        assert_eq!(key_ability(KeyCode::F1), None);
    }
}
