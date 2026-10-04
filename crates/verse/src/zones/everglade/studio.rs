//! The Agent Studio in Everglade (`docs/verse/agent-studio.md`, "The studio
//! in Verse"; `docs/verse/everglade.md`, "The workspace").
//!
//! The host's studio coordinator is the source of truth, and this module is
//! a view of it. A [`Source`] hands the view NIP-HOST studio snapshots
//! (`coder_access::studio`); the view never sends state back. From each
//! snapshot it draws:
//!
//! - Every seat as a walking figure at the station its activity names. The
//!   host already classified the seat's newest ATIF step with
//!   `atif::classify`, so the snapshot's station is the place; this module
//!   only turns it into a standing point. When a seat's station changes
//!   while it is still walking to the last one, or the walk would take
//!   longer than [`MAX_WALK`], it skips ahead to the latest station:
//!   movement presents the activity stream and never lags it.
//! - A nameplate over each seat with its name, activity, and route, and a
//!   lamp whose color is the seat's [`Attention`]. A seat waiting on a
//!   person also raises a beacon, so it reads from across the glade.
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

use super::{HALF_EXTENT, STATIONS, Station as Place, boards, height, layout::DESKS};
use crate::{
    avatar::{self, Gait},
    controller::Footprint,
    mesh::{Mesh, Vertex},
};
use coder_access::review::TaskReview;
use coder_access::studio::{self as wire, Activity, Snapshot, View};
use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};

#[cfg(feature = "model-host")]
pub mod fixture;
#[cfg(test)]
mod tests;

/// How fast a seat walks between stations, m/s.
pub const WALK_SPEED: f32 = 2.4;
/// The longest walk a seat takes, s. A station farther than this skips
/// ahead, so a seat never shows an activity long after it changed.
pub const MAX_WALK: f32 = 12.0;
/// How far from a desk's standing point the player stands to use that
/// seat's desk, m.
pub const DESK_RANGE: f32 = 1.4;
/// Space between seats sharing one station, m.
const SLOT: f32 = 0.9;
/// Within this distance of a waypoint, a seat takes the next, m.
const ARRIVED: f32 = 0.05;
/// Height of a seat's lamp over its feet, m.
const LAMP: f32 = 2.2;
/// Height of the bottom of a seat's nameplate over its feet, m.
const PLATE: f32 = 2.45;
/// Height of a waiting seat's beacon over its feet, m.
const BEACON: f32 = 7.0;
/// The most characters a nameplate line shows.
const PLATE_CHARS: usize = 24;

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
    /// when it holds one.
    fn review(&mut self, task: &str) -> Option<TaskReview>;
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
            _ => None,
        }
    }
}

/// The layout ID of the place a snapshot station is drawn at. A seat's
/// desk is its own desk, not the desks station.
#[must_use]
pub fn place_id(station: wire::Station) -> &'static str {
    match station {
        wire::Station::Desk => "desks",
        wire::Station::Library => "library",
        wire::Station::Workbench => "workbench",
        wire::Station::ProvingGround => "proving",
        wire::Station::Oracle => "oracle",
        wire::Station::Podium => "podium",
        wire::Station::Lounge => "lounge",
        wire::Station::TaskWall => "task_wall",
    }
}

fn place(id: &str) -> &'static Place {
    STATIONS
        .iter()
        .find(|station| station.id == id)
        .unwrap_or(&STATIONS[2])
}

/// Where a seat stands for `station`: its own desk's standing point for
/// the desk, else the station's point moved sideways to `slot` of `count`
/// seats there. Returns the point on the ground and the heading to face.
#[must_use]
pub fn standing(station: wire::Station, desk: u32, slot: usize, count: usize) -> ([f32; 2], f32) {
    if station == wire::Station::Desk
        && let Some(desk) = usize::try_from(desk).ok().and_then(|i| DESKS.get(i))
    {
        return (desk.seat, 0.0);
    }
    let place = place(place_id(station));
    // Sideways to the heading: forward is (sin, cos), so right is
    // (cos, -sin).
    let shift = (slot as f32 - (count.max(1) - 1) as f32 / 2.0) * SLOT;
    let (sin, cos) = place.facing.sin_cos();
    (
        [place.at[0] + cos * shift, place.at[1] - sin * shift],
        place.facing,
    )
}

