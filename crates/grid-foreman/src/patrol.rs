//! The Grid Foreman's patrol (#12445): four stops on the Grid plaza, the
//! legs between them planned around everything on the plaza, and the walker
//! that follows them, waiting for players in its way.
//!
//! The host runs one [`Walker`] and publishes its pose; every browser draws
//! what the host publishes, so every viewer sees one Foreman.

use glam::Vec3;

use crate::{BODY_RADIUS, FACES, HOME};

/// Ground speed while walking, m/s: the robot's stride at three times its
/// size would be 2.9 m/s, which reads too quick for a giant, so its walk
/// clip plays at [`crate::WALK_CLIP_RATE`].
pub const WALK_SPEED: f32 = 1.75;
/// How fast it turns, rad/s.
const TURN_SPEED: f32 = 1.6;
/// A player this close to the path ahead stops it, m.
const BLOCK_RANGE: f32 = 2.5;
/// How far ahead along its path it looks for players, m.
const LOOK_AHEAD: f32 = 3.0;
/// After this long waiting, it says so and looks for a way around, s.
const WAIT_SECONDS: f32 = 6.0;
/// A player this close turns its head at a stop, m.
const NOTICE_RANGE: f32 = 8.0;
/// Extra room it keeps from everything on the plaza, beyond its body, m.
const MARGIN: f32 = 0.6;

/// One stop of the patrol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// Beside the shared terminal: home.
    Terminal,
    /// On the Pylon Bay's west edge, looking in.
    Bay,
    /// In view of the build billboard.
    Billboard,
    /// Near the help desk, greeting arrivals from the spawn.
    HelpDesk,
}

/// The round: the terminal is home, and the Foreman goes back to it between
/// the other stops.
pub const ROUND: [Stop; 5] = [
    Stop::Terminal,
    Stop::Bay,
    Stop::Terminal,
    Stop::HelpDesk,
    Stop::Billboard,
];

impl Stop {
    /// Where it stands, world x and z, m.
    #[must_use]
    pub fn at(self) -> [f32; 2] {
        match self {
            Self::Terminal => [HOME[0] as f32, HOME[2] as f32],
            Self::Bay => [11.0, -6.0],
            // Off the billboard's east end, on the Pylon Bay's south side,
            // where it moved out of the spawn's view (#12482).
            Self::Billboard => [47.0, -5.0],
            Self::HelpDesk => [-11.0, -6.0],
        }
    }

    /// The point it faces while it stands there.
    #[must_use]
    pub fn faces(self) -> [f32; 2] {
        match self {
            Self::Terminal => [FACES[0] as f32, FACES[1] as f32],
            Self::Bay => [22.0, 6.0],
            // Just in front of the board's face, which looks into the Bay
            // (#12482), so it reads the face at a slant from its end.
            Self::Billboard => {
                let (site, faces) = (deploy_board::layout::SITE, deploy_board::layout::FACES);
                let (dx, dz) = (faces[0] - site[0], faces[1] - site[1]);
                let length = dx.hypot(dz).max(1e-6);
                [
                    (site[0] + dx / length * 4.0) as f32,
                    (site[1] + dz / length * 4.0) as f32,
                ]
            }
            Self::HelpDesk => [verse::world::SPAWN.x, verse::world::SPAWN.z],
        }
    }

    /// The status line while it walks here.
    #[must_use]
    pub fn walking(self) -> &'static str {
        match self {
            Self::Terminal => "WALKING TO THE TERMINAL",
            Self::Bay => "WALKING TO THE PYLON BAY",
            Self::Billboard => "WALKING TO THE BILLBOARD",
            Self::HelpDesk => "WALKING TO THE HELP DESK",
        }
    }

    /// The status line while it stands here, unless it has a better one.
    #[must_use]
    pub fn standing(self) -> &'static str {
        match self {
            Self::Terminal => "AT THE TERMINAL",
            Self::Bay => "WATCHING THE PYLONS",
            Self::Billboard => "READING THE BUILD BOARD",
            Self::HelpDesk => "SAYING HELLO",
        }
    }

    /// How long it stands here on round `lap`, s: 40 to 90, varied so the
    /// patrol doesn't look like a clock.
    #[must_use]
    pub fn dwell(self, lap: u64) -> f32 {
        let spread = (lap.wrapping_mul(37).wrapping_add(self as u64 * 13)) % 51;
        40.0 + spread as f32
    }
}

