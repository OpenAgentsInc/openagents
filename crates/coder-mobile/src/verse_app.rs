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
    /// The name shown over this player's head, from Account; the short key
    /// when absent.
    #[serde(default)]
    pub display_name: Option<String>,
    /// An explicit Leave choice overrides the public default and any stale URL.
    #[serde(default)]
    pub world_offline: bool,
    #[serde(default)]
    pub door_preferences: Option<String>,
    #[serde(default)]
    pub zone_cache_directory: Option<String>,
    /// Where the Grid's RESULTS board reads the Gym's published results: an
    /// `https://` base ending in `/`, or a local directory. The default is
    /// the public repository's publication.
    #[serde(default)]
    pub results_base: Option<String>,
    /// The app's cache directory for verified results, kept between visits.
    #[serde(default)]
    pub results_cache_directory: Option<String>,
    /// Draw the world computer's screen in the HUD. A host that keeps its
    /// native computer panel leaves this off.
    #[serde(default)]
    pub computer_hud: bool,
    /// The screen offers extended dynamic range and the layer is set up for
    /// it: request an extended-range surface.
    #[serde(default)]
    pub hdr: bool,
    /// Mount Verse's bare world: the plaza grid in the neutral palette with
    /// the same player controls, its shared ball and blocks, its portal to
    /// Lagrange 1, and its Gym. It has no map, doors, computer, or
    /// companion. Unless offline, it joins its own NIP-MV world
    /// ([`verse::session::BARE_WORLD`]) for avatar presence alone: no chat,
    /// gestures, agent, or profile. Its Gym takes the same `gym_code` and
    /// `synthetic_gym` preview as Coder's.
    #[serde(default)]
    pub bare: bool,
    /// The chamber the Grid's RITUAL arch opens: a `verse::ritual::Config`
    /// JSON file on this device. Without it the arch is closed.
    #[serde(default)]
    pub ritual: Option<String>,
    /// Levels come from the labeled tutorial fixture rather than the relay
    /// ([`verse::xp::fixture::tutorial_events`]), for simulator checks.
    #[serde(default)]
    pub xp_preview: bool,
    /// The player switched on **Compare notes** on the Grid's EVALS board:
    /// their agent may trade notes about published results with other
    /// trainers' agents in the Gym. Off by default.
    #[serde(default)]
    pub gym_notes: bool,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Request {
    Snapshot,
    ZoneCredits,
    Frame {
        timestamp: f64,
        /// The screen's current extended-range headroom over reference
        /// white; absent or 1.0 on a standard-range display.
        #[serde(default)]
        headroom: Option<f64>,
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
    /// Leave the shared chamber and return to the Grid at the RITUAL arch.
    LeaveChamber,
    /// Validated Rust-owned physical control bindings for the current visit.
    ChamberBindings {
        bindings: Vec<verse_world::controls::Binding>,
    },
    /// Cast the chamber hotbar's slot (0-based) at the selected target.
    ChamberCast {
        slot: usize,
    },
    /// Ask the chamber for a new character after death.
    ChamberRespawn,
    /// Load and enter Everglade from the Grid without walking to its arch,
    /// for scripted checks (`--verse-script everglade`).
    EnterEverglade,
    /// Stand at Everglade's station `station` (its map landmark ID, such as
    /// `podium`), for scripted checks (`--verse-script station=podium`).
    GoStation {
        station: String,
    },
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
    /// The reader worker's latest Computers and terminal views for the
    /// computer's HUD.
    ComputerFeed {
        feed: Box<crate::computer_hud::Feed>,
    },
    /// Show a page of the open computer. Chats is drawn natively.
    ComputerPage {
        page: crate::computer_hud::Page,
    },
    /// Activate a laid-out HUD control by key, as accessibility does.
    ComputerHudTap {
        key: String,
    },
    /// Scroll the HUD body, as accessibility does.
    ComputerHudScroll {
        delta: f32,
    },
    /// How far the software keyboard covers the surface's bottom, in
    /// points. The terminal page stays above it.
    ComputerKeyboard {
        bottom: f32,
    },
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
    /// Open the Grid's RESULTS board, as VoiceOver does: the same reach and
    /// line-of-sight checks as a tap on it.
    InteractResults,
    CloseResults,
    /// Read the results panel's current screen.
    ResultsView,
    /// A choice in the results panel.
    Results {
        command: verse::gym_results::Action,
    },
    /// Open the Grid's EVALS board, as VoiceOver does: the same reach and
    /// line-of-sight checks as a tap on it.
    InteractEvals,
    CloseEvals,
    /// Read the EVALS panel's current screen.
    EvalsView,
    /// A choice on the EVALS board.
    Evals {
        command: verse::gym_hall::Action,
    },
    /// Walk into the Grid's Gym, face the EVALS board, and open it: a
    /// chat card's **See the board**.
    GoEvals,
    /// Read the open Agent Studio panel's view. Everglade's Interact
    /// control (`Zone { intent: interact }`) opens the panel.
    StudioView,
    /// The host activated a control in the studio panel's view. The event
    /// carries identity only; the current view supplies the intent.
    StudioActivate {
        instance: String,
        revision: u64,
        node: String,
    },
    /// Close the Agent Studio panel, as the host's back gesture does.
    CloseStudio,
    /// Text the person typed into the open studio panel's field: an
    /// answer, a message, or a change request (`studio_panel::typed`).
    StudioText {
        text: String,
    },
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
    /// The renderer draws to an extended-range surface.
    pub hdr_output: bool,
    frames_presented: u64,
    position: [f32; 3],
    /// The bare world's ball, for the host's diagnostics.
    #[serde(skip_serializing_if = "Option::is_none")]
    ball: Option<BallPacket>,
    camera_mode: CameraMode,
    camera_yaw: f32,
    camera_pitch: f32,
    camera_distance: f32,
    /// Zoomed all the way in: the camera is at the player's head.
    camera_first_person: bool,
    /// The pointer holding the movement stick. The host keeps it out of
    /// pinch arbitration, so zooming never releases the stick.
    #[serde(skip_serializing_if = "Option::is_none")]
    stick_pointer: Option<u64>,
    /// The pointer holding the bare world's look stick, which the host keeps
    /// out of pinch arbitration in the same way.
    #[serde(skip_serializing_if = "Option::is_none")]
    look_stick_pointer: Option<u64>,
    motion_needed: bool,
    companion: Companion,
    computer: Computer,
    computer_open: bool,
    computer_page: crate::computer_hud::Page,
    computer_hud: crate::computer_hud::Snapshot,
    /// What the native host must do for the computer's HUD, once each.
    pub computer_commands: Vec<crate::computer_hud::Command>,
    gym: Gym,
    gym_open: bool,
    gym_revision: u64,
    gym_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gym_board: Option<verse::gym::BoardView>,
    /// The Grid's RESULTS board: where it is, and whether its panel is open.
    /// The panel's screen comes only in answer to the results requests.
    results: Gym,
    results_open: bool,
    results_revision: u64,
    results_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub results_view: Option<verse::gym_results::ResultsView>,
    /// The Grid's EVALS board: where it is, and whether its panel is open.
    /// The panel's screen comes only in answer to the EVALS requests.
    evals: Gym,
    evals_open: bool,
    evals_revision: u64,
    evals_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evals_view: Option<verse::gym_hall::View>,
    /// **Compare notes** is on, for the host to remember.
    gym_notes: bool,
    /// Everglade's Agent Studio panel is open over the world.
    studio_open: bool,
    /// The open studio panel's view revision, or zero. A host holding an
    /// older revision asks for the view again (`studio_view`).
    studio_revision: u64,
    /// The studio panel as a Rust Native view the host mounts. It comes
    /// only in answer to the studio requests and Everglade's Interact.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub studio_view: Option<View<crate::studio_panel::Intent>>,
    view: View<()>,
}

/// The bare world's ball: where it is, how fast it moves, and what its last
/// physics frame cost.
#[derive(Serialize)]
struct BallPacket {
    position: [f32; 3],
    speed: f32,
    asleep: bool,
    step_ms: f32,
    /// Bodies awake in the ball's world, which also holds the stack of
    /// cubes and the dominoes.
    awake: usize,
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
        hdr_output: false,
        frames_presented: frames,
        position,
        ball: None,
        camera_mode: CameraMode::Touch,
        camera_yaw: 0.0,
        camera_pitch: verse::camera::FollowCamera::default().pitch,
        camera_distance: verse::camera::FollowCamera::default().distance,
        camera_first_person: false,
        stick_pointer: None,
        look_stick_pointer: None,
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
        computer_page: crate::computer_hud::Page::Computers,
        computer_hud: crate::computer_hud::Snapshot::default(),
        computer_commands: Vec::new(),
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
        results: Gym {
            inside: false,
            near: false,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: 60.0,
        },
        results_open: false,
        results_revision: 0,
        results_active: false,
        results_view: None,
        evals: Gym {
            inside: false,
            near: false,
            visible: false,
            screen_x: 0.5,
            screen_y: 0.5,
            distance: 60.0,
        },
        evals_open: false,
        evals_revision: 0,
        evals_active: false,
        evals_view: None,
        gym_notes: false,
        studio_open: false,
        studio_revision: 0,
        studio_view: None,
        view,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WorldTarget {
    Computer,
    Gym,
    Results,
    Evals,
    Companion,
    Door(DoorId),
    Portal,
}

struct Touch {
    origin: [f32; 2],
    latest: [f32; 2],
    movement: bool,
    /// The touch holds the bare world's look stick. It is a look control, so
    /// it never shares the screen with a look drag.
    look_stick: bool,
    target: Option<WorldTarget>,
    tap_valid: bool,
    started: f64,
}

/// A button on a player's card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CardButton {
    Block,
    Mute,
    Close,
}

/// The card's size and inner spacing, in logical points.
const CARD_SIZE: [f32; 2] = [264.0, 96.0];
const CARD_PAD: f32 = 10.0;
const CARD_BUTTON_HEIGHT: f32 = 34.0;
/// How far from a name tag's center a tap still picks its player, in
/// logical points.
const TAG_REACH_POINTS: f32 = 28.0;

struct WorldTap {
    movement: bool,
    position: [f32; 2],
    finished: f64,
}

const TAP_DRIFT_POINTS: f32 = 12.0;
/// Slack past the mobile moving-pose interval for relay and scheduling jitter
/// when drawing bare-world players in the past.
const WORLD_TAP_SECONDS: f64 = 0.25;
/// Radius of the movement stick's drawn base, in logical points.
const STICK_RADIUS_POINTS: f32 = 56.0;
/// Characters of a player's hex pubkey shown over their head, as
/// [`verse::xp::name_tag`] shows them.
#[cfg(test)]
const PLAYER_TAG_CHARS: usize = 8;
/// A name tag's text color.
const TAG_COLOR: [f32; 4] = [0.9, 0.9, 0.9, 1.0];
/// Playtest title marks: a shape in gray, never a color.
const TITLE_COLOR: [f32; 4] = [0.62, 0.62, 0.62, 1.0];
const TITLE_PLAYTESTER: &str = "playtester";
const TITLE_FOUNDING: &str = "founding-playtester";
const TITLE_BUG_HUNTER: &str = "bug-hunter";
const TITLE_FIX_VERIFIER: &str = "fix-verifier";
const TITLE_RAIDER: &str = "raider";
/// The founding playtester's ground ring, in meters.
const FOUNDING_RING_RADIUS: f32 = 0.85;
const GROUND_RING_SEGMENTS: usize = 32;
/// The ball glows for a raider only while it moves at least this fast, in
/// meters per second.
const RAIDER_GLOW_SPEED: f64 = 0.3;
/// How close, in meters past the ball's radius, a raider's feet are when
/// pushing it.
const RAIDER_TOUCH: f32 = 0.8;
/// Players farther than this, in meters, carry no tag.
const PLAYER_TAG_RANGE: f32 = 60.0;
/// Height of a player's tag above their feet, in meters, as on desktop.
const PLAYER_TAG_LIFT: f32 = 2.2;
/// Gap between the stick's base and the safe area, in logical points.
const STICK_MARGIN_POINTS: f32 = 24.0;
/// Touches this far from the stick's center take the stick.
const STICK_GRAB_POINTS: f32 = STICK_RADIUS_POINTS * 1.25;
/// Stick deflection that starts movement along an axis, and the look
/// stick's radial dead zone.
const STICK_DEAD_POINTS: f32 = 12.0;
/// The bare world draws its sticks at this fraction of the Coder stick's
/// opacity, so they stay faint over the world.
const BARE_STICK_FAINTNESS: f32 = 0.5;
/// The look stick's turn rate at full deflection, in radians per second.
const LOOK_STICK_YAW_RATE: f32 = 1.9;
/// The look stick's pitch rate at full deflection, in radians per second.
const LOOK_STICK_PITCH_RATE: f32 = 1.15;
/// Time constant, in seconds, of the look stick's rate smoothing.
const LOOK_STICK_SMOOTHING_SECONDS: f32 = 0.12;
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

/// A stick touch's offset from the stick's `center`, clamped to its base.
fn deflection(center: [f32; 2], point: [f32; 2]) -> [f32; 2] {
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    let length = dx.hypot(dy);
    if length > STICK_RADIUS_POINTS {
        let k = STICK_RADIUS_POINTS / length;
        [dx * k, dy * k]
    } else {
        [dx, dy]
    }
}

pub(crate) struct Scene {
    /// The host asked for an extended-range surface (read by the iOS mount).
    #[cfg_attr(not(target_os = "ios"), allow(dead_code))]
    pub hdr_requested: bool,
    /// Boxed so a scene moves through the creation path as a pointer: the
    /// world's frames overflowed a phone's main-thread stack (#10928).
    pub world: Box<WorldRuntime>,
    pub atlas: verse::ui::Atlas,
    map: verse::minimap::MapHud,
    map_error: Option<String>,
    door_hud: verse::doors::hud::DoorHud,
    door_notice: Option<String>,
    zone_hud: verse::zones::hud::Hud,
    pub lifecycle: SurfaceLifecycle,
    pub session: Option<Box<Session>>,
    /// Remote geometry from the last presented frame, also used for picking.
    pub presented_entities: Box<verse::mesh::Mesh>,
    secret: secp256k1::SecretKey,
    public_key: String,
    pub(crate) relay: Option<String>,
    pub(crate) reader_relay: Option<String>,
    pub(crate) restore_spawn: bool,
    /// Where the block and mute lists live ([`verse::blocklist`]); none
    /// keeps them for this mount only.
    blocklist_directory: Option<std::path::PathBuf>,
    synthetic: bool,
    spawn_pending: bool,
    camera_mode: CameraMode,
    motion: Motion,
    /// The look stick's smoothed turn and pitch rates, in radians per second.
    look_rate: [f32; 2],
    frame_timestamp: Option<f64>,
    touches: BTreeMap<u64, Touch>,
    /// Safe-area insets in logical points: top, right, bottom, left.
    insets: [f32; 4],
    /// The touch holding Everglade's Levitate, while it is down.
    levitate: Option<u64>,
    /// A touch holding an Everglade hotbar slot: the touch, the slot, its
    /// intent, and when it went down on `pointer_clock`, s. A long press
    /// shows the slot's card instead of using it.
    slot_touch: Option<(u64, usize, ZoneIntent, f64)>,
    pointer_clock: Instant,
    last_world_tap: Option<WorldTap>,
    /// The player whose card is open: their name with Block, Mute, and
    /// Close.
    player_card: Option<String>,
    /// A contact that went down on a player's name tag; lifting it in place
    /// opens their card.
    card_touch: Option<(u64, String, [f32; 2])>,
    jump: bool,
    sprint: bool,
    computer_open: bool,
    computer_hud: crate::computer_hud::ComputerHud,
    pub(crate) gym_open: bool,
    gym_configuration_error: Option<String>,
    pub gym_board: verse::gym::Board,
    /// The host shows the native Gym panel, so a tap on the board may open
    /// it and the board shows its tap cue. A host without the panel keeps the
    /// building but never opens a panel it cannot close.
    pub(crate) gym_panel: bool,
    /// The Grid's RESULTS board and its panel. It needs no Gym connection.
    pub(crate) results: verse::gym_results::Results,
    pub(crate) results_open: bool,
    /// The host shows the native results panel, so a tap on the RESULTS
    /// board may open it and the board shows its tap cue.
    pub(crate) results_panel: bool,
    /// The Grid's EVALS board: published eval results and the agents'
    /// notes, read while the player is in the Gym. It exists while the
    /// bare world has a relay.
    pub(crate) hall: Option<verse::gym_hall::Hall>,
    /// The pylon league on the EVALS board, read while the player is in
    /// the Gym ([`verse::gym_league`]). Without a relay it reads nothing
    /// and says the Grid is offline.
    pub(crate) league: verse::gym_league::Reader,
    /// A host chose the league's relay and checkers (a local fixture);
    /// joining or leaving a relay keeps them.
    pub(crate) league_pinned: bool,
    pub(crate) evals_open: bool,
    /// The host shows the native EVALS panel, so a tap on the EVALS board
    /// may open it and the board shows its tap cue.
    pub(crate) evals_panel: bool,
    /// **Compare notes**, kept across relay changes.
    gym_notes: bool,
    /// The name over this player's head, from Account.
    display_name: Option<String>,
    /// The eval credit last passed to the hall.
    eval_credit_from: Option<usize>,
    /// The read-only NIP-XP reader, while the world is online. It trusts
    /// the OpenAgents referee alone and reads the public relay.
    xp: Option<verse::xp::Board>,
    /// The last ledger it derived, kept across tab switches for the tags.
    pub(crate) xp_snapshot: Option<verse::xp::Snapshot>,
    /// The read-only playtest reader, trusting the playtest referee alone.
    /// It never starts while [`verse::xp::PLAYTEST_REFEREE`] is unset.
    playtest: Option<verse::xp::Board>,
    /// Its last ledger: playtest titles, drawn as shapes on name tags.
    pub(crate) playtest_snapshot: Option<verse::xp::Snapshot>,
    /// Everglade's Agent Studio panel, while open.
    pub(crate) studio: Option<crate::studio_panel::Open>,
    /// The last studio view revision issued in this mount. Each rebuilt
    /// view takes the next, so a revision never names two trees.
    studio_revisions: u64,
    pub frames: u64,
    pub error: Option<String>,
    /// The shared chamber, from the RITUAL arch until the return.
    pub(crate) chamber: Option<Box<crate::chamber::Play>>,
}

/// The ledger the XP preview shows: six tutorial reproductions of 50 XP
/// by `secret`'s key, from a throwaway referee the preview alone trusts.
fn xp_preview(secret: secp256k1::SecretKey) -> Result<verse::xp::Snapshot, String> {
    let referee = verse::xp::fixture::signer(0x0a_de_fe_ee);
    let player = verse::identity::Identity::from_secret("phone", secret)?.signer;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1_790_000_000, |d| d.as_secs());
    let events = verse::xp::fixture::tutorial_events(&referee, &player, 6, at);
    let mut trust = verse::xp::openagents_trust();
    trust.referees = std::collections::BTreeSet::from([referee.pubkey().to_owned()]);
    Ok(verse::xp::snapshot(&events, &trust))
}