/// The text a nameplate shows: the name, the activity, and the route.
#[must_use]
pub fn nameplate(seat: &wire::Seat) -> [String; 3] {
    let name = match seat.role {
        wire::Role::Lead => format!("{} lead", seat.seat),
        wire::Role::Worker => seat.seat.clone(),
    };
    [name, word(seat.activity).into(), seat.route.clone()]
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

/// A seat's figure color: the lead in gold, workers in turn.
fn tint(seat: &wire::Seat, index: usize) -> [f32; 3] {
    const WORKERS: [[f32; 3]; 5] = [
        [0.45, 0.75, 1.0],
        [0.95, 0.5, 0.8],
        [0.55, 0.95, 0.6],
        [0.8, 0.6, 1.0],
        [1.0, 0.55, 0.4],
    ];
    match seat.role {
        wire::Role::Lead => [1.0, 0.82, 0.35],
        wire::Role::Worker => WORKERS[index % WORKERS.len()],
    }
}

/// One seat as the glade draws it.
struct SeatAgent {
    name: String,
    pos: Vec3,
    yaw: f32,
    /// Where it is going and the heading it takes there.
    target: [f32; 2],
    facing: f32,
    /// Waypoints still to walk, the last being `target`.
    route: Vec<[f32; 2]>,
    gait: Gait,
    activity: Activity,
    tint: [f32; 3],
    plate_text: [String; 3],
    /// The nameplate's faces in plate space: its face in the XY plane,
    /// facing -Z, its bottom at the origin.
    plate: Mesh,
}

impl SeatAgent {
    fn walking(&self) -> bool {
        !self.route.is_empty()
    }

    /// Stand at `target` at once, facing `facing`.
    fn skip_to(&mut self, target: [f32; 2], facing: f32) {
        self.target = target;
        self.facing = facing;
        self.route.clear();
        self.pos = Vec3::new(target[0], height(target[0], target[1]), target[1]);
        self.yaw = facing;
    }
}

/// The studio as Everglade draws it.
pub struct Studio {
    source: Option<Box<dyn Source>>,
    active: bool,
    snapshot: Option<Snapshot>,
    seats: Vec<SeatAgent>,
    /// The Task Wall's cards and the monitors' text, rebuilt only when the
    /// snapshot changes.
    boards: Mesh,
    /// Counts changes to what the studio shows, so a panel refreshes only
    /// when it changed.
    revision: u64,
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
        if !active {
            self.snapshot = None;
            self.seats.clear();
            self.boards = boards::live(None);
            self.revision += 1;
        }
    }

    /// Polls the source, when active, and takes a changed studio. Seats
    /// route around `blockers`.
    pub fn poll(&mut self, dt: f32, blockers: &[Footprint]) {
        if !self.active {
            return;
        }
        let Some(snapshot) = self.source.as_mut().and_then(|source| source.poll(dt)) else {
            return;
        };
        self.apply(snapshot, blockers);
    }

    /// Takes `snapshot` as the studio now: seats walk to their stations,
    /// and the boards redraw. A seat seen for the first time stands at its
    /// station at once.
    pub fn apply(&mut self, snapshot: Snapshot, blockers: &[Footprint]) {
        if self.snapshot.as_ref() == Some(&snapshot) {
            return;
        }
        let view = &snapshot.view;
        let mut seats = Vec::with_capacity(view.seats.len());
        for (index, seat) in view.seats.iter().enumerate() {
            let (slot, count) = sharing(view, index);
            let (target, facing) = standing(seat.station, seat.desk, slot, count);
            let plate_text = nameplate(seat);
            let existing = self
                .seats
                .iter()
                .position(|agent| agent.name == seat.seat)
                .map(|at| self.seats.swap_remove(at));
            let mut agent = match existing {
                Some(mut agent) => {
                    if agent.target != target {
                        retarget(&mut agent, target, facing, blockers);
                    } else if !agent.walking() {
                        agent.facing = facing;
                        agent.yaw = facing;
                    }
                    agent
                }
                None => {
                    let mut agent = SeatAgent {
                        name: seat.seat.clone(),
                        pos: Vec3::ZERO,
                        yaw: facing,
                        target,
                        facing,
                        route: Vec::new(),
                        gait: Gait::default(),
                        activity: seat.activity,
                        tint: tint(seat, index),
                        plate_text: Default::default(),
                        plate: Mesh::default(),
                    };
                    agent.skip_to(target, facing);
                    agent
                }
            };
            agent.activity = seat.activity;
            agent.tint = tint(seat, index);
            if agent.plate_text != plate_text {
                agent.plate = plate(&plate_text, Attention::of(seat.activity));
                agent.plate_text = plate_text;
            }
            seats.push(agent);
        }
        self.seats = seats;
        self.boards = boards::live(Some(view));
        self.snapshot = Some(snapshot);
        self.revision += 1;
    }

    /// Walks every seat for `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        for seat in &mut self.seats {
            let mut left = WALK_SPEED * dt;
            let mut moved = 0.0;
            while left > 0.0
                && let Some(&next) = seat.route.first()
            {
                let to = Vec3::new(next[0], 0.0, next[1]) - Vec3::new(seat.pos.x, 0.0, seat.pos.z);
                let distance = to.length();
                if distance <= left.max(ARRIVED) {
                    seat.pos.x = next[0];
                    seat.pos.z = next[1];
                    seat.route.remove(0);
                    left -= distance;
                    moved += distance;
                } else {
                    let step = to / distance * left;
                    seat.pos.x += step.x;
                    seat.pos.z += step.z;
                    seat.yaw = step.x.atan2(step.z);
                    moved += left;
                    left = 0.0;
                }
            }
            seat.pos.y = height(seat.pos.x, seat.pos.z);
            if seat.route.is_empty() {
                seat.yaw = seat.facing;
            }
            seat.gait.advance(moved / dt, false, dt);
        }
    }

    /// The studio last taken, while active.
    #[must_use]
    pub fn snapshot(&self) -> Option<&Snapshot> {
        self.snapshot.as_ref()
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
            .map(|seat| seat.pos)
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
        self.seats.iter().map(|seat| (seat.name.as_str(), seat.pos))
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

    /// The seats, their lamps and nameplates turned toward `eye`, and the
    /// live boards.
    #[must_use]
    pub fn mesh(&self, eye: Vec3) -> Mesh {
        let mut mesh = Mesh::default();
        mesh.extend(&self.boards);
        let full = crate::palette::amber(Intensity::Full);
        for seat in &self.seats {
            let mut figure = avatar::figure(
                seat.pos,
                Quat::from_rotation_y(seat.yaw),
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
            let attention = Attention::of(seat.activity);
            if let Some(color) = attention.lamp() {
                lamp(&mut mesh, seat.pos + Vec3::Y * LAMP, color);
                if attention == Attention::AwaitingInput {
                    let foot = seat.pos + Vec3::Y * (LAMP + 0.15);
                    let top = seat.pos + Vec3::Y * BEACON;
                    for p in [foot, top] {
                        mesh.lines.push(Vertex {
                            pos: p.to_array(),
                            color,
                            fog: 1.0,
                        });
                    }
                }
            }
            let anchor = seat.pos + Vec3::Y * PLATE;
            let toward = eye - anchor;
            let facing = toward.x.atan2(toward.z);
            let transform = Mat4::from_translation(anchor)
                * Mat4::from_rotation_y(facing + std::f32::consts::PI);
            mesh.faces.extend(seat.plate.faces.iter().map(|v| Vertex {
                pos: transform.transform_point3(Vec3::from(v.pos)).to_array(),
                ..*v
            }));
        }
        mesh
    }
}

/// Which of the seats at the same place as seat `index` it is, and how many
/// stand there. Each desk holds only its own seat.
fn sharing(view: &View, index: usize) -> (usize, usize) {
    let seat = &view.seats[index];
    let at_own_desk =
        |s: &wire::Seat| s.station == wire::Station::Desk && (s.desk as usize) < DESKS.len();
    if at_own_desk(seat) {
        return (0, 1);
    }
    let same = |s: &wire::Seat| !at_own_desk(s) && place_id(s.station) == place_id(seat.station);
    let slot = view.seats[..index].iter().filter(|s| same(s)).count();
    let count = view.seats.iter().filter(|s| same(s)).count();
    (slot, count)
}

/// Sends `seat` toward `target`: along a route around `blockers`, or at
/// once when it is still walking to an earlier station or the walk would
/// take longer than [`MAX_WALK`].
fn retarget(seat: &mut SeatAgent, target: [f32; 2], facing: f32, blockers: &[Footprint]) {
    if seat.walking() {
        seat.skip_to(target, facing);
        return;
    }
    let start = [seat.pos.x, seat.pos.z];
    let route = crate::nav::plan(start, target, blockers, HALF_EXTENT)
        .map(|route| route.waypoints)
        .unwrap_or_else(|_| vec![target]);
    let mut length = 0.0;
    let mut from = start;
    for point in &route {
        length += (point[0] - from[0]).hypot(point[1] - from[1]);
        from = *point;
    }
    if length / WALK_SPEED > MAX_WALK {
        seat.skip_to(target, facing);
        return;
    }
    seat.target = target;
    seat.facing = facing;
    seat.route = route;
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
fn plate(text: &[String; 3], attention: Attention) -> Mesh {
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