/// The status line while it waits for someone in its way.
pub const EXCUSE_ME: &str = "EXCUSE ME";

/// A point it keeps clear of, and how far its center must stay from it.
#[derive(Clone, Copy, Debug)]
pub struct KeepOut {
    pub what: &'static str,
    pub at: [f32; 2],
    pub radius: f32,
}

/// Everything on the plaza the Foreman's center keeps clear of, with its
/// body and a margin added: the arches and their approaches, the Gym's
/// walls, the pillar, the ball, the block stack, the dominoes, the
/// dancer, the patrolling robots' routes, the terminal, the help desk, the
/// Pylon Bay's pad, the billboard, and the spawn.
#[must_use]
pub fn keep_outs() -> Vec<KeepOut> {
    let body = BODY_RADIUS + MARGIN;
    let layout = verse::blocks::Layout::grid();
    let mut out = Vec::new();
    let mut add = |what: &'static str, at: Vec3, radius: f32| {
        out.push(KeepOut {
            what,
            at: [at.x, at.z],
            radius: radius + body,
        });
    };
    for gate in [
        verse::zones::Gate::grid(&layout),
        verse::zones::Gate::everglade(&layout),
        verse::zones::Gate::lagrange2(&layout),
        verse::zones::Gate::hosted(&layout),
        verse::zones::Gate::ritual(&layout),
    ] {
        add("an arch", gate.at, 2.4);
        add("an arch's approach", gate.front().0, 1.5);
    }
    add("the Game Lab arch", verse::zones::game_place::POSITION, 2.4);
    add("the pillar", verse::pillar::AT.as_vec3(), 1.0);
    add("the ball", verse::ball::START.as_vec3(), 1.5);
    add(
        "the stack",
        layout
            .at(verse::blocks::STACK_AT[0], verse::blocks::STACK_AT[1], 0.0)
            .as_vec3(),
        1.5,
    );
    let [side, ahead] = verse::blocks::DOMINOES_AT;
    for i in 0..=16 {
        let theta = std::f64::consts::FRAC_PI_2 * f64::from(i) / 16.0;
        let at = layout
            .at(
                side + 4.0 - 4.0 * theta.cos(),
                ahead + 4.0 * theta.sin(),
                0.0,
            )
            .as_vec3();
        add("a domino", at, 0.5);
    }
    add(
        "the dancer",
        verse::grid_robot::dancer_spot(),
        verse::grid_robot::BODY_RADIUS,
    );
    for [a, b] in verse::grid_robot::ROUTES {
        let (a, b) = (Vec3::new(a[0], 0.0, a[1]), Vec3::new(b[0], 0.0, b[1]));
        let steps = (a.distance(b).ceil() as usize).max(1);
        for i in 0..=steps {
            let at = a.lerp(b, i as f32 / steps as f32);
            add("a patrol", at, verse::grid_robot::BODY_RADIUS + 0.5);
        }
    }
    let terminal = pylons_view::terminal::POSITION;
    add(
        "the terminal",
        Vec3::new(terminal[0] as f32, 0.0, terminal[2] as f32),
        1.2,
    );
    add("the help desk", Vec3::new(-6.0, 0.0, -5.0), 2.2);
    // Small enough to leave a lane between the spawn and the pillar, so the
    // walk from the terminal to the Bay and the billboard doesn't circle
    // the plaza; it waits for anyone in its way.
    add("the spawn", verse::world::SPAWN, 1.5);
    // The billboard: its face, as points along its width.
    let site = deploy_board::layout::SITE;
    let faces = deploy_board::layout::FACES;
    let toward = Vec3::new(
        (faces[0] - site[0]) as f32,
        0.0,
        (faces[1] - site[1]) as f32,
    )
    .normalize_or_zero();
    let across = Vec3::new(toward.z, 0.0, -toward.x);
    let half = deploy_board::layout::WIDTH as f32 / 2.0;
    let center = Vec3::new(site[0] as f32, 0.0, site[1] as f32);
    for i in -6..=6 {
        add(
            "the billboard",
            center + across * (half * i as f32 / 6.0),
            1.5,
        );
    }
    out
}

