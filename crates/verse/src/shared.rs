//! Shared bodies: one arrangement of the bare world's ball and blocks for
//! everyone in the world, over NIP-MV's shared-body profile.
//!
//! Every client simulates every body on its own [`physics::World`]. Each body
//! also carries an authority [`Stamp`]: an epoch (the reset generation), a
//! revision (one per motion episode), and the owner that stamped it. The
//! stamps order totally, epoch first, then revision, then owner key, so every
//! client resolves a conflict the same way without a server.
//!
//! - **Last toucher owns.** When this player's capsule touches a body, or a
//!   body this player owns strikes another, this client claims it with the
//!   next revision. A claim with a higher stamp from someone else takes the
//!   body back.
//! - **The owner streams.** The owner puts its awake bodies, with velocity,
//!   spin, and stamp, in its pose frames at a bounded rate, and the rest pose
//!   once when a body falls asleep. A body that wakes again under the same
//!   owner starts a new revision, so a rest pose names exactly one episode.
//! - **Others follow.** A client that does not own a body keeps simulating it
//!   and snaps it to each newer report, hiding the jump behind a correction
//!   that decays over [`SMOOTH`] seconds.
//! - **Rest poses persist.** Each client records every body's last rest pose
//!   and stamp in one addressable snapshot. A client that joins later reads
//!   every snapshot and keeps, per body, the highest stamp.
//! - **Reset.** A reset raises the epoch and returns every body home. Its
//!   snapshot outranks every older stamp, so it reaches players online now
//!   and players who join later alike.
//! - **Orphans.** A body still moving here whose owner has said nothing about
//!   it for [`ORPHAN`] seconds is adopted, so it still comes to rest on the
//!   record when its owner leaves mid-motion.
//!
//! This module is pure: no I/O and no clock. [`crate::session::Session`]
//! carries its entries and snapshots, and [`crate::ball::Ball`] steps it.

use glam::{DQuat, DVec3};
use physics::{BodyId, World};

use crate::mv::{BODIES_ROLE, BODY_ROLE, BodyRest, EntityPose, State};

/// The bare world's body set: the ball, the stack of cubes, and the
/// dominoes, at their compiled home poses.
pub const SET: &str = "verse-bare.bodies.v1";
/// An owner silent this long about a body that still moves here loses it to
/// the first client that adopts it, s.
pub const ORPHAN: f64 = 6.0;
/// After losing a body, a touch does not claim it back for this long, s, so
/// two players leaning on one body do not trade it every step.
pub const RECLAIM: f64 = 0.3;
/// Time constant of the correction that hides a snap to a newer report, s.
pub const SMOOTH: f64 = 0.15;
/// A correction longer than this is shown as a jump, m.
const SNAP: f64 = 3.0;
/// Speed above which an owned body claims what it strikes, m/s.
const STRIKE_SPEED: f64 = 0.05;
/// Fastest a reported body may move, m/s, or spin, rad/s.
pub const MAX_SPEED: f64 = 60.0;
/// Highest a reported body may be, m.
const MAX_HEIGHT: f64 = 60.0;

/// Whether a reported pose and motion could be real in the bare world:
/// inside its walls, above its floor, and no faster than [`MAX_SPEED`].
fn plausible(pos: DVec3, vel: DVec3, omega: DVec3) -> bool {
    let half = f64::from(crate::world::HALF) + 2.0;
    pos.is_finite()
        && pos.x.abs() <= half
        && pos.z.abs() <= half
        && (-2.0..=MAX_HEIGHT).contains(&pos.y)
        && vel.length() <= MAX_SPEED
        && omega.length() <= MAX_SPEED
}

/// A body's authority: its reset generation, its motion episode, and the
/// owner that stamped it. Orders totally, field by field.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Stamp {
    /// Reset generation.
    pub epoch: u64,
    /// Motion episode within the epoch.
    pub rev: u64,
    /// The owner's public key, hex; empty for a body nobody has moved.
    pub owner: String,
}

/// A position and orientation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Position, m.
    pub pos: DVec3,
    /// Orientation.
    pub orientation: DQuat,
}

