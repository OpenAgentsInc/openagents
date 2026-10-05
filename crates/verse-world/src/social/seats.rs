//! How the Agent Studio's seats walk Everglade: each seat goes to the
//! standing point its activity names, around the zone's blockers, walking a
//! short way and running a long one, and skips ahead when it is still
//! walking to an earlier station or the walk would take longer than
//! [`MAX_WALK`]. A seat waiting at the podium walks over to a player who
//! comes near and goes back once they leave.
//!
//! A lone viewer walks its own seats with [`Walker`]. In a hosted instance
//! the world authority owns the seats as [`Seats`] and every viewer draws
//! the [`SeatPose`]s it publishes, so all of them see the same seat at the
//! same place.

use super::controller::Footprint;
use super::everglade::{HALF_EXTENT, height};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// How fast a seat walks between stations, m/s.
pub const WALK_SPEED: f32 = 2.4;
/// How fast a seat runs a long route, m/s.
pub const RUN_SPEED: f32 = 5.5;
/// A route longer than this is run rather than walked, m.
pub const RUN_DISTANCE: f32 = 10.0;
/// A waiting seat walks over to a player this near its station, m.
pub const APPROACH: f32 = 6.0;
/// A following seat goes back once the player is this far from its station, m.
pub const DEPART: f32 = 9.0;
/// How far from the player a following seat stands, m.
pub const APPROACH_GAP: f32 = 1.4;
/// How far the player moves before a following seat walks again, m.
const FOLLOW_AGAIN: f32 = 1.5;
/// The longest walk a seat takes, s. A station farther than this skips
/// the seat there at once.
pub const MAX_WALK: f32 = 12.0;
/// How near a waypoint counts as reached, m.
const ARRIVED: f32 = 0.05;

/// One seat's position and walk.
#[derive(Clone, Debug, PartialEq)]
pub struct Walker {
    pos: Vec3,
    yaw: f32,
    /// Where it is going and the heading it takes there.
    target: [f32; 2],
    facing: f32,
    /// Its station's standing point and heading, which a waiting seat
    /// leaves to meet the player.
    home: [f32; 2],
    home_facing: f32,
    /// Waypoints still to walk, the last being `target`.
    route: Vec<[f32; 2]>,
    /// The speed of the walk under way, m/s.
    pace: f32,
    /// How fast it moved over the last advance, m/s.
    speed: f32,
    /// A waiting seat walking to, or standing with, the player.
    following: bool,
    /// Whether the world authority's last pose had the seat walking.
    posed_walking: bool,
}

impl Walker {
    /// A seat seen for the first time: standing at `home` at once, facing
    /// `facing`.
    #[must_use]
    pub fn standing(home: [f32; 2], facing: f32) -> Self {
        let mut walker = Self {
            pos: Vec3::ZERO,
            yaw: facing,
            target: home,
            facing,
            home,
            home_facing: facing,
            route: Vec::new(),
            pace: WALK_SPEED,
            speed: 0.0,
            following: false,
            posed_walking: false,
        };
        walker.skip_to(home, facing);
        walker
    }

    /// Its feet.
    #[must_use]
    pub fn pos(&self) -> Vec3 {
        self.pos
    }

    /// Its heading, as the controller's yaw.
    #[must_use]
    pub fn yaw(&self) -> f32 {
        self.yaw
    }

    /// How fast it moved over the last advance, m/s.
    #[must_use]
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Whether it is still walking.
    #[must_use]
    pub fn walking(&self) -> bool {
        !self.route.is_empty() || self.posed_walking
    }

    /// Stands the seat where the world authority placed it: a viewer in a
    /// hosted instance draws the authority's seats instead of walking its
    /// own.
    pub fn take_pose(&mut self, pose: &SeatPose) {
        let pos = Vec3::from_array(pose.pos);
        if !pos.is_finite() || !pose.yaw.is_finite() || !pose.speed.is_finite() {
            return;
        }
        self.route.clear();
        self.pos = pos;
        self.yaw = pose.yaw;
        self.speed = pose.speed.max(0.0);
        self.posed_walking = pose.walking;
        self.following = false;
    }

    /// Whether it is walking to, or standing with, the player.
    #[must_use]
    pub fn following(&self) -> bool {
        self.following
    }

