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
}

pub(crate) struct Scene {
    pub world: WorldRuntime,
    pub lifecycle: SurfaceLifecycle,
    pub session: Option<Session>,
    secret: secp256k1::SecretKey,
    relay: Option<String>,
    synthetic: bool,
    spawn_pending: bool,
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
            secret,
            relay: None,
            synthetic: config.synthetic,
            spawn_pending: false,
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
        self.lifecycle
            .set_active(active)
            .map_err(|e| e.to_string())?;
        if !active {
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
        self.gym_board.set_active(false);
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.session = None;
        self.relay = None;
    }

    pub fn pointer(&mut self, id: u64, phase: PointerPhase, x: f32, y: f32) -> Result<(), String> {
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            self.touches.remove(&id);
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
                if self.touches.len() >= 2 {
                    return Ok(());
                }
                let size = self.lifecycle.viewport().logical_size();
                if x < 0.0 || y < 0.0 || x > size[0] || y > size[1] {
                    return Ok(());
                }
                let movement = x < size[0] * 0.5;
                if self.touches.values().any(|p| p.movement == movement) {
                    return Ok(());
                }
                self.touches.insert(
                    id,
                    Touch {
                        origin: [x, y],
                        latest: [x, y],
                        movement,
                    },
                );
                if !movement {
                    self.world.apply(Action::FaceCamera)?;
                }
            }
            PointerPhase::Move => {
                if let Some(touch) = self.touches.get_mut(&id) {
                    if !touch.movement {
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
        if let Some(touch) = self.touches.values().find(|p| p.movement) {
            let x = touch.latest[0] - touch.origin[0];
            let y = touch.latest[1] - touch.origin[1];
            input.forward = y < -12.0;
            input.backward = y > 12.0;
            input.strafe_left = x < -12.0;
            input.strafe_right = x > 12.0;
        }
        input.mouse_look = self.touches.values().any(|p| !p.movement);
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
        if self.spawn_pending {
            self.gym_board.set_active(false);
            if let Some(session) = &mut self.session {
                if let Some(spawn) =
                    session.poll_spawn(&self.world.world.blockers, verse::world::HALF)
                {
                    self.world.set_spawn(spawn.pos, spawn.yaw)?;
                    self.spawn_pending = false;
                } else {
                    return Ok(Some(0.0));
                }
            } else {
                self.spawn_pending = false;
            }
        }
        let input = self.input();
        self.world.tick(&input, dt);
        if !self.computer().near {
            self.computer_open = false;
        }
        if !self.gym().inside {
            self.gym_open = false;
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
                if !self.lifecycle.active() || !computer.near || !computer.visible {
                    return Err("Walk up to the computer to open it".into());
                }
                self.computer_open = true;
                self.gym_open = false;
                self.touches.clear();
                self.jump = false;
                self.sprint = false;
                Ok(())
            }
            Request::CloseComputer => {
                self.computer_open = false;
                Ok(())
            }
            Request::InteractGym => {
                let gym = self.gym();
                if !self.lifecycle.active() || !gym.inside || !gym.near || !gym.visible {
                    return Err("Walk inside the Gym and approach its board to open it".into());
                }
                self.gym_open = true;
                self.computer_open = false;
                self.touches.clear();
                self.jump = false;
                self.sprint = false;
                Ok(())
            }
            Request::CloseGym => {
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
}
