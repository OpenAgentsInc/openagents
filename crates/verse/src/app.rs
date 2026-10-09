//! The desktop window: winit events in, a frame out.

use std::sync::Arc;
use std::time::{Duration, Instant};

use glam::Vec3;
use rust_native::surface::{SurfaceLifecycle, Viewport};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, NamedKey, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

use crate::agent::Agent;
use crate::avatar::{self, Gait};
use crate::brain::{self, Brain};
use crate::camera::FollowCamera;
use crate::chat::{self, Channel};
use crate::controller::{InputState, PlayerController};
use crate::doors::hud::DoorHud;
use crate::doors::{DemoItem, DoorId, DoorIntent, Doors};
use crate::feed::Feed;
use crate::grid_engine::GridEngine;
use crate::grid_frame;
use crate::hud;
use crate::minimap::{MapAction, MapHud};
use crate::nav::NavigationStatus;
use crate::render::{self, Renderer, View};
use crate::replay::{self, Place, Replay};
use crate::runtime::{Action, WorldRuntime};
use crate::session::{self, Session, Status};
use crate::ui::Atlas;
use crate::world;
use crate::xp;
use crate::zones::everglade::studio::PanelKind as StudioPanel;
use crate::zones::{self, Intent as ZoneIntent};

/// The hotbar slot number a digit key presses, 1 to 9, or 0 for any
/// other key.
fn hotbar_number(code: KeyCode) -> usize {
    match code {
        KeyCode::Digit1 => 1,
        KeyCode::Digit2 => 2,
        KeyCode::Digit3 => 3,
        KeyCode::Digit4 => 4,
        KeyCode::Digit5 => 5,
        KeyCode::Digit6 => 6,
        KeyCode::Digit7 => 7,
        KeyCode::Digit8 => 8,
        KeyCode::Digit9 => 9,
        _ => 0,
    }
}

/// How much higher than the chamber's bar Everglade's hotbar sits on
/// desktop, logical points: none.
const HOTBAR_BOTTOM: f32 = 0.0;

/// How the window joins the shared world.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    pub capability_flow: Option<std::path::PathBuf>,
    /// Selected retained contribution records for the shared read-only pane.
    pub contribution_workbench: Option<std::path::PathBuf>,
    pub quest_workbench: Option<std::path::PathBuf>,
    pub onboarding_practice: Option<std::path::PathBuf>,
    pub onboarding_workbench: Option<std::path::PathBuf>,
    /// Private authenticated compute account configuration for the shared sheet.
    pub compute_workbench: Option<std::path::PathBuf>,
    /// Optional local floating studio screen, independent of work placement.
    pub workbench_screen: Option<terminal_gfx::screen::Mode>,
    pub workbench_screen_bounds: [u16; 4],
    /// Profile name; each profile is its own player key.
    pub profile: String,
    /// Relay URL, or `None` to play offline.
    pub relay: Option<String>,
    /// Relay the quest board and XP read, when it isn't `relay`.
    pub xp_relay: Option<String>,
    /// More public keys (npub or hex) whose XP counts as the player's.
    pub xp_keys: Vec<String>,
    /// More referees (npub or hex) to trust, beyond the trust file.
    pub xp_referees: Vec<String>,
    /// A Microcoder run to replay on launch: its directory or Gym run ID.
    pub replay: Option<String>,
    /// Signed Gym connection JSON, read only after entering the Gym.
    pub gym_connection: Option<std::path::PathBuf>,
    /// Play Agent Studio's simulated team in Everglade, in a scratch
    /// repository under the system's temporary directory.
    pub studio_sim: bool,
    /// Feed Everglade's Pylon Field from the labeled DEMO pool rather than
    /// this computer's lease table (`everglade::compute::sim`).
    pub pylon_sim: bool,
    /// The control socket of the host whose Agent Studio Everglade shows,
    /// in place of this computer's own host (`openagents_connect::control::socket_path`).
    pub studio_socket: Option<std::path::PathBuf>,
    /// Explicit paired host for the shared terminal mount.
    pub terminal_host: Option<String>,
    pub terminal_task: Option<String>,
    pub terminal_store: Option<std::path::PathBuf>,
    /// Exact retained generation and terminal, without opening a replacement.
    pub terminal_reference: Option<String>,
    /// Start with the studio's bell and chimes silent; V toggles them in
    /// Everglade.
    pub studio_muted: bool,
    /// Open straight into Everglade once the window shows, as
    /// `openagents studio up` asks, rather than in the plaza.
    pub everglade: bool,
    /// Open straight into the Grove, the druid training field, once the
    /// window shows (`verse --grove`).
    pub grove: bool,
    /// Open the standalone castle bombardment zone.
    pub meteor_stress_test: bool,
    /// Open the Meteor Showcase, two kit houses under an eight-meteor
    /// swarm (`verse --meteor-showcase`).
    pub meteor_showcase: bool,
    /// Open straight into the crypt lab once the window shows (`verse
    /// --crypt`).
    pub crypt: bool,
    /// Open straight into the Water Lab once the window shows (`verse
    /// --water-lab`).
    pub water_lab: bool,
    /// Open the tidal coast at launch (`--coast`).
    pub coast: bool,
    /// Open Everglade as the demolition yard (`--demolition`): two kit
    /// cottages to knock down with a sledgehammer.
    pub demolition: bool,
    /// Put Meteor Swarm and the sledgehammer on Everglade's hotbar, a local
    /// test of destruction (`--dev-destruction`). Only a build with the
    /// `dev-destruction` feature accepts it.
    pub dev_destruction: bool,
    /// Print one JSON line of frame times per second to stdout.
    pub frame_times: bool,
    /// A scripted terminal stress run in Everglade's town (the
    /// `terminal_stress` example): it records frame times and key latency,
    /// writes its report, and quits.
    pub terminal_stress: Option<crate::terminal::stress::Plan>,
    /// A notice Everglade's caption leads with, such as that no coding
    /// agent can sign in, so the studio's seats cannot work.
    pub studio_notice: Option<String>,
    /// A request to hand the workshop agent once she is at her desk, as
    /// if the player walked up to her, opened her panel, and typed it
    /// (`--workshop-ask`), for a demo or a capture.
    pub workshop_ask: Option<String>,
    /// Where to stand the player once Everglade is up, as `[x, z, yaw]`
    /// in centimeters and hundredths of a degree (`--place X,Z[,YAW]` in
    /// meters and degrees), for a demo or a capture.
    pub place: Option<[i32; 3]>,
    /// Stand the player just inside the owner's house's front door, facing
    /// the workshop agent at her workstation, once Everglade loads
    /// (`--owners-house`).
    pub owners_house: bool,
    /// The town clock Everglade's sky and villagers follow. The command
    /// line runs the compressed cycle unless `--town-clock off` stops it;
    /// `--town-clock wall[:MIN]` follows real hours and `--town-hour` pins
    /// the hour (`VERSE_TOWN_CLOCK`, `VERSE_TOWN_HOUR`). The default here
    /// is the stopped daytime clock, so a test's world doesn't change with
    /// the hour.
    pub town_clock: town_clock::Clock,
    /// The pinned chamber the Grid's RITUAL arch joins
    /// ([`crate::ritual::Config`]); `None` draws no arch.
    pub ritual: Option<std::path::PathBuf>,
    /// A hosted Everglade instance to join over REACH instead of walking
    /// the local world (`--join FILE`, [`crate::hosted::Join`]).
    #[cfg(feature = "remote-chamber")]
    pub chamber: Option<std::path::PathBuf>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            profile: "default".into(),
            relay: Some(session::DEFAULT_RELAY.into()),
            xp_relay: None,
            xp_keys: Vec::new(),
            xp_referees: Vec::new(),
            replay: None,
            gym_connection: None,
            studio_sim: false,
            pylon_sim: false,
            studio_socket: None,
            terminal_host: None,
            terminal_task: None,
            terminal_store: None,
            terminal_reference: None,
            capability_flow: None,
            contribution_workbench: None,
            quest_workbench: None,
            onboarding_practice: None,
            onboarding_workbench: None,
            compute_workbench: None,
            workbench_screen: None,
            workbench_screen_bounds: [40, 60, 900, 600],
            studio_muted: false,
            everglade: false,
            grove: false,
            meteor_stress_test: false,
            meteor_showcase: false,
            crypt: false,
            water_lab: false,
            coast: false,
            demolition: false,
            dev_destruction: false,
            frame_times: false,
            terminal_stress: None,
            studio_notice: None,
            workshop_ask: None,
            place: None,
            owners_house: false,
            town_clock: town_clock::Clock::DAYTIME,
            ritual: crate::ritual::default_config(),
            #[cfg(feature = "remote-chamber")]
            chamber: None,
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

/// What a capture shows of quests and XP.
#[derive(Clone, Debug, Default)]
pub struct CaptureXp {
    /// Relay to read quests and awards from; `None` shows the offline HUD.
    pub relay: Option<String>,
    /// Public keys whose XP the HUD shows as the player's.
    pub keys: Vec<String>,
    /// More referees to trust.
    pub referees: Vec<String>,
    /// Whether the quest board is open.
    pub board: bool,
    /// A Microcoder run to show replaying: its directory or Gym run ID.
    pub replay: Option<String>,
    /// Seconds into the replay to show; the middle when `None`.
    pub at: Option<f64>,
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
    shot: &CaptureXp,
) -> Result<(), String> {
    let world = world::build();
    let player = PlayerController::new(world::SPAWN, 0.0);
    let view = view(&camera, &player, width as f32 / height as f32);
    let mut dynamic = avatar::mesh(&player, &Gait::default());
    dynamic.extend(&world::computer_display(None));
    // `VERSE_PLAZA_LEGACY` renders the flat amber path for comparisons.
    if std::env::var_os("VERSE_PLAZA_LEGACY").is_none() {
        dynamic.neon = Some(crate::pbr::Neon::plaza(0.0));
    }
    let shown = match &shot.replay {
        Some(arg) => {
            let run = replay::find(arg)?;
            let mut r = Replay::load(&run, Place::Plaza.stand(false))?;
            if let Err(why) = &r.ghost {
                eprintln!("verse: no ghost: {why}");
            }
            let at = shot
                .at
                .map_or(r.clock.duration_ms as f64 / 2.0, |s| s * 1000.0);
            r.clock.seek(at);
            r.clock.playing = false;
            r.settle();
            let [mine, ghost] = r.carrots();
            let (agent, ghost) = (Agent::at(mine, 0.0), Agent::at(ghost, 0.0));
            dynamic.extend(&agent.mesh());
            if r.ghost.is_ok() {
                dynamic.extend(&ghost.mesh_at(coder_ui::theme::Intensity::ThreeQuarters));
            }
            Some((r, agent, ghost))
        }
        None => {
            dynamic.extend(&Agent::new(&player).mesh());
            None
        }
    };
    let atlas = Atlas::new(14.0);
    let board = shot.relay.as_ref().map(|relay| {
        let mut board = xp::Board::start(relay, &shot.referees, None);
        board.settle(Duration::from_millis(1500), Duration::from_secs(12));
        board
    });
    let mine = xp::my_keys(None, &shot.keys);
    let mut frame = sample_hud(&view, [width as f32, height as f32], &player);
    frame.xp = xp::strip(board.as_ref(), &mine);
    frame.board = shot
        .board
        .then(|| xp::board_lines(board.as_ref(), unix_now()));
    if let Some((r, agent, ghost)) = &shown {
        frame.replay = r.hud_lines();
        let mut overheads = landmark_overheads(player.pos);
        overheads.extend(replay_overheads(r, agent, ghost));
        overheads.push(frame.overheads[0].clone());
        frame.overheads = Box::leak(overheads.into_boxed_slice());
    }
    let (ui, _) = hud::build(&atlas, &frame);
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
    log.push(Line {
        channel: Some(Channel::Agent),
        from: "you".into(),
        to: None,
        text: "what's that tall thing?".into(),
        note: None,
    });
    log.push(Line {
        channel: Some(Channel::Agent),
        from: "agent".into(),
        to: None,
        text: "That's the pylon at the heart of the Plaza.".into(),
        note: None,
    });
    let agent = Agent::new(player);
    let overheads: &'a [hud::Overhead] = Box::leak(Box::new([
        hud::Overhead {
            feet: player.pos,
            lift: 2.2,
            name: Some("you".into()),
            name_step: coder_ui::theme::Intensity::ThreeQuarters,
            bubble: Some("gm verse".into()),
        },
        hud::Overhead {
            feet: agent.pos,
            lift: 0.5,
            name: None,
            name_step: coder_ui::theme::Intensity::Half,
            bubble: Some("That's the pylon at the heart of the Plaza.".into()),
        },
        hud::Overhead {
            feet: world::QUEST_BOARD,
            lift: 5.4,
            name: Some("QUEST BOARD".into()),
            name_step: coder_ui::theme::Intensity::Half,
            bubble: None,
        },
    ]));
    let pills = session::ROOMS.iter().fold(
        vec![
            ("ALL".to_owned(), true),
            ("ADS".to_owned(), false),
            ("ZONE".to_owned(), false),
            ("NEAR".to_owned(), false),
            ("HERE".to_owned(), false),
        ],
        |mut v, r| {
            v.push((format!("#{r}"), false));
            v
        },
    );
    let mut pills = pills;
    pills.push(("AGENT".into(), false));
    hud::Frame {
        size,
        scale: 1.0,
        view_proj: view.view_proj,
        log: Box::leak(Box::new(log)),
        nostr: Box::leak(Box::new(std::collections::VecDeque::new())),
        nostr_title: "live public notes · damus · primal".into(),
        left_tab: hud::LeftTab::World,
        input: Box::leak(Box::new(hud::Input::default())),
        pills,
        hint: format!("everyone in {}", session::WORLD),
        limit: Some(chat::MAX_BROADCAST),
        world_title: hud::world_title(session::WORLD, player.pos),
        overheads,
        time: 0.0,
        xp: Vec::new(),
        board: None,
        board_scroll: 0,
        replay: Vec::new(),
        picker: None,
        picker_scroll: 0,
    }
}

/// Name tags over the replay landmarks within 80 m of `from`.
fn landmark_overheads(from: Vec3) -> Vec<hud::Overhead> {
    use coder_ui::theme::Intensity;
    Place::LANDMARKS
        .iter()
        .filter(|p| p.position().distance(from) <= 80.0)
        .map(|p| hud::Overhead {
            feet: p.position(),
            // Above the spades' own tags when they visit.
            lift: match p {
                Place::Oracle => 7.4,
                Place::Library => 6.0,
                _ => 4.4,
            },
            name: Some(p.name().to_uppercase()),
            name_step: Intensity::Half,
            bubble: None,
        })
        .collect()
}

/// Who each replayed spade is and where it is: over the player's agent
/// and over the ghost.
fn replay_overheads(r: &Replay, agent: &Agent, ghost: &Agent) -> Vec<hud::Overhead> {
    use coder_ui::theme::Intensity;
    let (mine, theirs) = r.places();
    let mut out = vec![hud::Overhead {
        feet: agent.pos,
        lift: 0.6,
        name: Some(format!("Microcoder · {}", mine.name())),
        name_step: Intensity::Full,
        bubble: None,
    }];
    if let Some(theirs) = theirs {
        out.push(hud::Overhead {
            feet: ghost.pos,
            lift: 0.6,
            name: Some(format!(
                "{} · {}",
                gym::runs_beats_winner::REFERENCE,
                theirs.name()
            )),
            name_step: Intensity::ThreeQuarters,
            bubble: None,
        });
    }
    out
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn view(camera: &FollowCamera, player: &PlayerController, aspect: f32) -> View {
    View {
        view_proj: camera.view_proj(player.pos, player.yaw, aspect),
        eye: camera.eye(player.pos, player.yaw),
    }
}

/// Resolves a pubkey to a display name.
type NameOf<'a> = Box<dyn Fn(&str) -> String + 'a>;

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

/// A scene click stays distinct from a camera drag, including a drag back to its origin.
struct CompanionPress {
    origin: [f32; 2],
    started: Instant,
    valid: bool,
    travel: f32,
}

impl CompanionPress {
    fn new(origin: [f32; 2], started: Instant) -> Self {
        Self {
            origin,
            started,
            valid: true,
            travel: 0.0,
        }
    }

    fn moved(&mut self, at: [f32; 2]) {
        self.valid &= (at[0] - self.origin[0]).hypot(at[1] - self.origin[1]) <= 8.0;
    }

    fn motion(&mut self, dx: f32, dy: f32) {
        self.travel += dx.hypot(dy);
        self.valid &= self.travel <= 8.0;
    }

    fn released(mut self, at: [f32; 2], now: Instant) -> bool {
        self.moved(at);
        self.valid && now.duration_since(self.started) <= Duration::from_millis(300)
    }
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
    /// The legacy line renderer, for every zone but the Grid.
    renderer: Option<Renderer>,
    /// The engine renderer, while the window shows the Grid.
    grid: Option<GridEngine>,
    /// Frame-time summaries, when `--frame-times` asked for them.
    timing: Option<grid_frame::Timing>,
    runtime: WorldRuntime,
    keys: Keys,
    mount: Option<SurfaceLifecycle>,
    error: Option<String>,
    session: Option<Session>,
    /// Who may send this operator NIP-MV zone commands.
    zone_operators: crate::zones::operators::Operators,
    connection_options: Options,
    plaza_services_paused: bool,
    /// The chamber window a RITUAL crossing opened, until it closes.
    chamber: Option<std::process::Child>,
    plaza_presence: (PlayerController, Agent),
    rendered_zone_revision: u64,
    zone_hud: zones::hud::Hud,
    zone_frame: Option<zones::hud::Snapshot>,
    zone_press: Option<CompanionPress>,
    title: String,
    frames: u64,
    atlas: Option<Atlas>,
    map_atlas: Option<Atlas>,
    map: MapHud,
    map_frame: Option<crate::minimap::Snapshot>,
    map_error: Option<String>,
    companion_press: Option<CompanionPress>,
    door_press: Option<(DoorId, CompanionPress)>,
    door_hud: DoorHud,
    door_frame: Option<crate::doors::hud::Snapshot>,
    door_store: (std::path::PathBuf, String),
    door_error: Option<String>,
    door_storage_error: Option<String>,
    door_save_revision: u64,
    presented_entities: crate::mesh::Mesh,
    scale: f32,
    chat: hud::Input,
    method: Channel,
    brain: Brain,
    /// Conversations with Everglade's villagers, started on the first one,
    /// so a window that never talks reads no save and starts no voice.
    town_talk: Option<crate::town_talk::TownTalk>,
    agent_says: Option<(String, Option<Instant>)>,
    feed: Option<Feed>,
    left_tab: hud::LeftTab,
    cursor: [f32; 2],
    layout: hud::Layout,
    pm_target: Option<String>,
    offline_log: chat::Log,
    started: Instant,
    xp: Option<xp::Board>,
    board_open: bool,
    board_scroll: usize,
    my_keys: Vec<String>,
    /// The replay playing, if any.
    replay: Option<Replay>,
    /// The replay's ghost: Fable's cheapest winning run on the task.
    ghost: Agent,
    /// Whether each side's finish has been marked.
    finished: [bool; 2],
    /// The replay list, while it is open.
    picker: Option<Picker>,
    /// The retained runs the list offers, read when it first opens.
    choices: Option<Vec<replay::Choice>>,
    gym: Option<crate::gym::Board>,
    gym_view: Option<crate::gym::BoardView>,
    gym_identity_attempted: bool,
    gym_profile: String,
    gym_connection: Option<std::path::PathBuf>,
    gym_connection_attempted: bool,
    gym_open: bool,
    gym_recipes: bool,
    gym_selected: usize,
    gym_scroll: usize,
    gym_notice: Option<String>,
    /// The Rust Native panel over the world, while it is open (C).
    panel: Option<crate::panels::Panel>,
    /// The Agent Studio panel the panel shows, its controls, and the
    /// studio revision it was filled from; `None` while it shows the
    /// replay's transcript.
    studio_panel: Option<crate::panels::studio::Controller>,
    /// What a tap in progress in Everglade selects, while `zone_press`
    /// tracks it.
    studio_target: Option<StudioPanel>,
    /// Whether the panel took the left button's last press.
    panel_press: bool,
    /// Whether Shift is held while the panel has focus, for Shift+Enter
    /// and Shift+Tab.
    panel_shift: bool,
    /// The terminal overlay (T), its panes, and their sessions.
    terminal: crate::terminal::Overlay,
    onboarding_capture: Option<terminal_studio::onboarding::host::Capture>,
    onboarding_pending: Option<(u64, coder_access::Operation)>,
    onboarding_seen: Option<(String, u64)>,
    screen_mode: Option<terminal_gfx::screen::Mode>,
    screen_bounds: [u16; 4],
    screen: Option<terminal_gfx::screen::Screen>,
    screen_vertices: Vec<crate::ui::UiVertex>,
    screen_clock: Instant,
    screen_viewport: Option<([u32; 2], u32, u64)>,
    /// The workshop agent at her desk in Everglade, her panel, and the
    /// pane she drives.
    workshop: crate::workshop::Workshop,
    /// When `--workshop-ask`'s typed request is sent: a few seconds after
    /// the player reaches her, so a capture shows the request first.
    workshop_send_at: Option<Instant>,
    /// Whether the terminal overlay took the left button's last press.
    terminal_press: bool,
    /// A scripted terminal stress run, when one was asked for.
    stress: Option<crate::terminal::stress::Driver>,
    /// The atlas revision the renderer last uploaded.
    atlas_revision: u64,
    /// What the studio console keeps while its panel is closed: the
    /// history and the unsent draft.
    studio_recall: crate::panels::studio::Recall,
    /// Whether the studio's bell and chimes are silent (V in Everglade).
    studio_muted: bool,
    /// The goal bar's waiting badge this frame, which opens the decisions.
    studio_badge: Option<hud::Rect>,
    /// Whether the window is in front, so a studio signal also raises a
    /// desktop notice while it is not.
    window_focused: bool,
    /// `--everglade` asked to open Everglade and it has not loaded yet. The
    /// request survives a load that losing focus cancels, so a window that
    /// starts behind another still enters once it comes to the front.
    everglade_pending: Option<zones::ZoneId>,
    /// A held descent (-1, the X key) while levitating in Everglade, or 0.
    climb: f32,
    /// Whether Ctrl and Alt are held, for the Grove's rows 3 and 4.
    grove_ctrl: bool,
    grove_alt: bool,
    /// Each held Grove key and the slot it pressed, so letting go releases
    /// that slot whatever modifiers are held then.
    grove_keys: Vec<(KeyCode, ZoneIntent)>,
    /// The row a compact Grove bar shows.
    grove_row: usize,
    /// Whether the mouse holds Everglade's Levitate slot down.
    levitate_held: bool,
    /// Whether the mouse holds the Water Lab's Water Orb slot down, so
    /// letting go anywhere throws the orb.
    water_orb_held: bool,
    /// The hotbar slot the pointer rests on, for its card's hover delay.
    slot_tip: crate::tooltip::Dwell,
    /// The hosted instance this window walks in, and the heading its
    /// keyboard steers.
    #[cfg(feature = "remote-chamber")]
    hosted: Option<(crate::hosted::Link, f32)>,
    /// When the left button went down in the demolition yard: a quick
    /// click swings the hammer, a drag turns the camera.
    swing_press: Option<Instant>,
}

/// The replay list: the retained `beats-winner` runs and which is chosen.
struct Picker {
    choices: Vec<replay::Choice>,
    selected: usize,
    /// Why the last choice didn't start.
    notice: Option<String>,
}

impl Picker {
    fn lines(&self) -> Vec<(String, coder_ui::theme::Intensity)> {
        use coder_ui::theme::Intensity;
        let mut out = Vec::new();
        if self.choices.is_empty() {
            out.push((
                "No retained Microcoder pass beats Fable 5.1 low's cheapest or fastest winning run."
                    .to_owned(),
                Intensity::Half,
            ));
        }
        for (i, c) in self.choices.iter().enumerate() {
            let chosen = i == self.selected;
            out.push((
                format!("{} {}", if chosen { ">" } else { " " }, c.line),
                if chosen {
                    Intensity::Full
                } else {
                    Intensity::ThreeQuarters
                },
            ));
            out.push((
                format!("    [{}]", c.labels),
                if chosen {
                    Intensity::Half
                } else {
                    Intensity::Quarter
                },
            ));
        }
        out.push((
            "Each run: its cost and time, then Fable 5.1 low's cheapest winning run's. Up and Down choose; Enter plays.".to_owned(),
            Intensity::Quarter,
        ));
        if let Some(notice) = &self.notice {
            out.push((notice.clone(), Intensity::Full));
        }
        out
    }

    /// Rows to scroll so the chosen run stays in view: two rows a run.
    fn scroll(&self) -> usize {
        (self.selected * 2).saturating_sub(6)
    }
}

/// The zone operator list for this session's key: itself plus the keys in
/// `<VERSE_HOME>/zone-operators` and `VERSE_ZONE_OPERATORS`. Malformed
/// entries are noted in the session log rather than admitted.
fn zone_operators_for(session: Option<&Session>) -> crate::zones::operators::Operators {
    let Some(session) = session else {
        return crate::zones::operators::Operators::default();
    };
    let list = crate::zones::operators::Operators::load(&crate::identity::home(), session.pubkey());
    if !list.rejected.is_empty() {
        eprintln!(
            "verse: zone-operators: {} malformed entries skipped: {}",
            list.rejected.len(),
            list.rejected.join(", ")
        );
    }
    list
}

/// `session` with the block and mute lists this computer keeps in
/// [`crate::identity::home`]; an unreadable file leaves them empty.
fn with_saved_blocklist(mut session: Session) -> Session {
    if let Ok(people) = crate::blocklist::Blocklist::load(&crate::identity::home()) {
        session.set_blocklist(people);
    }
    session
}

/// Blocks or unblocks the player whose name or key starts with `name`, saves
/// the list, and returns the notice to show.
fn block_by_name(session: &mut Session, name: &str, block: bool) -> String {
    let found = if block {
        session.find_player(name).map(|(pubkey, _)| pubkey)
    } else {
        session
            .blocklist()
            .blocked
            .iter()
            .find(|pubkey| {
                pubkey.starts_with(name) || session.name_of(pubkey).to_lowercase().starts_with(name)
            })
            .cloned()
    };
    let Some(pubkey) = found else {
        return format!(
            "No player named {name} to {}.",
            if block { "block" } else { "unblock" }
        );
    };
    let who = session.name_of(&pubkey);
    let changed = if block {
        match session.block(&pubkey) {
            Ok(changed) => changed,
            Err(error) => return error,
        }
    } else {
        session.unblock(&pubkey)
    };
    if changed && let Err(error) = session.blocklist().save(&crate::identity::home()) {
        return format!("Couldn't save the block list: {error}");
    }
    if block {
        format!("Blocked {who}. Type !unblock {name} to see them again.")
    } else {
        format!("Unblocked {who}.")
    }
}

/// The terminal overlay, listening on its control socket so
/// `openagents verse terminal` can drive it. A socket that cannot be
/// bound is reported once and the overlay works from the keyboard alone.
fn onboarding_config(
    options: &Options,
) -> Result<Option<terminal_studio::onboarding::host::Config>, String> {
    match (&options.onboarding_practice, &options.onboarding_workbench) {
        (Some(_), Some(_)) => Err("Select either practice or retained onboarding evidence".into()),
        (Some(root), None) => terminal_studio::onboarding::host::practice::practice(root).map(Some),
        (None, Some(path)) => terminal_studio::onboarding::host::Config::load(path).map(Some),
        _ => Ok(None),
    }
}
fn terminal_overlay(
    options: &Options,
    onboarding: Option<&terminal_studio::onboarding::host::Config>,
) -> Result<crate::terminal::Overlay, String> {
    if onboarding.is_some() && options.terminal_host.is_some() {
        return Err("Onboarding requires its isolated local scratch workspace".into());
    }
    let mut overlay = if let Some(config) = onboarding {
        config.rows()?;
        crate::terminal::Overlay::with(
            &config.starter.join("home"),
            terminal_gfx::pty::user_shell(),
            terminal_gfx::pty::Program::Shell,
        )
    } else if let Some(host) = &options.terminal_host {
        let store = options
            .terminal_store
            .as_ref()
            .ok_or("--terminal-host requires --terminal-store.")?;
        let reference = options
            .terminal_reference
            .as_ref()
            .map(|value| {
                let (generation, terminal) = value
                    .split_once('/')
                    .ok_or("--terminal-reference needs generation/terminal.")?;
                crate::terminal::remote::reference(generation, terminal)
            })
            .transpose()?;
        let remote = crate::terminal::remote::Remote::paired(
            store,
            host.clone(),
            reference,
            crate::terminal::pty::for_user().0,
        )?;
        let remote = match &options.terminal_task {
            Some(task) => remote.for_task(task.clone())?,
            None => remote,
        };
        crate::terminal::Overlay::on_host(remote)
    } else {
        if options.terminal_store.is_some()
            || options.terminal_reference.is_some()
            || options.terminal_task.is_some()
        {
            return Err(
                "--terminal-store, --terminal-reference, and --terminal-task require --terminal-host.".into(),
            );
        }
        crate::terminal::Overlay::new()
    };
    if let Some(config) = onboarding {
        overlay.studio_transport = std::sync::Arc::new(
            terminal_studio::Native::new(Some(config.starter.join("home")))
                .with_onboarding(config.clone()),
        );
        terminal_studio::onboarding::host::mount(&mut overlay.core, config.clone())?;
        overlay.open = true;
    }
    if onboarding.is_none()
        && let Some(path) = crate::terminal::control::default_path()
        && let Err(error) = overlay.listen(&path)
    {
        eprintln!("verse: the terminal control socket is off: {error}");
    }
    if let Some(root) = &options.capability_flow {
        let owner = openagents_chat::plugin_workbench::Owner::open(
            root.clone(),
            openagents_chat::client::NoCoder,
        );
        let record = owner.read()?;
        let subject = workbench::pane::Subject::Resource {
            resource: workbench::ResourceRef::new(
                workbench::Kind::Evidence,
                workbench::Host::Local {
                    instance: openagents_chat::plugin_workbench::local_instance(root),
                },
                record.source.flow,
            ),
        };
        let draft = openagents_chat::plugin_workbench::DraftPane::open(root.clone());
        let subjects = draft.subjects()?;
        overlay.core.products.panes =
            std::mem::take(&mut overlay.core.products.panes).adapter(Box::new(draft));
        for file in subjects {
            overlay
                .core
                .products
                .open(workbench::pane::PaneKind::Artifact, &file)?;
        }
        overlay.core.products.panes =
            std::mem::take(&mut overlay.core.products.panes).adapter(Box::new(owner));
        overlay
            .core
            .products
            .open(workbench::pane::PaneKind::Evaluation, &subject)?;
        overlay.core.paper.on = true;
        overlay.open = true;
    }
    if let Some(path) = &options.quest_workbench {
        contribution_workbench::quests::host::mount(
            &mut overlay.core,
            contribution_workbench::quests::host::Config::load(path)?,
        )?;
        overlay.open = true;
    }
    if let Some(path) = &options.contribution_workbench {
        if let Some(onboarding) = onboarding {
            let selected = contribution_workbench::host::Config::load(path)?;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_secs());
            if let Some(row) = selected.read(now)?.first() {
                onboarding.inspect_contribution(path, &row.source_record)?;
                terminal_studio::onboarding::host::mount(&mut overlay.core, onboarding.clone())?;
            }
        }
        contribution_workbench::host::mount(
            &mut overlay.core,
            contribution_workbench::host::Config::load(path)?,
        )?;
        overlay.open = true;
    }
    if let Some(path) = &options.compute_workbench {
        compute_workbench::host::mount(
            &mut overlay.core,
            compute_workbench::host::Config::load(path)?,
        )?;
        overlay.open = true;
    }
    Ok(overlay)
}