    /// Stand at `target` at once, facing `facing`.
    pub fn skip_to(&mut self, target: [f32; 2], facing: f32) {
        self.target = target;
        self.facing = facing;
        self.route.clear();
        self.pos = Vec3::new(target[0], height(target[0], target[1]), target[1]);
        self.yaw = facing;
    }

    /// Takes a station's standing point `home` and heading `facing` from a
    /// new snapshot: a changed one sends the seat there around `blockers`,
    /// and an unchanged one turns a standing seat to its heading.
    pub fn plan(&mut self, home: [f32; 2], facing: f32, blockers: &[Footprint]) {
        if self.home != home {
            self.home = home;
            self.home_facing = facing;
            self.following = false;
            self.retarget(home, facing, blockers);
        } else if !self.walking() && !self.following {
            self.facing = facing;
            self.yaw = facing;
        }
    }

    /// Sends the seat toward `target`: along a route around `blockers`, or
    /// at once when it is still walking to an earlier station or the walk
    /// would take longer than [`MAX_WALK`].
    fn retarget(&mut self, target: [f32; 2], facing: f32, blockers: &[Footprint]) {
        if self.walking() {
            self.skip_to(target, facing);
            return;
        }
        let start = [self.pos.x, self.pos.z];
        let route = super::nav::plan(start, target, blockers, HALF_EXTENT)
            .map(|route| route.waypoints)
            .unwrap_or_else(|_| vec![target]);
        let length = route_length(start, &route);
        if length / WALK_SPEED > MAX_WALK {
            self.skip_to(target, facing);
            return;
        }
        self.target = target;
        self.facing = facing;
        self.route = route;
        self.pace = pace(length);
    }

    /// Sends the seat from where it stands toward `target` around
    /// `blockers`, even mid-walk. Leaves it as it was, and returns false,
    /// when no route reaches the target or the walk would take longer than
    /// [`MAX_WALK`].
    fn walk_to(&mut self, target: [f32; 2], facing: f32, blockers: &[Footprint]) -> bool {
        let start = [self.pos.x, self.pos.z];
        let Ok(route) = super::nav::plan(start, target, blockers, HALF_EXTENT) else {
            return false;
        };
        let length = route_length(start, &route.waypoints);
        if length / WALK_SPEED > MAX_WALK {
            return false;
        }
        self.target = target;
        self.facing = facing;
        self.route = route.waypoints;
        self.pace = pace(length);
        true
    }

    /// A seat that `waits` at the podium walks over to a `player` within
    /// [`APPROACH`] of its station, stands a little from them facing them,
    /// follows when they move, and goes back once they are farther away.
    pub fn follow(&mut self, waits: bool, player: Option<Vec3>, blockers: &[Footprint]) {
        let reach = if self.following { DEPART } else { APPROACH };
        let near = player.filter(|p| (p.x - self.home[0]).hypot(p.z - self.home[1]) <= reach);
        match near {
            Some(p) if waits => {
                let player = Vec2::new(p.x, p.z);
                let away = (Vec2::new(self.pos.x, self.pos.z) - player)
                    .try_normalize()
                    .or_else(|| (Vec2::from(self.home) - player).try_normalize())
                    .unwrap_or(Vec2::Y);
                let spot = player + away * APPROACH_GAP;
                let facing = (p.x - spot.x).atan2(p.z - spot.y);
                let moved = (spot - Vec2::from(self.target)).length() > FOLLOW_AGAIN;
                if (!self.following || moved) && self.walk_to(spot.to_array(), facing, blockers) {
                    self.following = true;
                } else if self.following && !self.walking() {
                    // Keep facing the player while standing with them.
                    self.facing = (p.x - self.pos.x).atan2(p.z - self.pos.z);
                }
            }
            _ if self.following => {
                self.following = false;
                let (home, facing) = (self.home, self.home_facing);
                if !self.walk_to(home, facing, blockers) {
                    self.skip_to(home, facing);
                }
            }
            _ => {}
        }
    }