/// Boxes it keeps clear of: the Bay's pad and the Gym's walls, grown by
/// its body and margin. `[min x, min z, max x, max z]`.
fn boxes() -> Vec<[f32; 4]> {
    let grow = BODY_RADIUS + MARGIN;
    let mut out = Vec::new();
    let [u0, u1, v0, v1] = pylons_view::bay::PAD;
    let (a, b) = (
        pylons_view::bay::world(u0, v0),
        pylons_view::bay::world(u1, v1),
    );
    out.push([
        a[0].min(b[0]) as f32 - grow,
        a[2].min(b[2]) as f32 - grow,
        a[0].max(b[0]) as f32 + grow,
        a[2].max(b[2]) as f32 + grow,
    ]);
    for wall in verse::world::GymSite::GRID.walls() {
        out.push([
            wall.min[0] - grow,
            wall.min[1] - grow,
            wall.max[0] + grow,
            wall.max[1] + grow,
        ]);
    }
    out
}

/// What a plan avoids: discs (center, radius) and boxes.
struct Obstacles {
    discs: Vec<([f32; 2], f32)>,
    boxes: Vec<[f32; 4]>,
    players: Vec<[f32; 2]>,
}

impl Obstacles {
    fn new(players: &[[f32; 2]]) -> Self {
        Self {
            discs: keep_outs().iter().map(|k| (k.at, k.radius)).collect(),
            boxes: boxes(),
            players: players.to_vec(),
        }
    }

    fn point_clear(&self, p: [f32; 2]) -> bool {
        p.iter()
            .all(|v| v.is_finite() && v.abs() < verse::world::HALF - 4.0)
            && self
                .discs
                .iter()
                .all(|(c, r)| (p[0] - c[0]).hypot(p[1] - c[1]) >= *r - 1e-3)
            && self
                .boxes
                .iter()
                .all(|b| !(p[0] > b[0] && p[0] < b[2] && p[1] > b[1] && p[1] < b[3]))
            && self
                .players
                .iter()
                .all(|at| (p[0] - at[0]).hypot(p[1] - at[1]) >= BLOCK_RANGE - 1e-3)
    }

    fn segment_clear(&self, a: [f32; 2], b: [f32; 2]) -> bool {
        self.discs
            .iter()
            .all(|(c, r)| to_segment(*c, a, b) >= *r - 1e-3)
            && self.boxes.iter().all(|bx| !segment_hits_box(a, b, *bx))
            && self
                .players
                .iter()
                .all(|p| to_segment(*p, a, b) >= player_clearance(a, *p) - 1e-3)
    }
}

/// Whether the segment from `a` to `b` passes through the inside of `bx`.
fn segment_hits_box(a: [f32; 2], b: [f32; 2], bx: [f32; 4]) -> bool {
    let (mut enter, mut leave) = (0.0_f32, 1.0_f32);
    for axis in 0..2 {
        let (lo, hi) = (bx[axis], bx[axis + 2]);
        let delta = b[axis] - a[axis];
        if delta.abs() < 1e-6 {
            if a[axis] <= lo || a[axis] >= hi {
                return false;
            }
        } else {
            let t0 = (lo - a[axis]) / delta;
            let t1 = (hi - a[axis]) / delta;
            enter = enter.max(t0.min(t1));
            leave = leave.min(t0.max(t1));
            if enter >= leave {
                return false;
            }
        }
    }
    true
}

