//! Coder's native Verse surface. Scene behavior comes from the shared Verse
//! runtime; this adapter owns touch gestures and mobile connection preferences.
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
use verse::doors::{DemoItem, DoorId, DoorIntent};
use verse::runtime::{Action, WorldRuntime};
use verse::session::Session;
use verse::zones::Intent as ZoneIntent;

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
    #[serde(default)]
    pub world_relay: Option<String>,
    /// An explicit Leave choice overrides the public default and any stale URL.
    #[serde(default)]
    pub world_offline: bool,
    #[serde(default)]
    pub door_preferences: Option<String>,
    #[serde(default)]
    pub zone_cache_directory: Option<String>,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Snapshot,
    ZoneCredits,
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
        received_at: f64,
    },
    ResetMotion,
    RecenterCamera,
    HudInsets {
        top: f32,
        right: f32,
        bottom: f32,
        left: f32,
    },
    MapToggle,
    MapCancel,
    MapWalk {
        x: f32,
        z: f32,
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
    PinchZoom {
        scale: f32,
    },
    Connect {
        relay: String,
    },
    Disconnect,
    DoorHold {
        item: DemoItem,
    },
    DoorTap {
        door: DoorId,
    },
    DoorReset {
        door: DoorId,
    },
    Zone {
        intent: ZoneIntent,
    },
    PetCompanion,
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
    connection: Connection,
    world_public_key: String,
    remote_entities: usize,
    live_remote_entities: usize,
    presented_remote_vertices: usize,
    map: verse::minimap::Snapshot,
    doors: DoorPacket,
    zone: ZonePacket,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub credits: Option<&'static str>,
    door_preferences: String,
    door_preferences_revision: u64,
    pub error: Option<String>,
    frames_presented: u64,
    position: [f32; 3],
    camera_mode: CameraMode,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    motion_needed: bool,
    companion: Companion,
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

#[derive(Serialize)]
struct ZonePacket {
    #[serde(flatten)]
    state: verse::zones::Snapshot,
    hud: verse::zones::hud::Snapshot,
}

#[derive(Serialize)]
struct DoorPacket {
    held: DemoItem,
    doors: Vec<DoorView>,
    hud: verse::doors::hud::Snapshot,
    error: Option<String>,
}

#[derive(Serialize)]
struct DoorView {
    #[serde(flatten)]
    projection: verse::runtime::DoorProjection,
    label: &'static str,
    state: verse::doors::DoorPhase,
    destination: Option<&'static str>,
    remembered: Option<DemoItem>,
}

#[derive(Serialize)]
struct Connection {
    state: &'static str,
    label: &'static str,
    relay: Option<String>,
    error: Option<&'static str>,
}

fn connection(
    relay: Option<&str>,
    active: bool,
    synthetic: bool,
    status: Option<verse::session::Status>,
    error: Option<&'static str>,
) -> Connection {
    let (state, label) = if relay.is_none() {
        ("offline", "Offline")
    } else if !active {
        ("paused", "Paused")
    } else if synthetic {
        ("preview", "Preview")
    } else {
        match status {
            Some(verse::session::Status::Online) => ("connected", "Connected"),
            Some(verse::session::Status::Offline) => ("retrying", "Not connected"),
            _ => ("connecting", "Connecting…"),
        }
    };
    Connection {
        state,
        label,
        relay: relay.map(str::to_owned),
        error,
    }
}

fn validated_world_relay(relay: &str) -> Result<String, String> {
    let relay = relay.trim();
    if relay.len() > 2048
        || coder_connect::RelayPolicy::Production
            .validate(relay)
            .is_err()
    {
        return Err("Use a wss:// relay URL without credentials, a query, or a fragment".into());
    }
    Ok(relay.to_owned())
}

/// Coordinates are normalized from the top-left of the Metal viewport.
/// Visibility describes projection, not occlusion by another world object.
#[derive(Serialize)]
struct Companion {
    near: bool,
    visible: bool,
    screen_x: f32,
    screen_y: f32,
    distance: f32,
    reacting: bool,
    cooldown_seconds: f32,
    pet_count: u64,
}

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
        connection: connection(None, false, false, None, None),
        world_public_key: String::new(),
        remote_entities: 0,
        live_remote_entities: 0,
        presented_remote_vertices: 0,
        map: verse::minimap::MapHud::default().snapshot(
            [393.0, 852.0],
            [0.0, -10.0],
            false,
            "",
            None,
        ),
        doors: DoorPacket {
            held: DemoItem::Prism,
            doors: Vec::new(),
            hud: verse::doors::hud::DoorHud::default().snapshot(
                [393.0, 852.0],
                None,
                &verse::doors::Doors::default(),
                false,
                None,
            ),
            error: None,
        },
        zone: {
            let state = verse::zones::Snapshot::default();
            let hud = verse::zones::hud::Hud::default().snapshot([393.0, 852.0], &state, false);
            ZonePacket { state, hud }
        },
        credits: None,
        door_preferences: verse::doors::Doors::default().document(),
        door_preferences_revision: 0,
        error,
        frames_presented: frames,
        position,
        camera_mode: CameraMode::Touch,
        camera_yaw: 0.0,
        camera_pitch: verse::camera::FollowCamera::default().pitch,
        camera_distance: verse::camera::FollowCamera::default().distance,
        motion_needed: false,
        companion: Companion {
            near: false,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: 0.0,
            reacting: false,
            cooldown_seconds: 0.0,
            pet_count: 0,
        },
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorldTarget {
    Computer,
    Gym,
    Companion,
    Door(DoorId),
    Portal,
}

struct Touch {
    origin: [f32; 2],
    latest: [f32; 2],
    movement: bool,
    target: Option<WorldTarget>,
    tap_valid: bool,
    started: f64,
}

struct WorldTap {
    movement: bool,
    position: [f32; 2],
    finished: f64,
}

const TAP_DRIFT_POINTS: f32 = 12.0;
const WORLD_TAP_SECONDS: f64 = 0.25;
const DOUBLE_TAP_SECONDS: f64 = 0.35;
const DOUBLE_TAP_DISTANCE_POINTS: f32 = 32.0;

#[derive(Default)]
struct Motion {
    baseline: Option<MotionBaseline>,
    last_sample: Option<f64>,
    last_received: Option<f64>,
    target: Option<[f32; 2]>,
}

struct MotionBaseline {
    sensor: [f32; 2],
    camera: [f32; 2],
    heading_valid: bool,
}

const MOTION_RESPONSE_SECONDS: f32 = 0.06;
const MOTION_RESET_GAP_SECONDS: f64 = 1.0;

/// Native adapters supply a Hamilton quaternion from portrait device axes
/// (+X right, +Y top, +Z out of the screen) to a reference with +Z up. Rotate
/// the phone's back direction into that reference. Positive heading turns
/// left, matching Verse yaw; positive pitch looks down. Screen roll cannot
/// tilt the horizon, and q and -q produce the same direction.
struct MotionOrientation {
    angles: [f32; 2],
    horizontal: f32,
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
        -2.0 * (x * z + w * y),
        2.0 * (w * x - y * z),
        2.0 * (x * x + y * y) - 1.0,
    ];
    let horizontal = forward[0].hypot(forward[1]);
    Some(MotionOrientation {
        angles: [
            (-forward[0]).atan2(forward[1]),
            (-forward[2]).atan2(horizontal),
        ],
        horizontal,
    })
}

pub(crate) struct Scene {
    pub world: WorldRuntime,
    pub atlas: verse::ui::Atlas,
    map: verse::minimap::MapHud,
    map_error: Option<String>,
    door_hud: verse::doors::hud::DoorHud,
    door_notice: Option<String>,
    zone_hud: verse::zones::hud::Hud,
    pub lifecycle: SurfaceLifecycle,
    pub session: Option<Session>,
    /// Remote geometry from the last presented frame, also used for picking.
    pub presented_entities: verse::mesh::Mesh,
    secret: secp256k1::SecretKey,
    public_key: String,
    relay: Option<String>,
    restore_spawn: bool,
    synthetic: bool,
    spawn_pending: bool,
    camera_mode: CameraMode,
    motion: Motion,
    frame_timestamp: Option<f64>,
    touches: BTreeMap<u64, Touch>,
    pointer_clock: Instant,
    last_world_tap: Option<WorldTap>,
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
        let selected_relay = if config.world_offline {
            None
        } else {
            config
                .world_relay
                .or_else(|| (!config.synthetic).then(|| verse::session::PUBLIC_RELAY.to_owned()))
        };
        let (relay, initial_error) = match selected_relay {
            Some(value) => match validated_world_relay(&value) {
                Ok(relay) => (Some(relay), None),
                Err(error) => (None, Some(error)),
            },
            None => (None, None),
        };
        let restore_spawn = relay.is_some() && !config.synthetic;
        let mut gym_board = verse::gym::Board::new(secret, config.synthetic);
        let initial_gym_error = config
            .gym_code
            .as_deref()
            .and_then(|code| gym_board.configure(code).err());
        if config.synthetic_gym && !config.synthetic {
            return Err("The Gym preview requires synthetic mode".into());
        }
        let mut world = WorldRuntime::new();
        if let Some(directory) = config.zone_cache_directory {
            if directory.is_empty()
                || directory.len() > 4096
                || !std::path::Path::new(&directory).is_absolute()
            {
                return Err("The zone cache requires an absolute native cache path".into());
            }
            world.configure_zone_cache(directory.into());
        }
        let door_notice = config
            .door_preferences
            .as_deref()
            .and_then(|value| world.restore_door_state(value).err());
        if config.synthetic_gym {
            // This explicit fixture starts outside the entrance. The real touch
            // path must cross the boundary before the board loads its rows.
            let mut outside = verse::world::GYM_ENTRANCE;
            outside.x -= 2.0;
            world.set_spawn(outside, std::f32::consts::FRAC_PI_2)?;
        }
        Ok(Self {
            world,
            atlas: verse::ui::Atlas::new(12.0),
            map: verse::minimap::MapHud::default(),
            map_error: None,
            door_hud: verse::doors::hud::DoorHud::default(),
            door_notice,
            zone_hud: verse::zones::hud::Hud::default(),
            lifecycle,
            session: None,
            presented_entities: verse::mesh::Mesh::default(),
            secret,
            public_key: verse::identity::Identity::from_secret("phone", secret)?
                .signer
                .pubkey()
                .to_owned(),
            relay,
            restore_spawn,
            synthetic: config.synthetic,
            spawn_pending: false,
            camera_mode: CameraMode::Touch,
            motion: Motion::default(),
            frame_timestamp: None,
            touches: BTreeMap::new(),
            pointer_clock: Instant::now(),
            last_world_tap: None,
            jump: false,
            sprint: false,
            computer_open: false,
            gym_open: false,
            gym_configuration_error: initial_gym_error,
            gym_board,
            frames: 0,
            error: initial_error,
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
            self.world.zone_cancel_loading();
            self.zone_hud.clear_contacts();
            self.world.cancel_navigation();
            self.map.clear_contacts();
            self.door_hud.clear_contacts();
            self.world.cancel_door_interactions();
            self.presented_entities = verse::mesh::Mesh::default();
            self.touches.clear();
            self.jump = false;
            self.sprint = false;
            self.session = None;
        } else if self.session.is_none()
            && self.relay.is_some()
            && !self.synthetic
            && self.plaza_online_allowed()
        {
            self.start_session()?;
        }
        self.sync_gym_interest();
        Ok(())
    }

    pub fn connect(&mut self, relay: String) -> Result<(), String> {
        let relay = validated_world_relay(&relay)?;
        self.session = None;
        self.presented_entities = verse::mesh::Mesh::default();
        self.relay = Some(relay);
        // Joining from the computer must keep the current pose and panel. Only
        // a new app mount restores the signed pose from a remembered relay.
        self.restore_spawn = false;
        self.spawn_pending = false;
        if self.lifecycle.active() && !self.synthetic && self.plaza_online_allowed() {
            self.start_session()?;
        }
        Ok(())
    }

