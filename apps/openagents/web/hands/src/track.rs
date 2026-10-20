//! From landmarks to input: each hand's landmarks smoothed with a One Euro
//! filter per coordinate, its pose read by `coder_hands`' rules with
//! hysteresis, and the first hand's cursor, press, and release, a second
//! hand's pinch for spreading the graph, and an open palm held for a second
//! to let go.

use coder_hands::gestures::{HAND_LOST, PALM_MIN, PINCH_RELEASE};
use coder_hands::pose::{PINCH_PALM, pinch_ratio};
use coder_hands::{
    HandPose, INDEX_MCP, INDEX_TIP, Landmark, MIDDLE_MCP, MIDDLE_TIP, PINKY_MCP, RING_MCP,
    THUMB_TIP, WRIST, recognize,
};

use crate::filter::OneEuro;

/// A point on the page, in CSS pixels.
pub type P = (f32, f32);

/// The landmarker's confidence that a hand is a hand, below which it is
/// ignored.
pub const MIN_SCORE: f32 = 0.5;
/// A pinch closes under [`PINCH_PALM`] of a palm and opens again past this,
/// `coder_hands`' measured release.
pub const PINCH_OPEN: f32 = PINCH_PALM * (1.0 + PINCH_RELEASE);
/// The seconds a fist must read before it grabs, and the seconds it may
/// read as something else before it lets go.
pub const FIST_ON: f64 = 0.06;
pub const FIST_GRACE: f64 = 0.15;
/// The seconds an open palm is held to let go of everything.
pub const PALM_HOLD: f64 = 1.0;
/// The seconds a palm may flicker to another reading without starting
/// over.
pub const PALM_GRACE: f64 = 0.2;
/// How far, in fractions of the frame, a wrist may move between frames and
/// still be the same hand.
pub const SAME_HAND: f32 = 0.3;
/// The hand is drawn at this size around its cursor: at the frame's full
/// mapping, a hand half a metre from a laptop's camera covers half the page.
pub const DRAW_SCALE: f32 = 0.6;
/// A press lands where the cursor was this many seconds before the pinch
/// closed: closing the fingers moves the hand a little, and the press
/// belongs to what the person was pointing at.
pub const PRESS_LOOKBACK: f64 = 0.12;

/// Where the frame lands on the page. The edges of the frame are hard to
/// reach (the hand leaves the picture), so a margin at each side maps to
/// the page's edge; a hand rarely goes low, so the bottom's is widest.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mapping {
    pub side: f32,
    pub top: f32,
    pub bottom: f32,
}

impl Default for Mapping {
    fn default() -> Self {
        Self {
            side: 0.12,
            top: 0.08,
            bottom: 0.3,
        }
    }
}

impl Mapping {
    /// A frame point (normalized, unmirrored) as fractions of the page,
    /// mirrored so the hand moves the way the person does.
    pub fn to_page(&self, x: f32, y: f32) -> (f32, f32) {
        (
            ((1.0 - x) - self.side) / (1.0 - 2.0 * self.side),
            (y - self.top) / (1.0 - self.top - self.bottom),
        )
    }

    /// The other way.
    pub fn to_frame(&self, px: f32, py: f32) -> (f32, f32) {
        (
            1.0 - (px * (1.0 - 2.0 * self.side) + self.side),
            py * (1.0 - self.top - self.bottom) + self.top,
        )
    }
}

/// The point a hand points with, in the space its landmarks are in: the
/// index tip, sliding to the middle of the thumb and index tips as they
/// close (so the cursor doesn't jump when a pinch closes), or the middle of
/// the palm for a fist (`fist` from 0 to 1 blends to it).
pub fn anchor(lm: &[Landmark], fist: f32) -> (f32, f32) {
    let tip = lm[INDEX_TIP];
    let thumb = lm[THUMB_TIP];
    let k = pinch_ratio(lm)
        .map(|r| ((0.9 - r) / (0.9 - PINCH_PALM)).clamp(0.0, 1.0))
        .unwrap_or(0.0);
    let point = (
        tip.x + ((tip.x + thumb.x) / 2.0 - tip.x) * k,
        tip.y + ((tip.y + thumb.y) / 2.0 - tip.y) * k,
    );
    let palm = [WRIST, INDEX_MCP, MIDDLE_MCP, RING_MCP, PINKY_MCP]
        .iter()
        .fold((0.0, 0.0), |acc, &i| {
            (acc.0 + lm[i].x / 5.0, acc.1 + lm[i].y / 5.0)
        });
    (
        point.0 + (palm.0 - point.0) * fist,
        point.1 + (palm.1 - point.1) * fist,
    )
}

