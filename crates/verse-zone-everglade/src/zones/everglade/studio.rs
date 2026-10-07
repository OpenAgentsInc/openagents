//! The Agent Studio in Everglade (`docs/verse/agent-studio.md`, "The studio
//! in Verse"; `docs/verse/everglade.md`, "The workspace").
//!
//! The host's studio coordinator is the source of truth, and this module is
//! a view of it. A [`Source`] hands the view NIP-HOST studio snapshots
//! (`coder_access::studio`): `fixture` plays the simulated team, and
//! `live` reads a host over its same-user control socket. The view never
//! sends state back; a panel sends intents ([`intents`]) through
//! [`Studio::send`], each one NIP-HOST operation the host checks against
//! its right, and the host's answer comes back as [`Studio::status`]. From
//! each snapshot it draws:
//!
//! - Every seat as a character at the station its activity names. The
//!   host already classified the seat's newest ATIF step with
//!   `atif::classify`, so the snapshot's station is the place; this module
//!   only turns it into a standing point and a [`Posture`]: typing
//!   standing at its desk, reading at the library, leaning at the proving ground,
//!   waiting at the podium. Seats walk the zone's navigation around
//!   obstacles, and run a long way. When a seat's station changes while it
//!   is still walking to the last one, or the walk would take longer than
//!   [`MAX_WALK`], it skips ahead to the latest station: movement presents
//!   the activity stream and never lags it. A waiting seat walks over to
//!   the player when the player comes near the podium. The zone draws each
//!   seat from [`Studio::figures`] as the pack's character
//!   (`player::Cast`); without the pack, it is a tinted boxy figure.
//! - A nameplate over each seat with its name, activity, and route, and a
//!   lamp whose color is the seat's [`Attention`]. A seat waiting on a
//!   person also raises a beacon, so it reads from across the glade. Over
//!   them stand a pulsing "!" on the seat that owns the oldest decision
//!   ([`marked`]) and a speech bubble while the seat speaks to a seat or to
//!   the person ([`Speech`]), and around the seat drift particles of its
//!   state ([`Particles`]).
//! - The Task Wall's cards and each desk monitor's log tail, through
//!   `boards::live`.
//!
//! The world carries the glanceable state; detail lives in the panel a
//! station opens ([`PanelKind`]), which `verse::panels::studio` fills.
//!
//! The studio observes only while the player is in Everglade
//! ([`Studio::set_active`]), as the Gym's boards load only while the player
//! is inside: leaving stops the source and drops every seat, card, and log
//! line it drew.

#[cfg(test)]
use super::{STATIONS, height};
use super::{
    boards,
    layout::{DESKS, Desk},
};
use crate::{
    avatar::{self, Gait},
    controller::Footprint,
    mesh::{Mesh, Vertex},
};
use coder_access::review::TaskReview;
use coder_access::studio::{self as wire, Activity, Snapshot, View};
use coder_access::{Code, Error as AccessError, Operation, Outcome, Right};
use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};

#[cfg(feature = "model-host")]
pub mod fixture;
pub mod intents;
#[cfg(feature = "studio-host")]
pub mod live;
#[cfg(test)]
mod tests;

use verse_world::social::seats::Walker;
pub use verse_world::social::seats::{
    APPROACH, APPROACH_GAP, DEPART, MAX_WALK, RUN_DISTANCE, RUN_SPEED, SeatPose, WALK_SPEED,
};
pub use verse_world::social::studio::PanelAccess;
pub use verse_world::social::studio::{DESK_ASIDE, SLOT, place_id, sharing, standing};
/// How long a speech bubble shows, s.
pub const SPEECH_SECONDS: f32 = 7.0;
/// The most characters a speech bubble's line shows.
const SPEECH_CHARS: usize = 22;
/// The most lines a speech bubble shows under its addressee.
const SPEECH_LINES: usize = 3;
/// How near a seat that is not busy turns its head to the player, m.
const NOTICE: f32 = 3.5;
/// How many particles a seat's state shows at once.
const PARTICLES: usize = 7;
/// How far from a desk's standing point the player stands to use that
/// seat's desk, m.
pub const DESK_RANGE: f32 = 1.4;
/// Gap between the top of a seat's nameplate and its lamp, m. The lamp
/// hangs over the plate, which moves with the eye, so they never overlap.
const LAMP_GAP: f32 = 0.12;
/// Height of the bottom of a seat's nameplate over its feet, m, for an
/// eye above it.
const PLATE: f32 = 2.45;
/// The lowest a nameplate hangs over a seat's feet, m: just over its head.
/// Under the hall's low camera a plate hangs lower than [`PLATE`], so it
/// stays inside the view instead of across its top edge.
const PLATE_LOW: f32 = 2.0;
/// A nameplate's height at full size, m: its three rows.
const PLATE_TALL: f32 = 0.42;
/// The most a nameplate subtends vertically at the eye, radians. Nearer
/// than `PLATE_TALL / PLATE_ANGLE` (6 m), a plate shrinks to keep this, as
/// the plaza's overhead names keep one size on screen.
pub const PLATE_ANGLE: f32 = 0.07;
/// Nearer the eye than this, m, a nameplate is not drawn.
const PLATE_HIDE: f32 = 1.0;
/// How far a nameplate stands out of its seat toward the eye, m, so the
/// seat's lamp does not cover it.
const PLATE_OUT: f32 = 0.2;
/// Height of a waiting seat's beacon over its feet, m.
const BEACON: f32 = 7.0;
/// The most characters a nameplate line shows.
const PLATE_CHARS: usize = 24;
/// The workshop agent's seat name, which a desktop window adds as a
/// resident seat (`crate::workshop`): Alice, drawn as her own character.
pub const WORKSHOP_AGENT: &str = "alice";
/// How near the workshop agent the player stands to talk to her, m.
pub const TALK_REACH: f32 = 2.4;

/// Where a view's studio comes from: a host connection, or a fixture.
///
/// A source does its own I/O off the frame. [`Source::poll`] returns at
/// once, with the studio as it is now when it changed since the last poll.
pub trait Source: Send {
    /// The player entered Everglade: start observing.
    fn start(&mut self) {}
    /// The player left Everglade, or the surface went inactive: stop
    /// observing. A later [`Source::start`] begins again from a snapshot.
    fn stop(&mut self) {}
    /// The studio now, when it changed. `dt` is the seconds since the last
    /// poll, for a source that plays a recording.
    fn poll(&mut self, dt: f32) -> Option<Snapshot>;
    /// The review the source last read of `task` (`studio.review.open`),
    /// when it holds one. A host source asks for one it does not hold, and
    /// it arrives in a later call.
    fn review(&mut self, task: &str) -> Option<TaskReview>;
    /// Whether the source is reachable without a known read failure.
    fn available(&self) -> bool {
        true
    }
    /// Whether task IDs belong to this computer's local run adapter.
    fn local_runs(&self) -> bool {
        false
    }
    /// The connection's independent rights; the host checks them again.
    fn rights(&self) -> &[Right] {
        &[]
    }
    /// Sends `operation`, a studio intent, to the host off the frame. What
    /// the host answers comes back from [`Source::answers`] under the
    /// returned ticket.
    ///
    /// # Errors
    /// `unsupported` from a source that only observes, and `unavailable`
    /// from one that is not observing now.
    fn send(&mut self, operation: Operation) -> Result<u64, AccessError> {
        let _ = operation;
        Err(AccessError::new(
            Code::Unsupported,
            "this studio source only observes",
        ))
    }
    /// What the host answered since the last call, oldest first: each sent
    /// intent's outcome or refusal, and a refused read.
    fn answers(&mut self) -> Vec<Answer> {
        Vec::new()
    }
}

