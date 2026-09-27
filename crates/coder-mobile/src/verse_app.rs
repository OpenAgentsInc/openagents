//! Coder's native Verse surface. Scene behavior comes from the shared Verse
//! runtime; this adapter owns touch gestures and explicit mobile connectivity.
use coder_ui::theme::{Intensity, NEAR_BLACK};
use rust_native::style::{Color, Style};
use rust_native::surface::{SurfaceLifecycle, Viewport};
use rust_native::{Element, Node, View};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
static NEXT_MOUNT: AtomicU64 = AtomicU64::new(1);
use verse::controller::InputState;
use verse::runtime::{Action, WorldRuntime};
use verse::session::Session;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    pub secret_hex: String,
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    #[serde(default)]
    pub synthetic: bool,
    #[serde(default)]
    pub gym_code: Option<String>,
    #[serde(default)]
    pub synthetic_gym: bool,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Snapshot,
    Frame {
        timestamp: f64,
    },
    Resize {
        width: u32,
        height: u32,
        scale: f32,
    },
    Active {
        active: bool,
    },
    CameraMode {
        mode: CameraMode,
    },
    DeviceMotion {
        quaternion: [f32; 4],
        timestamp: f64,
    },
    ResetMotion,
    Pointer {
        id: u64,
        phase: PointerPhase,
        x: f32,
        y: f32,
    },
    Jump,
    Sprint {
        enabled: bool,
    },
    Zoom {
        delta: f32,
    },
    Connect {
        relay: String,
    },
    Disconnect,
    InteractComputer,
    CloseComputer,
    InteractGym,
    CloseGym,
    GymView,
    GymConfigure {
        code: String,
    },
    GymSelectRun {
        id: String,
    },
    GymSelectRecipe {
        id: String,
    },
    GymLaunch,
    GymRetry,
    GymCloseDetail,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum CameraMode {
    #[default]
    Touch,
    Motion,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PointerPhase {
    Down,
    Move,
    Up,
    Cancel,
}

#[derive(Serialize)]
pub(crate) struct Packet {
    schema: &'static str,
    status: String,
    pub error: Option<String>,
    frames_presented: u64,
    position: [f32; 3],
    camera_mode: CameraMode,
    camera_yaw: f32,
    camera_pitch: f32,
    motion_needed: bool,
    computer: Computer,
    computer_open: bool,
    gym: Gym,
    gym_open: bool,
    gym_revision: u64,
    gym_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gym_board: Option<verse::gym::BoardView>,
    view: View<()>,
}

/// Coordinates are normalized from the top-left of the Metal viewport.
/// Visibility describes projection, not occlusion by another world object.
#[derive(Serialize)]
struct Computer {
    near: bool,
    visible: bool,
    screen_x: f32,
    screen_y: f32,
    distance: f32,
}

impl From<verse::runtime::Computer> for Computer {
    fn from(value: verse::runtime::Computer) -> Self {
        Self {
            near: value.near,
            visible: value.visible,
            screen_x: value.screen_x,
            screen_y: value.screen_y,
            distance: value.distance,
        }
    }
}

/// The Gym's interior membership and native board anchor come from the shared world.
#[derive(Serialize)]
struct Gym {
    inside: bool,
    near: bool,
    visible: bool,
    screen_x: f32,
    screen_y: f32,
    distance: f32,
}

impl From<verse::runtime::Gym> for Gym {
    fn from(value: verse::runtime::Gym) -> Self {
        Self {
            inside: value.inside,
            near: value.near,
            visible: value.visible,
            screen_x: value.screen_x,
            screen_y: value.screen_y,
            distance: value.distance,
        }
    }
}

pub(crate) fn blueprint() -> Packet {
    packet(
        "verse.blueprint",
        "Preparing Verse".into(),
        None,
        0,
        [0.0, 0.0, -10.0],
    )
}

fn packet(
    instance: &str,
    status: String,
    error: Option<String>,
    frames: u64,
    position: [f32; 3],
) -> Packet {
    let color = |rgb: u32| Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    let view = View::new(
        instance,
        1,
        Node {
            key: "verse-canvas".into(),
            style: Style {
                foreground: Some(color(Intensity::Full.color())),
                background: Some(color(NEAR_BLACK)),
                ..Style::default()
            },
            element: Element::Surface {
                resource: "verse.world".into(),
                label: "Verse world".into(),
            },
        },
    );
    Packet {
        schema: "coder.verse.v1",
        status,
        error,
        frames_presented: frames,
        position,
        camera_mode: CameraMode::Touch,
        camera_yaw: 0.0,
        camera_pitch: verse::camera::FollowCamera::default().pitch,
        motion_needed: false,
        computer: Computer {
            near: false,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: 5.0,
        },
        computer_open: false,
        gym: Gym {
            inside: false,
            near: false,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: 60.0,
        },
        gym_open: false,
        gym_revision: 0,
        gym_active: false,
        gym_board: None,
        view,
    }
}

struct Touch {
    origin: [f32; 2],
    latest: [f32; 2],
    movement: bool,
    computer: bool,
    tap_valid: bool,
    started: Option<f64>,
}

#[derive(Default)]
struct Motion {
    baseline: Option<MotionBaseline>,
    last_sample: Option<f64>,
}

struct MotionBaseline {
    sensor: [f32; 2],
    camera: [f32; 2],
}

/// Core Motion uses portrait device axes (+X right, +Y top, +Z out of the
/// screen) and a gravity-aligned reference (+Z up). Its attitude maps the
/// reference into device coordinates. Inverse-rotate the phone-back direction
/// into that reference, then retain heading and downward pitch only. Screen
/// roll cannot tilt the horizon. This also treats q and -q identically.
enum MotionOrientation {
    Angles([f32; 2]),
    Vertical,
}

fn motion_angles(quaternion: [f32; 4]) -> Option<MotionOrientation> {
    if quaternion.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let norm: f32 = quaternion.iter().map(|v| v * v).sum();
    if !(0.25..=4.0).contains(&norm) {
        return None;
    }
    let [x, y, z, w] = quaternion.map(|v| v / norm.sqrt());
    let forward = [
        2.0 * (w * y - x * z),
        -2.0 * (y * z + w * x),
        2.0 * (x * x + y * y) - 1.0,
    ];
    let horizontal = forward[0].hypot(forward[1]);
    // A phone pointing almost vertically has no stable heading. Keep the view
    // unchanged and establish a new baseline when a stable heading returns.
    if horizontal < 0.05 {
        return Some(MotionOrientation::Vertical);
    }
    Some(MotionOrientation::Angles([
        forward[0].atan2(forward[1]),
        (-forward[2]).atan2(horizontal),
    ]))
}

pub(crate) struct Scene {
    pub world: WorldRuntime,
    pub lifecycle: SurfaceLifecycle,
    pub session: Option<Session>,
    /// Remote geometry from the last presented frame, also used for picking.
    pub presented_entities: verse::mesh::Mesh,
    secret: secp256k1::SecretKey,
    relay: Option<String>,
    synthetic: bool,
    spawn_pending: bool,
    camera_mode: CameraMode,
    motion: Motion,
    frame_timestamp: Option<f64>,
    touches: BTreeMap<u64, Touch>,
    jump: bool,
    sprint: bool,
    computer_open: bool,
    gym_open: bool,
    gym_configuration_error: Option<String>,
    pub gym_board: verse::gym::Board,
    pub frames: u64,
    pub error: Option<String>,
}

impl Scene {
    pub fn new(config: Config) -> Result<Self, String> {
        let secret = config
            .secret_hex
            .parse()
            .map_err(|_| "Invalid Verse device identity".to_owned())?;
        let viewport =
            Viewport::new(config.width, config.height, config.scale).map_err(|e| e.to_string())?;
        let lifecycle = SurfaceLifecycle::new(
            format!("verse.mount.{}", NEXT_MOUNT.fetch_add(1, Ordering::Relaxed)),
            viewport,
        )
        .map_err(|e| e.to_string())?;
        let mut gym_board = verse::gym::Board::new(secret, config.synthetic);
        let initial_gym_error = config
            .gym_code
            .as_deref()
            .and_then(|code| gym_board.configure(code).err());
        if config.synthetic_gym && !config.synthetic {
            return Err("The Gym preview requires synthetic mode".into());
        }
        let mut world = WorldRuntime::new();
        if config.synthetic_gym {
            // This explicit fixture starts outside the entrance. The real touch
            // path must cross the boundary before the board loads its rows.
            let mut outside = verse::world::GYM_ENTRANCE;
            outside.x -= 2.0;
            world.set_spawn(outside, std::f32::consts::FRAC_PI_2)?;
        }
        Ok(Self {
            world,
            lifecycle,
            session: None,
            presented_entities: verse::mesh::Mesh::default(),
            secret,
            relay: None,
            synthetic: config.synthetic,
            spawn_pending: false,
            camera_mode: CameraMode::Touch,
            motion: Motion::default(),
            frame_timestamp: None,
            touches: BTreeMap::new(),
            jump: false,
            sprint: false,
            computer_open: false,
            gym_open: false,
            gym_configuration_error: initial_gym_error,
            gym_board,
            frames: 0,
            error: None,
        })
    }

    pub fn activate(&mut self, active: bool) -> Result<(), String> {
        let changed = self.lifecycle.active() != active;
        self.lifecycle
            .set_active(active)
            .map_err(|e| e.to_string())?;
        if changed {
            self.reset_motion();
            self.frame_timestamp = None;
        }
        if !active {
            self.presented_entities = verse::mesh::Mesh::default();
            self.touches.clear();
            self.jump = false;
            self.sprint = false;
            self.session = None;
        } else if self.session.is_none() && self.relay.is_some() {
            self.start_session()?;
        }
        self.sync_gym_interest();
        Ok(())
    }

    pub fn connect(&mut self, relay: String) -> Result<(), String> {
        if self.synthetic {
            return Err("Synthetic mode does not connect to a relay".into());
        }
        coder_connect::RelayPolicy::Production
            .validate(&relay)
            .map_err(|_| {
                "Use a credential-free wss:// Verse relay URL without a query or fragment"
                    .to_owned()
            })?;
        self.session = None;
        self.relay = Some(relay);
        if self.lifecycle.active() {
            self.start_session()?;
        }
        Ok(())
    }

    fn start_session(&mut self) -> Result<(), String> {
        let identity = verse::identity::Identity::from_secret("phone", self.secret)?;
        let mut session = Session::start_with_identity(
            identity,
            self.relay.as_deref().ok_or("No Verse relay selected")?,
        )?;
        session.set_publish_intervals(verse::session::PublishIntervals::mobile())?;
        session.begin_spawn(Duration::from_millis(1500));
        self.spawn_pending = true;
        self.reset_motion();
        self.gym_board.set_active(false);
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.presented_entities = verse::mesh::Mesh::default();
        self.session = None;
        self.relay = None;
    }

    pub fn pointer(&mut self, id: u64, phase: PointerPhase, x: f32, y: f32) -> Result<(), String> {
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            // Always release input, including cancelled or malformed native events.
            let touch = self.touches.remove(&id);
            if let Some(touch) = touch
                && matches!(phase, PointerPhase::Up)
                && touch.computer
                && touch.tap_valid
                && (x - touch.origin[0]).hypot(y - touch.origin[1]) <= 12.0
                && touch
                    .started
                    .zip(self.frame_timestamp)
                    .is_some_and(|(start, end)| (0.0..=0.65).contains(&(end - start)))
                && self.lifecycle.active()
                && !self.panel_open()
                && self.computer_hit(x, y)
            {
                self.open_computer();
            }
            return Ok(());
        }
        if !self.lifecycle.active() || self.panel_open() {
            return Ok(());
        }
        if !x.is_finite() || !y.is_finite() || x.abs() > 32768.0 || y.abs() > 32768.0 {
            return Err("Touch coordinates exceed their bounds".into());
        }
        match phase {
            PointerPhase::Down => {
                if self.touches.contains_key(&id) {
                    return Err("Touch identity is already active".into());
                }
                // A second finger makes this a movement gesture, never a tap.
                for touch in self.touches.values_mut() {
                    touch.tap_valid = false;
                }
                if self.touches.len() >= 2 {
                    return Ok(());
                }
                let size = self.lifecycle.viewport().logical_size();
                if x < 0.0 || y < 0.0 || x > size[0] || y > size[1] {
                    return Ok(());
                }
                let computer = self.touches.is_empty() && self.computer_hit(x, y);
                let movement = x < size[0] * 0.5;
                if !computer && !movement && self.camera_mode == CameraMode::Motion {
                    return Ok(());
                }
                if self.touches.values().any(|p| p.movement == movement) {
                    return Ok(());
                }
                self.touches.insert(
                    id,
                    Touch {
                        origin: [x, y],
                        latest: [x, y],
                        movement,
                        computer,
                        tap_valid: true,
                        started: self.frame_timestamp,
                    },
                );
                if !computer && !movement {
                    self.world.apply(Action::FaceCamera)?;
                }
            }
            PointerPhase::Move => {
                if let Some(touch) = self.touches.get_mut(&id) {
                    touch.tap_valid &= (x - touch.origin[0]).hypot(y - touch.origin[1]) <= 12.0;
                    if !touch.computer && !touch.movement && self.camera_mode == CameraMode::Touch {
                        self.world.apply(Action::Look {
                            dx: (x - touch.latest[0]).clamp(-500.0, 500.0),
                            dy: (y - touch.latest[1]).clamp(-500.0, 500.0),
                        })?;
                    }
                    touch.latest = [x, y];
                }
            }
            PointerPhase::Up | PointerPhase::Cancel => {}
        }
        Ok(())
    }

    fn input(&mut self) -> InputState {
        if self.panel_open() {
            return InputState::default();
        }
        let mut input = InputState {
            jump: std::mem::take(&mut self.jump),
            sprint: self.sprint,
            ..InputState::default()
        };
        if let Some(touch) = self.touches.values().find(|p| p.movement && !p.computer) {
            let x = touch.latest[0] - touch.origin[0];
            let y = touch.latest[1] - touch.origin[1];
            input.forward = match self.camera_mode {
                CameraMode::Touch => y < -12.0,
                CameraMode::Motion => y <= 12.0,
            };
            input.backward = y > 12.0;
            input.strafe_left = x < -12.0;
            input.strafe_right = x > 12.0;
        }
        input.mouse_look =
            self.motion_needed() || self.touches.values().any(|p| !p.movement && !p.computer);
        input
    }

    pub fn update(&mut self, timestamp: f64) -> Result<Option<f32>, String> {
        let Some(dt) = self
            .lifecycle
            .frame_delta(timestamp)
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        self.frame_timestamp = Some(timestamp);
        if self.spawn_pending {
            self.gym_board.set_active(false);
            if let Some(session) = &mut self.session {
                if let Some(spawn) =
                    session.poll_spawn(&self.world.world.blockers, verse::world::HALF)
                {
                    self.world.set_spawn(spawn.pos, spawn.yaw)?;
                    self.spawn_pending = false;
                    self.reset_motion();
                } else {
                    return Ok(Some(0.0));
                }
            } else {
                self.spawn_pending = false;
                self.reset_motion();
            }
        }
        let input = self.input();
        self.world.tick(&input, dt);
        let panel_was_open = self.panel_open();
        if !self.computer().near {
            self.computer_open = false;
        }
        if !self.gym().inside {
            self.gym_open = false;
        }
        if panel_was_open != self.panel_open() {
            self.reset_motion();
        }
        self.sync_gym_interest();
        self.gym_board.poll();
        let now = Instant::now();
        if let Some(session) = &mut self.session {
            session.tick(now, &self.world.player, &self.world.agent);
            if self.world.agent.take_scan() {
                session.request_scan(self.world.agent.pos);
            }
            if let Some(found) = session.scan_result(now, &self.world.agent) {
                self.world.agent.look_around(&found);
            }
            if let Some((key, at)) = session.greeting(now, &self.world.agent)
                && self.world.agent.greet(at)
            {
                session.greeted(&key, at, &self.world.agent, now);
            }
        } else if self.world.agent.take_scan() {
            self.world.agent.look_around(&[]);
        }
        Ok(Some(dt))
    }

    pub fn action(&mut self, request: Request) -> Result<(), String> {
        match request {
            Request::Active { active } => self.activate(active),
            Request::CameraMode { mode } => {
                if self.camera_mode != mode {
                    self.camera_mode = mode;
                    self.touches.retain(|_, touch| touch.movement);
                    self.reset_motion();
                }
                Ok(())
            }
            Request::DeviceMotion {
                quaternion,
                timestamp,
            } => {
                self.device_motion(quaternion, timestamp);
                Ok(())
            }
            Request::ResetMotion => {
                self.reset_motion();
                Ok(())
            }
            Request::Pointer { id, phase, x, y } => self.pointer(id, phase, x, y),
            Request::Jump => {
                if self.lifecycle.active() && !self.panel_open() {
                    self.jump = true;
                }
                Ok(())
            }
            Request::Sprint { enabled } => {
                self.sprint = self.lifecycle.active() && !self.panel_open() && enabled;
                Ok(())
            }
            Request::Zoom { delta } => {
                if self.panel_open() {
                    Ok(())
                } else {
                    self.world.apply(Action::Zoom { lines: delta })
                }
            }
            Request::Connect { relay } => self.connect(relay),
            Request::Disconnect => {
                self.disconnect();
                Ok(())
            }
            Request::InteractComputer => {
                let computer = self.computer();
                let size = self.lifecycle.viewport().logical_size();
                if !self.lifecycle.active()
                    || self.spawn_pending
                    || !self.computer_hit(computer.screen_x * size[0], computer.screen_y * size[1])
                {
                    return Err("Walk up to the computer to open it".into());
                }
                self.open_computer();
                Ok(())
            }
            Request::CloseComputer => {
                self.reset_motion();
                self.computer_open = false;
                Ok(())
            }
            Request::InteractGym => {
                let gym = self.gym();
                if !self.lifecycle.active() || !gym.inside || !gym.near || !gym.visible {
                    return Err("Walk inside the Gym and approach its board to open it".into());
                }
                self.reset_motion();
                self.gym_open = true;
                self.computer_open = false;
                self.touches.clear();
                self.jump = false;
                self.sprint = false;
                Ok(())
            }
            Request::CloseGym => {
                self.reset_motion();
                self.gym_open = false;
                Ok(())
            }
            Request::GymConfigure { code } => {
                self.require_gym_panel()?;
                self.gym_board.configure(&code)?;
                self.gym_configuration_error = None;
                Ok(())
            }
            Request::GymView => Ok(()),
            Request::GymSelectRun { id } => {
                self.require_gym_panel()?;
                self.gym_board.select_run(&id)
            }
            Request::GymSelectRecipe { id } => {
                self.require_gym_panel()?;
                self.gym_board.select_recipe(&id)
            }
            Request::GymLaunch => {
                self.require_gym_panel()?;
                self.gym_board.confirm_launch()
            }
            Request::GymRetry => {
                self.require_gym_panel()?;
                self.gym_board.retry_launch()
            }
            Request::GymCloseDetail => {
                self.require_gym_panel()?;
                self.gym_board.close_detail();
                Ok(())
            }
            Request::Snapshot => Ok(()),
            Request::Frame { .. } | Request::Resize { .. } => {
                Err("Request requires a native renderer".into())
            }
        }
    }

    pub fn packet(&self) -> Packet {
        let status = if !self.lifecycle.active() {
            "Verse paused".into()
        } else if self.spawn_pending && self.session.is_some() {
            "Verse · restoring world position".into()
        } else if let Some(session) = &self.session {
            format!(
                "Verse · {:?} · {} nearby entities",
                session.status,
                session.crowd.len()
            )
        } else {
            "Verse · offline world".into()
        };
        let mut packet = packet(
            self.lifecycle.id(),
            status,
            self.error.clone(),
            self.frames,
            self.world.player.pos.to_array(),
        );
        packet.camera_mode = self.camera_mode;
        packet.camera_yaw =
            verse::controller::wrap(self.world.player.yaw + self.world.camera.yaw_offset);
        packet.camera_pitch = self.world.camera.pitch;
        packet.motion_needed = self.motion_needed();
        packet.computer = self.computer().into();
        packet.computer_open = self.computer_open;
        packet.gym = self.gym().into();
        packet.gym_open = self.gym_open;
        packet.gym_revision = self.gym_board.revision();
        packet.gym_active = self.lifecycle.active() && !self.spawn_pending && self.gym().inside;
        packet
    }

    pub fn gym_view(&self) -> Option<verse::gym::BoardView> {
        self.require_gym_panel().ok().map(|()| {
            let mut view = self.gym_board.view();
            if view.error.is_none() {
                view.error = self.gym_configuration_error.clone();
            }
            view
        })
    }

    fn motion_needed(&self) -> bool {
        self.lifecycle.active()
            && self.camera_mode == CameraMode::Motion
            && !self.panel_open()
            && !self.spawn_pending
    }

    fn reset_motion(&mut self) {
        self.motion.baseline = None;
        // Keep the high-water mark across sensor restarts. An old native sample
        // must not become the baseline after a panel closes or the app resumes.
        if let Some(frame) = self.frame_timestamp {
            self.motion.last_sample = Some(self.motion.last_sample.unwrap_or(frame).max(frame));
        }
    }

    fn device_motion(&mut self, quaternion: [f32; 4], timestamp: f64) {
        if !self.motion_needed()
            || !timestamp.is_finite()
            || !(0.0..=1e12).contains(&timestamp)
            || self
                .motion
                .last_sample
                .is_some_and(|last| timestamp <= last)
            || self
                .frame_timestamp
                .is_some_and(|frame| (timestamp - frame).abs() > 0.25)
        {
            return;
        }
        let sensor = match motion_angles(quaternion) {
            Some(MotionOrientation::Angles(sensor)) => sensor,
            Some(MotionOrientation::Vertical) => {
                self.motion.baseline = None;
                return;
            }
            None => return,
        };
        if self
            .motion
            .last_sample
            .is_some_and(|last| timestamp - last > 1.0)
        {
            self.motion.baseline = None;
        }
        self.motion.last_sample = Some(timestamp);
        let Some(baseline) = &self.motion.baseline else {
            let yaw = verse::controller::wrap(self.world.player.yaw + self.world.camera.yaw_offset);
            self.motion.baseline = Some(MotionBaseline {
                sensor,
                camera: [yaw, self.world.camera.pitch],
            });
            // Taking over the orbit preserves the view and makes left movement
            // follow the direction the camera faces from the first sample.
            self.world.player.yaw = yaw;
            self.world.camera.yaw_offset = 0.0;
            return;
        };
        self.world.player.yaw = verse::controller::wrap(
            baseline.camera[0] + verse::controller::wrap(sensor[0] - baseline.sensor[0]),
        );
        self.world.camera.yaw_offset = 0.0;
        self.world.camera.pitch = (baseline.camera[1] + sensor[1] - baseline.sensor[1])
            .clamp(verse::camera::MIN_PITCH, verse::camera::MAX_PITCH);
    }

    fn panel_open(&self) -> bool {
        self.computer_open || self.gym_open
    }

    fn require_gym_panel(&self) -> Result<(), String> {
        if self.lifecycle.active() && !self.spawn_pending && self.gym_open && self.gym().inside {
            Ok(())
        } else {
            Err("Open the Gym board while inside to use its controls".into())
        }
    }

    fn sync_gym_interest(&mut self) {
        self.gym_board
            .set_active(self.lifecycle.active() && !self.spawn_pending && self.gym().inside);
    }

    fn gym(&self) -> verse::runtime::Gym {
        let viewport = self.lifecycle.viewport();
        self.world
            .gym(viewport.width() as f32 / viewport.height().max(1) as f32)
    }

    /// Called only after pointer or accessibility picking validates the target.
    fn open_computer(&mut self) {
        self.reset_motion();
        self.computer_open = true;
        self.gym_open = false;
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
    }

    fn computer_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.computer_hit_with_entities(
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    fn computer(&self) -> verse::runtime::Computer {
        let viewport = self.lifecycle.viewport();
        let aspect = if viewport.width() == 0 || viewport.height() == 0 {
            0.0
        } else {
            viewport.width() as f32 / viewport.height() as f32
        };
        self.world.computer(aspect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene {
        Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: false,
        })
        .unwrap()
    }
    fn gym_scene() -> Scene {
        Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: true,
        })
        .unwrap()
    }

    #[test]
    fn gym_fixture_loads_only_after_walking_inside_and_pauses_on_exit() {
        let mut scene = gym_scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        assert!(!scene.gym().inside);
        assert!(!scene.gym_board.view().active);
        assert!(scene.gym_board.view().runs.is_empty());
        assert!(scene.action(Request::GymLaunch).is_err());
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 100.0, 200.0).unwrap();
        for frame in 1..=160 {
            scene.update(1.0 + frame as f64 / 30.0).unwrap();
        }
        assert!(scene.gym().inside);
        assert!(scene.gym().near);
        assert!(scene.gym_board.view().active);
        assert!(!scene.gym_board.view().runs.is_empty());
        assert!(
            scene.gym_view().is_none(),
            "details require explicit board interaction"
        );
        scene.action(Request::InteractGym).unwrap();
        assert!(scene.gym_view().is_some());
        assert!(scene.touches.is_empty());
        let frame = serde_json::to_value(scene.packet()).unwrap();
        assert!(
            frame.get("gym_board").is_none(),
            "frame packets omit the run catalog"
        );
        scene.activate(false).unwrap();
        assert!(!scene.gym_board.view().active);
        assert!(scene.gym_view().is_none());
        assert!(scene.action(Request::GymLaunch).is_err());
        scene.activate(true).unwrap();
        scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
        scene.update(9.0).unwrap();
        assert!(!scene.gym_board.view().active);
        assert!(!scene.gym_open);
    }

    #[test]
    fn gym_requires_deliberate_selection_and_retains_the_preview_refusal_after_leaving() {
        let mut scene = gym_scene();
        scene.activate(true).unwrap();
        let mut near = verse::world::GYM_BOARD;
        near.x -= 3.0;
        near.y = 0.0;
        scene
            .world
            .set_spawn(near, std::f32::consts::FRAC_PI_2)
            .unwrap();
        scene.update(1.0).unwrap();
        let recipe = scene.gym_board.view().recipes[0].id.clone();
        assert!(
            scene
                .action(Request::GymSelectRecipe { id: recipe.clone() })
                .is_err()
        );
        scene.action(Request::InteractGym).unwrap();
        assert!(scene.action(Request::GymLaunch).is_err());
        scene
            .action(Request::GymSelectRecipe { id: recipe })
            .unwrap();
        scene.action(Request::GymLaunch).unwrap();
        let refused = scene.gym_board.view().launch.unwrap();
        assert_eq!(refused.phase, "rejected");
        assert!(refused.receipt.is_none());
        assert!(
            refused
                .error
                .as_deref()
                .unwrap()
                .contains("No training or evaluation was started")
        );
        scene.action(Request::CloseGym).unwrap();
        scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
        scene.update(1.03).unwrap();
        assert_eq!(
            scene.gym_board.view().launch.unwrap().request_id,
            refused.request_id
        );
        assert!(scene.gym_view().is_none());
    }

    #[test]
    fn invalid_relay_cannot_replace_a_previous_choice() {
        let mut scene = scene();
        scene.synthetic = false;
        scene.connect("wss://example.test/".into()).unwrap();
        for invalid in [
            "ws://example.test",
            "wss://user:password@example.test",
            "wss://example.test/?token=secret",
            "wss:///",
            "https://example.test",
        ] {
            assert!(scene.connect(invalid.into()).is_err());
            assert_eq!(scene.relay.as_deref(), Some("wss://example.test/"));
            assert!(scene.session.is_none());
        }
    }
    #[test]
    fn touches_move_shared_player_and_pause_cancels_motion() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 100.0, 200.0).unwrap();
        let start = scene.world.player.pos;
        for i in 1..=30 {
            scene.update(1.0 + i as f64 / 30.0).unwrap();
        }
        assert!(scene.world.player.pos.distance(start) > 3.0);
        assert!(
            scene.computer().near,
            "the desk stops forward movement within reach"
        );
        scene.activate(false).unwrap();
        let stopped = scene.world.player.pos;
        scene.update(100.0).unwrap();
        scene.activate(true).unwrap();
        scene.update(101.0).unwrap();
        scene.update(101.03).unwrap();
        assert_eq!(scene.world.player.pos, stopped);
        assert!(scene.touches.is_empty());
    }

    #[test]
    fn computer_requires_approach_and_releases_held_input() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        assert!(scene.action(Request::InteractComputer).is_err());
        scene.update(1.0).unwrap();
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 100.0, 200.0).unwrap();
        for frame in 1..=20 {
            scene.update(1.0 + frame as f64 / 30.0).unwrap();
        }
        scene.action(Request::Sprint { enabled: true }).unwrap();
        scene.action(Request::Jump).unwrap();
        scene.action(Request::InteractComputer).unwrap();
        assert!(scene.packet().computer_open);
        assert!(scene.touches.is_empty());
        let stopped = scene.world.player.pos;
        scene.pointer(2, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(2, PointerPhase::Move, 100.0, 200.0).unwrap();
        scene.action(Request::Jump).unwrap();
        scene.action(Request::Sprint { enabled: true }).unwrap();
        scene.update(2.0).unwrap();
        assert_eq!(scene.world.player.pos, stopped);
        scene.activate(false).unwrap();
        assert!(
            scene.packet().computer_open,
            "camera permission prompts preserve the panel"
        );
        scene.activate(true).unwrap();
        scene.update(3.0).unwrap();
        assert!(scene.packet().computer_open);
        scene.action(Request::CloseComputer).unwrap();
        assert!(!scene.packet().computer_open);
        assert!(!scene.input().forward);
        scene.action(Request::InteractComputer).unwrap();
        scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
        scene.update(3.1).unwrap();
        assert!(
            !scene.packet().computer_open,
            "moving away invalidates the open panel"
        );
        let packet = serde_json::to_value(scene.packet()).unwrap();
        assert_eq!(packet["computer"]["near"], false);
        assert_eq!(packet["computer_open"], false);
        assert!(packet["computer"]["screen_x"].as_f64().unwrap().is_finite());
    }
    fn computer_scene() -> (Scene, [f32; 2]) {
        let mut scene = scene();
        scene.activate(true).unwrap();
        let mut spawn = verse::world::SPAWN;
        spawn.z = -7.5;
        scene.world.set_spawn(spawn, 0.0).unwrap();
        scene.update(1.0).unwrap();
        scene.update(1.01).unwrap();
        let computer = scene.computer();
        let size = scene.lifecycle.viewport().logical_size();
        let point = [computer.screen_x * size[0], computer.screen_y * size[1]];
        assert!(scene.computer_hit(point[0], point[1]));
        (scene, point)
    }

    #[test]
    fn physical_monitor_tap_opens_without_moving_in_both_camera_modes() {
        for mode in [CameraMode::Touch, CameraMode::Motion] {
            let (mut scene, [x, y]) = computer_scene();
            scene.action(Request::CameraMode { mode }).unwrap();
            let position = scene.world.player.pos;
            let yaw = scene.world.player.yaw;
            scene.pointer(1, PointerPhase::Down, x, y).unwrap();
            scene.update(1.1).unwrap();
            scene.pointer(1, PointerPhase::Up, x, y).unwrap();
            assert!(scene.computer_open);
            assert!(scene.touches.is_empty());
            assert_eq!(scene.world.player.pos, position);
            assert_eq!(scene.world.player.yaw, yaw);
        }
    }

    #[test]
    fn exposed_monitor_edge_opens_even_when_its_center_is_occluded() {
        let (mut scene, [center_x, center_y]) = computer_scene();
        let size = scene.lifecycle.viewport().logical_size();
        let view = scene.world.view(size[0] / size[1]);
        let blocker_center = (view.eye + verse::world::COMPUTER_SCREEN) * 0.5;
        let corners = [(-0.1, -0.5), (0.1, -0.5), (0.1, 0.5), (-0.1, 0.5)].map(|(x, y)| {
            let mut corner = blocker_center;
            corner.x += x;
            corner.y += y;
            corner
        });
        scene.presented_entities.quad(corners);
        assert!(!scene.computer_hit(center_x, center_y));
        assert!(scene.action(Request::InteractComputer).is_err());
        let mut edge = verse::world::COMPUTER_SCREEN;
        edge.x += 0.9;
        let clip = view.view_proj * edge.extend(1.0);
        let x = (clip.x / clip.w * 0.5 + 0.5) * size[0];
        let y = (0.5 - clip.y / clip.w * 0.5) * size[1];
        assert!(scene.computer_hit(x, y));
        scene.pointer(1, PointerPhase::Down, x, y).unwrap();
        scene.pointer(1, PointerPhase::Up, x, y).unwrap();
        assert!(scene.computer_open);
    }

    #[test]
    fn monitor_gestures_reject_drag_cancel_long_hold_and_multiple_fingers() {
        for case in 0..6 {
            let (mut scene, [x, y]) = computer_scene();
            scene.pointer(1, PointerPhase::Down, x, y).unwrap();
            match case {
                0 => {
                    scene.pointer(1, PointerPhase::Move, x + 20.0, y).unwrap();
                    scene.pointer(1, PointerPhase::Move, x, y).unwrap();
                }
                1 => scene.pointer(1, PointerPhase::Cancel, f32::NAN, y).unwrap(),
                2 => {
                    scene.update(2.0).unwrap();
                }
                3 => scene.pointer(2, PointerPhase::Down, 10.0, 300.0).unwrap(),
                4 => {
                    scene.pointer(1, PointerPhase::Up, f32::NAN, y).unwrap();
                }
                5 => {
                    scene.world.set_spawn(verse::world::SPAWN, 0.0).unwrap();
                }
                _ => unreachable!(),
            }
            scene.pointer(1, PointerPhase::Up, x, y).unwrap();
            assert!(!scene.computer_open, "invalid gesture {case}");
            assert!(!scene.touches.contains_key(&1));
        }
        let (mut scene, [x, y]) = computer_scene();
        scene.pointer(1, PointerPhase::Down, 10.0, 30.0).unwrap();
        scene.pointer(1, PointerPhase::Up, x, y).unwrap();
        assert!(!scene.computer_open, "a tap must start on the monitor");
    }

    #[test]
    fn bad_and_excess_touches_do_not_poison_camera_or_keep_moving() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        scene.pointer(2, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(3, PointerPhase::Down, 50.0, 100.0).unwrap();
        assert_eq!(scene.touches.len(), 2);
        assert!(scene.pointer(2, PointerPhase::Move, f32::NAN, 0.0).is_err());
        scene.pointer(2, PointerPhase::Move, 320.0, 320.0).unwrap();
        assert!(scene.world.player.yaw.is_finite());
        scene
            .pointer(1, PointerPhase::Cancel, f32::NAN, 0.0)
            .unwrap();
        assert!(!scene.input().forward);
        assert!(scene.connect("wss://example.test".into()).is_err());
        scene.packet().view.validate().unwrap();
    }

    fn product(a: [f32; 4], b: [f32; 4]) -> [f32; 4] {
        let [x, y, z, w] = a;
        let [u, v, t, s] = b;
        [
            w * u + x * s + y * t - z * v,
            w * v - x * t + y * s + z * u,
            w * t + x * v - y * u + z * s,
            w * s - x * u - y * v - z * t,
        ]
    }

    /// Raw portrait reference-to-device attitude: upright points through the
    /// back of the phone along reference +Y, with reference +Z toward its top.
    fn attitude(yaw: f32, down_pitch: f32, screen_roll: f32) -> [f32; 4] {
        let x = (down_pitch - std::f32::consts::FRAC_PI_2) * 0.5;
        let z = yaw * 0.5;
        let r = screen_roll * 0.5;
        product(
            [0.0, 0.0, r.sin(), r.cos()],
            product([x.sin(), 0.0, 0.0, x.cos()], [0.0, 0.0, z.sin(), z.cos()]),
        )
    }

    fn close(a: f32, b: f32) {
        assert!((a - b).abs() < 0.0001, "{a} != {b}");
    }

    fn motion_scene() -> Scene {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene
            .action(Request::CameraMode {
                mode: CameraMode::Motion,
            })
            .unwrap();
        scene
    }

    #[test]
    fn motion_mapping_has_portrait_axes_without_screen_roll() {
        for (yaw, pitch) in [(0.0, 0.0), (0.7, 0.3), (-0.9, -0.4)] {
            for roll in [0.0, 0.8, -2.4] {
                let q = attitude(yaw, pitch, roll);
                for q in [q, q.map(|v| -v)] {
                    let MotionOrientation::Angles(angles) = motion_angles(q).unwrap() else {
                        panic!("a portrait attitude must have a heading");
                    };
                    close(angles[0], yaw);
                    close(angles[1], pitch);
                }
            }
        }
        assert!(motion_angles([0.0; 4]).is_none());
        assert!(motion_angles([f32::NAN; 4]).is_none());
        assert!(motion_angles([f32::MAX; 4]).is_none());
    }

    #[test]
    fn enabling_motion_preserves_view_and_wraps_yaw_across_pi() {
        let mut scene = motion_scene();
        scene.world.player.yaw = 0.4;
        scene.world.camera.yaw_offset = 0.2;
        let pitch = scene.world.camera.pitch;
        scene.device_motion(attitude(3.1, 0.1, 0.0), 1.01);
        close(scene.world.player.yaw, 0.6);
        close(scene.world.camera.yaw_offset, 0.0);
        close(scene.world.camera.pitch, pitch);
        scene.device_motion(attitude(-3.1, 0.3, 0.8), 1.02);
        close(scene.world.player.yaw, 0.6 + std::f32::consts::TAU - 6.2);
        close(scene.world.camera.pitch, pitch + 0.2);
        let orientation = (scene.world.player.yaw, scene.world.camera.pitch);
        scene.device_motion(attitude(-3.1, 0.3, 0.8).map(|v| -v), 1.03);
        close(scene.world.player.yaw, orientation.0);
        close(scene.world.camera.pitch, orientation.1);
        let packet = serde_json::to_value(scene.packet()).unwrap();
        assert_eq!(packet["camera_mode"], "motion");
        assert_eq!(packet["motion_needed"], true);
        assert!(packet["camera_yaw"].as_f64().unwrap().is_finite());
        assert!(packet["camera_pitch"].as_f64().unwrap().is_finite());
    }

    #[test]
    fn motion_left_hold_moves_while_phone_turns_and_right_touch_is_ignored() {
        let mut scene = motion_scene();
        scene.device_motion(attitude(0.0, 0.0, 0.0), 1.01);
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        assert!(scene.input().forward);
        scene.pointer(2, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(2, PointerPhase::Move, 390.0, 390.0).unwrap();
        assert_eq!(scene.touches.len(), 1);
        close(scene.world.player.yaw, 0.0);
        let start = scene.world.player.pos;
        scene.device_motion(attitude(std::f32::consts::FRAC_PI_2, 0.0, 0.0), 1.02);
        for frame in 1..=10 {
            scene.update(1.0 + f64::from(frame) / 30.0).unwrap();
        }
        assert!(scene.world.player.pos.x > start.x + 1.0);
        close(scene.world.player.pos.z, start.z);
        scene.pointer(1, PointerPhase::Move, 130.0, 350.0).unwrap();
        let input = scene.input();
        assert!(input.backward && input.strafe_right && !input.forward);
        scene.pointer(1, PointerPhase::Up, 130.0, 350.0).unwrap();
        assert!(!scene.input().forward && !scene.input().backward);
        scene
            .action(Request::CameraMode {
                mode: CameraMode::Touch,
            })
            .unwrap();
        scene.pointer(3, PointerPhase::Down, 100.0, 300.0).unwrap();
        assert!(
            !scene.input().forward,
            "touch mode still requires joystick drag"
        );
        scene.pointer(3, PointerPhase::Move, 100.0, 270.0).unwrap();
        assert!(scene.input().forward);
        scene.pointer(4, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(4, PointerPhase::Move, 320.0, 300.0).unwrap();
        close(scene.world.player.yaw, std::f32::consts::FRAC_PI_2 - 0.08);
    }

    #[test]
    fn invalid_and_stale_motion_do_not_poison_the_baseline() {
        let mut scene = motion_scene();
        scene.device_motion(attitude(0.0, 0.0, 0.0), 1.01);
        let q = attitude(0.5, 0.4, 0.0);
        for timestamp in [f64::NAN, f64::INFINITY, -1.0, 0.9, 1.01, 2.0] {
            scene.device_motion(q, timestamp);
        }
        for invalid in [[0.0; 4], [f32::INFINITY; 4], [f32::NAN; 4]] {
            scene.device_motion(invalid, 1.2);
        }
        assert_eq!(scene.motion.last_sample, Some(1.01));
        close(scene.world.player.yaw, 0.0);
        scene.device_motion(q, 1.02);
        close(scene.world.player.yaw, 0.5);
        scene.update(2.5).unwrap();
        scene.device_motion(attitude(-1.0, -0.3, 0.0), 2.51);
        close(scene.world.player.yaw, 0.5);
        scene.device_motion(attitude(-0.9, -0.3, 0.0), 2.52);
        close(scene.world.player.yaw, 0.6);
    }

    #[test]
    fn motion_pitch_is_clamped_and_vertical_phone_rebaselines_without_jump() {
        let mut scene = motion_scene();
        scene.device_motion(attitude(0.0, -1.0, 0.0), 1.01);
        scene.device_motion(attitude(0.0, 1.4, 0.0), 1.02);
        close(scene.world.camera.pitch, verse::camera::MAX_PITCH);
        scene.action(Request::ResetMotion).unwrap();
        scene.device_motion(attitude(0.0, 1.0, 0.0), 1.03);
        scene.device_motion(attitude(0.0, -1.4, 0.0), 1.04);
        close(scene.world.camera.pitch, verse::camera::MIN_PITCH);
        scene.device_motion(attitude(0.8, 0.0, 0.0), 1.05);
        let previous = (scene.world.player.yaw, scene.world.camera.pitch);
        scene.device_motion([0.0, 0.0, 0.0, 1.0], 1.06);
        assert!(scene.motion.baseline.is_none());
        scene.device_motion(attitude(-2.0, 0.0, 0.0), 1.07);
        close(scene.world.player.yaw, previous.0);
        close(scene.world.camera.pitch, previous.1);
    }

    #[test]
    fn motion_lifecycle_resets_and_ignores_samples_while_inactive_or_in_panels() {
        let mut scene = motion_scene();
        scene.device_motion(attitude(0.0, 0.0, 0.0), 1.01);
        scene.device_motion(attitude(0.4, 0.0, 0.0), 1.02);
        scene.activate(false).unwrap();
        assert!(!scene.packet().motion_needed);
        scene.device_motion(attitude(1.4, 0.0, 0.0), 1.03);
        close(scene.world.player.yaw, 0.4);
        scene.activate(true).unwrap();
        scene.update(2.0).unwrap();
        scene.device_motion(attitude(1.4, 0.0, 0.0), 2.01);
        close(scene.world.player.yaw, 0.4);
        scene.device_motion(attitude(1.5, 0.0, 0.0), 2.02);
        close(scene.world.player.yaw, 0.5);
        for computer in [true, false] {
            scene.reset_motion();
            scene.computer_open = computer;
            scene.gym_open = !computer;
            assert!(!scene.packet().motion_needed);
            scene.device_motion(attitude(-1.0, 0.0, 0.0), 2.03);
            close(scene.world.player.yaw, 0.5);
            scene
                .action(if computer {
                    Request::CloseComputer
                } else {
                    Request::CloseGym
                })
                .unwrap();
            scene.device_motion(attitude(-1.0, 0.0, 0.0), if computer { 2.04 } else { 2.05 });
            close(scene.world.player.yaw, 0.5);
        }
        scene.spawn_pending = true;
        scene.reset_motion();
        assert!(!scene.packet().motion_needed);
        scene.device_motion(attitude(0.0, 0.0, 0.0), 2.06);
        scene.update(2.1).unwrap(); // No session: finish the pending spawn.
        assert!(scene.packet().motion_needed);
        scene.device_motion(attitude(0.0, 0.0, 0.0), 2.11);
        close(scene.world.player.yaw, 0.5);
    }

    #[test]
    fn wire_motion_requests_are_closed_and_default_is_touch() {
        assert_eq!(
            serde_json::to_value(scene().packet()).unwrap()["camera_mode"],
            "touch"
        );
        let request: Request =
            serde_json::from_str(r#"{"action":"camera_mode","mode":"motion"}"#).unwrap();
        assert!(matches!(
            request,
            Request::CameraMode {
                mode: CameraMode::Motion
            }
        ));
        assert!(
            serde_json::from_str::<Request>(r#"{"action":"camera_mode","mode":"gyro"}"#).is_err()
        );
        assert!(
            serde_json::from_str::<Request>(
                r#"{"action":"device_motion","quaternion":[0,0,0,1],"timestamp":1,"extra":true}"#
            )
            .is_err()
        );
    }
}