impl App {
    fn new(options: &Options) -> Result<Self, String> {
        let onboarding = onboarding_config(options)?;
        let studio_socket = if let Some(config) = &onboarding {
            options
                .studio_socket
                .as_ref()
                .map(|path| {
                    let target = path
                        .canonicalize()
                        .map_err(|_| "Scratch studio socket unavailable")?;
                    let home = config
                        .starter
                        .join("home")
                        .canonicalize()
                        .map_err(|_| "Scratch home unavailable")?;
                    if !target.starts_with(home) {
                        return Err(
                            "Onboarding studio socket must be inside its scratch home".to_string()
                        );
                    }
                    Ok(target)
                })
                .transpose()?
        } else {
            options
                .studio_socket
                .clone()
                .or_else(openagents_connect::control::socket_path)
                .filter(|_| !cfg!(test) || options.studio_socket.is_some())
        };
        // Find OpenAgents Terminal for the terminal overlay ahead of time:
        // a fresh build's first run can take seconds while macOS checks it.
        #[cfg(not(test))]
        std::thread::spawn(crate::terminal::pty::openagents_terminal);
        let world = world::build();
        let mut player = PlayerController::new(world::SPAWN, 0.0);
        let mut session = match &options.relay {
            Some(relay) => Some(with_saved_blocklist(Session::start(
                &options.profile,
                relay,
            )?)),
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
        let xp_relay = options.xp_relay.clone().or_else(|| options.relay.clone());
        let board = xp_relay.as_deref().map(|relay| {
            xp::Board::start(
                relay,
                &options.xp_referees,
                session.as_ref().map(Session::signer),
            )
        });
        let my_keys = xp::my_keys(session.as_ref().map(Session::pubkey), &options.xp_keys);
        let agent = Agent::new(&player);
        let replay = match &options.replay {
            Some(arg) => {
                let run = replay::find(arg)?;
                let r = Replay::load(&run, agent.pos)?;
                if let Err(why) = &r.ghost {
                    eprintln!("verse: no ghost: {why}");
                }
                Some(r)
            }
            None => None,
        };
        let door_directory = crate::identity::home();
        let mut doors = Doors::default();
        let door_error = match crate::doors::store::load(&door_directory, &options.profile) {
            Ok(Some(document)) => doors.restore(&document).err(),
            Ok(None) => None,
            Err(error) => Some(error),
        };
        let door_save_revision = doors.revision();
        let mut runtime = WorldRuntime::new();
        runtime.world = world;
        runtime.player = player;
        runtime.agent = agent;
        runtime.doors = doors;
        runtime.configure_zone_cache(crate::identity::home().join("zones-cache"));
        // The medieval kit pack downloads from the web origin once and stays
        // in the cache by its digest; tests never reach the network.
        runtime.download_zone_kit(!cfg!(test));
        // The owner's private characters in Everglade, from placements in
        // Verse's home (docs/verse/private-assets.md). Tests never read the
        // real home.
        #[cfg(not(test))]
        runtime.configure_private_assets(crate::identity::home());
        if options.studio_sim {
            // Records the simulated team on first entry to Everglade, off the
            // frame, and plays a frame every two seconds.
            runtime.set_studio_source(Box::new(
                crate::zones::everglade::studio::fixture::Background::new(2.0),
            ));
        } else if let Some(path) = studio_socket.clone() {
            // The host on this computer, the one the desktop app pairs,
            // through its control socket. Nothing connects until the player
            // enters Everglade.
            runtime.set_studio_source(Box::new(
                crate::zones::everglade::studio::live::Live::control(path),
            ));
        }
        runtime.set_studio_notice(options.studio_notice.clone());
        // Everglade's Pylon Field: the labeled demo pool, or this computer
        // from its lease table and capacity book, read without changing
        // them, beside the relay's pylons from their verified beacons
        // (`VERSE_PYLON_RELAY=off` leaves those out). Tests never read the
        // real home or reach a relay.
        if options.pylon_sim {
            runtime.set_compute_source(Some(Box::new(crate::zones::everglade::compute::sim::Sim)));
        } else if cfg!(not(test))
            && let Ok(local) = crate::zones::everglade::compute::local::LocalSource::from_env()
        {
            #[allow(unused_mut)]
            let mut sources: Vec<
                Box<dyn crate::zones::everglade::compute::ComputeSource>,
            > = vec![Box::new(local)];
            #[cfg(feature = "pylon-relay")]
            if let Some(relay) = crate::zones::everglade::compute::relay::RelaySource::from_env() {
                sources.push(Box::new(relay));
            }
            runtime.set_compute_source(Some(Box::new(crate::zones::everglade::compute::Merged(
                sources,
            ))));
        }
        #[cfg(all(feature = "model-host", not(test), not(target_arch = "wasm32")))]
        if let Ok(source) = crate::zones::everglade::sales_floor::local::LocalOwner::from_env() {
            runtime.set_sales_source(Some(Box::new(source)));
        }
        #[cfg(feature = "remote-chamber")]
        let hosted = match &options.chamber {
            Some(path) => {
                let heading = runtime.player.yaw;
                let link = crate::hosted::Join::read(path)?.open(&mut runtime)?;
                Some((link, heading))
            }
            None => None,
        };
        runtime.set_demolition(options.demolition);
        runtime.set_town_clock(options.town_clock);
        runtime.set_dev_destruction(options.dev_destruction)?;
        let zone_operators = zone_operators_for(session.as_ref());
        Ok(Self {
            window: None,
            renderer: None,
            grid: None,
            timing: options
                .frame_times
                .then(|| grid_frame::Timing::start(Instant::now())),
            runtime,
            keys: Keys::default(),
            mount: None,
            error: None,
            session,
            zone_operators,
            connection_options: options.clone(),
            plaza_services_paused: false,
            chamber: None,
            plaza_presence: (player, agent),
            rendered_zone_revision: 0,
            zone_hud: zones::hud::Hud::default(),
            zone_frame: None,
            zone_press: None,
            title: String::new(),
            frames: 0,
            atlas: None,
            map_atlas: None,
            map: MapHud::default(),
            map_frame: None,
            map_error: None,
            companion_press: None,
            door_press: None,
            door_hud: DoorHud::default(),
            door_frame: None,
            door_store: (door_directory, options.profile.clone()),
            door_error: None,
            door_storage_error: door_error,
            door_save_revision,
            presented_entities: crate::mesh::Mesh::default(),
            scale: 1.0,
            chat: hud::Input::default(),
            method: Channel::All,
            brain: Brain::start(&options.profile),
            town_talk: None,
            agent_says: None,
            feed: options.relay.as_ref().map(|_| Feed::start()),
            left_tab: hud::LeftTab::World,
            cursor: [0.0, 0.0],
            layout: hud::Layout::default(),
            pm_target: None,
            offline_log: {
                let mut log = chat::Log::default();
                log.push(chat::Line::system(
                    "Welcome to Verse. Press Enter to chat, Tab to change channel.",
                ));
                log
            },
            started: Instant::now(),
            xp: board,
            board_open: false,
            board_scroll: 0,
            my_keys,
            replay,
            ghost: Agent::at(Place::Plaza.stand(true), 0.0),
            finished: [false; 2],
            picker: None,
            choices: None,
            gym: None,
            gym_view: None,
            gym_identity_attempted: false,
            gym_profile: options.profile.clone(),
            gym_connection: options.gym_connection.clone(),
            gym_connection_attempted: false,
            gym_open: false,
            gym_recipes: false,
            gym_selected: 0,
            gym_scroll: 0,
            gym_notice: None,
            panel: None,
            panel_press: false,
            studio_panel: None,
            studio_target: None,
            panel_shift: false,
            terminal: terminal_overlay(options, onboarding.as_ref())?,
            onboarding_capture: onboarding.map(terminal_studio::onboarding::host::Capture::new),
            onboarding_pending: None,
            onboarding_seen: None,
            screen_mode: options.workbench_screen,
            screen_bounds: options.workbench_screen_bounds,
            screen: None,
            screen_vertices: Vec::new(),
            screen_clock: Instant::now(),
            screen_viewport: None,
            // Alice is a client of the host the studio reads, over the same
            // control socket; a test never reaches the person's own host.
            workshop: crate::workshop::Workshop::control(studio_socket),
            workshop_send_at: None,
            terminal_press: false,
            stress: options
                .terminal_stress
                .clone()
                .map(crate::terminal::stress::Driver::new),
            atlas_revision: 0,
            studio_recall: crate::panels::studio::Recall::default(),
            studio_muted: options.studio_muted,
            studio_badge: None,
            window_focused: true,
            everglade_pending: None,
            swing_press: None,
            climb: 0.0,
            grove_ctrl: false,
            grove_alt: false,
            grove_keys: Vec::new(),
            grove_row: 0,
            levitate_held: false,
            water_orb_held: false,
            slot_tip: crate::tooltip::Dwell::default(),
            #[cfg(feature = "remote-chamber")]
            hosted,
        })
    }

    /// Plays the studio's signals since the last frame, the most urgent
    /// one of a burst, unless muted, and raises a desktop notice for it
    /// while the window is not in front.
    fn studio_signals(&mut self) {
        // A revoked grant or a changed host process drops private context.
        if self.terminal.workshop().is_some_and(|opening| {
            !self
                .runtime
                .studio()
                .rights()
                .contains(&coder_access::Right::Observe)
                || self
                    .runtime
                    .studio()
                    .source_snapshot()
                    .is_none_or(|snapshot| snapshot.stream != opening.stream)
        }) {
            self.terminal.clear_workshop();
        }
        self.workbench_studio();
        use crate::zones::everglade::signals::{Signal, deliver};
        let events = self.runtime.take_studio_events();
        let Some(signal) = Signal::most_urgent(events.iter().map(|e| e.signal)) else {
            return;
        };
        // V mutes for the window; the console's `/sound off` for the session.
        if !self.studio_muted && self.runtime.studio().sounds() {
            deliver::play(signal);
        }
        if !self.window_focused
            && let Some(event) = events.iter().find(|e| e.signal == signal)
        {
            deliver::notify(event);
        }
    }

    fn workbench_studio(&mut self) {
        if let (Some(capture), Some(snapshot)) = (
            &self.onboarding_capture,
            self.runtime.studio().source_snapshot(),
        ) {
            let identity = (snapshot.stream.clone(), snapshot.sequence);
            if self.onboarding_seen.as_ref() != Some(&identity) {
                if let Err(error) = capture.record_snapshot(snapshot.clone()) {
                    self.terminal.paper.studio.notice =
                        Some(format!("Onboarding evidence unavailable: {error}"));
                }
                self.onboarding_seen = Some(identity);
            }
        }
        if let Some((ticket, _)) = &self.onboarding_pending
            && let Some(answer) = self
                .runtime
                .studio()
                .status()
                .filter(|answer| answer.ticket == *ticket)
        {
            let result = answer.result.clone();
            let (ticket, operation) = self.onboarding_pending.take().unwrap();
            if let (Some(capture), Ok(outcome)) = (&self.onboarding_capture, result)
                && let Err(error) =
                    capture.complete(&format!("ticket:{ticket}"), operation, outcome)
            {
                self.terminal.paper.studio.notice =
                    Some(format!("Onboarding evidence unavailable: {error}"));
            }
        }
        self.validate_screen();
        if self.terminal.workshop().is_none() {
            return;
        }
        let studio = self.runtime.studio();
        if !studio.available() {
            let refusal = studio
                .status()
                .and_then(|answer| answer.result.as_ref().err())
                .map(ToString::to_string);
            self.terminal.paper.studio.revoke();
            if let Some(refusal) = refusal {
                self.terminal.paper.studio.notice = Some(format!(
                    "{refusal}. Studio observation unavailable; no automatic replay."
                ));
            }
            return;
        }
        let mut rights = studio.rights();
        if self
            .screen
            .as_ref()
            .is_some_and(|screen| screen.mode == terminal_gfx::screen::Mode::Watch)
        {
            rights.retain(|right| *right == coder_access::Right::Observe);
        }
        let Some(snapshot) = studio.source_snapshot() else {
            return;
        };
        if self.terminal.paper.studio.view.as_ref().is_none_or(|view| {
            view.stream != snapshot.stream
                || view.sequence != snapshot.sequence
                || view.operate != rights.contains(&coder_access::Right::Operate)
                || view.review != rights.contains(&coder_access::Right::Review)
        }) {
            let projection = terminal_studio::studio::project(snapshot, &rights);
            match projection {
                Ok(mut view) => {
                    if studio.local_runs() {
                        view.local_runs =
                            snapshot.view.tasks.iter().map(|t| t.task.clone()).collect();
                    }
                    let _ = self.terminal.paper.studio.update(view);
                }
                Err(error) => {
                    self.terminal.paper.studio.revoke();
                    self.terminal.paper.studio.notice = Some(error);
                    self.screen_vertices.clear();
                    return;
                }
            }
        }
        if let Some(task) = self.terminal.paper.studio.review_task.clone() {
            let stream = snapshot.stream.clone();
            if let Some(review) = self.runtime.studio_review(&task) {
                self.terminal.paper.studio.review_task = None;
                self.terminal
                    .paper
                    .studio
                    .reviewed(terminal_studio::studio::project_review(&stream, &review));
            }
        }
        let Some(snapshot) = self.runtime.studio().source_snapshot() else {
            return;
        };
        if let Some(line) = self.terminal.paper.studio.prepare.take() {
            let workspace = self.terminal.paper.studio.workspace.as_deref().or_else(|| {
                self.terminal
                    .workshop()
                    .and_then(|opening| opening.workspace.as_deref())
            });
            let prepared = serde_json::from_slice::<coder_access::studio::Snapshot>(
                &self.terminal.paper.studio.prepare_source,
            )
            .map_err(|e| e.to_string())
            .and_then(|displayed| {
                terminal_studio::studio::prepare(
                    self.terminal.paper.studio.prepare_review.as_ref(),
                    &displayed,
                    &rights,
                    &line,
                    workspace,
                )
            });
            self.terminal.paper.studio.prepared(prepared);
        }
        if let Some(prepared) = self.terminal.paper.studio.send.take() {
            let result = if prepared.stream != snapshot.stream
                || !rights.contains(&if prepared.review {
                    coder_access::Right::Review
                } else {
                    coder_access::Right::Operate
                }) {
                Err("Studio command is stale or revoked; nothing sent.".into())
            } else {
                serde_json::from_slice::<coder_access::Operation>(&prepared.bytes)
                    .map_err(|e| e.to_string())
                    .and_then(|operation| {
                        let captured_operation = operation.clone();
                        let displayed = serde_json::from_slice::<coder_access::studio::Snapshot>(
                            &self.terminal.paper.studio.prepare_source,
                        )
                        .ok();
                        let review = self
                            .terminal
                            .paper
                            .studio
                            .prepare_review
                            .as_ref()
                            .and_then(|review| serde_json::from_slice(&review.source).ok());
                        self.runtime
                            .studio_send(operation)
                            .map(|id| {
                                if let (Some(capture), Some(displayed)) =
                                    (&self.onboarding_capture, displayed)
                                {
                                    capture.prepare(&format!("ticket:{id}"), displayed, review);
                                    self.onboarding_pending = Some((id, captured_operation));
                                }
                                self.terminal.paper.studio.ticket = Some(id);
                                format!("Submitted request {id}; awaiting the host's receipt.")
                            })
                            .map_err(|e| e.to_string())
                    })
            };
            self.terminal.paper.studio.notice = Some(result.unwrap_or_else(|e| e));
        }
        if let Some(answer) = self
            .runtime
            .studio()
            .status()
            .filter(|answer| self.terminal.paper.studio.ticket == Some(answer.ticket))
        {
            self.terminal.paper.studio.ticket = None;
            self.terminal.paper.studio.notice = Some(match &answer.result {
                Ok(coder_access::Outcome::Dispatched { receipt }) => {
                    format!("Host receipt: {} {}", receipt.operation, receipt.reference)
                }
                Ok(coder_access::Outcome::Merged { merged }) => {
                    format!("Host merge result: {merged:?}")
                }
                Ok(_) => format!(
                    "Host answered {} request {}",
                    answer.operation, answer.ticket
                ),
                Err(error) => format!("Host refused {}: {}", answer.operation, error),
            });
        }
    }

    /// Opens the panel, or closes it when it is open.
    fn toggle_panel(&mut self) {
        self.keep_console();
        self.panel = match self.panel.take() {
            Some(_) => None,
            None => Some(crate::panels::Panel::new("Agent transcript")),
        };
        self.studio_panel = None;
        self.panel_press = false;
    }

    /// Opens the Agent Studio panel `kind` over the world, in place of any
    /// open panel.
    fn open_studio_panel(&mut self, kind: StudioPanel) {
        self.keep_console();
        let review = self.studio_review(&kind);
        let studio = self.runtime.studio();
        let mut panel =
            crate::panels::Panel::new(&crate::panels::studio::title(&kind, studio.view()));
        let mut controller = crate::panels::studio::Controller::new(kind.clone());
        controller.fill(
            &mut panel,
            studio.revision(),
            studio.view(),
            review.as_ref(),
            &studio.rights(),
            studio.status(),
        );
        if kind == StudioPanel::Review && review.is_some() {
            panel.apply(crate::panels::Intent::Show(crate::panels::Tab::Changes));
        }
        if kind == StudioPanel::Console {
            controller.restore(std::mem::take(&mut self.studio_recall), &mut panel);
        }
        self.panel = Some(panel);
        self.studio_panel = Some(controller);
        self.panel_press = false;
    }

    /// Fills the open studio panel from the studio now.
    fn fill_studio_panel(&mut self) {
        let Some(kind) = self.studio_panel.as_ref().map(|c| c.kind().clone()) else {
            return;
        };
        let review = self.studio_review(&kind);
        let studio = self.runtime.studio();
        if let (Some(controller), Some(panel)) = (&mut self.studio_panel, &mut self.panel) {
            controller.fill(
                panel,
                studio.revision(),
                studio.view(),
                review.as_ref(),
                &studio.rights(),
                studio.status(),
            );
        }
    }

    /// Keeps what the open console holds, its history and unsent draft,
    /// for the next time it opens.
    fn keep_console(&mut self) {
        if let (Some(controller), Some(panel)) = (&self.studio_panel, &self.panel)
            && *controller.kind() == StudioPanel::Console
        {
            self.studio_recall = controller.recall(panel);
        }
    }

    /// Carries out an intent a studio panel's control resolved: sends its
    /// studio intent, or opens another panel, then fills the panel again.
    fn studio_panel_intent(&mut self, intent: crate::panels::Intent) {
        let Some(kind) = self.studio_panel.as_ref().map(|c| c.kind().clone()) else {
            return;
        };
        let review = self.studio_review(&kind);
        let effects = match (&mut self.studio_panel, &mut self.panel) {
            (Some(controller), Some(panel)) => {
                controller.intent(intent, panel, self.runtime.studio().view(), review.as_ref())
            }
            _ => return,
        };
        self.studio_effects(effects);
    }

    /// Carries out what a studio panel's control or key asked for, then
    /// fills the panel again.
    fn studio_effects(&mut self, effects: Vec<crate::panels::studio::Effect>) {
        use crate::panels::studio::Effect;
        for effect in effects {
            match effect {
                Effect::Send(action) => {
                    let operation =
                        action.operation(crate::zones::everglade::studio::intents::now());
                    let captured_operation = operation.clone();
                    let displayed = self.runtime.studio().source_snapshot().cloned();
                    let review = self.studio_panel.as_ref().map(|c| c.kind().clone());
                    let review = review.as_ref().and_then(|kind| self.studio_review(kind));
                    let result = self
                        .runtime
                        .studio_send(operation)
                        .map(|ticket| {
                            if let (Some(capture), Some(displayed)) =
                                (&self.onboarding_capture, displayed)
                            {
                                capture.prepare(&format!("ticket:{ticket}"), displayed, review);
                                self.onboarding_pending = Some((ticket, captured_operation));
                            }
                            ticket
                        })
                        .map_err(|error| {
                            format!(
                                "Not sent (`{}`): {}",
                                crate::panels::studio::code_word(error.code),
                                error.message
                            )
                        });
                    if let (Some(controller), Some(panel)) =
                        (&mut self.studio_panel, &mut self.panel)
                    {
                        controller.sent(result, panel);
                    }
                }
                Effect::Open(kind) => {
                    self.open_studio_panel(kind);
                    return;
                }
                Effect::Answer(decision) => {
                    self.open_studio_panel(StudioPanel::Decisions);
                    if let Some(controller) = &mut self.studio_panel {
                        controller.select(&decision);
                    }
                    self.fill_studio_panel();
                    return;
                }
                Effect::Copy(text) => {
                    let copied = arboard::Clipboard::new()
                        .and_then(|mut clipboard| clipboard.set_text(text))
                        .is_ok();
                    if !copied && let Some(controller) = &mut self.studio_panel {
                        controller.say("The clipboard did not take the path.");
                    }
                }
                Effect::Sound(on) => self.runtime.studio_sounds(on),
            }
        }
        self.fill_studio_panel();
    }

    /// Runs `intent`, which the open panel resolved: a studio control's
    /// goes to its controller, and any other to the panel, which closes on
    /// **Close**.
    fn panel_intent(&mut self, intent: crate::panels::Intent) {
        use crate::panels::Intent;
        if matches!(intent, Intent::Action(_) | Intent::Submit) {
            self.studio_panel_intent(intent);
            return;
        }
        if let Some(panel) = &mut self.panel
            && !panel.apply(intent)
        {
            self.keep_console();
            self.panel = None;
        }
    }

    /// The review the merge station shows: the newest done task whose
    /// review the studio's source holds.
    fn studio_review(&mut self, kind: &StudioPanel) -> Option<coder_access::review::TaskReview> {
        if *kind != StudioPanel::Review {
            return None;
        }
        let tasks: Vec<String> = self
            .runtime
            .studio()
            .view()
            .map(|view| {
                crate::panels::studio::reviewable(view)
                    .into_iter()
                    .map(|task| task.task.clone())
                    .collect()
            })
            .unwrap_or_default();
        tasks
            .iter()
            .find_map(|task| self.runtime.studio_review(task))
    }

    /// Refills an open studio panel when the studio changed, and forgets it
    /// once the panel closed.
    fn refresh_studio_panel(&mut self) {
        if self.panel.is_none() {
            self.studio_panel = None;
            return;
        }
        let Some(kind) = self.studio_panel.as_ref().map(|c| c.kind().clone()) else {
            return;
        };
        let revision = self.runtime.studio().revision();
        let review = self.studio_review(&kind);
        if self
            .studio_panel
            .as_ref()
            .is_some_and(|controller| controller.stale(revision, review.as_ref()))
        {
            self.fill_studio_panel();
        }
    }

    /// The studio target under the cursor in Everglade, when nothing else
    /// on screen takes the click.
    fn studio_at_cursor(&self) -> Option<StudioPanel> {
        if self.runtime.zone_loading()
            || self.map.expanded
            || self.cursor_on_map()
            || self.cursor_on_zone_hud()
            || self.cursor_on_door_hud()
            || self.layout.owns(self.cursor[0], self.cursor[1])
            || !self
                .mount
                .as_ref()
                .is_some_and(|m| m.active() && m.viewport().drawable())
        {
            return None;
        }
        let (size, aspect) = self.viewport()?;
        self.runtime
            .studio_pick(aspect, self.cursor[0] / size[0], self.cursor[1] / size[1])
    }

    /// Loads the workshop agent in Everglade, drives her pane, and puts
    /// her seat in the studio, once a frame.
    fn step_workshop(&mut self) {
        let in_glade =
            self.runtime.zone == zones::ZoneId::Everglade && self.runtime.studio().active();
        if in_glade {
            self.workshop.load();
        } else {
            self.workshop.open = false;
        }
        self.workshop.frame(&mut self.terminal);
        self.runtime.set_studio_resident(self.workshop.seats());
        self.runtime.set_studio_plan(self.workshop.plan().cloned());
        // `--place`: stand the player there once, when Everglade is up.
        if in_glade && let Some([x, z, yaw]) = self.connection_options.place.take() {
            let y = self.runtime.player.pos.y + 1.0;
            let (x, z) = (x as f32 / 100.0, z as f32 / 100.0);
            if let Err(error) = self
                .runtime
                .place_player(Vec3::new(x, y, z), (yaw as f32 / 100.0).to_radians())
            {
                eprintln!("--place: {error}");
            }
        }
        self.runtime.set_workshop_owner(self.workshop.owner());
        // `--owners-house`: in through the front door, facing her.
        if in_glade && std::mem::take(&mut self.connection_options.owners_house) {
            use zones::everglade::layout::estate;
            let [x, z] = estate::OWNERS_HOUSE.world([0.0, -12.8]);
            let [tx, tz] = estate::OWNERS_HOUSE.world([0.0, -20.0]);
            let _ = self
                .runtime
                .set_spawn(Vec3::new(x, estate::floor(), z), (tx - x).atan2(tz - z));
        }
        // `--workshop-ask`: walk up to her and say it, once she stands at
        // her desk.
        let name = crate::workshop::NAME;
        if in_glade
            && self.workshop.connected()
            && self.connection_options.workshop_ask.is_some()
            && !self.runtime.studio().seat_walking(name)
            && let Some(at) = self.runtime.studio().seat_position(name)
            && let Some(text) = self.connection_options.workshop_ask.take()
        {
            // Across her workstation in the owner's house, facing her.
            let (_, facing) = zones::everglade::layout::estate::AliceSpot::Desk.world();
            let toward = Vec3::new(facing.sin(), 0.0, facing.cos());
            self.runtime.player.pos = at + toward * crate::workshop::WALK_UP;
            self.runtime.player.yaw = (-toward.x).atan2(-toward.z);
            self.workshop.open = true;
            self.workshop.input = text;
            self.workshop_send_at = Some(Instant::now() + std::time::Duration::from_secs(6));
        }
        if self.workshop_send_at.is_some_and(|at| Instant::now() >= at) {
            self.workshop_send_at = None;
            self.workshop.open = true;
            self.workshop.key(crate::workshop::PanelKey::Enter);
        }
    }

    /// Whether the player stands near enough to the workshop agent to
    /// talk to her.
    fn near_workshop_agent(&self) -> bool {
        self.runtime.zone == zones::ZoneId::Everglade
            && self
                .runtime
                .studio()
                .seat_position(crate::workshop::NAME)
                .is_some_and(|at| {
                    crate::workshop::Workshop::within_reach(self.runtime.player.pos, at)
                })
    }

    /// Hands a key to the workshop agent's panel while it is open. Every
    /// key is consumed then. A repeated or synthetic ENTER or ESC never
    /// answers a proposal.
    fn workshop_key(&mut self, event: &winit::event::KeyEvent, synthetic: bool) -> bool {
        use crate::workshop::PanelKey;
        if !self.workshop.open {
            return false;
        }
        if event.state != ElementState::Pressed {
            return true;
        }
        let PhysicalKey::Code(code) = event.physical_key else {
            return true;
        };
        let key = match code {
            KeyCode::Enter | KeyCode::NumpadEnter => PanelKey::Enter,
            KeyCode::Escape => PanelKey::Escape,
            KeyCode::Backspace => PanelKey::Backspace,
            KeyCode::PageUp => PanelKey::PageUp,
            KeyCode::PageDown => PanelKey::PageDown,
            KeyCode::ArrowUp => PanelKey::Up,
            KeyCode::ArrowDown => PanelKey::Down,
            KeyCode::F2 => PanelKey::Memory,
            KeyCode::F3 => PanelKey::Plan,
            KeyCode::F4 => PanelKey::Journal,
            KeyCode::F7 => PanelKey::Stop,
            KeyCode::F8 => PanelKey::Pause,
            _ => {
                for c in event.text.as_deref().unwrap_or("").chars() {
                    self.workshop.key(PanelKey::Char(c));
                }
                return true;
            }
        };
        if (event.repeat || synthetic) && matches!(key, PanelKey::Enter | PanelKey::Escape) {
            return true;
        }
        self.workshop.key(key);
        true
    }

    /// Opens the terminal overlay with focus, or hides it. Hidden, its
    /// sessions keep running until Verse exits.
    fn toggle_terminal(&mut self) {
        if !self.terminal.open {
            if let Some(kind) = self.runtime.studio_panel_here()
                && self.runtime.studio().source_snapshot().is_some()
                && self
                    .runtime
                    .studio()
                    .rights()
                    .contains(&coder_access::Right::Observe)
            {
                self.open_workbench(kind);
                return;
            }
            self.terminal.clear_workshop();
            self.clear_screen();
        }
        self.terminal.toggle();
        if self.terminal.focused {
            self.take_keys_for_terminal();
        }
    }

    /// Opens the shared sheet at existing studio references. Admission is
    /// checked again on every opening; no goal or task is submitted.
    fn open_workbench(&mut self, kind: StudioPanel) {
        if !self
            .runtime
            .studio()
            .rights()
            .contains(&coder_access::Right::Observe)
        {
            self.terminal.clear_workshop();
            self.terminal.notice = Some("studio observation is not admitted".into());
            return;
        }
        let review = self.studio_review(&kind);
        let studio = self.runtime.studio();
        let opening = studio
            .source_snapshot()
            .ok_or_else(|| "the studio is unavailable".to_owned())
            .and_then(|snapshot| {
                crate::workbench_opening::context(
                    snapshot,
                    &studio.rights(),
                    &kind,
                    review.as_ref(),
                )
            });
        let disclosure = format!("{:?}", studio.rights());
        let opening = opening.and_then(|opening| {
            if let Some(mode) = self.screen_mode {
                let Some(resource) = opening
                    .task
                    .as_ref()
                    .or(opening.seat.as_ref())
                    .or(opening.goal.as_ref())
                    .cloned()
                else {
                    self.clear_screen();
                    return self.terminal.open_workshop(opening, true);
                };
                self.screen = Some(terminal_gfx::screen::Screen::new(
                    resource,
                    opening.clone(),
                    mode,
                    disclosure.clone(),
                )?);
                self.screen_vertices.clear();
                let [x, y, w, h] = self.screen_bounds.map(|value| value as f32 * self.scale);
                if let Some(screen) = &self.screen {
                    self.terminal
                        .configure_screen(screen, crate::terminal::layout::Rect::new(x, y, w, h));
                }
            }
            self.terminal.open_workshop(opening, true)
        });
        match opening {
            Ok(()) => self.take_keys_for_terminal(),
            Err(reason) => {
                self.terminal.clear_workshop();
                self.clear_screen();
                self.terminal.notice = Some(reason);
            }
        }
    }

    fn clear_screen(&mut self) {
        self.screen = None;
        self.screen_vertices.clear();
        self.screen_viewport = None;
        self.terminal.screen_rect = None;
        self.terminal.screen_watch = false;
    }

    /// Current observation admission is checked before cached private pixels are reused.
    fn validate_screen(&mut self) {
        let Some(screen) = &self.screen else {
            return;
        };
        let studio = self.runtime.studio();
        let rights = studio.rights();
        let selected = studio.source_snapshot().is_some_and(|snapshot| {
            use crate::terminal::opening::StudioPart;
            match screen.resource.part {
                Some(StudioPart::Task) => snapshot
                    .view
                    .tasks
                    .iter()
                    .any(|record| record.task == screen.resource.id),
                Some(StudioPart::Seat) => snapshot
                    .view
                    .seats
                    .iter()
                    .any(|record| record.seat == screen.resource.id),
                Some(StudioPart::Goal) => snapshot
                    .view
                    .goals
                    .iter()
                    .any(|record| record.goal == screen.resource.id),
                _ => screen.resource.id == snapshot.stream,
            }
        });
        let admitted = selected
            && studio.available()
            && studio.source_snapshot().is_some_and(|snapshot| {
                screen.admitted(
                    &snapshot.stream,
                    &format!("{rights:?}"),
                    rights.contains(&coder_access::Right::Observe),
                )
            });
        if !admitted {
            self.terminal.clear_workshop();
            self.terminal.open = false;
            self.terminal.focused = false;
            self.clear_screen();
        }
    }

    /// Stops the character and frees the cursor while the terminal types.
    fn take_keys_for_terminal(&mut self) {
        self.keys = Keys::default();
        self.climb = 0.0;
        self.capture(false);
    }

    /// Hands a key to the terminal overlay while it has focus: every key
    /// is its then, so none moves the character. Returns false when the
    /// world should handle the key.
    fn terminal_key(&mut self, key: &crate::terminal::KeyIn) -> bool {
        self.validate_screen();
        let was = self.terminal.focused;
        let taken = self.terminal.key(key);
        if !was && self.terminal.focused {
            self.take_keys_for_terminal();
        }
        taken
    }

    /// Hands a key to the panel while it has focus. Every key is consumed
    /// then, so none reaches the character controller. Returns false when
    /// the world should handle the key.
    /// `text` is what the key typed, for a panel's composer.
    fn panel_key(&mut self, code: KeyCode, pressed: bool, text: Option<&str>) -> bool {
        use crate::panels::Key;
        let shift_key = matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight);
        if shift_key {
            self.panel_shift = pressed;
        }
        if !self.panel.as_ref().is_some_and(|p| p.focused()) {
            return false;
        }
        if shift_key {
            return true;
        }
        if !pressed {
            return true;
        }
        let shift = self.panel_shift;
        let keys = match code {
            KeyCode::Escape => vec![Key::Escape],
            KeyCode::Tab if shift => vec![Key::BackTab],
            KeyCode::Tab => vec![Key::Tab],
            KeyCode::ArrowUp => vec![Key::Up],
            KeyCode::ArrowDown => vec![Key::Down],
            KeyCode::PageUp => vec![Key::PageUp],
            KeyCode::PageDown => vec![Key::PageDown],
            KeyCode::Home => vec![Key::Home],
            KeyCode::End => vec![Key::End],
            KeyCode::Enter | KeyCode::NumpadEnter if shift => vec![Key::NewLine],
            KeyCode::Enter | KeyCode::NumpadEnter => vec![Key::Enter],
            KeyCode::Backspace => vec![Key::Backspace],
            _ => match text.filter(|text| !text.chars().any(char::is_control)) {
                Some(text) if !text.is_empty() => text.chars().map(Key::Char).collect(),
                _ => vec![Key::Other],
            },
        };
        for key in keys {
            self.panel_key_one(key);
        }
        true
    }