/// What the host answered to one operation a view sent or read.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The ticket [`Source::send`] returned, or 0 for a read.
    pub ticket: u64,
    /// The operation's NIP-HOST name, such as `studio.merge.decide`.
    pub operation: &'static str,
    pub result: Result<Outcome, AccessError>,
}

/// How much a seat needs the person, most urgent first: the same order as
/// `openagents_chat_app::attention::Indicator`, which the panels' roster
/// reads, so the world's lamps and the roster agree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Attention {
    /// A question or an approval waits for the person.
    AwaitingInput,
    /// The seat's task failed, or the seat is blocked.
    Errored,
    /// The seat is working.
    Working,
    /// The seat's task finished.
    Completed,
    /// Nothing needs the person.
    Idle,
}

impl Attention {
    /// The attention a seat doing `activity` asks for.
    #[must_use]
    pub fn of(activity: Activity) -> Self {
        match activity {
            Activity::Waiting => Self::AwaitingInput,
            Activity::Failed | Activity::Blocked => Self::Errored,
            Activity::Reading
            | Activity::Editing
            | Activity::Running
            | Activity::Testing
            | Activity::Judging
            | Activity::Thinking => Self::Working,
            Activity::Done => Self::Completed,
            Activity::Idle | Activity::Paused => Self::Idle,
        }
    }

    /// The lamp's color, or `None` for no lamp.
    #[must_use]
    pub fn lamp(self) -> Option<[f32; 3]> {
        match self {
            Self::AwaitingInput => Some([1.0, 0.72, 0.18]),
            Self::Errored => Some([0.95, 0.22, 0.18]),
            Self::Working => Some([0.35, 0.9, 0.45]),
            Self::Completed => Some([0.35, 0.6, 1.0]),
            Self::Idle => None,
        }
    }
}

/// The word a nameplate spells an activity with.
#[must_use]
pub fn word(activity: Activity) -> &'static str {
    match activity {
        Activity::Idle => "idle",
        Activity::Reading => "reading",
        Activity::Editing => "editing",
        Activity::Running => "running",
        Activity::Testing => "testing",
        Activity::Judging => "judging",
        Activity::Thinking => "thinking",
        Activity::Waiting => "waiting",
        Activity::Blocked => "blocked",
        Activity::Paused => "paused",
        Activity::Done => "done",
        Activity::Failed => "failed",
    }
}

/// The panel a station, a desk, or a seat opens over the world.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PanelKind {
    /// The console, at the Task Wall's notice board.
    Console,
    /// The panel of the seat at this desk.
    Desk(u32),
    /// A seat's panel, opened by selecting the seat.
    Seat(String),
    /// The open decisions, oldest first, at the podium.
    Decisions,
    /// The diff review, at the merge station.
    Review,
    /// One task's details, opened by selecting its Task Wall card.
    Task(String),
    /// Shared memory with the pinned plan first, at the library.
    Library,
}

impl PanelKind {
    /// The panel the station with layout ID `id` opens. The desks station
    /// opens the nearest desk's seat, which [`Studio::panel_at`] resolves.
    #[must_use]
    pub fn at_station(id: &str) -> Option<Self> {
        match id {
            "task_wall" => Some(Self::Console),
            "podium" => Some(Self::Decisions),
            "merge" => Some(Self::Review),
            "library" => Some(Self::Library),
            _ => None,
        }
    }
}

/// Where a seat works at `desk`: [`DESK_ASIDE`] from the desk's standing
/// point toward the middle of the hall, at the same depth, facing the
/// bench.
#[must_use]
pub fn at_desk(desk: &Desk) -> [f32; 2] {
    verse_world::social::studio::at_desk(desk.seat)
}

fn plate_lift(feet: Vec3, eye: Vec3) -> f32 {
    // An eye level with the plate looks at its lower rows.
    (eye.y - feet.y - PLATE_TALL / 4.0).clamp(PLATE_LOW, PLATE)
}

/// Height of a seat's lamp over its feet for `eye`, m: just over the top of
/// its full-size nameplate.
fn lamp_height(feet: Vec3, eye: Vec3) -> f32 {
    plate_lift(feet, eye) + PLATE_TALL + LAMP_GAP
}

/// Plate space to the glade for the nameplate of a seat at `feet` seen
/// from `eye`, or `None` when the plate is too near the eye to draw.
///
/// The plate turns toward the eye and stands a little out of the seat
/// toward it. It hangs at [`PLATE`] for an eye above that, and lower, down
/// to [`PLATE_LOW`], for a lower eye, such as the camera under the hall's
/// ceiling. Nearer than `PLATE_TALL / PLATE_ANGLE` it shrinks, so it never
/// subtends more than about [`PLATE_ANGLE`] however near the eye comes.
pub(super) fn plate_transform(feet: Vec3, eye: Vec3) -> Option<Mat4> {
    billboard(feet, plate_lift(feet, eye), eye)
}

/// Glade space for a speech bubble over the nameplate of a seat at `feet`.
pub(super) fn over_plate(feet: Vec3, eye: Vec3) -> Option<Mat4> {
    billboard(feet, plate_lift(feet, eye) + PLATE_TALL + LAMP_GAP, eye)
}

/// The text a nameplate shows: the name, the activity, and the route.
#[must_use]
pub fn nameplate(seat: &wire::Seat) -> [String; 3] {
    let name = match seat.role {
        wire::Role::Lead => format!("{} lead", seat.seat),
        wire::Role::Worker => seat.seat.clone(),
    };
    [name, word(seat.activity).into(), plate_route(&seat.route)]
}

/// A route short enough for a nameplate line: whole as it is when it fits
/// in [`PLATE_CHARS`], else without its leading `Coder V1` and model, so
/// `Coder V1, coding on Codex` reads `Coding on Codex` rather than losing
/// its last letter.
fn plate_route(route: &str) -> String {
    if route.chars().count() <= PLATE_CHARS {
        return route.to_owned();
    }
    match route.rsplit_once(", ") {
        Some((_, tail)) if tail.chars().count() <= PLATE_CHARS => {
            let mut chars = tail.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
        _ => route.to_owned(),
    }
}

/// `text` in the in-world lettering's alphabet (A–Z, 0–9, `/`, `.`, and
/// space), upper case, at most `max` characters.
#[must_use]
pub fn lettering(text: &str, max: usize) -> String {
    text.chars()
        .map(|ch| {
            let ch = ch.to_ascii_uppercase();
            if ch.is_ascii_uppercase() || ch.is_ascii_digit() || matches!(ch, '/' | '.' | ' ') {
                ch
            } else {
                ' '
            }
        })
        .take(max)
        .collect::<String>()
        .trim_end()
        .to_owned()
}

/// A seat's figure color: the color its look names, else the lead in gold
/// and workers in turn.
fn tint(seat: &wire::Seat, index: usize) -> [f32; 3] {
    const LOOKS: [(&str, [f32; 3]); 8] = [
        ("gold", [1.0, 0.82, 0.35]),
        ("blue", [0.45, 0.75, 1.0]),
        ("rose", [0.95, 0.5, 0.8]),
        ("green", [0.55, 0.95, 0.6]),
        ("violet", [0.8, 0.6, 1.0]),
        ("orange", [1.0, 0.55, 0.4]),
        ("teal", [0.4, 0.9, 0.85]),
        ("red", [0.95, 0.35, 0.3]),
    ];
    if let Some((_, color)) = LOOKS
        .iter()
        .find(|(name, _)| seat.look.eq_ignore_ascii_case(name))
    {
        return *color;
    }
    match seat.role {
        wire::Role::Lead => LOOKS[0].1,
        wire::Role::Worker => LOOKS[1 + index % 5].1,
    }
}

/// How a seat holds itself at its station when it is not walking. The
/// zone's character plays a clip for each (`pose::authored`). Every
/// posture stands: the desks are standing desks, and a seat never sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Posture {
    /// At ease: the idle clip.
    Stand,
    /// Standing at its desk, typing.
    Type,
    /// Reading a book held open, at the library.
    Read,
    /// Bent over the ring's rail, hands on its knees, at the proving
    /// ground.
    Lean,
    /// Hands clasped, at the podium.
    Wait,
    /// A hand at the chin, at the oracle.
    Think,
    /// Hands at the wagon's bench, at the workbench.
    Work,
    /// Gesturing while it speaks.
    Talk,
}