/// What a hand reads as, for the legend and the drawing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Gesture {
    #[default]
    None,
    Point,
    Pinch,
    Fist,
    Palm,
}

impl Gesture {
    pub fn word(self) -> &'static str {
        match self {
            Gesture::None => "Hand",
            Gesture::Point => "Point",
            Gesture::Pinch => "Pinch",
            Gesture::Fist => "Grab",
            Gesture::Palm => "Open palm",
        }
    }
}

/// One hand the landmarker found in a frame.
#[derive(Clone, Debug)]
pub struct RawHand {
    pub score: f32,
    pub landmarks: [Landmark; 21],
}

/// One frame from the landmarker.
#[derive(Clone, Debug, Default)]
pub struct RawFrame {
    /// The frame's width over its height.
    pub aspect: f32,
    pub hands: Vec<RawHand>,
}

/// Floats per hand on the wire: the score, then 21 joints of x, y, z.
pub const HAND_FLOATS: usize = 1 + 21 * 3;

impl RawFrame {
    /// The page's packed frame: `[aspect, count, (score, x0, y0, z0, ...)
    /// per hand]`. `None` when it is cut short.
    pub fn decode(data: &[f32]) -> Option<RawFrame> {
        let (&aspect, rest) = data.split_first()?;
        let (&count, rest) = rest.split_first()?;
        let count = count as usize;
        if count > 4 || rest.len() < count * HAND_FLOATS || !(aspect > 0.1 && aspect < 10.0) {
            return None;
        }
        let hands = rest
            .chunks_exact(HAND_FLOATS)
            .take(count)
            .map(|h| {
                let mut landmarks = [Landmark::default(); 21];
                for (i, lm) in landmarks.iter_mut().enumerate() {
                    *lm = Landmark {
                        x: h[1 + i * 3],
                        y: h[2 + i * 3],
                        z: h[3 + i * 3],
                    };
                }
                RawHand {
                    score: h[0],
                    landmarks,
                }
            })
            .collect();
        Some(RawFrame { aspect, hands })
    }

    /// [`RawFrame::decode`]'s input, for the tests and the test hook.
    pub fn encode(&self) -> Vec<f32> {
        let mut out = vec![self.aspect, self.hands.len() as f32];
        for h in &self.hands {
            out.push(h.score);
            for lm in &h.landmarks {
                out.extend([lm.x, lm.y, lm.z]);
            }
        }
        out
    }
}

/// One tracked hand.
#[derive(Clone, Debug)]
pub struct Tracker {
    filters: Vec<OneEuro>,
    /// Smoothed, in the frame's normalized coordinates.
    pub smoothed: [Landmark; 21],
    pub born: f64,
    pub seen: f64,
    pub pinched: bool,
    pub ratio: f32,
    fist: bool,
    fist_since: Option<f64>,
    fist_last: f64,
    fist_w: f32,
    palm_since: Option<f64>,
    palm_last: f64,
    palm_fired: bool,
    pub gesture: Gesture,
}

/// What one frame changed for a hand.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    /// The hand started or stopped holding (a pinch or a fist).
    pub grab: Option<bool>,
    /// An open palm was held long enough.
    pub palm: bool,
}

impl Tracker {
    pub fn new(t: f64) -> Self {
        Self {
            filters: vec![OneEuro::default(); 63],
            smoothed: [Landmark::default(); 21],
            born: t,
            seen: t,
            pinched: false,
            ratio: 1.0,
            fist: false,
            fist_since: None,
            fist_last: f64::NEG_INFINITY,
            fist_w: 0.0,
            palm_since: None,
            palm_last: f64::NEG_INFINITY,
            palm_fired: false,
            gesture: Gesture::None,
        }
    }