    /// Hands one key to the open panel: a studio panel's controller takes
    /// it first, for completion, history, number keys, and the review's
    /// keys; else the panel does.
    fn panel_key_one(&mut self, key: crate::panels::Key) {
        if let Some(kind) = self.studio_panel.as_ref().map(|c| c.kind().clone()) {
            let review = self.studio_review(&kind);
            let now = std::time::Instant::now();
            let taken = match (&mut self.studio_panel, &mut self.panel) {
                (Some(controller), Some(panel)) => controller.key(
                    key,
                    panel,
                    self.runtime.studio().view(),
                    review.as_ref(),
                    now,
                ),
                _ => None,
            };
            if let Some(effects) = taken {
                self.studio_effects(effects);
                return;
            }
        }
        if let Some(intent) = self.panel.as_mut().and_then(|panel| panel.key(key)) {
            self.panel_intent(intent);
        }
    }

    /// Gives the panel a left-button press or release. Returns true when
    /// the panel took it, so the world does not.
    fn panel_button(&mut self, pressed: bool) -> bool {
        let at = self.cursor.map(|v| v / self.scale);
        let Some(panel) = &mut self.panel else {
            return false;
        };
        if pressed {
            let was = panel.focused();
            self.panel_press = panel.press(at);
            if self.panel_press && !was {
                // The character stops when the panel takes focus.
                self.keys = Keys::default();
                self.capture(false);
            }
            return self.panel_press;
        }
        if !std::mem::take(&mut self.panel_press) {
            return false;
        }
        if let Some(intent) = panel.release(at) {
            self.panel_intent(intent);
        }
        true
    }

    /// The replay's visits so far, as transcript rows, for the panel.
    fn panel_rows(&self) -> Vec<rust_native::Node<()>> {
        let Some(r) = &self.replay else {
            return Vec::new();
        };
        let t = r.clock.elapsed_ms;
        let Some(current) = r.mine.current(t) else {
            return Vec::new();
        };
        let done = r.mine.done(t);
        let first = (current + 1).saturating_sub(200);
        r.mine.visits[first..=current.min(r.mine.visits.len().saturating_sub(1))]
            .iter()
            .enumerate()
            .map(|(i, visit)| {
                let index = first + i;
                crate::panels::tool(
                    &format!("visit-{index}"),
                    visit.place.name(),
                    &visit.what,
                    "",
                    if index == current && !done {
                        rust_native::ToolState::Running
                    } else {
                        rust_native::ToolState::Done
                    },
                )
            })
            .collect()
    }

    fn plaza_interactive(&self) -> bool {
        self.runtime.is_plaza() && !self.runtime.zone_loading() && self.chamber.is_none()
    }

    /// Opens the pinned chamber when the player walks through the RITUAL
    /// arch, and puts the player back in front of it when that window
    /// closes. The Grid's presence, chat, feed, XP, and board pause while
    /// the chamber is open, as in a zone.
    fn tick_ritual(&mut self) {
        if let Some(child) = &mut self.chamber {
            match child.try_wait() {
                Ok(None) => return,
                Ok(Some(status)) => {
                    if !status.success() {
                        self.offline_log.push(chat::Line::system(format!(
                            "The chamber closed with {status}"
                        )));
                    }
                }
                Err(error) => self
                    .offline_log
                    .push(chat::Line::system(format!("The chamber was lost: {error}"))),
            }
            self.chamber = None;
            if let Err(error) = self.runtime.return_from_ritual() {
                self.offline_log.push(chat::Line::system(error));
            }
            return;
        }
        let Some(config) = self.runtime.take_ritual_crossing() else {
            return;
        };
        match crate::ritual::open(&config, &self.connection_options.profile) {
            Ok(child) => {
                self.chamber = Some(child);
                self.sync_zone_services(true);
            }
            Err(error) => self.offline_log.push(chat::Line::system(error)),
        }
    }

    /// A zone change drops every plaza subscription before the new pose ticks.
    /// Returning reconnects the saved profile without applying relay spawn state.
    fn sync_zone_services(&mut self, active: bool) {
        let allowed = active && self.plaza_interactive();
        if !allowed && !self.plaza_services_paused {
            if let Some(session) = &mut self.session {
                session.leave(&self.plaza_presence.0, &self.plaza_presence.1);
            }
            self.session = None;
            self.feed = None;
            self.xp = None;
            self.update_gym(false);
            self.gym_view = None;
            self.replay = None;
            self.picker = None;
            self.board_open = false;
            self.chat.open = false;
            self.agent_says = None;
            self.presented_entities = crate::mesh::Mesh::default();
            self.plaza_services_paused = true;
        } else if allowed && self.plaza_services_paused {
            self.plaza_services_paused = false;
            if let Some(relay) = &self.connection_options.relay {
                match Session::start(&self.connection_options.profile, relay) {
                    Ok(session) => {
                        let session = with_saved_blocklist(session);
                        self.zone_operators = zone_operators_for(Some(&session));
                        self.session = Some(session);
                    }
                    Err(error) => self.offline_log.push(chat::Line::system(error)),
                }
                self.feed = Some(Feed::start());
            }
            if let Some(relay) = self
                .connection_options
                .xp_relay
                .as_ref()
                .or(self.connection_options.relay.as_ref())
            {
                self.xp = Some(xp::Board::start(
                    relay,
                    &self.connection_options.xp_referees,
                    self.session.as_ref().map(Session::signer),
                ));
            }
            self.update_title();
        }
        self.sync_zone_presence(active);
    }