impl Posture {
    /// Every posture, in clip order.
    pub const ALL: [Self; 8] = [
        Self::Stand,
        Self::Type,
        Self::Read,
        Self::Lean,
        Self::Wait,
        Self::Think,
        Self::Work,
        Self::Talk,
    ];

    /// The posture of a seat doing `activity` at `station`. `own_desk`
    /// holds when the station is the seat's own desk, and `speaking` while
    /// its speech bubble shows: a standing seat gestures as it speaks.
    #[must_use]
    pub fn of(activity: Activity, station: wire::Station, own_desk: bool, speaking: bool) -> Self {
        let posture = match station {
            wire::Station::Desk if own_desk => match activity {
                Activity::Reading
                | Activity::Editing
                | Activity::Running
                | Activity::Testing
                | Activity::Judging
                | Activity::Thinking => Self::Type,
                Activity::Idle
                | Activity::Waiting
                | Activity::Blocked
                | Activity::Paused
                | Activity::Done
                | Activity::Failed => Self::Stand,
            },
            wire::Station::Library => Self::Read,
            wire::Station::ProvingGround => Self::Lean,
            wire::Station::Podium => Self::Wait,
            wire::Station::Oracle => Self::Think,
            wire::Station::Workbench => Self::Work,
            wire::Station::Desk | wire::Station::Lounge | wire::Station::TaskWall => Self::Stand,
        };
        if speaking && matches!(posture, Self::Stand | Self::Wait) {
            Self::Talk
        } else {
            posture
        }
    }
}

/// The particles that drift around a seat in each state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Particles {
    /// Pale bubbles rising over the head.
    Thinking,
    /// Sparks from the hands.
    Working,
    /// Red puffs over the head.
    Error,
    /// Green sparkles spiraling up around the seat.
    Done,
}

impl Particles {
    /// The particles a seat doing `activity` shows, if any.
    #[must_use]
    pub fn of(activity: Activity) -> Option<Self> {
        match activity {
            Activity::Thinking | Activity::Judging => Some(Self::Thinking),
            Activity::Reading | Activity::Editing | Activity::Running | Activity::Testing => {
                Some(Self::Working)
            }
            Activity::Failed | Activity::Blocked => Some(Self::Error),
            Activity::Done => Some(Self::Done),
            Activity::Idle | Activity::Waiting | Activity::Paused => None,
        }
    }

    /// The particles' color.
    #[must_use]
    pub fn color(self) -> [f32; 3] {
        match self {
            Self::Thinking => [0.72, 0.85, 1.0],
            Self::Working => [1.0, 0.78, 0.3],
            Self::Error => [0.95, 0.25, 0.2],
            Self::Done => [0.45, 0.95, 0.55],
        }
    }

    /// Where particle `i` of a seat at `feet` facing `yaw` is at `clock`,
    /// and its half size, m. `seed` staggers seats.
    fn place(self, feet: Vec3, yaw: f32, clock: f32, i: usize, seed: usize) -> (Vec3, f32) {
        let rate = match self {
            Self::Thinking => 0.35,
            Self::Working => 0.9,
            Self::Error => 0.3,
            Self::Done => 0.25,
        };
        let offset = (seed as f32 * 0.37).fract();
        let t = (clock * rate + i as f32 / PARTICLES as f32 + offset).fract();
        // The golden angle spreads the particles evenly around the seat.
        let angle = i as f32 * 2.399 + seed as f32;
        let around = |a: f32| Vec3::new(a.cos(), 0.0, a.sin());
        match self {
            Self::Thinking => (
                feet + Vec3::Y * (1.95 + 0.5 * t) + around(angle + t * 3.0) * 0.12 * (1.0 + t),
                0.025 + 0.03 * t,
            ),
            Self::Working => {
                let hands = feet + Vec3::Y * 1.05 + Vec3::new(yaw.sin(), 0.0, yaw.cos()) * 0.4;
                (
                    hands + around(angle) * 0.3 * t + Vec3::Y * (0.35 * t - 0.5 * t * t),
                    0.008 + 0.02 * (1.0 - t),
                )
            }
            Self::Error => (
                feet + Vec3::Y * (1.9 + 0.6 * t) + around(angle + (clock * 2.0).sin()) * 0.15,
                0.05 + 0.04 * t,
            ),
            Self::Done => (
                feet + Vec3::Y * (0.3 + 1.8 * t) + around(angle + clock * 1.2) * 0.55,
                0.01 + 0.03 * (1.0 - t),
            ),
        }
    }
}

/// The seat that owns the decision the podium answers next
/// ([`intents::decisions`]: most urgent kind first, then oldest): the
/// asking seat, or the goal's lead for a decision about the goal.
#[must_use]
pub fn marked(view: &View) -> Option<&str> {
    let oldest = intents::decisions(view).into_iter().next()?;
    match oldest.seat.as_deref() {
        Some(seat) => Some(seat),
        None => view
            .goals
            .iter()
            .find(|goal| goal.goal == oldest.goal)
            .map(|goal| goal.lead.as_str()),
    }
}

/// Who a seat speaks to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Addressee {
    /// The person at the studio.
    Person,
    /// Another seat, by name.
    Seat(String),
}

/// One thing a seat says, shown as a bubble over it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Speech {
    pub speaker: String,
    pub to: Addressee,
    pub text: String,
}

