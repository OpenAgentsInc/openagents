//! From the hand landmarker's points to two hands in the player's body.
//!
//! The browser's landmarker reports up to two hands a frame as 21 points in
//! the camera's picture: normalized, unmirrored, `y` down. This module keeps
//! one smoothed [`Tracker`] for each of the player's hands, decides which
//! hand is which, and places each in **body space**: metres, the origin on
//! the floor under the player, `+x` to the player's left, `+y` up, and `+z`
//! forward, toward the enemy. (Facing `+z` in a right-handed world, the
//! player's left is `+x`.)
//!
//! A laptop camera sees the person from the front, so the person's right
//! hand is on the picture's left. Height comes from the picture's `y`, side
//! from its `x`, and reach from how big the palm looks: a hand pushed toward
//! the camera looks bigger.

use glam::Vec3;
use openagents_web_hands::track::{Gesture, RawFrame, Tracker};
use openagents_web_hands::{Landmark, track::MIN_SCORE};

/// The wrist and the knuckles the palm's center averages.
const WRIST: usize = 0;
const INDEX_MCP: usize = 5;
const MIDDLE_MCP: usize = 9;
const PINKY_MCP: usize = 17;

/// How many metres one picture height spans where the hands are.
pub const FRAME_METRES: f32 = 1.3;
/// The body height the middle of the picture maps to, m: about the chin.
pub const CENTER_HEIGHT: f32 = 1.45;
/// The palm's length (the wrist to the middle knuckle) in picture heights
/// at the resting reach: a hand about half a metre from a laptop camera.
pub const PALM_AT_REST: f32 = 0.2;
/// How far forward a hand at the resting reach is, m.
pub const REST_REACH: f32 = 0.35;
/// Reach gained, m, for each palm length the hand grows by.
pub const REACH_GAIN: f32 = 0.6;
/// The reach's limits, m.
pub const REACH_MIN: f32 = 0.15;
pub const REACH_MAX: f32 = 0.85;
/// How far forward a held blade leans from the fingers' direction.
pub const BLADE_LEAN: f32 = 0.9;
/// A hand not seen for this long is gone, s.
pub const LOST_AFTER: f64 = 0.25;
/// A hand this close to where a tracked hand was, in picture heights, is
/// that hand.
pub const SAME_HAND: f32 = 0.3;
/// The velocity reads the hand's motion over about this long, s.
pub const VELOCITY_SPAN: f64 = 0.06;

/// Which of the player's hands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// The shield hand.
    Left,
    /// The weapon hand.
    Right,
}

/// What a hand is doing with its fingers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Grip {
    /// Every finger out.
    Open,
    /// A fist or a pinch: the hand holds something.
    Closed,
    /// Anything between.
    #[default]
    Loose,
}

impl Grip {
    fn from_gesture(gesture: Gesture) -> Self {
        match gesture {
            Gesture::Fist | Gesture::Pinch => Grip::Closed,
            Gesture::Palm => Grip::Open,
            Gesture::Point | Gesture::None => Grip::Loose,
        }
    }
}

/// One hand in body space this frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandPose {
    /// The palm's center, m.
    pub pos: Vec3,
    /// From the wrist toward the fingers, a unit vector: the way a held
    /// blade points.
    pub up: Vec3,
    /// The hand's velocity, m/s.
    pub vel: Vec3,
    pub grip: Grip,
}

impl HandPose {
    /// A still hand at `pos` with its fingers up, for tests and captures.
    #[must_use]
    pub fn at(pos: Vec3, grip: Grip) -> Self {
        Self {
            pos,
            up: Vec3::Y,
            vel: Vec3::ZERO,
            grip,
        }
    }
}

/// Where a point of the picture is in body space, before reach: `x` and
/// `y` are the picture's normalized coordinates, `aspect` its width over
/// its height.
#[must_use]
pub fn picture_to_body(x: f32, y: f32, aspect: f32) -> (f32, f32) {
    (
        (x - 0.5) * aspect * FRAME_METRES,
        CENTER_HEIGHT + (0.5 - y) * FRAME_METRES,
    )
}

/// The reach for a palm `palm` picture heights long.
#[must_use]
pub fn reach(palm: f32) -> f32 {
    (REST_REACH + (palm / PALM_AT_REST - 1.0) * REACH_GAIN).clamp(REACH_MIN, REACH_MAX)
}

/// The palm's center in the picture, as `(x, y)` normalized.
fn palm_center(lm: &[Landmark; 21]) -> (f32, f32) {
    let ids = [WRIST, INDEX_MCP, MIDDLE_MCP, PINKY_MCP];
    let (x, y) = ids
        .iter()
        .fold((0.0, 0.0), |acc, &i| (acc.0 + lm[i].x, acc.1 + lm[i].y));
    (x / ids.len() as f32, y / ids.len() as f32)
}

/// The hand's place and direction in body space from its points.
#[must_use]
pub fn place(lm: &[Landmark; 21], aspect: f32) -> (Vec3, Vec3) {
    let (cx, cy) = palm_center(lm);
    let (bx, by) = picture_to_body(cx, cy, aspect);
    let wrist = lm[WRIST];
    let knuckle = lm[MIDDLE_MCP];
    let dx = (knuckle.x - wrist.x) * aspect;
    let dy = knuckle.y - wrist.y;
    let palm = dx.hypot(dy);
    // The picture's right is the body's left (`+x`), and its y runs down.
    let flat = Vec3::new(dx, -dy, 0.0).normalize_or(Vec3::Y);
    // A held blade leans forward of the fingers, toward the enemy.
    let up = (flat + Vec3::Z * BLADE_LEAN).normalize();
    (Vec3::new(bx, by, reach(palm)), up)
}