/// The waypoints from `from` to `to`, around everything on the plaza and
/// around `players`, or `None` when there's no way: the shortest path over
/// points just outside each obstacle.
#[must_use]
pub fn plan(from: [f32; 2], to: [f32; 2], players: &[[f32; 2]]) -> Option<Vec<[f32; 2]>> {
    let obstacles = Obstacles::new(players);
    if !obstacles.point_clear(to) {
        return None;
    }
    if obstacles.segment_clear(from, to) {
        return Some(vec![to]);
    }
    let mut nodes = vec![from, to];
    const AROUND: usize = 10;
    for (c, r) in obstacles
        .discs
        .iter()
        .copied()
        .chain(obstacles.players.iter().map(|at| (*at, BLOCK_RANGE)))
    {
        let r = r + 0.35;
        for i in 0..AROUND {
            let a = std::f32::consts::TAU * i as f32 / AROUND as f32;
            let p = [c[0] + r * a.cos(), c[1] + r * a.sin()];
            if obstacles.point_clear(p) {
                nodes.push(p);
            }
        }
    }
    // A 2 m lattice around the two ends, for the gaps between obstacles.
    let (lo, hi) = (
        [from[0].min(to[0]) - 12.0, from[1].min(to[1]) - 12.0],
        [from[0].max(to[0]) + 12.0, from[1].max(to[1]) + 12.0],
    );
    let mut x = lo[0];
    while x <= hi[0] {
        let mut z = lo[1];
        while z <= hi[1] {
            if obstacles.point_clear([x, z]) {
                nodes.push([x, z]);
            }
            z += 2.0;
        }
        x += 2.0;
    }
    for b in &obstacles.boxes {
        for p in [
            [b[0] - 0.35, b[1] - 0.35],
            [b[2] + 0.35, b[1] - 0.35],
            [b[0] - 0.35, b[3] + 0.35],
            [b[2] + 0.35, b[3] + 0.35],
        ] {
            if obstacles.point_clear(p) {
                nodes.push(p);
            }
        }
    }
    // Dijkstra from `from` (node 0) to `to` (node 1), testing edges as
    // they're reached.
    let n = nodes.len();
    let mut best = vec![f32::INFINITY; n];
    let mut back = vec![usize::MAX; n];
    let mut done = vec![false; n];
    best[0] = 0.0;
    loop {
        let mut current = usize::MAX;
        let mut lowest = f32::INFINITY;
        for i in 0..n {
            if !done[i] && best[i] < lowest {
                lowest = best[i];
                current = i;
            }
        }
        if current == usize::MAX {
            return None;
        }
        if current == 1 {
            break;
        }
        done[current] = true;
        let here = nodes[current];
        for next in 0..n {
            if done[next] {
                continue;
            }
            let there = nodes[next];
            let cost = lowest + (there[0] - here[0]).hypot(there[1] - here[1]);
            if cost < best[next] && obstacles.segment_clear(here, there) {
                best[next] = cost;
                back[next] = current;
            }
        }
    }
    let mut path = Vec::new();
    let mut at = 1;
    while at != 0 {
        path.push(nodes[at]);
        at = back[at];
    }
    path.reverse();
    // Pull the string: from each point, go straight to the farthest later
    // point it can see.
    let mut smooth = Vec::new();
    let mut here = from;
    let mut i = 0;
    while i < path.len() {
        let mut far = i;
        for j in (i..path.len()).rev() {
            if obstacles.segment_clear(here, path[j]) {
                far = j;
                break;
            }
        }
        smooth.push(path[far]);
        here = path[far];
        i = far + 1;
    }
    Some(smooth)
}

/// Its legs around the round, each from one stop to the next.
#[must_use]
pub fn legs() -> Vec<(Stop, Stop, Vec<[f32; 2]>)> {
    (0..ROUND.len())
        .filter_map(|i| {
            let (from, to) = (ROUND[i], ROUND[(i + 1) % ROUND.len()]);
            plan(from.at(), to.at(), &[]).map(|path| (from, to, path))
        })
        .collect()
}

/// Where the Foreman is and what it's doing this moment.
#[derive(Clone, Debug, PartialEq)]
pub struct Moment {
    pub pos: Vec3,
    pub yaw: f32,
    /// Ground speed, m/s.
    pub speed: f32,
    /// Its status line, unless the host has a better one at a stop.
    pub status: &'static str,
    /// The stop it stands at, when it stands at one.
    pub at: Option<Stop>,
}