    /// Whether the hand is holding something: a pinch or a fist.
    pub fn grabbing(&self) -> bool {
        self.pinched || self.fist
    }

    /// How far through the palm hold the hand is, from 0 to 1.
    pub fn palm_progress(&self, t: f64) -> f32 {
        match self.palm_since {
            Some(since) if !self.palm_fired => ((t - since) / PALM_HOLD).clamp(0.0, 1.0) as f32,
            _ => 0.0,
        }
    }

    /// The landmarks in aspect space, where both axes share a unit.
    fn aspect_space(&self, aspect: f32) -> [Landmark; 21] {
        self.smoothed.map(|p| Landmark {
            x: p.x * aspect,
            ..p
        })
    }

    /// The cursor, as fractions of the page.
    pub fn cursor(&self, aspect: f32, map: &Mapping) -> (f32, f32) {
        let (x, y) = anchor(&self.aspect_space(aspect), self.fist_w);
        map.to_page(x / aspect, y)
    }

    pub fn update(&mut self, raw: &[Landmark; 21], aspect: f32, t: f64) -> Step {
        let dt = (t - self.seen).max(0.0) as f32;
        self.seen = t;
        for (i, lm) in raw.iter().enumerate() {
            let s = &mut self.smoothed[i];
            s.x = self.filters[i * 3].filter(lm.x, t);
            s.y = self.filters[i * 3 + 1].filter(lm.y, t);
            s.z = self.filters[i * 3 + 2].filter(lm.z, t);
        }
        let lm = self.aspect_space(aspect);
        let ratio = pinch_ratio(&lm).unwrap_or(1.0);
        self.ratio = ratio;
        let pose = recognize(&lm);
        let mut step = Step::default();
        let was = self.grabbing();

        // The pinch, with its hysteresis.
        if self.pinched {
            if ratio > PINCH_OPEN {
                self.pinched = false;
            }
        } else if ratio < PINCH_PALM && !self.fist {
            self.pinched = true;
        }

        // The fist, after a short read, with a grace to let go.
        if pose == HandPose::Fist && !self.pinched {
            self.fist_last = t;
            let since = *self.fist_since.get_or_insert(t);
            if t - since >= FIST_ON {
                self.fist = true;
            }
        } else if t - self.fist_last > FIST_GRACE {
            self.fist = false;
            self.fist_since = None;
        }
        let target = if self.fist { 1.0 } else { 0.0 };
        let ease = 1.0 - (-dt / 0.08).exp();
        self.fist_w += (target - self.fist_w) * ease.clamp(0.0, 1.0);

        // The open palm, held.
        let palm = matches!(pose, HandPose::OpenHand | HandPose::FlatHand);
        if palm {
            self.palm_last = t;
            let since = *self.palm_since.get_or_insert(t);
            if !self.palm_fired && t - since >= PALM_HOLD {
                self.palm_fired = true;
                step.palm = true;
            }
        } else if t - self.palm_last > PALM_GRACE {
            self.palm_since = None;
            self.palm_fired = false;
        }

        self.gesture = if self.pinched {
            Gesture::Pinch
        } else if self.fist {
            Gesture::Fist
        } else if palm {
            Gesture::Palm
        } else if pointing(&lm) {
            Gesture::Point
        } else {
            Gesture::None
        };
        let now = self.grabbing();
        if now != was {
            step.grab = Some(now);
        }
        step
    }
}

/// The pointing hand, by `coder_hands`' rule: the index tip well out from
/// the wrist and the middle finger curled, measured in palms (the wrist to
/// the middle knuckle).
fn pointing(lm: &[Landmark]) -> bool {
    let d = |a: Landmark, b: Landmark| ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt();
    let palm = d(lm[WRIST], lm[MIDDLE_MCP]);
    if palm <= f32::EPSILON {
        return false;
    }
    d(lm[WRIST], lm[INDEX_TIP]) / palm > coder_hands::gestures::POINT_EXTENDED
        && d(lm[WRIST], lm[MIDDLE_TIP]) / palm < coder_hands::gestures::POINT_CURLED
}