/// One shared body.
#[derive(Clone, Debug)]
struct Replica {
    id: String,
    body: BodyId,
    home: Pose,
    stamp: Stamp,
    /// The last rest pose on record and the stamp of the episode it ended.
    rest: Option<(Pose, Stamp)>,
    /// When the owner last reported this body (clock, s).
    heard: f64,
    /// The owner's time of that report, ms, to order reports of one stamp.
    heard_t: u64,
    /// When this body last woke here while someone else owned it.
    woke: f64,
    /// When this client last put it in a frame.
    sent: f64,
    /// An owned body came to rest and its rest pose is not yet sent.
    rest_pending: bool,
    /// When this client last lost it to a higher stamp.
    lost: f64,
    /// Whether it slept after the last observed step.
    asleep: bool,
    /// Drawn minus simulated pose, decaying to nothing.
    offset: (DVec3, DQuat),
}

/// Every shared body's authority and record.
#[derive(Clone, Debug)]
pub struct Shared {
    me: Option<String>,
    bodies: Vec<Replica>,
    /// Seconds of simulated time.
    clock: f64,
    /// The highest epoch seen.
    epoch: u64,
    /// A rest pose changed since the last snapshot.
    dirty: bool,
    /// The pending snapshot records a reset, and should go out at once.
    urgent: bool,
}

impl Shared {
    /// Bodies `(id, body, home)` at home, owned by nobody.
    #[must_use]
    pub fn new(bodies: Vec<(String, BodyId, Pose)>) -> Self {
        Self {
            me: None,
            bodies: bodies
                .into_iter()
                .map(|(id, body, home)| Replica {
                    id,
                    body,
                    home,
                    stamp: Stamp::default(),
                    rest: None,
                    heard: f64::NEG_INFINITY,
                    heard_t: 0,
                    woke: f64::NEG_INFINITY,
                    sent: f64::NEG_INFINITY,
                    rest_pending: false,
                    lost: f64::NEG_INFINITY,
                    asleep: true,
                    offset: (DVec3::ZERO, DQuat::IDENTITY),
                })
                .collect(),
            clock: 0.0,
            epoch: 0,
            dirty: false,
            urgent: false,
        }
    }

    /// This client's public key, hex. Until it is known, nothing is claimed.
    /// A reset made before joining is stamped with it now.
    pub fn join(&mut self, me: &str) {
        if self.me.as_deref() == Some(me) {
            return;
        }
        self.me = Some(me.to_owned());
        if self.urgent {
            for replica in &mut self.bodies {
                if replica.stamp.owner.is_empty() && replica.stamp.epoch == self.epoch {
                    replica.stamp.owner = me.to_owned();
                    replica.rest = Some((replica.home, replica.stamp.clone()));
                }
            }
        }
    }