#[derive(Clone, Debug)]
enum Phase {
    Standing {
        stop: Stop,
        left: f32,
    },
    Walking {
        to: Stop,
        path: Vec<[f32; 2]>,
        next: usize,
        waited: f32,
    },
}

/// The patrol's walker: the host steps it a few times a second with the
/// players it can see.
#[derive(Clone, Debug)]
pub struct Walker {
    pos: Vec3,
    yaw: f32,
    speed: f32,
    phase: Phase,
    /// The index into [`ROUND`] of the stop it is at or walking to.
    index: usize,
    lap: u64,
    excuse: bool,
}

impl Default for Walker {
    fn default() -> Self {
        Self::new()
    }
}

impl Walker {
    /// A walker standing at home, facing the terminal.
    #[must_use]
    pub fn new() -> Self {
        let [x, z] = Stop::Terminal.at();
        Self {
            pos: Vec3::new(x, 0.0, z),
            yaw: crate::home_yaw() as f32,
            speed: 0.0,
            phase: Phase::Standing {
                stop: Stop::Terminal,
                left: 30.0,
            },
            index: 0,
            lap: 0,
            excuse: false,
        }
    }

    /// Now.
    #[must_use]
    pub fn moment(&self) -> Moment {
        let (status, at) = match &self.phase {
            Phase::Standing { stop, .. } => (stop.standing(), Some(*stop)),
            Phase::Walking { .. } if self.excuse => (EXCUSE_ME, None),
            Phase::Walking { to, .. } => (to.walking(), None),
        };
        Moment {
            pos: self.pos,
            yaw: self.yaw,
            speed: self.speed,
            status,
            at,
        }
    }

    /// Stays at its stop until `hold` is false; the host holds it at the
    /// terminal while it has work there.
    pub fn hold_at_terminal(&mut self, hold: bool) {
        if hold
            && let Phase::Standing { stop, left } = &mut self.phase
            && *stop == Stop::Terminal
        {
            *left = left.max(1.0);
        }
    }

    /// Calls it home to the terminal to work (#12447): from a stop, it sets
    /// off at once; on a leg to another stop, it turns for the terminal
    /// from where it is; at the terminal, it stays at least `hold` more
    /// seconds.
    pub fn summon(&mut self, hold: f32) {
        match &mut self.phase {
            Phase::Standing { stop, left } if *stop == Stop::Terminal => {
                *left = left.max(hold);
            }
            Phase::Walking { to, .. } if *to == Stop::Terminal => {}
            _ => {
                // The next terminal in the round, so the patrol carries on
                // from there afterwards.
                let next = (1..=ROUND.len())
                    .map(|i| (self.index + i) % ROUND.len())
                    .find(|i| ROUND[*i] == Stop::Terminal)
                    .unwrap_or(0);
                if next <= self.index {
                    self.lap += 1;
                }
                self.index = next;
                let from = [self.pos.x, self.pos.z];
                let path = plan(from, Stop::Terminal.at(), &[])
                    .unwrap_or_else(|| vec![Stop::Terminal.at()]);
                self.excuse = false;
                self.phase = Phase::Walking {
                    to: Stop::Terminal,
                    path,
                    next: 0,
                    waited: 0.0,
                };
            }
        }
    }

