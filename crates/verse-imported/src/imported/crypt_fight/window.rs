//! The fight's desktop window: the ritual chamber's classic controls over
//! a local authority. Keys and buttons are the chamber window's
//! (`crates/verse/examples/chamber_app`): W, A, S, D, Q, and E move, the
//! right button steers and the left orbits, 1 through 0 cast the bar,
//! Shift with a digit casts the second row, Tab cycles targets, a click
//! selects one, Space jumps, the wheel zooms, F1 restarts the fight, Enter
//! or the button on the death screen returns to the landing, and Escape
//! closes the window.
use super::{Fight, overlay};
use std::{collections::HashSet, path::PathBuf, sync::Arc, time::Instant};
use verse_world::{
    controls::{ClassicControls, Held},
    play::Ability,
};
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

/// Builds the fight's content, then opens its window and plays until the
/// window closes.
///
/// # Errors
///
/// Returns a message when the content, the GPU, or the window fails.
pub fn run() -> Result<(), String> {
    let dir = std::env::temp_dir().join(format!("verse-great-crypt-{}", std::process::id()));
    eprintln!("verse: building the great crypt in {}", dir.display());
    let fight = Fight::load(&dir)?;
    let event_loop = EventLoop::new().map_err(|e| e.to_string())?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        fight,
        window: None,
        renderer: None,
        presenter: None,
        schedule: verse_engine::core::FixedSchedule::new(30, 3)?,
        interpolation: 1.0,
        last: Instant::now(),
        keys: HashSet::new(),
        controls: ClassicControls::default(),
        cursor: [0.0; 2],
        pointer: [0.0; 2],
        captured: false,
        raw_pointer: false,
        pending_select: false,
        dragged: 0.0,
        error: None,
        dir,
    };
    let result = event_loop.run_app(&mut app).map_err(|e| e.to_string());
    let _ = std::fs::remove_dir_all(&app.dir);
    result?;
    app.error.map_or(Ok(()), Err)
}

struct App {
    fight: Fight,
    window: Option<Arc<Window>>,
    renderer: Option<super::Renderer>,
    presenter: Option<super::super::WindowPresenter>,
    schedule: verse_engine::core::FixedSchedule,
    interpolation: f32,
    last: Instant,
    keys: HashSet<KeyCode>,
    controls: ClassicControls,
    cursor: [f32; 2],
    pointer: [f64; 2],
    captured: bool,
    raw_pointer: bool,
    pending_select: bool,
    dragged: f64,
    error: Option<String>,
    dir: PathBuf,
}

impl App {
    fn activate(&mut self, ability: Ability) {
        if let Err(e) = self.fight.game.activate(ability) {
            self.fight.game.message = e;
        }
    }

    fn overlay_size(&self) -> [f32; 2] {
        let size = self.window.as_ref().map_or([1280, 720], |w| {
            let s = w.inner_size();
            [s.width, s.height]
        });
        [720.0 * size[0].max(1) as f32 / size[1].max(1) as f32, 720.0]
    }

    fn respawn(&mut self) {
        match self.fight.game.respawn_player() {
            Ok(()) => {
                self.controls = ClassicControls::default();
                self.keys.clear();
                self.pending_select = false;
                self.dragged = 0.0;
            }
            Err(error) => self.fight.game.message = error,
        }
    }

    /// Restarts the fight, with the player (F1) or the chamber's combat
    /// agent (F2) in control.
    fn restart(&mut self, agent: bool) {
        if let Err(error) = self.fight.game.restart_combat(agent) {
            self.fight.game.message = error;
            return;
        }
        self.fight.game.camera.yaw = self.fight.game.yaw;
        self.controls.clear();
        self.keys.clear();
        self.pending_select = false;
        self.capture_pointer();
    }

    fn step(&mut self) -> Result<(), String> {
        let elapsed = self.last.elapsed().as_secs_f64();
        self.last = Instant::now();
        let key = |k| self.keys.contains(&k);
        let held = Held {
            forward: key(KeyCode::KeyW) || key(KeyCode::ArrowUp),
            backward: key(KeyCode::KeyS) || key(KeyCode::ArrowDown),
            turn_left: key(KeyCode::KeyA) || key(KeyCode::ArrowLeft),
            turn_right: key(KeyCode::KeyD) || key(KeyCode::ArrowRight),
            strafe_left: key(KeyCode::KeyQ),
            strafe_right: key(KeyCode::KeyE),
        };
        let batch = self.schedule.advance(elapsed)?;
        self.interpolation = batch.interpolation;
        let game = &mut self.fight.game;
        for _ in 0..batch.steps {
            let movement = if game.unlocked() {
                let mut yaw = game.yaw;
                let movement = self
                    .controls
                    .step(held, batch.seconds, &mut yaw, &mut game.camera);
                game.yaw = yaw;
                movement
            } else {
                [0.0; 2]
            };
            game.tick(batch.seconds, movement)?;
        }
        if game.snapshot().player.hp == 0 {
            self.controls = ClassicControls::default();
            self.capture_pointer();
        }
        Ok(())
    }