/// One tracked hand.
struct Slot {
    tracker: Tracker,
    /// Where it was last in the picture, normalized.
    last: (f32, f32),
    /// Recent body positions, for the velocity.
    trail: Vec<(f64, Vec3)>,
    pose: HandPose,
}

impl Slot {
    fn new(t: f64, at: (f32, f32)) -> Self {
        Self {
            tracker: Tracker::new(t),
            last: at,
            trail: Vec::new(),
            pose: HandPose::at(Vec3::ZERO, Grip::Loose),
        }
    }

    fn update(&mut self, lm: &[Landmark; 21], aspect: f32, t: f64) {
        let _ = self.tracker.update(lm, aspect, t);
        self.last = palm_center(lm);
        let (pos, up) = place(&self.tracker.smoothed, aspect);
        self.trail.push((t, pos));
        self.trail.retain(|(at, _)| t - at <= VELOCITY_SPAN * 3.0);
        let vel = velocity(&self.trail, t);
        self.pose = HandPose {
            pos,
            up,
            vel,
            grip: Grip::from_gesture(self.tracker.gesture),
        };
    }
}

/// The velocity across the trail's last [`VELOCITY_SPAN`].
fn velocity(trail: &[(f64, Vec3)], t: f64) -> Vec3 {
    let Some(&(_, now)) = trail.last() else {
        return Vec3::ZERO;
    };
    let from = trail
        .iter()
        .find(|(at, _)| t - at <= VELOCITY_SPAN)
        .or(trail.first());
    match from {
        Some(&(at, then)) if t - at > 1e-4 => (now - then) / (t - at) as f32,
        _ => Vec3::ZERO,
    }
}

/// Both hands, tracked across frames.
#[derive(Default)]
pub struct Body {
    left: Option<Slot>,
    right: Option<Slot>,
    /// Frames read, for the screen's tracking note.
    pub frames: u64,
}

impl Body {
    /// Forget both hands.
    pub fn clear(&mut self) {
        self.left = None;
        self.right = None;
    }

    /// The hand on `side`, while it is tracked.
    #[must_use]
    pub fn hand(&self, side: Side) -> Option<HandPose> {
        match side {
            Side::Left => self.left.as_ref(),
            Side::Right => self.right.as_ref(),
        }
        .map(|slot| slot.pose)
    }

    /// The hand's smoothed points, for drawing it.
    #[must_use]
    pub fn points(&self, side: Side) -> Option<[Landmark; 21]> {
        match side {
            Side::Left => self.left.as_ref(),
            Side::Right => self.right.as_ref(),
        }
        .map(|slot| slot.tracker.smoothed)
    }

    /// Reads one frame at `t` seconds.
    pub fn frame(&mut self, frame: &RawFrame, t: f64) {
        self.frames += 1;
        let aspect = frame.aspect;
        let mut found: Vec<([Landmark; 21], (f32, f32))> = frame
            .hands
            .iter()
            .filter(|h| h.score >= MIN_SCORE)
            .take(2)
            .map(|h| (h.landmarks, palm_center(&h.landmarks)))
            .collect();
        let mut used = [false, false];
        // Each tracked hand takes the nearest new hand within reach.
        for side in [Side::Left, Side::Right] {
            let slot = match side {
                Side::Left => &mut self.left,
                Side::Right => &mut self.right,
            };
            let Some(s) = slot.as_mut() else { continue };
            let nearest = found
                .iter()
                .enumerate()
                .filter(|(i, _)| !used[*i])
                .map(|(i, (_, c))| (i, ((c.0 - s.last.0) * aspect).hypot(c.1 - s.last.1)))
                .filter(|(_, d)| *d < SAME_HAND)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            if let Some((i, _)) = nearest {
                used[i] = true;
                s.update(&found[i].0, aspect, t);
            }
        }
        // New hands: the person's right hand is on the picture's left.
        let mut fresh: Vec<usize> = (0..found.len()).filter(|i| !used[*i]).collect();
        fresh.sort_by(|a, b| found[*a].1.0.total_cmp(&found[*b].1.0));
        for i in fresh {
            let (lm, center) = found[i];
            let prefer = if center.0 < 0.5 {
                Side::Right
            } else {
                Side::Left
            };
            let other = match prefer {
                Side::Left => Side::Right,
                Side::Right => Side::Left,
            };
            let side = if self.slot(prefer).is_none() {
                prefer
            } else if self.slot(other).is_none() {
                other
            } else {
                continue;
            };
            let mut slot = Slot::new(t, center);
            slot.update(&lm, aspect, t);
            match side {
                Side::Left => self.left = Some(slot),
                Side::Right => self.right = Some(slot),
            }
        }
        found.clear();
        self.expire(t);
    }

    fn slot(&self, side: Side) -> Option<&Slot> {
        match side {
            Side::Left => self.left.as_ref(),
            Side::Right => self.right.as_ref(),
        }
    }

    /// Drops hands not seen since `t - LOST_AFTER`, for frames with no
    /// hands at all.
    pub fn expire(&mut self, t: f64) {
        for slot in [&mut self.left, &mut self.right] {
            if slot
                .as_ref()
                .is_some_and(|s| t - s.tracker.seen > LOST_AFTER)
            {
                *slot = None;
            }
        }
    }
}