/// Pointer input, from a hand or the mouse.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Move(P),
    Down(P),
    Up(P),
    /// Let go of everything (an open palm held).
    Cancel,
    /// Both hands pinched, this far apart.
    ZoomStart(f32),
    Zoom(f32),
    ZoomEnd,
}

/// A hand as the page draws it.
#[derive(Clone, Debug, PartialEq)]
pub struct HandView {
    /// Each joint on the page.
    pub points: [P; 21],
    /// Each joint's depth, nearer the camera more negative.
    pub depth: [f32; 21],
    pub cursor: P,
    pub gesture: Gesture,
    pub grabbing: bool,
    pub primary: bool,
    pub palm: f32,
}

/// Every hand, and the input they make.
#[derive(Clone, Debug, Default)]
pub struct Hands {
    pub map: Mapping,
    slots: [Option<Tracker>; 2],
    primary: Option<usize>,
    aspect: f32,
    holding: bool,
    zooming: bool,
    last_cursor: Option<P>,
    /// The first hand's recent cursor, for where a press lands.
    trail: Vec<(f64, P)>,
    /// When each recent frame came, for the rate.
    stamps: Vec<f64>,
    pub frames: u64,
}

impl Hands {
    pub fn new() -> Self {
        Self {
            aspect: 4.0 / 3.0,
            ..Self::default()
        }
    }

    /// Frames a second over the last second.
    pub fn fps(&self) -> f32 {
        match (self.stamps.first(), self.stamps.last()) {
            (Some(a), Some(b)) if b > a => (self.stamps.len() - 1) as f32 / (b - a) as f32,
            _ => 0.0,
        }
    }

    /// Whether any hand is in view.
    pub fn any(&self) -> bool {
        self.slots.iter().any(Option::is_some)
    }

    /// Forgets every hand, releasing what the first one held.
    pub fn clear(&mut self) -> Vec<Input> {
        let mut out = Vec::new();
        if self.zooming {
            out.push(Input::ZoomEnd);
        }
        if self.holding
            && let Some(p) = self.last_cursor
        {
            out.push(Input::Up(p));
        }
        *self = Self {
            map: self.map,
            aspect: self.aspect,
            ..Self::default()
        };
        out
    }

    fn page(&self, t: &Tracker, size: P) -> P {
        let (x, y) = t.cursor(self.aspect, &self.map);
        (x * size.0, y * size.1)
    }

