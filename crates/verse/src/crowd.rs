//! Other players: their avatars and agents, as NIP-MV reports them.
//!
//! Frames arrive a few times a second with network jitter. Each entity
//! keeps a short buffer of timed poses and is drawn [`DELAY`] in the past,
//! interpolated with `lerp` for position and `slerp` for orientation, so
//! motion looks continuous. When the next frame is late, the entity keeps
//! its last velocity for up to one frame interval, then holds still. An entity with no recent frame falls back to
//! its durable state and is drawn dim: offline, where its owner left it.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use coder_ui::theme::Intensity;
use glam::{Mat4, Quat, Vec3};

use crate::agent;
use crate::avatar::{self, Gait};
use crate::mesh::Mesh;
use crate::mv::{EntityPose, Received};

/// How far in the past remote entities are drawn: a frame and a half at the
/// shared 5 Hz cadence, so one late frame does not stop the motion.
pub const DELAY: Duration = Duration::from_millis(300);
/// With no frame for this long, an entity counts as offline.
pub const STALE: Duration = Duration::from_secs(10);
/// Most entities tracked per publisher.
const PER_PUBLISHER: usize = 8;
/// Most entities tracked in all.
const MAX_TRACKED: usize = 512;
/// Most buffered samples per entity.
const SAMPLES: usize = 32;
const MAX_SESSIONS: usize = 16;
/// Fastest motion continued past the newest sample; anything quicker is a
/// teleport or a new session, which holds still instead.
const MAX_EXTRAPOLATED_SPEED: f32 = 15.0;

#[derive(Debug)]
struct SessionOrder {
    active: String,
    sequence: u64,
    retired: HashSet<String>,
    latest_time: u64,
}

#[derive(Clone, Copy, Debug)]
struct Sample {
    at: Instant,
    pos: Vec3,
    rot: Quat,
}

/// One remote entity.
#[derive(Debug)]
struct Remote {
    role: String,
    samples: VecDeque<Sample>,
    last_frame: Option<Instant>,
    /// Durable position, orientation, and whether it said online.
    state: Option<(Vec3, Quat, bool)>,
    gait: Gait,
    last_drawn: Option<Vec3>,
}

/// What the crowd shows for one entity this frame.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// Publisher pubkey.
    pub pubkey: String,
    /// Entity id.
    pub id: String,
    /// Entity role.
    pub role: String,
    /// Interpolated position.
    pub pos: Vec3,
    /// Interpolated orientation.
    pub rot: Quat,
    /// Whether frames are arriving.
    pub online: bool,
}

/// Every remote entity in one world.
#[derive(Debug)]
pub struct Crowd {
    me: String,
    entities: HashMap<(String, String), Remote>,
    sessions: HashMap<String, SessionOrder>,
    /// How far in the past entities are drawn; [`DELAY`] unless set.
    delay: Duration,
    /// Whether offline entities are hidden instead of drawn dim.
    live_only: bool,
}

impl Crowd {
    /// An empty crowd that ignores events from `me`.
    #[must_use]
    pub fn new(me: &str) -> Self {
        Self {
            me: me.to_owned(),
            entities: HashMap::new(),
            sessions: HashMap::new(),
            delay: DELAY,
            live_only: false,
        }
    }

    /// Hides offline entities instead of drawing them dim where their owners
    /// left them. A world with presence alone shows only who is there now.
    pub fn set_live_only(&mut self, live_only: bool) {
        self.live_only = live_only;
    }

    /// Draws entities `delay` in the past instead of [`DELAY`]. Publishers
    /// with a sparse cadence (the mobile profile sends a moving pose every
    /// few seconds) need a delay a little longer than their frame interval,
    /// so each drawn position lies between two received poses and motion
    /// stays continuous rather than jumping at every frame. Bounded to
    /// `[DELAY, STALE / 2]`.
    pub fn set_delay(&mut self, delay: Duration) {
        self.delay = delay.clamp(DELAY, STALE / 2);
    }