/// What the seats say between `before` and `after`: a seat asks the
/// person each new decision it owns, and a goal's lead hands each worker
/// its newly planned task. A first view, with no `before`, says only its
/// open decisions.
#[must_use]
pub fn speeches(before: Option<&View>, after: &View) -> Vec<Speech> {
    let lead = |goal: &str| {
        after
            .goals
            .iter()
            .find(|g| g.goal == goal)
            .map(|g| g.lead.clone())
    };
    let mut said = Vec::new();
    for decision in &after.decisions {
        if before.is_some_and(|b| b.decisions.iter().any(|d| d.decision == decision.decision)) {
            continue;
        }
        let Some(speaker) = decision.seat.clone().or_else(|| lead(&decision.goal)) else {
            continue;
        };
        said.push(Speech {
            speaker,
            to: Addressee::Person,
            text: decision.text.clone(),
        });
    }
    let Some(before) = before else {
        return said;
    };
    for task in &after.tasks {
        if task.entry == "lead" || before.tasks.iter().any(|t| t.task == task.task) {
            continue;
        }
        let Some(speaker) = lead(&task.goal).filter(|lead| *lead != task.seat) else {
            continue;
        };
        said.push(Speech {
            speaker,
            to: Addressee::Seat(task.seat.clone()),
            text: task.title.clone(),
        });
    }
    said
}

/// A speech's bubble while it shows, or waits behind the speaker's last.
struct Bubble {
    speech: Speech,
    /// Seconds it still shows.
    left: f32,
    /// Its faces in plate space, as a nameplate's.
    mesh: Mesh,
}

/// The most speeches a seat queues behind the one it says now.
const SPEECH_QUEUE: usize = 3;

/// What the zone draws one seat as: where it stands, how it holds itself,
/// and where it looks.
#[derive(Clone, Debug, PartialEq)]
pub struct SeatFigure {
    /// The seat's name, its key.
    pub name: String,
    /// Its feet.
    pub pos: Vec3,
    /// Its heading, as the controller's yaw.
    pub yaw: f32,
    /// How fast it moves now, m/s; a moving seat walks or runs instead of
    /// holding its posture.
    pub speed: f32,
    pub posture: Posture,
    /// The point its head turns toward, when it looks at something.
    pub look: Option<Vec3>,
    /// Its outfit's color.
    pub tint: [f32; 3],
    /// The pack form it is drawn as in place of the player's character,
    /// such as Alice's own (`npc/alice`) for the workshop agent.
    pub form: Option<&'static str>,
}

/// One seat as the glade draws it.
struct SeatAgent {
    name: String,
    /// Where it stands and how it walks: its own walk for a lone viewer,
    /// or the world authority's pose in a hosted instance.
    walk: Walker,
    gait: Gait,
    activity: Activity,
    station: wire::Station,
    /// Its desk, and whether its station is that desk.
    desk: u32,
    own_desk: bool,
    tint: [f32; 3],
    /// The pack form its look names, if any ([`super::npcs::form_of`]).
    form: Option<&'static str>,
    /// The workshop agent, who works in the owner's house.
    home: bool,
    plate_text: [String; 3],
    /// The nameplate's faces in plate space: its face in the XY plane,
    /// facing -Z, its bottom at the origin.
    plate: Mesh,
}

impl SeatAgent {
    fn walking(&self) -> bool {
        self.walk.walking()
    }
}

/// The studio as Everglade draws it.
pub struct Studio {
    source: Option<Box<dyn Source>>,
    pub active: bool,
    snapshot: Option<Snapshot>,
    seats: Vec<SeatAgent>,
    /// The Task Wall's cards and the monitors' text, rebuilt only when the
    /// snapshot changes.
    boards: Mesh,
    /// Counts changes to what the studio shows, so a panel refreshes only
    /// when it changed.
    revision: u64,
    /// The host's newest answer to something a panel sent or read.
    status: Option<Answer>,
    /// Whether `/sound off` silenced the studio for the session.
    muted: bool,
    /// What the last snapshot showed, for the changes that ring a signal.
    signals: super::signals::Signals,
    /// Signals not yet taken ([`Studio::take_events`]).
    events: Vec<super::signals::Event>,
    /// The zone's navigation blockers, from the last poll.
    blockers: Vec<Footprint>,
    /// Where the player stands, when the zone says.
    player: Option<Vec3>,
    /// Seconds of studio time, for pulses and particles.
    clock: f32,
    /// Speeches showing and queued, oldest first.
    bubbles: Vec<Bubble>,
    /// The seat that owns the oldest decision ([`marked`]).
    marked: Option<String>,
    /// In a hosted instance, the seats as the world authority last placed
    /// them; a lone viewer walks its own seats.
    authority: Option<Vec<SeatPose>>,
    /// In a hosted instance, the NIP-HOST rights the viewer's grant holds,
    /// which its panels need; a lone viewer's panels follow its source.
    grant: Option<Vec<Right>>,
    /// The studio as the source last sent it, before resident seats join.
    hosted: Option<Snapshot>,
    /// Seats this computer draws beside the source's, such as the workshop
    /// agent at its desk (`docs/verse/workshop-agent.md`). A resident seat
    /// wins over a source seat of the same name: the workshop agent's task
    /// mode gives her a studio seat too, and her own view says more.
    resident: Vec<wire::Seat>,
    /// The workshop agent's day plan, as her host's view carries it, and
    /// the plan board in the great room drawn from it.
    plan: Option<coder_access::day_plan::DayPlan>,
    plan_board: Mesh,
}

impl Default for Studio {
    fn default() -> Self {
        Self {
            source: None,
            active: false,
            snapshot: None,
            seats: Vec::new(),
            boards: boards::live(None),
            revision: 0,
            status: None,
            muted: false,
            signals: super::signals::Signals::default(),
            events: Vec::new(),
            blockers: Vec::new(),
            player: None,
            clock: 0.0,
            bubbles: Vec::new(),
            marked: None,
            authority: None,
            grant: None,
            hosted: None,
            resident: Vec::new(),
            plan: None,
            plan_board: boards::plan_board(None),
        }
    }
}

impl Studio {
    /// Where the studio comes from. Replacing the source forgets what the
    /// last one showed; an active studio starts the new one.
    pub fn set_source(&mut self, source: Box<dyn Source>) {
        let active = self.active;
        self.set_active(false);
        self.source = Some(source);
        self.set_active(active);
    }

    /// Whether a source is configured.
    #[must_use]
    pub fn has_source(&self) -> bool {
        self.source.is_some()
    }

    /// Whether the studio observes now.
    #[must_use]
    pub fn active(&self) -> bool {
        self.active
    }

    /// The caller supplies `surface_active && in_everglade`. Entering
    /// starts the source; leaving stops it and drops what it showed.
    pub fn set_active(&mut self, active: bool) {
        if active == self.active {
            return;
        }
        self.active = active;
        if let Some(source) = &mut self.source {
            if active {
                source.start();
            } else {
                source.stop();
            }
        }
        if active && !self.resident.is_empty() {
            let blockers = self.blockers.clone();
            self.show(self.merged(), &blockers);
        }
        if !active {
            self.hosted = None;
            self.snapshot = None;
            self.seats.clear();
            self.boards = boards::live(None);
            self.status = None;
            self.signals.reset();
            self.events.clear();
            self.bubbles.clear();
            self.marked = None;
            self.revision += 1;
        }
    }

    /// The signals the snapshots taken since the last call rang, oldest
    /// first (`signals::Signals::observe`).
    pub fn take_events(&mut self) -> Vec<super::signals::Event> {
        std::mem::take(&mut self.events)
    }

    /// Where the player stands, once a frame before [`Studio::tick`], so
    /// a waiting seat can meet them and heads can turn to them.
    pub fn set_player(&mut self, player: Option<Vec3>) {
        self.player = player.filter(|p| p.is_finite());
    }