    /// The body ids, in order.
    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.bodies.iter().map(|r| r.id.as_str())
    }

    /// A body's stamp.
    #[must_use]
    pub fn stamp(&self, id: &str) -> Option<&Stamp> {
        self.index(id).map(|i| &self.bodies[i].stamp)
    }

    /// The highest epoch seen.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.epoch
    }

    /// Whether this client owns body `id`.
    #[must_use]
    pub fn owns(&self, id: &str) -> bool {
        self.index(id).is_some_and(|i| self.mine(i))
    }

    /// The drawn correction of `body`: add the offset to its simulated
    /// position and rotate its simulated orientation by the rotation.
    #[must_use]
    pub fn offset(&self, body: BodyId) -> (DVec3, DQuat) {
        self.bodies
            .iter()
            .find(|r| r.body == body)
            .map_or((DVec3::ZERO, DQuat::IDENTITY), |r| r.offset)
    }

    /// Whether any body sits away from home.
    #[must_use]
    pub fn displaced(&self, world: &World) -> bool {
        self.bodies.iter().any(|r| {
            let body = &world[r.body];
            !body.sleeping
                || body.pos.distance(r.home.pos) > 0.05
                || body.orientation.angle_between(r.home.orientation) > 0.05
        })
    }

    fn index(&self, id: &str) -> Option<usize> {
        self.bodies.iter().position(|r| r.id == id)
    }

    fn index_of(&self, body: BodyId) -> Option<usize> {
        self.bodies.iter().position(|r| r.body == body)
    }

    fn mine(&self, i: usize) -> bool {
        self.me
            .as_deref()
            .is_some_and(|me| self.bodies[i].stamp.owner == me)
    }

    /// Advances the clock by `dt` seconds and decays the corrections.
    pub fn advance(&mut self, dt: f64) {
        self.clock += dt;
        let keep = (-dt / SMOOTH).exp();
        for replica in &mut self.bodies {
            let (pos, rotation) = replica.offset;
            replica.offset = (
                pos * keep,
                DQuat::IDENTITY.slerp(rotation, keep).normalize(),
            );
        }
    }

    /// Reads one physics step: episodes of owned bodies begin and end, and
    /// the player (body `player`) and owned bodies claim what they touch.
    pub fn observe(&mut self, world: &World, player: BodyId) {
        let Some(me) = self.me.clone() else {
            for replica in &mut self.bodies {
                replica.asleep = world[replica.body].sleeping;
            }
            return;
        };
        for i in 0..self.bodies.len() {
            let mine = self.mine(i);
            let replica = &mut self.bodies[i];
            let body = &world[replica.body];
            match (replica.asleep, body.sleeping) {
                (true, false) if mine => {
                    // An owned body moves again: a new episode.
                    replica.stamp.rev += 1;
                    replica.rest_pending = false;
                }
                (true, false) => replica.woke = self.clock,
                (false, true) if mine => {
                    let pose = Pose {
                        pos: body.pos,
                        orientation: body.orientation,
                    };
                    replica.rest = Some((pose, replica.stamp.clone()));
                    replica.rest_pending = true;
                    self.dirty = true;
                }
                _ => {}
            }
            replica.asleep = body.sleeping;
        }
        let mut touched = Vec::new();
        for contact in &world.contacts {
            let (a, b) = (self.index_of(contact.body_a), self.index_of(contact.body_b));
            for (from, to, other) in [(a, b, contact.body_a), (b, a, contact.body_b)] {
                let Some(to) = to else { continue };
                let strikes = from.is_some_and(|from| {
                    let body = &world[self.bodies[from].body];
                    self.mine(from)
                        && !body.sleeping
                        && (body.vel.length() > STRIKE_SPEED || body.omega.length() > STRIKE_SPEED)
                });
                if other == player || strikes {
                    touched.push(to);
                }
            }
        }
        for i in touched {
            if !self.mine(i) && self.clock - self.bodies[i].lost >= RECLAIM {
                self.claim(i, &me, world);
            }
        }
        for i in 0..self.bodies.len() {
            let replica = &self.bodies[i];
            let silent = self.clock - replica.heard.max(replica.woke);
            if !self.mine(i) && !world[replica.body].sleeping && silent > ORPHAN {
                self.claim(i, &me, world);
            }
        }
    }

    fn claim(&mut self, i: usize, me: &str, world: &World) {
        let replica = &mut self.bodies[i];
        replica.stamp = Stamp {
            epoch: replica.stamp.epoch,
            rev: replica.stamp.rev + 1,
            owner: me.to_owned(),
        };
        replica.asleep = world[replica.body].sleeping;
        replica.rest_pending = false;
        replica.sent = f64::NEG_INFINITY;
    }

    /// Adopts a newer epoch: every body of an older one goes home.
    fn raise_epoch(&mut self, world: &mut World, epoch: u64) {
        if epoch <= self.epoch {
            return;
        }
        self.epoch = epoch;
        let stamp = Stamp {
            epoch,
            rev: 0,
            owner: String::new(),
        };
        for i in 0..self.bodies.len() {
            if self.bodies[i].stamp.epoch < epoch {
                if self.mine(i) {
                    self.bodies[i].lost = self.clock;
                }
                let home = self.bodies[i].home;
                self.settle(world, i, home, stamp.clone(), false);
            }
        }
    }

    /// Puts body `i` at rest at `pose` under `stamp`, recording the rest.
    fn settle(&mut self, world: &mut World, i: usize, pose: Pose, stamp: Stamp, smooth: bool) {
        self.place(world, i, pose, DVec3::ZERO, DVec3::ZERO, true, smooth);
        let replica = &mut self.bodies[i];
        replica.rest = Some((pose, stamp.clone()));
        replica.stamp = stamp;
        replica.rest_pending = false;
        replica.asleep = true;
    }

    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        world: &mut World,
        i: usize,
        pose: Pose,
        vel: DVec3,
        omega: DVec3,
        rest: bool,
        smooth: bool,
    ) {
        let replica = &mut self.bodies[i];
        let body = &mut world[replica.body];
        let drawn = body.pos + replica.offset.0;
        let drawn_rotation = replica.offset.1 * body.orientation;
        let offset = drawn - pose.pos;
        replica.offset = if smooth && offset.length() < SNAP && offset.is_finite() {
            (
                offset,
                (drawn_rotation * pose.orientation.inverse()).normalize(),
            )
        } else {
            (DVec3::ZERO, DQuat::IDENTITY)
        };
        body.pos = pose.pos;
        body.prev_pos = pose.pos;
        body.orientation = pose.orientation;
        body.prev_orientation = pose.orientation;
        if rest {
            body.vel = DVec3::ZERO;
            body.omega = DVec3::ZERO;
            body.sleeping = true;
            body.sleep_time = 0.0;
        } else {
            body.vel = vel;
            // `omega` is carried in the world frame; the body keeps its own.
            body.omega = pose.orientation.inverse() * omega;
            body.wake();
        }
    }

    /// Applies one body's report from `from`'s pose frame, sent at the
    /// publisher's time `t`, ms. Reports this client sent, of unknown
    /// bodies, or with an older stamp, change nothing.
    pub fn receive(&mut self, world: &mut World, from: &str, pose: &EntityPose, t: u64) {
        if self.me.as_deref() == Some(from) || pose.role != BODY_ROLE {
            return;
        }
        let (Some(i), Some([epoch, rev])) = (self.index(&pose.id), pose.k) else {
            return;
        };
        let vel = pose
            .v
            .map_or(DVec3::ZERO, |v| glam::Vec3::from(v).as_dvec3());
        let omega = pose
            .w
            .map_or(DVec3::ZERO, |w| glam::Vec3::from(w).as_dvec3());
        if !plausible(pose.pos().as_dvec3(), vel, omega) {
            return;
        }
        self.raise_epoch(world, epoch);
        let stamp = Stamp {
            epoch,
            rev,
            owner: from.to_owned(),
        };
        let replica = &self.bodies[i];
        if stamp < replica.stamp || (stamp == replica.stamp && t <= replica.heard_t) {
            return;
        }
        if self.mine(i) {
            self.bodies[i].lost = self.clock;
        }
        let target = Pose {
            pos: pose.pos().as_dvec3(),
            orientation: pose.rot().as_dquat().normalize(),
        };
        if pose.r {
            self.settle(world, i, target, stamp, true);
        } else {
            self.place(world, i, target, vel, omega, false, true);
            let replica = &mut self.bodies[i];
            replica.stamp = stamp;
            replica.rest_pending = false;
            replica.asleep = false;
        }
        let replica = &mut self.bodies[i];
        replica.heard = self.clock;
        replica.heard_t = t;
    }

    /// Applies `from`'s snapshot: each recorded rest pose whose stamp is
    /// newer than this client's, or that ends the episode this client last
    /// heard of. Snapshots of another body set change nothing.
    pub fn receive_snapshot(&mut self, world: &mut World, from: &str, state: &State) {
        if state.role != BODIES_ROLE || state.set.as_deref() != Some(SET) {
            return;
        }
        let Some(bodies) = &state.b else { return };
        let at = |rest: &BodyRest| glam::Vec3::from(rest.p).as_dvec3();
        if !bodies
            .iter()
            .all(|rest| plausible(at(rest), DVec3::ZERO, DVec3::ZERO))
        {
            return;
        }
        if let Some(epoch) = bodies.iter().map(|b| b.k[0]).max() {
            self.raise_epoch(world, epoch);
        }
        for rest in bodies {
            let Some(i) = self.index(&rest.id) else {
                continue;
            };
            let stamp = Stamp {
                epoch: rest.k[0],
                rev: rest.k[1],
                owner: rest.o.clone().unwrap_or_else(|| from.to_owned()),
            };
            let replica = &self.bodies[i];
            let ends_episode = stamp == replica.stamp
                && !self.mine(i)
                && replica.rest.as_ref().is_none_or(|(_, s)| *s != stamp);
            if stamp > replica.stamp || ends_episode {
                if self.mine(i) {
                    self.bodies[i].lost = self.clock;
                }
                let pose = Pose {
                    pos: glam::Vec3::from(rest.p).as_dvec3(),
                    orientation: glam::Quat::from_array(rest.q).normalize().as_dquat(),
                };
                self.settle(world, i, pose, stamp, true);
                self.bodies[i].heard = self.clock;
            }
        }
    }

    /// Whether this client has body reports to send: an owned body awake, or
    /// one that came to rest since its last report.
    #[must_use]
    pub fn has_outgoing(&self, world: &World) -> bool {
        (0..self.bodies.len()).any(|i| {
            self.mine(i) && (self.bodies[i].rest_pending || !world[self.bodies[i].body].sleeping)
        })
    }

    /// Up to `max` reports for the next pose frame, rest poses first, then
    /// the awake bodies reported longest ago.
    pub fn frame_entries(&mut self, world: &World, max: usize) -> Vec<EntityPose> {
        let mut due: Vec<usize> = (0..self.bodies.len())
            .filter(|&i| {
                self.mine(i)
                    && (self.bodies[i].rest_pending || !world[self.bodies[i].body].sleeping)
            })
            .collect();
        due.sort_by(|&a, &b| {
            let (a, b) = (&self.bodies[a], &self.bodies[b]);
            b.rest_pending
                .cmp(&a.rest_pending)
                .then(a.sent.total_cmp(&b.sent))
        });
        due.truncate(max);
        due.into_iter()
            .map(|i| {
                let replica = &mut self.bodies[i];
                let body = &world[replica.body];
                let rest = replica.rest_pending && body.sleeping;
                replica.rest_pending = false;
                replica.sent = self.clock;
                EntityPose {
                    v: (!rest).then(|| body.vel.as_vec3().to_array()),
                    w: (!rest).then(|| body.omega_world().as_vec3().to_array()),
                    k: Some([replica.stamp.epoch, replica.stamp.rev]),
                    r: rest,
                    ..EntityPose::new(
                        &replica.id,
                        BODY_ROLE,
                        body.pos.as_vec3(),
                        body.orientation.as_quat(),
                    )
                }
            })
            .collect()
    }

    /// Returns every body home at rest under a new epoch, owned by this
    /// client, and queues the snapshot that tells everyone.
    pub fn reset(&mut self, world: &mut World) {
        self.epoch += 1;
        let stamp = Stamp {
            epoch: self.epoch,
            rev: 0,
            owner: self.me.clone().unwrap_or_default(),
        };
        for i in 0..self.bodies.len() {
            let home = self.bodies[i].home;
            self.settle(world, i, home, stamp.clone(), false);
        }
        self.dirty = true;
        self.urgent = true;
    }

    /// Whether the pending snapshot records a reset.
    #[must_use]
    pub fn urgent(&self) -> bool {
        self.urgent && self.dirty
    }

    /// The snapshot to publish, when a rest pose changed since the last one:
    /// every body's last rest pose on record, at publisher time `t`, ms.
    #[must_use]
    pub fn snapshot(&self, t: u64) -> Option<State> {
        let me = self.me.as_deref()?;
        if !self.dirty {
            return None;
        }
        let b = self
            .bodies
            .iter()
            .filter_map(|replica| {
                let (pose, stamp) = replica.rest.as_ref()?;
                (!stamp.owner.is_empty()).then(|| BodyRest {
                    id: replica.id.clone(),
                    p: pose.pos.as_vec3().to_array(),
                    q: pose.orientation.as_quat().normalize().to_array(),
                    k: [stamp.epoch, stamp.rev],
                    o: (stamp.owner != me).then(|| stamp.owner.clone()),
                })
            })
            .collect();
        Some(State {
            v: 1,
            id: BODIES_ROLE.into(),
            role: BODIES_ROLE.into(),
            p: [0.0; 3],
            q: [0.0, 0.0, 0.0, 1.0],
            t,
            online: true,
            follows: None,
            name: None,
            set: Some(SET.into()),
            b: Some(b),
        })
    }

    /// Records that the snapshot went out.
    pub fn snapshot_sent(&mut self) {
        self.dirty = false;
        self.urgent = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ball::{Ball, RADIUS, START};
    use crate::controller::{InputState, PlayerController};
    use glam::Vec3;

    const FRAME: f32 = 1.0 / 60.0;
    const A: &str = "aa";
    const B: &str = "bb";

    /// One client: its world and its player, far from everything unless
    /// placed.
    struct Client {
        key: &'static str,
        ball: Ball,
        player: PlayerController,
        t: u64,
    }

    impl Client {
        fn new(key: &'static str) -> Self {
            let mut ball = Ball::new();
            ball.join(key);
            Self {
                key,
                ball,
                player: PlayerController::new(Vec3::new(-40.0, 0.0, -40.0), 0.0),
                t: 0,
            }
        }

        fn frames(&mut self, frames: usize, forward: bool) {
            let input = InputState {
                forward,
                ..InputState::default()
            };
            for _ in 0..frames {
                let from = self.player.pos;
                self.player.update(&input, FRAME, &[], crate::world::HALF);
                self.ball.advance(from, &mut self.player, FRAME);
                self.t += 16;
            }
        }

        /// Sends this client's due reports to `to`, as one pose frame.
        fn report(&mut self, to: &mut Client) -> usize {
            let entries = self.ball.frame_entries(15);
            for entry in &entries {
                to.ball.receive(self.key, entry, self.t);
            }
            entries.len()
        }

        fn snapshot(&mut self) -> Option<State> {
            let state = self.ball.snapshot(self.t)?;
            self.ball.snapshot_sent();
            Some(state)
        }

        fn ball_pos(&self) -> DVec3 {
            self.ball.body().pos
        }
    }

    /// Stands `client` behind the ball, facing it along +Z.
    fn behind_ball(client: &mut Client) {
        client.player =
            PlayerController::new(Vec3::new(START.x as f32, 0.0, START.z as f32 - 4.0), 0.0);
    }

    #[test]
    fn stamps_order_by_epoch_then_revision_then_owner() {
        let s = |epoch, rev, owner: &str| Stamp {
            epoch,
            rev,
            owner: owner.into(),
        };
        assert!(s(1, 0, "") > s(0, 99, "ff"));
        assert!(s(0, 2, "aa") > s(0, 1, "ff"));
        assert!(s(0, 1, "bb") > s(0, 1, "aa"));
        assert!(s(0, 0, "aa") > Stamp::default());
    }

    #[test]
    fn the_pusher_owns_the_ball_and_the_other_follows_it() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(90, true);
        assert!(a.ball.shared().owns("ball"));
        assert_eq!(a.ball.shared().stamp("ball").unwrap().rev, 1);
        assert!(a.ball_pos().z > START.z + 0.5);
        // B hears the moving ball and carries it on its own simulation.
        assert_eq!(a.report(&mut b), 1);
        assert!(b.ball_pos().distance(a.ball_pos()) < 1e-3);
        assert!(!b.ball.shared().owns("ball"));
        for _ in 0..20 {
            a.frames(45, false);
            b.frames(45, false);
            a.report(&mut b);
        }
        // Both came to rest at the same place: the owner's rest report wins.
        assert!(a.ball.body().sleeping && b.ball.body().sleeping);
        assert!(
            b.ball_pos().distance(a.ball_pos()) < 1e-3,
            "{:?}",
            b.ball_pos()
        );
        assert!(a.ball_pos().z > START.z + 3.0);
        // Once at rest, nothing more is reported.
        assert!(!a.ball.has_outgoing());
        assert_eq!(a.report(&mut b), 0);
    }

    #[test]
    fn the_last_toucher_takes_the_ball_and_the_old_owner_yields() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(60, true);
        a.report(&mut b);
        // B walks into the ball from the far side and claims it.
        let at = b.ball_pos();
        b.player = PlayerController::new(
            Vec3::new(at.x as f32, 0.0, at.z as f32 + 3.0),
            std::f32::consts::PI,
        );
        b.frames(40, true);
        assert!(b.ball.shared().owns("ball"));
        let taken = b.ball.shared().stamp("ball").unwrap().clone();
        assert!(taken > *a.ball.shared().stamp("ball").unwrap());
        b.report(&mut a);
        assert!(!a.ball.shared().owns("ball"));
        assert_eq!(a.ball.shared().stamp("ball"), Some(&taken));
        // A's older report no longer moves B's ball.
        let before = b.ball_pos();
        let stale = EntityPose {
            k: Some([0, 1]),
            v: Some([0.0, 0.0, 9.0]),
            ..EntityPose::new(
                "ball",
                BODY_ROLE,
                Vec3::new(0.0, 1.2, 30.0),
                glam::Quat::IDENTITY,
            )
        };
        b.ball.receive(A, &stale, a.t + 1);
        assert!(b.ball_pos().distance(before) < 1e-9);
    }

    #[test]
    fn simultaneous_claims_resolve_to_the_same_owner_everywhere() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        // Each walks into the ball in its own world before hearing the other.
        behind_ball(&mut a);
        b.player = PlayerController::new(
            Vec3::new(START.x as f32, 0.0, START.z as f32 + 4.0),
            std::f32::consts::PI,
        );
        a.frames(40, true);
        b.frames(40, true);
        assert!(a.ball.shared().owns("ball") && b.ball.shared().owns("ball"));
        assert_eq!(a.ball.shared().stamp("ball").unwrap().rev, 1);
        assert_eq!(b.ball.shared().stamp("ball").unwrap().rev, 1);
        a.report(&mut b);
        b.report(&mut a);
        // The higher key wins the tie in both worlds.
        for client in [&a, &b] {
            assert_eq!(client.ball.shared().stamp("ball").unwrap().owner, B);
        }
        assert!(!a.ball.shared().owns("ball"));
        assert!(a.ball_pos().distance(b.ball_pos()) < 1e-3);
        // A report from oneself changes nothing.
        let mine = b.ball.frame_entries(15);
        let before = b.ball.shared().stamp("ball").cloned();
        for entry in &mine {
            b.ball.receive(B, entry, u64::MAX);
        }
        assert_eq!(b.ball.shared().stamp("ball").cloned(), before);
    }

    #[test]
    fn a_late_joiner_finds_every_rest_pose_from_the_snapshots() {
        let (mut a, mut c) = (Client::new(A), Client::new("cc"));
        behind_ball(&mut a);
        a.frames(60, true);
        a.frames(60 * 20, false);
        assert!(a.ball.body().sleeping);
        let snapshot = a.snapshot().expect("a rest was recorded");
        assert_eq!(snapshot.set.as_deref(), Some(SET));
        let bodies = snapshot.b.as_ref().unwrap();
        assert_eq!(bodies.len(), 1, "only the ball moved");
        assert_eq!(bodies[0].o, None, "A owns what it recorded");
        assert!(a.snapshot().is_none(), "nothing new to record");
        // C joins after A left and applies what the relay kept.
        c.ball.receive_snapshot(A, &snapshot);
        assert!(c.ball_pos().distance(a.ball_pos()) < 1e-3);
        assert!(c.ball.body().sleeping);
        // A snapshot from someone who knew less changes nothing.
        let old = Client::new("dd").ball.snapshot(1);
        assert!(
            old.is_none(),
            "a client that saw nothing move records nothing"
        );
        let mut stale = snapshot.clone();
        stale.b.as_mut().unwrap()[0].k = [0, 0];
        stale.b.as_mut().unwrap()[0].p = [0.0, RADIUS as f32, 50.0];
        c.ball.receive_snapshot("dd", &stale);
        assert!(c.ball_pos().distance(a.ball_pos()) < 1e-3);
        // A snapshot of another body set is ignored.
        let mut other = snapshot;
        other.set = Some("elsewhere.v1".into());
        other.b.as_mut().unwrap()[0].k = [9, 9];
        c.ball.receive_snapshot(A, &other);
        assert_eq!(c.ball.shared().epoch(), 0);
    }

    #[test]
    fn a_reset_outranks_every_older_stamp_and_reaches_everyone() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(60, true);
        a.frames(60 * 20, false);
        let moved = a.snapshot().unwrap();
        b.ball.receive_snapshot(A, &moved);
        // B knocks over the stack and owns the cubes.
        let (stack, _) = b.ball.blocks().stack_pool();
        b.player = PlayerController::new(Vec3::new(stack.x, 0.0, stack.z - 4.0), 0.0);
        b.frames(150, true);
        assert!(b.ball.shared().owns("cube-15"));
        // A resets everything.
        a.ball.press_reset();
        assert!(a.ball.snapshot_urgent());
        let reset = a.snapshot().unwrap();
        assert_eq!(reset.b.as_ref().unwrap().len(), 27);
        b.ball.receive_snapshot(A, &reset);
        assert_eq!(b.ball.shared().epoch(), 1);
        assert!(!b.ball.shared().owns("cube-15"));
        assert!(!b.ball.shared().displaced(b.ball.world()));
        assert!((b.ball_pos() - START).length() < 1e-6);
        // A late joiner merging both snapshots keeps the reset.
        let mut c = Client::new("cc");
        c.ball.receive_snapshot(A, &reset);
        c.ball.receive_snapshot(A, &moved);
        assert!(!c.ball.shared().displaced(c.ball.world()));
        // A report from before the reset moves nothing.
        let old = EntityPose {
            k: Some([0, 7]),
            ..EntityPose::new(
                "ball",
                BODY_ROLE,
                Vec3::new(3.0, 1.2, 9.0),
                glam::Quat::IDENTITY,
            )
        };
        b.ball.receive(B, &old, 1);
        c.ball.receive("ee", &old, u64::MAX);
        assert!((c.ball_pos() - START).length() < 1e-6);
    }

    #[test]
    fn implausible_reports_change_nothing() {
        let mut b = Client::new(B);
        let report = |p: [f32; 3], v: [f32; 3]| EntityPose {
            k: Some([0, 5]),
            v: Some(v),
            ..EntityPose::new("ball", BODY_ROLE, Vec3::from(p), glam::Quat::IDENTITY)
        };
        for bad in [
            report([0.0, 1.2, 400.0], [0.0; 3]),
            report([0.0, -30.0, 0.0], [0.0; 3]),
            report([0.0, 1.2, 0.0], [0.0, 0.0, 500.0]),
        ] {
            b.ball.receive(A, &bad, 1);
            assert!((b.ball_pos() - START).length() < 1e-9);
        }
        assert_eq!(b.ball.shared().stamp("ball"), Some(&Stamp::default()));
    }

    #[test]
    fn a_report_from_a_newer_epoch_sends_every_older_body_home() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(60, true);
        a.frames(60 * 20, false);
        b.ball.receive_snapshot(A, &a.snapshot().unwrap());
        assert!(b.ball.shared().displaced(b.ball.world()));
        // A frame names epoch 1 before its snapshot arrives.
        let cube = EntityPose {
            k: Some([1, 1]),
            v: Some([0.0; 3]),
            ..EntityPose::new(
                "cube-0",
                BODY_ROLE,
                Vec3::new(-5.0, 0.4, 8.0),
                glam::Quat::IDENTITY,
            )
        };
        b.ball.receive("cc", &cube, 1);
        assert!((b.ball_pos() - START).length() < 1e-6);
        assert_eq!(b.ball.shared().epoch(), 1);
    }

    #[test]
    fn a_body_left_moving_by_a_silent_owner_is_adopted_and_recorded() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(45, true);
        a.report(&mut b);
        // A leaves mid-roll. B's copy keeps rolling and comes to rest.
        b.frames(60 * 20, false);
        assert!(b.ball.body().sleeping);
        assert!(b.ball.shared().owns("ball"), "B adopted the orphan");
        let snapshot = b.snapshot().expect("B records where it stopped");
        assert!(snapshot.b.unwrap().iter().any(|body| body.id == "ball"));
    }

    #[test]
    fn corrections_are_drawn_away_smoothly() {
        let (mut a, mut b) = (Client::new(A), Client::new(B));
        behind_ball(&mut a);
        a.frames(60, true);
        a.report(&mut b);
        b.frames(30, false);
        a.frames(30, false);
        // B's copy drifted slightly; a new report snaps it, and the drawn
        // ball moves there over a fraction of a second.
        let drawn = b.ball.pose().0;
        a.report(&mut b);
        let after = b.ball.pose().0;
        assert!(after.distance(drawn) < 0.05, "no visible jump");
        b.frames(30, false);
        let (shift, _) = b.ball.shared().offset(b.ball.ball_id());
        assert!(shift.length() < 1e-3);
    }

    #[test]
    fn pushing_the_ball_into_the_stack_claims_the_cubes_it_strikes() {
        let mut a = Client::new(A);
        let (stack, _) = a.ball.blocks().stack_pool();
        let id = a.ball.ball_id();
        let body = &mut a.ball.world_mut()[id];
        body.pos = DVec3::new(f64::from(stack.x), RADIUS, f64::from(stack.z) - 5.0);
        body.prev_pos = body.pos;
        let at = body.pos;
        // A pushed it: A owns the rolling ball.
        a.player = PlayerController::new(Vec3::new(at.x as f32, 0.0, at.z as f32 - 4.0), 0.0);
        a.frames(50, true);
        assert!(a.ball.shared().owns("ball"));
        a.frames(60 * 4, false);
        let owned = a
            .ball
            .shared()
            .ids()
            .filter(|id| id.starts_with("cube-"))
            .filter(|id| a.ball.shared().owns(id))
            .count();
        assert!(owned >= 4, "{owned} cubes claimed");
    }
}
