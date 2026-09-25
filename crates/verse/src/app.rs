//! The desktop window: winit events in, a frame out.

use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::agent::Agent;
use crate::avatar::{self, Gait};
use crate::camera::FollowCamera;
use crate::controller::{InputState, PlayerController};
use crate::render::{self, Renderer, View};
use crate::world::{self, World};

/// Opens the Verse window and runs until it closes.
///
/// # Errors
///
/// Returns a message when the event loop or the renderer cannot start.
pub fn run() -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop
        .run_app(&mut app)
        .map_err(|e| format!("the event loop failed: {e}"))?;
    app.error.map_or(Ok(()), Err)
}

/// Renders the spawn view through `camera` to a PNG, without a window.
///
/// # Errors
///
/// Returns a message when no GPU is available or the file cannot be written.
pub fn capture(
    path: &std::path::Path,
    width: u32,
    height: u32,
    camera: FollowCamera,
) -> Result<(), String> {
    let world = world::build();
    let player = PlayerController::new(world::SPAWN, 0.0);
    let view = view(&camera, &player, width as f32 / height as f32);
    let mut dynamic = avatar::mesh(&player, &Gait::default());
    dynamic.extend(&Agent::new(&player).mesh());
    render::capture(path, width, height, &world.mesh, view, &dynamic)
}

fn view(camera: &FollowCamera, player: &PlayerController, aspect: f32) -> View {
    View {
        view_proj: camera.view_proj(player.pos, player.yaw, aspect),
        eye: camera.eye(player.pos, player.yaw),
    }
}

/// Raw key and button state, resolved into [`InputState`] once per frame.
#[derive(Default)]
struct Keys {
    w: bool,
    s: bool,
    a: bool,
    d: bool,
    q: bool,
    e: bool,
    shift: bool,
    jump: bool,
    left_button: bool,
    right_button: bool,
}

impl Keys {
    fn input(&self) -> InputState {
        let both = self.left_button && self.right_button;
        InputState {
            forward: self.w || both,
            backward: self.s,
            left: self.a,
            right: self.d,
            strafe_left: self.q,
            strafe_right: self.e,
            mouse_look: self.right_button,
            sprint: self.shift,
            jump: self.jump,
        }
    }
}

struct App {
    window: Option<Arc<Window>>,
    renderer: Option<Renderer>,
    world: World,
    player: PlayerController,
    camera: FollowCamera,
    gait: Gait,
    agent: Agent,
    keys: Keys,
    last: Instant,
    error: Option<String>,
}

impl App {
    fn new() -> Self {
        Self {
            window: None,
            renderer: None,
            world: world::build(),
            player: PlayerController::new(world::SPAWN, 0.0),
            agent: Agent::new(&PlayerController::new(world::SPAWN, 0.0)),
            camera: FollowCamera::default(),
            gait: Gait::default(),
            keys: Keys::default(),
            last: Instant::now(),
            error: None,
        }
    }

    fn key(&mut self, code: KeyCode, pressed: bool, event_loop: &ActiveEventLoop) {
        match code {
            KeyCode::KeyW | KeyCode::ArrowUp => self.keys.w = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.keys.s = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => self.keys.a = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.keys.d = pressed,
            KeyCode::KeyQ => self.keys.q = pressed,
            KeyCode::KeyE => self.keys.e = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.shift = pressed,
            KeyCode::Space if pressed => self.keys.jump = true,
            KeyCode::Escape if pressed => event_loop.exit(),
            _ => {}
        }
    }

    fn button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.keys.left_button = pressed,
            MouseButton::Right => {
                self.keys.right_button = pressed;
                if pressed {
                    self.player.yaw += self.camera.take_offset();
                }
            }
            _ => return,
        }
        self.capture(self.keys.left_button || self.keys.right_button);
    }

    /// Hides and locks the cursor while a mouse button drags the view.
    fn capture(&self, on: bool) {
        let Some(window) = &self.window else { return };
        if on {
            let _ = window
                .set_cursor_grab(CursorGrabMode::Locked)
                .or_else(|_| window.set_cursor_grab(CursorGrabMode::Confined));
        } else {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        window.set_cursor_visible(!on);
    }

    fn mouse(&mut self, dx: f32, dy: f32) {
        if self.keys.right_button {
            self.player.yaw =
                crate::controller::wrap(self.player.yaw + self.camera.mouselook(dx, dy));
        } else if self.keys.left_button {
            self.camera.orbit(dx, dy);
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.1);
        self.last = now;

        let input = self.keys.input();
        self.keys.jump = false;
        self.player
            .update(&input, dt, &self.world.blockers, world::HALF);
        if self.player.speed > 0.1 && !self.keys.left_button {
            self.camera.settle(dt);
        }
        self.gait
            .advance(self.player.speed, self.player.airborne(), dt);
        self.agent.update(&self.player, dt);

        let Some(renderer) = &mut self.renderer else {
            return;
        };
        let view = view(&self.camera, &self.player, renderer.aspect());
        let mut dynamic = avatar::mesh(&self.player, &self.gait);
        dynamic.extend(&self.agent.mesh());
        renderer.draw(view, &dynamic);
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attributes = Window::default_attributes()
            .with_title("Verse")
            .with_inner_size(LogicalSize::new(1440.0, 900.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(e) => {
                self.error = Some(format!("cannot open a window: {e}"));
                event_loop.exit();
                return;
            }
        };
        match Renderer::new(window.clone(), &self.world.mesh) {
            Ok(renderer) => self.renderer = Some(renderer),
            Err(e) => {
                self.error = Some(e);
                event_loop.exit();
                return;
            }
        }
        window.focus_window();
        self.window = Some(window);
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && !event.repeat
                {
                    self.key(code, event.state == ElementState::Pressed, event_loop);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.button(button, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                self.camera.zoom(lines);
            }
            WindowEvent::Focused(false) => {
                self.keys = Keys::default();
                self.capture(false);
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.mouse(dx as f32, dy as f32);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}