    /// Polls the source, when active, and takes a changed studio and the
    /// host's newest answer. Seats route around `blockers`.
    pub fn poll(&mut self, dt: f32, blockers: &[Footprint]) {
        if !self.active {
            return;
        }
        if self.blockers != blockers {
            self.blockers = blockers.to_vec();
        }
        let Some(source) = self.source.as_mut() else {
            return;
        };
        let snapshot = source.poll(dt);
        if let Some(answer) = source.answers().pop() {
            self.status = Some(answer);
            self.revision += 1;
        }
        if let Some(snapshot) = snapshot {
            self.apply(snapshot, blockers);
        }
    }

    /// The rights the studio's panels act under, while active: in a hosted
    /// instance, those both the viewer's grant and the source hold; for a
    /// lone viewer, the source's.
    #[must_use]
    pub fn rights(&self) -> Vec<Right> {
        let held = match &self.source {
            Some(source) if self.active => source.rights().to_vec(),
            _ => Vec::new(),
        };
        match &self.grant {
            Some(grant) => held.into_iter().filter(|r| grant.contains(r)).collect(),
            None => held,
        }
    }

    /// Joins a hosted instance whose viewer's grant holds `rights`, or
    /// leaves it with `None`. In an instance the `world` right admits
    /// walking only: panels open with `observe`, act with `operate`, and
    /// decide merges with `review` ([`Studio::access`]).
    pub fn set_grant(&mut self, rights: Option<Vec<Right>>) {
        self.grant = rights;
        self.revision += 1;
    }

    /// What the viewer may do with the studio's panels. A lone viewer reads
    /// whatever its source shows; in a hosted instance reading needs the
    /// grant's `observe`.
    #[must_use]
    pub fn access(&self) -> PanelAccess {
        let mut access = PanelAccess::of(&self.rights());
        access.read = match &self.grant {
            Some(grant) => grant.contains(&Right::Observe),
            None => true,
        };
        access
    }

    /// Draws the seats where the world authority places them, in a hosted
    /// instance, instead of walking them here: every viewer then sees the
    /// same seat at the same place. `None` returns to the local walk.
    pub fn follow_authority(&mut self, poses: Option<Vec<SeatPose>>) {
        self.authority = poses;
    }

    /// Sends `operation`, a studio intent, through the source. The host's
    /// answer becomes [`Studio::status`] at a later poll.
    ///
    /// # Errors
    /// `unavailable` while the studio is not observing, `missing_right`
    /// when the source's rights do not hold the operation's right, and
    /// the source's own refusal.
    pub fn send(&mut self, operation: Operation) -> Result<u64, AccessError> {
        let rights = self.rights();
        if self.active && !self.access().read {
            return Err(verse_world::social::studio::missing(Right::Observe));
        }
        let source = match self.source.as_mut() {
            Some(source) if self.active => source,
            _ => {
                return Err(AccessError::new(
                    Code::Unavailable,
                    "the studio is not loaded; it loads while you are in Everglade",
                ));
            }
        };
        if let Some(right) = operation.required()
            && !rights.contains(&right)
        {
            let mut refused = AccessError::new(
                Code::MissingRight,
                format!("this connection lacks the `{}` right", right.as_str()),
            );
            refused.missing = Some(right);
            return Err(refused);
        }
        source.send(operation)
    }

    /// Whether the admitted source is active and has no known read failure.
    #[must_use]
    pub fn available(&self) -> bool {
        self.active
            && self.access().read
            && self
                .source
                .as_ref()
                .is_some_and(|source| source.available())
    }

    /// Whether the admitted source uses this computer's run adapter.
    pub fn local_runs(&self) -> bool {
        self.active
            && self.access().read
            && self
                .source
                .as_ref()
                .is_some_and(|source| source.local_runs())
    }

    /// The host's newest answer to a sent operation or read.
    pub fn status(&self) -> Option<&Answer> {
        self.status.as_ref()
    }

    /// Sets the seats this computer draws beside the source's, such as
    /// the workshop agent. While the studio observes, they join the view at
    /// once, with or without a source.
    pub fn set_resident(&mut self, seats: Vec<wire::Seat>) {
        if self.resident == seats {
            return;
        }
        self.resident = seats;
        if self.active {
            let blockers = self.blockers.clone();
            self.show(self.merged(), &blockers);
        }
    }

    /// Sets the workshop agent's day plan, which the plan board in the
    /// great room shows; the board redraws only when it changed.
    pub fn set_plan(&mut self, plan: Option<coder_access::day_plan::DayPlan>) {
        if self.plan == plan {
            return;
        }
        self.plan_board = boards::plan_board(plan.as_ref());
        self.plan = plan;
        self.revision += 1;
    }

    /// The workshop agent's day plan, when her host sent one.
    #[must_use]
    pub fn plan(&self) -> Option<&coder_access::day_plan::DayPlan> {
        self.plan.as_ref()
    }

    /// The source's studio with the resident seats added.
    fn merged(&self) -> Snapshot {
        let mut snapshot = self.hosted.clone().unwrap_or_else(|| Snapshot {
            stream: "resident".into(),
            sequence: 0,
            view: View::default(),
        });
        for seat in &self.resident {
            match snapshot.view.seats.iter_mut().find(|s| s.seat == seat.seat) {
                Some(hosted) => *hosted = seat.clone(),
                None => snapshot.view.seats.push(seat.clone()),
            }
        }
        snapshot
    }

    /// Takes `snapshot` from the source as the studio now, with the
    /// resident seats added: seats walk to their stations, say what
    /// changed ([`speeches`]), and the boards redraw. A seat seen for the
    /// first time stands at its station at once.
    pub fn apply(&mut self, snapshot: Snapshot, blockers: &[Footprint]) {
        self.hosted = Some(snapshot);
        self.show(self.merged(), blockers);
    }

    /// Shows `snapshot`, the source's studio with the resident seats.
    fn show(&mut self, snapshot: Snapshot, blockers: &[Footprint]) {
        if self.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        let view = &snapshot.view;
        let said = speeches(self.snapshot.as_ref().map(|s| &s.view), view);
        let mut seats = Vec::with_capacity(view.seats.len());
        for (index, seat) in view.seats.iter().enumerate() {
            let (slot, count) = sharing(view, index);
            // The workshop agent works in the owner's house, not at a
            // workshop desk.
            let home = seat.seat == WORKSHOP_AGENT;
            let (target, facing) = if home {
                super::layout::estate::AliceSpot::of(seat.station).world()
            } else {
                standing(seat.station, seat.desk, slot, count)
            };
            let plate_text = nameplate(seat);
            let existing = self
                .seats
                .iter()
                .position(|agent| agent.name == seat.seat)
                .map(|at| self.seats.swap_remove(at));
            let mut agent = match existing {
                Some(mut agent) => {
                    agent.walk.plan(target, facing, blockers);
                    agent
                }
                None => {
                    let walk = Walker::standing(target, facing);
                    let walk = if home {
                        walk.within(super::layout::estate::alice_area())
                    } else {
                        walk
                    };
                    let agent = SeatAgent {
                        name: seat.seat.clone(),
                        walk,
                        gait: Gait::default(),
                        activity: seat.activity,
                        station: seat.station,
                        desk: seat.desk,
                        own_desk: false,
                        tint: tint(seat, index),
                        form: super::npcs::form_of(&seat.look),
                        plate_text: Default::default(),
                        plate: Mesh::default(),
                        home,
                    };
                    agent
                }
            };
            agent.activity = seat.activity;
            agent.station = seat.station;
            agent.desk = seat.desk;
            agent.own_desk =
                seat.station == wire::Station::Desk && (home || (seat.desk as usize) < DESKS.len());
            agent.home = home;
            agent.tint = tint(seat, index);
            agent.form = super::npcs::form_of(&seat.look);
            if agent.plate_text != plate_text {
                agent.plate = plate(&plate_text, Attention::of(seat.activity));
                agent.plate_text = plate_text;
            }
            seats.push(agent);
        }
        self.seats = seats;
        let events = self.signals.observe(view);
        self.events.extend(events);
        // A host that never takes them keeps only the newest.
        let over = self
            .events
            .len()
            .saturating_sub(super::signals::MAX_PENDING);
        self.events.drain(..over);
        self.boards = boards::live(Some(view));
        self.marked = marked(view).map(str::to_owned);
        self.snapshot = Some(snapshot);
        for speech in said {
            self.say(speech);
        }
        self.revision += 1;
    }

