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
use crate::chat::{self, Channel};
use crate::controller::{InputState, PlayerController};
use crate::hud;
use crate::render::{self, Renderer, View};
use crate::session::{self, Session, Status};
use crate::ui::Atlas;
use crate::world::{self, World};

/// How the window joins the shared world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Profile name; each profile is its own player key.
    pub profile: String,
    /// Relay URL, or `None` to play offline.
    pub relay: Option<String>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: "default".into(),
            relay: Some(session::DEFAULT_RELAY.into()),
        }
    }
}

/// Opens the Verse window and runs until it closes.
///
/// # Errors
///
/// Returns a message when the identity, the event loop, or the renderer
/// cannot start.
pub fn run(options: &Options) -> Result<(), String> {
    let event_loop = EventLoop::new().map_err(|e| format!("cannot start the event loop: {e}"))?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new(options)?;
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
    let atlas = Atlas::new(14.0);
    let ui = hud::build(
        &atlas,
        &sample_hud(&view, [width as f32, height as f32], &player),
    );
    render::capture(
        path,
        width,
        height,
        &world.mesh,
        view,
        &dynamic,
        &ui,
        &atlas,
    )
}

/// A sample conversation, so a capture shows the chat design.
fn sample_hud<'a>(view: &View, size: [f32; 2], player: &PlayerController) -> hud::Frame<'a> {
    use crate::chat::{Line, Log};
    let mut log = Log::default();
    let line = |channel: Channel, from: &str, text: &str, note: Option<&str>| Line {
        channel: Some(channel),
        from: from.into(),
        to: None,
        text: text.into(),
        note: note.map(str::to_owned),
    };
    log.push(Line::system("Player north has logged in"));
    log.push(line(Channel::All, "north", "gm verse", None));
    log.push(line(
        Channel::Ads,
        "south",
        "*BUYING* a spare pylon, PM me",
        Some("[4 listening]"),
    ));
    log.push(line(
        Channel::Zone,
        "kiki",
        "anyone on the plaza want to race?",
        None,
    ));
    log.push(Line::system("Player south has logged in"));
    log.push(line(
        Channel::Near,
        "south",
        "the tower by the pylon is huge",
        None,
    ));
    log.push(line(Channel::Here, "north", "hi!", Some("(2 here)")));
    log.push(line(
        Channel::Room("lounge".into()),
        "kiki",
        "welcome in",
        None,
    ));
    log.push(Line {
        channel: Some(Channel::Pm("x".into())),
        from: "kiki".into(),
        to: Some("north".into()),
        text: "want to build together?".into(),
        note: None,
    });
    let overheads: &'a [hud::Overhead] = Box::leak(Box::new([hud::Overhead {
        feet: player.pos,
        name: None,
        bubble: Some("gm verse".into()),
    }]));
    hud::Frame {
        size,
        scale: 1.0,
        view_proj: view.view_proj,
        log: Box::leak(Box::new(log)),
        input: Box::leak(Box::new(hud::Input::default())),
        method: "ALL".into(),
        world_title: hud::world_title(session::WORLD, player.pos),
        overheads,
        time: 0.0,
    }
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
    session: Option<Session>,
    title: String,
    frames: u64,
    atlas: Option<Atlas>,
    scale: f32,
    chat: hud::Input,
    method: Channel,
    pm_target: Option<String>,
    offline_log: chat::Log,
    started: Instant,
}

impl App {
    fn new(options: &Options) -> Result<Self, String> {
        let world = world::build();
        let mut player = PlayerController::new(world::SPAWN, 0.0);
        let mut session = match &options.relay {
            Some(relay) => Some(Session::start(&options.profile, relay)?),
            None => None,
        };
        let spawn = match &mut session {
            Some(s) => s.spawn(
                &world.blockers,
                world::HALF,
                std::time::Duration::from_millis(1500),
            ),
            None => session::Spawn {
                pos: session::random_spawn(&world.blockers),
                yaw: 0.0,
                resumed: false,
            },
        };
        player.pos = spawn.pos;
        player.yaw = spawn.yaw;
        if let Some(s) = &session {
            eprintln!(
                "verse: {} ({}…) on {}; {}",
                s.profile(),
                &s.pubkey()[..12],
                s.relay(),
                if spawn.resumed {
                    "resumed where you left"
                } else {
                    "new spawn on the plaza"
                }
            );
        }
        Ok(Self {
            window: None,
            renderer: None,
            agent: Agent::new(&player),
            world,
            player,
            camera: FollowCamera::default(),
            gait: Gait::default(),
            keys: Keys::default(),
            last: Instant::now(),
            error: None,
            session,
            title: String::new(),
            frames: 0,
            atlas: None,
            scale: 1.0,
            chat: hud::Input::default(),
            method: Channel::All,
            pm_target: None,
            offline_log: {
                let mut log = chat::Log::default();
                log.push(chat::Line::system(
                    "Welcome to Verse. Press Enter to chat, Tab to change channel.",
                ));
                log
            },
            started: Instant::now(),
        })
    }