/// The playtest ledger the XP preview shows: a labeled fixture in which
/// `secret`'s key holds every playtest title, from a throwaway playtest
/// referee the preview alone trusts.
fn playtest_preview(secret: secp256k1::SecretKey) -> Result<verse::xp::Snapshot, String> {
    let referee = verse::xp::fixture::signer(0x91a7_7e57);
    let player = verse::identity::Identity::from_secret("phone", secret)?.signer;
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(1_790_000_000, |d| d.as_secs());
    let events = verse::xp::fixture::playtest_events(&referee, &player, at);
    let trust = verse::xp::XpTrust {
        referees: std::collections::BTreeSet::from([referee.pubkey().to_owned()]),
        runners: std::collections::BTreeSet::new(),
    };
    Ok(verse::xp::snapshot(&events, &trust))
}

/// Added to the moving interval for the crowd's drawing delay.
const BARE_PRESENCE_MARGIN: Duration = Duration::from_millis(300);

/// The chamber hotbar's abilities, in slot order.
const CHAMBER_SLOTS: [verse_world::play::Ability; 4] = [
    verse_world::play::Ability::Bow,
    verse_world::play::Ability::FireBolt,
    verse_world::play::Ability::MagicMissile,
    verse_world::play::Ability::Shield,
];

/// What a touch on the chamber HUD hit.
enum ChamberHit {
    Slot(usize),
    Leave,
    Respawn,
}