    /// Shows `speech` over its speaker, after what the speaker says now.
    fn say(&mut self, speech: Speech) {
        let queued = self
            .bubbles
            .iter()
            .filter(|b| b.speech.speaker == speech.speaker)
            .count();
        if queued > SPEECH_QUEUE {
            return;
        }
        self.bubbles.push(Bubble {
            mesh: bubble(&speech),
            left: SPEECH_SECONDS,
            speech,
        });
    }

    /// The bubbles showing now: each speaker's oldest.
    fn showing(&self) -> impl Iterator<Item = &Bubble> {
        self.bubbles.iter().enumerate().filter_map(|(i, bubble)| {
            let first = !self.bubbles[..i]
                .iter()
                .any(|b| b.speech.speaker == bubble.speech.speaker);
            first.then_some(bubble)
        })
    }

    /// What the seat named `name` says now, while its bubble shows.
    #[must_use]
    pub fn speech(&self, name: &str) -> Option<&Speech> {
        self.showing()
            .find(|b| b.speech.speaker == name)
            .map(|b| &b.speech)
    }

    /// The seat the pulsing "!" stands over: the owner of the oldest
    /// decision.
    #[must_use]
    pub fn marked_seat(&self) -> Option<&str> {
        self.marked.as_deref()
    }

    /// Walks every seat for `dt` seconds, sends a waiting seat to meet a
    /// player near its station and back once they leave, and counts down
    /// each speech.
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.clock = (self.clock + dt) % 3600.0;
        let mut counted: Vec<String> = Vec::new();
        for bubble in &mut self.bubbles {
            if counted.contains(&bubble.speech.speaker) {
                continue;
            }
            counted.push(bubble.speech.speaker.clone());
            bubble.left -= dt;
        }
        self.bubbles.retain(|b| b.left > 0.0);
        let player = self.player;
        for seat in &mut self.seats {
            match &self.authority {
                Some(poses) => {
                    if let Some(pose) = poses.iter().find(|pose| pose.name == seat.name) {
                        seat.walk.take_pose(pose);
                    }
                }
                None => {
                    let waits =
                        seat.activity == Activity::Waiting && seat.station == wire::Station::Podium;
                    seat.walk.follow(waits, player, &self.blockers);
                    seat.walk.advance(dt);
                }
            }
            seat.gait.advance(seat.walk.speed(), false, dt);
        }
    }

    /// The studio last taken, while active.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
    }

    /// The source's own snapshot, excluding this viewer's resident seats.
    /// Private consumers must check the source's observation rights.
    #[must_use]
    pub fn source_snapshot(&self) -> Option<&Snapshot> {
        self.hosted.as_ref().filter(|_| self.active)
    }

    /// The studio's view, while active and loaded.
    #[must_use]
    pub fn view(&self) -> Option<&View> {
        self.snapshot.as_ref().map(|snapshot| &snapshot.view)
    }

    /// Counts changes to what the studio shows.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Where the seat named `name` stands now.
    #[must_use]
    pub fn seat_position(&self, name: &str) -> Option<Vec3> {
        self.seats
            .iter()
            .find(|seat| seat.name == name)
            .map(|seat| seat.walk.pos())
    }

    /// Whether the seat named `name` is still walking.
    #[must_use]
    pub fn seat_walking(&self, name: &str) -> bool {
        self.seats
            .iter()
            .any(|seat| seat.name == name && seat.walking())
    }

    /// Every seat's name and where it stands, in key order.
    pub fn seats(&self) -> impl Iterator<Item = (&str, Vec3)> {
        self.seats
            .iter()
            .map(|seat| (seat.name.as_str(), seat.walk.pos()))
    }

    /// The middle of each Task Wall card, in the glade, with its task.
    #[must_use]
    pub fn cards(&self) -> Vec<(Vec3, String)> {
        self.view().map(boards::card_targets).unwrap_or_default()
    }

    /// Whether the studio's sounds play: `/sound off` at the console
    /// silences them for the session.
    #[must_use]
    pub fn sounds(&self) -> bool {
        !self.muted
    }

    /// Turns the studio's sounds on or off.
    pub fn set_sounds(&mut self, on: bool) {
        self.muted = !on;
    }

    /// Every seat as the zone draws it: where it stands, its posture, and
    /// where it looks.
    #[must_use]
    pub fn figures(&self) -> Vec<SeatFigure> {
        self.seats
            .iter()
            .map(|seat| {
                let speaking = self.speech(&seat.name);
                let posture = if seat.walking() {
                    Posture::Stand
                } else {
                    Posture::of(
                        seat.activity,
                        seat.station,
                        seat.own_desk && !seat.walk.following(),
                        speaking.is_some(),
                    )
                };
                SeatFigure {
                    name: seat.name.clone(),
                    pos: seat.walk.pos(),
                    yaw: seat.walk.yaw(),
                    speed: seat.walk.speed(),
                    posture,
                    look: self.look(seat, speaking),
                    tint: seat.tint,
                    form: seat.form,
                }
            })
            .collect()
    }

    /// Where `seat` looks: at whom it speaks to, at a seat speaking to
    /// it, at its monitor while at its desk, or at a player it waits for
    /// or who stands near.
    fn look(&self, seat: &SeatAgent, speaking: Option<&Speech>) -> Option<Vec3> {
        let head = |p: Vec3| p + Vec3::Y * 1.6;
        let player = self
            .player
            .map(|p| (p, (p.x - seat.walk.pos().x).hypot(p.z - seat.walk.pos().z)));
        if let Some(speech) = speaking {
            match &speech.to {
                Addressee::Seat(to) => {
                    if let Some(at) = self.seat_position(to) {
                        return Some(head(at));
                    }
                }
                Addressee::Person => {
                    if let Some((p, d)) = player
                        && d <= DEPART
                    {
                        return Some(head(p));
                    }
                }
            }
        }
        let listening = self
            .showing()
            .find(|b| matches!(&b.speech.to, Addressee::Seat(to) if *to == seat.name));
        if let Some(at) = listening.and_then(|b| self.seat_position(&b.speech.speaker)) {
            return Some(head(at));
        }
        if seat.own_desk && seat.home && !seat.walking() && !seat.walk.following() {
            return Some(super::layout::estate::alice_screens());
        }
        if seat.own_desk
            && !seat.walking()
            && !seat.walk.following()
            && let Some(desk) = usize::try_from(seat.desk).ok().and_then(|i| DESKS.get(i))
        {
            return Some(desk.monitor.center);
        }
        match player {
            Some((p, d)) if d <= NOTICE || (seat.activity == Activity::Waiting && d <= DEPART) => {
                Some(head(p))
            }
            _ => None,
        }
    }

    /// The review the source holds of `task`.
    pub fn review(&mut self, task: &str) -> Option<TaskReview> {
        if !self.active {
            return None;
        }
        self.source.as_mut().and_then(|source| source.review(task))
    }

    /// The panel the player standing at `at` opens with the interact key:
    /// a desk's seat within [`DESK_RANGE`] of its standing point, else the
    /// station's panel.
    #[must_use]
    pub fn panel_at(at: Vec3) -> Option<PanelKind> {
        let desk = DESKS
            .iter()
            .enumerate()
            .map(|(i, desk)| (i, (desk.seat[0] - at.x).hypot(desk.seat[1] - at.z)))
            .filter(|(_, d)| *d <= DESK_RANGE)
            .min_by(|a, b| a.1.total_cmp(&b.1));
        if let Some((i, _)) = desk {
            return Some(PanelKind::Desk(i as u32));
        }
        let station = super::station_near(at.x, at.z)?;
        if station.id == "desks" {
            let nearest = DESKS
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let da = (a.1.seat[0] - at.x).hypot(a.1.seat[1] - at.z);
                    let db = (b.1.seat[0] - at.x).hypot(b.1.seat[1] - at.z);
                    da.total_cmp(&db)
                })
                .map_or(0, |(i, _)| i);
            return Some(PanelKind::Desk(nearest as u32));
        }
        PanelKind::at_station(station.id)
    }

    /// The seats as tinted boxy figures, and what [`Studio::draw`] draws
    /// over them.
    #[must_use]
    pub fn mesh(&self, eye: Vec3) -> Mesh {
        self.draw(eye, true)
    }

    /// What the studio draws seen from `eye`: the live boards, and over
    /// each seat its nameplate and lamp, turned toward `eye` and bounded in
    /// size there ([`PLATE_ANGLE`]), its beacon, "!", speech bubble, and
    /// particles. With `boxes`, each seat is also a tinted boxy figure;
    /// without, the zone draws the seats as characters from
    /// [`Studio::figures`].
    #[must_use]
    pub fn draw(&self, eye: Vec3, boxes: bool) -> Mesh {
        let mut mesh = Mesh::default();
        mesh.extend(&self.boards);
        mesh.extend(&self.plan_board);
        let full = crate::palette::amber(Intensity::Full);
        for (index, seat) in self.seats.iter().enumerate() {
            if boxes {
                let mut figure = avatar::figure(
                    seat.walk.pos(),
                    Quat::from_rotation_y(seat.walk.yaw()),
                    &seat.gait,
                    Intensity::Full,
                );
                let recolor = |v: &mut Vertex| {
                    let k = (v.color[0] / full[0].max(0.001)).clamp(0.0, 1.5);
                    v.color = seat.tint.map(|c| c * k);
                };
                figure.lines.iter_mut().for_each(recolor);
                figure.faces.iter_mut().for_each(recolor);
                mesh.extend(&figure);
            }
            let attention = Attention::of(seat.activity);
            let lamp_at = lamp_height(seat.walk.pos(), eye);
            if let Some(color) = attention.lamp() {
                lamp(&mut mesh, seat.walk.pos() + Vec3::Y * lamp_at, color);
                if attention == Attention::AwaitingInput {
                    let foot = seat.walk.pos() + Vec3::Y * (lamp_at + 0.15);
                    let top = seat.walk.pos() + Vec3::Y * BEACON;
                    for p in [foot, top] {
                        mesh.lines.push(Vertex {
                            pos: p.to_array(),
                            color,
                            fog: 1.0,
                        });
                    }
                }
            }
            // The "!" and the bubble stack over the lamp.
            let mut top = lamp_at + LAMP_GAP;
            if self.marked.as_deref() == Some(seat.name.as_str()) {
                if let Some(transform) = billboard(seat.walk.pos(), top, eye) {
                    mark(&mut mesh, transform, self.clock);
                }
                top += MARK_TALL + LAMP_GAP;
            }
            if let Some(bubble) = self.showing().find(|b| b.speech.speaker == seat.name)
                && let Some(transform) = billboard(seat.walk.pos(), top, eye)
            {
                mesh.faces.extend(bubble.mesh.faces.iter().map(|v| Vertex {
                    pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                    ..*v
                }));
            }
            if let Some(kind) = Particles::of(seat.activity) {
                particles(
                    &mut mesh,
                    kind,
                    seat.walk.pos(),
                    seat.walk.yaw(),
                    index,
                    self.clock,
                    eye,
                );
            }
            let Some(transform) = plate_transform(seat.walk.pos(), eye) else {
                continue;
            };
            mesh.faces.extend(seat.plate.faces.iter().map(|v| Vertex {
                pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                ..*v
            }));
        }
        mesh
    }
}