    fn draw(&mut self) -> Result<(), String> {
        let window = self.window.clone().ok_or("No window")?;
        let overlay = self.overlay_size();
        let renderer = self.renderer.as_mut().ok_or("No renderer")?;
        if renderer.recover_if_lost(&self.fight.atlas)? {
            self.presenter = Some(renderer.attach_window(window.clone())?);
        }
        let size = window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        renderer.resize(size.width, size.height)?;
        let composed = self.fight.compose(
            self.interpolation,
            renderer.dimensions(),
            overlay,
            self.cursor,
            None,
        )?;
        renderer.set_overlay_size(overlay[0], overlay[1]);
        renderer.draw_live(
            composed.view,
            &composed.instances,
            &composed.ui,
            &composed.lighting,
        )?;
        let presenter = self.presenter.as_mut().ok_or("No presenter")?;
        renderer.present_window(presenter, [size.width, size.height])
    }

    fn capture_pointer(&mut self) {
        let wanted = self.fight.game.snapshot().player.hp > 0 && self.controls.looking();
        if wanted == self.captured {
            return;
        }
        let Some(window) = &self.window else {
            return;
        };
        self.captured = wanted;
        if wanted {
            self.raw_pointer = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined))
                .is_ok();
            window.set_cursor_visible(false);
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
            window.set_cursor_visible(true);
            let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(
                self.pointer[0],
                self.pointer[1],
            ));
        }
    }

    fn mouse_motion(&mut self, delta: [f64; 2]) {
        self.dragged += delta[0].abs() + delta[1].abs();
        let game = &mut self.fight.game;
        let mut yaw = game.yaw;
        self.controls.motion(delta, &mut yaw, &mut game.camera);
        game.yaw = yaw;
    }

    /// Selects the living actor whose nameplate is nearest the cursor.
    fn select(&mut self) {
        let game = &mut self.fight.game;
        let frame = game.frame();
        let size = self
            .renderer
            .as_ref()
            .map_or([1280, 720], |r| r.dimensions());
        let vp = frame.view_projection(size[0] as f32 / size[1].max(1) as f32);
        let [width, height] = {
            let s = self.window.as_ref().map_or([1280, 720], |w| {
                let s = w.inner_size();
                [s.width, s.height]
            });
            [720.0 * s[0].max(1) as f32 / s[1].max(1) as f32, 720.0]
        };
        let mut closest: Option<(u64, f32)> = None;
        for a in frame
            .actors
            .iter()
            .filter(|a| a.actor.nameplate && a.health > 0)
        {
            let clip = vp * (a.actor.position + glam::Vec3::Y * 1.2 * a.actor.scale).extend(1.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.truncate() / clip.w;
            let dx = (ndc.x + 1.0) * width * 0.5 - self.cursor[0];
            let dy = (1.0 - ndc.y) * height * 0.5 - self.cursor[1];
            let distance = dx * dx + dy * dy;
            if distance < 40.0 * 40.0 && closest.is_none_or(|(_, d)| distance < d) {
                closest = Some((a.actor.id, distance));
            }
        }
        if let Some((id, _)) = closest {
            game.selected = id;
        }
    }
}

/// The second row's slot for a digit key: 1 is slot 0 and 0 is slot 9.
fn row_two_slot(key: KeyCode) -> Option<u8> {
    [
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
    ]
    .iter()
    .position(|k| *k == key)
    .map(|slot| slot as u8)
}