    /// Walks the seat `dt` seconds along its route, on the ground.
    pub fn advance(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.posed_walking = false;
        let mut left = self.pace * dt;
        let mut moved = 0.0;
        while left > 0.0
            && let Some(&next) = self.route.first()
        {
            let to = Vec3::new(next[0], 0.0, next[1]) - Vec3::new(self.pos.x, 0.0, self.pos.z);
            let distance = to.length();
            if distance <= left.max(ARRIVED) {
                self.pos.x = next[0];
                self.pos.z = next[1];
                self.route.remove(0);
                left -= distance;
                moved += distance;
            } else {
                let step = to / distance * left;
                self.pos.x += step.x;
                self.pos.z += step.z;
                self.yaw = step.x.atan2(step.z);
                moved += left;
                left = 0.0;
            }
        }
        self.pos.y = height(self.pos.x, self.pos.z);
        if self.route.is_empty() {
            self.yaw = self.facing;
        }
        self.speed = moved / dt;
    }
}

/// How fast a seat covers a route `length` m long: a run when long.
fn pace(length: f32) -> f32 {
    if length > RUN_DISTANCE {
        RUN_SPEED
    } else {
        WALK_SPEED
    }
}

/// The length of a walk from `start` along `route`, m.
fn route_length(start: [f32; 2], route: &[[f32; 2]]) -> f32 {
    let mut length = 0.0;
    let mut from = start;
    for point in route {
        length += (point[0] - from[0]).hypot(point[1] - from[1]);
        from = *point;
    }
    length
}

/// Where one seat belongs now, from a studio snapshot.
#[derive(Clone, Debug, PartialEq)]
pub struct SeatPlan {
    /// The seat's name, its key.
    pub name: String,
    /// Its station's standing point, x and z, m.
    pub home: [f32; 2],
    /// The heading it takes there, as the controller's yaw.
    pub facing: f32,
    /// Whether it waits at the podium for a person, and so walks over to
    /// a player who comes near.
    pub waits: bool,
}

/// One seat as the authority places it, for every viewer to draw.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatPose {
    /// The seat's name, its key.
    pub name: String,
    /// Its feet, m.
    pub pos: [f32; 3],
    /// Its heading, as the controller's yaw.
    pub yaw: f32,
    /// How fast it moves now, m/s.
    pub speed: f32,
    /// Whether it is still walking.
    pub walking: bool,
}

/// Every seat of a hosted instance, owned by its world authority.
#[derive(Clone, Debug, Default)]
pub struct Seats {
    walkers: Vec<(SeatPlan, Walker)>,
    blockers: Vec<Footprint>,
}

impl Seats {
    /// No seats yet; they route around `blockers`.
    #[must_use]
    pub fn new(blockers: Vec<Footprint>) -> Self {
        Self {
            walkers: Vec::new(),
            blockers,
        }
    }

    /// Takes `plans` as the studio now, in the snapshot's order: a seat seen
    /// before walks to its new home, a new seat stands at its home at once,
    /// and a seat the snapshot no longer lists leaves.
    pub fn apply(&mut self, plans: &[SeatPlan]) {
        let mut walkers = Vec::with_capacity(plans.len());
        for plan in plans {
            let walker = match self.walkers.iter().position(|(p, _)| p.name == plan.name) {
                Some(at) => {
                    let (_, mut walker) = self.walkers.swap_remove(at);
                    walker.plan(plan.home, plan.facing, &self.blockers);
                    walker
                }
                None => Walker::standing(plan.home, plan.facing),
            };
            walkers.push((plan.clone(), walker));
        }
        self.walkers = walkers;
    }

    /// Walks every seat `dt` seconds. A waiting seat meets the nearest of
    /// `players`, the avatars' feet.
    pub fn tick(&mut self, dt: f32, players: &[Vec3]) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        for (plan, walker) in &mut self.walkers {
            let home = Vec2::from(plan.home);
            let nearest = players
                .iter()
                .copied()
                .filter(|p| p.is_finite())
                .min_by(|a, b| {
                    let da = (Vec2::new(a.x, a.z) - home).length();
                    let db = (Vec2::new(b.x, b.z) - home).length();
                    da.total_cmp(&db)
                });
            walker.follow(plan.waits, nearest, &self.blockers);
            walker.advance(dt);
        }
    }

    /// Where every seat stands now, in the snapshot's order.
    #[must_use]
    pub fn poses(&self) -> Vec<SeatPose> {
        self.walkers
            .iter()
            .map(|(plan, walker)| SeatPose {
                name: plan.name.clone(),
                pos: walker.pos().to_array(),
                yaw: walker.yaw(),
                speed: walker.speed(),
                walking: walker.walking(),
            })
            .collect()
    }
}