impl Scene {
    /// `#[inline(never)]` keeps this frame out of its callers'; the creation
    /// path's frames must stay small (#10928).
    #[inline(never)]
    pub fn new(config: Config) -> Result<Box<Self>, String> {
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
        if config.bare && (config.door_preferences.is_some() || config.computer_hud) {
            return Err("The bare world has no doors or computer".into());
        }
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
        // The Grid's shared ball, blocks, and pillar stand at fixed places, so
        // every visit starts at its spawn in view of them rather than wherever
        // the relay last saw this player.
        let restore_spawn = relay.is_some() && !config.synthetic && !config.bare;
        let mut gym_board = verse::gym::Board::new(secret, config.synthetic);
        let initial_gym_error = config
            .gym_code
            .as_deref()
            .and_then(|code| gym_board.configure(code).err());
        if config.synthetic_gym && !config.synthetic {
            return Err("The Gym preview requires synthetic mode".into());
        }
        let mut world = if config.bare {
            Box::new(WorldRuntime::bare())
        } else {
            let mut world = Box::new(WorldRuntime::new());
            // A phone opens a station's panel with the zone panel's button.
            world.interact_hint = verse::runtime::InteractHint::Tap;
            // Everglade's town clock runs as on the desktop, so a phone and
            // a computer see the same hour and villagers; a synthetic
            // session stays in daylight, so its checks don't change with
            // the hour.
            if !config.synthetic {
                world.set_town_clock(verse::town_clock::Clock::RUNNING);
            }
            world
        };
        if config.bare {
            world.set_ritual(config.ritual.as_deref().map(std::path::PathBuf::from));
        }
        if let Some(directory) = config.zone_cache_directory {
            if directory.is_empty()
                || directory.len() > 4096
                || !std::path::Path::new(&directory).is_absolute()
            {
                return Err("The zone cache requires an absolute native cache path".into());
            }
            world.configure_zone_cache(directory.into());
            // The medieval kit pack downloads from the web origin once and
            // stays in the cache by its digest.
            world.download_zone_kit(true);
            world.zone_kit_phone_tier();
        }
        // The bare world's only text is players' tags over the world, so
        // its font is rasterized at the screen's pixel scale. Everglade's
        // hotbar icons share the atlas.
        let mut atlas = verse::ui::Atlas::new(if config.bare {
            12.0 * config.scale.clamp(1.0, 4.0)
        } else {
            12.0
        });
        verse::zones::everglade::hotbar::add_sprites(&mut atlas)?;
        if config.computer_hud {
            atlas.reserve_glyphs(1024)?;
        }
        let mut zone_hud = verse::zones::hud::Hud::default();
        if config.bare {
            // Lagrange 1's panel stands above the Grid's sticks.
            zone_hud.set_bottom_clearance(STICK_MARGIN_POINTS + 2.0 * STICK_RADIUS_POINTS + 8.0)?;
        }
        let door_notice = config
            .door_preferences
            .as_deref()
            .and_then(|value| world.restore_door_state(value).err());
        if config.synthetic_gym
            && let Some(site) = world.gym_site()
        {
            // This explicit fixture starts outside the entrance, facing in. The
            // real touch path must cross the boundary before the board loads
            // its rows.
            let mut outside = verse::world::GYM_ENTRANCE;
            outside.x -= 2.0;
            world.set_spawn(
                site.point(outside),
                site.yaw_of(std::f32::consts::FRAC_PI_2),
            )?;
        }
        Ok(Box::new(Self {
            world,
            atlas,
            map: verse::minimap::MapHud::default(),
            map_error: None,
            door_hud: verse::doors::hud::DoorHud::default(),
            door_notice,
            zone_hud,
            lifecycle,
            session: None,
            presented_entities: Box::new(verse::mesh::Mesh::default()),
            secret,
            public_key: verse::identity::Identity::from_secret("phone", secret)?
                .signer
                .pubkey()
                .to_owned(),
            relay,
            restore_spawn,
            blocklist_directory: None,
            reader_relay: None,
            synthetic: config.synthetic,
            spawn_pending: false,
            camera_mode: CameraMode::Touch,
            motion: Motion::default(),
            look_rate: [0.0, 0.0],
            frame_timestamp: None,
            touches: BTreeMap::new(),
            insets: [0.0; 4],
            levitate: None,
            slot_touch: None,
            pointer_clock: Instant::now(),
            last_world_tap: None,
            player_card: None,
            card_touch: None,
            jump: false,
            sprint: false,
            computer_open: false,
            computer_hud: crate::computer_hud::ComputerHud::new(config.computer_hud),
            hdr_requested: config.hdr,
            gym_open: false,
            gym_configuration_error: initial_gym_error,
            gym_board,
            gym_panel: true,
            results: verse::gym_results::Results::new(verse::gym_results::Config {
                base: config
                    .results_base
                    .clone()
                    .unwrap_or_else(|| verse::gym_results::DEFAULT_BASE_URL.into()),
                cache_directory: config
                    .results_cache_directory
                    .as_ref()
                    .map(std::path::PathBuf::from),
            }),
            results_open: false,
            results_panel: false,
            hall: None,
            league: verse::gym_league::Reader::new(None, std::collections::BTreeSet::new()),
            league_pinned: false,
            evals_open: false,
            evals_panel: false,
            gym_notes: config.gym_notes,
            display_name: config.display_name.clone(),
            eval_credit_from: None,
            xp: None,
            xp_snapshot: if config.xp_preview {
                Some(xp_preview(secret)?)
            } else {
                None
            },
            playtest: None,
            playtest_snapshot: if config.xp_preview {
                Some(playtest_preview(secret)?)
            } else {
                None
            },
            studio: None,
            studio_revisions: 0,
            frames: 0,
            error: initial_error,
            chamber: None,
        }))
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
        if let Some(chamber) = &mut self.chamber {
            if active {
                chamber.resume();
            } else {
                chamber.suspend();
            }
        }
        if !active {
            self.world.zone_cancel_loading();
            self.zone_hud.clear_contacts();
            self.computer_hud.clear_contacts();
            self.world.cancel_navigation();
            self.map.clear_contacts();
            self.door_hud.clear_contacts();
            self.world.cancel_door_interactions();
            self.presented_entities = Box::new(verse::mesh::Mesh::default());
            self.touches.clear();
            self.jump = false;
            self.sprint = false;
            self.session = None;
            self.xp = None;
            self.playtest = None;
            // A paused surface stops the studio's source and drops its panel.
            self.studio = None;
            self.world.update_studio(false, 0.0);
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
        self.hall = None;
        if !self.league_pinned {
            self.league = verse::gym_league::Reader::new(None, std::collections::BTreeSet::new());
        }
        self.presented_entities = Box::new(verse::mesh::Mesh::default());
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

    /// Keeps the block and mute lists in `directory`, read each time the
    /// world's presence starts and written on each change.
    pub(crate) fn set_blocklist_directory(&mut self, directory: Option<std::path::PathBuf>) {
        self.blocklist_directory = directory;
    }

    /// Lets Everglade draw the owner's private placements kept in
    /// `directory`, signing grant requests with this mount's world key
    /// (`docs/verse/private-assets.md`).
    pub(crate) fn configure_private_assets(&mut self, directory: &str) -> Result<(), String> {
        let path = std::path::Path::new(directory);
        if directory.is_empty() || directory.len() > 4096 || !path.is_absolute() {
            return Err("Private assets require an absolute native directory".into());
        }
        let signer = verse::identity::Identity::from_secret("phone", self.secret)?.signer;
        self.world
            .configure_private_assets_as(path.to_path_buf(), signer);
        Ok(())
    }

    /// Blocks or unblocks the player with `pubkey` in the running presence
    /// session and saves the list. Returns whether it changed.
    pub fn set_player_blocked(&mut self, pubkey: &str, blocked: bool) -> Result<bool, String> {
        let session = self.session.as_mut().ok_or("The world is offline")?;
        let changed = if blocked {
            session.block(pubkey)?
        } else {
            session.unblock(pubkey)
        };
        if changed && let Some(directory) = &self.blocklist_directory {
            session.blocklist().save(directory)?;
        }
        Ok(changed)
    }

    /// Mutes or unmutes the player with `pubkey` in the running presence
    /// session and saves the list. Returns whether it changed.
    pub fn set_player_muted(&mut self, pubkey: &str, muted: bool) -> Result<bool, String> {
        let session = self.session.as_mut().ok_or("The world is offline")?;
        let changed = if muted {
            session.mute_player(pubkey)?
        } else {
            session.unmute_player(pubkey)
        };
        if changed && let Some(directory) = &self.blocklist_directory {
            session.blocklist().save(directory)?;
        }
        Ok(changed)
    }

    /// The remote player whose name tag is under `(x, y)`, in logical
    /// points: the nearest within [`TAG_REACH_POINTS`] of a tag's center.
    pub(crate) fn player_at(&self, x: f32, y: f32) -> Option<String> {
        if !self.world.is_bare()
            || self.chamber.is_some()
            || !self.lifecycle.active()
            || self.panel_open()
            || !x.is_finite()
            || !y.is_finite()
        {
            return None;
        }
        let session = self.session.as_ref()?;
        let size = self.lifecycle.viewport().logical_size();
        let line = self.atlas.line / self.lifecycle.viewport().scale().max(1.0);
        let view_proj = self.world.view(self.aspect()).view_proj;
        session
            .crowd
            .shown(Instant::now())
            .into_iter()
            .filter(|shown| {
                shown.role == "avatar"
                    && shown.pubkey != self.public_key
                    && shown.pos.distance(self.world.player.pos) <= PLAYER_TAG_RANGE
            })
            .filter_map(|shown| {
                let mut head = shown.pos;
                head.y += PLAYER_TAG_LIFT;
                let [tx, ty] = verse::hud::project(view_proj, size, head)?;
                let distance = (x - tx).hypot(y - (ty - line / 2.0));
                (distance <= TAG_REACH_POINTS).then_some((distance, shown.pubkey))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, pubkey)| pubkey)
    }

    /// Opens `pubkey`'s card.
    pub(crate) fn open_player_card(&mut self, pubkey: String) {
        self.player_card = Some(pubkey);
    }

    /// Whose card is open.
    pub(crate) fn player_card(&self) -> Option<&str> {
        self.player_card.as_deref()
    }

    /// The card's rectangle, `[x, y, width, height]` in logical points:
    /// centered, below the top inset.
    fn card_rect(&self) -> [f32; 4] {
        let size = self.lifecycle.viewport().logical_size();
        let width = CARD_SIZE[0].min(size[0] - 2.0 * CARD_PAD).max(0.0);
        [
            (size[0] - width) / 2.0,
            self.insets[0] + 72.0,
            width,
            CARD_SIZE[1],
        ]
    }

    fn card_buttons(&self) -> [(CardButton, [f32; 4]); 3] {
        let [x, y, width, height] = self.card_rect();
        let button = (width - 4.0 * CARD_PAD) / 3.0;
        let top = y + height - CARD_PAD - CARD_BUTTON_HEIGHT;
        let at = |n: f32| {
            [
                x + CARD_PAD + n * (button + CARD_PAD),
                top,
                button,
                CARD_BUTTON_HEIGHT,
            ]
        };
        [
            (CardButton::Block, at(0.0)),
            (CardButton::Mute, at(1.0)),
            (CardButton::Close, at(2.0)),
        ]
    }

    /// The card button under `point`, in logical points, while a card is
    /// open.
    pub(crate) fn card_hit(&self, point: [f32; 2]) -> Option<CardButton> {
        self.player_card.as_ref()?;
        self.card_buttons()
            .into_iter()
            .find(|(_, [x, y, w, h])| {
                (*x..=x + w).contains(&point[0]) && (*y..=y + h).contains(&point[1])
            })
            .map(|(button, _)| button)
    }

    /// Carries out a card button and closes the card.
    pub(crate) fn card_press(&mut self, button: CardButton) -> Result<(), String> {
        let Some(pubkey) = self.player_card.take() else {
            return Ok(());
        };
        match button {
            CardButton::Block => {
                self.set_player_blocked(&pubkey, true)?;
            }
            CardButton::Mute => {
                let muted = self
                    .session
                    .as_ref()
                    .is_some_and(|session| session.blocklist().muted.contains(&pubkey));
                self.set_player_muted(&pubkey, !muted)?;
            }
            CardButton::Close => {}
        }
        Ok(())
    }

    /// The open card, drawn into `ui` in pixels: the player's name, then
    /// Block, Mute (or Unmute), and Close.
    fn draw_player_card(&self, ui: &mut verse::ui::UiBatch) {
        let Some(pubkey) = &self.player_card else {
            return;
        };
        let scale = self.lifecycle.viewport().scale();
        let px = |rect: [f32; 4]| rect.map(|v| v * scale);
        let [x, y, w, h] = px(self.card_rect());
        ui.rect(&self.atlas, x, y, w, h, [0.05, 0.05, 0.05, 0.92]);
        ui.frame(&self.atlas, x, y, w, h, scale, [0.6, 0.6, 0.6, 1.0]);
        let name = self
            .session
            .as_ref()
            .map_or_else(|| pubkey[..8].to_owned(), |session| session.name_of(pubkey));
        let pad = CARD_PAD * scale;
        ui.text(&self.atlas, x + pad, y + pad, &name, TAG_COLOR);
        let muted = self
            .session
            .as_ref()
            .is_some_and(|session| session.blocklist().muted.contains(pubkey));
        for (button, rect) in self.card_buttons() {
            let [bx, by, bw, bh] = px(rect);
            ui.frame(&self.atlas, bx, by, bw, bh, scale, [0.6, 0.6, 0.6, 1.0]);
            let label = match button {
                CardButton::Block => "BLOCK",
                CardButton::Mute if muted => "UNMUTE",
                CardButton::Mute => "MUTE",
                CardButton::Close => "CLOSE",
            };
            let width = self.atlas.measure(label);
            ui.text(
                &self.atlas,
                bx + (bw - width) / 2.0,
                by + (bh - self.atlas.line) / 2.0,
                label,
                TAG_COLOR,
            );
        }
    }

    pub(crate) fn world_signer(&self) -> Result<nostr::domain::RelaySigner, String> {
        Ok(verse::identity::Identity::from_secret("phone", self.secret)?.signer)
    }

    fn start_session(&mut self) -> Result<(), String> {
        let world = self.presence_world().ok_or("The zone is still loading")?;
        let identity = verse::identity::Identity::from_secret("phone", self.secret)?;
        let relay = self.relay.as_deref().ok_or("No Verse relay selected")?;
        let intervals = verse::session::PublishIntervals::mobile();
        let mut session = if world != verse::session::WORLD {
            let mut session = Session::start_presence(identity, relay, world)?;
            session.set_display_name(self.display_name.as_deref());
            // Every bare-world player publishes at this mobile cadence; draw
            // them one moving interval in the past so they walk continuously
            // between sparse poses instead of jumping at each one.
            session
                .crowd
                .set_delay(intervals.moving + BARE_PRESENCE_MARGIN);
            // Only players there now: nobody who left stays behind as a figure.
            session.crowd.set_live_only(true);
            session
        } else {
            Session::start_with_identity(identity, relay)?
        };
        session.set_publish_intervals(intervals)?;
        if let Some(directory) = &self.blocklist_directory
            && let Ok(people) = verse::blocklist::Blocklist::load(directory)
        {
            session.set_blocklist(people);
        }
        self.spawn_pending = std::mem::take(&mut self.restore_spawn);
        if self.spawn_pending {
            session.begin_spawn(Duration::from_millis(1500));
        }
        self.reset_motion();
        self.gym_board.set_active(false);
        self.results.set_active(false);
        self.session = Some(Box::new(session));
        if self.world.is_bare() && self.hall.is_none() {
            self.hall = Some(verse::gym_hall::Hall::new(
                verse::gym_hall::Config {
                    relay: self.relay.clone().ok_or("No Verse relay selected")?,
                    world: verse::session::BARE_WORLD.to_owned(),
                    signer: verse::identity::Identity::from_secret("phone", self.secret)?.signer,
                },
                self.gym_notes,
            ));
            self.eval_credit_from = None;
        }
        if self.world.is_bare() && !self.league_pinned && self.league.relay().is_none() {
            // The same relay and trusted checkers as `openagents pylon league`.
            self.league = verse::gym_league::Reader::new(
                Some(
                    self.reader_relay
                        .clone()
                        .unwrap_or_else(|| verse::session::PUBLIC_RELAY.to_owned()),
                ),
                verse::gym_league::Reader::trusted(),
            );
        }
        if self.world.is_bare() && self.xp.is_none() {
            let signer = verse::identity::Identity::from_secret("phone", self.secret)?.signer;
            self.xp = Some(verse::xp::Board::start_with(
                self.reader_relay
                    .as_deref()
                    .unwrap_or(verse::session::PUBLIC_RELAY),
                verse::xp::openagents_trust(),
                None,
                Some(signer.clone()),
            ));
        }
        if self.world.is_bare()
            && self.playtest.is_none()
            && let Some(trust) = verse::xp::playtest_trust()
        {
            self.playtest = Some(verse::xp::Board::start_with(
                self.reader_relay
                    .as_deref()
                    .unwrap_or(verse::session::PUBLIC_RELAY),
                trust,
                None,
                Some(verse::identity::Identity::from_secret("phone", self.secret)?.signer),
            ));
        }
        Ok(())
    }

    pub fn disconnect(&mut self) {
        self.presented_entities = Box::new(verse::mesh::Mesh::default());
        self.session = None;
        self.hall = None;
        if !self.league_pinned {
            self.league = verse::gym_league::Reader::new(None, std::collections::BTreeSet::new());
        }
        self.evals_open = false;
        self.xp = None;
        self.playtest = None;
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
            self.computer_hud.clear_contacts();
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
        // The open computer's HUD takes every contact; the world behind it
        // sees none. A contact that began on the HUD ends there.
        if (self.computer_open && self.computer_hud.drawn()) || self.computer_hud.captured(id) {
            if !x.is_finite() || !y.is_finite() || x.abs() > 32768.0 || y.abs() > 32768.0 {
                self.computer_hud.clear_contacts();
                return Err("Touch coordinates exceed their bounds".into());
            }
            let size = self.lifecycle.viewport().logical_size();
            match phase {
                PointerPhase::Down if self.lifecycle.active() => {
                    self.computer_hud.down(id, point);
                }
                PointerPhase::Down => {}
                PointerPhase::Move => self.computer_hud.moved(&self.atlas, size, id, point),
                PointerPhase::Up | PointerPhase::Cancel => {
                    let close = self.computer_hud.up(
                        &self.atlas,
                        size,
                        id,
                        point,
                        matches!(phase, PointerPhase::Cancel) || !self.computer_open,
                    );
                    if close {
                        self.close_computer();
                    }
                }
            }
            return Ok(());
        }
        // A touch on a hotbar slot other than Levitate acts when it lifts
        // after a tap; a long press shows the slot's card instead. Levitate
        // rises while held and is let go when the touch lifts.
        if let Some((held, _, intent, at)) = self.slot_touch
            && held == id
        {
            if matches!(phase, PointerPhase::Up | PointerPhase::Cancel) {
                self.slot_touch = None;
                if self.levitate == Some(id) {
                    self.levitate = None;
                    let _ = self.world.everglade_levitate(false);
                }
                let long = verse::tooltip::long_press(
                    (self.pointer_clock.elapsed().as_secs_f64() - at) as f32,
                );
                if matches!(phase, PointerPhase::Up) && !long && intent != ZoneIntent::Levitate {
                    self.zone_intent(intent)?;
                }
            }
            return Ok(());
        }
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
        // An open player card takes every new contact: a button acts, and
        // anywhere else closes it.
        if self.player_card.is_some() && self.chamber.is_none() {
            if matches!(phase, PointerPhase::Down) {
                self.cancel_taps();
                match self.card_hit(point) {
                    Some(button) => self.card_press(button)?,
                    None => self.player_card = None,
                }
                return Ok(());
            }
            if !self.touches.contains_key(&id) {
                return Ok(());
            }
        }
        // The bare world draws no map or door controls to touch, and zone
        // controls only while a zone loads or inside the zone a portal leads
        // to.
        if self.chamber.is_some() {
            if matches!(phase, PointerPhase::Down)
                && let Some(hit) = self.chamber_hud_hit(point)
            {
                self.cancel_taps();
                match hit {
                    ChamberHit::Slot(slot) => self.chamber_cast(slot),
                    ChamberHit::Leave => self.leave_chamber()?,
                    ChamberHit::Respawn => {
                        if let Some(session) = self.chamber.as_mut().and_then(|c| c.session_mut()) {
                            session.respawn();
                        }
                    }
                }
                return Ok(());
            }
            return self.pointer_at(id, phase, x, y, self.pointer_clock.elapsed().as_secs_f64());
        }
        // Everglade has no arch back to the Grid; its Leave button is the
        // way out.
        if matches!(phase, PointerPhase::Down)
            && self.everglade_hotbar_shown()
            && self.on_everglade_leave(point)
        {
            self.cancel_taps();
            return self.zone_intent(ZoneIntent::Return);
        }
        if matches!(phase, PointerPhase::Down)
            && self.everglade_hotbar_shown()
            && let Some(slots) = self.world.everglade_hotbar()
            && let Some(index) = verse::zones::everglade::hotbar::slot_under(
                point,
                self.lifecycle.viewport().logical_size(),
                self.hotbar_bottom(),
                slots.len(),
            )
        {
            self.cancel_taps();
            let intent = verse::zones::everglade::hotbar::SLOTS[index].0;
            self.slot_touch = Some((
                id,
                index,
                intent,
                self.pointer_clock.elapsed().as_secs_f64(),
            ));
            if intent == ZoneIntent::Levitate
                && self.levitate.is_none()
                && self.world.everglade_levitate(true).is_ok()
            {
                self.levitate = Some(id);
            }
            return Ok(());
        }
        if matches!(phase, PointerPhase::Down) && self.bare_zone_panel() {
            let snapshot = self.zone_hud_snapshot();
            if self.zone_hud.down(id, point, &snapshot) {
                self.cancel_taps();
                return Ok(());
            }
        }
        if matches!(phase, PointerPhase::Down) && !self.world.is_bare() {
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
        // A tap on a player's name tag opens their card; a drag that starts
        // there still turns the camera.
        match phase {
            PointerPhase::Down => {
                self.card_touch = self.player_at(x, y).map(|pubkey| (id, pubkey, point));
            }
            PointerPhase::Up => {
                if let Some((_, pubkey, origin)) =
                    self.card_touch.take_if(|(touch, _, _)| *touch == id)
                    && (x - origin[0]).hypot(y - origin[1]) <= TAP_DRIFT_POINTS
                {
                    self.cancel_taps();
                    self.player_card = Some(pubkey);
                }
            }
            PointerPhase::Cancel => self.card_touch = None,
            PointerPhase::Move => {}
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
                    Some(WorldTarget::Results) => self.open_results(),
                    Some(WorldTarget::Evals) => self.open_evals(),
                    Some(WorldTarget::Companion) => {
                        self.world.pet_companion();
                    }
                    Some(WorldTarget::Door(door)) => {
                        self.door_intent_at(DoorIntent::Tap(door), Some([x, y]))?;
                    }
                    None => {}
                }
            } else if valid
                && touch.target.is_none()
                && !touch.movement
                && elapsed <= WORLD_TAP_SECONDS
                && self.world.demolition_targeting()
            {
                // Aiming Meteor Swarm: a tap casts it where it lands.
                let size = self.lifecycle.viewport().logical_size();
                let aspect = self.aspect();
                self.world.demolition_aim(
                    aspect,
                    (x / size[0].max(1.0)).clamp(0.0, 1.0),
                    (y / size[1].max(1.0)).clamp(0.0, 1.0),
                );
                self.world.demolition_confirm();
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
                let movement = self.on_stick(x, y);
                let look_stick = !movement && self.on_look_stick(x, y);
                // A tap off the stick is independent from an established movement
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
                        look_stick,
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
                        // Continue with the control chosen at the touch's start.
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
                    // The look stick turns at a rate in `update`, not by
                    // its travel.
                    if touch.target.is_none()
                        && !touch.movement
                        && !touch.look_stick
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

    /// Center of the movement stick, above the bottom-left safe area.
    fn stick_center(&self) -> [f32; 2] {
        let size = self.lifecycle.viewport().logical_size();
        [
            self.insets[3] + STICK_MARGIN_POINTS + STICK_RADIUS_POINTS,
            size[1] - self.insets[2] - STICK_MARGIN_POINTS - STICK_RADIUS_POINTS,
        ]
    }

    /// Center of the look stick, above the bottom-right safe area: the
    /// bare world's second stick, shown only in touch look. Motion look
    /// turns the camera with the phone, so a stick there would fight it.
    fn look_stick_center(&self) -> Option<[f32; 2]> {
        if !self.world.is_bare() || self.camera_mode != CameraMode::Touch {
            return None;
        }
        let size = self.lifecycle.viewport().logical_size();
        Some([
            size[0] - self.insets[1] - STICK_MARGIN_POINTS - STICK_RADIUS_POINTS,
            size[1] - self.insets[2] - STICK_MARGIN_POINTS - STICK_RADIUS_POINTS,
        ])
    }

    fn on_stick(&self, x: f32, y: f32) -> bool {
        let center = self.stick_center();
        (x - center[0]).hypot(y - center[1]) <= STICK_GRAB_POINTS
    }

    fn on_look_stick(&self, x: f32, y: f32) -> bool {
        self.look_stick_center()
            .is_some_and(|center| (x - center[0]).hypot(y - center[1]) <= STICK_GRAB_POINTS)
    }

    /// A held stick touch's offset from the stick's center, clamped to its base.
    fn stick_deflection(&self, point: [f32; 2]) -> [f32; 2] {
        deflection(self.stick_center(), point)
    }

    /// The touch holding the look stick, if one does.
    fn look_stick_touch(&self) -> Option<(u64, &Touch)> {
        self.touches
            .iter()
            .find(|(_, touch)| touch.look_stick && touch.target.is_none())
            .map(|(id, touch)| (*id, touch))
    }

    /// The turn and pitch rates, in radians per second, that the look stick
    /// asks for with a touch at `point`: nothing inside the dead zone, then
    /// rising with the square of the deflection past it, for fine aim near
    /// the center and a quick turn at the rim.
    fn look_stick_target(&self, point: [f32; 2]) -> [f32; 2] {
        let Some(center) = self.look_stick_center() else {
            return [0.0, 0.0];
        };
        let [dx, dy] = deflection(center, point);
        let length = dx.hypot(dy);
        if length <= STICK_DEAD_POINTS {
            return [0.0, 0.0];
        }
        let reach = ((length - STICK_DEAD_POINTS) / (STICK_RADIUS_POINTS - STICK_DEAD_POINTS))
            .clamp(0.0, 1.0);
        let magnitude = reach * reach / length;
        [
            dx * magnitude * LOOK_STICK_YAW_RATE,
            dy * magnitude * LOOK_STICK_PITCH_RATE,
        ]
    }

    /// Turns the camera at the look stick's smoothed rate for `dt` seconds.
    /// Right turns right and up looks up, as a look drag does. Releasing the
    /// stick stops the turn at once; smoothing only steadies a held thumb.
    fn advance_look_stick(&mut self, dt: f32) -> Result<(), String> {
        let held = self
            .look_stick_touch()
            .map(|(_, touch)| touch.latest)
            .filter(|_| self.lifecycle.active() && !self.panel_open() && !self.spawn_pending);
        let Some(point) = held else {
            self.look_rate = [0.0, 0.0];
            return Ok(());
        };
        let target = self.look_stick_target(point);
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let k = 1.0 - (-dt / LOOK_STICK_SMOOTHING_SECONDS).exp();
        for (rate, target) in self.look_rate.iter_mut().zip(target) {
            *rate += (target - *rate) * k;
        }
        let dx = self.look_rate[0] * dt / verse::camera::SENSITIVITY;
        let dy = self.look_rate[1] * dt / verse::camera::SENSITIVITY;
        if dx != 0.0 || dy != 0.0 {
            self.world.apply(Action::FaceCamera)?;
            self.world.apply(Action::Look { dx, dy })?;
        }
        Ok(())
    }

    /// The bare world's sticks, ring and knob, faint over the world: the
    /// movement stick at the bottom left and, in touch look, the look stick
    /// at the bottom right. Coder's world draws only its movement stick, at
    /// full strength.
    fn stick_ui(&self) -> verse::ui::UiBatch {
        let mut ui = verse::ui::UiBatch::default();
        if !self.lifecycle.active() || self.panel_open() || self.spawn_pending {
            return ui;
        }
        let faint = if self.world.is_bare() {
            BARE_STICK_FAINTNESS
        } else {
            1.0
        };
        let movement = self
            .touches
            .values()
            .find(|p| p.movement && p.target.is_none())
            .map(|touch| touch.latest);
        self.draw_stick(&mut ui, self.stick_center(), movement, faint);
        if let Some(center) = self.look_stick_center() {
            let look = self.look_stick_touch().map(|(_, touch)| touch.latest);
            self.draw_stick(&mut ui, center, look, faint);
        }
        ui
    }

    fn draw_stick(
        &self,
        ui: &mut verse::ui::UiBatch,
        center: [f32; 2],
        held: Option<[f32; 2]>,
        faint: f32,
    ) {
        let scale = self.lifecycle.viewport().scale();
        let knob = held.map_or([0.0, 0.0], |point| deflection(center, point));
        let base = STICK_RADIUS_POINTS * scale;
        let alpha = if held.is_some() { 0.55 } else { 0.3 };
        let x = center[0] * scale;
        let y = center[1] * scale;
        ui.ring(
            &self.atlas,
            x,
            y,
            base,
            2.0 * scale,
            [1.0, 1.0, 1.0, alpha * faint],
        );
        ui.disc(
            &self.atlas,
            x + knob[0] * scale,
            y + knob[1] * scale,
            base * 0.35,
            [1.0, 1.0, 1.0, (alpha + 0.25) * faint],
        );
    }

    /// Every player's pubkey prefix over their head, and their level when
    /// their key has XP under the OpenAgents referee (`650a2a22 · lv 3`),
    /// this player's included unless the camera is inside its head.
    pub(crate) fn player_tags(&self) -> verse::ui::UiBatch {
        let mut ui = verse::ui::UiBatch::default();
        if !self.lifecycle.active() || self.panel_open() {
            return ui;
        }
        let mut players = Vec::new();
        if !self.world.first_person() {
            players.push((self.public_key.as_str(), self.world.player.pos));
        }
        let shown = self
            .session
            .as_ref()
            .map(|session| session.crowd.shown(Instant::now()))
            .unwrap_or_default();
        players.extend(
            shown
                .iter()
                .filter(|shown| {
                    shown.role == "avatar"
                        && shown.pos.distance(self.world.player.pos) <= PLAYER_TAG_RANGE
                })
                .map(|shown| (shown.pubkey.as_str(), shown.pos)),
        );
        let viewport = self.lifecycle.viewport();
        let size = viewport.logical_size().map(|v| v * viewport.scale());
        let view_proj = self.world.view(self.aspect()).view_proj;
        for (pubkey, feet) in players {
            let mut head = feet;
            head.y += PLAYER_TAG_LIFT;
            let Some([x, y]) = verse::hud::project(view_proj, size, head) else {
                continue;
            };
            let name = self
                .session
                .as_ref()
                .map(|session| session.name_of(pubkey))
                .filter(|name| !name.ends_with('…'))
                .or_else(|| {
                    (pubkey == self.public_key)
                        .then(|| self.display_name.clone())
                        .flatten()
                });
            let tag = verse::xp::name_tag(self.xp_snapshot.as_ref(), pubkey, name.as_deref());
            let width = self.atlas.measure(&tag);
            let top = y - self.atlas.line;
            ui.text(&self.atlas, x - width / 2.0, top, &tag, TAG_COLOR);
            let titles = verse::xp::playtest_titles(self.playtest_snapshot.as_ref(), pubkey);
            if !titles.is_empty() {
                self.playtest_marks(&mut ui, &titles, [x, top], width, feet.to_array());
            }
        }
        self.raider_glow(&mut ui);
        self.draw_player_card(&mut ui);
        ui
    }

    /// Playtest titles as shapes, never colors: **PLAYTESTER** under the
    /// tag, a crosshair left of it for `bug-hunter`, a check right of it for
    /// `fix-verifier`, and a thin ring on the ground under the avatar for
    /// `founding-playtester`. `raider` shows on the ball
    /// ([`Self::raider_glow`]).
    fn playtest_marks(
        &self,
        ui: &mut verse::ui::UiBatch,
        titles: &std::collections::BTreeSet<String>,
        [x, top]: [f32; 2],
        width: f32,
        feet: [f32; 3],
    ) {
        let line = self.atlas.line;
        let half = line * 0.32;
        let stroke = (line * 0.09).max(1.0);
        let middle = top + line * 0.5;
        if titles.contains(TITLE_PLAYTESTER) {
            let label = "PLAYTESTER";
            let w = self.atlas.measure(label);
            ui.text(&self.atlas, x - w / 2.0, top + line, label, TITLE_COLOR);
        }
        if titles.contains(TITLE_BUG_HUNTER) {
            let cx = x - width / 2.0 - line * 0.6;
            ui.ring(&self.atlas, cx, middle, half, stroke, TITLE_COLOR);
            for [dx, dy] in [[1.0, 0.0], [-1.0, 0.0], [0.0, 1.0], [0.0, -1.0]] {
                ui.line(
                    &self.atlas,
                    [cx + dx * half * 0.5, middle + dy * half * 0.5],
                    [cx + dx * half * 1.4, middle + dy * half * 1.4],
                    stroke,
                    TITLE_COLOR,
                );
            }
        }
        if titles.contains(TITLE_FIX_VERIFIER) {
            let left = x + width / 2.0 + line * 0.25;
            let low = [left + half * 0.7, middle + half * 0.8];
            ui.line(&self.atlas, [left, middle], low, stroke * 1.4, TITLE_COLOR);
            ui.line(
                &self.atlas,
                low,
                [left + half * 2.0, middle - half],
                stroke * 1.4,
                TITLE_COLOR,
            );
        }
        if titles.contains(TITLE_FOUNDING) {
            self.ground_ring(ui, feet, FOUNDING_RING_RADIUS, stroke, TITLE_COLOR);
        }
    }

    /// A thin circle of radius `radius` meters on the ground at `center`,
    /// drawn as projected strokes.
    fn ground_ring(
        &self,
        ui: &mut verse::ui::UiBatch,
        center: [f32; 3],
        radius: f32,
        stroke: f32,
        color: [f32; 4],
    ) {
        let viewport = self.lifecycle.viewport();
        let size = viewport.logical_size().map(|v| v * viewport.scale());
        let view_proj = self.world.view(self.aspect()).view_proj;
        let point = |i: usize| {
            let a = i as f32 / GROUND_RING_SEGMENTS as f32 * std::f32::consts::TAU;
            // A world point, in the type the player's position has.
            let mut p = self.world.player.pos;
            p.x = center[0] + radius * a.cos();
            p.y = center[1] + 0.03;
            p.z = center[2] + radius * a.sin();
            verse::hud::project(view_proj, size, p)
        };
        for i in 0..GROUND_RING_SEGMENTS {
            if let (Some(a), Some(b)) = (point(i), point(i + 1)) {
                ui.line(&self.atlas, a, b, stroke, color);
            }
        }
    }

    /// The ball glows briefly, as a ring around it, while a player with
    /// the `raider` title is pushing it: touching it as it moves.
    fn raider_glow(&self, ui: &mut verse::ui::UiBatch) {
        let Some(ball) = self.world.ball() else {
            return;
        };
        let body = ball.body();
        if body.vel.length() < RAIDER_GLOW_SPEED {
            return;
        }
        let center = body.pos.as_vec3();
        let reach = (verse::ball::RADIUS as f32) + RAIDER_TOUCH;
        let mut players = vec![(self.public_key.clone(), self.world.player.pos)];
        if let Some(session) = self.session.as_ref() {
            players.extend(
                session
                    .crowd
                    .shown(Instant::now())
                    .into_iter()
                    .filter(|shown| shown.role == "avatar")
                    .map(|shown| (shown.pubkey, shown.pos)),
            );
        }
        let pushing = players.iter().any(|(pubkey, feet)| {
            let flat = (feet.x - center.x).hypot(feet.z - center.z);
            flat <= reach
                && verse::xp::playtest_titles(self.playtest_snapshot.as_ref(), pubkey)
                    .contains(TITLE_RAIDER)
        });
        if pushing {
            let floor = [center.x, center.y - verse::ball::RADIUS as f32, center.z];
            let stroke = (self.atlas.line * 0.12).max(1.0);
            self.ground_ring(
                ui,
                floor,
                verse::ball::RADIUS as f32 * 1.35,
                stroke,
                TITLE_COLOR,
            );
        }
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
            let [x, y] = self.stick_deflection(touch.latest);
            input.forward = match self.camera_mode {
                CameraMode::Touch => y < -STICK_DEAD_POINTS,
                CameraMode::Motion => y <= STICK_DEAD_POINTS,
            };
            input.backward = y > STICK_DEAD_POINTS;
            input.strafe_left = x < -STICK_DEAD_POINTS;
            input.strafe_right = x > STICK_DEAD_POINTS;
        }
        input.mouse_look = self.motion_needed()
            || self
                .touches
                .values()
                .any(|p| !p.movement && p.target.is_none());
        input
    }

    pub fn update(&mut self, timestamp: f64) -> Result<Option<f32>, String> {
        self.update_with_input(timestamp, None)
    }

    /// Native callers supply keyboard intent while sharing the same scene.
    pub(crate) fn update_with_input(
        &mut self,
        timestamp: f64,
        input: Option<InputState>,
    ) -> Result<Option<f32>, String> {
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
        if let Some(config) = self.world.take_ritual_crossing() {
            self.reset_zone_inputs();
            self.chamber = Some(Box::new(crate::chamber::Play::open(config, self.secret)));
            // The Grid's presence pauses while the player is in the chamber.
            self.session = None;
        }
        if self.chamber.is_some() {
            self.step_chamber(camera_dt as f32, input)?;
            return Ok(Some(dt));
        }
        if self.spawn_pending {
            self.gym_board.set_active(false);
            self.results.set_active(false);
            if let Some(hall) = &mut self.hall {
                hall.set_active(false);
            }
            if let Some(session) = &mut self.session {
                if let Some(spawn) =
                    session.poll_spawn(&self.world.world.blockers, self.world.zone_half())
                {
                    // A new Grid player starts at the same place as mobile;
                    // only a signed retained pose may replace that spawn.
                    if !self.world.is_bare() || spawn.resumed {
                        self.world.set_spawn(spawn.pos, spawn.yaw)?;
                    }
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
        self.advance_look_stick(camera_dt as f32)?;
        // Other players' avatars, where they are drawn, are solid.
        let now = Instant::now();
        self.world
            .set_avatars(self.session.as_ref().map_or_else(Vec::new, |session| {
                session
                    .crowd
                    .shown(now)
                    .into_iter()
                    .filter(|shown| shown.role == "avatar")
                    .map(|shown| shown.pos)
                    .collect()
            }));
        let input = if self.panel_open() {
            InputState::default()
        } else {
            input.unwrap_or_else(|| self.input())
        };
        let revision = self.world.zone_revision;
        let loading = self.world.zone_loading();
        self.world.tick(&input, dt);
        if self.world.zone_revision != revision || self.world.zone_loading() != loading {
            // The player walked through a portal, or into one whose zone
            // now loads: drop held input, and pause or resume the world's
            // presence as a button entry would.
            self.reset_zone_inputs();
            self.sync_zone_session()?;
        }
        self.map.tick(dt);
        let panel_was_open = self.panel_open();
        if self.computer_open && !self.computer().near {
            self.computer_open = false;
            self.computer_hud.close();
        }
        if !self.gym().inside {
            self.gym_open = false;
            self.results_open = false;
            self.evals_open = false;
        }
        // Everglade's studio observes only while the surface is active and
        // the player is in Everglade; an open panel follows its changes.
        self.world.update_studio(self.lifecycle.active(), dt);
        self.sync_studio()?;
        if panel_was_open != self.panel_open() {
            self.reset_motion();
        }
        self.sync_gym_interest();
        self.gym_board.poll();
        self.results.poll();
        if let Some(board) = &mut self.xp {
            board.tick();
            if let Some(snapshot) = board.snapshot.take() {
                self.xp_snapshot = Some(snapshot);
            }
        }
        if let Some(board) = &mut self.playtest {
            board.tick();
            if let Some(snapshot) = board.snapshot.take() {
                self.playtest_snapshot = Some(snapshot);
            }
        }
        self.poll_hall(now);
        self.league.poll();
        if self.results_open {
            self.results.tick(f64::from(dt));
        }
        // An open trace plays in the Gym: its ghost glides to the station
        // of the viewer's current row.
        let target = self
            .results_open
            .then(|| self.results.replay_place())
            .flatten()
            .zip(self.world.gym_site())
            .map(|(place, site)| verse::gym_replay::ghost_at(site, place));
        self.world.trace_ghost = target.map(|target| {
            self.world
                .trace_ghost
                .map_or(target, |at| at + (target - at) * (dt * 4.0).min(1.0))
        });
        let now = Instant::now();
        if let Some(session) = &mut self.session
            && self.world.is_bare()
        {
            // Presence and the shared ball and blocks: the bare world's
            // session has no agent to scan or greet with.
            session.tick_world(now, &mut self.world);
        } else if let Some(session) = &mut self.session {
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
                self.computer_hud.set_insets([top, right, bottom, left]);
                self.zone_hud.set_insets([top, right, bottom, left])?;
                self.insets = [top, right, bottom, left];
                Ok(())
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
            Request::EnterEverglade => self.world.enter_everglade(),
            Request::LeaveChamber => self.leave_chamber(),
            Request::ChamberBindings { bindings } => self
                .chamber
                .as_mut()
                .ok_or("No chamber visit is active")?
                .remap(bindings),
            Request::ChamberCast { slot } => {
                self.chamber_cast(slot);
                Ok(())
            }
            Request::ChamberRespawn => {
                if let Some(session) = self.chamber.as_mut().and_then(|c| c.session_mut()) {
                    session.respawn();
                }
                Ok(())
            }
            Request::GoStation { station } => self.go_station(&station),
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
                // A pinch owns its two fingers, never a stick's: a thumb on
                // a stick keeps walking or looking while other fingers zoom.
                self.touches
                    .retain(|_, touch| touch.movement || touch.look_stick);
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
                self.close_computer();
                Ok(())
            }
            Request::ComputerFeed { feed } => self.computer_hud.feed(*feed),
            Request::ComputerPage { page } => {
                if !self.computer_open {
                    return Err("Open the computer first".into());
                }
                self.computer_hud.set_page(page)
            }
            Request::ComputerHudTap { key } => {
                if !self.lifecycle.active() || !self.computer_open || !self.computer_hud.drawn() {
                    return Err("Open the computer first".into());
                }
                let size = self.lifecycle.viewport().logical_size();
                match self.computer_hud.act(&self.atlas, size, &key) {
                    Some(true) => {
                        self.close_computer();
                        Ok(())
                    }
                    Some(false) => Ok(()),
                    None => Err("That control isn't on the computer's screen".into()),
                }
            }
            Request::ComputerHudScroll { delta } => {
                let size = self.lifecycle.viewport().logical_size();
                self.computer_hud.scroll_by(&self.atlas, size, delta);
                Ok(())
            }
            Request::ComputerKeyboard { bottom } => self.computer_hud.set_keyboard(bottom),
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
            Request::InteractResults => {
                let results = self.world.results(self.aspect());
                let size = self.lifecycle.viewport().logical_size();
                if !self.lifecycle.active()
                    || !self.results_hit(results.screen_x * size[0], results.screen_y * size[1])
                {
                    return Err(
                        "Walk inside the Gym and approach its RESULTS board to open it".into(),
                    );
                }
                self.open_results();
                Ok(())
            }
            Request::CloseResults => {
                self.reset_motion();
                self.results_open = false;
                Ok(())
            }
            Request::ResultsView => Ok(()),
            Request::Results { command } => {
                self.require_results_panel()?;
                self.results.act(command)
            }
            Request::InteractEvals => {
                let evals = self.world.evals(self.aspect());
                let size = self.lifecycle.viewport().logical_size();
                if !self.lifecycle.active()
                    || !self.evals_hit(evals.screen_x * size[0], evals.screen_y * size[1])
                {
                    return Err(
                        "Walk inside the Gym and approach its EVALS board to open it".into(),
                    );
                }
                self.open_evals();
                Ok(())
            }
            Request::CloseEvals => {
                self.reset_motion();
                self.evals_open = false;
                Ok(())
            }
            Request::EvalsView => Ok(()),
            Request::Evals { command } => {
                self.require_evals_panel()?;
                let hall = self.hall.as_mut().ok_or("The Gym is offline")?;
                hall.act(&command);
                self.gym_notes = hall.opted_in();
                Ok(())
            }
            Request::GoEvals => self.go_evals(),
            Request::StudioView => Ok(()),
            Request::StudioActivate {
                instance,
                revision,
                node,
            } => self.studio_activate(&rust_native::Activation {
                instance,
                revision,
                node,
            }),
            Request::CloseStudio => {
                self.close_studio();
                Ok(())
            }
            Request::StudioText { text } => {
                let open = self.studio.as_ref().ok_or("Open a studio station first")?;
                crate::studio_panel::typed(&mut self.world, open, &text)
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
        packet.ball = self.world.ball().map(|ball| {
            let body = ball.body();
            BallPacket {
                position: body.pos.as_vec3().to_array(),
                speed: body.vel.length() as f32,
                asleep: body.sleeping,
                step_ms: ball.step_time.as_secs_f32() * 1_000.0,
                awake: ball.world().stats.awake,
            }
        });
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
        if self.world.zone_loading() {
            packet.connection.state = "local_zone";
            packet.connection.label = "Loading zone";
        } else if !self.world.is_plaza() {
            packet.connection.label = self.world.zone_label();
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
        packet.camera_first_person = self.world.first_person();
        packet.stick_pointer = self
            .touches
            .iter()
            .find(|(_, touch)| touch.movement && touch.target.is_none())
            .map(|(id, _)| *id);
        packet.look_stick_pointer = self.look_stick_touch().map(|(id, _)| id);
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
        packet.computer_page = self.computer_hud.page();
        packet.computer_hud = self.computer_hud.snapshot(
            &self.atlas,
            self.lifecycle.viewport().logical_size(),
            self.computer_open && self.lifecycle.active(),
        );
        packet.gym = self.gym().into();
        packet.gym_open = self.gym_open;
        packet.gym_revision = self.gym_board.revision();
        packet.gym_active = self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.gym().inside;
        packet.results = self.world.results(self.aspect()).into();
        packet.results_open = self.results_open;
        packet.results_revision = self.results.revision();
        packet.results_active = self.results_panel && packet.gym_active;
        packet.evals = self.world.evals(self.aspect()).into();
        packet.evals_open = self.evals_open;
        // Both only grow, so their sum changes whenever either screen does.
        packet.evals_revision = self
            .hall
            .as_ref()
            .map_or(0, verse::gym_hall::Hall::revision)
            + self.league.revision();
        packet.evals_active = self.evals_panel && self.hall.is_some() && packet.gym_active;
        packet.gym_notes = self.gym_notes;
        packet.studio_open = self.studio.is_some();
        packet.studio_revision = self
            .studio
            .as_ref()
            .map_or(0, |open| open.view.view().revision);
        packet
    }

    /// The open studio panel's view, for the host to mount.
    pub fn studio_view(&self) -> Option<View<crate::studio_panel::Intent>> {
        self.studio.as_ref().map(|open| open.view.view().clone())
    }

    /// Stands the player at Everglade's station `id`, for scripted checks.
    fn go_station(&mut self, id: &str) -> Result<(), String> {
        if self.world.zone != verse::zones::ZoneId::Everglade || self.world.zone_loading() {
            return Err("Enter Everglade first".into());
        }
        let at = verse::zones::everglade::STATIONS
            .iter()
            .find(|station| station.id == id)
            .ok_or("Everglade has no such station")?
            .at;
        self.world.cancel_navigation();
        self.reset_motion();
        self.world.place_player([at[0], 0.0, at[1]].into(), 0.0)
    }

    /// Opens the panel of the Everglade station in reach. Called only after
    /// the shared runtime admitted the Interact intent where the player
    /// stands.
    fn open_studio(&mut self) -> Result<(), String> {
        let kind = self
            .world
            .studio_panel_here()
            .ok_or("Walk up to a station")?;
        let open = self.studio_panel(kind)?;
        self.reset_motion();
        self.world.cancel_navigation();
        self.map.clear_contacts();
        self.door_hud.clear_contacts();
        self.zone_hud.clear_contacts();
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
        self.studio = Some(open);
        Ok(())
    }

    /// The panel of `kind` built from the studio as it is now, under the
    /// next view revision.
    fn studio_panel(
        &mut self,
        kind: verse::zones::everglade::studio::PanelKind,
    ) -> Result<crate::studio_panel::Open, String> {
        let revision = self.studio_revisions + 1;
        let open = crate::studio_panel::open(
            &mut self.world,
            kind,
            &format!("{}.studio", self.lifecycle.id()),
            revision,
        )
        .map_err(|error| error.to_string())?;
        self.studio_revisions = revision;
        Ok(open)
    }

    /// Closes the studio panel outside Everglade, and rebuilds it when the
    /// studio changed.
    fn sync_studio(&mut self) -> Result<(), String> {
        let Some(open) = &self.studio else {
            return Ok(());
        };
        if self.world.zone != verse::zones::ZoneId::Everglade || self.world.zone_loading() {
            self.close_studio();
            return Ok(());
        }
        if crate::studio_panel::current(open, &mut self.world) {
            return Ok(());
        }
        let kind = open.kind.clone();
        self.studio = Some(self.studio_panel(kind)?);
        Ok(())
    }

    fn studio_activate(&mut self, event: &rust_native::Activation) -> Result<(), String> {
        let open = self.studio.as_ref().ok_or("Open a studio station first")?;
        match crate::studio_panel::activate(open, event)? {
            crate::studio_panel::Intent::Close => {
                self.close_studio();
                Ok(())
            }
            intent => crate::studio_panel::act(&mut self.world, &intent),
        }
    }

    fn close_studio(&mut self) {
        if self.studio.take().is_some() {
            self.reset_motion();
        }
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
        let eva = self.world.eva_map_status();
        let state = self.map_error.as_deref().unwrap_or(match eva {
            Some((status, _)) => status,
            None => match self.world.navigation().status() {
                Status::Idle => "Choose a place to walk",
                Status::Walking => "Walking",
                Status::Arrived => "Arrived",
                Status::Cancelled => "Walk stopped",
                Status::Blocked => "Route blocked",
            },
        });
        self.map.snapshot_for_zone(
            self.lifecycle.viewport().logical_size(),
            [self.world.player.pos.x, self.world.player.pos.z],
            self.lifecycle.active() && !self.panel_open() && !self.spawn_pending,
            state,
            eva.map_or_else(
                || self.world.navigation().destination(),
                |(_, target)| target,
            ),
            self.world.zone,
        )
    }

    pub(crate) fn prepare_terminal_glyphs(&mut self) {
        let size = self.lifecycle.viewport().logical_size();
        if self.computer_open {
            self.computer_hud.prepare_glyphs(&mut self.atlas, size);
        }
    }

    pub fn map_ui(&self) -> verse::ui::UiBatch {
        if self.chamber.is_some() {
            let mut ui = self.chamber_ui();
            ui.vertices.extend(self.stick_ui().vertices);
            return ui;
        }
        if self.world.is_bare() {
            // Loading controls take the Grid's neutral palette; Everglade's
            // hotbar is the chamber's icon tray. Both stand above the sticks.
            let mut ui = self.player_tags();
            let scale = self.lifecycle.viewport().scale();
            if let Some(layout) = self.atlas.layout_at_scale(scale) {
                if self.bare_zone_panel() {
                    let mut zone_ui = self
                        .zone_hud
                        .draw(&layout, &self.zone_hud_snapshot(), scale);
                    zone_ui.neutralize();
                    ui.vertices.extend(zone_ui.vertices);
                }
                if self.everglade_hotbar_shown()
                    && let Some(slots) = self.world.everglade_hotbar()
                {
                    // Laid out in logical points; this batch is in pixels.
                    let mut bar = verse::ui::UiBatch::default();
                    verse::zones::everglade::hotbar::draw(
                        &mut bar,
                        &layout,
                        self.lifecycle.viewport().logical_size(),
                        self.hotbar_bottom(),
                        &slots,
                    );
                    if let Some(index) = self.held_slot_tip() {
                        verse::zones::everglade::hotbar::draw_tip(
                            &mut bar,
                            &layout,
                            self.lifecycle.viewport().logical_size(),
                            self.hotbar_bottom(),
                            slots.len(),
                            index,
                        );
                    }
                    // The breath bar over the tray under water.
                    if let Some(breath) = self.world.everglade_breath() {
                        verse::zones::everglade::water::draw_breath(
                            &mut bar,
                            &layout,
                            self.lifecycle.viewport().logical_size(),
                            self.hotbar_bottom(),
                            &breath,
                        );
                    }
                    // Meteor Swarm's help and cast bar over the tray.
                    if let Some(swarm) = self.world.everglade_swarm() {
                        verse::zones::everglade::demolition::hotbar::draw_town(
                            &mut bar,
                            &layout,
                            self.lifecycle.viewport().logical_size(),
                            self.hotbar_bottom(),
                            slots.len(),
                            &swarm,
                        );
                    }
                    // Everglade's Leave button, where the chamber's stands:
                    // the zone has no arch back to the Grid.
                    let [lx, ly, lw, lh] = self.chamber_leave_rect();
                    bar.rect(&layout, lx, ly, lw, lh, [0.1, 0.1, 0.12, 0.8]);
                    bar.text(&layout, lx + 12.0, ly + 9.0, "LEAVE", [1.0; 4]);
                    for vertex in &mut bar.vertices {
                        vertex.pos = vertex.pos.map(|v| v * scale);
                    }
                    ui.vertices.extend(bar.vertices);
                }
            }
            ui.vertices.extend(self.stick_ui().vertices);
            return ui;
        }
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
        ui.vertices.extend(self.stick_ui().vertices);
        if self.computer_open && self.computer_hud.drawn() {
            let size = self.lifecycle.viewport().logical_size();
            let computer = self.computer();
            let anchor = computer
                .visible
                .then(|| [computer.screen_x * size[0], computer.screen_y * size[1]]);
            let computer_ui = self.computer_hud.draw(
                &self.atlas,
                size,
                self.lifecycle.viewport().scale(),
                anchor,
            );
            ui.vertices.extend(computer_ui.vertices);
        }
        ui
    }

    fn zone_hud_snapshot(&self) -> verse::zones::hud::Snapshot {
        self.zone_hud.snapshot(
            self.lifecycle.viewport().logical_size(),
            &self.zone_snapshot(),
            self.lifecycle.active()
                && !self.computer_open
                && !self.gym_open
                && !self.results_open
                && !self.evals_open
                && self.studio.is_none()
                && !self.map.expanded,
        )
    }

    /// Whether the bare world draws the zone panel: only while a zone loads
    /// or failed to load (with Cancel, or Retry and Dismiss). Inside a zone
    /// it draws none (owner, 2026-10-04); Everglade draws its hotbar.
    fn bare_zone_panel(&self) -> bool {
        self.world.is_bare() && self.world.zone_load_state() != verse::zones::LoadState::Idle
    }

    /// The hotbar slot a touch has held past a long press, whose card shows.
    fn held_slot_tip(&self) -> Option<usize> {
        let (_, index, _, at) = self.slot_touch?;
        let held = self.pointer_clock.elapsed().as_secs_f64() - at;
        verse::tooltip::long_press(held as f32).then_some(index)
    }

    /// Whether the bare world draws Everglade's movement hotbar.
    fn everglade_hotbar_shown(&self) -> bool {
        self.world.is_bare()
            && self.world.zone == verse::zones::ZoneId::Everglade
            && self.world.zone_load_state() == verse::zones::LoadState::Idle
    }

    /// The hotbar's distance above the screen's bottom edge: above the sticks.
    /// The player is in the shared chamber (connecting, joined, or failed).
    pub(crate) fn in_chamber(&self) -> bool {
        self.chamber.is_some()
    }

    /// The chamber's engine frame for a viewport of `size` pixels, while
    /// the session is joined.
    pub(crate) fn chamber_frame(
        &self,
        size: [u32; 2],
    ) -> Result<Option<verse::imported::chamber_session::Frame>, String> {
        match &self.chamber {
            Some(play) => play.frame_in(size, self.lifecycle.viewport().logical_size()),
            None => Ok(None),
        }
    }

    /// Steps the chamber one frame: the sticks steer and turn the shared
    /// character; a failed connection returns the player to the Grid.
    fn step_chamber(&mut self, camera_dt: f32, input: Option<InputState>) -> Result<(), String> {
        let input = if self.panel_open() {
            InputState::default()
        } else {
            input.unwrap_or_else(|| self.input())
        };
        let held = verse::imported::chamber_session::Held {
            forward: input.forward,
            backward: input.backward,
            strafe_left: input.strafe_left,
            strafe_right: input.strafe_right,
            turn_left: false,
            turn_right: false,
        };
        let look = self.chamber_look(camera_dt);
        let Some(play) = &mut self.chamber else {
            return Ok(());
        };
        if !self.lifecycle.active() {
            return Ok(());
        }
        let joined = play.step(held);
        let scene = play.content.as_ref().map(|c| c.scene.clone());
        if joined && let Some(session) = play.session_mut() {
            if look != [0.0, 0.0] {
                session.camera.yaw -= look[0];
                session.camera.pitch = (session.camera.pitch + look[1]).clamp(-1.2, 1.2);
                session.yaw = session.camera.yaw;
            }
            if input.jump
                && let Some(scene) = &scene
            {
                session.jump(scene);
            }
            play.frames = play.frames.saturating_add(1);
        }
        if let Some(message) = play.failed() {
            let message = message.to_owned();
            self.leave_chamber()?;
            self.error = Some(message);
        }
        Ok(())
    }

    /// The look stick's turn for this frame, in radians: yaw then pitch.
    fn chamber_look(&mut self, dt: f32) -> [f32; 2] {
        let held = self
            .look_stick_touch()
            .map(|(_, touch)| touch.latest)
            .filter(|_| self.lifecycle.active() && !self.panel_open());
        let Some(point) = held else {
            self.look_rate = [0.0, 0.0];
            return [0.0, 0.0];
        };
        let target = self.look_stick_target(point);
        let dt = if dt.is_finite() {
            dt.clamp(0.0, 0.1)
        } else {
            0.0
        };
        let k = 1.0 - (-dt / LOOK_STICK_SMOOTHING_SECONDS).exp();
        for (rate, target) in self.look_rate.iter_mut().zip(target) {
            *rate += (target - *rate) * k;
        }
        [self.look_rate[0] * dt, self.look_rate[1] * dt]
    }

    fn chamber_cast(&mut self, slot: usize) {
        let Some(play) = &mut self.chamber else {
            return;
        };
        let Some(ability) = CHAMBER_SLOTS.get(slot).copied() else {
            return;
        };
        let Some(scene) = play.content.as_ref().map(|c| c.scene.clone()) else {
            return;
        };
        if let Some(session) = play.session_mut() {
            if session.view().target().is_none() {
                session.target_nearest();
            }
            session.cast(&scene, ability);
        }
    }

    /// Leaves the chamber: stops its worker and stands the player in front
    /// of the RITUAL arch, as closing the desktop window does.
    fn leave_chamber(&mut self) -> Result<(), String> {
        let Some(mut play) = self.chamber.take() else {
            return Ok(());
        };
        play.suspend();
        self.reset_zone_inputs();
        self.world.return_from_ritual()?;
        self.sync_zone_session()
    }

    /// The chamber hotbar's slots, laid out above the right stick in
    /// logical points: `(x, y, size)` for each slot.
    fn chamber_slots(&self) -> Vec<[f32; 3]> {
        let size = self.lifecycle.viewport().logical_size();
        let slot = 52.0;
        let gap = 8.0;
        let bottom = self.hotbar_bottom() + 16.0;
        let total = CHAMBER_SLOTS.len() as f32 * slot + (CHAMBER_SLOTS.len() as f32 - 1.0) * gap;
        let x0 = (size[0] - total) / 2.0;
        (0..CHAMBER_SLOTS.len())
            .map(|i| [x0 + i as f32 * (slot + gap), size[1] - bottom - slot, slot])
            .collect()
    }

    /// Whether `point` (logical points) is on Everglade's Leave button,
    /// which stands where the chamber's does.
    fn on_everglade_leave(&self, point: [f32; 2]) -> bool {
        let [x, y, w, h] = self.chamber_leave_rect();
        point[0] >= x && point[0] <= x + w && point[1] >= y && point[1] <= y + h
    }

    /// The Leave button's rectangle, top right under the insets.
    fn chamber_leave_rect(&self) -> [f32; 4] {
        let size = self.lifecycle.viewport().logical_size();
        [
            size[0] - self.insets[1] - 96.0,
            self.insets[0] + 12.0,
            84.0,
            36.0,
        ]
    }

    fn chamber_hud_hit(&self, point: [f32; 2]) -> Option<ChamberHit> {
        let inside = |r: [f32; 4]| {
            point[0] >= r[0]
                && point[0] <= r[0] + r[2]
                && point[1] >= r[1]
                && point[1] <= r[1] + r[3]
        };
        if inside(self.chamber_leave_rect()) {
            return Some(ChamberHit::Leave);
        }
        let dead = self
            .chamber
            .as_ref()
            .and_then(|c| c.session())
            .is_some_and(|s| s.dead());
        if dead {
            let size = self.lifecycle.viewport().logical_size();
            if inside([size[0] / 2.0 - 70.0, size[1] / 2.0 - 20.0, 140.0, 40.0]) {
                return Some(ChamberHit::Respawn);
            }
        }
        for (i, [x, y, w]) in self.chamber_slots().into_iter().enumerate() {
            if inside([x, y, w, w]) {
                return Some(ChamberHit::Slot(i));
            }
        }
        None
    }

    /// The chamber's phone HUD in pixels: the hotbar, Leave, the
    /// connection state, and Respawn after death. The authority's own HUD
    /// (health, target, quests) comes with the engine frame.
    fn chamber_ui(&self) -> verse::ui::UiBatch {
        let mut ui = verse::ui::UiBatch::default();
        let Some(play) = &self.chamber else {
            return ui;
        };
        let scale = self.lifecycle.viewport().scale();
        let atlas = &self.atlas;
        let px = |v: f32| v * scale;
        let [lx, ly, lw, lh] = self.chamber_leave_rect();
        ui.rect(atlas, px(lx), px(ly), px(lw), px(lh), [0.1, 0.1, 0.12, 0.8]);
        ui.text(atlas, px(lx + 12.0), px(ly + 9.0), "LEAVE", [1.0; 4]);
        let size = self.lifecycle.viewport().logical_size();
        if !play.joined() {
            let message = play
                .failed()
                .map_or("Entering the chamber", |_| "The chamber refused");
            ui.text(
                atlas,
                px(size[0] / 2.0 - 80.0),
                px(size[1] / 2.0),
                message,
                [1.0; 4],
            );
            return ui;
        }
        for (i, [x, y, w]) in self.chamber_slots().into_iter().enumerate() {
            ui.rect(atlas, px(x), px(y), px(w), px(w), [0.1, 0.1, 0.12, 0.7]);
            ui.text(
                atlas,
                px(x + 6.0),
                px(y + 4.0),
                &format!("{}", i + 1),
                [1.0, 0.9, 0.6, 1.0],
            );
            ui.text(
                atlas,
                px(x + 6.0),
                px(y + w - 18.0),
                CHAMBER_SLOTS[i].label(),
                [0.9, 0.9, 0.9, 1.0],
            );
        }
        if play.session().is_some_and(|s| s.dead()) {
            ui.rect(
                atlas,
                px(size[0] / 2.0 - 70.0),
                px(size[1] / 2.0 - 20.0),
                px(140.0),
                px(40.0),
                [0.3, 0.05, 0.05, 0.85],
            );
            ui.text(
                atlas,
                px(size[0] / 2.0 - 36.0),
                px(size[1] / 2.0 - 8.0),
                "RESPAWN",
                [1.0; 4],
            );
        }
        ui
    }

    fn hotbar_bottom(&self) -> f32 {
        self.insets[2] + STICK_MARGIN_POINTS + 2.0 * STICK_RADIUS_POINTS
    }

    fn plaza_online_allowed(&self) -> bool {
        self.world.is_plaza() && !self.world.zone_loading()
    }

    fn zone_snapshot(&self) -> verse::zones::Snapshot {
        let mut snapshot = self.world.zone_snapshot(self.aspect());
        if self.world.is_bare() {
            // The OpenAgents app offers a station's studio panel only once
            // its host connected the studio to a paired computer
            // (`connect_studio`); the return arch replaces Return.
            let studio = self.world.studio().has_source();
            snapshot.controls.retain(|control| match control.action {
                ZoneIntent::Return => false,
                ZoneIntent::Interact => studio,
                _ => true,
            });
        }
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
        self.presented_entities = Box::new(verse::mesh::Mesh::default());
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
        self.computer_open = false;
        self.gym_open = false;
        self.results_open = false;
        self.evals_open = false;
        self.studio = None;
        self.reset_motion();
        self.restore_spawn = false;
        self.spawn_pending = false;
        self.sync_gym_interest();
    }

    /// The NIP-MV world presence joins where the player stands: the Grid's
    /// or the plaza's world, or the zone's own shared world. `None` while
    /// a zone loads, when nobody is anywhere yet.
    fn presence_world(&self) -> Option<&'static str> {
        if self.world.zone_loading() {
            None
        } else if !self.world.is_plaza() {
            Some(self.world.zone.world_id())
        } else if self.world.is_bare() {
            Some(verse::session::BARE_WORLD)
        } else {
            Some(verse::session::WORLD)
        }
    }

    fn sync_zone_session(&mut self) -> Result<(), String> {
        let wanted = self.presence_world();
        if self
            .session
            .as_ref()
            .is_some_and(|session| Some(session.world()) != wanted)
        {
            // Through an arch: the old world's presence ends before the new
            // world's pose ticks, so nobody sees a player in two places.
            self.session = None;
            self.presented_entities = Box::new(verse::mesh::Mesh::default());
        }
        if wanted.is_none() {
            self.spawn_pending = false;
            self.restore_spawn = false;
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
        if !self.lifecycle.active()
            || self.computer_open
            || self.gym_open
            || self.results_open
            || self.evals_open
            || self.studio.is_some()
            || self.map.expanded
        {
            return Err("Return to the world to use the portal".into());
        }
        if intent == ZoneIntent::Interact
            && self.world.is_bare()
            && !self.world.studio().has_source()
        {
            return Err("Connect a computer to use the studio".into());
        }
        if intent == ZoneIntent::Enter {
            let portal = self.world.zone_snapshot(self.aspect()).portal;
            let size = self.lifecycle.viewport().logical_size();
            let at = point.unwrap_or([portal.screen_x * size[0], portal.screen_y * size[1]]);
            if !self.portal_hit(at[0], at[1]) {
                return Err("Approach a visible portal to enter its zone".into());
            }
        }
        self.world.zone_intent(intent)?;
        if intent == ZoneIntent::Interact {
            // The runtime admitted a station in reach; the panel is ours.
            return self.open_studio();
        }
        if matches!(
            intent,
            ZoneIntent::Enter | ZoneIntent::Return | ZoneIntent::Cancel | ZoneIntent::Retry
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
        self.computer_open
            || self.gym_open
            || self.results_open
            || self.evals_open
            || self.studio.is_some()
            || self.world.zone_loading()
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
        let inside = self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.gym().inside;
        self.gym_board.set_active(inside);
        // Entering the Gym starts the results load; leaving cancels it. It
        // needs no Gym connection.
        self.results.set_active(inside && self.results_panel);
        // The EVALS board reads, and the agent may speak, only while the
        // player stands in the Gym.
        if let Some(hall) = &mut self.hall {
            hall.set_active(inside && self.evals_panel);
        }
        self.league.set_active(inside && self.evals_panel);
    }

    fn require_evals_panel(&self) -> Result<(), String> {
        if self.evals_panel
            && self.hall.is_some()
            && self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.evals_open
            && self.gym().inside
        {
            Ok(())
        } else {
            Err("Open the EVALS board while inside to use it".into())
        }
    }

    /// The EVALS panel's current screen, while it is open.
    pub fn evals_view(&self) -> Option<verse::gym_hall::View> {
        self.require_evals_panel()
            .ok()
            .and(self.hall.as_ref())
            .map(verse::gym_hall::Hall::view)
    }

    /// The pylon league section of the open EVALS panel.
    pub fn league_view(&self) -> Option<verse::gym_league::View> {
        self.require_evals_panel().ok().map(|()| self.league.view())
    }

    /// Reads the league from `relay`, counting `checkers`' verdicts, for
    /// the rest of this mount.
    pub(crate) fn pin_league(
        &mut self,
        relay: String,
        checkers: std::collections::BTreeSet<String>,
    ) {
        self.league = verse::gym_league::Reader::new(Some(relay), checkers);
        self.league_pinned = true;
        self.sync_gym_interest();
    }

    /// Called only after pointer or accessibility picking validates the
    /// board, or after [`Self::go_evals`] placed the player before it.
    fn open_evals(&mut self) {
        self.open_gym();
        self.gym_open = false;
        self.evals_open = true;
    }

    /// Walks the player into the Grid's Gym before the EVALS board and
    /// opens it.
    fn go_evals(&mut self) -> Result<(), String> {
        if !self.evals_panel || !self.world.is_bare() {
            return Err("This world has no EVALS board".into());
        }
        let site = self
            .world
            .gym_site()
            .filter(|_| self.plaza_online_allowed())
            .ok_or("Return to the Grid to visit the Gym")?;
        if !self.lifecycle.active() {
            return Err("Open the Verse to visit the Gym".into());
        }
        if self.computer_open {
            self.close_computer();
        }
        self.spawn_pending = false;
        self.restore_spawn = false;
        self.world.cancel_navigation();
        self.world.set_spawn(
            site.point(verse::world::GYM_EVALS_STAND),
            site.yaw_of(std::f32::consts::FRAC_PI_2),
        )?;
        self.reset_motion();
        self.sync_gym_interest();
        self.open_evals();
        Ok(())
    }

    /// Passes the hall who else stands in the Gym and each trainer's eval
    /// credit, and takes what its reader derived.
    fn poll_hall(&mut self, now: Instant) {
        let Some(hall) = &mut self.hall else {
            return;
        };
        if hall.active() {
            let peers = match (&self.session, self.world.gym_site()) {
                (Some(session), Some(site)) => {
                    verse::gym_hall::peers_inside(site, &session.crowd.shown(now))
                }
                _ => std::collections::BTreeSet::new(),
            };
            hall.set_peers(peers);
            if let Some(snapshot) = &self.xp_snapshot {
                let stamp = snapshot.credits.len();
                if self.eval_credit_from != Some(stamp) {
                    self.eval_credit_from = Some(stamp);
                    hall.set_credit(verse::gym_evals::eval_credit(
                        snapshot
                            .credits
                            .iter()
                            .map(|c| (c.rule.as_str(), c.pubkey.as_str(), c.xp)),
                    ));
                }
            }
        }
        hall.poll();
    }

    fn require_results_panel(&self) -> Result<(), String> {
        if self.results_panel
            && self.lifecycle.active()
            && self.plaza_online_allowed()
            && !self.spawn_pending
            && self.results_open
            && self.gym().inside
        {
            Ok(())
        } else {
            Err("Open the RESULTS board while inside to use it".into())
        }
    }

    /// The results panel's current screen, while it is open.
    pub fn results_view(&self) -> Option<verse::gym_results::ResultsView> {
        self.require_results_panel()
            .ok()
            .map(|()| self.results.view())
    }

    /// Called only after pointer or accessibility picking validates the board.
    fn open_results(&mut self) {
        self.open_gym();
        self.gym_open = false;
        self.results_open = true;
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
        self.computer_hud.open();
        self.studio = None;
        self.gym_open = false;
        self.results_open = false;
        self.evals_open = false;
        self.touches.clear();
        self.jump = false;
        self.sprint = false;
    }

    fn close_computer(&mut self) {
        self.reset_motion();
        self.computer_open = false;
        self.computer_hud.close();
    }

    /// What the native host must do for the computer's HUD, once each.
    pub(crate) fn take_computer_commands(&mut self) -> Vec<crate::computer_hud::Command> {
        if self.computer_open {
            let size = self.lifecycle.viewport().logical_size();
            self.computer_hud.sync_terminal(&self.atlas, size);
        }
        self.computer_hud.take_commands()
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
        self.results_open = false;
        self.evals_open = false;
        self.computer_open = false;
        self.studio = None;
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
        } else if self.results_hit(x, y) {
            Some(WorldTarget::Results)
        } else if self.evals_hit(x, y) {
            Some(WorldTarget::Evals)
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

    pub(crate) fn gym_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.gym_panel
            && self.world.is_plaza()
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

    pub(crate) fn results_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.results_panel
            && self.world.is_plaza()
            && !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.results_hit_with_entities(
                size[0] / size[1],
                x / size[0],
                y / size[1],
                &self.presented_entities,
            )
    }

    pub(crate) fn evals_hit(&self, x: f32, y: f32) -> bool {
        let size = self.lifecycle.viewport().logical_size();
        self.evals_panel
            && self.hall.is_some()
            && self.world.is_plaza()
            && !self.spawn_pending
            && size[0] > 0.0
            && size[1] > 0.0
            && self.world.evals_hit_with_entities(
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
#[path = "bare_bodies_tests.rs"]
mod bare_bodies_tests;
#[cfg(test)]
#[path = "bare_evals_tests.rs"]
mod bare_evals_tests;
#[cfg(test)]
#[path = "bare_gym_tests.rs"]
mod bare_gym_tests;
#[cfg(test)]
#[path = "bare_presence_tests.rs"]
pub(crate) mod bare_presence_tests;
#[cfg(test)]
#[path = "bare_results_tests.rs"]
mod bare_results_tests;
#[cfg(test)]
#[path = "studio_tests.rs"]
mod studio_tests;

#[cfg(test)]
mod tests {
    use super::*;

    fn scene() -> Box<Scene> {
        Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: false,
            world_relay: None,
            display_name: None,
            world_offline: false,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: true,
            hdr: false,
            bare: false,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        })
        .unwrap()
    }
    /// Where the plaza's arch to Everglade stands.
    fn everglade_arch() -> [f32; 3] {
        verse::zones::ZoneId::Plaza
            .portals()
            .into_iter()
            .find(|&(zone, _)| zone == verse::zones::ZoneId::Everglade)
            .unwrap()
            .1
            .to_array()
    }

    /// A plaza scene standing before Everglade's arch, with the committed,
    /// pinned Everglade pack in its zone cache.
    fn cached_zone_scene() -> (Box<Scene>, tempfile::TempDir) {
        use verse::zones::everglade_pack::{PACK_DIRECTORY, PACK_EXTENSION, PACK_SHA256};
        let cache = tempfile::tempdir().unwrap();
        let name = format!("{PACK_SHA256}.{PACK_EXTENSION}");
        let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PACK_DIRECTORY)
            .join(&name);
        std::fs::copy(source, cache.path().join(name)).unwrap();
        let mut scene = scene();
        scene.world.configure_zone_cache(cache.path().to_owned());
        scene.activate(true).unwrap();
        let [x, y, z] = everglade_arch();
        scene.world.set_spawn([x, y, z - 4.0].into(), 0.0).unwrap();
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
    fn zone_transition_clears_input_and_keeps_plaza_identity_out_of_the_zone() {
        let (mut scene, _cache) = cached_zone_scene();
        // A local socket keeps this an offline test while exercising a real
        // Session owner that must be dropped before any zone simulation.
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
        let deadline = Instant::now() + Duration::from_secs(180);
        let mut clock = 1.0;
        while scene.world.zone_loading() && Instant::now() < deadline {
            scene.update(clock).unwrap();
            clock += 0.02;
            std::thread::sleep(Duration::from_millis(2));
        }
        assert_eq!(
            scene.world.zone,
            verse::zones::ZoneId::Everglade,
            "{:?}",
            scene.world.zone_snapshot(scene.aspect()).error
        );
        // Everglade has its own shared presence world; the plaza's world
        // stays behind the arch.
        let session = scene
            .session
            .as_ref()
            .expect("an Everglade presence session");
        assert_eq!(session.world(), verse::zones::ZoneId::Everglade.world_id());
        assert_ne!(session.world(), verse::session::WORLD);
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
        let anchor = verse::zones::ZoneId::Plaza
            .portals()
            .into_iter()
            .find(|&(zone, _)| zone == verse::zones::ZoneId::Everglade)
            .unwrap()
            .1
            .with_y(2.5);
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

    fn gym_scene() -> Box<Scene> {
        Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: true,
            gym_code: None,
            synthetic_gym: true,
            world_relay: None,
            display_name: None,
            world_offline: false,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: true,
            hdr: false,
            bare: false,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        })
        .unwrap()
    }

    fn door_scene(id: DoorId) -> Box<Scene> {
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
            display_name: None,
            world_offline: false,
            door_preferences: Some(saved.clone()),
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: true,
            hdr: false,
            bare: false,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
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

    fn companion_scene() -> (Box<Scene>, [f32; 2]) {
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
        scene.pointer(1, PointerPhase::Down, 60.0, 530.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 60.0, 480.0).unwrap();
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
        scene.pointer(1, PointerPhase::Down, 80.0, 520.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 80.0, 420.0).unwrap();
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
        scene.pointer(1, PointerPhase::Down, 80.0, 520.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 80.0, 420.0).unwrap();
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
        scene.pointer(1, PointerPhase::Down, 80.0, 520.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 80.0, 420.0).unwrap();
        for frame in 1..=20 {
            scene.update(1.0 + frame as f64 / 30.0).unwrap();
        }
        scene.action(Request::Sprint { enabled: true }).unwrap();
        scene.action(Request::Jump).unwrap();
        scene.action(Request::InteractComputer).unwrap();
        assert!(scene.packet().computer_open);
        assert!(scene.touches.is_empty());
        let stopped = scene.world.player.pos;
        scene.pointer(2, PointerPhase::Down, 80.0, 520.0).unwrap();
        scene.pointer(2, PointerPhase::Move, 80.0, 420.0).unwrap();
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
    fn computer_scene() -> (Box<Scene>, [f32; 2]) {
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

    fn computers_feed() -> crate::computer_hud::Feed {
        serde_json::from_value(serde_json::json!({
            "computers": {
                "schema": "rust-native.view.v2", "instance": "computers-test", "revision": 3,
                "root": {"key": "root", "style": {}, "element": {"kind": "stack", "props": {
                    "axis": "vertical", "children": [
                        {"key": "computers-title", "style": {}, "element": {"kind": "text",
                            "props": {"value": "Computers", "role": "heading"}}},
                        {"key": "host-0-open", "style": {}, "element": {"kind": "button",
                            "props": {"label": "Open", "enabled": true, "intent": {"kind": "refresh"}}}}
                    ]}}}
            },
            "busy": false
        }))
        .unwrap()
    }

    #[test]
    fn the_open_computer_draws_computers_in_the_hud_and_owns_every_touch() {
        let (mut scene, [x, y]) = computer_scene();
        scene.pointer(1, PointerPhase::Down, x, y).unwrap();
        scene.pointer(1, PointerPhase::Up, x, y).unwrap();
        assert!(scene.computer_open);
        // Opening asks the native host for a fresh Computers view.
        assert_eq!(
            scene.take_computer_commands(),
            vec![crate::computer_hud::Command::Refresh]
        );
        scene
            .action(Request::ComputerFeed {
                feed: Box::new(computers_feed()),
            })
            .unwrap();
        let packet = scene.packet();
        assert!(packet.computer_hud.visible);
        assert!(!scene.map_ui().vertices.is_empty());
        let open = packet
            .computer_hud
            .items
            .iter()
            .find(|item| item.key == "host-0-open")
            .unwrap()
            .clone();
        // A drag on the left half does not walk while the computer is open.
        let position = scene.world.player.pos;
        scene.pointer(2, PointerPhase::Down, 40.0, 540.0).unwrap();
        scene.pointer(2, PointerPhase::Move, 40.0, 440.0).unwrap();
        scene.update(1.2).unwrap();
        scene.update(1.4).unwrap();
        scene.pointer(2, PointerPhase::Up, 40.0, 440.0).unwrap();
        assert_eq!(scene.world.player.pos, position);
        assert!(scene.touches.is_empty());
        // A tap on a laid-out control names the node of the current view.
        let [fx, fy, fw, fh] = open.frame;
        scene
            .pointer(3, PointerPhase::Down, fx + fw / 2.0, fy + fh / 2.0)
            .unwrap();
        scene
            .pointer(3, PointerPhase::Up, fx + fw / 2.0, fy + fh / 2.0)
            .unwrap();
        assert_eq!(
            scene.take_computer_commands(),
            vec![crate::computer_hud::Command::Activate {
                surface: crate::computer_hud::Surface::Computers,
                instance: "computers-test".into(),
                revision: 3,
                node: "host-0-open".into(),
            }]
        );
        // Chats stays reachable as the native page.
        scene
            .action(Request::ComputerHudTap {
                key: "hud-tab-chats".into(),
            })
            .unwrap();
        let packet = scene.packet();
        assert_eq!(packet.computer_page, crate::computer_hud::Page::Chats);
        assert!(!packet.computer_hud.visible);
        scene
            .action(Request::ComputerPage {
                page: crate::computer_hud::Page::Computers,
            })
            .unwrap();
        assert!(
            scene
                .action(Request::ComputerHudTap {
                    key: "missing".into()
                })
                .is_err()
        );
        scene
            .action(Request::ComputerHudTap {
                key: "hud-close".into(),
            })
            .unwrap();
        assert!(!scene.computer_open);
        assert!(!scene.packet().computer_hud.visible);
    }

    #[test]
    fn a_native_computer_panel_keeps_world_routing_unchanged() {
        let (mut scene, [x, y]) = computer_scene();
        scene.computer_hud = crate::computer_hud::ComputerHud::new(false);
        scene.pointer(1, PointerPhase::Down, x, y).unwrap();
        scene.pointer(1, PointerPhase::Up, x, y).unwrap();
        assert!(scene.computer_open);
        assert!(scene.take_computer_commands().is_empty());
        assert!(!scene.packet().computer_hud.visible);
        assert!(
            scene
                .action(Request::ComputerPage {
                    page: crate::computer_hud::Page::Computers
                })
                .is_err()
        );
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
            let pitch = scene.world.camera.pitch;
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
            } else {
                assert_ne!(
                    scene.world.camera.pitch, pitch,
                    "Monitor-started drag must look"
                );
            }
            assert_eq!(
                scene.world.player.pos, position,
                "A drag off the stick must not move"
            );
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
                3 => scene.pointer(2, PointerPhase::Down, 60.0, 560.0).unwrap(),
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
        scene.pointer(1, PointerPhase::Down, 70.0, 530.0).unwrap();
        scene.pointer(1, PointerPhase::Up, x, y).unwrap();
        assert!(!scene.computer_open, "a tap must start on the monitor");
    }

    #[test]
    fn the_bare_world_keeps_the_player_controls_and_nothing_else() {
        let config = |relay: Option<&str>, offline: bool| Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: false,
            gym_code: None,
            synthetic_gym: false,
            world_relay: relay.map(str::to_owned),
            display_name: None,
            world_offline: offline,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: false,
            hdr: false,
            bare: true,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        };
        // Online, it selects the public relay unless another is named.
        let online = Scene::new(config(None, false)).unwrap();
        assert_eq!(online.relay.as_deref(), Some(verse::session::PUBLIC_RELAY));
        let named = Scene::new(config(Some("wss://relay.example.com"), false)).unwrap();
        assert_eq!(named.relay.as_deref(), Some("wss://relay.example.com"));
        assert!(
            Scene::new(Config {
                computer_hud: true,
                ..config(None, true)
            })
            .is_err()
        );
        let mut scene = Scene::new(config(None, true)).unwrap();
        scene.activate(true).unwrap();
        assert!(scene.world.is_bare() && scene.relay.is_none() && scene.session.is_none());
        assert_eq!(scene.packet().connection.state, "offline");
        scene.update(1.0).unwrap();
        // The sticks are the only drawn controls, beside the player's own
        // tag; the map's corner looks instead.
        assert_eq!(
            scene.map_ui().vertices.len(),
            scene.stick_ui().vertices.len() + PLAYER_TAG_CHARS * 6
        );
        let yaw = scene.world.player.yaw;
        scene.pointer(1, PointerPhase::Down, 340.0, 60.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 300.0, 60.0).unwrap();
        scene.pointer(1, PointerPhase::Up, 300.0, 60.0).unwrap();
        assert_ne!(scene.world.player.yaw, yaw);
        assert!(!scene.map_snapshot().expanded);
        // The movement stick is a circle above the bottom-left corner, and
        // the look stick mirrors it at the bottom right.
        let size = scene.lifecycle.viewport().logical_size();
        let [sx, sy] = scene.stick_center();
        assert_eq!([sx, sy], [80.0, size[1] - 80.0]);
        assert_eq!(scene.look_stick_center(), Some([size[0] - 80.0, sy]));
        // The stick walks the player forward.
        let start = scene.world.player.pos;
        scene.pointer(2, PointerPhase::Down, sx, sy).unwrap();
        scene.pointer(2, PointerPhase::Move, sx, sy - 80.0).unwrap();
        for frame in 1..=30 {
            scene.update(1.0 + f64::from(frame) / 60.0).unwrap();
        }
        scene.pointer(2, PointerPhase::Up, sx, sy - 80.0).unwrap();
        assert!(scene.world.player.pos.distance(start) > 1.0);
        // The Grid keeps only the Gym: no ball (owner, 2026-10-01).
        assert!(scene.world.ball().is_none());
        let packet = serde_json::to_value(scene.packet()).unwrap();
        assert!(packet["ball"].is_null(), "{}", packet["ball"]);
        // A double tap on open ground jumps.
        for (id, at) in [(3, 10.0), (4, 10.1)] {
            scene
                .pointer_at(id, PointerPhase::Down, 200.0, 200.0, at)
                .unwrap();
            scene
                .pointer_at(id, PointerPhase::Up, 200.0, 200.0, at + 0.05)
                .unwrap();
        }
        assert!(scene.jump);
        // Standing at the plaza computer's place opens nothing.
        let mut desk = verse::world::COMPUTER;
        desk.z -= 2.0;
        scene.world.set_spawn(desk, 0.0).unwrap();
        assert!(scene.action(Request::InteractComputer).is_err());
        assert!(scene.world_target(200.0, 300.0).is_none());
    }

    /// An offline bare world, active and past its first frame.
    fn bare_config(relay: Option<String>) -> Config {
        Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: false,
            gym_code: None,
            synthetic_gym: false,
            world_offline: relay.is_none(),
            world_relay: relay,
            display_name: None,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: false,
            hdr: false,
            bare: true,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        }
    }

    fn bare_scene() -> Box<Scene> {
        let mut scene = Scene::new(Config {
            secret_hex: "11".repeat(32),
            width: 800,
            height: 1200,
            scale: 2.0,
            synthetic: false,
            gym_code: None,
            synthetic_gym: false,
            world_relay: None,
            display_name: None,
            world_offline: true,
            door_preferences: None,
            zone_cache_directory: None,
            results_base: None,
            results_cache_directory: None,
            computer_hud: false,
            hdr: false,
            bare: true,
            xp_preview: false,
            gym_notes: false,
            ritual: None,
        })
        .unwrap();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        scene
    }

    #[test]
    fn the_ritual_arch_opens_the_chamber_and_a_refused_connection_returns_to_the_grid() {
        let mut scene = Scene::new(Config {
            ritual: Some("/nonexistent/ritual.json".into()),
            ..bare_config(None)
        })
        .unwrap();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        let gate = scene.world.ritual_gate().expect("the RITUAL arch");
        let (front, away) = gate.front();
        scene
            .world
            .place_player(front, away + std::f32::consts::PI)
            .unwrap();
        let forward = InputState {
            forward: true,
            ..InputState::default()
        };
        let mut t = 1.0;
        let mut entered = false;
        for _ in 0..240 {
            t += 1.0 / 60.0;
            scene.update_with_input(t, Some(forward.clone())).unwrap();
            if scene.in_chamber() {
                entered = true;
                break;
            }
        }
        assert!(entered, "walking through the arch opens the chamber");
        // The Grid's presence rests while the chamber is open, and the HUD
        // says the chamber is being entered.
        assert!(scene.session.is_none());
        assert!(scene.chamber_frame([800, 1200]).unwrap().is_none());
        assert!(!scene.map_ui().vertices.is_empty());
        // The connection fails on its thread; the next frames return the
        // player to the Grid in front of the arch with the reason shown.
        let mut returned = false;
        for _ in 0..300 {
            t += 1.0 / 60.0;
            scene.update_with_input(t, None).unwrap();
            if !scene.in_chamber() {
                returned = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(returned, "a refused chamber returns the player");
        assert!(scene.error.is_some());
        assert!(scene.world.is_plaza());
        assert!(scene.world.player.pos.distance(front) < 0.5);
    }

    #[test]
    fn a_player_tag_is_the_pubkey_prefix_and_its_level_when_it_has_xp() {
        // No ledger yet: the first eight characters of the pubkey.
        let plain = bare_scene();
        let key = plain.public_key.clone();
        assert_eq!(
            verse::xp::name_tag(plain.xp_snapshot.as_ref(), &key, None),
            key[..8]
        );
        let plain_vertices = plain.player_tags().vertices.len();
        assert_eq!(plain_vertices, PLAYER_TAG_CHARS * 6);

        // The labeled preview credits this player six tutorial
        // reproductions of 50 XP: level 3 under trainer-curve-v1.
        let mut preview = Scene::new(Config {
            xp_preview: true,
            ..bare_config(None)
        })
        .unwrap();
        preview.activate(true).unwrap();
        preview.update(1.0).unwrap();
        assert!(preview.session.is_none() && preview.xp.is_none());
        let key = preview.public_key.clone();
        let snapshot = preview.xp_snapshot.as_ref().unwrap();
        assert_eq!(snapshot.xp_of(std::slice::from_ref(&key)), 300);
        assert_eq!(
            verse::xp::name_tag(Some(snapshot), &key, None),
            format!("{} · lv 3", &key[..8])
        );
        assert!(preview.player_tags().vertices.len() > plain_vertices);
    }

    #[test]
    #[ignore = "the Grid's ball is off (owner, 2026-10-01); this exercises the ball"]
    fn playtest_titles_draw_as_shapes_on_the_tag_the_ground_and_the_ball() {
        // No playtest referee key exists yet, so a live scene reads none.
        assert!(verse::xp::playtest_trust().is_none());
        let mut scene = Scene::new(Config {
            xp_preview: true,
            ..bare_config(None)
        })
        .unwrap();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        assert!(scene.playtest.is_none());
        let key = scene.public_key.clone();
        let titles = verse::xp::playtest_titles(scene.playtest_snapshot.as_ref(), &key);
        assert_eq!(
            titles.iter().map(String::as_str).collect::<Vec<_>>(),
            [
                "bug-hunter",
                "fix-verifier",
                "founding-playtester",
                "playtester",
                "raider"
            ]
        );
        // Playtest XP never reaches the level on the tag.
        assert_eq!(
            verse::xp::name_tag(scene.xp_snapshot.as_ref(), &key, None),
            format!("{} · lv 3", &key[..8])
        );
        let marked = scene.player_tags().vertices;
        let snapshot = scene.playtest_snapshot.take();
        let plain = scene.player_tags().vertices;
        // PLAYTESTER under the tag, a crosshair, a check, and a ground ring.
        assert!(marked.len() >= plain.len() + "PLAYTESTER".len() * 6 + GROUND_RING_SEGMENTS * 6);
        // Shapes, not colors: every mark is gray.
        assert!(marked[plain.len()..].iter().all(|v| {
            let [r, g, b, _] = v.color;
            r == g && g == b
        }));
        scene.playtest_snapshot = snapshot;
        // A player without titles gets none.
        assert!(
            verse::xp::playtest_titles(scene.playtest_snapshot.as_ref(), &"ab".repeat(32))
                .is_empty()
        );

        // Walking into the ball as a raider makes it glow while it moves.
        let [sx, sy] = scene.stick_center();
        scene.pointer(2, PointerPhase::Down, sx, sy).unwrap();
        scene.pointer(2, PointerPhase::Move, sx, sy - 80.0).unwrap();
        let mut glowed = false;
        for frame in 1..=210 {
            scene.update(1.0 + f64::from(frame) / 60.0).unwrap();
            let mut ui = verse::ui::UiBatch::default();
            scene.raider_glow(&mut ui);
            glowed |= !ui.vertices.is_empty();
        }
        scene.pointer(2, PointerPhase::Up, sx, sy - 80.0).unwrap();
        assert!(glowed);
        // Without the title, pushing the ball draws nothing extra.
        scene.playtest_snapshot = None;
        let mut ui = verse::ui::UiBatch::default();
        scene.raider_glow(&mut ui);
        assert!(ui.vertices.is_empty());
    }

    #[test]
    fn pinching_all_the_way_in_on_the_grid_enters_first_person() {
        let mut grid = bare_scene();
        for _ in 0..40 {
            grid.action(Request::PinchZoom { scale: 1.1 }).unwrap();
        }
        let packet = serde_json::to_value(grid.packet()).unwrap();
        assert_eq!(packet["camera_first_person"], true);
        // In first person the player's own tag would sit in the camera.
        grid.activate(true).unwrap();
        assert!(grid.player_tags().vertices.is_empty());
        for _ in 0..3 {
            grid.action(Request::PinchZoom { scale: 0.9 }).unwrap();
        }
        let packet = serde_json::to_value(grid.packet()).unwrap();
        assert_eq!(packet["camera_first_person"], false);
        assert!(!grid.player_tags().vertices.is_empty());
        // Coder's plaza keeps its nearest orbit.
        let mut plaza = scene();
        plaza.activate(true).unwrap();
        for _ in 0..40 {
            plaza.action(Request::PinchZoom { scale: 1.1 }).unwrap();
        }
        assert!(!plaza.packet().camera_first_person);
    }

    #[test]
    fn the_stick_keeps_walking_while_the_other_hand_pinches() {
        let mut grid = bare_scene();
        let [sx, sy] = grid.stick_center();
        grid.pointer(7, PointerPhase::Down, sx, sy).unwrap();
        grid.pointer(7, PointerPhase::Move, sx, sy - 80.0).unwrap();
        assert_eq!(grid.packet().stick_pointer, Some(7));
        grid.update(1.1).unwrap();
        let distance = grid.world.camera.distance;
        // Two fingers land and pinch while the thumb holds the stick.
        grid.pointer(8, PointerPhase::Down, 200.0, 300.0).unwrap();
        grid.pointer(9, PointerPhase::Down, 500.0, 300.0).unwrap();
        grid.pointer(8, PointerPhase::Cancel, 0.0, 0.0).unwrap();
        grid.pointer(9, PointerPhase::Cancel, 0.0, 0.0).unwrap();
        let start = grid.world.player.pos;
        for frame in 1..=30 {
            grid.action(Request::PinchZoom { scale: 1.02 }).unwrap();
            grid.update(1.1 + f64::from(frame) / 60.0).unwrap();
        }
        assert!(grid.world.camera.distance < distance);
        assert!(
            grid.world.player.pos.distance(start) > 2.0,
            "the stick stopped driving during the pinch"
        );
        assert!(grid.input().forward);
        assert_eq!(grid.packet().stick_pointer, Some(7));
        grid.pointer(7, PointerPhase::Up, sx, sy - 80.0).unwrap();
        assert_eq!(grid.packet().stick_pointer, None);
        assert!(!grid.input().forward);
    }

    #[test]
    fn the_grid_draws_two_faint_sticks_in_touch_look_and_one_in_motion_look() {
        let mut grid = bare_scene();
        // Coder's stick draws a ring and a knob; the Grid draws two of each,
        // at half the opacity.
        let mut plaza = scene();
        plaza.activate(true).unwrap();
        plaza.update(1.0).unwrap();
        let coder = plaza.stick_ui().vertices;
        let both = grid.stick_ui().vertices;
        assert_eq!(both.len(), 2 * coder.len());
        let ring = coder.len() / 2;
        let alpha = |v: &[verse::ui::UiVertex]| v.iter().map(|v| v.color[3]).fold(0.0, f32::max);
        assert!((alpha(&both) - BARE_STICK_FAINTNESS * alpha(&coder)).abs() < 1e-6);
        assert!(ring > 0);
        // Motion look turns with the phone: the look stick hides, and a touch
        // where it stood does not take it.
        grid.action(Request::CameraMode {
            mode: CameraMode::Motion,
        })
        .unwrap();
        assert!(grid.look_stick_center().is_none());
        assert_eq!(grid.stick_ui().vertices.len(), coder.len());
        let size = grid.lifecycle.viewport().logical_size();
        grid.pointer(3, PointerPhase::Down, size[0] - 80.0, size[1] - 80.0)
            .unwrap();
        assert_eq!(grid.packet().look_stick_pointer, None);
        grid.pointer(3, PointerPhase::Up, size[0] - 80.0, size[1] - 80.0)
            .unwrap();
        // Back in touch look it returns; Coder's world never has one.
        grid.action(Request::CameraMode {
            mode: CameraMode::Touch,
        })
        .unwrap();
        assert!(grid.look_stick_center().is_some());
        assert!(plaza.look_stick_center().is_none());
    }

    #[test]
    fn the_look_stick_turns_and_pitches_at_a_rate_past_its_dead_zone() {
        let mut grid = bare_scene();
        let [lx, ly] = grid.look_stick_center().unwrap();
        let heading = |grid: &Scene| {
            verse::controller::wrap(grid.world.player.yaw + grid.world.camera.yaw_offset)
        };
        grid.pointer(4, PointerPhase::Down, lx, ly).unwrap();
        assert_eq!(grid.packet().look_stick_pointer, Some(4));
        assert_eq!(grid.packet().stick_pointer, None);
        // Inside the dead zone nothing turns, and holding it never walks.
        let yaw = heading(&grid);
        let pitch = grid.world.camera.pitch;
        grid.pointer(4, PointerPhase::Move, lx + STICK_DEAD_POINTS - 1.0, ly)
            .unwrap();
        for frame in 1..=30 {
            grid.update(1.0 + f64::from(frame) / 60.0).unwrap();
        }
        assert_eq!(heading(&grid), yaw);
        assert_eq!(grid.world.camera.pitch, pitch);
        assert!(!grid.input().forward && !grid.input().strafe_right);
        // Pushed right, the view turns right, as a drag to the right does,
        // at a steady rate once smoothing settles: a small push turns
        // slowly, the rim at the full rate.
        let turn = |grid: &mut Scene, dx: f32, start: f64| {
            grid.pointer(4, PointerPhase::Move, lx + dx, ly).unwrap();
            for frame in 1..=30 {
                grid.update(start + f64::from(frame) / 60.0).unwrap();
            }
            let before = heading(grid);
            for frame in 31..=90 {
                grid.update(start + f64::from(frame) / 60.0).unwrap();
            }
            verse::controller::wrap(heading(grid) - before)
        };
        let slow = turn(&mut grid, 30.0, 2.0);
        let full = turn(&mut grid, 200.0, 4.0);
        assert!(
            slow < 0.0 && full < 0.0,
            "a push right turns right: {slow} {full}"
        );
        assert!(
            (full + LOOK_STICK_YAW_RATE).abs() < 0.05,
            "the rim turns at the full rate for one second: {full}"
        );
        assert!(slow.abs() < 0.25 * full.abs(), "{slow} {full}");
        // Pushed up, it looks up.
        grid.pointer(4, PointerPhase::Move, lx, ly - 200.0).unwrap();
        let pitch = grid.world.camera.pitch;
        for frame in 1..=20 {
            grid.update(6.0 + f64::from(frame) / 60.0).unwrap();
        }
        assert!(grid.world.camera.pitch < pitch);
        // Releasing stops the turn at once.
        grid.pointer(4, PointerPhase::Up, lx, ly - 200.0).unwrap();
        assert_eq!(grid.packet().look_stick_pointer, None);
        let yaw = heading(&grid);
        let pitch = grid.world.camera.pitch;
        grid.update(6.5).unwrap();
        assert_eq!(grid.look_rate, [0.0, 0.0]);
        assert_eq!(heading(&grid), yaw);
        assert_eq!(grid.world.camera.pitch, pitch);
    }

    #[test]
    fn both_thumbs_move_and_look_at_once_while_other_fingers_pinch() {
        let mut grid = bare_scene();
        let [sx, sy] = grid.stick_center();
        let [lx, ly] = grid.look_stick_center().unwrap();
        grid.pointer(1, PointerPhase::Down, sx, sy).unwrap();
        grid.pointer(1, PointerPhase::Move, sx, sy - 80.0).unwrap();
        grid.update(1.1).unwrap();
        grid.pointer(2, PointerPhase::Down, lx, ly).unwrap();
        grid.pointer(2, PointerPhase::Move, lx + 80.0, ly).unwrap();
        let packet = grid.packet();
        assert_eq!(
            (packet.stick_pointer, packet.look_stick_pointer),
            (Some(1), Some(2))
        );
        let start = grid.world.player.pos;
        let yaw = grid.world.player.yaw;
        let distance = grid.world.camera.distance;
        // Two more fingers pinch; the host cancels them in Rust and sends
        // the zoom, and neither thumb lets go.
        grid.pointer(3, PointerPhase::Down, 150.0, 300.0).unwrap();
        grid.pointer(5, PointerPhase::Down, 250.0, 300.0).unwrap();
        grid.pointer(3, PointerPhase::Cancel, 0.0, 0.0).unwrap();
        grid.pointer(5, PointerPhase::Cancel, 0.0, 0.0).unwrap();
        for frame in 1..=45 {
            grid.action(Request::PinchZoom { scale: 1.01 }).unwrap();
            grid.update(1.1 + f64::from(frame) / 60.0).unwrap();
        }
        assert!(grid.world.camera.distance < distance, "the pinch zoomed");
        assert!(
            grid.world.player.pos.distance(start) > 2.0,
            "the left stick kept walking"
        );
        assert!(
            verse::controller::wrap(grid.world.player.yaw - yaw) < -0.5,
            "the right stick kept turning"
        );
        assert!(grid.input().forward);
        let packet = grid.packet();
        assert_eq!(
            (packet.stick_pointer, packet.look_stick_pointer),
            (Some(1), Some(2))
        );
        // Letting go of one thumb leaves the other working.
        grid.pointer(2, PointerPhase::Up, lx + 80.0, ly).unwrap();
        assert!(grid.input().forward);
        grid.pointer(1, PointerPhase::Up, sx, sy - 80.0).unwrap();
        assert!(!grid.input().forward);
    }

    #[test]
    fn whole_screen_looks_and_only_the_bottom_stick_moves() {
        let mut scene = scene();
        scene.activate(true).unwrap();
        scene.update(1.0).unwrap();
        assert_eq!(scene.stick_center(), [80.0, 520.0]);
        let yaw = scene.world.player.yaw;
        // A left-side drag away from the stick looks instead of walking.
        scene.pointer(1, PointerPhase::Down, 60.0, 200.0).unwrap();
        scene.pointer(1, PointerPhase::Move, 120.0, 200.0).unwrap();
        assert!(!scene.input().forward && !scene.input().backward);
        assert_ne!(scene.world.player.yaw, yaw);
        scene.pointer(1, PointerPhase::Up, 120.0, 200.0).unwrap();
        // Holding the stick's center with touch look stays still; pushing past
        // its base clamps to one axis's full deflection.
        scene.pointer(2, PointerPhase::Down, 80.0, 520.0).unwrap();
        assert!(!scene.input().forward);
        scene.pointer(2, PointerPhase::Move, 80.0, 300.0).unwrap();
        assert!(scene.input().forward && !scene.input().strafe_right);
        assert_eq!(
            scene.stick_deflection([80.0, 300.0]),
            [0.0, -STICK_RADIUS_POINTS]
        );
        assert!(!scene.stick_ui().vertices.is_empty());
        scene.pointer(2, PointerPhase::Up, 80.0, 300.0).unwrap();
        // Safe-area insets lift the stick.
        scene
            .action(Request::HudInsets {
                top: 0.0,
                right: 0.0,
                bottom: 34.0,
                left: 10.0,
            })
            .unwrap();
        assert_eq!(scene.stick_center(), [90.0, 486.0]);
        scene.open_computer();
        assert!(scene.stick_ui().vertices.is_empty());
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
        // Only the stick's touch outlives a pinch.
        assert!(scene.touches.len() == 1 && scene.touches[&1].movement);
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
        scene.pointer(1, PointerPhase::Down, 80.0, 520.0).unwrap();
        scene.pointer(2, PointerPhase::Down, 300.0, 300.0).unwrap();
        scene.pointer(3, PointerPhase::Down, 50.0, 500.0).unwrap();
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

    fn motion_scene() -> Box<Scene> {
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
        scene.pointer(1, PointerPhase::Down, 80.0, 520.0).unwrap();
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
        scene.pointer(1, PointerPhase::Move, 130.0, 570.0).unwrap();
        let input = scene.input();
        assert!(input.backward && input.strafe_right && !input.forward);
        scene.pointer(1, PointerPhase::Up, 130.0, 570.0).unwrap();
        assert!(!scene.input().forward && !scene.input().backward);
        let yaw = scene.world.player.yaw;
        scene
            .action(Request::CameraMode {
                mode: CameraMode::Touch,
            })
            .unwrap();
        assert!(scene.motion.target.is_none());
        scene.pointer(3, PointerPhase::Down, 80.0, 520.0).unwrap();
        assert!(!scene.input().forward, "touch mode requires joystick drag");
        scene.pointer(3, PointerPhase::Move, 80.0, 490.0).unwrap();
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