    /// In a zone, presence alone joins the zone's shared NIP-MV world, so
    /// players who walked through the same arch see each other there; the
    /// plaza's chat, feed, XP, and board stay paused.
    fn sync_zone_presence(&mut self, active: bool) {
        let wanted = (active && !self.runtime.is_plaza() && !self.runtime.zone_loading())
            .then(|| self.runtime.zone.world_id());
        let Some(relay) = self.connection_options.relay.clone() else {
            return;
        };
        if self
            .session
            .as_ref()
            .is_some_and(|session| Some(session.world()) != wanted && !self.plaza_interactive())
        {
            if let Some(session) = &mut self.session {
                session.leave(&self.runtime.player, &self.runtime.agent);
            }
            self.session = None;
            self.presented_entities = crate::mesh::Mesh::default();
        }
        let Some(world) = wanted else {
            return;
        };
        if self.session.is_some() {
            return;
        }
        let started = crate::identity::load_or_create(
            &crate::identity::home(),
            &self.connection_options.profile,
        )
        .and_then(|id| Session::start_presence(id, &relay, world));
        match started {
            Ok(session) => {
                let mut session = with_saved_blocklist(session);
                session.set_display_name(Some(&self.connection_options.profile));
                session.crowd.set_live_only(true);
                self.session = Some(session);
            }
            Err(error) => self.offline_log.push(chat::Line::system(error)),
        }
    }

    fn zone_action(&mut self, action: ZoneIntent) {
        if action == ZoneIntent::Interact {
            if let Some(kind) = self.runtime.studio_panel_here() {
                self.open_studio_panel(kind);
            }
            return;
        }
        if self.runtime.zone_intent(action).is_ok() {
            // Only a transition resets input; a lab knob keeps held keys.
            if !matches!(
                action,
                ZoneIntent::Enter | ZoneIntent::Return | ZoneIntent::Cancel | ZoneIntent::Retry
            ) {
                return;
            }
            self.stop_map();
            self.keys = Keys::default();
            self.capture(false);
            self.sync_zone_services(true);
        }
    }

    /// Start loading Everglade from the plaza without walking to its
    /// portal, then hand off as a portal entry does.
    /// Starts or restarts the `--everglade` load while it is pending and the
    /// window is in front, and drops the request once Everglade is in.
    /// `--grove` does the same for the Grove, which loads Everglade's pack.
    fn open_pending_everglade(&mut self) {
        let Some(zone) = self.everglade_pending else {
            return;
        };
        // In, or failed with its error on screen: a failed load is not
        // retried, so the player sees why.
        if self.runtime.zone == zone
            || self.runtime.zone_load_state() == crate::zones::LoadState::Failed
        {
            self.everglade_pending = None;
        } else if self.window_focused
            && self.runtime.is_plaza()
            && !self.runtime.zone_loading()
            && self.runtime.everglade_loader_idle()
        {
            self.open_everglade(zone);
        }
    }

    fn open_everglade(&mut self, zone: zones::ZoneId) {
        let entered = if zone == zones::ZoneId::MeteorStressTest {
            self.runtime.enter_meteor_stress_test()
        } else if zone == zones::ZoneId::MeteorShowcase {
            self.runtime.enter_meteor_showcase()
        } else if zone == zones::ZoneId::Grove {
            self.runtime.enter_grove()
        } else if zone == zones::ZoneId::Crypt {
            self.runtime.enter_crypt()
        } else if zone == zones::ZoneId::Coast {
            self.runtime.enter_coast()
        } else if zone == zones::ZoneId::WaterLab {
            self.runtime.enter_water_lab()
        } else {
            self.runtime.enter_everglade()
        };
        match entered {
            Ok(()) => {
                self.stop_map();
                self.keys = Keys::default();
                self.capture(false);
                self.sync_zone_services(true);
            }
            Err(error) => eprintln!("verse: cannot open {}: {error}", zone.label()),
        }
    }

    /// Backgrounding invalidates loading and input even when no frame can run.
    /// Forgets the mouse buttons held. A window manager can take a press
    /// or its release for itself (a double-click on the title bar or the
    /// zoom button maximizes the window), which would leave a button
    /// looking held: the view would keep orbiting, hotbar cards would not
    /// show, and clicks would not register.
    fn release_buttons(&mut self) {
        if self.keys.left_button || self.keys.right_button {
            self.keys.left_button = false;
            self.keys.right_button = false;
            self.capture(false);
        }
        self.swing_press = None;
    }

    fn suspend_world(&mut self) {
        self.runtime.zone_cancel_loading();
        self.sync_zone_services(false);
        self.stop_map();
        self.update_gym(false);
        self.runtime.update_studio(false, 0.0);
        self.keys = Keys::default();
        self.capture(false);
    }

    fn cursor_on_zone_hud(&self) -> bool {
        self.zone_frame.as_ref().is_some_and(|frame| {
            let [x, y] = self.cursor.map(|v| v / self.scale);
            let [left, top, width, height] = frame.frame;
            frame.visible && x >= left && x <= left + width && y >= top && y <= top + height
        })
    }

    fn portal_at_cursor(&self) -> bool {
        if self.runtime.zone_loading()
            || self.map.expanded
            || self.cursor_on_map()
            || self.cursor_on_zone_hud()
            || self.cursor_on_door_hud()
            || self.layout.owns(self.cursor[0], self.cursor[1])
            || !self
                .mount
                .as_ref()
                .is_some_and(|m| m.active() && m.viewport().drawable())
        {
            return false;
        }
        let Some((size, aspect)) = self.viewport() else {
            return false;
        };
        self.runtime.zone_hit_with_entities(
            aspect,
            self.cursor[0] / size[0],
            self.cursor[1] / size[1],
            &self.presented_entities,
        )
    }

    /// The spatial and lifecycle gate owns all Gym reads. Merely starting Verse
    /// does not read a connection file, open a Gym socket, or list local runs.
    fn update_gym(&mut self, active: bool) {
        let inside = active && self.plaza_interactive() && self.runtime.gym(1.0).inside;
        if !inside {
            if let Some(board) = &mut self.gym {
                board.set_active(false);
            }
            self.gym_open = false;
            return;
        }
        if self.gym.is_none() && !self.gym_identity_attempted {
            self.gym_identity_attempted = true;
            match crate::identity::load_or_create(&crate::identity::home(), &self.gym_profile) {
                Ok(identity) => self.gym = Some(crate::gym::Board::new(identity.secret, false)),
                Err(error) => {
                    self.gym_notice = Some(error);
                    return;
                }
            }
        }
        let Some(board) = self.gym.as_mut() else {
            return;
        };
        if !self.gym_connection_attempted {
            self.gym_connection_attempted = true;
            let result = self.gym_connection.as_ref().map_or(Ok(()), |path| {
                read_gym_connection(path).and_then(|code| board.configure(&code))
            });
            if let Err(error) = result {
                self.gym_notice = Some(error);
                // A failed configured read is retried only by an explicit key.
                board.set_active(false);
                return;
            }
        }
        board.set_active(true);
        board.poll();
        if self
            .gym_view
            .as_ref()
            .is_none_or(|v| v.revision != board.revision())
        {
            let mut view = board.view();
            prioritize_gym_runs(&mut view);
            let count = if self.gym_recipes {
                view.recipes.len()
            } else {
                view.runs.len()
            };
            self.gym_selected = self.gym_selected.min(count.saturating_sub(1));
            self.gym_view = Some(view);
        }
    }

    fn gym_key(&mut self, code: KeyCode, pressed: bool) -> bool {
        if !self.plaza_interactive() {
            return false;
        }
        if code == KeyCode::KeyG && pressed && self.runtime.gym(1.0).inside {
            self.stop_map();
            self.gym_open = !self.gym_open;
            self.chat.open = false;
            self.board_open = false;
            self.picker = None;
            self.keys = Keys::default();
            self.capture(false);
            return true;
        }
        if !self.gym_open {
            return false;
        }
        if !pressed {
            return true;
        }
        if code == KeyCode::F5 {
            self.gym_identity_attempted = false;
            self.gym_connection_attempted = false;
            self.gym_notice = None;
            return true;
        }
        match code {
            KeyCode::PageUp => {
                self.gym_scroll = self.gym_scroll.saturating_sub(8);
                return true;
            }
            KeyCode::PageDown => {
                self.gym_scroll = self.gym_scroll.saturating_add(8).min(512);
                return true;
            }
            _ => {}
        }
        let Some(board) = &mut self.gym else {
            if code == KeyCode::Escape {
                self.gym_open = false;
            }
            return true;
        };
        let mut view = board.view();
        prioritize_gym_runs(&mut view);
        let count = if self.gym_recipes {
            view.recipes.len()
        } else {
            view.runs.len()
        };
        let result = match code {
            KeyCode::Escape => {
                self.gym_open = false;
                board.close_detail();
                Ok(())
            }
            KeyCode::Tab => {
                self.gym_recipes = !self.gym_recipes;
                self.gym_selected = 0;
                self.gym_scroll = 0;
                board.close_detail();
                Ok(())
            }
            KeyCode::ArrowUp => {
                self.gym_selected = self.gym_selected.saturating_sub(1);
                self.gym_scroll = 0;
                board.close_detail();
                Ok(())
            }
            KeyCode::ArrowDown => {
                self.gym_selected = self
                    .gym_selected
                    .saturating_add(1)
                    .min(count.saturating_sub(1));
                self.gym_scroll = 0;
                board.close_detail();
                Ok(())
            }
            KeyCode::Backspace => {
                self.gym_scroll = 0;
                board.close_detail();
                Ok(())
            }
            KeyCode::KeyY => board.retry_launch(),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                self.gym_scroll = 0;
                if self.gym_recipes {
                    if view.selected_recipe.is_some() {
                        board.confirm_launch()
                    } else if let Some(recipe) = view.recipes.get(self.gym_selected) {
                        board.select_recipe(&recipe.id)
                    } else {
                        Ok(())
                    }
                } else if let Some(run) = view.runs.get(self.gym_selected) {
                    board.select_run(&run.id)
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            self.gym_notice = Some(error);
        } else {
            self.gym_notice = None;
        }
        true
    }

    /// Opens the replay list, reading the retained runs the first time.
    fn open_picker(&mut self) {
        if !self.plaza_interactive() {
            return;
        }
        self.stop_map();
        self.board_open = false;
        let choices = self
            .choices
            .get_or_insert_with(replay::beats_winner_runs)
            .clone();
        self.picker = Some(Picker {
            choices,
            selected: 0,
            notice: None,
        });
    }

    /// Handles a key press while the replay list is open. Returns true
    /// when the list took it.
    fn picker_key(&mut self, code: KeyCode) -> bool {
        let Some(picker) = &mut self.picker else {
            return false;
        };
        let last = picker.choices.len().saturating_sub(1);
        match code {
            KeyCode::ArrowUp | KeyCode::KeyW => picker.selected = picker.selected.saturating_sub(1),
            KeyCode::ArrowDown | KeyCode::KeyS => picker.selected = (picker.selected + 1).min(last),
            KeyCode::PageUp => picker.selected = picker.selected.saturating_sub(5),
            KeyCode::PageDown => picker.selected = (picker.selected + 5).min(last),
            KeyCode::Enter | KeyCode::NumpadEnter => {
                let Some(choice) = picker.choices.get(picker.selected) else {
                    return true;
                };
                match Replay::load(&choice.run, self.runtime.agent.pos) {
                    Ok(r) => self.start_replay(r),
                    Err(e) => {
                        if let Some(p) = &mut self.picker {
                            p.notice = Some(format!("Can't replay it: {e}"));
                        }
                    }
                }
            }
            KeyCode::Escape | KeyCode::KeyR => self.picker = None,
            _ => return false,
        }
        true
    }

    fn start_replay(&mut self, r: Replay) {
        self.stop_map();
        self.ghost = Agent::at(Place::Plaza.stand(true), 0.0);
        self.finished = [false; 2];
        self.replay = Some(r);
        self.picker = None;
    }

    /// Advances the replay and flies both spades toward their places, or
    /// lets the agent follow the player when nothing is replaying.
    fn step_agents(&mut self, dt: f32) {
        if !self.plaza_interactive() {
            return;
        }
        let Some(r) = &mut self.replay else {
            return;
        };
        r.tick(dt);
        let [mine, ghost] = r.carrots();
        self.runtime.agent.visit(mine, dt);
        self.ghost.visit(ghost, dt);
        let t = r.clock.elapsed_ms;
        let passed = |track: &replay::Track| track.result.starts_with("passed");
        if !self.finished[0] && r.mine.done(t) {
            self.finished[0] = true;
            if passed(&r.mine) {
                self.runtime.agent.celebrate();
            }
        }
        if let Ok(g) = &r.ghost
            && !self.finished[1]
            && g.done(t)
        {
            self.finished[1] = true;
            if passed(g) {
                self.ghost.celebrate();
            }
        }
        if t < 1.0 {
            self.finished = [false; 2];
        }
    }

    /// Records the player as offline on the relay before quitting.
    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        self.terminal.shutdown();
        self.runtime.zone_cancel_loading();
        self.sync_zone_services(false);
        self.update_gym(false);
        if let Some(session) = &mut self.session {
            session.leave(&self.runtime.player, &self.runtime.agent);
        }
        self.session = None;
        event_loop.exit();
    }

    fn update_title(&mut self) {
        let title = if !self.runtime.is_plaza() {
            format!("Verse — {} — local", self.runtime.zone_label())
        } else {
            match &self.session {
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
        if !self.plaza_interactive() {
            return;
        }
        self.stop_map();
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
        if !self.plaza_interactive() {
            return;
        }
        let now = Instant::now();
        let command = chat::parse(text, &self.method);
        if let chat::Command::Send(Channel::Agent, text) = &command {
            self.ask_agent(text);
            return;
        }
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
            chat::Command::Block(name) => Some(block_by_name(session, &name, true)),
            chat::Command::Unblock(name) => Some(block_by_name(session, &name, false)),
        };
        if let Some(notice) = notice {
            session.log.push(chat::Line::system(notice));
        }
    }

    /// Sends a line to the player's own agent: logged privately, answered
    /// by the model, and spoken in a bubble over the spade.
    fn ask_agent(&mut self, text: &str) {
        if !self.plaza_interactive() {
            return;
        }
        let me = self
            .session
            .as_ref()
            .map_or_else(|| "you".to_owned(), |s| s.profile().to_owned());
        let line = chat::Line {
            channel: Some(Channel::Agent),
            from: me,
            to: None,
            text: text.to_owned(),
            note: None,
        };
        match &mut self.session {
            Some(s) => s.log.push(line),
            None => self.offline_log.push(line),
        }
        let surroundings = self.surroundings();
        self.brain.ask(brain::Ask {
            text: text.to_owned(),
            surroundings,
        });
        self.agent_says = Some(("…".to_owned(), None));
        let head = self.runtime.player.pos + Vec3::Y * 1.7;
        self.runtime.agent.greet(head);
    }

    /// What the agent can see, in plain sentences.
    fn surroundings(&self) -> String {
        let pos = self.runtime.player.pos;
        let mut out = vec![
            format!(
                "- You and your player are in the {} of world {}, at x {:.0}, z {:.0}.",
                chat::zone_name(chat::zone_of(pos)),
                session::WORLD,
                pos.x,
                pos.z
            ),
            format!(
                "- The tall amber pylon is {:.0} m away. Wireframe towers ring the plaza.",
                pos.distance(world::PYLON)
            ),
        ];
        if let Some(s) = &self.session {
            let now = Instant::now();
            let mut near: Vec<(f32, String)> = s
                .crowd
                .shown(now)
                .into_iter()
                .filter(|e| e.role == "avatar")
                .map(|e| {
                    let d = e.pos.distance(pos);
                    let state = if e.online {
                        "online"
                    } else {
                        "resting, offline"
                    };
                    (
                        d,
                        format!("{} ({state}, {d:.0} m away)", s.name_of(&e.pubkey)),
                    )
                })
                .collect();
            near.sort_by(|a, b| a.0.total_cmp(&b.0));
            if near.is_empty() {
                out.push("- No other players are around.".into());
            } else {
                let names: Vec<String> = near.into_iter().take(6).map(|(_, n)| n).collect();
                out.push(format!("- Players you can see: {}.", names.join("; ")));
            }
            let recent: Vec<String> = s
                .log
                .world
                .iter()
                .chain(&s.log.personal)
                .filter(|l| !matches!(l.channel, Some(Channel::Agent) | Some(Channel::Pm(_))))
                .rev()
                .take(5)
                .map(|l| {
                    if l.from.is_empty() {
                        l.text.clone()
                    } else {
                        format!("{}: {}", l.from, l.text)
                    }
                })
                .collect();
            if !recent.is_empty() {
                out.push(format!("- Recent public chat: {}", recent.join(" | ")));
            }
        }
        if let Some(feed) = &self.feed {
            let notes = feed.recent(3);
            if !notes.is_empty() {
                out.push(format!(
                    "- Stand-ins for people posting on Nostr gather around the pylon. Latest: {}",
                    notes.join(" | ")
                ));
            }
        }
        out.join("\n")
    }

    /// Talks to Everglade's villager `id` (`crate::town_talk`): its words
    /// show over it now, or when the model's reply comes.
    fn talk_to_villager(&mut self, id: &str) {
        let Some(time) = self.runtime.town_time() else {
            return;
        };
        let (roster, _) = zones::everglade::townsfolk::roster();
        let Some(villager) = roster.villager(id) else {
            return;
        };
        // The pool's news first: every villager has heard it (P4).
        let news = self.runtime.compute().rumor(time.day);
        let rumors: Vec<&townsfolk::rumor::Rumor> = news
            .iter()
            .chain(zones::everglade::townsfolk::known_rumors(id, time))
            .collect();
        let profile = &self.connection_options.profile;
        let talk = self.town_talk.get_or_insert_with(|| {
            crate::town_talk::TownTalk::new(
                profile,
                Some(crate::town_talk::save_path(
                    &crate::identity::home(),
                    profile,
                )),
                brain::Voice::start(),
            )
        });
        if talk.waiting() {
            return;
        }
        let said = talk.talk(&villager.npc, &rumors, roster.town.budgets, time);
        self.runtime
            .villager_say(id, said.as_deref().unwrap_or("..."));
    }

    /// Shows a villager's model reply once it comes.
    fn hear_villagers(&mut self) {
        if let Some(talk) = &mut self.town_talk
            && let Some((id, text)) = talk.poll()
        {
            self.runtime.villager_say(&id, &text);
        }
    }

    /// Streams the agent's reply into its bubble and the personal window.
    fn hear_agent(&mut self, now: Instant) {
        for reply in self.brain.drain() {
            let (text, done) = match reply {
                brain::Reply::Piece(piece) => {
                    let so_far = match &self.agent_says {
                        Some((t, None)) if t != "…" => format!("{t}{piece}"),
                        _ => piece,
                    };
                    (so_far, false)
                }
                brain::Reply::Done(text) | brain::Reply::Failed(text) => (text, true),
            };
            if done {
                let line = chat::Line {
                    channel: Some(Channel::Agent),
                    from: "agent".into(),
                    to: None,
                    text: text.clone(),
                    note: None,
                };
                match &mut self.session {
                    Some(s) => s.log.push(line),
                    None => self.offline_log.push(line),
                }
                let hold = Duration::from_secs(6) + Duration::from_millis(60 * text.len() as u64);
                self.agent_says = Some((text, Some(now + hold.min(Duration::from_secs(20)))));
            } else {
                self.agent_says = Some((text, None));
            }
        }
        if self
            .agent_says
            .as_ref()
            .is_some_and(|(_, until)| until.is_some_and(|u| u <= now))
        {
            self.agent_says = None;
        }
    }

    /// Everglade draws no map and no zone panel, only the movement hotbar
    /// (owner, 2026-10-04): the glade is the screen. It has no arch back
    /// (owner, 2026-10-05); G leaves for the Grid. A load in progress or a
    /// failed one still shows the panel.
    /// The crypt walks as Everglade does, with its hotbar and no map.
    fn in_bare_everglade(&self) -> bool {
        matches!(
            self.runtime.zone,
            zones::ZoneId::Everglade
                | zones::ZoneId::Crypt
                | zones::ZoneId::MeteorStressTest
                | zones::ZoneId::MeteorShowcase
        ) && self.runtime.zone_load_state() == zones::LoadState::Idle
    }

    /// The Grove shows its hotbar and nothing else over the meadow, as
    /// Everglade does.
    fn in_bare_grove(&self) -> bool {
        self.runtime.zone == zones::ZoneId::Grove
            && self.runtime.zone_load_state() == zones::LoadState::Idle
    }

    /// The Water Lab, loaded, which shows its own spell bar.
    fn in_water_lab(&self) -> bool {
        self.runtime.zone == zones::ZoneId::WaterLab
            && self.runtime.zone_load_state() == zones::LoadState::Idle
    }

    /// The Water Lab's bar slot under `at`, in logical units.
    fn water_hotbar_at(&self, at: [f32; 2]) -> Option<usize> {
        let (size, _) = self.viewport()?;
        zones::water::hotbar::slot_under(at, size.map(|v| v / self.scale), HOTBAR_BOTTOM)
    }

    /// How the Grove's bar lays out on a screen of `size` logical points.
    fn grove_layout(&self, size: [f32; 2]) -> zones::grove::hotbar::Layout {
        zones::grove::hotbar::Layout::for_screen(size, self.grove_row)
    }

    /// What the Grove's bar has under `at`, in logical units.
    fn grove_hotbar_at(&self, at: [f32; 2]) -> Option<zones::grove::hotbar::Hit> {
        let (size, _) = self.viewport()?;
        let size = size.map(|v| v / self.scale);
        zones::grove::hotbar::hit(at, size, HOTBAR_BOTTOM, self.grove_layout(size))
    }

    /// The hotbar slot whose card shows this frame on a screen of `size`
    /// logical points: the one the pointer has rested on for the hover
    /// delay, while no button is held.
    fn hotbar_tip(&mut self, size: [f32; 2]) -> Option<usize> {
        let at = self.cursor.map(|v| v / self.scale);
        let free = !self.keys.left_button && !self.keys.right_button;
        let slot = if !free {
            None
        } else if self.in_bare_everglade() && self.runtime.demolition_bar().is_some() {
            zones::everglade::demolition::hotbar::slot_under(at, size, HOTBAR_BOTTOM)
        } else if self.in_bare_everglade()
            && let Some(slots) = self.runtime.everglade_hotbar()
        {
            zones::everglade::hotbar::slot_under(at, size, HOTBAR_BOTTOM, slots.len())
        } else if self.in_bare_grove() && self.runtime.grove_bar().is_some() {
            zones::grove::hotbar::slot_under(at, size, HOTBAR_BOTTOM, self.grove_layout(size))
        } else if self.in_water_lab() {
            zones::water::hotbar::slot_under(at, size, HOTBAR_BOTTOM)
        } else {
            None
        };
        self.slot_tip
            .update(slot, self.started.elapsed().as_secs_f32())
    }

    /// Aims the Water Lab's orbs and bolts at what lies under the cursor,
    /// or ahead of the player while the cursor rests on the spell bar.
    fn aim_water_lab(&mut self) {
        if !self.in_water_lab() {
            return;
        }
        let Some((size, aspect)) = self.viewport() else {
            return;
        };
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return;
        }
        let logical = size.map(|v| v / self.scale);
        let at = self.cursor.map(|v| v / self.scale);
        let [left, top, width, _] = zones::water::hotbar::frame(logical, HOTBAR_BOTTOM);
        let on_bar = at[1] >= top - 8.0 && at[0] >= left - 8.0 && at[0] <= left + width + 8.0;
        let point = (!on_bar).then(|| {
            (
                (self.cursor[0] / size[0]).clamp(0.0, 1.0),
                (self.cursor[1] / size[1]).clamp(0.0, 1.0),
            )
        });
        self.runtime.water_aim(aspect, point);
    }