    /// How far in the past entities are drawn.
    #[must_use]
    pub fn delay(&self) -> Duration {
        self.delay
    }

    /// Number of tracked entities.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    /// Entities with a recent pose frame, excluding retained offline states.
    #[must_use]
    pub fn live_len(&self, now: Instant) -> usize {
        self.entities
            .values()
            .filter(|remote| {
                remote
                    .at(now, self.delay)
                    .is_some_and(|(_, _, online)| online)
            })
            .count()
    }

    /// True when nothing is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }

    /// Folds one received event in, at local time `now`.
    pub fn apply(&mut self, received: Received, now: Instant) {
        match received {
            Received::Frame { pubkey, frame } => {
                if pubkey == self.me {
                    return;
                }
                if let Some(order) = self.sessions.get_mut(&pubkey) {
                    if order.active == frame.s {
                        if frame.n <= order.sequence {
                            return;
                        }
                    } else {
                        if order.retired.contains(&frame.s)
                            || frame.t < order.latest_time
                            || order.retired.len() >= MAX_SESSIONS
                        {
                            return;
                        }
                        order.retired.insert(order.active.clone());
                        order.active.clone_from(&frame.s);
                    }
                    order.sequence = frame.n;
                    order.latest_time = order.latest_time.max(frame.t);
                } else {
                    if self.sessions.len() >= MAX_TRACKED {
                        return;
                    }
                    self.sessions.insert(
                        pubkey.clone(),
                        SessionOrder {
                            active: frame.s.clone(),
                            sequence: frame.n,
                            retired: HashSet::new(),
                            latest_time: frame.t,
                        },
                    );
                }
                for pose in frame.e {
                    self.push(&pubkey, &pose, now);
                }
            }
            Received::State { pubkey, state } => {
                if pubkey == self.me {
                    return;
                }
                let pose = state.pose();
                if let Some(remote) = self.entry(&pubkey, &pose) {
                    remote.state = Some((pose.pos(), pose.rot(), state.online));
                }
            }
            Received::Gesture { .. } | Received::Command { .. } => {}
        }
    }

    fn entry(&mut self, pubkey: &str, pose: &EntityPose) -> Option<&mut Remote> {
        let key = (pubkey.to_owned(), pose.id.clone());
        if !self.entities.contains_key(&key) {
            let mine = self.entities.keys().filter(|(p, _)| p == pubkey).count();
            if mine >= PER_PUBLISHER || self.entities.len() >= MAX_TRACKED {
                return None;
            }
        }
        let remote = self.entities.entry(key).or_insert_with(|| Remote {
            role: pose.role.clone(),
            samples: VecDeque::new(),
            last_frame: None,
            state: None,
            gait: Gait::default(),
            last_drawn: None,
        });
        Some(remote)
    }

    fn push(&mut self, pubkey: &str, pose: &EntityPose, now: Instant) {
        let Some(remote) = self.entry(pubkey, pose) else {
            return;
        };
        remote.role.clone_from(&pose.role);
        remote.samples.push_back(Sample {
            at: now,
            pos: pose.pos(),
            rot: pose.rot(),
        });
        while remote.samples.len() > SAMPLES {
            remote.samples.pop_front();
        }
        remote.last_frame = Some(now);
    }

    /// Where every entity is drawn at local time `now`.
    #[must_use]
    pub fn shown(&self, now: Instant) -> Vec<Shown> {
        let mut out: Vec<Shown> = self
            .entities
            .iter()
            .filter_map(|((pubkey, id), remote)| {
                let (pos, rot, online) = remote.at(now, self.delay)?;
                if self.live_only && !online {
                    return None;
                }
                Some(Shown {
                    pubkey: pubkey.clone(),
                    id: id.clone(),
                    role: remote.role.clone(),
                    pos,
                    rot,
                    online,
                })
            })
            .collect();
        out.sort_by(|a, b| (&a.pubkey, &a.id).cmp(&(&b.pubkey, &b.id)));
        out
    }