    /// Reads one frame at `t` seconds on a page `size` big.
    pub fn frame(&mut self, frame: &RawFrame, t: f64, size: P) -> Vec<Input> {
        self.frames += 1;
        self.stamps.push(t);
        self.stamps.retain(|s| t - s <= 1.0);
        self.aspect = frame.aspect;
        let mut out = Vec::new();
        let mut used = [false; 2];
        let mut cancel = false;
        let mut primary_grab: Option<bool> = None;
        for hand in &frame.hands {
            if hand.score < MIN_SCORE {
                continue;
            }
            let lm = &hand.landmarks;
            let palm_h = ((lm[WRIST].x - lm[MIDDLE_MCP].x).powi(2) * frame.aspect.powi(2)
                + (lm[WRIST].y - lm[MIDDLE_MCP].y).powi(2))
            .sqrt();
            if palm_h < PALM_MIN {
                continue;
            }
            // The nearest tracked wrist, else a free slot.
            let near = (0..2)
                .filter(|&i| !used[i])
                .filter_map(|i| {
                    let tr = self.slots[i].as_ref()?;
                    let w = tr.smoothed[WRIST];
                    let d = ((w.x - lm[WRIST].x).powi(2) + (w.y - lm[WRIST].y).powi(2)).sqrt();
                    (d < SAME_HAND).then_some((i, d))
                })
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i);
            let Some(slot) = near.or_else(|| (0..2).find(|&i| !used[i] && self.slots[i].is_none()))
            else {
                continue;
            };
            used[slot] = true;
            let tracker = self.slots[slot].get_or_insert_with(|| Tracker::new(t));
            let step = tracker.update(lm, frame.aspect, t);
            cancel |= step.palm;
            if self.primary.is_none() {
                self.primary = Some(slot);
            }
            if self.primary == Some(slot) {
                primary_grab = step.grab;
            }
        }
        // Hands gone long enough are dropped.
        for i in 0..2 {
            if !used[i]
                && self.slots[i]
                    .as_ref()
                    .is_some_and(|tr| t - tr.seen > HAND_LOST)
            {
                self.slots[i] = None;
                if self.primary == Some(i) {
                    self.primary = None;
                    self.trail.clear();
                    if self.holding
                        && let Some(p) = self.last_cursor
                    {
                        out.push(Input::Up(p));
                    }
                    self.holding = false;
                }
            }
        }
        if self.primary.is_none() {
            self.primary = (0..2)
                .filter(|&i| self.slots[i].is_some())
                .min_by(|&a, &b| {
                    let born = |i: usize| self.slots[i].as_ref().map_or(f64::MAX, |s| s.born);
                    born(a).total_cmp(&born(b))
                });
            // A hand that takes over while holding starts held: no press.
            self.holding = self
                .primary
                .and_then(|i| self.slots[i].as_ref())
                .is_some_and(Tracker::grabbing);
        }
        if let Some(i) = self.primary
            && used[i]
            && let Some(tr) = self.slots[i].as_ref()
        {
            let p = self.page(tr, size);
            self.last_cursor = Some(p);
            self.trail.push((t, p));
            self.trail.retain(|(s, _)| t - s <= 0.5);
            out.push(Input::Move(p));
            match primary_grab {
                Some(true) => {
                    self.holding = true;
                    let back = self
                        .trail
                        .iter()
                        .rev()
                        .find(|(s, _)| t - s >= PRESS_LOOKBACK)
                        .or(self.trail.first())
                        .map_or(p, |(_, q)| *q);
                    out.push(Input::Down(back));
                }
                Some(false) if self.holding => {
                    self.holding = false;
                    out.push(Input::Up(p));
                }
                _ => {}
            }
        }
        // Two hands holding: spread.
        let both = match (&self.slots[0], &self.slots[1]) {
            (Some(a), Some(b)) if a.grabbing() && b.grabbing() => {
                let (pa, pb) = (self.page(a, size), self.page(b, size));
                Some(((pa.0 - pb.0).powi(2) + (pa.1 - pb.1).powi(2)).sqrt())
            }
            _ => None,
        };
        match (both, self.zooming) {
            (Some(d), false) => {
                self.zooming = true;
                out.push(Input::ZoomStart(d));
            }
            (Some(d), true) => out.push(Input::Zoom(d)),
            (None, true) => {
                self.zooming = false;
                out.push(Input::ZoomEnd);
            }
            (None, false) => {}
        }
        if cancel {
            out.push(Input::Cancel);
        }
        out
    }

    /// Every hand in view, as the page draws it.
    pub fn views(&self, t: f64, size: P) -> Vec<HandView> {
        (0..2)
            .filter_map(|i| {
                let tr = self.slots[i].as_ref()?;
                let cursor = self.page(tr, size);
                let mut points = [(0.0, 0.0); 21];
                let mut depth = [0.0; 21];
                for (j, lm) in tr.smoothed.iter().enumerate() {
                    let (x, y) = self.map.to_page(lm.x, lm.y);
                    points[j] = (
                        cursor.0 + (x * size.0 - cursor.0) * DRAW_SCALE,
                        cursor.1 + (y * size.1 - cursor.1) * DRAW_SCALE,
                    );
                    depth[j] = lm.z;
                }
                Some(HandView {
                    points,
                    depth,
                    cursor,
                    gesture: tr.gesture,
                    grabbing: tr.grabbing(),
                    primary: self.primary == Some(i),
                    palm: tr.palm_progress(t),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::synth::{Pose, hand};

    const SIZE: P = (1440.0, 900.0);
    const ASPECT: f32 = 4.0 / 3.0;

    /// A recorded-style sequence: each step a pose at a page fraction, for
    /// so many frames at 30 a second.
    fn play(hands: &mut Hands, t: &mut f64, steps: &[(Pose, f32, f32, u32)]) -> Vec<Input> {
        let mut out = Vec::new();
        for &(pose, x, y, frames) in steps {
            for _ in 0..frames {
                let frame = RawFrame {
                    aspect: ASPECT,
                    hands: vec![RawHand {
                        score: 0.95,
                        landmarks: hand(pose, x, y, ASPECT, &hands.map),
                    }],
                };
                // Through the wire format, as the page sends it.
                let frame = RawFrame::decode(&frame.encode()).unwrap();
                out.extend(hands.frame(&frame, *t, SIZE));
                *t += 1.0 / 30.0;
            }
        }
        out
    }

    fn presses(inputs: &[Input]) -> Vec<&'static str> {
        inputs
            .iter()
            .filter_map(|i| match i {
                Input::Down(_) => Some("down"),
                Input::Up(_) => Some("up"),
                Input::Cancel => Some("cancel"),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn the_cursor_lands_where_the_hand_points() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let out = play(&mut hands, &mut t, &[(Pose::Point, 0.25, 0.4, 20)]);
        let Some(Input::Move(p)) = out.last() else {
            panic!("{out:?}")
        };
        assert!(
            (p.0 - 0.25 * SIZE.0).abs() < 4.0 && (p.1 - 0.4 * SIZE.1).abs() < 4.0,
            "{p:?}"
        );
        assert_eq!(hands.views(t, SIZE)[0].gesture, Gesture::Point);
        assert!(presses(&out).is_empty());
    }

    #[test]
    fn a_pinch_presses_once_and_releases_once() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let out = play(
            &mut hands,
            &mut t,
            &[
                (Pose::Point, 0.5, 0.5, 10),
                (Pose::Pinch, 0.5, 0.5, 10),
                (Pose::Point, 0.5, 0.5, 10),
            ],
        );
        assert_eq!(presses(&out), ["down", "up"]);
    }

    #[test]
    fn a_pinch_that_flickers_at_the_threshold_stays_held() {
        // Gaps between the close and open thresholds: hysteresis keeps it.
        let mut tr = Tracker::new(0.0);
        let mut t = 0.0;
        let mut downs = 0;
        let mut ups = 0;
        let mut lm = hand(Pose::Pinch, 0.5, 0.5, ASPECT, &Mapping::default());
        let base = lm;
        let palm = coder_hands::pose::palm_span(
            &base
                .iter()
                .map(|p| Landmark {
                    x: p.x * ASPECT,
                    ..*p
                })
                .collect::<Vec<_>>(),
        )
        .unwrap();
        for i in 0..90 {
            // The thumb moves between 0.1 and 0.5 palms from the index tip.
            let gap = if i < 15 {
                0.1
            } else if i % 2 == 0 {
                0.5
            } else {
                0.3
            };
            lm[THUMB_TIP].x = base[INDEX_TIP].x;
            lm[THUMB_TIP].y = base[INDEX_TIP].y + gap * palm;
            // Hold the filter out of it: the raw value each frame.
            tr.filters.iter_mut().for_each(OneEuro::reset);
            match tr.update(&lm, ASPECT, t).grab {
                Some(true) => downs += 1,
                Some(false) => ups += 1,
                None => {}
            }
            t += 1.0 / 30.0;
        }
        assert_eq!((downs, ups), (1, 0));
        // Past the open threshold, it lets go.
        lm[THUMB_TIP].y = base[INDEX_TIP].y + 0.8 * palm;
        tr.filters.iter_mut().for_each(OneEuro::reset);
        assert_eq!(tr.update(&lm, ASPECT, t).grab, Some(false));
    }

    #[test]
    fn a_fist_grabs_and_drags() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let mut steps = vec![(Pose::Point, 0.3, 0.5, 8), (Pose::Fist, 0.3, 0.5, 8)];
        for k in 0..10 {
            steps.push((Pose::Fist, 0.3 + 0.03 * k as f32, 0.5, 1));
        }
        steps.push((Pose::Open, 0.6, 0.5, 6));
        let out = play(&mut hands, &mut t, &steps);
        assert_eq!(presses(&out), ["down", "up"]);
        let down = out
            .iter()
            .position(|i| matches!(i, Input::Down(_)))
            .unwrap();
        let up = out.iter().position(|i| matches!(i, Input::Up(_))).unwrap();
        let moved: Vec<f32> = out[down..up]
            .iter()
            .filter_map(|i| match i {
                Input::Move(p) => Some(p.0),
                _ => None,
            })
            .collect();
        assert!(moved.last().unwrap() - moved.first().unwrap() > 0.15 * SIZE.0);
    }

    #[test]
    fn an_open_palm_held_a_second_cancels_once() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let short = play(&mut hands, &mut t, &[(Pose::Open, 0.5, 0.5, 20)]);
        assert!(presses(&short).is_empty());
        assert!(hands.views(t, SIZE)[0].palm > 0.5);
        let long = play(&mut hands, &mut t, &[(Pose::Open, 0.5, 0.5, 40)]);
        assert_eq!(presses(&long), ["cancel"]);
    }

    #[test]
    fn two_hands_pinching_spread() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let mut out = Vec::new();
        for f in 0..30 {
            let spread = if f < 10 { 0.0 } else { (f - 10) as f32 * 0.01 };
            let pose = if f < 5 { Pose::Point } else { Pose::Pinch };
            let frame = RawFrame {
                aspect: ASPECT,
                hands: vec![
                    RawHand {
                        score: 0.9,
                        landmarks: hand(pose, 0.4 - spread, 0.5, ASPECT, &hands.map),
                    },
                    RawHand {
                        score: 0.9,
                        landmarks: hand(pose, 0.6 + spread, 0.5, ASPECT, &hands.map),
                    },
                ],
            };
            out.extend(hands.frame(&frame, t, SIZE));
            t += 1.0 / 30.0;
        }
        let start = out.iter().find_map(|i| match i {
            Input::ZoomStart(d) => Some(*d),
            _ => None,
        });
        let last = out.iter().rev().find_map(|i| match i {
            Input::Zoom(d) => Some(*d),
            _ => None,
        });
        let (start, last) = (start.unwrap(), last.unwrap());
        assert!(last > start * 1.5, "{start} {last}");
        // Losing the hands ends it, and releases the first one.
        let end = hands.frame(
            &RawFrame {
                aspect: ASPECT,
                hands: vec![],
            },
            t + 1.0,
            SIZE,
        );
        assert!(end.contains(&Input::ZoomEnd));
        assert!(end.iter().any(|i| matches!(i, Input::Up(_))));
    }

    #[test]
    fn a_hand_lost_mid_pinch_releases() {
        let mut hands = Hands::new();
        let mut t = 0.0;
        let out = play(&mut hands, &mut t, &[(Pose::Pinch, 0.5, 0.5, 10)]);
        assert_eq!(presses(&out), ["down"]);
        let blip = hands.frame(
            &RawFrame {
                aspect: ASPECT,
                hands: vec![],
            },
            t + 0.1,
            SIZE,
        );
        assert!(presses(&blip).is_empty(), "a short gap holds");
        let gone = hands.frame(
            &RawFrame {
                aspect: ASPECT,
                hands: vec![],
            },
            t + 0.5,
            SIZE,
        );
        assert_eq!(presses(&gone), ["up"]);
        assert!(!hands.any());
    }

    #[test]
    fn unsure_hands_and_bad_frames_are_ignored() {
        let mut hands = Hands::new();
        let frame = RawFrame {
            aspect: ASPECT,
            hands: vec![RawHand {
                score: 0.2,
                landmarks: hand(Pose::Point, 0.5, 0.5, ASPECT, &hands.map),
            }],
        };
        assert!(hands.frame(&frame, 0.0, SIZE).is_empty());
        assert!(RawFrame::decode(&[1.3, 2.0, 0.5]).is_none());
        assert!(RawFrame::decode(&[]).is_none());
    }

    #[test]
    fn the_mapping_mirrors_and_round_trips() {
        let m = Mapping::default();
        let (x, _) = m.to_page(0.2, 0.5);
        assert!(x > 0.5, "the right of the frame is the left of the page");
        let (fx, fy) = m.to_frame(0.3, 0.7);
        let (px, py) = m.to_page(fx, fy);
        assert!((px - 0.3).abs() < 1e-5 && (py - 0.7).abs() < 1e-5);
    }
}