    /// Puts Meteor Swarm's circle on the ground under the cursor while
    /// the demolition yard aims it.
    fn aim_meteor_swarm(&mut self) {
        if !self.runtime.demolition_targeting() {
            return;
        }
        let Some((size, aspect)) = self.viewport() else {
            return;
        };
        if size[0] <= 0.0 || size[1] <= 0.0 {
            return;
        }
        self.runtime.demolition_aim(
            aspect,
            (self.cursor[0] / size[0]).clamp(0.0, 1.0),
            (self.cursor[1] / size[1]).clamp(0.0, 1.0),
        );
    }

    /// Everglade's hotbar slot under `at`, in logical units.
    fn hotbar_at(&self, at: [f32; 2]) -> Option<ZoneIntent> {
        let (size, _) = self.viewport()?;
        let size = size.map(|v| v / self.scale);
        if self.runtime.in_demolition() {
            return zones::everglade::demolition::hotbar::hit(at, size, HOTBAR_BOTTOM);
        }
        zones::everglade::hotbar::hit_ordered(
            at,
            size,
            HOTBAR_BOTTOM,
            &self.runtime.everglade_hotbar_order(),
        )
    }

    /// How many slots Everglade's hotbar shows: its five, or seven in the
    /// Meteor Stress Test or with the dev build's destruction switched on.
    fn everglade_bar_len(&self) -> usize {
        self.runtime
            .everglade_hotbar()
            .map_or(zones::everglade::hotbar::COUNT, |slots| slots.len())
    }

    /// Everglade's tray frame in logical points on a screen of `size`.
    fn everglade_tray(&self, size: [f32; 2]) -> [f32; 4] {
        zones::everglade::hotbar::frame_of(size, HOTBAR_BOTTOM, self.everglade_bar_len())
    }

    fn map_visible(&self) -> bool {
        !self.in_bare_everglade()
            && !self.in_bare_grove()
            && !self.runtime.zone_loading()
            && !self.chat.open
            && !self.board_open
            && !self.gym_open
            && self.picker.is_none()
            && self.replay.is_none()
    }

    fn stop_map(&mut self) {
        self.zone_press = None;
        self.zone_hud.clear_contacts();
        self.zone_frame = None;
        self.companion_press = None;
        self.door_press = None;
        self.door_hud.clear_contacts();
        self.door_frame = None;
        self.runtime.doors.cancel_transient();
        self.runtime.cancel_navigation();
        self.map.expanded = false;
        self.map.clear_contacts();
        self.map_frame = None;
    }

    fn map_action(&mut self, action: MapAction) {
        self.companion_press = None;
        self.door_press = None;
        self.door_hud.clear_contacts();
        self.door_frame = None;
        match action {
            MapAction::Toggle => {
                self.map.expanded = !self.map.expanded;
            }
            MapAction::Cancel => {
                self.runtime.cancel_navigation();
                self.map_error = None;
            }
            MapAction::Walk(destination) => {
                // A held movement key still wins over the newly selected route.
                self.capture(false);
                self.map_error = self
                    .runtime
                    .navigate_to(destination)
                    .err()
                    .map(|error| error.to_string());
                if self.map_error.is_none() {
                    self.map.expanded = false;
                }
            }
        }
        self.map.clear_contacts();
        self.map_frame = None;
    }

    fn cursor_on_map(&self) -> bool {
        let Some(frame) = self.map_frame.as_ref().filter(|frame| frame.visible) else {
            return false;
        };
        let [x, y] = self.cursor.map(|value| value / self.scale);
        let [left, top, width, height] = frame.frame;
        x >= left && x <= left + width && y >= top && y <= top + height
    }

    /// A left click at the cursor, if it lands on the HUD. Returns true
    /// when the HUD took it.
    fn click(&mut self) -> bool {
        if !self.plaza_interactive() {
            return false;
        }
        let [x, y] = self.cursor;
        if !self.layout.owns(x, y) {
            return false;
        }
        if let Some(i) = self.layout.pills.iter().position(|r| r.contains(x, y)) {
            if let Some(method) = self.methods().get(i).cloned() {
                self.method = method;
                self.open_chat("");
            }
        } else if let Some(i) = self.layout.tabs.iter().position(|r| r.contains(x, y)) {
            self.left_tab = if i == 0 {
                hud::LeftTab::World
            } else {
                hud::LeftTab::Nostr
            };
        } else if self.layout.bar.contains(x, y) {
            self.open_chat("");
        }
        true
    }

    fn key(&mut self, code: KeyCode, pressed: bool, _event_loop: &ActiveEventLoop) {
        if self.runtime.zone_loading() {
            if pressed && code == KeyCode::Escape {
                self.zone_action(ZoneIntent::Cancel);
            }
            return;
        }
        // Holding the zone's Levitate key (the number key of its slot in
        // the displayed order) or L rises; letting go hovers, and a tap
        // while hovering falls.
        if (code == KeyCode::KeyL
            || zones::everglade::hotbar::key_intent(
                hotbar_number(code),
                &self.runtime.everglade_hotbar_order(),
            ) == Some(ZoneIntent::Levitate))
            && self.in_bare_everglade()
            && !self.runtime.in_demolition()
            && (!pressed || (!self.chat.open && !self.map.expanded))
        {
            let _ = self.runtime.everglade_levitate(pressed);
            return;
        }
        // While levitating in Everglade, holding X descends until it is let
        // go.
        // As a flying Wild Shape form, holding Space climbs the same way.
        let climb = match code {
            KeyCode::KeyX => -1.0,
            KeyCode::Space if self.runtime.grove_form_flies() => 1.0,
            _ => 0.0,
        };
        if climb != 0.0 && (self.in_bare_everglade() || self.in_bare_grove()) && !self.chat.open {
            // A flying form takes off with the climb.
            if pressed && (self.runtime.everglade_levitating() || climb > 0.0) {
                self.climb = climb;
                return;
            }
            if !pressed && self.climb == climb {
                self.climb = 0.0;
                return;
            }
        }
        // The Grove's hotbar: 1 to 9, 0, -, and = cast what their slots
        // hold. Every press casts, and a held key recasts until it is let
        // go.
        if self.in_bare_grove() && (!pressed || (!self.chat.open && !self.map.expanded)) {
            match code {
                KeyCode::ControlLeft | KeyCode::ControlRight => self.grove_ctrl = pressed,
                KeyCode::AltLeft | KeyCode::AltRight => self.grove_alt = pressed,
                _ => {}
            }
            let key = match code {
                KeyCode::Digit0 => Some('0'),
                KeyCode::Digit1 => Some('1'),
                KeyCode::Digit2 => Some('2'),
                KeyCode::Digit3 => Some('3'),
                KeyCode::Digit4 => Some('4'),
                KeyCode::Digit5 => Some('5'),
                KeyCode::Digit6 => Some('6'),
                KeyCode::Digit7 => Some('7'),
                KeyCode::Digit8 => Some('8'),
                KeyCode::Digit9 => Some('9'),
                KeyCode::Minus => Some('-'),
                KeyCode::Equal => Some('='),
                _ => None,
            };
            // Shift, Ctrl, and Alt choose the row when the key goes down;
            // letting go releases the slot it pressed.
            let intent = if pressed {
                let row =
                    zones::grove::hotbar::row_of(self.keys.shift, self.grove_ctrl, self.grove_alt);
                let intent = key.and_then(|k| zones::grove::hotbar::key(k, row));
                if let Some(intent) = intent {
                    self.grove_keys.retain(|(c, _)| *c != code);
                    self.grove_keys.push((code, intent));
                }
                intent
            } else {
                self.grove_keys
                    .iter()
                    .position(|(c, _)| *c == code)
                    .map(|i| self.grove_keys.remove(i).1)
            };
            if let Some(intent) = intent {
                let _ = self.runtime.grove_key(intent, pressed);
                return;
            }
        }
        // The Water Lab's Water Orb grows while 6 is held and flies when it
        // is let go; Shift as it is let go holds the orb in place.
        if code == KeyCode::Digit6
            && self.in_water_lab()
            && (!pressed || (!self.chat.open && !self.map.expanded))
        {
            let orb = zones::water::hotbar::ORB;
            let result = if pressed {
                self.runtime.water_press(orb, self.keys.shift).map(Some)
            } else {
                self.runtime.water_release(orb, self.keys.shift)
            };
            if let Err(error) = result {
                eprintln!("verse: {error}");
            }
            return;
        }
        if pressed && !self.chat.open && !self.map.expanded {
            let snapshot = self
                .runtime
                .zone_snapshot(self.viewport().map_or(1.0, |(_, aspect)| aspect));
            if code == KeyCode::KeyF && snapshot.portal.near && snapshot.portal.visible {
                self.zone_action(if self.runtime.is_plaza() {
                    ZoneIntent::Enter
                } else {
                    ZoneIntent::Return
                });
                return;
            }
            // The demolition yard's hotbar: 1 swings the sledgehammer, 2
            // aims Meteor Swarm, and R rebuilds. Escape leaves the aim or
            // stops the cast.
            if self.runtime.in_demolition() {
                if code == KeyCode::Escape && self.runtime.demolition_cancel() {
                    return;
                }
                let name = match code {
                    KeyCode::Digit1 => "Digit1",
                    KeyCode::Digit2 => "Digit2",
                    KeyCode::KeyR => "KeyR",
                    _ => "",
                };
                if let Some(intent) = zones::everglade::demolition::hotbar::key(name) {
                    return self.zone_action(intent);
                }
            }
            // Everglade has no arch back, and the Water Lab no panel: G
            // leaves for the Grid or the plaza.
            if code == KeyCode::KeyG
                && ((self.in_bare_everglade() && self.runtime.zone == zones::ZoneId::Everglade)
                    || self.in_water_lab())
            {
                self.zone_action(ZoneIntent::Return);
                return;
            }
            // The Water Lab: 1 to 7 press its hotbar (Shift ends Control
            // Water or casts Destroy Water; 6, the Water Orb, is handled on
            // press and release above), 8 or B drops a float, T turns the
            // hour, Y the sea, and U the weather.
            if self.runtime.zone == zones::ZoneId::WaterLab {
                let slot = match code {
                    KeyCode::Digit1 => Some(0),
                    KeyCode::Digit2 => Some(1),
                    KeyCode::Digit3 => Some(2),
                    KeyCode::Digit4 => Some(3),
                    KeyCode::Digit5 => Some(4),
                    KeyCode::Digit7 => Some(6),
                    KeyCode::Digit8 | KeyCode::KeyB => Some(7),
                    _ => None,
                };
                let result = match (slot, code) {
                    (Some(slot), _) => Some(self.runtime.water_press(slot, self.keys.shift)),
                    (None, KeyCode::KeyT) => Some(self.runtime.water_hour()),
                    (None, KeyCode::KeyY) => Some(self.runtime.water_sea()),
                    (None, KeyCode::KeyU) => Some(self.runtime.water_weather()),
                    _ => None,
                };
                if let Some(result) = result {
                    if let Err(error) = result {
                        eprintln!("verse: {error}");
                    }
                    return;
                }
            }
            // In the crypt, F at the door leaves, whichever way the player
            // faces.
            if code == KeyCode::KeyF && self.runtime.crypt_door_near() {
                self.zone_action(ZoneIntent::Return);
                return;
            }
            // The Meteor Stress Test, Everglade's town with the dev build's
            // destruction on, and the Grove: Escape leaves Meteor Swarm's
            // aim or stops its cast, and R restores the buildings, the
            // castle, or the Grove's tower.
            if (self.in_bare_everglade() && self.runtime.everglade_swarm().is_some())
                || (self.in_bare_grove() && self.runtime.grove_swarm().is_some())
            {
                if code == KeyCode::Escape && self.runtime.demolition_cancel() {
                    return;
                }
                if code == KeyCode::KeyR {
                    return self.zone_action(ZoneIntent::Rebuild);
                }
            }
            // Number keys follow the zone's displayed spell order. In
            // Everglade, 1 is Levitate and 2 to 5 cast the utility spells;
            // with the dev build's destruction on, 1 aims Meteor Swarm, 2
            // the Thunderbolt, 3 is Levitate, 4 to 7 cast the utility
            // spells, and 8 swings the sledgehammer.
            if self.in_bare_everglade() {
                let n = hotbar_number(code);
                if let Some(intent) =
                    zones::everglade::hotbar::key_intent(n, &self.runtime.everglade_hotbar_order())
                {
                    self.zone_action(intent);
                    return;
                }
            }
            // In Everglade the interact key next to the workshop agent
            // opens her panel; elsewhere it opens the station in reach.
            // Only her owner's window, which has its own host to ask,
            // opens her panel, and sets her up there when she does not
            // exist yet; anyone else sees her and her caption says so.
            if code == KeyCode::KeyF
                && !self.keys.shift
                && self.near_workshop_agent()
                && self.workshop.owner()
            {
                self.workshop.open_panel();
                self.keys = Keys::default();
                self.climb = 0.0;
                return;
            }
            if code == KeyCode::KeyF
                && let Some(kind) = self.runtime.studio_panel_here()
            {
                if self.keys.shift {
                    self.open_workbench(kind);
                } else {
                    self.open_studio_panel(kind);
                }
                return;
            }
            // Next to one of Everglade's villagers, the interact key talks
            // to it.
            if code == KeyCode::KeyF
                && !self.keys.shift
                && let Some((id, _)) = self.runtime.villager_in_reach()
            {
                self.talk_to_villager(&id);
                return;
            }
            // By one of Everglade's rowboats, the interact key boards it,
            // leaves it, or rights it.
            if code == KeyCode::KeyF && !self.keys.shift && self.runtime.boat_interact().is_some() {
                return;
            }
            // In Everglade J opens the waiting decisions, and V mutes the
            // studio's bell and chimes.
            if self.runtime.studio().active() {
                match code {
                    KeyCode::KeyJ => {
                        self.open_studio_panel(StudioPanel::Decisions);
                        return;
                    }
                    KeyCode::KeyV => {
                        self.studio_muted = !self.studio_muted;
                        return;
                    }
                    _ => {}
                }
            }
            if !self.runtime.is_plaza() {
                // Number keys press the zone's controls in order.
                let index = match code {
                    KeyCode::Digit1 => Some(0),
                    KeyCode::Digit2 => Some(1),
                    KeyCode::Digit3 => Some(2),
                    KeyCode::Digit4 => Some(3),
                    KeyCode::Digit5 => Some(4),
                    KeyCode::Digit6 => Some(5),
                    KeyCode::Digit7 => Some(6),
                    KeyCode::Digit8 => Some(7),
                    KeyCode::Digit9 => Some(8),
                    _ => None,
                };
                if let Some(action) = index
                    .and_then(|i| snapshot.controls.get(i))
                    .filter(|c| c.enabled)
                    .map(|c| c.action)
                {
                    self.zone_action(action);
                    return;
                }
            }
        }
        if self.gym_key(code, pressed) {
            return;
        }
        if pressed && self.picker_key(code) {
            return;
        }
        if pressed && self.map_visible() {
            if code == KeyCode::KeyM {
                self.map_action(MapAction::Toggle);
                return;
            }
            if code == KeyCode::Escape
                && (self.map.expanded || self.runtime.navigation().is_active())
            {
                self.stop_map();
                return;
            }
        }
        if pressed
            && self.door_frame.as_ref().is_some_and(|frame| frame.visible)
            && let Some(door) = self.door_context()
        {
            let action = match code {
                KeyCode::Digit1 => Some(DoorIntent::Hold(DemoItem::Prism)),
                KeyCode::Digit2 => Some(DoorIntent::Hold(DemoItem::Ring)),
                KeyCode::Digit3 => Some(DoorIntent::Hold(DemoItem::Bolt)),
                KeyCode::Digit4 => Some(DoorIntent::Hold(DemoItem::Empty)),
                KeyCode::Digit5 => Some(DoorIntent::Reset(door)),
                KeyCode::KeyF => Some(DoorIntent::Tap(door)),
                _ => None,
            };
            if let Some(action) = action {
                self.door_action(action);
                return;
            }
        }
        let replaying = self.replay.is_some();
        match code {
            KeyCode::KeyR if pressed => self.open_picker(),
            KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 if pressed && replaying => {
                let speed = match code {
                    KeyCode::Digit1 => replay::SPEEDS[0],
                    KeyCode::Digit2 => replay::SPEEDS[1],
                    _ => replay::SPEEDS[2],
                };
                if let Some(r) = &mut self.replay {
                    r.clock.set_speed(speed);
                }
            }
            KeyCode::KeyP if pressed && replaying => {
                if let Some(r) = &mut self.replay {
                    r.clock.toggle();
                }
            }
            KeyCode::Home if pressed && replaying => {
                if let Some(r) = &mut self.replay {
                    r.clock.seek(0.0);
                    r.clock.playing = true;
                }
            }
            KeyCode::KeyC if pressed => self.toggle_panel(),
            KeyCode::KeyT if pressed => self.toggle_terminal(),
            KeyCode::KeyN if pressed && self.plaza_interactive() => {
                self.left_tab = match self.left_tab {
                    hud::LeftTab::World => hud::LeftTab::Nostr,
                    hud::LeftTab::Nostr => hud::LeftTab::World,
                };
            }
            KeyCode::Enter | KeyCode::NumpadEnter if pressed => self.open_chat(""),
            KeyCode::Slash if pressed => self.open_chat("/"),
            KeyCode::Tab if pressed && self.plaza_interactive() => self.cycle_method(),
            KeyCode::KeyW | KeyCode::ArrowUp => self.keys.w = pressed,
            KeyCode::KeyS | KeyCode::ArrowDown => self.keys.s = pressed,
            KeyCode::KeyA | KeyCode::ArrowLeft => self.keys.a = pressed,
            KeyCode::KeyD | KeyCode::ArrowRight => self.keys.d = pressed,
            KeyCode::KeyQ => self.keys.q = pressed,
            KeyCode::KeyE => self.keys.e = pressed,
            KeyCode::ShiftLeft | KeyCode::ShiftRight => self.keys.shift = pressed,
            KeyCode::Space if pressed => self.keys.jump = true,
            KeyCode::KeyB if pressed && self.plaza_interactive() => {
                self.stop_map();
                self.board_open = !self.board_open;
                self.keys = Keys::default();
                self.capture(false);
            }
            KeyCode::PageDown if pressed && self.board_open => self.scroll_board(8),
            KeyCode::PageUp if pressed && self.board_open => self.scroll_board(-8),
            KeyCode::Escape if pressed && self.board_open => self.board_open = false,
            KeyCode::Escape if pressed && replaying => self.replay = None,
            // Escape hides the terminal overlay; it never quits Verse, so a
            // stray press can't end the terminal's sessions. Closing the
            // window (Cmd+Q on macOS) quits.
            KeyCode::Escape if pressed && self.terminal.open => self.toggle_terminal(),
            KeyCode::Escape => {}
            _ => {}
        }
    }

    /// Scrolls the open quest board by `rows`, within what it holds.
    fn scroll_board(&mut self, rows: i32) {
        let max = self.layout.board.map_or(0, |(_, max)| max);
        let next = self.board_scroll.min(max) as i64 + i64::from(rows);
        self.board_scroll = next.clamp(0, max as i64) as usize;
    }

    fn companion_at_cursor(&self) -> bool {
        if !self.map_visible()
            || !self
                .mount
                .as_ref()
                .is_some_and(|mount| mount.active() && mount.viewport().drawable())
            || self.layout.owns(self.cursor[0], self.cursor[1])
            || self.cursor_on_map()
            || self.cursor_on_door_hud()
        {
            return false;
        }
        let Some((size, aspect)) = self.viewport() else {
            return false;
        };
        self.runtime.companion_hit_with_entities(
            aspect,
            self.cursor[0] / size[0],
            self.cursor[1] / size[1],
            &self.presented_entities,
        )
    }

    fn door_context(&self) -> Option<DoorId> {
        if !self.plaza_interactive()
            || !self.map_visible()
            || self.map.expanded
            || !self
                .mount
                .as_ref()
                .is_some_and(|m| m.active() && m.viewport().drawable())
        {
            return None;
        }
        let (_, aspect) = self.viewport()?;
        DoorId::ALL
            .into_iter()
            .filter(|&id| {
                let door = self.runtime.door(id, aspect);
                door.near
                    && door.visible
                    && self.runtime.door_hit_with_entities(
                        id,
                        aspect,
                        door.screen_x,
                        door.screen_y,
                        &self.presented_entities,
                    )
            })
            .min_by(|&a, &b| {
                self.runtime
                    .door(a, aspect)
                    .distance
                    .total_cmp(&self.runtime.door(b, aspect).distance)
            })
    }

    fn door_at_cursor(&self) -> Option<DoorId> {
        if !self.plaza_interactive()
            || !self.map_visible()
            || self.map.expanded
            || !self
                .mount
                .as_ref()
                .is_some_and(|mount| mount.active() && mount.viewport().drawable())
            || self.cursor_on_map()
            || self.cursor_on_door_hud()
            || self.layout.owns(self.cursor[0], self.cursor[1])
        {
            return None;
        }
        let (size, aspect) = self.viewport()?;
        point_door(
            &self.runtime,
            aspect,
            [self.cursor[0] / size[0], self.cursor[1] / size[1]],
            &self.presented_entities,
        )
    }

    fn cursor_on_door_hud(&self) -> bool {
        self.door_frame.as_ref().is_some_and(|frame| {
            let [x, y] = self.cursor.map(|v| v / self.scale);
            let [left, top, width, height] = frame.frame;
            frame.visible && x >= left && x <= left + width && y >= top && y <= top + height
        })
    }

    fn door_action(&mut self, action: DoorIntent) {
        let Some(context) = self.door_context() else {
            return;
        };
        if matches!(action, DoorIntent::Tap(id) | DoorIntent::Reset(id) if id != context) {
            return;
        }
        self.apply_admitted_door_action(action);
    }

    /// Pointer taps admit their exact visible point; HUD and keyboard actions admit the anchor.
    fn apply_admitted_door_action(&mut self, action: DoorIntent) {
        let result = match action {
            DoorIntent::Hold(item) => {
                self.runtime.hold_door_item(item);
                Ok(())
            }
            DoorIntent::Tap(id) => self.runtime.tap_door(id).map(|_| ()),
            DoorIntent::Reset(id) => {
                self.runtime.reset_door(id);
                Ok(())
            }
        };
        self.door_error = result.err();
        let revision = self.runtime.doors.revision();
        if revision != self.door_save_revision {
            match crate::doors::store::save(
                &self.door_store.0,
                &self.door_store.1,
                &self.runtime.doors.document(),
            ) {
                Ok(()) => {
                    self.door_save_revision = revision;
                    self.door_storage_error = None;
                }
                Err(error) => self.door_storage_error = Some(error),
            }
        }
    }