    /// Positions of entities within `radius` of `from`, nearest first.
    #[must_use]
    pub fn nearby(&self, from: Vec3, radius: f32, now: Instant) -> Vec<Vec3> {
        let mut found: Vec<Vec3> = self
            .shown(now)
            .into_iter()
            .map(|s| s.pos)
            .filter(|p| p.distance(from) <= radius)
            .collect();
        found.sort_by(|a, b| a.distance(from).total_cmp(&b.distance(from)));
        found
    }

    /// Advances walk cycles and builds every remote entity's geometry.
    pub fn mesh(&mut self, now: Instant, dt: f32) -> Mesh {
        let mut mesh = Mesh::default();
        for remote in self.entities.values_mut() {
            let Some((pos, rot, online)) = remote.at(now, self.delay) else {
                continue;
            };
            if self.live_only && !online {
                continue;
            }
            let bright = if online {
                Intensity::Full
            } else {
                Intensity::Quarter
            };
            match remote.role.as_str() {
                "avatar" => {
                    let flat = Vec3::new(pos.x, 0.0, pos.z);
                    let speed = remote.last_drawn.map_or(0.0, |last| {
                        let d = flat - Vec3::new(last.x, 0.0, last.z);
                        if dt > 0.0 { d.length() / dt } else { 0.0 }
                    });
                    remote.last_drawn = Some(pos);
                    remote.gait.advance(speed.min(12.0), pos.y > 0.05, dt);
                    mesh.extend(&avatar::figure(pos, rot, &remote.gait, bright));
                }
                "agent" => {
                    let transform = Mat4::from_rotation_translation(rot, pos);
                    mesh.extend(&agent::spade(transform, bright));
                }
                _ => {}
            }
        }
        mesh
    }

    /// Forgets entities that have neither frames nor durable state.
    pub fn prune(&mut self, now: Instant) {
        self.entities
            .retain(|_, r| r.state.is_some() || r.last_frame.is_some_and(|t| now - t < STALE * 3));
    }
}

impl Remote {
    /// Pose at `now`: interpolated frames while online, durable state
    /// otherwise.
    fn at(&self, now: Instant, delay: Duration) -> Option<(Vec3, Quat, bool)> {
        let live = self.last_frame.is_some_and(|t| now - t < STALE);
        if live {
            let render = now.checked_sub(delay).unwrap_or(now);
            return Some(interpolate(&self.samples, render)).map(|(p, q)| (p, q, true));
        }
        self.state.map(|(p, q, _)| (p, q, false))
    }
}