/// Glade space for billboarded lettering over a seat at `feet`, its bottom
/// `lift` over the feet, seen from `eye`, or `None` when it is too near the
/// eye to draw.
///
/// The billboard turns toward the eye and stands a little out of the seat
/// toward it. Nearer than `PLATE_TALL / PLATE_ANGLE` it shrinks, so a
/// nameplate never subtends more than about [`PLATE_ANGLE`] however near
/// the eye comes.
fn billboard(feet: Vec3, lift: f32, eye: Vec3) -> Option<Mat4> {
    let mut anchor = feet + Vec3::Y * lift;
    let level = Vec3::new(eye.x - anchor.x, 0.0, eye.z - anchor.z);
    if level.length() > 2.0 * PLATE_OUT {
        anchor += level.normalize() * PLATE_OUT;
    }
    let toward = eye - anchor;
    let distance = toward.length();
    if !distance.is_finite() || distance < PLATE_HIDE {
        return None;
    }
    let scale = (distance * PLATE_ANGLE / PLATE_TALL).min(1.0);
    let facing = toward.x.atan2(toward.z);
    Some(
        Mat4::from_translation(anchor)
            * Mat4::from_rotation_y(facing + std::f32::consts::PI)
            * Mat4::from_scale(Vec3::splat(scale)),
    )
}

/// One flat quad of `color` from its corners.
fn quad(mesh: &mut Mesh, corners: [Vec3; 4], color: [f32; 3]) {
    let [a, b, c, d] = corners;
    for p in [a, b, c, a, c, d] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.0,
        });
    }
}

/// A plate-space rectangle from `(x0, y0)` to `(x1, y1)` at depth `z`.
fn rect(mesh: &mut Mesh, [x0, y0]: [f32; 2], [x1, y1]: [f32; 2], z: f32, color: [f32; 3]) {
    quad(
        mesh,
        [
            Vec3::new(x1, y0, z),
            Vec3::new(x0, y0, z),
            Vec3::new(x0, y1, z),
            Vec3::new(x1, y1, z),
        ],
        color,
    );
}

/// Height of the "!" over a seat, m.
const MARK_TALL: f32 = 0.36;