    /// Records the player as offline on the relay before quitting.
    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(session) = &mut self.session {
            session.leave(&self.player, &self.agent);
        }
        self.session = None;
        event_loop.exit();
    }

    fn update_title(&mut self) {
        let title = match &self.session {
            None => "Verse — offline".to_owned(),
            Some(s) => {
                let status = match s.status {
                    Status::Connecting => "connecting",
                    Status::Online => "online",
                    Status::Offline => "relay unreachable, retrying",
                };
                let shown = s.crowd.shown(Instant::now());
                let avatars = shown.iter().filter(|e| e.role == "avatar");
                let online = avatars.clone().filter(|e| e.online).count();
                let resting = avatars.count() - online;
                format!(
                    "Verse — {} — {} — {online} other players online, {resting} resting",
                    s.profile(),
                    status,
                )
            }
        };
        if title != self.title
            && let Some(window) = &self.window
        {
            window.set_title(&title);
            self.title = title;
        }
    }

    fn methods(&self) -> Vec<Channel> {
        let rooms: Vec<String> = session::ROOMS.iter().map(|r| (*r).to_owned()).collect();
        chat::methods(&rooms, self.pm_target.as_deref())
    }

    fn cycle_method(&mut self) {
        let methods = self.methods();
        let i = methods.iter().position(|m| *m == self.method).unwrap_or(0);
        self.method = methods[(i + 1) % methods.len()].clone();
    }

    fn open_chat(&mut self, seed: &str) {
        self.chat.open = true;
        self.chat.text = seed.to_owned();
        let (left, right) = (self.keys.left_button, self.keys.right_button);
        self.keys = Keys {
            left_button: left,
            right_button: right,
            ..Keys::default()
        };
    }

    /// Handles a key while the chat line is open.
    fn chat_key(&mut self, event: &winit::event::KeyEvent) {
        use winit::keyboard::{Key, NamedKey};
        if !event.state.is_pressed() {
            return;
        }
        match &event.logical_key {
            Key::Named(NamedKey::Enter) => {
                let text = std::mem::take(&mut self.chat.text);
                self.chat.open = false;
                if !text.trim().is_empty() {
                    self.chat.last.clone_from(&text);
                    self.submit(&text);
                }
            }
            Key::Named(NamedKey::Escape) => {
                self.chat.open = false;
                self.chat.text.clear();
            }
            Key::Named(NamedKey::Backspace) => {
                self.chat.text.pop();
            }
            Key::Named(NamedKey::Tab) => self.cycle_method(),
            Key::Named(NamedKey::ArrowUp) => self.chat.text.clone_from(&self.chat.last),
            _ => {
                if let Some(text) = &event.text {
                    for c in text.chars().filter(|c| !c.is_control()) {
                        if self.chat.text.chars().count() < chat::MAX_LINE {
                            self.chat.text.push(c);
                        }
                    }
                }
            }
        }
    }

    /// Carries out one submitted chat line.
    fn submit(&mut self, text: &str) {
        let now = Instant::now();
        let command = chat::parse(text, &self.method);
        let Some(session) = &mut self.session else {
            self.offline_log
                .push(chat::Line::system("You are offline. Chat needs a relay."));
            return;
        };
        let notice = match command {
            chat::Command::Nothing => None,
            chat::Command::Send(channel, text) => session.say(&channel, &text, now).err(),
            chat::Command::Whisper(name, text) => match session.find_player(&name) {
                Some((pubkey, _)) => session.pm(&pubkey, &text).err(),
                None => Some("Could not find player to Private chat!".to_owned()),
            },
            chat::Command::Target(name) => match session.find_player(&name) {
                Some((pubkey, name)) => {
                    self.pm_target = Some(pubkey.clone());
                    self.method = Channel::Pm(pubkey);
                    Some(format!(
                        "Now chatting privately with {name}. Tab to switch back."
                    ))
                }
                None => Some("Could not find player to Private chat!".to_owned()),
            },
            chat::Command::Mute(word) => Some(session.set_mute(&word, true)),
            chat::Command::Unmute(word) => Some(session.set_mute(&word, false)),
        };
        if let Some(notice) = notice {
            session.log.push(chat::Line::system(notice));
        }
    }

    fn key(&mut self, code: KeyCode, pressed: bool, event_loop: &ActiveEventLoop) {
        match code {
            KeyCode::Enter | KeyCode::NumpadEnter if pressed => self.open_chat(""),
            KeyCode::Slash if pressed => self.open_chat("/"),
            KeyCode::Tab if pressed => self.cycle_method(),
            KeyCode::KeyW | KeyCode::ArrowUp => self.keys.w = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.keys.s = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => self.keys.a = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.keys.d = pressed,
            KeyCode::KeyQ => self.keys.q = pressed,
            KeyCode::KeyE => self.keys.e = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.shift = pressed,
            KeyCode::Space if pressed => self.keys.jump = true,
            KeyCode::Escape if pressed => self.quit(event_loop),
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

        // The look-around is the agent assessing what is near: it asks the
        // relay for entity states around it, then glances at what it found.
        if self.agent.take_scan() {
            match &mut self.session {
                Some(session) => session.request_scan(self.agent.pos),
                None => self.agent.look_around(&[]),
            }
        }
        let mut dynamic = avatar::mesh(&self.player, &self.gait);
        dynamic.extend(&self.agent.mesh());
        if let Some(session) = &mut self.session {
            session.tick(now, &self.player, &self.agent);
            if let Some(found) = session.scan_result(now, &self.agent) {
                self.agent.look_around(&found);
            }
            // Two agents that meet greet each other.
            if let Some((pubkey, at)) = session.greeting(now, &self.agent)
                && self.agent.greet(at)
            {
                session.greeted(&pubkey, at, &self.agent, now);
            }
            dynamic.extend(&session.crowd.mesh(now, dt));
        }
        if self.frames.is_multiple_of(30) {
            self.update_title();
        }
        self.frames = self.frames.wrapping_add(1);

        let Some(renderer) = &self.renderer else {
            return;
        };
        let view = view(&self.camera, &self.player, renderer.aspect());
        let size = renderer.size();
        let overheads = self.overheads(now);
        let ui = match &self.atlas {
            Some(atlas) => {
                let (log, method) = match &self.session {
                    Some(s) => (&s.log, hud::method_label(&self.method, |p| s.name_of(p))),
                    None => (
                        &self.offline_log,
                        hud::method_label(&self.method, str::to_owned),
                    ),
                };
                hud::build(
                    atlas,
                    &hud::Frame {
                        size,
                        scale: self.scale,
                        view_proj: view.view_proj,
                        log,
                        input: &self.chat,
                        method,
                        world_title: hud::world_title(session::WORLD, self.player.pos),
                        overheads: &overheads,
                        time: (now - self.started).as_secs_f32(),
                    },
                )
            }
            None => crate::ui::UiBatch::default(),
        };
        if let Some(renderer) = &mut self.renderer {
            renderer.draw(view, &dynamic, &ui);
        }
    }

    /// Name tags over nearby players and speech bubbles over speakers.
    fn overheads(&self, now: Instant) -> Vec<hud::Overhead> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        let bubble_for = |pubkey: &str| {
            session
                .bubbles
                .iter()
                .find(|b| b.pubkey == pubkey)
                .map(|b| b.text.clone())
        };
        let mut out: Vec<hud::Overhead> = session
            .crowd
            .shown(now)
            .into_iter()
            .filter(|e| e.role == "avatar" && e.pos.distance(self.player.pos) < 60.0)
            .map(|e| hud::Overhead {
                feet: e.pos,
                name: Some(session.name_of(&e.pubkey)),
                bubble: bubble_for(&e.pubkey),
            })
            .collect();
        if let Some(text) = bubble_for(session.pubkey()) {
            out.push(hud::Overhead {
                feet: self.player.pos,
                name: None,
                bubble: Some(text),
            });
        }
        out
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
        self.scale = window.scale_factor() as f32;
        let atlas = Atlas::new((14.0 * self.scale).round());
        match Renderer::new(window.clone(), &self.world.mesh, &atlas) {
            Ok(renderer) => {
                self.renderer = Some(renderer);
                self.atlas = Some(atlas);
            }
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
            WindowEvent::CloseRequested => self.quit(event_loop),
            WindowEvent::Resized(size) => {
                if let Some(renderer) = &mut self.renderer {
                    renderer.resize(size.width, size.height);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if self.chat.open {
                    self.chat_key(&event);
                } else if let PhysicalKey::Code(code) = event.physical_key
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