    fn start_session(&mut self) -> Result<(), String> {
        if !self.plaza_online_allowed() {
            return Err("The forest is a local zone".into());
        }
        let identity = verse::identity::Identity::from_secret("phone", self.secret)?;
        let mut session = Session::start_with_identity(
            identity,
            self.relay.as_deref().ok_or("No Verse relay selected")?,
        )?;
        session.set_publish_intervals(verse::session::PublishIntervals::mobile())?;
        self.spawn_pending = std::mem::take(&mut self.restore_spawn);
        if self.spawn_pending {
            session.begin_spawn(Duration::from_millis(1500));
        }
        self.reset_motion();
        self.gym_board.set_active(false);
        self.session = Some(session);
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.presented_entities = verse::mesh::Mesh::default();
        self.session = None;
        self.relay = None;
        self.restore_spawn = false;
        self.spawn_pending = false;
    }

    /// A resize invalidates input in the previous viewport's coordinate space.
    pub fn resize(&mut self, viewport: Viewport) -> Result<(), String> {
        let changed = self.lifecycle.viewport() != viewport;
        self.lifecycle.resize(viewport).map_err(|e| e.to_string())?;
        if changed {
            self.zone_hud.clear_contacts();
            self.map.clear_contacts();
            self.door_hud.clear_contacts();
            self.world.cancel_door_interactions();
            self.touches.clear();
            self.jump = false;
            self.reset_motion();
        }
        Ok(())
    }

    pub fn pointer(&mut self, id: u64, phase: PointerPhase, x: f32, y: f32) -> Result<(), String> {
        let point = [x, y];
        if self.zone_hud.captured(id) {
            match phase {
                PointerPhase::Move => self.zone_hud.moved(id, point),
                PointerPhase::Up | PointerPhase::Cancel => {
                    if let Some(intent) =
                        self.zone_hud
                            .up(id, point, matches!(phase, PointerPhase::Cancel))
                    {
                        self.zone_intent(intent)?;
                    }
                }
                PointerPhase::Down => {}
            }
            return Ok(());
        }
        if self.map.captured(id) {
            match phase {
                PointerPhase::Move => self.map.moved(id, point),
                PointerPhase::Up | PointerPhase::Cancel => {
                    if let Some(action) =
                        self.map
                            .up(id, point, matches!(phase, PointerPhase::Cancel))
                    {
                        self.map_action(action)?;
                    }
                }
                PointerPhase::Down => {}
            }
            return Ok(());
        }
        if self.door_hud.captured(id) {
            match phase {
                PointerPhase::Move => self.door_hud.moved(id, point),
                PointerPhase::Up | PointerPhase::Cancel => {
                    if let Some(intent) =
                        self.door_hud
                            .up(id, point, matches!(phase, PointerPhase::Cancel))
                    {
                        self.door_intent(intent)?;
                    }
                }
                PointerPhase::Down => {}
            }
            return Ok(());
        }
        if matches!(phase, PointerPhase::Down) {
            let snapshot = self.zone_hud_snapshot();
            if self.zone_hud.down(id, point, &snapshot) {
                self.cancel_taps();
                return Ok(());
            }
            let snapshot = self.map_snapshot();
            if self.map.down(id, point, &snapshot) {
                self.cancel_taps();
                return Ok(());
            }
            let snapshot = self.door_snapshot();
            if self.door_hud.down(id, point, &snapshot) {
                self.cancel_taps();
                return Ok(());
            }
        }
        // Gesture duration follows receipt time, not the last rendered frame.
        // A slow frame must not turn a long hold into a tap.
        self.pointer_at(id, phase, x, y, self.pointer_clock.elapsed().as_secs_f64())
    }