    /// Steps `dt` seconds with `players` (feet) on the plaza.
    pub fn step(&mut self, dt: f32, players: &[Vec3]) {
        let dt = dt.clamp(0.0, 1.0);
        self.speed = 0.0;
        match &mut self.phase {
            Phase::Standing { stop, left } => {
                let stop = *stop;
                *left -= dt;
                let done = *left <= 0.0;
                let nearest = players
                    .iter()
                    .filter(|p| flat_distance(**p, self.pos) <= NOTICE_RANGE)
                    .min_by(|a, b| {
                        flat_distance(**a, self.pos).total_cmp(&flat_distance(**b, self.pos))
                    });
                let face = nearest.map_or(stop.faces(), |p| [p.x, p.z]);
                self.turn_toward(face, dt);
                if done {
                    self.index = (self.index + 1) % ROUND.len();
                    if self.index == 0 {
                        self.lap += 1;
                    }
                    let to = ROUND[self.index];
                    let from = [self.pos.x, self.pos.z];
                    let path = plan(from, to.at(), &[]).unwrap_or_else(|| vec![to.at()]);
                    self.phase = Phase::Walking {
                        to,
                        path,
                        next: 0,
                        waited: 0.0,
                    };
                }
            }
            Phase::Walking { .. } => self.walk(dt, players),
        }
    }

    fn walk(&mut self, dt: f32, players: &[Vec3]) {
        let Phase::Walking {
            to,
            path,
            next,
            waited,
        } = &mut self.phase
        else {
            return;
        };
        let to = *to;
        let Some(target) = path.get(*next).copied() else {
            self.arrive(to);
            return;
        };
        let here = [self.pos.x, self.pos.z];
        let blocked = players.iter().any(|p| {
            let at = [p.x, p.z];
            ahead_distance(here, path, *next, LOOK_AHEAD, at) < player_clearance(here, at) - 1e-3
        });
        if blocked {
            *waited += dt;
            if *waited >= WAIT_SECONDS {
                self.excuse = true;
                let around: Vec<[f32; 2]> = players.iter().map(|p| [p.x, p.z]).collect();
                if let Some(detour) = plan(here, to.at(), &around) {
                    *path = detour;
                    *next = 0;
                    *waited = 0.0;
                }
            }
            if let Some(p) = players.iter().min_by(|a, b| {
                flat_distance(**a, self.pos).total_cmp(&flat_distance(**b, self.pos))
            }) {
                self.turn_toward([p.x, p.z], dt);
            }
            return;
        }
        *waited = 0.0;
        self.excuse = false;
        let to_target = Vec3::new(target[0] - self.pos.x, 0.0, target[1] - self.pos.z);
        let distance = to_target.length();
        let step = WALK_SPEED * dt;
        if distance <= step.max(1e-3) {
            self.pos = Vec3::new(target[0], 0.0, target[1]);
            *next += 1;
            if *next >= path.len() {
                self.arrive(to);
            }
            self.speed = WALK_SPEED;
            return;
        }
        self.pos += to_target / distance * step;
        self.speed = WALK_SPEED;
        self.turn_toward(target, dt);
    }

    fn arrive(&mut self, stop: Stop) {
        let [x, z] = stop.at();
        self.pos = Vec3::new(x, 0.0, z);
        self.excuse = false;
        self.phase = Phase::Standing {
            stop,
            left: stop.dwell(self.lap),
        };
    }

    fn turn_toward(&mut self, at: [f32; 2], dt: f32) {
        let (dx, dz) = (at[0] - self.pos.x, at[1] - self.pos.z);
        if dx.hypot(dz) < 1e-3 {
            return;
        }
        let want = dx.atan2(dz);
        let delta = verse::controller::wrap(want - self.yaw);
        let most = TURN_SPEED * dt;
        self.yaw = verse::controller::wrap(self.yaw + delta.clamp(-most, most));
    }
}

fn flat_distance(a: Vec3, b: Vec3) -> f32 {
    (a.x - b.x).hypot(a.z - b.z)
}

/// Keep the same room in plans and movement checks. If a player has
/// already stepped closer, allow movement away without coming any closer.
fn player_clearance(from: [f32; 2], player: [f32; 2]) -> f32 {
    BLOCK_RANGE.min((from[0] - player[0]).hypot(from[1] - player[1]))
}