fn interpolate(samples: &VecDeque<Sample>, at: Instant) -> (Vec3, Quat) {
    let first = samples.front().expect("a live entity has a sample");
    if at <= first.at {
        return (first.pos, first.rot);
    }
    for pair in samples.iter().zip(samples.iter().skip(1)) {
        let (a, b) = pair;
        if at <= b.at {
            let span = (b.at - a.at).as_secs_f32().max(1e-4);
            let t = ((at - a.at).as_secs_f32() / span).clamp(0.0, 1.0);
            return (a.pos.lerp(b.pos, t), a.rot.slerp(b.rot, t));
        }
    }
    let last = samples.back().expect("non-empty");
    let n = samples.len();
    if n < 2 {
        return (last.pos, last.rot);
    }
    // Past the newest sample: continue at the last velocity for at most one
    // frame interval, the most a single missed frame can cost.
    let prev = samples[n - 2];
    let interval = last.at.saturating_duration_since(prev.at);
    if interval.is_zero() {
        return (last.pos, last.rot);
    }
    let velocity = (last.pos - prev.pos) / interval.as_secs_f32();
    if velocity.length() > MAX_EXTRAPOLATED_SPEED {
        return (last.pos, last.rot);
    }
    let ahead = at.saturating_duration_since(last.at).min(interval);
    (last.pos + velocity * ahead.as_secs_f32(), last.rot)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mv::{Frame, State};

    fn frame(n: u64, s: &str, x: f32) -> Received {
        Received::Frame {
            pubkey: "them".into(),
            frame: Frame {
                v: 1,
                s: s.into(),
                n,
                t: 0,
                e: vec![EntityPose::new(
                    "avatar",
                    "avatar",
                    Vec3::new(x, 0.0, 0.0),
                    Quat::IDENTITY,
                )],
            },
        }
    }

    #[test]
    fn frames_interpolate_in_the_past() {
        let mut crowd = Crowd::new("me");
        let t0 = Instant::now();
        crowd.apply(frame(1, "s", 0.0), t0);
        crowd.apply(frame(2, "s", 10.0), t0 + Duration::from_millis(200));
        let shown = crowd.shown(t0 + DELAY + Duration::from_millis(100));
        assert_eq!(shown.len(), 1);
        assert!((shown[0].pos.x - 5.0).abs() < 0.01, "{}", shown[0].pos.x);
        assert!(shown[0].online);
        assert_eq!(crowd.live_len(t0 + Duration::from_millis(200)), 1);
        assert_eq!(crowd.live_len(t0 + STALE + Duration::from_secs(1)), 0);
        assert_eq!(
            crowd.len(),
            1,
            "Stale geometry is retained but not counted as live"
        );
    }

    #[test]
    fn a_late_frame_is_bridged_by_one_interval_of_extrapolation() {
        let mut crowd = Crowd::new("me");
        let t0 = Instant::now();
        let step = Duration::from_millis(200);
        crowd.apply(frame(1, "s", 0.0), t0);
        crowd.apply(frame(2, "s", 2.0), t0 + step);
        let x_at = |after: Duration| crowd.shown(t0 + DELAY + after)[0].pos.x;
        // Half a frame past the newest sample: still moving at 10 m/s.
        assert!(
            (x_at(step + step / 2) - 3.0).abs() < 0.01,
            "{}",
            x_at(step + step / 2)
        );
        // One interval past: a full frame of motion, then the entity holds.
        assert!((x_at(step * 2) - 4.0).abs() < 0.01);
        assert!((x_at(step * 5) - 4.0).abs() < 0.01);
        // A jump too fast to be walking holds still.
        let mut jump = Crowd::new("me");
        jump.apply(frame(1, "s", 0.0), t0);
        jump.apply(frame(2, "s", 50.0), t0 + step);
        assert!((jump.shown(t0 + DELAY + step * 2)[0].pos.x - 50.0).abs() < 0.01);
        // A single sample has no velocity to continue.
        let mut lone = Crowd::new("me");
        lone.apply(frame(1, "s", 7.0), t0);
        assert!((lone.shown(t0 + Duration::from_secs(2))[0].pos.x - 7.0).abs() < 0.01);
    }

    #[test]
    fn a_longer_delay_keeps_sparse_frames_continuous() {
        let t0 = Instant::now();
        let gap = Duration::from_secs(3);
        let shown_at = |crowd: &Crowd, after: Duration| crowd.shown(t0 + after)[0].pos.x;
        // With the default delay, a pose three seconds after the last one
        // is drawn almost at once.
        let mut default = Crowd::new("me");
        default.apply(frame(1, "s", 0.0), t0);
        default.apply(frame(2, "s", 30.0), t0 + gap);
        assert!(shown_at(&default, gap) > 25.0);
        // A delay just past the frame interval walks between the two poses.
        let mut sparse = Crowd::new("me");
        sparse.set_delay(gap + Duration::from_millis(300));
        sparse.apply(frame(1, "s", 0.0), t0);
        sparse.apply(frame(2, "s", 30.0), t0 + gap);
        let early = shown_at(&sparse, gap + Duration::from_millis(1300));
        let later = shown_at(&sparse, gap + Duration::from_millis(2300));
        assert!(
            (early - 10.0).abs() < 0.1 && (later - 20.0).abs() < 0.1,
            "{early} {later}"
        );
        assert!(sparse.shown(t0 + gap)[0].online);
        // The delay is bounded.
        sparse.set_delay(Duration::from_secs(60));
        assert_eq!(sparse.delay(), STALE / 2);
        sparse.set_delay(Duration::ZERO);
        assert_eq!(sparse.delay(), DELAY);
    }

    #[test]
    fn stale_and_reordered_frames_are_dropped() {
        let mut crowd = Crowd::new("me");
        let t0 = Instant::now();
        crowd.apply(frame(5, "s", 1.0), t0);
        crowd.apply(frame(4, "s", 99.0), t0);
        let shown = crowd.shown(t0 + Duration::from_secs(1));
        assert!((shown[0].pos.x - 1.0).abs() < 1e-4);
        // A new session starts its own sequence.
        crowd.apply(frame(1, "t", 2.0), t0 + Duration::from_millis(10));
        let shown = crowd.shown(t0 + Duration::from_secs(1));
        assert!((shown[0].pos.x - 2.0).abs() < 1e-4);
        // A retired session cannot become current again, even at a higher n.
        crowd.apply(frame(99, "s", 99.0), t0 + Duration::from_millis(20));
        assert!((crowd.shown(t0 + Duration::from_secs(1))[0].pos.x - 2.0).abs() < 1e-4);
    }

    #[test]
    fn publisher_session_tracking_is_bounded_even_for_unrendered_entities() {
        let mut crowd = Crowd::new("me");
        for i in 0..MAX_TRACKED + 20 {
            let Received::Frame { frame, .. } = frame(1, "session", 0.0) else {
                panic!()
            };
            crowd.apply(
                Received::Frame {
                    pubkey: i.to_string(),
                    frame,
                },
                Instant::now(),
            );
        }
        assert_eq!(crowd.sessions.len(), MAX_TRACKED);
        assert!(crowd.entities.len() <= MAX_TRACKED);
    }

    #[test]
    fn my_own_events_are_ignored() {
        let mut crowd = Crowd::new("them");
        crowd.apply(frame(1, "s", 0.0), Instant::now());
        assert!(crowd.is_empty());
    }

    #[test]
    fn an_offline_entity_rests_at_its_state() {
        let mut crowd = Crowd::new("me");
        let t0 = Instant::now();
        crowd.apply(
            Received::State {
                pubkey: "them".into(),
                state: State {
                    v: 1,
                    id: "avatar".into(),
                    role: "avatar".into(),
                    p: [7.0, 0.0, 7.0],
                    q: [0.0, 0.0, 0.0, 1.0],
                    t: 0,
                    online: false,
                    follows: None,
                    name: None,
                    set: None,
                    b: None,
                },
            },
            t0,
        );
        crowd.apply(frame(1, "s", 1.0), t0);
        let later = t0 + STALE + Duration::from_secs(1);
        let shown = crowd.shown(later);
        assert!(!shown[0].online);
        assert_eq!(shown[0].pos, Vec3::new(7.0, 0.0, 7.0));
        assert_eq!(crowd.nearby(Vec3::ZERO, 20.0, later).len(), 1);
        assert!(crowd.nearby(Vec3::ZERO, 5.0, later).is_empty());
        // Presence alone shows only who is there now.
        crowd.set_live_only(true);
        assert!(crowd.shown(t0).first().is_some_and(|s| s.online));
        assert!(crowd.shown(later).is_empty());
        assert!(crowd.mesh(later, 0.016).faces.is_empty());
    }

    #[test]
    fn a_publisher_cannot_flood_the_crowd() {
        let mut crowd = Crowd::new("me");
        let now = Instant::now();
        let many: Vec<EntityPose> = (0..16)
            .map(|i| EntityPose::new(&format!("e{i}"), "object", Vec3::ZERO, Quat::IDENTITY))
            .collect();
        crowd.apply(
            Received::Frame {
                pubkey: "them".into(),
                frame: Frame {
                    v: 1,
                    s: "s".into(),
                    n: 1,
                    t: 0,
                    e: many,
                },
            },
            now,
        );
        assert_eq!(crowd.len(), PER_PUBLISHER);
    }
}