fn key_ability(key: KeyCode) -> Option<Ability> {
    Some(match key {
        KeyCode::Digit1 => Ability::Bow,
        KeyCode::Digit2 => Ability::FireBolt,
        KeyCode::Digit3 => Ability::MagicMissile,
        KeyCode::Digit4 => Ability::Fireball,
        KeyCode::Digit5 => Ability::MistyStep,
        KeyCode::Digit6 => Ability::Thunderwave,
        KeyCode::Digit7 => Ability::Web,
        KeyCode::Digit8 => Ability::Grease,
        KeyCode::Digit9 => Ability::Light,
        KeyCode::Digit0 => Ability::Shield,
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let result = (|| -> Result<(), String> {
            let window = Arc::new(
                event_loop
                    .create_window(
                        Window::default_attributes()
                            .with_title("Verse — The great crypt")
                            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0)),
                    )
                    .map_err(|e| e.to_string())?,
            );
            let size = window.inner_size();
            let renderer = self.fight.renderer(size.width, size.height)?;
            let presenter = renderer.attach_window(window.clone())?;
            self.window = Some(window);
            self.renderer = Some(renderer);
            self.presenter = Some(presenter);
            self.last = Instant::now();
            Ok(())
        })();
        if let Err(e) = result {
            self.error = Some(e);
            event_loop.exit();
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if self.captured && self.raw_pointer {
            if let DeviceEvent::MouseMotion { delta } = event {
                self.mouse_motion([delta.0, delta.1]);
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Focused(false) | WindowEvent::Resized(_) => {
                self.keys.clear();
                self.controls.clear();
                self.pending_select = false;
                self.capture_pointer();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(key) = event.physical_key else {
                    return;
                };
                if event.state != ElementState::Pressed {
                    self.keys.remove(&key);
                    return;
                }
                self.keys.insert(key);
                if event.repeat {
                    return;
                }
                let shift = self.keys.contains(&KeyCode::ShiftLeft)
                    || self.keys.contains(&KeyCode::ShiftRight);
                match key {
                    KeyCode::Escape => event_loop.exit(),
                    KeyCode::F1 | KeyCode::F2 => self.restart(key == KeyCode::F2),
                    KeyCode::Enter if self.fight.game.snapshot().player.hp == 0 => self.respawn(),
                    key if shift && row_two_slot(key).is_some() => {
                        self.activate(Ability::Spell(row_two_slot(key).unwrap_or(0)));
                    }
                    KeyCode::Space => {
                        if let Err(error) = self.fight.game.jump() {
                            self.fight.game.message = error;
                        }
                    }
                    KeyCode::NumLock => self.controls.autorun = !self.controls.autorun,
                    KeyCode::Tab => self.fight.game.cycle_target(),
                    key => {
                        if let Some(ability) = key_ability(key) {
                            self.activate(ability);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let size = self
                    .window
                    .as_ref()
                    .map_or(winit::dpi::PhysicalSize::new(1280, 720), |w| w.inner_size());
                if self.captured {
                    if !self.raw_pointer {
                        let center = [size.width as f64 * 0.5, size.height as f64 * 0.5];
                        let delta = [position.x - center[0], position.y - center[1]];
                        if delta[0].abs() + delta[1].abs() > 0.5 {
                            self.mouse_motion(delta);
                            if let Some(window) = &self.window {
                                let _ = window.set_cursor_position(
                                    winit::dpi::PhysicalPosition::new(center[0], center[1]),
                                );
                            }
                        }
                    }
                } else {
                    let overlay = self.overlay_size();
                    self.pointer = [position.x, position.y];
                    self.cursor = [
                        position.x as f32 / size.width.max(1) as f32 * overlay[0],
                        position.y as f32 / size.height.max(1) as f32 * overlay[1],
                    ];
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.fight.game.camera.zoom(steps);
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                let [width, height] = self.overlay_size();
                match button {
                    MouseButton::Right => {
                        if down {
                            self.pending_select = false;
                        }
                        let game = &mut self.fight.game;
                        let mut yaw = game.yaw;
                        self.controls.button(true, down, &mut yaw, &game.camera);
                        game.yaw = yaw;
                    }
                    MouseButton::Left => {
                        if down
                            && self.fight.game.snapshot().player.hp == 0
                            && overlay::respawn_at(self.cursor[0], self.cursor[1], width, height)
                        {
                            self.respawn();
                            return;
                        }
                        if down && !self.controls.looking() {
                            if let Some(ability) =
                                overlay::action_at(self.cursor[0], self.cursor[1], width, height)
                            {
                                self.activate(ability);
                                return;
                            }
                            if overlay::chrome_at(self.cursor[0], self.cursor[1], width, height) {
                                return;
                            }
                            self.pending_select = true;
                            self.dragged = 0.0;
                        }
                        if !down && self.pending_select {
                            if self.dragged < 4.0 {
                                self.select();
                            }
                            self.pending_select = false;
                        }
                        let game = &mut self.fight.game;
                        let mut yaw = game.yaw;
                        self.controls.button(false, down, &mut yaw, &game.camera);
                        game.yaw = yaw;
                    }
                    MouseButton::Back if down => self.controls.autorun = !self.controls.autorun,
                    _ => {}
                }
                self.capture_pointer();
            }
            WindowEvent::RedrawRequested => {
                if event_loop.exiting() {
                    return;
                }
                if let Err(error) = self.step().and_then(|()| self.draw()) {
                    self.error = Some(error);
                    event_loop.exit();
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