fn to_segment(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> f32 {
    let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
    let length = dx * dx + dz * dz;
    let t = if length < 1e-6 {
        0.0
    } else {
        (((p[0] - a[0]) * dx + (p[1] - a[1]) * dz) / length).clamp(0.0, 1.0)
    };
    (p[0] - a[0] - dx * t).hypot(p[1] - a[1] - dz * t)
}

/// How near `p` comes to the next `reach` meters of the path from `here`
/// through `path[next..]`.
fn ahead_distance(here: [f32; 2], path: &[[f32; 2]], next: usize, reach: f32, p: [f32; 2]) -> f32 {
    let mut left = reach;
    let mut from = here;
    let mut best = f32::INFINITY;
    for &point in path.iter().skip(next) {
        let length = (point[0] - from[0]).hypot(point[1] - from[1]);
        let to = if length > left && length > 1e-6 {
            let t = left / length;
            [
                from[0] + (point[0] - from[0]) * t,
                from[1] + (point[1] - from[1]) * t,
            ]
        } else {
            point
        };
        best = best.min(to_segment(p, from, to));
        left -= length;
        if left <= 0.0 {
            break;
        }
        from = point;
    }
    best.min((p[0] - here[0]).hypot(p[1] - here[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clear_of_everything(p: [f32; 2]) -> Result<(), String> {
        for k in keep_outs() {
            let d = (p[0] - k.at[0]).hypot(p[1] - k.at[1]);
            if d < k.radius - MARGIN {
                return Err(format!("{p:?} is {d:.2} m from {} at {:?}", k.what, k.at));
            }
        }
        Ok(())
    }

    #[test]
    fn every_stop_stands_clear() {
        for stop in ROUND {
            clear_of_everything(stop.at()).unwrap();
        }
    }

    #[test]
    fn every_leg_has_a_path_that_stays_clear() {
        let legs = legs();
        assert_eq!(legs.len(), ROUND.len(), "a leg has no path");
        for (from, to, path) in legs {
            let mut at = from.at();
            for &point in &path {
                let steps = ((point[0] - at[0]).hypot(point[1] - at[1]) / 0.25).ceil() as usize;
                for i in 0..=steps.max(1) {
                    let t = i as f32 / steps.max(1) as f32;
                    let p = [
                        at[0] + (point[0] - at[0]) * t,
                        at[1] + (point[1] - at[1]) * t,
                    ];
                    clear_of_everything(p)
                        .map_err(|e| format!("{from:?} to {to:?}: {e}"))
                        .unwrap();
                }
                at = point;
            }
            assert_eq!(path.last().copied(), Some(to.at()));
        }
    }

    #[test]
    fn the_round_comes_home_between_stops() {
        assert_eq!(ROUND[0], Stop::Terminal);
        assert_eq!(ROUND[2], Stop::Terminal);
        for stop in ROUND {
            let dwell = stop.dwell(7);
            assert!((40.0..=90.0).contains(&dwell));
        }
    }

    #[test]
    fn the_walker_walks_the_round_alone() {
        let mut walker = Walker::new();
        let mut seen = Vec::new();
        for _ in 0..(3600 * 4) {
            walker.step(0.25, &[]);
            if let Some(stop) = walker.moment().at
                && seen.last() != Some(&stop)
            {
                seen.push(stop);
            }
        }
        assert!(seen.len() > ROUND.len(), "{seen:?}");
        for (i, stop) in seen.iter().enumerate() {
            assert_eq!(*stop, ROUND[i % ROUND.len()]);
        }
    }

    #[test]
    fn it_waits_for_a_player_in_its_way_then_goes_around() {
        for distance in [2.0, 4.0] {
            walks_around_player(distance);
        }
    }

    fn walks_around_player(distance: f32) {
        let mut walker = Walker::new();
        // Leave home.
        while walker.moment().at.is_some() {
            walker.step(0.25, &[]);
        }
        walker.step(0.25, &[]);
        let Phase::Walking { path, next, .. } = &walker.phase else {
            panic!("not walking");
        };
        let target = path[*next];
        let here = walker.moment().pos;
        let toward = (Vec3::new(target[0], 0.0, target[1]) - here).normalize();
        let player = here + toward * distance;
        let before = walker.moment().pos;
        for _ in 0..8 {
            walker.step(0.25, &[player]);
        }
        assert_eq!(walker.moment().pos, before, "it walked into someone");
        assert_eq!(walker.moment().speed, 0.0);
        for _ in 0..24 {
            walker.step(0.25, &[player]);
        }
        let status = walker.moment().status;
        assert!(status == EXCUSE_ME || walker.moment().pos != before);
        // A detour must reach the next stop, not only avoid a collision.
        let mut arrived = false;
        for _ in 0..400 {
            walker.step(0.25, &[player]);
            let d = flat_distance(walker.moment().pos, player);
            assert!(d > BODY_RADIUS, "{d}");
            if walker.moment().at == Some(Stop::Bay) {
                arrived = true;
                break;
            }
        }
        assert!(arrived, "it stayed blocked: {:?}", walker.moment());
    }

    #[test]
    fn it_can_walk_away_from_a_player_already_close_behind() {
        let mut walker = Walker::new();
        while walker.moment().at.is_some() {
            walker.step(0.25, &[]);
        }
        let Phase::Walking { path, next, .. } = &walker.phase else {
            panic!("not walking");
        };
        let target = path[*next];
        let before = walker.moment().pos;
        let toward = (Vec3::new(target[0], 0.0, target[1]) - before).normalize();
        let player = before - toward * 2.0;
        walker.step(0.25, &[player]);
        assert!(flat_distance(walker.moment().pos, player) > 2.0);
        assert!(walker.moment().speed > 0.0);
    }

    #[test]
    fn a_summons_brings_it_home_and_holds_it_there() {
        let mut walker = Walker::new();
        // Walk until it stands at the Pylon Bay.
        for _ in 0..4000 {
            walker.step(0.25, &[]);
            if walker.moment().at == Some(Stop::Bay) {
                break;
            }
        }
        assert_eq!(walker.moment().at, Some(Stop::Bay));
        walker.summon(30.0);
        let mut home = false;
        for _ in 0..4000 {
            walker.step(0.25, &[]);
            walker.summon(30.0);
            if walker.moment().at == Some(Stop::Terminal) {
                home = true;
                break;
            }
        }
        assert!(home, "it never came home");
        // Held while summoned.
        for _ in 0..1000 {
            walker.summon(30.0);
            walker.step(0.25, &[]);
        }
        assert_eq!(walker.moment().at, Some(Stop::Terminal));
        // Released, it carries on its round.
        let mut left = false;
        for _ in 0..2000 {
            walker.step(0.25, &[]);
            if walker.moment().at.is_none() {
                left = true;
                break;
            }
        }
        assert!(left);
    }

    #[test]
    fn it_faces_a_player_who_walks_up_at_a_stop() {
        let mut walker = Walker::new();
        let home = walker.moment().pos;
        let player = home + Vec3::new(-4.0, 0.0, -4.0);
        for _ in 0..20 {
            walker.step(0.25, &[player]);
        }
        let yaw = walker.moment().yaw;
        let want = (player.x - home.x).atan2(player.z - home.z);
        assert!(verse::controller::wrap(want - yaw).abs() < 0.05);
    }

    #[test]
    fn the_words_are_plain() {
        let mut lines = vec![EXCUSE_ME];
        for stop in ROUND {
            lines.push(stop.walking());
            lines.push(stop.standing());
        }
        for line in lines {
            assert!(line.len() <= crate::STATUS_MAX, "{line}");
            assert_eq!(crate::clean_line(line), line);
            assert!(oa_copy::violations(line, &[]).is_empty(), "{line}");
        }
    }

    #[test]
    fn legs_stay_short_enough_to_walk() {
        // No leg circles the plaza: each is at most twice its straight line
        // plus a detour's allowance.
        for (from, to, path) in legs() {
            let mut at = from.at();
            let mut walked = 0.0;
            for p in &path {
                walked += (p[0] - at[0]).hypot(p[1] - at[1]);
                at = *p;
            }
            let straight = (to.at()[0] - from.at()[0]).hypot(to.at()[1] - from.at()[1]);
            assert!(
                walked <= straight * 2.0 + 20.0,
                "{from:?} to {to:?}: {walked:.0} m for {straight:.0} m"
            );
        }
    }
}