/// The pulsing "!" through `transform`, a billboard whose bottom is the
/// mark's, at `clock` seconds.
fn mark(mesh: &mut Mesh, transform: Mat4, clock: f32) {
    let pulse = (clock * 6.0).sin();
    let scale = 1.0 + 0.18 * pulse;
    let color = [1.0, 0.8 + 0.12 * pulse, 0.2];
    let mut local = Mesh::default();
    let half = 0.045;
    rect(&mut local, [-half, 0.14], [half, MARK_TALL], 0.0, color);
    rect(&mut local, [-half, 0.0], [half, 2.0 * half], 0.0, color);
    let center = Vec3::Y * (MARK_TALL / 2.0);
    let pulse = Mat4::from_translation(center)
        * Mat4::from_scale(Vec3::splat(scale))
        * Mat4::from_translation(-center);
    let to_glade = transform * pulse;
    mesh.faces.extend(local.faces.into_iter().map(|v| Vertex {
        pos: to_glade.transform_point3(Vec3::from(v.pos)).to_array(),
        ..v
    }));
}

/// `text` in at most `rows` lines of at most `width` characters in the
/// in-world lettering's alphabet, broken between words. Text that does
/// not fit ends its last line with "...".
#[must_use]
pub fn wrap(text: &str, width: usize, rows: usize) -> Vec<String> {
    let clean = lettering(text, width * (rows + 1));
    let mut lines: Vec<String> = Vec::new();
    let mut line = String::new();
    let mut cut = clean.len() < lettering(text, usize::MAX).len();
    for word in clean.split_whitespace() {
        let word: String = word.chars().take(width).collect();
        if line.is_empty() {
            line = word;
        } else if line.len() + 1 + word.len() <= width {
            line.push(' ');
            line.push_str(&word);
        } else {
            lines.push(std::mem::replace(&mut line, word));
            if lines.len() == rows {
                cut = true;
                line.clear();
                break;
            }
        }
    }
    if !line.is_empty() {
        lines.push(line);
    }
    if lines.len() > rows {
        lines.truncate(rows);
        cut = true;
    }
    if cut && let Some(last) = lines.last_mut() {
        while last.len() + 3 > width && !last.is_empty() {
            last.pop();
        }
        last.push_str("...");
    }
    lines
}

/// A speech bubble's faces in plate space, bottom at the origin: who it is
/// to over the wrapped text, on a pale card with a tail toward the
/// speaker.
fn bubble(speech: &Speech) -> Mesh {
    bubble_sized(speech, SPEECH_CHARS, SPEECH_LINES)
}

/// [`bubble`] with `chars` a line and at most `rows` lines.
pub(super) fn bubble_sized(speech: &Speech, chars: usize, rows: usize) -> Mesh {
    const PAPER: [f32; 3] = [0.93, 0.9, 0.8];
    const EDGE: [f32; 3] = [0.3, 0.26, 0.2];
    const INK: [f32; 3] = [0.08, 0.08, 0.08];
    const TO: [f32; 3] = [0.5, 0.36, 0.12];
    const SIZE: f32 = 0.085;
    const HEAD: f32 = 0.065;
    const GAP: f32 = 0.035;
    const PAD: f32 = 0.06;
    const TAIL: f32 = 0.08;
    let to = match &speech.to {
        Addressee::Person => "TO YOU".to_owned(),
        Addressee::Seat(seat) => lettering(&format!("to {seat}"), chars),
    };
    let lines = wrap(&speech.text, chars, rows);
    let advance = |chars: usize, size: f32| chars as f32 * 6.0 / 7.0 * size;
    let wide = lines
        .iter()
        .map(|l| advance(l.len(), SIZE))
        .fold(advance(to.len(), HEAD), f32::max);
    let half = wide / 2.0 + PAD;
    let tall = 2.0 * PAD + HEAD + lines.len() as f32 * (SIZE + GAP);
    let mut mesh = Mesh::default();
    // The card behind the lettering, a frame behind the card, and the tail.
    let edge = 0.012;
    rect(&mut mesh, [-half, TAIL], [half, TAIL + tall], 0.01, PAPER);
    rect(
        &mut mesh,
        [-half - edge, TAIL - edge],
        [half + edge, TAIL + tall + edge],
        0.02,
        EDGE,
    );
    for p in [
        Vec3::new(0.06, TAIL, 0.01),
        Vec3::new(-0.06, TAIL, 0.01),
        Vec3::new(0.0, 0.0, 0.01),
    ] {
        mesh.faces.push(Vertex {
            pos: p.to_array(),
            color: PAPER,
            fog: 1.0,
        });
    }
    let mut y = TAIL + tall - PAD - HEAD;
    boards::letters(&mut mesh, &to, 0.0, y, 0.0, HEAD, TO);
    for line in &lines {
        y -= GAP + SIZE;
        boards::letters(&mut mesh, line, 0.0, y, 0.0, SIZE, INK);
    }
    mesh
}

/// `kind`'s particles around a seat at `feet` facing `yaw`, at `clock`,
/// each a small square turned toward `eye`.
fn particles(
    mesh: &mut Mesh,
    kind: Particles,
    feet: Vec3,
    yaw: f32,
    seed: usize,
    clock: f32,
    eye: Vec3,
) {
    let color = kind.color();
    for i in 0..PARTICLES {
        let (at, half) = kind.place(feet, yaw, clock, i, seed);
        let toward = Vec3::new(eye.x - at.x, 0.0, eye.z - at.z).normalize_or(Vec3::Z);
        let right = Vec3::Y.cross(toward).normalize_or(Vec3::X) * half;
        let up = Vec3::Y * half;
        quad(
            mesh,
            [
                at - right - up,
                at + right - up,
                at + right + up,
                at - right + up,
            ],
            color,
        );
    }
}

/// A small lit cube at `at`.
fn lamp(mesh: &mut Mesh, at: Vec3, color: [f32; 3]) {
    let h = 0.09;
    let corner = |i: usize| {
        at + Vec3::new(
            if i & 1 == 0 { -h } else { h },
            if i & 2 == 0 { -h } else { h },
            if i & 4 == 0 { -h } else { h },
        )
    };
    let c: [Vec3; 8] = std::array::from_fn(corner);
    for [a, b, cc, d] in [
        [0, 2, 3, 1],
        [4, 5, 7, 6],
        [0, 1, 5, 4],
        [2, 6, 7, 3],
        [0, 4, 6, 2],
        [1, 3, 7, 5],
    ] {
        for p in [c[a], c[b], c[cc], c[a], c[cc], c[d]] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color,
                fog: 1.0,
            });
        }
    }
}

/// A nameplate's faces in plate space: the name over the activity over the
/// route, the activity in the lamp's color.
pub(super) fn plate(text: &[String; 3], attention: Attention) -> Mesh {
    const NAME: [f32; 3] = [0.95, 0.92, 0.82];
    const ROUTE: [f32; 3] = [0.6, 0.6, 0.55];
    let mut mesh = Mesh::default();
    let activity = attention.lamp().unwrap_or([0.75, 0.75, 0.7]);
    let rows = [
        (&text[2], 0.0, 0.08, ROUTE),
        (&text[1], 0.12, 0.1, activity),
        (&text[0], 0.27, 0.15, NAME),
    ];
    for (line, y, size, color) in rows {
        let line = lettering(line, PLATE_CHARS);
        if !line.is_empty() {
            boards::letters(&mut mesh, &line, 0.0, y, 0.0, size, color);
        }
    }
    mesh
}