    fn pointer_at(
        &mut self,
        id: u64,
        phase: PointerPhase,
        x: f32,
        y: f32,
        timestamp: f64,
    ) -> Result<(), String> {
        if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
            // Always release input, including cancelled or malformed native events.
            let touch = self.touches.remove(&id);
            if matches!(phase, PointerPhase::Cancel) {
                self.cancel_taps();
                return Ok(());
            }
            let Some(touch) = touch else {
                return Ok(());
            };
            let elapsed = timestamp - touch.started;
            let size = self.lifecycle.viewport().logical_size();
            let valid = touch.tap_valid
                && (0.0..=size[0]).contains(&x)
                && (0.0..=size[1]).contains(&y)
                && (x - touch.origin[0]).hypot(y - touch.origin[1]) <= TAP_DRIFT_POINTS
                && (0.0..=0.65).contains(&elapsed)
                && self.lifecycle.active()
                && !self.panel_open()
                && !self.spawn_pending
                && (self.touches.is_empty()
                    || (!touch.movement
                        && touch.target.is_none()
                        && self
                            .touches
                            .values()
                            .all(|other| other.movement && other.target.is_none())));
            if valid && touch.target.is_some() && self.world_target(x, y) == touch.target {
                match touch.target {
                    Some(WorldTarget::Portal) => {
                        self.zone_intent_at(
                            if self.world.is_plaza() {
                                ZoneIntent::Enter
                            } else {
                                ZoneIntent::Return
                            },
                            Some([x, y]),
                        )?;
                    }
                    Some(WorldTarget::Computer) => self.open_computer(),
                    Some(WorldTarget::Gym) => self.open_gym(),
                    Some(WorldTarget::Companion) => {
                        self.world.pet_companion();
                    }
                    Some(WorldTarget::Door(door)) => {
                        self.door_intent_at(DoorIntent::Tap(door), Some([x, y]))?;
                    }
                    None => {}
                }
            } else if valid && touch.target.is_none() && elapsed <= WORLD_TAP_SECONDS {
                self.world_tap([x, y], timestamp, touch.movement);
            } else if self
                .last_world_tap
                .as_ref()
                .is_some_and(|tap| tap.movement == touch.movement)
            {
                self.last_world_tap = None;
            }
            return Ok(());
        }
        if !self.lifecycle.active() || self.panel_open() || self.spawn_pending {
            self.cancel_taps();
            return Ok(());
        }
        if !x.is_finite() || !y.is_finite() || x.abs() > 32768.0 || y.abs() > 32768.0 {
            self.cancel_taps();
            return Err("Touch coordinates exceed their bounds".into());
        }
        match phase {
            PointerPhase::Down => {
                if self.touches.contains_key(&id) {
                    self.cancel_taps();
                    return Err("Touch identity is already active".into());
                }
                let single_touch = self.touches.is_empty();
                let size = self.lifecycle.viewport().logical_size();
                let movement = x < size[0] * 0.5;
                // A right-side tap is independent from an established movement
                // hold. Near-simultaneous contacts still cancel taps for pinch.
                let beside_movement = !movement
                    && self.touches.values().all(|other| {
                        other.movement
                            && other.target.is_none()
                            && (!other.tap_valid || timestamp - other.started > 0.15)
                    });
                if !single_touch && !beside_movement {
                    self.cancel_taps();
                }
                if self.touches.len() >= 2 {
                    return Ok(());
                }
                if x < 0.0 || y < 0.0 || x > size[0] || y > size[1] {
                    self.cancel_taps();
                    return Ok(());
                }
                let target = single_touch.then(|| self.world_target(x, y)).flatten();
                if target.is_some() {
                    self.last_world_tap = None;
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
                        target,
                        tap_valid: single_touch || beside_movement,
                        started: timestamp,
                    },
                );
            }
            PointerPhase::Move => {
                if let Some(touch) = self.touches.get_mut(&id) {
                    let dragged =
                        (x - touch.origin[0]).hypot(y - touch.origin[1]) > TAP_DRIFT_POINTS;
                    touch.tap_valid &= !dragged;
                    if dragged {
                        // World objects capture a tap, not the rest of a drag.
                        // Continue with the control chosen by the starting side.
                        touch.target = None;
                    }
                    if !touch.tap_valid
                        && self
                            .last_world_tap
                            .as_ref()
                            .is_some_and(|tap| tap.movement == touch.movement)
                    {
                        self.last_world_tap = None;
                    }
                    if touch.target.is_none()
                        && !touch.movement
                        && self.camera_mode == CameraMode::Touch
                    {
                        self.world.apply(Action::FaceCamera)?;
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

    fn cancel_taps(&mut self) {
        self.last_world_tap = None;
        for touch in self.touches.values_mut() {
            touch.tap_valid = false;
        }
    }

    fn world_tap(&mut self, position: [f32; 2], timestamp: f64, movement: bool) {
        if self.last_world_tap.take().is_some_and(|previous| {
            previous.movement == movement
                && (0.0..=DOUBLE_TAP_SECONDS).contains(&(timestamp - previous.finished))
                && (position[0] - previous.position[0]).hypot(position[1] - previous.position[1])
                    <= DOUBLE_TAP_DISTANCE_POINTS
        }) {
            self.jump = true;
        } else {
            self.last_world_tap = Some(WorldTap {
                movement,
                position,
                finished: timestamp,
            });
        }
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
        if let Some(touch) = self
            .touches
            .values()
            .find(|p| p.movement && p.target.is_none())
        {
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
        input.mouse_look = self.motion_needed()
            || self
                .touches
                .values()
                .any(|p| !p.movement && p.target.is_none());
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
        if self.world.zone_tick() {
            self.reset_zone_inputs();
        }
        self.sync_zone_session()?;
        let camera_dt = self.frame_timestamp.map_or(0.0, |last| timestamp - last);
        self.frame_timestamp = Some(timestamp);
        if self.spawn_pending {
            self.gym_board.set_active(false);
            if let Some(session) = &mut self.session {
                if let Some(spawn) =
                    session.poll_spawn(&self.world.world.blockers, self.world.zone_half())
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
        if camera_dt > MOTION_RESET_GAP_SECONDS
            || self
                .motion
                .last_sample
                .is_some_and(|last| timestamp - last > MOTION_RESET_GAP_SECONDS)
        {
            self.reset_motion();
        }
        self.advance_motion(camera_dt as f32);
        let input = self.input();
        self.world.tick(&input, dt);
        self.map.tick(dt);
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
            Request::HudInsets {
                top,
                right,
                bottom,
                left,
            } => {
                self.map.set_insets([top, right, bottom, left])?;
                self.door_hud.set_insets([top, right, bottom, left])?;
                self.zone_hud.set_insets([top, right, bottom, left])
            }
            Request::MapToggle => self.map_action(verse::minimap::MapAction::Toggle),
            Request::MapCancel => self.map_action(verse::minimap::MapAction::Cancel),
            Request::MapWalk { x, z } => self.map_action(verse::minimap::MapAction::Walk([x, z])),
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
                received_at,
            } => {
                self.device_motion(quaternion, timestamp, received_at);
                Ok(())
            }
            Request::ResetMotion => {
                self.reset_motion();
                Ok(())
            }
            Request::RecenterCamera => {
                self.reset_motion();
                self.world.camera.yaw_offset = 0.0;
                self.world.camera.pitch = verse::camera::FollowCamera::default().pitch;
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
            Request::PinchZoom { scale } => {
                self.touches.clear();
                self.cancel_taps();
                self.jump = false;
                if !self.lifecycle.active() || self.panel_open() || self.spawn_pending {
                    return Ok(());
                }
                self.world.apply(Action::PinchZoom { scale })
            }
            Request::Connect { relay } => self.connect(relay),
            Request::Disconnect => {
                self.disconnect();
                Ok(())
            }
            Request::Zone { intent } => self.zone_intent(intent),
            Request::DoorHold { item } => self.door_intent(DoorIntent::Hold(item)),
            Request::DoorTap { door } => self.door_intent(DoorIntent::Tap(door)),
            Request::DoorReset { door } => self.door_intent(DoorIntent::Reset(door)),
            Request::PetCompanion => {
                let companion = self.world.companion(self.aspect());
                let size = self.lifecycle.viewport().logical_size();
                if !self.lifecycle.active()
                    || self.panel_open()
                    || self.spawn_pending
                    || !self
                        .companion_hit(companion.screen_x * size[0], companion.screen_y * size[1])
                {
                    return Err("Bring the companion into view to greet it".into());
                }
                self.cancel_taps();
                self.world.pet_companion();
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
                let size = self.lifecycle.viewport().logical_size();
                if !self.lifecycle.active()
                    || !self.gym_hit(gym.screen_x * size[0], gym.screen_y * size[1])
                {
                    return Err("Walk inside the Gym and approach its board to open it".into());
                }
                self.open_gym();
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
            Request::Snapshot | Request::ZoneCredits => Ok(()),
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
        packet.connection = connection(
            self.relay.as_deref(),
            self.lifecycle.active(),
            self.synthetic,
            self.session.as_ref().map(|session| session.status),
            self.session
                .as_ref()
                .and_then(|session| session.connection_error),
        );
        packet.world_public_key.clone_from(&self.public_key);
        packet.remote_entities = self
            .session
            .as_ref()
            .map_or(0, |session| session.crowd.len());
        packet.live_remote_entities = self
            .session
            .as_ref()
            .map_or(0, |session| session.crowd.live_len(Instant::now()));
        packet.presented_remote_vertices =
            self.presented_entities.faces.len() + self.presented_entities.lines.len();
        if !self.plaza_online_allowed() {
            packet.connection.state = "local_zone";
            packet.connection.label = if self.world.zone_loading() {
                "Loading zone"
            } else {
                "Local forest"
            };
        }
        packet.zone = ZonePacket {
            state: self.zone_snapshot(),
            hud: self.zone_hud_snapshot(),
        };
        packet.map = self.map_snapshot();
        packet.door_preferences = self.world.doors.document();
        packet.door_preferences_revision = self.world.doors.revision();
        packet.doors = DoorPacket {
            held: self.world.doors.held(),
            doors: DoorId::ALL
                .iter()
                .map(|&id| {
                    let state = self.world.doors.state(id);
                    DoorView {
                        projection: self.world.door(id, self.aspect()),
                        label: id.label(),
                        state: state.phase,
                        destination: state.selected.map(|value| value.label()),
                        remembered: state.last,
                    }
                })
                .collect(),
            hud: self.door_snapshot(),
            error: self.door_notice.clone(),
        };
        packet.camera_mode = self.camera_mode;
        packet.camera_yaw =
            verse::controller::wrap(self.world.player.yaw + self.world.camera.yaw_offset);
        packet.camera_pitch = self.world.camera.pitch;
        packet.camera_distance = self.world.camera.distance;
        packet.motion_needed = self.motion_needed();
        let companion = self.world.companion(self.aspect());
        packet.companion = Companion {
            near: companion.near,
            visible: companion.visible,
            screen_x: companion.screen_x,
            screen_y: companion.screen_y,
            distance: companion.distance,
            reacting: companion.reacting,
            cooldown_seconds: companion.cooldown_seconds,
            pet_count: self.world.agent.pet_count(),
        };
        packet.computer = self.computer().into();
        packet.computer_open = self.computer_open;
        packet.gym = self.gym().into();
        packet.gym_open = self.gym_open;
        packet.gym_revision = self.gym_board.revision();
        packet.gym_active = self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.gym().inside;
        packet
    }

    fn map_action(&mut self, action: verse::minimap::MapAction) -> Result<(), String> {
        if !self.lifecycle.active() || self.panel_open() || self.spawn_pending {
            return Err("Return to the world to use the map".into());
        }
        match action {
            verse::minimap::MapAction::Toggle => {
                self.map.expanded = !self.map.expanded;
                self.zone_hud.clear_contacts();
                self.door_hud.clear_contacts();
                self.cancel_taps();
            }
            verse::minimap::MapAction::Cancel => {
                self.world.cancel_navigation();
                self.map_error = None;
            }
            verse::minimap::MapAction::Walk(target) => {
                self.cancel_taps();
                self.jump = false;
                match self.world.navigate_to(target) {
                    Ok(()) => {
                        self.map_error = None;
                        self.map.expanded = false;
                    }
                    Err(error) => {
                        self.map_error = Some(error.to_string());
                        self.map.expanded = true;
                    }
                }
            }
        }
        Ok(())
    }

    fn map_snapshot(&self) -> verse::minimap::Snapshot {
        use verse::nav::NavigationStatus as Status;
        let state = self
            .map_error
            .as_deref()
            .unwrap_or(match self.world.navigation().status() {
                Status::Idle => "Choose a place to walk",
                Status::Walking => "Walking",
                Status::Arrived => "Arrived",
                Status::Cancelled => "Walk stopped",
                Status::Blocked => "Route blocked",
            });
        self.map.snapshot_for_zone(
            self.lifecycle.viewport().logical_size(),
            [self.world.player.pos.x, self.world.player.pos.z],
            self.lifecycle.active() && !self.panel_open() && !self.spawn_pending,
            state,
            self.world.navigation().destination(),
            self.world.zone,
        )
    }

    pub fn map_ui(&self) -> verse::ui::UiBatch {
        let mut ui = self.map.draw(
            &self.atlas,
            &self.map_snapshot(),
            &self.world.world.blockers,
            [self.world.player.pos.x, self.world.player.pos.z],
            self.world.player.yaw,
            self.world.navigation().waypoints(),
            self.lifecycle.viewport().scale(),
        );
        let door_ui = self.door_hud.draw(
            &self.atlas,
            &self.door_snapshot(),
            self.lifecycle.viewport().scale(),
        );
        ui.vertices.extend(door_ui.vertices);
        let zone_ui = self.zone_hud.draw(
            &self.atlas,
            &self.zone_hud_snapshot(),
            self.lifecycle.viewport().scale(),
        );
        ui.vertices.extend(zone_ui.vertices);
        ui
    }

    fn zone_hud_snapshot(&self) -> verse::zones::hud::Snapshot {
        self.zone_hud.snapshot(
            self.lifecycle.viewport().logical_size(),
            &self.zone_snapshot(),
            self.lifecycle.active() && !self.computer_open && !self.gym_open && !self.map.expanded,
        )
    }

    fn plaza_online_allowed(&self) -> bool {
        self.world.is_plaza() && !self.world.zone_loading()
    }

    fn zone_snapshot(&self) -> verse::zones::Snapshot {
        let mut snapshot = self.world.zone_snapshot(self.aspect());
        let size = self.lifecycle.viewport().logical_size();
        if snapshot.portal.visible
            && snapshot.portal.near
            && !self.portal_hit(
                snapshot.portal.screen_x * size[0],
                snapshot.portal.screen_y * size[1],
            )
        {
            snapshot.portal.visible = false;
            snapshot
                .controls
                .retain(|control| control.action != ZoneIntent::Enter);
        }
        snapshot
    }

    fn reset_zone_inputs(&mut self) {
        self.world.cancel_navigation();
        self.map.expanded = false;
        self.map.clear_contacts();
        self.map_error = None;
        self.door_hud.clear_contacts();
        self.zone_hud.clear_contacts();
        self.world.cancel_door_interactions();
        self.presented_entities = verse::mesh::Mesh::default();
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
        self.computer_open = false;
        self.gym_open = false;
        self.reset_motion();
        self.restore_spawn = false;
        self.spawn_pending = false;
        self.sync_gym_interest();
    }

    fn sync_zone_session(&mut self) -> Result<(), String> {
        if !self.plaza_online_allowed() {
            self.session = None;
            self.spawn_pending = false;
            self.restore_spawn = false;
            self.presented_entities = verse::mesh::Mesh::default();
        } else if self.lifecycle.active()
            && self.session.is_none()
            && self.relay.is_some()
            && !self.synthetic
        {
            self.start_session()?;
        }
        Ok(())
    }

    fn zone_intent(&mut self, intent: ZoneIntent) -> Result<(), String> {
        self.zone_intent_at(intent, None)
    }

    fn zone_intent_at(
        &mut self,
        intent: ZoneIntent,
        point: Option<[f32; 2]>,
    ) -> Result<(), String> {
        if !self.lifecycle.active() || self.computer_open || self.gym_open || self.map.expanded {
            return Err("Return to the world to use the portal".into());
        }
        if intent == ZoneIntent::Enter {
            let portal = self.world.zone_snapshot(self.aspect()).portal;
            let size = self.lifecycle.viewport().logical_size();
            let at = point.unwrap_or([portal.screen_x * size[0], portal.screen_y * size[1]]);
            if !self.portal_hit(at[0], at[1]) {
                return Err("Approach a visible portal to enter the forest".into());
            }
        }
        self.world.zone_intent(intent)?;
        if matches!(
            intent,
            ZoneIntent::Enter
                | ZoneIntent::Return
                | ZoneIntent::Cancel
                | ZoneIntent::Retry
                | ZoneIntent::StartEncounter
                | ZoneIntent::ResetEncounter
        ) {
            self.reset_zone_inputs();
        }
        self.sync_zone_session()
    }

    fn door_snapshot(&self) -> verse::doors::hud::Snapshot {
        self.door_hud.snapshot(
            self.lifecycle.viewport().logical_size(),
            self.world.nearest_door(self.aspect()),
            &self.world.doors,
            self.lifecycle.active()
                && self.world.is_plaza()
                && !self.panel_open()
                && !self.spawn_pending
                && !self.map.expanded,
            self.door_notice.as_deref(),
        )
    }

    fn door_intent(&mut self, intent: DoorIntent) -> Result<(), String> {
        self.door_intent_at(intent, None)
    }

    fn door_intent_at(
        &mut self,
        intent: DoorIntent,
        point: Option<[f32; 2]>,
    ) -> Result<(), String> {
        if !self.lifecycle.active()
            || !self.world.is_plaza()
            || self.panel_open()
            || self.spawn_pending
            || self.map.expanded
        {
            return Err("Return to the world to use a gate".into());
        }
        if let Some([x, y]) = point {
            // A visible edge remains tappable when the named accessibility
            // anchor is hidden. Recheck the actual point, never a second point.
            if !matches!(intent, DoorIntent::Tap(id) if self.door_hit(id, x, y)) {
                return Err("Tap a visible part of the gate".into());
            }
        } else {
            let near = self.world.nearest_door(self.aspect());
            let door = match intent {
                DoorIntent::Hold(_) => near,
                DoorIntent::Tap(id) | DoorIntent::Reset(id) => {
                    Some(id).filter(|id| Some(*id) == near)
                }
            }
            .ok_or("Approach a visible gate to use it")?;
            let projected = self.world.door(door, self.aspect());
            let size = self.lifecycle.viewport().logical_size();
            if !self.door_hit(
                door,
                projected.screen_x * size[0],
                projected.screen_y * size[1],
            ) {
                return Err("Approach the front of the gate to use it".into());
            }
        }
        self.cancel_taps();
        self.jump = false;
        self.door_notice = None;
        match intent {
            DoorIntent::Hold(item) => {
                self.world.hold_door_item(item);
            }
            DoorIntent::Reset(door) => {
                self.world.reset_door(door);
            }
            DoorIntent::Tap(door) => {
                if let Err(error) = self.world.tap_door(door) {
                    self.door_notice = Some(error);
                }
            }
        }
        Ok(())
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
        self.cancel_taps();
        self.motion.baseline = None;
        self.motion.target = None;
        // Keep the high-water mark across sensor restarts. An old native sample
        // must not become the baseline after a panel closes or the app resumes.
        if let Some(frame) = self.frame_timestamp {
            self.motion.last_sample = Some(self.motion.last_sample.unwrap_or(frame).max(frame));
        }
    }

    fn device_motion(&mut self, quaternion: [f32; 4], timestamp: f64, received_at: f64) {
        if !self.motion_needed()
            || !timestamp.is_finite()
            || !(0.0..=1e12).contains(&timestamp)
            || !received_at.is_finite()
            || !(0.0..=1e12).contains(&received_at)
            || !(-0.005..=0.25).contains(&(received_at - timestamp))
            || self
                .motion
                .last_sample
                .is_some_and(|last| timestamp <= last)
            || self
                .motion
                .last_received
                .is_some_and(|last| received_at < last)
            || self
                .frame_timestamp
                .is_some_and(|frame| received_at < frame)
        {
            return;
        }
        let Some(orientation) = motion_angles(quaternion) else {
            return;
        };
        let sensor = orientation.angles;
        if self
            .motion
            .last_sample
            .is_some_and(|last| timestamp - last > MOTION_RESET_GAP_SECONDS)
        {
            self.reset_motion();
        }
        self.motion.last_sample = Some(timestamp);
        self.motion.last_received = Some(received_at);
        let yaw = verse::controller::wrap(self.world.player.yaw + self.world.camera.yaw_offset);
        let Some(baseline) = &mut self.motion.baseline else {
            self.motion.baseline = Some(MotionBaseline {
                sensor,
                camera: [yaw, self.world.camera.pitch],
                heading_valid: orientation.horizontal >= 0.15,
            });
            self.motion.target = Some([yaw, self.world.camera.pitch]);
            // Taking over the orbit preserves the view and makes left movement
            // follow the direction the camera faces from the first sample.
            self.world.player.yaw = yaw;
            self.world.camera.yaw_offset = 0.0;
            return;
        };
        // Near a pole, heading is undefined. Keep pitch responsive, freeze yaw,
        // and recenter only yaw when the phone leaves the wider recovery band.
        // Hysteresis prevents noise from repeatedly entering and leaving it.
        let heading_threshold = if baseline.heading_valid { 0.08 } else { 0.15 };
        let target_yaw = if orientation.horizontal < heading_threshold {
            baseline.heading_valid = false;
            yaw
        } else if !baseline.heading_valid {
            baseline.sensor[0] = sensor[0];
            baseline.camera[0] = yaw;
            baseline.heading_valid = true;
            yaw
        } else {
            verse::controller::wrap(
                baseline.camera[0] + verse::controller::wrap(sensor[0] - baseline.sensor[0]),
            )
        };
        self.motion.target = Some([
            target_yaw,
            (baseline.camera[1] + sensor[1] - baseline.sensor[1])
                .clamp(verse::camera::MIN_PITCH, verse::camera::MAX_PITCH),
        ]);
    }

    fn advance_motion(&mut self, dt: f32) {
        if !self.motion_needed() {
            return;
        }
        let Some([yaw, pitch]) = self.motion.target else {
            return;
        };
        // Exponential response gives the same result at different display
        // rates. Sensor callbacks update the target; only frames move the view.
        let amount = -(-dt / MOTION_RESPONSE_SECONDS).exp_m1();
        let current = self.world.player.yaw + self.world.camera.yaw_offset;
        self.world.player.yaw =
            verse::controller::wrap(current + verse::controller::wrap(yaw - current) * amount);
        self.world.camera.yaw_offset = 0.0;
        self.world.camera.pitch += (pitch - self.world.camera.pitch) * amount;
    }

    fn panel_open(&self) -> bool {
        self.computer_open || self.gym_open || self.world.zone_loading()
    }

    fn require_gym_panel(&self) -> Result<(), String> {
        if self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.gym_open
            && self.gym().inside
        {
            Ok(())
        } else {
            Err("Open the Gym board while inside to use its controls".into())
        }
    }

    fn sync_gym_interest(&mut self) {
        self.gym_board.set_active(
            self.lifecycle.active()
                && self.plaza_online_allowed()
                && !self.spawn_pending
                && self.gym().inside,
        );
    }

    fn gym(&self) -> verse::runtime::Gym {
        let viewport = self.lifecycle.viewport();
        self.world
            .gym(viewport.width() as f32 / viewport.height().max(1) as f32)
    }

    /// Called only after pointer or accessibility picking validates the target.
    fn open_computer(&mut self) {
        self.zone_hud.clear_contacts();
        self.world.cancel_navigation();
        self.map.clear_contacts();
        self.door_hud.clear_contacts();
        self.world.cancel_door_interactions();
        self.reset_motion();
        self.computer_open = true;
        self.gym_open = false;
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
    }

    /// Called only after pointer or accessibility picking validates the board.
    fn open_gym(&mut self) {
        self.reset_motion();
        self.world.cancel_navigation();
        self.map.clear_contacts();
        self.door_hud.clear_contacts();
        self.world.cancel_door_interactions();
        self.zone_hud.clear_contacts();
        self.gym_open = true;
        self.computer_open = false;
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
    }

    fn aspect(&self) -> f32 {
        let size = self.lifecycle.viewport().logical_size();
        size[0] / size[1].max(1.0)
    }

    fn world_target(&self, x: f32, y: f32) -> Option<WorldTarget> {
        if self.companion_hit(x, y) {
            Some(WorldTarget::Companion)
        } else if self.computer_hit(x, y) {
            Some(WorldTarget::Computer)
        } else if self.gym_hit(x, y) {
            Some(WorldTarget::Gym)
        } else if self.portal_hit(x, y) {
            Some(WorldTarget::Portal)
        } else {
            DoorId::ALL
                .iter()
                .copied()
                .find(|id| self.door_hit(*id, x, y))
                .map(WorldTarget::Door)
        }
    }

    fn portal_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.zone_hit_with_entities(
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    fn door_hit(&self, id: DoorId, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.world.is_plaza()
            && !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.door_hit_with_entities(
                id,
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    fn companion_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.companion_hit_with_entities(
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    fn gym_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.world.is_plaza()
            && !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.gym_hit_with_entities(
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    fn computer_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.world.is_plaza()
            && !self.spawn_pending
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
            world_relay: None,
            world_offline: false,
            door_preferences: None,
            zone_cache_directory: None,
        })
        .unwrap()
    }
    fn cached_zone_scene() -> (Scene, tempfile::TempDir) {
        let cache = tempfile::tempdir().unwrap();
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../assets/verse/forest/7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7.vzp");
        std::fs::copy(
            source,
            cache
                .path()
                .join(format!("{}.vzp", verse::zones::assets::PACK_SHA256)),
        )
        .unwrap();
        let mut scene = scene();
        scene.world.configure_zone_cache(cache.path().to_owned());
        scene.activate(true).unwrap();
        scene
            .world
            .set_spawn([-12.0, 0.0, 8.0].into(), 0.0)
            .unwrap();
        (scene, cache)
    }

    #[test]
    fn zone_cache_setup_is_inert_and_entry_checks_proximity() {
        let temp = tempfile::tempdir().unwrap();
        let cache = temp.path().join("unopened-zone-cache");
        let mut scene = scene();
        scene.world.configure_zone_cache(cache.clone());
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        assert!(!cache.exists());
        assert!(scene.world.is_plaza());
        assert!(!scene.world.zone_loading());
        assert!(
            scene
                .action(Request::Zone {
                    intent: ZoneIntent::Enter
                })
                .is_err()
        );
        assert!(!cache.exists());
        assert!(scene.world.is_plaza());
    }

    #[test]
    fn zone_transition_clears_input_and_keeps_plaza_identity_out_of_forest() {
        let (mut scene, _cache) = cached_zone_scene();
        // A local socket keeps this an offline test while exercising a real
        // Session owner that must be dropped before any forest simulation.
        let relay_socket = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let relay = format!("wss://{}", relay_socket.local_addr().unwrap());
        scene.synthetic = false;
        scene.connect(relay.clone()).unwrap();
        assert!(scene.session.is_some());
        let before = scene.world.player.pos;
        scene.jump = true;
        scene.sprint = true;
        scene
            .action(Request::Zone {
                intent: ZoneIntent::Enter,
            })
            .unwrap();
        assert!(scene.world.zone_loading());
        assert!(scene.session.is_none());
        assert_eq!(scene.packet().connection.state, "local_zone");
        assert_eq!(
            scene.packet().connection.relay.as_deref(),
            Some(relay.as_str())
        );
        assert!(!scene.jump && !scene.sprint && scene.touches.is_empty());
        assert!(!scene.motion_needed());
        assert!(!scene.packet().gym_active);
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut clock = 1.0;
        while scene.world.zone_loading() && Instant::now() < deadline {
            scene.update(clock).unwrap();
            clock += 0.02;
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            scene.world.zone,
            verse::zones::ZoneId::Forest,
            "{:?}",
            scene.world.zone_snapshot(scene.aspect()).error
        );
        assert!(scene.session.is_none());
        assert!(!scene.packet().gym_active);
        assert!(!scene.packet().computer.near);
        assert!(!scene.packet().doors.hud.visible);
        assert!(scene.action(Request::InteractComputer).is_err());
        assert!(scene.action(Request::InteractGym).is_err());
        assert!(!scene.map_snapshot().landmarks.iter().any(|p| p.id == "gym"));
        scene.world.set_spawn([0.0, 0.0, 14.0].into(), 0.0).unwrap();
        scene
            .action(Request::Zone {
                intent: ZoneIntent::Return,
            })
            .unwrap();
        assert!(scene.world.is_plaza());
        assert_eq!(scene.world.player.pos, before);
        assert!(!scene.restore_spawn && !scene.spawn_pending);
        assert!(scene.session.is_some());
        assert_ne!(scene.packet().connection.state, "local_zone");
        assert_eq!(scene.relay.as_deref(), Some(relay.as_str()));
    }

    #[test]
    fn background_cancels_loading_and_late_ready_cannot_enter_a_zone() {
        let (mut scene, _cache) = cached_zone_scene();
        scene
            .action(Request::Zone {
                intent: ZoneIntent::Enter,
            })
            .unwrap();
        assert!(scene.world.zone_loading());
        scene.activate(false).unwrap();
        assert!(!scene.world.zone_loading());
        scene.activate(true).unwrap();
        for tick in 0..20 {
            scene.update(1.0 + tick as f64 * 0.02).unwrap();
            std::thread::sleep(Duration::from_millis(2));
        }
        assert!(scene.world.is_plaza());
        assert!(!scene.world.zone_loading());
        assert_eq!(scene.world.zone_revision, 0);
        assert!(scene.packet().zone.state.error.is_none());
    }

    #[test]
    fn a_remote_entity_hides_portal_controls_and_blocks_native_entry() {
        let (mut scene, _cache) = cached_zone_scene();
        let portal = scene.zone_snapshot().portal;
        assert!(portal.near && portal.visible);
        let size = scene.lifecycle.viewport().logical_size();
        let point = [portal.screen_x * size[0], portal.screen_y * size[1]];
        let anchor = verse::zones::ZoneId::Plaza.portal().with_y(2.5);
        let obstruction = scene.world.view(scene.aspect()).eye.lerp(anchor, 0.8);
        let right = anchor.with_x(0.4).with_y(0.0).with_z(0.0);
        let up = anchor.with_x(0.0).with_y(0.5).with_z(0.0);
        scene.presented_entities.quad([
            obstruction - right - up,
            obstruction + right - up,
            obstruction + right + up,
            obstruction - right + up,
        ]);
        assert!(!scene.portal_hit(point[0], point[1]));
        assert!(!scene.zone_snapshot().portal.visible);
        assert!(
            !scene
                .zone_snapshot()
                .controls
                .iter()
                .any(|c| c.action == ZoneIntent::Enter)
        );
        assert!(
            scene
                .action(Request::Zone {
                    intent: ZoneIntent::Enter
                })
                .is_err()
        );
        assert!(!scene.world.zone_loading());
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
            world_relay: None,
            world_offline: false,
            door_preferences: None,
            zone_cache_directory: None,
        })
        .unwrap()
    }

    fn door_scene(id: DoorId) -> Scene {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene
            .world
            .set_spawn(
                id.position() + verse::world::SPAWN.with_x(0.0).with_z(-3.0),
                0.0,
            )
            .unwrap();
        scene.update(1.0).unwrap();
        scene.update(1.01).unwrap();
        assert_eq!(scene.door_snapshot().door, Some(id));
        assert!(scene.door_snapshot().visible);
        scene
    }

    fn tap_door_plane(scene: &mut Scene, id: DoorId) {
        let projected = scene.world.door(id, scene.aspect());
        let size = scene.lifecycle.viewport().logical_size();
        let [x, y] = [projected.screen_x * size[0], projected.screen_y * size[1]];
        assert!(scene.door_hit(id, x, y));
        scene.pointer(41, PointerPhase::Down, x, y).unwrap();
        scene.pointer(41, PointerPhase::Up, x, y).unwrap();
    }

    #[test]
    fn door_taps_select_then_walk_and_background_preserves_only_choices() {
        let mut scene = door_scene(DoorId::Spark);
        let before = scene.world.player.pos;
        tap_door_plane(&mut scene, DoorId::Spark);
        assert_eq!(
            scene.world.doors.state(DoorId::Spark).last,
            Some(DemoItem::Prism)
        );
        assert_eq!(
            scene.world.doors.state(DoorId::Spark).phase,
            verse::doors::DoorPhase::Reacting
        );
        assert!(!scene.world.navigation().is_active());
        for i in 1..=60 {
            scene.update(1.01 + f64::from(i) / 60.0).unwrap();
        }
        tap_door_plane(&mut scene, DoorId::Spark);
        assert!(scene.world.navigation().is_active());
        for i in 1..=30 {
            scene.update(2.01 + f64::from(i) / 60.0).unwrap();
        }
        assert!(scene.world.player.pos.distance(before) > 0.2);
        let saved = scene.packet().door_preferences;
        scene.activate(false).unwrap();
        assert!(!scene.world.navigation().is_active());
        assert_eq!(scene.packet().door_preferences, saved);
        assert!(scene.session.is_none());
        assert!(!scene.packet().gym_active);
        let mut restored = super::Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: false,
            world_relay: None,
            world_offline: false,
            door_preferences: Some(saved.clone()),
            zone_cache_directory: None,
        })
        .unwrap();
        restored.activate(true).unwrap();
        assert_eq!(restored.packet().door_preferences, saved);
        assert_eq!(
            restored.world.doors.state(DoorId::Spark).phase,
            verse::doors::DoorPhase::Idle
        );
        assert!(!restored.world.navigation().is_active());
    }

    #[test]
    fn door_item_strip_captures_input_and_refusal_keeps_memory() {
        let mut scene = door_scene(DoorId::Spark);
        tap_door_plane(&mut scene, DoorId::Spark);
        let position = scene.world.player.pos;
        let yaw = scene.world.player.yaw;
        let hud = scene.door_snapshot();
        let ring = hud
            .buttons
            .iter()
            .find(|button| button.id == "ring")
            .unwrap()
            .frame;
        let [x, y] = [ring[0] + ring[2] / 2.0, ring[1] + ring[3] / 2.0];
        scene.pointer(42, PointerPhase::Down, x, y).unwrap();
        assert!(scene.door_hud.captured(42));
        assert!(scene.touches.is_empty());
        scene.pointer(42, PointerPhase::Up, x, y).unwrap();
        assert_eq!(scene.world.doors.held(), DemoItem::Ring);
        tap_door_plane(&mut scene, DoorId::Spark);
        assert!(scene.door_snapshot().caption.contains("does not fit"));
        assert_eq!(
            scene.world.doors.state(DoorId::Spark).last,
            Some(DemoItem::Prism)
        );
        assert!(!scene.world.navigation().is_active());
        assert!(!scene.jump);
        assert_eq!(scene.world.player.pos, position);
        assert_eq!(scene.world.player.yaw, yaw);
        scene
            .action(Request::DoorReset {
                door: DoorId::Spark,
            })
            .unwrap();
        assert_eq!(scene.world.doors.state(DoorId::Spark).last, None);
        scene.action(Request::MapToggle).unwrap();
        assert!(!scene.door_snapshot().visible);
        assert!(
            scene
                .action(Request::DoorTap {
                    door: DoorId::Spark
                })
                .is_err()
        );
    }

    #[test]
    fn visible_door_edge_tap_does_not_require_an_unobstructed_accessibility_anchor() {
        let id = DoorId::Spark;
        let mut scene = door_scene(id);
        let view = scene.world.view(scene.aspect());
        let mut anchor = id.plane();
        anchor.y += 0.8;
        let obstruction = view.eye.lerp(anchor, 0.8);
        let right = verse::world::SPAWN.with_x(0.18).with_y(0.0).with_z(0.0);
        let up = verse::world::SPAWN.with_x(0.0).with_y(0.18).with_z(0.0);
        scene.presented_entities.quad([
            obstruction - right - up,
            obstruction + right - up,
            obstruction + right + up,
            obstruction - right + up,
        ]);
        assert!(scene.action(Request::DoorTap { door: id }).is_err());
        let mut side = anchor;
        side.x += 1.0;
        let clip = view.view_proj * side.extend(1.0);
        let size = scene.lifecycle.viewport().logical_size();
        let point = [
            (clip.x / clip.w * 0.5 + 0.5) * size[0],
            (0.5 - clip.y / clip.w * 0.5) * size[1],
        ];
        assert!(scene.door_hit(id, point[0], point[1]));
        scene
            .pointer_at(7, PointerPhase::Down, point[0], point[1], 1.02)
            .unwrap();
        scene
            .pointer_at(7, PointerPhase::Up, point[0], point[1], 1.08)
            .unwrap();
        assert_eq!(
            scene.world.doors.state(id).phase,
            verse::doors::DoorPhase::Reacting
        );
        assert_eq!(scene.world.doors.state(id).last, Some(DemoItem::Prism));
    }

    #[test]
    fn door_taps_reject_drag_cancel_pinch_and_hidden_panels() {
        for case in 0..4 {
            let mut scene = door_scene(DoorId::Halo);
            let projection = scene.world.door(DoorId::Halo, scene.aspect());
            let size = scene.lifecycle.viewport().logical_size();
            let [x, y] = [projection.screen_x * size[0], projection.screen_y * size[1]];
            scene.pointer_at(1, PointerPhase::Down, x, y, 1.02).unwrap();
            match case {
                0 => {
                    scene
                        .pointer_at(1, PointerPhase::Move, x + 30.0, y, 1.03)
                        .unwrap();
                }
                1 => {
                    scene
                        .pointer_at(1, PointerPhase::Cancel, x, y, 1.03)
                        .unwrap();
                }
                2 => {
                    scene.action(Request::PinchZoom { scale: 1.1 }).unwrap();
                }
                _ => {
                    scene.computer_open = true;
                }
            }
            scene.pointer_at(1, PointerPhase::Up, x, y, 1.06).unwrap();
            assert_eq!(
                scene.world.doors.state(DoorId::Halo).last,
                None,
                "Invalid gesture {case}"
            );
            assert!(!scene.world.navigation().is_active());
        }
    }

    fn companion_scene() -> (Scene, [f32; 2]) {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene.update(1.01).unwrap();
        let companion = scene.world.companion(scene.aspect());
        let size = scene.lifecycle.viewport().logical_size();
        let point = [companion.screen_x * size[0], companion.screen_y * size[1]];
        assert!(scene.companion_hit(point[0], point[1]));
        (scene, point)
    }

    #[test]
    fn companion_tap_reacts_once_and_does_not_move_or_jump() {
        for mode in [CameraMode::Touch, CameraMode::Motion] {
            let (mut scene, [x, y]) = companion_scene();
            scene.action(Request::CameraMode { mode }).unwrap();
            let position = scene.world.player.pos;
            scene.pointer_at(1, PointerPhase::Down, x, y, 1.02).unwrap();
            scene.pointer_at(1, PointerPhase::Up, x, y, 1.06).unwrap();
            assert_eq!(scene.world.agent.pet_count(), 1);
            assert_eq!(scene.world.agent.emote(), Some(verse::agent::Emote::Wiggle));
            scene.action(Request::PetCompanion).unwrap();
            assert_eq!(
                scene.world.agent.pet_count(),
                1,
                "Cooldown never queues another reaction"
            );
            assert!(!scene.jump);
            assert!(!scene.computer_open);
            for step in 1..=120 {
                scene.update(1.01 + f64::from(step) / 60.0).unwrap();
            }
            assert_eq!(scene.world.player.pos, position);
            assert_ne!(scene.world.agent.emote(), Some(verse::agent::Emote::Wiggle));
            scene.action(Request::PetCompanion).unwrap();
            assert_eq!(scene.packet().companion.pet_count, 2);
        }
    }

    #[test]
    fn companion_rejects_drag_cancel_pinch_hidden_and_inactive_taps() {
        for case in 0..5 {
            let (mut scene, [x, y]) = companion_scene();
            scene.pointer_at(1, PointerPhase::Down, x, y, 1.02).unwrap();
            match case {
                0 => {
                    scene
                        .pointer_at(1, PointerPhase::Move, x + 30.0, y, 1.03)
                        .unwrap();
                }
                1 => {
                    scene
                        .pointer_at(1, PointerPhase::Cancel, x, y, 1.03)
                        .unwrap();
                }
                2 => {
                    scene.action(Request::PinchZoom { scale: 1.1 }).unwrap();
                }
                3 => {
                    scene.computer_open = true;
                }
                _ => {
                    scene.activate(false).unwrap();
                }
            }
            scene.pointer_at(1, PointerPhase::Up, x, y, 1.06).unwrap();
            assert_eq!(scene.world.agent.pet_count(), 0, "Invalid gesture {case}");
            assert!(!scene.jump);
        }
    }

    #[test]
    fn map_taps_walk_and_lifecycle_cancels_without_world_input_leaking() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene
            .action(Request::HudInsets {
                top: 50.0,
                right: 0.0,
                bottom: 24.0,
                left: 0.0,
            })
            .unwrap();
        let compact = scene.map_snapshot();
        let tap = [compact.frame[0] + 10.0, compact.frame[1] + 10.0];
        scene
            .pointer(77, PointerPhase::Down, tap[0], tap[1])
            .unwrap();
        assert_eq!(scene.map_snapshot().captured_pointers, vec![77]);
        assert!(scene.touches.is_empty());
        scene.pointer(77, PointerPhase::Up, tap[0], tap[1]).unwrap();
        assert!(scene.map_snapshot().expanded);
        scene
            .action(Request::MapWalk { x: -5.0, z: -10.0 })
            .unwrap();
        assert!(scene.world.navigation().is_active());
        assert!(!scene.map_snapshot().expanded);
        scene.update(0.0).unwrap();
        for frame in 1..120 {
            scene.update(f64::from(frame) / 60.0).unwrap();
        }
        assert!((scene.world.player.pos.x + 5.0).abs() < 0.15);
        assert_eq!(
            scene.world.navigation().status(),
            verse::nav::NavigationStatus::Arrived
        );
        scene
            .action(Request::MapWalk { x: -10.0, z: -10.0 })
            .unwrap();
        scene.activate(false).unwrap();
        assert!(!scene.world.navigation().is_active());
        assert!(!scene.map_snapshot().visible);
        assert!(scene.action(Request::MapWalk { x: 0.0, z: -10.0 }).is_err());
    }

    #[test]
    fn held_manual_movement_keeps_priority_over_a_new_map_route() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.pointer(1, PointerPhase::Down, 50.0, 350.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 50.0, 300.0).unwrap();
        scene
            .action(Request::MapWalk { x: -5.0, z: -10.0 })
            .unwrap();
        assert!(scene.touches.contains_key(&1));
        scene.update(0.0).unwrap();
        scene.update(0.05).unwrap();
        assert_eq!(
            scene.world.navigation().status(),
            verse::nav::NavigationStatus::Cancelled
        );
    }

    #[test]
    fn blocked_map_destination_is_visible_and_every_landmark_is_reachable() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.action(Request::MapWalk { x: 0.0, z: -5.0 }).unwrap();
        assert_eq!(scene.map_snapshot().state, "That destination is blocked");
        assert!(scene.map_snapshot().expanded);
        assert!(!scene.world.navigation().is_active());
        for landmark in verse::minimap::LANDMARKS {
            scene
                .world
                .navigate_to([landmark.x, landmark.z])
                .unwrap_or_else(|e| panic!("{}: {e}", landmark.label));
        }
        let ui = scene.map_ui();
        assert!(!ui.vertices.is_empty());
        assert!(
            ui.vertices
                .iter()
                .all(|v| v.pos.iter().all(|x| x.is_finite()))
        );
    }

    #[test]
    fn physical_gym_board_taps_open_but_drags_and_cancelled_contacts_do_not() {
        for gesture in 0..4 {
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
            let gym = scene.gym();
            let size = scene.lifecycle.viewport().logical_size();
            let [x, y] = [gym.screen_x * size[0], gym.screen_y * size[1]];
            assert!(scene.gym_hit(x, y));
            assert_eq!(scene.world_target(x, y), Some(WorldTarget::Gym));
            scene.pointer_at(1, PointerPhase::Down, x, y, 1.0).unwrap();
            match gesture {
                1 => scene
                    .pointer_at(1, PointerPhase::Move, x + 50.0, y, 1.1)
                    .unwrap(),
                2 => scene
                    .pointer_at(1, PointerPhase::Cancel, x, y, 1.1)
                    .unwrap(),
                3 => scene.activate(false).unwrap(),
                _ => {}
            }
            scene.pointer_at(1, PointerPhase::Up, x, y, 1.2).unwrap();
            assert_eq!(scene.gym_open, gesture == 0);
            assert!(scene.touches.is_empty());
            assert!(!scene.jump);
        }
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
    fn world_selection_survives_panels_suspension_and_remount_without_preview_networking() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.world.set_spawn([1.0, 0.0, -2.0].into(), 0.7).unwrap();
        scene.open_computer();
        let position = scene.world.player.pos;
        scene
            .connect("  wss://relay.example.test/world  ".into())
            .unwrap();
        assert_eq!(
            scene.packet().connection.relay.as_deref(),
            Some("wss://relay.example.test/world")
        );
        assert_eq!(scene.packet().connection.state, "preview");
        assert!(scene.computer_open);
        assert_eq!(scene.world.player.pos, position);
        assert!(!scene.spawn_pending);
        assert!(!scene.restore_spawn);
        assert!(scene.session.is_none());
        scene.computer_open = false;
        scene.open_computer();
        assert!(scene.packet().connection.relay.is_some());
        scene.activate(false).unwrap();
        assert_eq!(scene.packet().connection.state, "paused");
        scene.activate(true).unwrap();
        assert_eq!(scene.world.player.pos, position);
        assert!(scene.session.is_none());
        let config: Config = serde_json::from_value(serde_json::json!({
            "secret_hex": "11".repeat(32), "width": 800, "height": 1200, "scale": 2,
            "synthetic": true, "world_relay": scene.packet().connection.relay
        }))
        .unwrap();
        let mut restored = Scene::new(config).unwrap();
        restored.activate(true).unwrap();
        assert_eq!(restored.packet().connection.state, "preview");
        assert_eq!(
            restored.packet().connection.relay,
            scene.packet().connection.relay
        );
        assert!(restored.session.is_none());
        restored.disconnect();
        restored.activate(false).unwrap();
        restored.activate(true).unwrap();
        assert_eq!(restored.packet().connection.state, "offline");
        assert!(restored.packet().connection.relay.is_none());
        assert!(restored.session.is_none());
    }

    #[test]
    fn new_installs_default_to_public_plaza_without_opening_a_socket_before_activation() {
        let base = serde_json::json!({
            "secret_hex": "11".repeat(32), "width": 800, "height": 1200, "scale": 2
        });
        for (overrides, expected) in [
            (serde_json::json!({}), Some(verse::session::PUBLIC_RELAY)),
            (serde_json::json!({"synthetic": true}), None),
            (serde_json::json!({"world_offline": true}), None),
            (
                serde_json::json!({"world_relay": "wss://relay.example.test"}),
                Some("wss://relay.example.test"),
            ),
            (
                serde_json::json!({"world_offline": true, "world_relay": "wss://relay.example.test"}),
                None,
            ),
        ] {
            let mut value = base.clone();
            value
                .as_object_mut()
                .unwrap()
                .extend(overrides.as_object().unwrap().clone());
            let scene = Scene::new(serde_json::from_value(value).unwrap()).unwrap();
            assert_eq!(scene.relay.as_deref(), expected);
            assert_eq!(scene.restore_spawn, expected.is_some());
            assert!(scene.session.is_none());
            assert!(!scene.lifecycle.active());
            assert!(!scene.gym_board.view().active);
        }
    }

    #[test]
    fn remembered_world_relay_is_validated_before_restore() {
        for relay in [
            "ws://remote.example.test",
            "wss://example.test/?token=secret",
            "wss://user:pass@example.test",
        ] {
            let config: Config = serde_json::from_value(serde_json::json!({
                "secret_hex": "11".repeat(32), "width": 800, "height": 1200, "scale": 2,
                "world_relay": relay
            }))
            .unwrap();
            let restored = Scene::new(config).unwrap();
            assert!(restored.relay.is_none());
            assert!(restored.error.is_some());
            assert!(restored.session.is_none());
        }
    }

    #[test]
    fn world_connection_labels_follow_the_observed_transport_status() {
        use verse::session::Status;
        for (status, expected) in [
            (None, "connecting"),
            (Some(Status::Connecting), "connecting"),
            (Some(Status::Online), "connected"),
            (Some(Status::Offline), "retrying"),
        ] {
            let value = connection(Some("wss://example.test"), true, false, status, None);
            assert_eq!(value.state, expected);
        }
        let failed = connection(
            Some("wss://example.test"),
            true,
            false,
            Some(Status::Offline),
            Some("Relay refused world updates."),
        );
        assert_eq!(failed.error, Some("Relay refused world updates."));
        assert_eq!(connection(None, true, false, None, None).state, "offline");
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
    fn dragging_from_the_monitor_releases_tap_capture_to_world_controls() {
        for right in [false, true] {
            let (mut scene, [center_x, y]) = computer_scene();
            let x = center_x + if right { 4.0 } else { -4.0 };
            assert!(scene.computer_hit(x, y));
            let position = scene.world.player.pos;
            let yaw = scene.world.player.yaw;
            scene.pointer_at(1, PointerPhase::Down, x, y, 1.02).unwrap();
            scene.update(1.04).unwrap();
            assert_eq!(scene.world.player.pos, position);
            assert_eq!(scene.world.player.yaw, yaw);
            let end = if right { [x + 40.0, y] } else { [x, y - 40.0] };
            scene
                .pointer_at(1, PointerPhase::Move, end[0], end[1], 1.08)
                .unwrap();
            scene.update(1.08).unwrap();
            if right {
                assert_ne!(
                    scene.world.player.yaw, yaw,
                    "Monitor-started drag must look"
                );
                assert_eq!(scene.world.player.pos, position);
            } else {
                assert_ne!(
                    scene.world.player.pos, position,
                    "Monitor-started drag must move"
                );
            }
            scene
                .pointer_at(1, PointerPhase::Up, end[0], end[1], 1.1)
                .unwrap();
            assert!(!scene.computer_open);
            assert!(!scene.jump);
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
                    let touch = scene.touches.get_mut(&1).unwrap();
                    touch.started -= 1.0;
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

    fn world_tap(scene: &mut Scene, x: f32, timestamp: f64) {
        scene
            .pointer_at(1, PointerPhase::Down, x, 550.0, timestamp)
            .unwrap();
        scene
            .pointer_at(1, PointerPhase::Up, x, 550.0, timestamp + 0.04)
            .unwrap();
    }

    #[test]
    fn nearby_double_taps_jump_once_in_each_camera_mode_and_touch_region() {
        for mode in [CameraMode::Touch, CameraMode::Motion] {
            for x in [100.0, 300.0] {
                let mut scene = scene();
                scene.activate(true).unwrap();
                scene.action(Request::CameraMode { mode }).unwrap();
                scene.update(1.0).unwrap();
                world_tap(&mut scene, x, 1.0);
                assert!(!scene.jump, "a single tap cannot jump");
                world_tap(&mut scene, x + 5.0, 1.15);
                assert!(scene.jump);
                let initial_height = scene.world.player.pos.y;
                scene.update(1.04).unwrap();
                assert!(scene.world.player.pos.y > initial_height);
                assert!(!scene.jump, "the shared controller consumes the jump once");
                world_tap(&mut scene, x, 1.3);
                assert!(!scene.jump, "a third tap starts another pair");
            }
        }
    }

    #[test]
    fn held_movement_and_right_look_remain_independent() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene
            .pointer_at(10, PointerPhase::Down, 100.0, 500.0, 1.0)
            .unwrap();
        scene
            .pointer_at(10, PointerPhase::Move, 100.0, 450.0, 1.05)
            .unwrap();
        let before = scene.world.player.pos;
        let yaw = scene.world.player.yaw;
        scene
            .pointer_at(20, PointerPhase::Down, 300.0, 500.0, 1.2)
            .unwrap();
        scene
            .pointer_at(20, PointerPhase::Move, 350.0, 480.0, 1.25)
            .unwrap();
        assert!(scene.input().forward);
        assert_ne!(scene.world.player.yaw, yaw);
        scene.update(1.04).unwrap();
        assert_ne!(scene.world.player.pos, before);
        scene
            .pointer_at(20, PointerPhase::Up, 350.0, 480.0, 1.3)
            .unwrap();
        assert!(
            scene.input().forward,
            "Releasing look must keep movement held"
        );
        assert!(!scene.jump);
    }

    #[test]
    fn right_double_tap_jumps_while_left_movement_is_held() {
        for mode in [CameraMode::Touch, CameraMode::Motion] {
            let mut scene = scene();
            scene.activate(true).unwrap();
            scene.action(Request::CameraMode { mode }).unwrap();
            scene.update(1.0).unwrap();
            scene
                .pointer_at(10, PointerPhase::Down, 100.0, 500.0, 1.0)
                .unwrap();
            if mode == CameraMode::Touch {
                scene
                    .pointer_at(10, PointerPhase::Move, 100.0, 450.0, 1.05)
                    .unwrap();
            }
            world_tap(&mut scene, 300.0, 1.3);
            // Ordinary joystick updates between the taps cannot erase them.
            scene
                .pointer_at(10, PointerPhase::Move, 100.0, 450.0, 1.4)
                .unwrap();
            world_tap(&mut scene, 300.0, 1.45);
            assert!(
                scene.jump,
                "Right-side double tap must work beside {mode:?} movement"
            );
            let height = scene.world.player.pos.y;
            scene.update(1.04).unwrap();
            assert!(scene.input().forward);
            assert!(scene.world.player.pos.y > height);
            assert!(scene.touches.contains_key(&10));
        }
    }

    #[test]
    fn explicit_recenter_restores_default_view_and_requires_fresh_motion() {
        let mut scene = motion_scene();
        scene.world.player.yaw = 0.7;
        scene.world.camera.yaw_offset = 0.4;
        scene.world.camera.pitch = -0.9;
        scene.world.camera.distance = 5.0;
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.01);
        sample(&mut scene, attitude(1.0, -1.0, 0.0), 1.02);
        let position = scene.world.player.pos;
        let heading = scene.world.player.yaw;
        scene.world.camera.yaw_offset = 0.4;
        scene.action(Request::RecenterCamera).unwrap();
        close(scene.world.camera.yaw_offset, 0.0);
        close(scene.world.camera.pitch, 0.28);
        close(scene.world.camera.distance, 5.0);
        close(scene.world.player.yaw, heading);
        assert_eq!(scene.world.player.pos, position);
        sample(&mut scene, attitude(0.5, -0.5, 0.0), 1.01);
        assert!(scene.motion.target.is_none());
        sample(&mut scene, attitude(-1.0, 0.7, 0.0), 1.03);
        scene.advance_motion(0.05);
        close(scene.world.player.yaw, heading);
        close(scene.world.camera.pitch, 0.28);
        assert!(serde_json::from_str::<Request>(r#"{"action":"recenter_camera"}"#).is_ok());
    }

    #[test]
    fn double_tap_requires_a_close_pair_without_drag_hold_cancel_or_second_finger() {
        for case in 0..8 {
            let mut scene = scene();
            scene.activate(true).unwrap();
            scene.update(1.0).unwrap();
            world_tap(&mut scene, 300.0, 1.0);
            scene
                .pointer_at(1, PointerPhase::Down, 300.0, 550.0, 1.1)
                .unwrap();
            let mut finished = 1.15;
            match case {
                0 => finished = 1.5,
                1 => {
                    scene
                        .pointer_at(1, PointerPhase::Move, 325.0, 550.0, 1.11)
                        .unwrap();
                    scene
                        .pointer_at(1, PointerPhase::Move, 300.0, 550.0, 1.12)
                        .unwrap();
                }
                2 => {
                    scene
                        .pointer_at(1, PointerPhase::Cancel, f32::NAN, 550.0, 1.11)
                        .unwrap();
                }
                3 => {
                    scene
                        .pointer_at(2, PointerPhase::Down, 100.0, 550.0, 1.11)
                        .unwrap();
                    scene
                        .pointer_at(2, PointerPhase::Up, 100.0, 550.0, 1.12)
                        .unwrap();
                }
                4 => {
                    scene
                        .pointer_at(1, PointerPhase::Up, f32::NAN, 550.0, 1.11)
                        .unwrap();
                }
                5 => {
                    assert!(
                        scene
                            .pointer_at(1, PointerPhase::Move, f32::NAN, 550.0, 1.11)
                            .is_err()
                    );
                }
                6 => {
                    // A second finger on the same half is not retained as a
                    // controller touch, but still cancels the tap candidate.
                    scene
                        .pointer_at(2, PointerPhase::Down, 320.0, 550.0, 1.11)
                        .unwrap();
                    scene
                        .pointer_at(2, PointerPhase::Up, 320.0, 550.0, 1.12)
                        .unwrap();
                }
                7 => {
                    scene
                        .pointer_at(1, PointerPhase::Cancel, 300.0, 550.0, 1.11)
                        .unwrap();
                    world_tap(&mut scene, 350.0, 1.12);
                }
                _ => unreachable!(),
            }
            scene
                .pointer_at(1, PointerPhase::Up, 300.0, 550.0, finished)
                .unwrap();
            assert!(!scene.jump, "invalid gesture {case}");
            if case != 7 {
                world_tap(&mut scene, 300.0, finished + 0.01);
                assert!(
                    !scene.jump,
                    "invalid gesture {case} must discard the first tap"
                );
            }
        }
        for (second_x, second_time) in [(300.0, 1.5), (350.0, 1.15)] {
            let mut scene = scene();
            scene.activate(true).unwrap();
            world_tap(&mut scene, 300.0, 1.0);
            world_tap(&mut scene, second_x, second_time);
            assert!(!scene.jump, "a distant or expired pair cannot jump");
        }
    }

    #[test]
    fn render_stalls_do_not_turn_held_touches_into_taps() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        world_tap(&mut scene, 300.0, 1.0);
        scene
            .pointer_at(1, PointerPhase::Down, 300.0, 550.0, 1.1)
            .unwrap();
        scene
            .pointer_at(1, PointerPhase::Up, 300.0, 550.0, 2.0)
            .unwrap();
        assert_eq!(scene.frame_timestamp, Some(1.0));
        assert!(!scene.jump);
        assert!(scene.last_world_tap.is_none());
    }

    #[test]
    fn lifecycle_camera_and_panel_changes_discard_double_taps() {
        for case in 0..7 {
            let (mut scene, [monitor_x, monitor_y]) = computer_scene();
            world_tap(&mut scene, 300.0, 1.0);
            match case {
                0 => {
                    scene.activate(false).unwrap();
                    scene.activate(true).unwrap();
                }
                1 => scene.action(Request::ResetMotion).unwrap(),
                2 => scene
                    .action(Request::CameraMode {
                        mode: CameraMode::Motion,
                    })
                    .unwrap(),
                3 => scene
                    .resize(Viewport::new(900, 1200, 2.0).unwrap())
                    .unwrap(),
                4 => {
                    scene
                        .pointer_at(1, PointerPhase::Down, monitor_x, monitor_y, 1.1)
                        .unwrap();
                    scene
                        .pointer_at(1, PointerPhase::Up, monitor_x, monitor_y, 1.14)
                        .unwrap();
                    assert!(scene.computer_open);
                    assert!(!scene.jump, "the monitor keeps its immediate single tap");
                    world_tap(&mut scene, 300.0, 1.15);
                    assert!(!scene.jump, "an open panel cannot jump");
                    scene.action(Request::CloseComputer).unwrap();
                }
                5 => {
                    scene.gym_open = true;
                    world_tap(&mut scene, 300.0, 1.15);
                    assert!(!scene.jump);
                    scene.action(Request::CloseGym).unwrap();
                }
                6 => {
                    scene.activate(false).unwrap();
                    scene.lifecycle.destroy();
                }
                _ => unreachable!(),
            }
            world_tap(&mut scene, 300.0, 1.2);
            assert!(!scene.jump, "transition {case} must discard the first tap");
        }
    }

    #[test]
    fn pinch_scales_camera_distance_and_cancels_pending_input() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        world_tap(&mut scene, 300.0, 1.0);
        scene
            .pointer_at(1, PointerPhase::Down, 100.0, 550.0, 1.1)
            .unwrap();
        scene.action(Request::Jump).unwrap();
        let initial = scene.world.camera.distance;
        scene.action(Request::PinchZoom { scale: 2.0 }).unwrap();
        assert_eq!(scene.world.camera.distance, initial / 2.0);
        assert_eq!(scene.packet().camera_distance, initial / 2.0);
        assert!(scene.touches.is_empty());
        assert!(scene.last_world_tap.is_none());
        assert!(!scene.jump);
        scene.action(Request::PinchZoom { scale: 0.5 }).unwrap();
        assert_eq!(scene.world.camera.distance, initial);
        scene.action(Request::PinchZoom { scale: 10.0 }).unwrap();
        assert_eq!(scene.world.camera.distance, verse::camera::MIN_DISTANCE);
        scene.action(Request::PinchZoom { scale: 0.1 }).unwrap();
        scene.action(Request::PinchZoom { scale: 0.1 }).unwrap();
        assert_eq!(scene.world.camera.distance, verse::camera::MAX_DISTANCE);
        for scale in [0.0, -1.0, 0.01, 100.0, f32::NAN, f32::INFINITY] {
            assert!(scene.action(Request::PinchZoom { scale }).is_err());
            assert_eq!(scene.world.camera.distance, verse::camera::MAX_DISTANCE);
        }
        for case in 0..4 {
            scene.computer_open = case == 0;
            scene.gym_open = case == 1;
            scene.spawn_pending = case == 2;
            if case == 3 {
                scene.activate(false).unwrap();
            }
            scene.action(Request::PinchZoom { scale: 2.0 }).unwrap();
            assert_eq!(scene.world.camera.distance, verse::camera::MAX_DISTANCE);
        }
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
        scene.connect("wss://example.test".into()).unwrap();
        assert!(scene.session.is_none());
        assert_eq!(scene.packet().connection.state, "preview");
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

    /// Start with a portrait phone: its right edge points along reference +X,
    /// its top points up (+Z), and its back points forward (+Y). A body turn
    /// acts around world +Z; pitch acts around the phone's right edge; screen
    /// roll acts last around its local +Z. These are active physical rotations.
    fn attitude(yaw: f32, down_pitch: f32, screen_roll: f32) -> [f32; 4] {
        let x = (std::f32::consts::FRAC_PI_2 - down_pitch) * 0.5;
        let z = yaw * 0.5;
        let r = screen_roll * 0.5;
        product(
            [0.0, 0.0, z.sin(), z.cos()],
            product([x.sin(), 0.0, 0.0, x.cos()], [0.0, 0.0, r.sin(), r.cos()]),
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

    fn sample(scene: &mut Scene, quaternion: [f32; 4], timestamp: f64) {
        scene.device_motion(quaternion, timestamp, timestamp);
    }

    #[test]
    fn native_portrait_fixtures_turn_the_body_left_and_right_and_look_up() {
        let s = std::f32::consts::FRAC_1_SQRT_2;
        // Core Motion's upright quaternion has +X, not -X. Its public rotation
        // matrix maps reference gravity to device -Y; the quaternion is the
        // inverse of that matrix. The two quarter turns are around world up.
        for (q, heading, down_pitch) in [
            ([s, 0.0, 0.0, s], 0.0, 0.0),
            ([0.5, 0.5, 0.5, 0.5], std::f32::consts::FRAC_PI_2, 0.0),
            ([0.5, -0.5, -0.5, 0.5], -std::f32::consts::FRAC_PI_2, 0.0),
            (
                [0.923_879_5, 0.0, 0.0, 0.382_683_4],
                0.0,
                -std::f32::consts::FRAC_PI_4,
            ),
            (
                [0.382_683_4, 0.0, 0.0, 0.923_879_5],
                0.0,
                std::f32::consts::FRAC_PI_4,
            ),
        ] {
            for q in [q, q.map(|v| -v)] {
                let orientation = motion_angles(q).unwrap();
                close(orientation.angles[0], heading);
                close(orientation.angles[1], down_pitch);
            }
        }
    }

    #[test]
    fn motion_mapping_keeps_heading_and_pitch_independent_of_screen_roll() {
        for (yaw, pitch) in [(0.0, 0.0), (0.7, 0.3), (-0.9, -0.4), (2.7, -1.3)] {
            for roll in [0.0, 0.8, -2.4] {
                let q = attitude(yaw, pitch, roll);
                for q in [q, q.map(|v| -v), q.map(|v| v * 1.01)] {
                    let angles = motion_angles(q).unwrap().angles;
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
    fn enabling_motion_preserves_view_and_frames_smooth_the_shortest_turn() {
        let mut scene = motion_scene();
        scene.world.player.yaw = 2.9;
        scene.world.camera.yaw_offset = 0.2;
        let pitch = scene.world.camera.pitch;
        sample(&mut scene, attitude(3.0, 0.1, 0.0), 1.01);
        close(scene.world.player.yaw, 3.1);
        close(scene.world.camera.yaw_offset, 0.0);
        close(scene.world.camera.pitch, pitch);
        sample(&mut scene, attitude(-3.0, 0.3, 0.8), 1.02);
        close(scene.world.player.yaw, 3.1);
        close(scene.world.camera.pitch, pitch);
        scene.update(1.0 + 1.0 / 60.0).unwrap();
        let advanced = verse::controller::wrap(scene.world.player.yaw - 3.1);
        assert!(
            advanced > 0.0 && advanced < 0.15,
            "short left turn: {advanced}"
        );
        assert!(scene.world.camera.pitch > pitch && scene.world.camera.pitch < pitch + 0.2);
        let target = scene.motion.target.unwrap();
        sample(&mut scene, attitude(-3.0, 0.3, 0.8).map(|v| -v), 1.03);
        close(scene.motion.target.unwrap()[0], target[0]);
        close(scene.motion.target.unwrap()[1], target[1]);
        scene.advance_motion(1.0);
        close(
            scene.world.player.yaw,
            verse::controller::wrap(3.1 + std::f32::consts::TAU - 6.0),
        );
        close(scene.world.camera.pitch, pitch + 0.2);
        let packet = serde_json::to_value(scene.packet()).unwrap();
        assert_eq!(packet["camera_mode"], "motion");
        assert_eq!(packet["motion_needed"], true);
    }

    #[test]
    fn motion_response_is_independent_of_display_and_sensor_rates() {
        let mut results = Vec::new();
        for display_hz in [15, 30, 60, 120] {
            for sensor_hz in [30, 60, 120] {
                let mut scene = motion_scene();
                sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.0 + 0.0001);
                sample(&mut scene, attitude(0.6, -0.8, 0.0), 1.0 + 0.0002);
                let mut next_sample = 1;
                for frame in 1..=display_hz / 5 {
                    let timestamp = 1.0 + f64::from(frame) / f64::from(display_hz);
                    while 1.0 + f64::from(next_sample) / f64::from(sensor_hz) <= timestamp {
                        sample(
                            &mut scene,
                            attitude(0.6, -0.8, 0.0),
                            1.0 + f64::from(next_sample) / f64::from(sensor_hz),
                        );
                        next_sample += 1;
                    }
                    scene.update(timestamp).unwrap();
                }
                results.push([scene.world.player.yaw, scene.world.camera.pitch]);
            }
        }
        for result in &results {
            close(result[0], results[0][0]);
            close(result[1], results[0][1]);
        }
        assert!(results[0][0] > 0.57 && results[0][0] < 0.6);
        assert!(results[0][1] < -0.48);
    }

    #[test]
    fn motion_left_hold_follows_the_smoothed_view_and_right_drag_does_not_look() {
        let mut scene = motion_scene();
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.01);
        scene.pointer(1, PointerPhase::Down, 100.0, 300.0).unwrap();
        assert!(scene.input().forward);
        scene.pointer(2, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(2, PointerPhase::Move, 390.0, 390.0).unwrap();
        assert!(!scene.jump);
        close(
            scene.world.camera.pitch,
            verse::camera::FollowCamera::default().pitch,
        );
        close(scene.world.player.yaw, 0.0);
        let start = scene.world.player.pos;
        sample(
            &mut scene,
            attitude(std::f32::consts::FRAC_PI_2, 0.0, 0.0),
            1.02,
        );
        for frame in 1..=10 {
            scene.update(1.0 + f64::from(frame) / 30.0).unwrap();
        }
        assert!(scene.world.player.pos.x > start.x + 1.0);
        assert!(scene.world.player.pos.z > start.z);
        scene.pointer(1, PointerPhase::Move, 130.0, 350.0).unwrap();
        let input = scene.input();
        assert!(input.backward && input.strafe_right && !input.forward);
        scene.pointer(1, PointerPhase::Up, 130.0, 350.0).unwrap();
        assert!(!scene.input().forward && !scene.input().backward);
        let yaw = scene.world.player.yaw;
        scene
            .action(Request::CameraMode {
                mode: CameraMode::Touch,
            })
            .unwrap();
        assert!(scene.motion.target.is_none());
        scene.pointer(3, PointerPhase::Down, 100.0, 300.0).unwrap();
        assert!(!scene.input().forward, "touch mode requires joystick drag");
        scene.pointer(3, PointerPhase::Move, 100.0, 270.0).unwrap();
        assert!(scene.input().forward);
        scene.pointer(4, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(4, PointerPhase::Move, 320.0, 300.0).unwrap();
        close(scene.world.player.yaw, yaw - 0.08);
    }

    #[test]
    fn fresh_motion_is_admitted_after_a_slow_frame_but_stale_or_future_motion_is_not() {
        let mut scene = motion_scene();
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.01);
        // The previous frame is 500 ms old. The current sensor sample is fresh
        // against its receipt time and must still turn the next frame.
        scene.device_motion(attitude(0.5, 0.0, 0.0), 1.49, 1.5);
        assert_eq!(scene.motion.last_sample, Some(1.49));
        scene.update(1.5).unwrap();
        assert!(scene.world.player.yaw > 0.49);
        let target = scene.motion.target;
        for (timestamp, received_at) in [
            (1.51, 1.9),
            (1.7, 1.6),
            (1.48, 1.5),
            (1.51, 1.49),
            (f64::NAN, 1.6),
            (1.6, f64::NAN),
            (1.6, f64::INFINITY),
            (-1.0, 1.6),
        ] {
            scene.device_motion(attitude(-1.0, 0.5, 0.0), timestamp, received_at);
        }
        for invalid in [[0.0; 4], [f32::INFINITY; 4], [f32::NAN; 4]] {
            scene.device_motion(invalid, 1.6, 1.6);
        }
        assert_eq!(scene.motion.last_sample, Some(1.49));
        assert_eq!(scene.motion.last_received, Some(1.5));
        assert_eq!(scene.motion.target, target);
    }

    #[test]
    fn motion_gaps_and_recenter_discard_pending_interpolation_without_jumping() {
        let mut scene = motion_scene();
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.01);
        sample(&mut scene, attitude(1.0, -1.0, 0.0), 1.02);
        scene.update(1.04).unwrap();
        let previous = [scene.world.player.yaw, scene.world.camera.pitch];
        scene.action(Request::ResetMotion).unwrap();
        sample(&mut scene, attitude(0.5, -0.5, 0.0), 1.03);
        assert!(
            scene.motion.target.is_none(),
            "ignore queued samples before recenter"
        );
        scene.update(1.08).unwrap();
        close(scene.world.player.yaw, previous[0]);
        close(scene.world.camera.pitch, previous[1]);
        sample(&mut scene, attitude(-1.5, 0.7, 0.0), 1.09);
        scene.update(1.12).unwrap();
        close(scene.world.player.yaw, previous[0]);
        close(scene.world.camera.pitch, previous[1]);
        sample(&mut scene, attitude(-1.0, 0.7, 0.0), 1.13);
        scene.update(2.5).unwrap();
        assert!(scene.motion.target.is_none());
        close(scene.world.player.yaw, previous[0]);
        sample(&mut scene, attitude(2.0, -0.5, 0.0), 2.51);
        scene.update(2.54).unwrap();
        close(scene.world.player.yaw, previous[0]);
        close(scene.world.camera.pitch, previous[1]);
        sample(&mut scene, attitude(2.5, -0.5, 0.0), 2.55);
        // A sensor gap also clears an unfinished target before any new frame.
        sample(&mut scene, attitude(-1.0, 0.5, 0.0), 4.0);
        scene.advance_motion(0.05);
        close(scene.world.player.yaw, previous[0]);
        close(scene.world.camera.pitch, previous[1]);
    }

    #[test]
    fn motion_pitch_is_bounded_and_pole_crossings_keep_yaw_stable() {
        let mut scene = motion_scene();
        sample(&mut scene, attitude(0.0, -1.0, 0.0), 1.01);
        sample(&mut scene, attitude(0.0, 1.4, 0.0), 1.02);
        close(scene.motion.target.unwrap()[1], verse::camera::MAX_PITCH);
        scene.action(Request::ResetMotion).unwrap();
        sample(&mut scene, attitude(0.0, 1.0, 0.0), 1.03);
        sample(&mut scene, attitude(0.0, -1.4, 0.0), 1.04);
        close(scene.motion.target.unwrap()[1], verse::camera::MIN_PITCH);
        scene.advance_motion(0.1);
        let yaw = scene.world.player.yaw;
        let previous_pitch = scene.world.camera.pitch;
        sample(&mut scene, attitude(0.8, -1.56, 0.0), 1.05);
        assert!(!scene.motion.baseline.as_ref().unwrap().heading_valid);
        scene.advance_motion(0.05);
        close(scene.world.player.yaw, yaw);
        assert!(
            scene.world.camera.pitch < previous_pitch,
            "pitch keeps approaching the sky"
        );
        // Cross the pole, pass through the hysteresis band, then recover the
        // heading on the far side without the geometrical half-turn.
        for (index, pitch) in [-1.58, -1.68, -1.8].into_iter().enumerate() {
            sample(
                &mut scene,
                attitude(-2.0, pitch, 0.0),
                1.06 + index as f64 * 0.01,
            );
            scene.advance_motion(0.03);
            close(scene.world.player.yaw, yaw);
        }
        assert!(scene.motion.baseline.as_ref().unwrap().heading_valid);
        sample(&mut scene, attitude(-1.8, -1.8, 0.0), 1.09);
        close(
            verse::controller::wrap(scene.motion.target.unwrap()[0] - yaw),
            0.2,
        );
    }

    #[test]
    fn motion_lifecycle_resets_pending_targets_and_ignores_inactive_samples() {
        let mut scene = motion_scene();
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 1.01);
        sample(&mut scene, attitude(0.4, 0.0, 0.0), 1.02);
        scene.update(1.04).unwrap();
        let yaw = scene.world.player.yaw;
        scene.activate(false).unwrap();
        assert!(!scene.packet().motion_needed);
        assert!(scene.motion.target.is_none());
        sample(&mut scene, attitude(1.4, 0.0, 0.0), 1.05);
        close(scene.world.player.yaw, yaw);
        scene.activate(true).unwrap();
        scene.update(2.0).unwrap();
        sample(&mut scene, attitude(1.4, 0.0, 0.0), 2.01);
        scene.update(2.02).unwrap();
        close(scene.world.player.yaw, yaw);
        for computer in [true, false] {
            scene.reset_motion();
            scene.computer_open = computer;
            scene.gym_open = !computer;
            assert!(!scene.packet().motion_needed);
            sample(&mut scene, attitude(-1.0, 0.0, 0.0), 2.03);
            close(scene.world.player.yaw, yaw);
            scene
                .action(if computer {
                    Request::CloseComputer
                } else {
                    Request::CloseGym
                })
                .unwrap();
            sample(
                &mut scene,
                attitude(-1.0, 0.0, 0.0),
                if computer { 2.04 } else { 2.05 },
            );
            scene.advance_motion(0.05);
            close(scene.world.player.yaw, yaw);
        }
        scene.spawn_pending = true;
        scene.reset_motion();
        assert!(!scene.packet().motion_needed);
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 2.06);
        scene.update(2.1).unwrap();
        assert!(scene.packet().motion_needed);
        sample(&mut scene, attitude(0.0, 0.0, 0.0), 2.11);
        close(scene.world.player.yaw, yaw);
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
                r#"{"action":"device_motion","quaternion":[0,0,0,1],"timestamp":1,"received_at":1}"#
            )
            .is_ok()
        );
        assert!(
            serde_json::from_str::<Request>(
                r#"{"action":"device_motion","quaternion":[0,0,0,1],"timestamp":1}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<Request>(
                r#"{"action":"device_motion","quaternion":[0,0,0,1],"timestamp":1,"received_at":1,"extra":true}"#
            )
            .is_err()
        );
    }
}