    fn button(&mut self, button: MouseButton, pressed: bool) {
        self.validate_screen();
        if button == MouseButton::Left {
            if pressed && self.terminal.on_button(self.cursor) {
                self.terminal_press = true;
                self.toggle_terminal();
                return;
            }
            if pressed {
                self.terminal_press = self.terminal.press(self.cursor);
                if self.terminal_press {
                    self.keys = Keys::default();
                    return;
                }
            } else if std::mem::take(&mut self.terminal_press) {
                self.terminal.release(self.cursor);
                return;
            }
        } else {
            let other = match button {
                MouseButton::Right => Some(crate::terminal::mouse::Button::Right),
                MouseButton::Middle => Some(crate::terminal::mouse::Button::Middle),
                _ => None,
            };
            if let Some(other) = other
                && self.terminal.button(other, pressed, self.cursor)
            {
                return;
            }
        }
        if button == MouseButton::Left && self.panel_button(pressed) {
            return;
        }
        // The goal bar's waiting badge opens the decisions.
        if button == MouseButton::Left
            && pressed
            && self
                .studio_badge
                .is_some_and(|badge| badge.contains(self.cursor[0], self.cursor[1]))
        {
            self.open_studio_panel(StudioPanel::Decisions);
            return;
        }
        // Other buttons pressed over the panel do not reach the world.
        if pressed
            && button != MouseButton::Left
            && self
                .panel
                .as_ref()
                .is_some_and(|p| p.bounds().contains(self.cursor.map(|v| v / self.scale)))
        {
            return;
        }
        if button == MouseButton::Left {
            let at = self.cursor.map(|value| value / self.scale);
            if !pressed && self.map.captured(1) {
                if let Some(action) = self.map.up(1, at, !self.map_visible()) {
                    self.map_action(action);
                }
                return;
            }
            if pressed
                && !self.keys.left_button
                && !self.keys.right_button
                && self.map_visible()
                && self
                    .map_frame
                    .as_ref()
                    .is_some_and(|frame| self.map.down(1, at, frame))
            {
                return;
            }
        }
        if self.map.captured(1) {
            return;
        }
        // The Water Orb's slot held with the mouse: letting go anywhere
        // throws the orb, or with Shift holds it in place.
        if button == MouseButton::Left && !pressed && self.water_orb_held {
            self.water_orb_held = false;
            if let Err(error) = self
                .runtime
                .water_release(zones::water::hotbar::ORB, self.keys.shift)
            {
                eprintln!("verse: {error}");
            }
            return;
        }
        // The Water Lab's bar: a click casts the slot's spell, and never
        // reaches the zone panel behind the tray.
        if button == MouseButton::Left
            && self.in_water_lab()
            && let Some(index) = self.water_hotbar_at(self.cursor.map(|v| v / self.scale))
        {
            if pressed && !self.keys.left_button && !self.keys.right_button {
                match self.runtime.water_press(index, self.keys.shift) {
                    Ok(_) => self.water_orb_held = index == zones::water::hotbar::ORB,
                    Err(error) => eprintln!("verse: {error}"),
                }
            }
            return;
        }
        if button == MouseButton::Left
            && pressed
            && !self.keys.left_button
            && !self.keys.right_button
            && self.in_bare_everglade()
            && let Some(intent) = self.hotbar_at(self.cursor.map(|v| v / self.scale))
        {
            // Levitate rises while held; the spells cast once.
            if intent == ZoneIntent::Levitate {
                self.levitate_held = self.runtime.everglade_levitate(true).is_ok();
            } else {
                self.zone_action(intent);
            }
            return;
        }
        if button == MouseButton::Left
            && pressed
            && !self.keys.left_button
            && !self.keys.right_button
            && self.in_bare_grove()
            && let Some(hit) = self.grove_hotbar_at(self.cursor.map(|v| v / self.scale))
        {
            match hit {
                zones::grove::hotbar::Hit::Slot(index) => {
                    if let Some(intent) = zones::grove::hotbar::intent(index) {
                        self.zone_action(intent);
                    }
                }
                zones::grove::hotbar::Hit::Switch => {
                    self.grove_row = (self.grove_row + 1) % zones::grove::slots::ROWS;
                }
            }
            return;
        }
        // Aiming Meteor Swarm in the demolition yard: a click casts it at
        // the circle, and a right click leaves the aim.
        if pressed && self.runtime.demolition_targeting() {
            match button {
                MouseButton::Left => {
                    self.aim_meteor_swarm();
                    self.runtime.demolition_confirm();
                    return;
                }
                MouseButton::Right => {
                    self.runtime.demolition_cancel();
                    return;
                }
                _ => {}
            }
        }
        if button == MouseButton::Left && !pressed && self.levitate_held {
            self.levitate_held = false;
            let _ = self.runtime.everglade_levitate(false);
        }
        if button == MouseButton::Left {
            let at = self.cursor.map(|v| v / self.scale);
            if !pressed && self.zone_hud.captured(1) {
                if let Some(action) = self.zone_hud.up(1, at, false) {
                    self.zone_action(action);
                }
                return;
            }
            if pressed
                && !self.keys.left_button
                && !self.keys.right_button
                && self
                    .zone_frame
                    .as_ref()
                    .is_some_and(|frame| self.zone_hud.down(1, at, frame))
            {
                self.companion_press = None;
                self.door_press = None;
                self.zone_press = None;
                return;
            }
        }
        if self.zone_hud.captured(1) || self.runtime.zone_loading() {
            return;
        }
        if button == MouseButton::Left
            && !pressed
            && let Some(tap) = self.zone_press.take()
        {
            let studio = self.studio_target.take();
            let tapped = tap.released(self.cursor.map(|v| v / self.scale), Instant::now());
            if let Some(kind) = studio {
                if tapped {
                    if self.keys.shift {
                        self.open_workbench(kind);
                    } else {
                        self.open_studio_panel(kind);
                    }
                }
            } else if tapped && self.portal_at_cursor() {
                self.zone_action(if self.runtime.is_plaza() {
                    ZoneIntent::Enter
                } else {
                    ZoneIntent::Return
                });
            }
            return;
        }
        if button == MouseButton::Left
            && pressed
            && !self.keys.left_button
            && !self.keys.right_button
            && self.portal_at_cursor()
        {
            self.studio_target = None;
            self.zone_press = Some(CompanionPress::new(
                self.cursor.map(|v| v / self.scale),
                Instant::now(),
            ));
            return;
        }
        // A tap on a seat, a monitor, or a station in Everglade selects it.
        if button == MouseButton::Left
            && pressed
            && !self.keys.left_button
            && !self.keys.right_button
            && let Some(kind) = self.studio_at_cursor()
        {
            self.studio_target = Some(kind);
            self.zone_press = Some(CompanionPress::new(
                self.cursor.map(|v| v / self.scale),
                Instant::now(),
            ));
            return;
        }
        if button == MouseButton::Left {
            let at = self.cursor.map(|v| v / self.scale);
            if !pressed && self.door_hud.captured(1) {
                let cancelled = self.door_context().is_none();
                if let Some(action) = self.door_hud.up(1, at, cancelled) {
                    self.door_action(action);
                }
                return;
            }
            if pressed
                && !self.keys.left_button
                && !self.keys.right_button
                && self.door_context().is_some()
                && self
                    .door_frame
                    .as_ref()
                    .is_some_and(|frame| self.door_hud.down(1, at, frame))
            {
                self.companion_press = None;
                self.door_press = None;
                return;
            }
        }
        if self.door_hud.captured(1) {
            return;
        }
        if self.gym_open {
            return;
        }
        if button == MouseButton::Left
            && !pressed
            && let Some((id, tap)) = self.door_press.take()
        {
            if tap.released(self.cursor.map(|v| v / self.scale), Instant::now())
                && self.door_at_cursor() == Some(id)
            {
                self.apply_admitted_door_action(DoorIntent::Tap(id));
            }
            return;
        }
        if button == MouseButton::Left
            && pressed
            && !self.keys.left_button
            && !self.keys.right_button
            && let Some(id) = self.door_at_cursor()
        {
            self.door_press = Some((
                id,
                CompanionPress::new(self.cursor.map(|v| v / self.scale), Instant::now()),
            ));
            return;
        }
        if button == MouseButton::Left
            && !pressed
            && let Some(tap) = self.companion_press.take()
        {
            if tap.released(self.cursor.map(|value| value / self.scale), Instant::now())
                && self.companion_at_cursor()
            {
                self.runtime.pet_companion();
            }
            return;
        }
        if button == MouseButton::Left && self.runtime.in_demolition() {
            if pressed {
                self.swing_press = Some(Instant::now());
            } else if self
                .swing_press
                .take()
                .is_some_and(|at| at.elapsed() <= Duration::from_millis(300))
            {
                self.zone_action(ZoneIntent::Swing);
            }
        }
        match button {
            MouseButton::Left if pressed && !self.keys.left_button && self.click() => return,
            MouseButton::Left
                if pressed
                    && !self.keys.left_button
                    && !self.keys.right_button
                    && self.companion_at_cursor() =>
            {
                self.companion_press = Some(CompanionPress::new(
                    self.cursor.map(|value| value / self.scale),
                    Instant::now(),
                ));
                return;
            }
            MouseButton::Left => self.keys.left_button = pressed,
            MouseButton::Right => {
                if pressed
                    && (self.companion_press.take().is_some()
                        || self.door_press.take().is_some()
                        || self.zone_press.take().is_some())
                {
                    self.keys.left_button = true;
                }
                self.keys.right_button = pressed;
                if pressed {
                    let _ = self.runtime.apply(Action::FaceCamera);
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
        if self.gym_open
            || self.runtime.zone_loading()
            || self.zone_hud.captured(1)
            || self.map.captured(1)
            || self.door_hud.captured(1)
        {
            return;
        }
        if let Some(tap) = self
            .companion_press
            .as_mut()
            .or_else(|| self.door_press.as_mut().map(|(_, tap)| tap))
            .or(self.zone_press.as_mut())
        {
            tap.motion(dx / self.scale, dy / self.scale);
            if tap.valid {
                return;
            }
            self.companion_press = None;
            self.door_press = None;
            self.zone_press = None;
            self.keys.left_button = true;
            self.capture(true);
        }
        if self.keys.right_button {
            let _ = self.runtime.apply(Action::Look { dx, dy });
        } else if self.keys.left_button {
            let _ = self.runtime.apply(Action::Orbit { dx, dy });
        }
    }

    fn frame(&mut self) {
        let now = Instant::now();
        let dt = match self
            .mount
            .as_mut()
            .map(|m| m.frame_delta(self.started.elapsed().as_secs_f64()))
        {
            Some(Ok(Some(dt))) => dt,
            Some(Err(error)) => {
                self.error = Some(error.to_string());
                return;
            }
            _ => return,
        };
        let wall_dt = dt;

        // Suspend the plaza before a completed download can install a zone pose.
        self.sync_zone_services(true);
        self.tick_ritual();
        self.runtime.zone_tick();
        self.aim_meteor_swarm();
        self.aim_water_lab();
        self.open_pending_everglade();
        if self.runtime.zone_revision != self.rendered_zone_revision {
            self.stop_map();
            self.map_error = None;
            self.layout = hud::Layout::default();
            self.keys = Keys::default();
            self.capture(false);
            self.presented_entities = crate::mesh::Mesh::default();
            if self.on_grid() != self.grid.is_some() {
                if let (Some(window), Some(atlas)) = (self.window.clone(), self.atlas.take()) {
                    let opened = self.open_surface(window, &atlas);
                    self.atlas = Some(atlas);
                    if let Err(error) = opened {
                        self.error = Some(error);
                        return;
                    }
                }
            } else if let Some(renderer) = &mut self.renderer {
                if let Err(error) = renderer
                    .replace_world(&self.runtime.world.mesh)
                    .and_then(|()| renderer.set_atmosphere(zones::atmosphere(self.runtime.zone)))
                {
                    self.error = Some(error);
                    return;
                }
                self.rendered_zone_revision = self.runtime.zone_revision;
            } else {
                self.rendered_zone_revision = self.runtime.zone_revision;
            }
            self.sync_zone_services(true);
        }
        self.map.tick(dt);
        let input = self.keys.input();
        self.keys.jump = false;
        let dt =
            self.runtime
                .tick_with_mode(&input, dt, self.keys.left_button, self.replay.is_none());
        if self.climb != 0.0 {
            if (self.in_bare_everglade() || self.in_bare_grove())
                && (self.runtime.everglade_levitating()
                    || (self.climb > 0.0 && self.runtime.grove_form_flies()))
            {
                self.runtime.everglade_climb(self.climb, dt);
            } else {
                self.climb = 0.0;
            }
        }
        #[cfg(feature = "remote-chamber")]
        if let Some((link, heading)) = &mut self.hosted {
            link.steer_input(&input, heading, dt);
            if let Err(error) = link.pump(&mut self.runtime) {
                self.error = Some(error);
                self.hosted = None;
                self.runtime.leave_hosted_social();
            }
        }
        self.update_gym(true);
        self.runtime.update_studio(true, dt);
        self.step_workshop();
        self.studio_signals();
        self.refresh_studio_panel();
        self.step_agents(dt);
        if self.plaza_interactive() {
            self.plaza_presence = (self.runtime.player, self.runtime.agent);
        }

        // The look-around is the agent assessing what is near: it asks the
        // relay for entity states around it, then glances at what it found.
        if self.runtime.agent.take_scan() {
            match &mut self.session {
                Some(session) => session.request_scan(self.runtime.agent.pos),
                None => self.runtime.agent.look_around(&[]),
            }
        }
        let on_grid = self.grid.is_some();
        let mut dynamic = if on_grid {
            crate::mesh::Mesh::default()
        } else {
            self.runtime.dynamic_mesh()
        };
        let mut entities = crate::mesh::Mesh::default();
        let mut peers: Vec<crate::crowd::Figure> = Vec::new();
        let mut standing: Vec<grid_frame::Standing> = Vec::new();
        if self.replay.as_ref().is_some_and(|r| r.ghost.is_ok()) {
            entities.extend(
                &self
                    .ghost
                    .mesh_at(coder_ui::theme::Intensity::ThreeQuarters),
            );
        }
        if let Some(session) = &mut self.session {
            if self.runtime.is_hosted() {
                // Other players come from the host's snapshots; the
                // relay's crowd keeps discovery but draws no pose here.
                session.tick_world(now, &mut self.runtime);
            } else {
                session.tick(now, &self.runtime.player, &self.runtime.agent);
            }
            if let Some(found) = session.scan_result(now, &self.runtime.agent) {
                self.runtime.agent.look_around(&found);
            }
            // Two agents that meet greet each other.
            if let Some((pubkey, at)) = session.greeting(now, &self.runtime.agent)
                && self.runtime.agent.greet(at)
            {
                session.greeted(&pubkey, at, &self.runtime.agent, now);
            }
            for zone_command in session.take_zone_commands() {
                let outcome = if self.zone_operators.allows(&zone_command.from) {
                    self.runtime.zone_command(&zone_command.command)
                } else {
                    Err("sender is not a listed zone operator".to_owned())
                };
                let ok = outcome.is_ok();
                let text = match &outcome {
                    Ok(value) => format!(
                        "zone {} {} from {}…: {value}",
                        zone_command.command.id,
                        zone_command.command.cmd,
                        &zone_command.from[..8.min(zone_command.from.len())],
                    ),
                    Err(error) => format!(
                        "zone {} {} from {}… refused: {error}",
                        zone_command.command.id,
                        zone_command.command.cmd,
                        &zone_command.from[..8.min(zone_command.from.len())],
                    ),
                };
                session.log.push(chat::Line::system(text));
                session.report_zone(
                    &zone_command.from,
                    &zone_command.command.id,
                    ok,
                    self.runtime.player.pos,
                );
            }
            if on_grid {
                peers = session.crowd.figures(now, dt);
            } else {
                entities.extend(&session.crowd.mesh(now, dt));
            }
        }
        if self.frames.is_multiple_of(30) {
            self.update_title();
        }
        self.frames = self.frames.wrapping_add(1);

        let Some((size, aspect)) = self.viewport() else {
            return;
        };
        // The runtime's view, which every surface shares: the eye stops
        // short of the zone's walls, vaults, and roofs.
        let view = self.runtime.view(aspect);
        if let Some(feed) = &mut self.feed {
            feed.tick(now);
            for v in &feed.visitors {
                if on_grid {
                    standing.push(grid_frame::Standing {
                        pos: v.pos,
                        yaw: v.yaw,
                    });
                    continue;
                }
                let rot = glam::Quat::from_rotation_y(v.yaw);
                entities.extend(&avatar::figure(
                    v.pos,
                    rot,
                    &Gait::default(),
                    coder_ui::theme::Intensity::Half,
                ));
            }
        }
        if self.plaza_interactive() {
            self.hear_agent(now);
        } else {
            self.brain.drain();
        }
        self.hear_villagers();
        if let Some(board) = &mut self.xp {
            board.tick();
        }
        let navigation = self.runtime.navigation();
        let eva = self.runtime.eva_map_status();
        let map_status = self.map_error.as_deref().unwrap_or(match eva {
            Some((status, _)) => status,
            None => match navigation.status() {
                NavigationStatus::Idle => "Choose a place to walk",
                NavigationStatus::Walking => "Walking",
                NavigationStatus::Arrived => "Arrived",
                NavigationStatus::Cancelled => "Walking cancelled",
                NavigationStatus::Blocked => "Route blocked",
            },
        });
        self.map_frame = Some(self.map.snapshot_for_zone(
            size.map(|value| value / self.scale),
            [self.runtime.player.pos.x, self.runtime.player.pos.z],
            self.map_visible(),
            map_status,
            eva.map_or_else(|| navigation.destination(), |(_, target)| target),
            self.runtime.zone,
        ));
        let overheads = self.overheads(now);
        let tip = self.hotbar_tip(size.map(|v| v / self.scale));
        let mut ui = match &self.atlas {
            Some(atlas) => {
                let (log, name_of): (&chat::Log, NameOf<'_>) = match &self.session {
                    Some(s) => (&s.log, Box::new(|p: &str| s.name_of(p))),
                    None => (&self.offline_log, Box::new(str::to_owned)),
                };
                let pills = self
                    .methods()
                    .iter()
                    .map(|m| (hud::method_label(m, &name_of), *m == self.method))
                    .collect();
                let (near, here) = self.session.as_ref().map_or((0, 0), |s| {
                    let shown = s.crowd.shown(now);
                    let avatars = shown.iter().filter(|e| e.role == "avatar" && e.online);
                    let flat = |v: Vec3| Vec3::new(v.x, 0.0, v.z);
                    let d = |e: &&crate::crowd::Shown| {
                        flat(e.pos).distance(flat(self.runtime.player.pos))
                    };
                    (
                        avatars
                            .clone()
                            .filter(|e| d(e) <= chat::NEAR_RADIUS)
                            .count(),
                        avatars.filter(|e| d(e) <= chat::HERE_RADIUS).count(),
                    )
                });
                let zone = chat::zone_of(self.runtime.player.pos);
                let hint = hud::audience(&self.method, session::WORLD, zone, near, here, &name_of);
                let limit = matches!(self.method, Channel::All | Channel::Ads)
                    .then_some(chat::MAX_BROADCAST);
                let empty = std::collections::VecDeque::new();
                let (nostr, nostr_title) = match &self.feed {
                    Some(feed) => (&feed.lines, feed.title()),
                    None => (&empty, "offline: start Verse with a relay".to_owned()),
                };
                let (mut ui, mut layout) = if self.plaza_interactive() {
                    hud::build(
                        atlas,
                        &hud::Frame {
                            size,
                            scale: self.scale,
                            view_proj: view.view_proj,
                            log,
                            nostr,
                            nostr_title,
                            left_tab: self.left_tab,
                            input: &self.chat,
                            pills,
                            hint,
                            limit,
                            world_title: hud::world_title(session::WORLD, self.runtime.player.pos),
                            overheads: &overheads,
                            time: (now - self.started).as_secs_f32(),
                            xp: xp::strip(self.xp.as_ref(), &self.my_keys),
                            board: self
                                .board_open
                                .then(|| xp::board_lines(self.xp.as_ref(), unix_now())),
                            board_scroll: self.board_scroll,
                            replay: self
                                .replay
                                .as_ref()
                                .map(Replay::hud_lines)
                                .unwrap_or_default(),
                            picker: self.picker.as_ref().map(Picker::lines),
                            picker_scroll: self.picker.as_ref().map_or(0, Picker::scroll),
                        },
                    )
                } else {
                    (crate::ui::UiBatch::default(), hud::Layout::default())
                };
                if self.gym_open && self.runtime.gym(size[0] / size[1].max(1.0)).inside {
                    let panel = hud::gym_panel(
                        &mut ui,
                        atlas,
                        size,
                        self.scale,
                        &hud::GymPanel {
                            view: self.gym_view.as_ref(),
                            recipes: self.gym_recipes,
                            selected: self.gym_selected,
                            scroll: self.gym_scroll,
                            notice: self.gym_notice.as_deref(),
                        },
                    );
                    layout.panels.push(panel);
                }
                // The studio's goal bar and waiting badge, in Everglade. The
                // bar sits at the top, so it is not one of the bottom panels
                // the zone controls clear.
                let summary = self
                    .runtime
                    .studio()
                    .active()
                    .then(|| self.runtime.studio().view())
                    .flatten()
                    .and_then(crate::zones::everglade::signals::Summary::of);
                self.studio_badge = summary.and_then(|summary| {
                    hud::studio_strip(
                        &mut ui,
                        atlas,
                        size,
                        self.scale,
                        &summary,
                        self.studio_muted,
                    )
                    .badge
                });
                // The workshop agent's panel, anchored to the bottom edge.
                if self.workshop.open {
                    let cols = hud::workshop_cols(atlas, size, self.scale);
                    // It stands on the hotbar, which stays visible; while
                    // the terminal's panes show, it sits under them in the
                    // hotbar's place.
                    let logical = size.map(|v| v / self.scale);
                    let tray = self.everglade_tray(logical);
                    let (bottom, top) = if self.terminal.open {
                        (
                            0.0,
                            (size[1] * 0.82).round() + atlas.line + 8.0 * self.scale,
                        )
                    } else {
                        ((size[1] - tray[1] * self.scale).max(0.0), 0.0)
                    };
                    let room = size[1] - bottom - top - 28.0 * self.scale;
                    let count = ((room / atlas.line.max(1.0)).floor().max(0.0) as usize)
                        .clamp(5, hud::WORKSHOP_ROWS);
                    let rows = self.workshop.rows(cols, count);
                    let _ = hud::workshop_panel(&mut ui, atlas, size, self.scale, bottom, &rows);
                }
                if let (Some(map_atlas), Some(map_frame)) = (&self.map_atlas, &self.map_frame) {
                    ui.vertices.extend(
                        self.map
                            .draw(
                                map_atlas,
                                map_frame,
                                &self.runtime.world.blockers,
                                [self.runtime.player.pos.x, self.runtime.player.pos.z],
                                self.runtime.player.yaw,
                                self.runtime.navigation().waypoints(),
                                self.scale,
                            )
                            .vertices,
                    );
                }
                let bottom_clearance = layout
                    .panels
                    .iter()
                    .map(|panel| (size[1] - panel.y) / self.scale)
                    .fold(86.0_f32, f32::max)
                    .min(2048.0);
                let _ = self.door_hud.set_bottom_clearance(bottom_clearance);
                self.door_frame = Some(
                    self.door_hud.snapshot(
                        size.map(|v| v / self.scale),
                        self.door_context(),
                        &self.runtime.doors,
                        self.map_visible() && !self.map.expanded,
                        self.door_error
                            .as_deref()
                            .or(self.door_storage_error.as_deref()),
                    ),
                );
                if let (Some(atlas), Some(frame)) = (&self.map_atlas, &self.door_frame) {
                    ui.vertices
                        .extend(self.door_hud.draw(atlas, frame, self.scale).vertices);
                }
                // In the crypt, the door's panel stands above the hotbar.
                let crypt_clearance = || {
                    let logical = size.map(|v| v / self.scale);
                    let tray = self.everglade_tray(logical);
                    (logical[1] - tray[1] + 8.0).clamp(12.0, 2048.0)
                };
                let _ = self
                    .zone_hud
                    .set_bottom_clearance(if self.plaza_interactive() {
                        bottom_clearance
                    } else if self.runtime.zone == zones::ZoneId::Crypt {
                        crypt_clearance()
                    } else if self.in_water_lab() {
                        // The Water Lab's panel stands above its spell bar.
                        let logical = size.map(|v| v / self.scale);
                        let tray = zones::water::hotbar::frame(logical, HOTBAR_BOTTOM);
                        (logical[1] - tray[1] + 8.0).clamp(12.0, 2048.0)
                    } else {
                        12.0
                    });
                self.zone_frame = Some(self.zone_hud.snapshot(
                    size.map(|v| v / self.scale),
                    &self.runtime.zone_snapshot(size[0] / size[1].max(1.0)),
                    !self.map.expanded
                        && !self.chat.open
                        && !self.board_open
                        && !self.gym_open
                        && self.picker.is_none()
                        // The crypt shows its panel only at the door.
                        && (!self.in_bare_everglade() || self.runtime.crypt_door_near())
                        && !self.in_bare_grove()
                        // The Water Lab shows its spell bar and no panel; F
                        // at the lantern or G leaves.
                        && !self.in_water_lab(),
                ));
                if let (Some(atlas), Some(frame)) = (&self.map_atlas, &self.zone_frame) {
                    ui.vertices
                        .extend(self.zone_hud.draw(atlas, frame, self.scale).vertices);
                }
                // The workshop agent's panel takes the hotbar's place while
                // it sits under the terminal's panes.
                if self.in_bare_everglade()
                    && !(self.workshop.open && self.terminal.open)
                    && let (Some(atlas), Some(slots)) =
                        (&self.map_atlas, self.runtime.everglade_hotbar())
                {
                    // The bar is laid out in logical points; this batch is
                    // in pixels.
                    let mut bar = crate::ui::UiBatch::default();
                    zones::everglade::hotbar::draw_ordered(
                        &mut bar,
                        atlas,
                        size.map(|v| v / self.scale),
                        HOTBAR_BOTTOM,
                        &slots,
                        &self.runtime.everglade_hotbar_order(),
                    );
                    if let Some(index) = tip {
                        zones::everglade::hotbar::draw_tip_ordered(
                            &mut bar,
                            atlas,
                            size.map(|v| v / self.scale),
                            HOTBAR_BOTTOM,
                            index,
                            &self.runtime.everglade_hotbar_order(),
                        );
                    }
                    // The breath bar over the tray while the player is
                    // under water or catching its breath.
                    if let Some(breath) = self.runtime.everglade_breath() {
                        zones::everglade::water::draw_breath(
                            &mut bar,
                            atlas,
                            size.map(|v| v / self.scale),
                            HOTBAR_BOTTOM,
                            &breath,
                        );
                    }
                    // Meteor Swarm's help and cast bar over the tray.
                    if let Some(swarm) = self.runtime.everglade_swarm() {
                        zones::everglade::demolition::hotbar::draw_town(
                            &mut bar,
                            atlas,
                            size.map(|v| v / self.scale),
                            HOTBAR_BOTTOM,
                            slots.len(),
                            &swarm,
                        );
                    }
                    if self.runtime.zone == zones::ZoneId::MeteorStressTest {
                        bar.text(
                            atlas,
                            20.0,
                            20.0,
                            "Meteor Stress Test · 5 casters · castle rebuilds every 3 min",
                            [1.0, 0.8, 0.4, 1.0],
                        );
                        bar.text(
                            atlas,
                            20.0,
                            20.0 + atlas.line,
                            "WASD: approach · 1: Meteor Swarm · click: cast · R: rebuild · 6: levitate",
                            [0.9, 0.9, 0.9, 1.0],
                        );
                    }
                    for vertex in &mut bar.vertices {
                        vertex.pos = vertex.pos.map(|v| v * self.scale);
                    }
                    ui.vertices.extend(bar.vertices);
                }
                if self.in_bare_everglade()
                    && let (Some(atlas), Some(bar)) =
                        (&self.map_atlas, self.runtime.demolition_bar())
                {
                    let mut batch = crate::ui::UiBatch::default();
                    zones::everglade::demolition::hotbar::draw(
                        &mut batch,
                        atlas,
                        size.map(|v| v / self.scale),
                        HOTBAR_BOTTOM,
                        &bar,
                    );
                    if let Some(index) = tip {
                        zones::everglade::demolition::hotbar::draw_tip(
                            &mut batch,
                            atlas,
                            size.map(|v| v / self.scale),
                            HOTBAR_BOTTOM,
                            index,
                        );
                    }
                    for vertex in &mut batch.vertices {
                        vertex.pos = vertex.pos.map(|v| v * self.scale);
                    }
                    ui.vertices.extend(batch.vertices);
                }
                // The Water Lab's spell bar.
                if self.runtime.zone == zones::ZoneId::WaterLab
                    && let (Some(atlas), Some(bar)) = (&self.map_atlas, self.runtime.water_bar())
                {
                    let mut batch = crate::ui::UiBatch::default();
                    let logical = size.map(|v| v / self.scale);
                    let slots: Vec<_> = bar.iter().map(|(_, slot)| *slot).collect();
                    zones::water::hotbar::draw(&mut batch, atlas, logical, HOTBAR_BOTTOM, &slots);
                    if let Some(index) = tip {
                        zones::water::hotbar::draw_tip(
                            &mut batch,
                            atlas,
                            logical,
                            HOTBAR_BOTTOM,
                            index,
                        );
                    }
                    for vertex in &mut batch.vertices {
                        vertex.pos = vertex.pos.map(|v| v * self.scale);
                    }
                    ui.vertices.extend(batch.vertices);
                }
                if self.in_bare_grove()
                    && let (Some(atlas), Some(slots)) = (&self.map_atlas, self.runtime.grove_bar())
                {
                    let mut bar = crate::ui::UiBatch::default();
                    let logical = size.map(|v| v / self.scale);
                    let layout = self.grove_layout(logical);
                    zones::grove::hotbar::draw(
                        &mut bar,
                        atlas,
                        logical,
                        HOTBAR_BOTTOM,
                        layout,
                        &slots,
                    );
                    if let Some((status, lines)) = self.runtime.grove_log() {
                        zones::grove::hotbar::draw_log(
                            &mut bar,
                            atlas,
                            logical,
                            HOTBAR_BOTTOM,
                            layout,
                            &status,
                            &lines,
                        );
                    }
                    // Meteor Swarm's or the Thunderbolt's help and cast bar.
                    if let Some(swarm) = self.runtime.grove_swarm() {
                        zones::everglade::demolition::hotbar::draw_aim(
                            &mut bar, atlas, logical, &swarm,
                        );
                    }
                    if let Some(index) = tip {
                        zones::grove::hotbar::draw_tip(
                            &mut bar,
                            atlas,
                            logical,
                            HOTBAR_BOTTOM,
                            layout,
                            &slots,
                            index,
                        );
                    }
                    for vertex in &mut bar.vertices {
                        vertex.pos = vertex.pos.map(|v| v * self.scale);
                    }
                    ui.vertices.extend(bar.vertices);
                }
                self.layout = layout;
                ui
            }
            None => crate::ui::UiBatch::default(),
        };
        // The terminal's hotbar button: right of Everglade's tray, else in
        // the bottom-right corner.
        let tray = self.in_bare_everglade().then(|| {
            self.everglade_tray(size.map(|v| v / self.scale))
                .map(|v| v * self.scale)
        });
        self.terminal.button = Some(crate::terminal::Overlay::button_for(size, self.scale, tray));
        self.terminal.scale = self.scale;
        self.validate_screen();
        // The terminal overlay draws over every other HUD element. It may
        // add fallback glyphs to the atlas, which the renderer then takes.
        match &mut self.atlas {
            Some(atlas) => {
                if let Some(screen) = &mut self.screen {
                    let [x, y, w, h] = self.screen_bounds.map(|value| value as f32 * self.scale);
                    self.terminal.screen_rect =
                        Some(crate::terminal::layout::Rect::new(x, y, w, h));
                    let viewport = (
                        size.map(f32::to_bits),
                        self.scale.to_bits(),
                        atlas.revision(),
                    );
                    let changed = self.screen_viewport != Some(viewport);
                    if changed {
                        self.screen_vertices.clear();
                    }
                    let visible = self.terminal.open;
                    if !visible {
                        self.screen_vertices.clear();
                    }
                    let active = self
                        .mount
                        .as_ref()
                        .is_some_and(|mount| mount.active() && mount.viewport().drawable());
                    if screen.frame(
                        self.screen_clock.elapsed().as_millis() as u64,
                        visible,
                        active,
                    ) {
                        let mut screen_batch = crate::ui::UiBatch::default();
                        self.terminal.draw(&mut screen_batch, atlas, size);
                        self.screen_vertices = screen_batch.vertices;
                        self.screen_viewport = Some((
                            size.map(f32::to_bits),
                            self.scale.to_bits(),
                            atlas.revision(),
                        ));
                    }
                    ui.vertices.extend_from_slice(&self.screen_vertices);
                } else {
                    self.terminal.draw(&mut ui, atlas, size);
                }
                if atlas.revision() != self.atlas_revision {
                    self.atlas_revision = atlas.revision();
                    let uploaded = match (&mut self.grid, &mut self.renderer) {
                        (Some(grid), _) => grid.update_atlas(atlas),
                        (None, Some(renderer)) => renderer.update_atlas(atlas),
                        (None, None) => true,
                    };
                    if !uploaded {
                        eprintln!(
                            "verse: the glyph atlas changed size; new glyphs show after the next zone change"
                        );
                    }
                }
            }
            None if self.screen.is_none() => self.terminal.tick(),
            None => (),
        }
        // A request over the control socket may have given the overlay
        // focus (or taken it): the character stops, as it does for T.
        if self.terminal.focus_changed() == Some(true) {
            self.take_keys_for_terminal();
        }
        dynamic.extend(&entities);
        // A studio panel keeps the rows the studio filled it with.
        let rows = (self.panel.is_some() && self.studio_panel.is_none()).then(|| self.panel_rows());
        let px = [size[0] as u32, size[1] as u32];
        let overlay = match (&mut self.panel, rows) {
            (Some(panel), Some(rows)) => {
                panel.set_rows(rows);
                panel.image(px, self.scale).map(Some)
            }
            (Some(panel), None) => panel.image(px, self.scale).map(Some),
            _ => Ok(None),
        };
        let mut render_ms = 0.0;
        let mut instances = 0;
        if let Some(grid) = &mut self.grid {
            if let Err(error) = overlay {
                eprintln!("verse: panel not drawn: {error}");
                self.panel = None;
            }
            let dynamic = grid_frame::dynamic(&self.runtime, &peers, &standing);
            instances = dynamic.len();
            let lighting = grid_frame::lighting(&self.runtime.atmosphere());
            match grid.draw(view, &dynamic, &ui, &lighting) {
                Ok(ms) => render_ms = ms,
                Err(error) => self.error = Some(error),
            }
        } else if let Some(renderer) = &mut self.renderer {
            if let Err(error) = overlay.and_then(|image| renderer.set_overlay(image)) {
                eprintln!("verse: panel not drawn: {error}");
                self.panel = None;
            }
            #[cfg(target_os = "macos")]
            renderer.set_headroom(crate::edr::current_headroom());
            match renderer.draw(view, &dynamic, &ui) {
                render::DrawStatus::Presented => self.presented_entities = entities,
                render::DrawStatus::Error(error) => self.error = Some(error),
                render::DrawStatus::Skipped(_) => {}
            }
        }
        if let Some(timing) = &mut self.timing
            && let Some(summary) = timing.frame(now, wall_dt, instances, render_ms)
            && let Ok(line) = serde_json::to_string(&summary)
        {
            println!("{line}");
        }
        self.terminal.frame_done(now);
        self.stress_step();
    }

    /// Runs the scripted terminal stress run's next actions.
    fn stress_step(&mut self) {
        use crate::terminal::stress::Action;
        let ready = self.in_bare_everglade() && self.atlas.is_some() && self.viewport().is_some();
        let Some(driver) = &mut self.stress else {
            return;
        };
        let actions = driver.step(ready);
        if driver.recording() && !self.terminal.stats.record {
            self.terminal.stats.record = true;
            self.terminal.stats.frames.clear();
            self.terminal.stats.latencies.clear();
        }
        for action in actions {
            match action {
                Action::Open => {
                    let Some(driver) = &self.stress else { return };
                    let programs = match driver.programs() {
                        Ok(programs) => programs,
                        Err(error) => {
                            eprintln!("verse: the stress run cannot start: {error}");
                            std::process::exit(1);
                        }
                    };
                    let root = driver.root().to_path_buf();
                    self.terminal.shutdown();
                    self.terminal = crate::terminal::Overlay::with(
                        &root,
                        "/bin/sh".into(),
                        programs[0].clone(),
                    );
                    if let (Some(atlas), Some((size, _))) = (&self.atlas, self.viewport()) {
                        self.terminal.fit(atlas, size);
                        // Meteor Swarm aims where the cursor rests.
                        self.cursor = [size[0] * 0.5, size[1] * 0.62];
                    }
                    let ids = self.terminal.open_grid(&programs);
                    eprintln!("verse: stress run opened {} panes", ids.len());
                }
                Action::WindWall => self.zone_action(ZoneIntent::WindWall),
                // Everglade's hotbar has no Meteor Swarm; the run drives
                // the town's demolition directly.
                Action::MeteorSwarm => {
                    self.runtime.scripted_meteor_swarm();
                }
                Action::Key(c) => {
                    let (code, logical) = if c == '\r' {
                        (KeyCode::Enter, winit::keyboard::Key::Named(NamedKey::Enter))
                    } else {
                        (
                            KeyCode::KeyA,
                            winit::keyboard::Key::Character(c.to_string().into()),
                        )
                    };
                    self.terminal.key(&crate::terminal::KeyIn {
                        code,
                        logical,
                        text: (c != '\r').then(|| c.to_string()),
                        plain: None,
                        pressed: true,
                        repeat: false,
                        synthetic: false,
                    });
                }
                Action::Finish => {
                    let Some(driver) = &self.stress else { return };
                    let report =
                        driver.report(&self.terminal.stats.frames, &self.terminal.stats.latencies);
                    let json = serde_json::to_string_pretty(&report).unwrap_or_default();
                    if let Err(error) = std::fs::write(&driver.plan.out, &json) {
                        eprintln!("verse: cannot write {}: {error}", driver.plan.out.display());
                    }
                    println!("{json}");
                    self.terminal.shutdown();
                    driver.clean();
                    std::process::exit(0);
                }
            }
        }
    }

    /// Whether the window shows the Grid, which the engine renderer draws.
    fn on_grid(&self) -> bool {
        self.runtime.is_bare() && self.runtime.is_plaza()
    }

    /// The viewport in pixels and its aspect, from whichever renderer is
    /// attached.
    fn viewport(&self) -> Option<([f32; 2], f32)> {
        if let Some(grid) = &self.grid {
            return Some((grid.size(), grid.aspect()));
        }
        let renderer = self.renderer.as_ref()?;
        Some((renderer.size(), renderer.aspect()))
    }

    /// Attaches the renderer the current zone needs to the window, dropping
    /// the other one first because they cannot share its surface.
    fn open_surface(&mut self, window: Arc<Window>, atlas: &Atlas) -> Result<(), String> {
        if self.on_grid() {
            self.renderer = None;
            let size = window.inner_size();
            self.grid = Some(GridEngine::new(window, atlas, size.width, size.height)?);
        } else {
            self.grid = None;
            let mut renderer = Renderer::new(window, &self.runtime.world.mesh, atlas)?;
            renderer.set_atmosphere(zones::atmosphere(self.runtime.zone))?;
            self.renderer = Some(renderer);
        }
        self.rendered_zone_revision = self.runtime.zone_revision;
        Ok(())
    }

    /// Name tags and speech bubbles: over you, your agent, nearby players,
    /// and Nostr stand-ins.
    /// Name tags in a zone: the players sharing its world, and this one.
    fn zone_overheads(&self, now: Instant) -> Vec<hud::Overhead> {
        use coder_ui::theme::Intensity;
        let Some(s) = &self.session else {
            return Vec::new();
        };
        if self.runtime.zone_loading() {
            return Vec::new();
        }
        let snapshot = self.xp.as_ref().and_then(|b| b.snapshot.as_ref());
        let mut out = vec![hud::Overhead {
            feet: self.runtime.player.pos,
            lift: 2.2,
            name: Some(xp::name_tag(snapshot, s.pubkey(), Some(s.profile()))),
            name_step: Intensity::ThreeQuarters,
            bubble: None,
        }];
        for e in s.crowd.shown(now) {
            if e.role != "avatar" || e.pos.distance(self.runtime.player.pos) > 60.0 {
                continue;
            }
            let name = s.name_of(&e.pubkey);
            let name = (!name.ends_with('\u{2026}')).then_some(name);
            out.push(hud::Overhead {
                feet: e.pos,
                lift: 2.2,
                name: Some(xp::name_tag(snapshot, &e.pubkey, name.as_deref())),
                name_step: if e.online {
                    Intensity::Half
                } else {
                    Intensity::Quarter
                },
                bubble: None,
            });
        }
        out
    }

    fn overheads(&self, now: Instant) -> Vec<hud::Overhead> {
        if !self.plaza_interactive() {
            return self.zone_overheads(now);
        }
        use coder_ui::theme::Intensity;
        let mut out = Vec::new();
        let snapshot = self.xp.as_ref().and_then(|b| b.snapshot.as_ref());
        let tagged = |name: String, keys: &[String]| match xp::level_tag(snapshot, keys) {
            Some(level) => format!("{name} · {level}"),
            None => name,
        };
        let (my_name, my_bubble) = match &self.session {
            Some(s) => (
                tagged(s.profile().to_owned(), &self.my_keys),
                s.bubbles
                    .iter()
                    .find(|b| b.pubkey == s.pubkey())
                    .map(|b| b.text.clone()),
            ),
            None => (tagged("you".to_owned(), &self.my_keys), None),
        };
        out.push(hud::Overhead {
            feet: self.runtime.player.pos,
            lift: 2.2,
            name: Some(my_name),
            name_step: Intensity::ThreeQuarters,
            bubble: my_bubble,
        });
        if let Some((text, _)) = &self.agent_says {
            out.push(hud::Overhead {
                feet: self.runtime.agent.pos,
                lift: 0.5,
                name: None,
                name_step: Intensity::Half,
                bubble: Some(text.clone()),
            });
        }
        if let Some(s) = &self.session {
            for e in s.crowd.shown(now) {
                if e.role != "avatar" || e.pos.distance(self.runtime.player.pos) > 60.0 {
                    continue;
                }
                out.push(hud::Overhead {
                    feet: e.pos,
                    lift: 2.2,
                    name: Some(match xp::trainer_level_tag(snapshot, &e.pubkey) {
                        Some(level) => format!("{} · {level}", s.name_of(&e.pubkey)),
                        None => s.name_of(&e.pubkey),
                    }),
                    name_step: if e.online {
                        Intensity::Half
                    } else {
                        Intensity::Quarter
                    },
                    bubble: s
                        .bubbles
                        .iter()
                        .find(|b| b.pubkey == e.pubkey)
                        .map(|b| b.text.clone()),
                });
            }
        }
        out.extend(landmark_overheads(self.runtime.player.pos));
        let gym = self.runtime.gym(1.0);
        out.push(hud::Overhead {
            feet: crate::world::GYM_ENTRANCE,
            lift: 8.6,
            name: Some(if gym.inside {
                "GYM · G opens the board".into()
            } else {
                "GYM · enter through the west door".into()
            }),
            name_step: Intensity::ThreeQuarters,
            bubble: None,
        });
        if let Some(r) = &self.replay {
            out.extend(replay_overheads(r, &self.runtime.agent, &self.ghost));
        }
        let board = world::QUEST_BOARD;
        if board.distance(self.runtime.player.pos) <= 80.0 {
            let near = board.distance(self.runtime.player.pos) <= xp::BOARD_REACH;
            out.push(hud::Overhead {
                feet: board,
                lift: 5.4,
                name: Some(if near && !self.board_open {
                    "QUEST BOARD · press B to read".to_owned()
                } else {
                    "QUEST BOARD".to_owned()
                }),
                name_step: if near {
                    Intensity::Full
                } else {
                    Intensity::Half
                },
                bubble: None,
            });
        }
        if let Some(feed) = &self.feed {
            for v in &feed.visitors {
                if v.pos.distance(self.runtime.player.pos) > 60.0 {
                    continue;
                }
                out.push(hud::Overhead {
                    feet: v.pos,
                    lift: 2.2,
                    name: Some(format!("{} · nostr", feed.name_of(&v.pubkey))),
                    name_step: Intensity::Quarter,
                    bubble: v.bubble.as_ref().map(|(t, _)| t.clone()),
                });
            }
        }
        out
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            if let Some(mount) = &mut self.mount {
                let _ = mount.set_active(true);
            }
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
        let atlas = ui_atlas(self.scale);
        self.atlas_revision = atlas.revision();
        if let Err(error) = self.open_surface(window.clone(), &atlas) {
            self.error = Some(error);
            event_loop.exit();
            return;
        }
        self.map_atlas = atlas.layout_at_scale(self.scale);
        self.atlas = Some(atlas);
        window.focus_window();
        let size = window.inner_size();
        let mount = Viewport::new(size.width, size.height, self.scale)
            .and_then(|viewport| SurfaceLifecycle::new("verse-desktop", viewport));
        match mount {
            Ok(mut mount) => {
                let _ = mount.set_active(true);
                self.mount = Some(mount);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                event_loop.exit();
            }
        }
        self.window = Some(window);
        // `--everglade` enters once, from the first window; a later resume
        // leaves the player where they are.
        if std::mem::take(&mut self.connection_options.everglade) {
            self.everglade_pending = Some(zones::ZoneId::Everglade);
            self.open_pending_everglade();
        }
        // `--grove` likewise, through the same pending load.
        if std::mem::take(&mut self.connection_options.meteor_stress_test) {
            self.everglade_pending = Some(zones::ZoneId::MeteorStressTest);
            self.open_pending_everglade();
        }
        if std::mem::take(&mut self.connection_options.meteor_showcase) {
            self.everglade_pending = Some(zones::ZoneId::MeteorShowcase);
            self.open_pending_everglade();
        }
        if std::mem::take(&mut self.connection_options.grove) {
            self.everglade_pending = Some(zones::ZoneId::Grove);
            self.open_pending_everglade();
        }
        // `--crypt` likewise: the crypt loads Everglade's pack for its
        // character.
        if std::mem::take(&mut self.connection_options.crypt) {
            self.everglade_pending = Some(zones::ZoneId::Crypt);
            self.open_pending_everglade();
        }
        // The coast and cove load Everglade's pack for their character.
        if std::mem::take(&mut self.connection_options.coast) {
            self.everglade_pending = Some(zones::ZoneId::Coast);
            self.open_pending_everglade();
        }
        if std::mem::take(&mut self.connection_options.water_lab) {
            self.everglade_pending = Some(zones::ZoneId::WaterLab);
            self.open_pending_everglade();
        }
    }

    fn suspended(&mut self, _: &ActiveEventLoop) {
        self.suspend_world();
        if let Some(mount) = &mut self.mount {
            let _ = mount.set_active(false);
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => self.quit(event_loop),
            WindowEvent::Resized(size) => {
                self.release_buttons();
                self.companion_press = None;
                self.door_press = None;
                self.door_hud.clear_contacts();
                self.door_frame = None;
                self.zone_hud.clear_contacts();
                self.zone_frame = None;
                self.zone_press = None;
                self.runtime.doors.cancel_transient();
                self.map.clear_contacts();
                self.map_frame = None;
                if size.width == 0 || size.height == 0 {
                    self.suspend_world();
                }
                let resized = Viewport::new(size.width, size.height, self.scale)
                    .and_then(|v| self.mount.as_mut().map_or(Ok(()), |m| m.resize(v)));
                match resized {
                    Ok(()) => {
                        if let Some(renderer) = &mut self.renderer
                            && let Err(error) = renderer.resize(size.width, size.height)
                        {
                            self.error = Some(error);
                        }
                        if let Some(grid) = &mut self.grid
                            && let Err(error) = grid.resize(size.width, size.height)
                        {
                            self.error = Some(error);
                        }
                    }
                    Err(error) => {
                        self.error = Some(error.to_string());
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                if let PhysicalKey::Code(code) = event.physical_key
                    && self.terminal_key(&crate::terminal::KeyIn {
                        code,
                        logical: event.logical_key.clone(),
                        text: event.text.as_ref().map(ToString::to_string),
                        plain: plain_key(&event),
                        pressed: event.state == ElementState::Pressed,
                        repeat: event.repeat,
                        synthetic: is_synthetic,
                    })
                {
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key
                    && self.panel_key(
                        code,
                        event.state == ElementState::Pressed,
                        event.text.as_deref(),
                    )
                {
                    return;
                }
                if self.workshop_key(&event, is_synthetic) {
                    return;
                }
                if self.chat.open {
                    self.chat_key(&event);
                } else if let PhysicalKey::Code(code) = event.physical_key
                    && !event.repeat
                {
                    self.key(code, event.state == ElementState::Pressed, event_loop);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = [position.x as f32, position.y as f32];
                self.terminal.pointer(self.cursor);
                if let Some(tap) = &mut self.companion_press {
                    tap.moved(self.cursor.map(|value| value / self.scale));
                }
                if let Some((_, tap)) = &mut self.door_press {
                    tap.moved(self.cursor.map(|v| v / self.scale));
                }
                self.door_hud.moved(1, self.cursor.map(|v| v / self.scale));
                self.zone_hud.moved(1, self.cursor.map(|v| v / self.scale));
                if let Some(tap) = &mut self.zone_press {
                    tap.moved(self.cursor.map(|v| v / self.scale));
                }
                self.map
                    .moved(1, self.cursor.map(|value| value / self.scale));
                if let Some(panel) = &mut self.panel {
                    panel.moved(self.cursor.map(|v| v / self.scale));
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => {
                self.terminal.modifiers(modifiers.state());
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.button(button, state == ElementState::Pressed);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.zone_press = None;
                self.companion_press = None;
                self.door_press = None;
                let panel_lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                if self.terminal.wheel(self.cursor, panel_lines) {
                    return;
                }
                let at = self.cursor.map(|v| v / self.scale);
                if self
                    .panel
                    .as_mut()
                    .is_some_and(|panel| panel.wheel(at, panel_lines))
                {
                    return;
                }
                if self.cursor_on_map()
                    || self.map.captured(1)
                    || self.cursor_on_door_hud()
                    || self.door_hud.captured(1)
                    || self.cursor_on_zone_hud()
                    || self.zone_hud.captured(1)
                    || self.runtime.zone_loading()
                {
                    return;
                }
                let lines = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / 40.0,
                };
                let [x, y] = self.cursor;
                if self.gym_open {
                    self.gym_scroll = (self.gym_scroll as i64 - (lines * 3.0).round() as i64)
                        .clamp(0, 512) as usize;
                } else if self.board_open
                    && self.layout.board.is_some_and(|(r, _)| r.contains(x, y))
                {
                    self.scroll_board((-lines * 3.0).round() as i32);
                } else {
                    let _ = self.runtime.apply(Action::Zoom { lines });
                }
            }
            WindowEvent::Focused(focused) => {
                self.window_focused = focused;
                self.release_buttons();
                if !focused {
                    self.suspend_world();
                    self.runtime.grove_release();
                }
                self.open_pending_everglade();
                if let Some(mount) = &mut self.mount {
                    let _ = mount.set_active(focused);
                }
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                self.companion_press = None;
                self.door_press = None;
                self.door_hud.clear_contacts();
                self.door_frame = None;
                self.zone_hud.clear_contacts();
                self.zone_frame = None;
                self.zone_press = None;
                self.map.clear_contacts();
                self.map_frame = None;
                let changed = (self.scale - scale_factor as f32).abs() > f32::EPSILON;
                self.scale = scale_factor as f32;
                // Text is rasterized at the display's backing scale: moving
                // between displays rasterizes it again, so it stays crisp.
                if changed
                    && let Some(window) = self.window.clone()
                    && self.atlas.is_some()
                {
                    let atlas = ui_atlas(self.scale);
                    if let Err(error) = self.open_surface(window, &atlas) {
                        self.error = Some(error);
                    }
                    self.atlas_revision = atlas.revision();
                    self.map_atlas = atlas.layout_at_scale(self.scale);
                    if let Some((size, _)) = self.viewport() {
                        self.terminal.fit(&atlas, size);
                    }
                    self.atlas = Some(atlas);
                }
                if let Some(window) = &self.window {
                    let size = window.inner_size();
                    if let Ok(viewport) = Viewport::new(size.width, size.height, self.scale)
                        && let Some(mount) = &mut self.mount
                    {
                        let _ = mount.resize(viewport);
                    }
                }
            }
            WindowEvent::RedrawRequested => self.frame(),
            WindowEvent::CursorLeft { .. } => {
                self.zone_press = None;
                self.companion_press = None;
                self.door_press = None;
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.mouse(dx as f32, dy as f32);
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let drawable = self
            .mount
            .as_ref()
            .is_some_and(|m| m.active() && m.viewport().drawable());
        event_loop.set_control_flow(if drawable {
            ControlFlow::Poll
        } else {
            ControlFlow::Wait
        });
        if drawable && let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

/// What a key types with no modifiers, which the terminal sends after
/// Escape when Option acts as Meta.
fn plain_key(event: &winit::event::KeyEvent) -> Option<String> {
    #[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
    {
        use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
        match event.key_without_modifiers() {
            winit::keyboard::Key::Character(text) => Some(text.to_string()),
            _ => None,
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
    {
        let _ = event;
        None
    }
}

/// Pick the visible point independently from the anchor used by contextual controls.
fn point_door(
    runtime: &WorldRuntime,
    aspect: f32,
    point: [f32; 2],
    entities: &crate::mesh::Mesh,
) -> Option<DoorId> {
    DoorId::ALL
        .into_iter()
        .filter(|&id| runtime.door_hit_with_entities(id, aspect, point[0], point[1], entities))
        .min_by(|&a, &b| {
            runtime
                .door(a, aspect)
                .distance
                .total_cmp(&runtime.door(b, aspect).distance)
        })
}

/// Keep the desktop row order and its keyboard selection in agreement.
fn prioritize_gym_runs(view: &mut crate::gym::BoardView) {
    view.runs.sort_by_key(|run| match run.category {
        gym_bridge::Category::Agent => 0,
        gym_bridge::Category::Evaluation => 1,
        gym_bridge::Category::Training => 2,
    });
}

/// Read an operator-selected connection only after entering the Gym.
fn read_gym_connection(path: &std::path::Path) -> Result<String, String> {
    use std::io::Read;
    use std::os::unix::fs::OpenOptionsExt;
    const CAP: usize = 64 * 1024;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "The Gym connection file could not be opened.".to_owned())?;
    let metadata = file
        .metadata()
        .map_err(|_| "The Gym connection file could not be inspected.".to_owned())?;
    if !metadata.is_file() || metadata.len() > CAP as u64 {
        return Err("The Gym connection must be a regular file of at most 64 KiB.".into());
    }
    let mut bytes = Vec::new();
    file.take((CAP + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| "The Gym connection file could not be read.".to_owned())?;
    if bytes.len() > CAP {
        return Err("The Gym connection exceeds 64 KiB.".into());
    }
    String::from_utf8(bytes).map_err(|_| "The Gym connection file must contain UTF-8 text.".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offline_app() -> App {
        App::new(&Options {
            profile: format!("zone-test-offline-{}", std::process::id()),
            relay: None,
            ..Options::default()
        })
        .expect("offline desktop state")
    }

    #[test]
    fn a_resize_or_focus_change_forgets_a_held_button() {
        let mut app = offline_app();
        // Maximizing can deliver a press without its release.
        app.keys.left_button = true;
        app.release_buttons();
        assert!(!app.keys.left_button && !app.keys.right_button);
        app.keys.right_button = true;
        app.release_buttons();
        assert!(!app.keys.left_button && !app.keys.right_button);
    }

    #[test]
    fn a_focused_panel_takes_keys_and_presses_from_the_character() {
        let mut app = offline_app();
        app.scale = 1.0;
        app.toggle_panel();
        let panel = app.panel.as_mut().unwrap();
        let _ = panel.image([1280, 800], 1.0).unwrap();
        let bounds = panel.bounds();
        // Unfocused, keys still move the character.
        assert!(!app.panel_key(KeyCode::KeyW, true, None));
        // A press inside focuses the panel and stops the character.
        app.keys.w = true;
        app.cursor = [bounds.x + bounds.w / 2.0, bounds.y + bounds.h / 2.0];
        assert!(app.panel_button(true));
        assert!(app.panel_button(false));
        assert!(!app.keys.w && !app.keys.input().forward);
        for code in [KeyCode::KeyW, KeyCode::KeyA, KeyCode::Space, KeyCode::KeyT] {
            assert!(app.panel_key(code, true, None));
            assert!(app.panel_key(code, false, None));
        }
        assert!(!app.keys.input().forward && !app.keys.input().left);
        assert!(!app.chat.open);
        // Escape gives focus back; the world handles keys again.
        assert!(app.panel_key(KeyCode::Escape, true, None));
        assert!(app.panel.as_ref().is_some_and(|p| !p.focused()));
        assert!(!app.panel_key(KeyCode::KeyW, true, None));
        // A press outside the panel is the world's.
        app.cursor = [4.0, 4.0];
        assert!(!app.panel_button(true));
        assert!(!app.panel_button(false));
        app.toggle_panel();
        assert!(app.panel.is_none());
    }

    #[test]
    fn a_focused_terminal_takes_movement_keys_from_the_character() {
        use winit::keyboard::{Key, SmolStr};
        let mut app = offline_app();
        let key = |code, c: &str| crate::terminal::KeyIn {
            code,
            logical: Key::Character(SmolStr::new(c)),
            text: Some(c.to_owned()),
            plain: Some(c.to_owned()),
            pressed: true,
            repeat: false,
            synthetic: false,
        };
        // Closed, the world has every key.
        assert!(!app.terminal_key(&key(KeyCode::KeyW, "w")));
        app.keys.w = true;
        app.toggle_terminal();
        assert!(app.terminal.open && app.terminal.focused);
        assert!(!app.keys.w);
        for (code, c) in [
            (KeyCode::KeyW, "w"),
            (KeyCode::KeyA, "a"),
            (KeyCode::KeyS, "s"),
            (KeyCode::KeyD, "d"),
            (KeyCode::KeyT, "t"),
        ] {
            assert!(app.terminal_key(&key(code, c)));
        }
        assert!(!app.keys.input().forward && !app.keys.input().left);
        assert!(!app.chat.open);
        // Hidden, the world has keys again; sessions would keep running.
        app.toggle_terminal();
        assert!(!app.terminal.open);
        assert!(!app.terminal_key(&key(KeyCode::KeyW, "w")));
    }

    /// The committed, pinned Everglade pack's file name and path.
    fn everglade_pack() -> (String, std::path::PathBuf) {
        use zones::everglade_pack::{PACK_DIRECTORY, PACK_EXTENSION, PACK_SHA256};
        let name = format!("{PACK_SHA256}.{PACK_EXTENSION}");
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(PACK_DIRECTORY)
            .join(&name);
        (name, path)
    }

    #[test]
    fn zone_desktop_gates_plaza_services_and_returns_to_the_same_pose() {
        let mut app = offline_app();
        let plaza = app.runtime.player;
        app.runtime.install_lagrange();
        app.sync_zone_services(true);
        assert!(app.plaza_services_paused);
        assert!(app.session.is_none() && app.feed.is_none() && app.xp.is_none());
        app.open_chat("zone text");
        app.ask_agent("must not reach a model");
        app.open_picker();
        app.update_gym(true);
        assert!(!app.chat.open && !app.brain.thinking);
        assert!(app.choices.is_none() && app.picker.is_none());
        assert!(!app.gym_identity_attempted);
        assert!(app.overheads(Instant::now()).is_empty());
        app.keys.w = true;
        app.zone_action(ZoneIntent::Return);
        assert!(app.runtime.is_plaza());
        assert!(!app.plaza_services_paused && !app.keys.w);
        assert_eq!(app.runtime.player.pos, plaza.pos);
        assert_eq!(app.runtime.player.yaw, plaza.yaw);
        assert!(app.session.is_none(), "offline return does not connect");
    }

    #[test]
    fn lagrange_1_desktop_joins_the_zones_shared_world_and_the_return_leaves_it() {
        let relay = crate::loopback::LoopbackRelay::start();
        let home = std::env::temp_dir().join(format!(
            "verse-desktop-zone-presence-{}",
            crate::identity::random_hex(8)
        ));
        std::fs::create_dir_all(&home).unwrap();
        // SAFETY: tests in this module run on the test harness's threads only.
        unsafe { std::env::set_var("VERSE_HOME", &home) };
        let mut app = App::new(&Options {
            profile: format!("zone-test-{}", std::process::id()),
            relay: Some(relay.url.clone()),
            ..Options::default()
        })
        .expect("desktop state");
        app.runtime.install_lagrange();
        app.sync_zone_services(true);
        assert!(app.plaza_services_paused && app.feed.is_none() && app.xp.is_none());
        let zone_world = zones::ZoneId::Lagrange1.world_id();
        let session = app.session.as_ref().expect("zone presence");
        assert_eq!(session.world(), zone_world);
        assert_eq!(zone_world, "verse-lagrange-1");

        let other =
            crate::identity::Identity::from_secret("zoned", crate::identity::random_secret())
                .unwrap();
        let mut zoned = Session::start_presence(other, &relay.url, zone_world).unwrap();
        let mut zoned_player = PlayerController::new(app.runtime.player.pos, 0.0);
        zoned.set_display_name(Some("Zed"));
        let agent = Agent::new(&zoned_player);
        let started = Instant::now();
        let mut met = false;
        while started.elapsed() < Duration::from_secs(8) && !met {
            let now = Instant::now();
            zoned.tick(now, &zoned_player, &agent);
            zoned_player.pos.x += 0.01;
            if let Some(session) = &mut app.session {
                session.tick(now, &app.runtime.player, &app.runtime.agent);
                met = session
                    .crowd
                    .shown(now)
                    .iter()
                    .any(|e| e.pubkey == zoned.pubkey())
                    && session.name_of(zoned.pubkey()) == "Zed";
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(met, "the zone's other player never appeared");
        let tags = app.overheads(Instant::now());
        assert!(
            tags.iter()
                .any(|tag| tag.name.as_deref().is_some_and(|n| n.starts_with("Zed"))),
            "no name tag for the zone's other player: {:?}",
            tags.iter().map(|t| t.name.clone()).collect::<Vec<_>>()
        );

        app.zone_action(ZoneIntent::Return);
        assert!(app.runtime.is_plaza());
        assert_eq!(
            app.session.as_ref().map(Session::world),
            Some(session::WORLD),
            "the return rejoins the plaza's world"
        );
        unsafe { std::env::remove_var("VERSE_HOME") };
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn desktop_background_cancels_loading_without_a_render_frame() {
        let mut app = offline_app();
        let cache = std::env::temp_dir().join(format!(
            "verse-desktop-zone-{}",
            crate::identity::random_hex(8)
        ));
        std::fs::create_dir_all(&cache).unwrap();
        let (name, pack) = everglade_pack();
        std::fs::copy(pack, cache.join(name)).unwrap();
        app.runtime.configure_zone_cache(cache.clone());
        let everglade = zones::ZoneId::Plaza
            .portals()
            .into_iter()
            .find(|&(zone, _)| zone == zones::ZoneId::Everglade)
            .unwrap()
            .1;
        app.runtime.player.pos = everglade + Vec3::new(0.0, 0.0, -3.0);
        app.zone_action(ZoneIntent::Enter);
        assert!(app.runtime.zone_loading() && app.plaza_services_paused);
        assert!(!app.map_visible());
        app.keys.w = true;
        app.zone_press = Some(CompanionPress::new([0.0, 0.0], Instant::now()));
        app.suspend_world();
        assert!(!app.runtime.zone_loading());
        assert!(app.runtime.is_plaza());
        assert!(!app.keys.w && app.zone_press.is_none());
        // Late completion cannot change the selected world after suspension.
        std::thread::sleep(Duration::from_millis(100));
        assert!(!app.runtime.zone_tick());
        assert!(app.runtime.is_plaza());
        std::fs::remove_dir_all(cache).unwrap();
    }

    #[test]
    fn desktop_door_pick_accepts_visible_edges_around_an_occluded_anchor() {
        let mut runtime = WorldRuntime::new();
        let aspect = 1.6;
        for id in DoorId::ALL {
            runtime.player.pos = id.position() + Vec3::new(0.0, 0.0, -3.0);
            let view = runtime.view(aspect);
            let anchor = id.plane() + Vec3::Y * 0.8;
            let edge = anchor + Vec3::X;
            let project = |point: Vec3| {
                let clip = view.view_proj * point.extend(1.0);
                [0.5 + clip.x / clip.w * 0.5, 0.5 - clip.y / clip.w * 0.5]
            };
            let mut entities = crate::mesh::Mesh::default();
            entities.cube(
                glam::Mat4::from_translation((view.eye + anchor) * 0.5)
                    * glam::Mat4::from_scale(Vec3::splat(0.12)),
                coder_ui::theme::Intensity::Half,
            );
            assert_eq!(
                point_door(&runtime, aspect, project(anchor), &entities),
                None
            );
            assert_eq!(
                point_door(&runtime, aspect, project(edge), &entities),
                Some(id)
            );
            assert_eq!(
                point_door(&runtime, aspect, [f32::NAN, 0.5], &entities),
                None
            );
        }
    }

    #[test]
    fn companion_click_rejects_long_holds_and_returning_drags() {
        let now = Instant::now();
        let point = [400.0, 300.0];
        assert!(
            CompanionPress::new(point, now)
                .released([403.0, 302.0], now + Duration::from_millis(200))
        );
        assert!(!CompanionPress::new(point, now).released(point, now + Duration::from_millis(301)));
        let mut drag = CompanionPress::new(point, now);
        drag.moved([410.0, 300.0]);
        assert!(!drag.released(point, now + Duration::from_millis(100)));
        let mut backtrack = CompanionPress::new(point, now);
        backtrack.motion(5.0, 0.0);
        backtrack.motion(-5.0, 0.0);
        assert!(!backtrack.released(point, now + Duration::from_millis(100)));
        let mut invalid = CompanionPress::new(point, now);
        invalid.motion(f32::NAN, 0.0);
        assert!(!invalid.released(point, now + Duration::from_millis(100)));
    }

    #[test]
    fn gym_prioritizes_agent_and_evaluation_rows_without_reordering_each_group() {
        let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let mut board = crate::gym::Board::new(secret, true);
        board.set_active(true);
        let mut view = board.view();
        let mut run = view.runs[0].clone();
        view.runs.clear();
        for (id, category) in [
            ("training", gym_bridge::Category::Training),
            ("first", gym_bridge::Category::Agent),
            ("evaluation", gym_bridge::Category::Evaluation),
            ("second", gym_bridge::Category::Agent),
        ] {
            run.id = id.into();
            run.category = category;
            view.runs.push(run.clone());
        }
        prioritize_gym_runs(&mut view);
        assert_eq!(
            view.runs.iter().map(|r| r.id.as_str()).collect::<Vec<_>>(),
            ["first", "second", "evaluation", "training"]
        );
    }

    #[test]
    fn gym_connection_files_are_bounded_regular_utf8_and_do_not_follow_links() {
        use std::os::unix::fs::symlink;
        let dir = std::env::temp_dir().join(format!(
            "verse-gym-file-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("connection");
        std::fs::write(&file, b"synthetic code").unwrap();
        assert_eq!(read_gym_connection(&file).unwrap(), "synthetic code");
        let link = dir.join("link");
        symlink(&file, &link).unwrap();
        assert!(read_gym_connection(&link).is_err());
        assert!(read_gym_connection(&dir).is_err());
        std::fs::write(&file, [255]).unwrap();
        assert!(read_gym_connection(&file).is_err());
        std::fs::write(&file, vec![b'x'; 64 * 1024 + 1]).unwrap();
        assert!(read_gym_connection(&file).is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn the_replay_list_marks_the_chosen_run_and_keeps_it_in_view() {
        let run = replay::find("microcoder/embedding-drift-monitor-1790393791").unwrap();
        let choice = |n: usize| replay::Choice {
            run: run.clone(),
            line: format!("run {n} · $0.02 vs $0.74 · 2m vs 2m 19s"),
            labels: "in-sample · knowledge-assisted · OpenRouter · billed cost".into(),
        };
        let mut picker = Picker {
            choices: (0..12).map(choice).collect(),
            selected: 5,
            notice: None,
        };
        let lines = picker.lines();
        let chosen: Vec<&str> = lines
            .iter()
            .map(|(t, _)| t.as_str())
            .filter(|t| t.starts_with('>'))
            .collect();
        assert_eq!(chosen, ["> run 5 · $0.02 vs $0.74 · 2m vs 2m 19s"]);
        assert!(
            lines.iter().any(|(t, _)| t.contains("[in-sample")),
            "labels print"
        );
        assert_eq!(picker.scroll(), 4);
        picker.selected = 0;
        assert_eq!(picker.scroll(), 0);
        picker.choices.clear();
        assert!(picker.lines()[0].0.starts_with("No retained"));
    }
}

/// The HUD and terminal glyph atlas, rasterized at `scale` physical pixels
/// per point so text is crisp on a Retina display.
fn ui_atlas(scale: f32) -> Atlas {
    let mut atlas = Atlas::new((14.0 * scale).round());
    if let Err(error) = zones::everglade::hotbar::add_sprites(&mut atlas) {
        eprintln!("verse: Everglade's hotbar has no icons: {error}");
    }
    if let Err(error) = zones::grove::hotbar::add_sprites(&mut atlas) {
        eprintln!("verse: the Grove's hotbar has no icons: {error}");
    }
    if let Err(error) = zones::water::hotbar::add_sprites(&mut atlas) {
        eprintln!("verse: the Water Lab's hotbar has no icons: {error}");
    }
    if let Err(error) = zones::everglade::demolition::hotbar::add_sprites(&mut atlas) {
        eprintln!("verse: the demolition yard's hotbar has no icons: {error}");
    }
    // Room for the terminal's fallback glyphs (CJK, emoji, symbols),
    // rasterized when a pane first shows them.
    if let Err(error) = atlas.reserve_glyphs(crate::terminal::GLYPH_ROWS) {
        eprintln!("verse: the terminal has no room for fallback glyphs: {error}");
    }
    atlas
}
