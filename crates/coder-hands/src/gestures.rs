//! The gestures a tracked hand makes, and what each one asks the desk for.
//!
//! The camera daemon publishes 21 landmarks a frame. This module turns a
//! sequence of those frames into desk input: an extended index finger moves
//! the pointer, a pinch presses and releases, a flat palm moved sideways
//! switches desks, and a fist held for two seconds sends Escape.
//!
//! The pose under each gesture is the one `crates/coder-hands` recognizes,
//! and every pose rule's margin comes from `crate::margins`: the
//! measured ratio less the rule's constant, in units of the constant, so a
//! margin near zero is a reading the next frame could flip. This module
//! adds one rule of its own, the pointing hand, which the pose set has no
//! label for, and measures it the same way. Each rule's margin goes to the
//! log beside its label, and the window's label sequence goes with it,
//! because a thin margin is what the Jev seam in [`crate::watch`]
//! triggers on and the sequence is that seam's corpus. Nothing here asks a model
//! anything.
//!
//! The pointing rule measures in palms: the wrist to the middle finger's
//! knuckle is one, so the same constant holds at any distance from the
//! camera. Frame coordinates are the camera's, with `y` down and the
//! picture unmirrored, so a point is mirrored before it reaches a screen
//! and the hand moves the pointer the way a mouse does.
//!
//! `bench/golden/hands/` holds one landmark sequence a case, one wire line
//! a row, with the acts each owes in a sidecar; the test at the bottom
//! walks it.

use std::collections::VecDeque;

use crate::wire::{Line, WireHand};
use crate::{
    HandPose, INDEX_TIP, Landmark, MIDDLE_MCP, MIDDLE_TIP, Margins, THUMB_TIP, WRIST, margins,
    recognize,
};
use coder_wm::Dir;

/// How far past the pinch rule's own constant a held pinch opens before it
/// releases, in units of that constant: a margin at or under the negative
/// of this lets go. The gap between it and the rule's zero is the
/// hysteresis, so a pinch that flickers at the threshold stays held.
///
/// This puts the release at 0.55 of a palm. On the recordings of
/// 2026-09-18 no reading of a closed pinch reached 0.43 of a palm and a
/// deliberately open hand sat above 0.42, so the band from 0.35 to 0.55
/// is where a hand is neither clearly holding nor clearly open, and a
/// drag holds through it. Replaying those recordings, the shallower 0.35
/// this replaces took 86 presses with a median hold of 0.33 seconds where
/// this takes 74 with a median hold of 0.47.
pub const PINCH_RELEASE: f32 = 0.57;
/// An index tip this many palms from the wrist counts as extended, which
/// with a curled middle finger is the pointing hand.
///
/// Over the 732 recorded readings the pose set has no label for, the
/// index tip sat between 1.3 and 1.9 palms from the wrist whenever the
/// middle finger was already curled, with its tenth percentile at 1.28
/// and no trough anywhere in that range. The 1.50 this replaces cut
/// through the middle of that one population: 75 readings sat within
/// 0.05 palms of it, so half the hands whose middle finger was curled
/// were rejected on a margin the next frame could flip. At 1.30 nine
/// readings sit that close, the rule reads 142 frames as a point rather
/// than 102, and the label flips over the recordings fall from 324 to
/// 317. The curled clause below has the trough, so it does the deciding
/// and this one guards against a hand with nothing extended at all.
pub const POINT_EXTENDED: f32 = 1.30;
/// A middle tip within this many palms of the wrist counts as curled.
/// The recorded readings put a trough here: the middle tip's distance is
/// two humps, one under 1.1 palms for a curled finger and one over 1.3
/// for an extended one, with the fewest readings of all in the 1.2 bin
/// between them. Unchanged.
pub const POINT_CURLED: f32 = 1.20;
/// How far a flat palm moves across the frame, as a fraction of its
/// width, for a swipe. Unmeasured: the recordings of 2026-09-18 hold no
/// swipe, so nothing in them says whether this holds for a real hand.
pub const SWIPE_DISTANCE: f32 = 0.25;
/// The seconds a swipe has to cover [`SWIPE_DISTANCE`] in. Unmeasured,
/// for the same reason as [`SWIPE_DISTANCE`].
pub const SWIPE_WINDOW: f64 = 0.60;
/// The seconds after a swipe before the next one can fire. Unmeasured,
/// for the same reason as [`SWIPE_DISTANCE`].
pub const SWIPE_COOLDOWN: f64 = 0.80;
/// The seconds a fist is held before it sends Escape. The recordings
/// hold 34 fist runs, of which two reached two seconds; at 1.5 seconds
/// three reach it and at 1.0 four do, over about two minutes of tracked
/// hand that asked for no Escape at all. Unchanged, because every step
/// down multiplies an Escape nobody asked for.
pub const FIST_HOLD: f64 = 2.0;
/// The seconds a hold survives a frame that reads as something else.
/// Doubling it to 0.50 bridges the gap between two separate fist runs in
/// the recordings and turns three into an Escape rather than two.
/// Unchanged.
pub const HOLD_GRACE: f64 = 0.25;
/// The seconds without a hand after which a held pinch releases.
pub const HAND_LOST: f64 = 0.30;
/// The time constant of the landmark smoothing, in seconds. The smoothing is applied
/// as `1 - exp(-dt / tau)`, so it holds its meaning at any frame rate,
/// which matters because the recordings of 2026-09-18 ran at 30 frames a
/// second twice and 15 once. Replaying them, the label flips fall from
/// 631 with no smoothing to 324 here and 279 at 0.15, while the drawn
/// index tip trails the camera's by 0.018 of the frame here and 0.032 at
/// 0.15. This is the knee: 0.15 buys 45 fewer flips for three quarters
/// more lag. Unchanged.
pub const SMOOTHING_TAU: f32 = 0.09;
/// The fraction of the frame at each edge the pointer does not need: a
/// hand at 15 percent from the frame's edge is at the screen's edge.
///
/// Across the recorded frames that steered, the index tip ran from 0.21
/// to 0.88 of the frame's width, which this maps onto very nearly the
/// whole screen, and 3% of those frames were pinned to an edge against
/// 8% at 0.20. Vertically the same tip ran from 0.15 to 0.53, so the
/// lower half of the screen was out of reach; one constant at both edges
/// of both axes cannot fix that, and widening it only pins more frames.
/// Unchanged, and the vertical reach is recorded in `docs/os/camera-and-hands.md`
/// rather than papered over here.
pub const EDGE: f32 = 0.15;
/// The smallest palm, as a fraction of the frame's height, the rules read.
/// A smaller hand is too far from the camera to be steering the desk.
pub const PALM_MIN: f32 = 0.03;
/// The seconds of tracked hand between two lines of the window log.
pub const LOG_EVERY: f64 = 1.0;
/// The seconds of labels the window log covers.
pub const WINDOW: f64 = 1.0;
/// How far the pointer moves, as a fraction of the screen, before a
/// motion is sent.
pub const POINTER_STEP: f32 = 0.002;

/// How many rules one reading carries: the five `crate::margins`
/// measures and the pointing rule this module adds.
pub const RULE_COUNT: usize = 6;

/// What one frame's hand is doing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Label {
    /// No rule holds, or no hand is in the frame.
    #[default]
    None,
    /// Every finger curled.
    Fist,
    /// The thumb on the index finger.
    Pinch,
    /// The index finger extended and the middle finger curled.
    Point,
    /// Every finger extended, flat or spread.
    Palm,
}

impl Label {
    /// The word the log and the overlay print.
    pub fn word(self) -> &'static str {
        match self {
            Label::None => "none",
            Label::Fist => "fist",
            Label::Pinch => "pinch",
            Label::Point => "point",
            Label::Palm => "palm",
        }
    }
}

/// What one frame's hand reads as: the label, the margin of the rule that
/// decided it, and every rule's margin beside its label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub label: Label,
    /// The deciding rule's margin, or the thinnest of them when no rule
    /// decided.
    pub margin: f32,
    /// Each rule by name with its margin, in the order [`RULES`] names.
    pub rules: [(&'static str, f32); RULE_COUNT],
}

impl Reading {
    /// The reading of a frame with no hand, or one the rules cannot read.
    pub fn none() -> Reading {
        Reading {
            label: Label::None,
            margin: 0.0,
            rules: RULES.map(|label| (label, 0.0)),
        }
    }

    /// Every rule with its margin, as the log writes them.
    pub fn text(&self) -> String {
        self.rules
            .iter()
            .map(|(label, margin)| format!("{label} {margin:+.2}"))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// The rules a reading carries, in order: the five the pose set measures
/// and the pointing rule this module adds.
pub const RULES: [&str; RULE_COUNT] = [
    "pinch",
    "fist",
    "two finger v",
    "open hand",
    "flat hand",
    "point",
];

/// The pinch rule's place in a reading, which the hysteresis reads.
pub const PINCH_RULE: usize = 0;

/// The pointing rule's margin: the thinner of an extended index finger and
/// a curled middle finger, each measured in palms. `None` when the hand is
/// too small to read, which is the palm the pose rules also refuse.
pub fn point_margin(hand: &[Landmark; 21], aspect: f32) -> Option<f32> {
    let gap = |a: usize, b: usize| {
        let dx = (hand[a].x - hand[b].x) * aspect;
        let dy = hand[a].y - hand[b].y;
        (dx * dx + dy * dy).sqrt()
    };
    let palm = gap(WRIST, MIDDLE_MCP);
    if palm < PALM_MIN {
        return None;
    }
    let extended = (gap(INDEX_TIP, WRIST) / palm - POINT_EXTENDED) / POINT_EXTENDED;
    let curled = (POINT_CURLED - gap(MIDDLE_TIP, WRIST) / palm) / POINT_CURLED;
    Some(extended.min(curled))
}

/// What one hand reads as. The pose comes from `crate::recognize`,
/// which reads a pinch before a fist and a fist before the open poses, and
/// the pointing rule answers for the hands that set has no label for.
pub fn classify(hand: &[Landmark; 21], aspect: f32) -> Reading {
    let Some(poses) = margins(hand) else {
        return Reading::none();
    };
    let point = point_margin(hand, aspect);
    let Some(point) = point else {
        return Reading::none();
    };
    let rules = rules_of(&poses, point);
    let (label, margin) = match recognize(hand) {
        HandPose::PinchClosed => (Label::Pinch, poses.pinch),
        HandPose::Fist => (Label::Fist, poses.fist),
        HandPose::OpenHand => (Label::Palm, poses.open_hand),
        HandPose::FlatHand => (Label::Palm, poses.flat_hand),
        HandPose::TwoFingerV | HandPose::None if point >= 0.0 => (Label::Point, point),
        _ => {
            let (_, thinnest) = poses.thinnest();
            (Label::None, thinnest)
        }
    };
    Reading {
        label,
        margin,
        rules,
    }
}

/// Every rule by name with its margin, in the order [`RULES`] names.
fn rules_of(poses: &Margins, point: f32) -> [(&'static str, f32); RULE_COUNT] {
    [
        (RULES[0], poses.pinch),
        (RULES[1], poses.fist),
        (RULES[2], poses.two_finger_v),
        (RULES[3], poses.open_hand),
        (RULES[4], poses.flat_hand),
        (RULES[5], point),
    ]
}

/// What one frame asks the desk for. Positions are fractions of the
/// focused screen, mirrored and with the edge zone removed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Act {
    /// Move the pointer.
    Point(f32, f32),
    /// Press the left button, with the pointer here.
    Press(f32, f32),
    /// Move the pointer with the button held.
    Drag(f32, f32),
    /// Release the left button.
    Release,
    /// Switch desks: `Left` for the desk after this one, `Right` for the
    /// one before it, so the desks follow the hand.
    Swipe(Dir),
    /// Send Escape to the focused window.
    Escape,
}

impl Act {
    /// The word the log prints for the act.
    pub fn word(&self) -> &'static str {
        match self {
            Act::Point(..) => "point",
            Act::Press(..) => "press",
            Act::Drag(..) => "drag",
            Act::Release => "release",
            Act::Swipe(Dir::Left) => "swipe left",
            Act::Swipe(Dir::Right) => "swipe right",
            Act::Swipe(_) => "swipe",
            Act::Escape => "escape",
        }
    }
}

/// What one frame produced: the acts, the reading for the overlay, and any
/// line for the log.
#[derive(Clone, Debug, Default)]
pub struct Step {
    pub acts: Vec<Act>,
    pub label: Label,
    pub margin: f32,
    /// The smoothed hand the overlay draws, when one is tracked.
    pub hand: Option<[Landmark; 21]>,
    /// Lines for the log: each act as it happened, and the window's label
    /// sequence once a second.
    pub log: Vec<String>,
}

/// A pinch in progress.
#[derive(Clone, Copy, Debug)]
struct Held {
    /// Where the pinch's midpoint was when it pressed.
    origin: (f32, f32),
    /// Where the pointer was when it pressed.
    pointer: (f32, f32),
}

/// The state machine over frames.
#[derive(Debug)]
pub struct Gestures {
    aspect: f32,
    smoothed: Option<[Landmark; 21]>,
    last_seen: Option<f64>,
    last_frame: Option<f64>,
    pointer: Option<(f32, f32)>,
    sent: Option<(f32, f32)>,
    held: Option<Held>,
    fist_since: Option<f64>,
    fist_last: f64,
    fist_fired: bool,
    palm_track: VecDeque<(f64, f32)>,
    swipe_until: f64,
    window: VecDeque<(f64, Label, f32)>,
    last_log: Option<f64>,
    /// Frames the daemon's lane dropped since the last window line, which
    /// are frames no rule read.
    dropped: u64,
}

impl Gestures {
    /// A machine over frames whose width is `aspect` times their height.
    pub fn new(aspect: f32) -> Gestures {
        Gestures {
            aspect: if aspect.is_finite() && aspect > 0.0 {
                aspect
            } else {
                16.0 / 9.0
            },
            smoothed: None,
            last_seen: None,
            last_frame: None,
            pointer: None,
            sent: None,
            held: None,
            fist_since: None,
            fist_last: 0.0,
            fist_fired: false,
            palm_track: VecDeque::new(),
            swipe_until: 0.0,
            window: VecDeque::new(),
            last_log: None,
            dropped: 0,
        }
    }

    /// Whether a pinch is held.
    pub fn holding(&self) -> bool {
        self.held.is_some()
    }

    /// Where the hand last aimed, as fractions of the focused screen, and
    /// nothing before a hand has pointed. The Jev seam's ask carries it,
    /// so an answer that acts acts where the window pointed rather than
    /// where the hand has since moved.
    pub fn pointer(&self) -> Option<(f32, f32)> {
        self.pointer
    }

    /// Forgets the hand, which is what turning tracking off does. A held
    /// pinch releases.
    pub fn reset(&mut self) -> Vec<Act> {
        let acts = if self.held.take().is_some() {
            vec![Act::Release]
        } else {
            Vec::new()
        };
        self.smoothed = None;
        self.last_seen = None;
        self.last_frame = None;
        self.pointer = None;
        self.sent = None;
        self.fist_since = None;
        self.fist_fired = false;
        self.palm_track.clear();
        self.window.clear();
        self.last_log = None;
        self.dropped = 0;
        acts
    }

    /// Reads one line off the socket.
    pub fn feed(&mut self, line: &Line) -> Step {
        let t = line.timestamp;
        let dt = self
            .last_frame
            .map(|last| (t - last).clamp(0.0, 1.0) as f32)
            .unwrap_or(1.0);
        self.last_frame = Some(t);
        self.dropped += line.dropped;
        let mut step = Step::default();
        let Some(raw) = self.pick(&line.hands) else {
            return self.no_hand(t, step);
        };
        self.last_seen = Some(t);
        let hand = self.smooth(raw, dt);
        let reading = classify(&hand, self.aspect);
        step.hand = Some(hand);
        step.label = reading.label;
        step.margin = reading.margin;
        self.remember(t, &reading, &mut step);
        self.pointer_from(&hand, reading.label);
        self.pinch(&hand, &reading, &mut step);
        self.swipe(t, &hand, reading.label, &mut step);
        self.fist(t, reading.label, reading.margin, &mut step);
        if self.held.is_none()
            && reading.label == Label::Point
            && let Some(at) = self.pointer
            && self.moved(at)
        {
            self.sent = Some(at);
            step.acts.push(Act::Point(at.0, at.1));
        }
        step
    }

    /// A frame with no hand: a held pinch lets go once the hand has been
    /// gone long enough, and nothing else happens.
    fn no_hand(&mut self, t: f64, mut step: Step) -> Step {
        let gone = self.last_seen.is_none_or(|seen| t - seen >= HAND_LOST);
        if gone {
            if self.held.take().is_some() {
                step.acts.push(Act::Release);
                step.log
                    .push("hands: release, the hand left the frame".to_string());
            }
            self.smoothed = None;
            self.pointer = None;
            self.sent = None;
            self.fist_since = None;
            self.fist_fired = false;
            self.palm_track.clear();
        }
        step
    }

    /// The hand to follow: the one nearest the hand followed so far, or
    /// the first.
    fn pick(&self, hands: &[WireHand]) -> Option<[Landmark; 21]> {
        let landmarks = |hand: &WireHand| -> Option<[Landmark; 21]> {
            if hand.landmarks.len() < 21 {
                return None;
            }
            let mut out = [Landmark::default(); 21];
            for (slot, point) in out.iter_mut().zip(hand.landmarks.iter()) {
                *slot = Landmark::from(*point);
            }
            Some(out)
        };
        let candidates: Vec<[Landmark; 21]> = hands.iter().filter_map(landmarks).collect();
        match self.smoothed {
            Some(followed) => candidates.into_iter().min_by(|a, b| {
                let da = wrist_gap(a, &followed);
                let db = wrist_gap(b, &followed);
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            }),
            None => candidates.into_iter().next(),
        }
    }

    /// The hand after exponential smoothing toward the new reading.
    fn smooth(&mut self, raw: [Landmark; 21], dt: f32) -> [Landmark; 21] {
        let alpha = 1.0 - (-dt / SMOOTHING_TAU.max(0.001)).exp();
        let hand = match self.smoothed {
            None => raw,
            Some(mut held) => {
                for (slot, point) in held.iter_mut().zip(raw.iter()) {
                    slot.x += (point.x - slot.x) * alpha;
                    slot.y += (point.y - slot.y) * alpha;
                    slot.z += (point.z - slot.z) * alpha;
                }
                held
            }
        };
        self.smoothed = Some(hand);
        hand
    }

    /// Whether the pointer moved far enough to send another motion.
    fn moved(&self, at: (f32, f32)) -> bool {
        self.sent
            .is_none_or(|sent| (sent.0 - at.0).abs().max((sent.1 - at.1).abs()) >= POINTER_STEP)
    }

    /// Keeps the last second of labels and writes the window to the log
    /// once a second of tracked hand, with every rule's margin beside it.
    fn remember(&mut self, t: f64, reading: &Reading, step: &mut Step) {
        self.window.push_back((t, reading.label, reading.margin));
        while self
            .window
            .front()
            .is_some_and(|(first, _, _)| t - first > WINDOW)
        {
            self.window.pop_front();
        }
        let due = self.last_log.is_none_or(|last| t - last >= LOG_EVERY);
        if due {
            self.last_log = Some(t);
            let dropped = match self.dropped {
                0 => String::new(),
                count => format!("; {count} frame(s) dropped"),
            };
            self.dropped = 0;
            step.log.push(format!(
                "hands window: {}; rules {}{dropped}",
                self.window_text(),
                reading.text()
            ));
        }
    }

    /// The window's label sequence, each run with its margins, such as
    /// `point +0.31..+0.42 x12, pinch +0.05 x1`.
    pub fn window_text(&self) -> String {
        let mut runs: Vec<(Label, f32, f32, usize)> = Vec::new();
        for (_, label, margin) in &self.window {
            match runs.last_mut() {
                Some(run) if run.0 == *label => {
                    run.1 = run.1.min(*margin);
                    run.2 = run.2.max(*margin);
                    run.3 += 1;
                }
                _ => runs.push((*label, *margin, *margin, 1)),
            }
        }
        if runs.is_empty() {
            return "no hand".to_string();
        }
        runs.iter()
            .map(|(label, low, high, count)| {
                if (high - low).abs() < 0.005 {
                    format!("{} {:+.2} x{count}", label.word(), low)
                } else {
                    format!("{} {:+.2}..{:+.2} x{count}", label.word(), low, high)
                }
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Where the index tip puts the pointer: mirrored, with the edge zone
    /// removed, and held at the screen's edge.
    fn pointer_from(&mut self, hand: &[Landmark; 21], label: Label) {
        if self.held.is_some() {
            return;
        }
        if label == Label::Point || label == Label::Pinch {
            self.pointer = Some(to_screen(tip(hand, INDEX_TIP)));
        }
    }

    /// The press, the drag, and the release. A pinch presses on the frame
    /// the pose rule reads it, and lets go once the rule's margin is past
    /// [`PINCH_RELEASE`] on the other side, so a reading that flickers at
    /// the threshold keeps the button down.
    ///
    /// The margin is the only thing that lets go. A fist used to let go
    /// as well, and on the recordings of 2026-09-18 that was half of
    /// every release: a pinched hand has its four fingers curled, so one
    /// frame where the thumb drifts past the bound reads as a fist and
    /// the hysteresis never gets to hold the button down. A hand that is
    /// really a fist opens past the release bound in 71% of the recorded
    /// fist readings, so the margin lets go of one within a frame or two
    /// anyway.
    fn pinch(&mut self, hand: &[Landmark; 21], reading: &Reading, step: &mut Step) {
        let pinch = reading.rules[PINCH_RULE];
        match self.held {
            None => {
                if reading.label == Label::Pinch {
                    let at = self
                        .pointer
                        .unwrap_or_else(|| to_screen(tip(hand, INDEX_TIP)));
                    self.held = Some(Held {
                        origin: midpoint(hand),
                        pointer: at,
                    });
                    self.pointer = Some(at);
                    self.sent = Some(at);
                    step.acts.push(Act::Press(at.0, at.1));
                    step.log.push(format!(
                        "hands: press at ({:.3}, {:.3}), {} {:+.2}",
                        at.0, at.1, pinch.0, pinch.1
                    ));
                }
            }
            Some(held) => {
                if pinch.1 <= -PINCH_RELEASE {
                    self.held = None;
                    step.acts.push(Act::Release);
                    step.log
                        .push(format!("hands: release, {} {:+.2}", pinch.0, pinch.1));
                    return;
                }
                let mid = midpoint(hand);
                let (dx, dy) = (mid.0 - held.origin.0, mid.1 - held.origin.1);
                let span = 1.0 - 2.0 * EDGE;
                let at = (
                    (held.pointer.0 - dx / span).clamp(0.0, 1.0),
                    (held.pointer.1 + dy / span).clamp(0.0, 1.0),
                );
                self.pointer = Some(at);
                if self.moved(at) {
                    self.sent = Some(at);
                    step.acts.push(Act::Drag(at.0, at.1));
                }
            }
        }
    }

    /// The desk switch: a flat palm that crosses [`SWIPE_DISTANCE`] of the
    /// frame inside [`SWIPE_WINDOW`], and then waits out the cooldown.
    fn swipe(&mut self, t: f64, hand: &[Landmark; 21], label: Label, step: &mut Step) {
        if label != Label::Palm {
            self.palm_track.clear();
            return;
        }
        let x = 1.0 - hand[MIDDLE_MCP].x;
        self.palm_track.push_back((t, x));
        while self
            .palm_track
            .front()
            .is_some_and(|(first, _)| t - first > SWIPE_WINDOW)
        {
            self.palm_track.pop_front();
        }
        if t < self.swipe_until {
            return;
        }
        let Some((_, from)) = self.palm_track.front().copied() else {
            return;
        };
        let moved = x - from;
        if moved.abs() >= SWIPE_DISTANCE {
            let dir = if moved < 0.0 { Dir::Left } else { Dir::Right };
            let margin = (moved.abs() - SWIPE_DISTANCE) / SWIPE_DISTANCE;
            self.swipe_until = t + SWIPE_COOLDOWN;
            self.palm_track.clear();
            step.acts.push(Act::Swipe(dir));
            step.log.push(format!(
                "hands: {}, palm moved {:+.2} of the frame, margin {:+.2}",
                Act::Swipe(dir).word(),
                moved,
                margin
            ));
        }
    }

    /// Escape: a fist held [`FIST_HOLD`] seconds, once a hold.
    fn fist(&mut self, t: f64, label: Label, margin: f32, step: &mut Step) {
        if label == Label::Fist {
            self.fist_last = t;
            let since = *self.fist_since.get_or_insert(t);
            if !self.fist_fired && t - since >= FIST_HOLD {
                self.fist_fired = true;
                step.acts.push(Act::Escape);
                step.log.push(format!(
                    "hands: escape, fist held {:.1}s, margin {margin:+.2}",
                    t - since
                ));
            }
        } else if self.fist_since.is_some() && t - self.fist_last > HOLD_GRACE {
            self.fist_since = None;
            self.fist_fired = false;
        }
    }
}

/// One landmark as a point in frame coordinates.
fn tip(hand: &[Landmark; 21], joint: usize) -> (f32, f32) {
    (hand[joint].x, hand[joint].y)
}

/// The midpoint of the thumb tip and the index tip, which is where a pinch
/// holds the desk.
fn midpoint(hand: &[Landmark; 21]) -> (f32, f32) {
    (
        (hand[THUMB_TIP].x + hand[INDEX_TIP].x) * 0.5,
        (hand[THUMB_TIP].y + hand[INDEX_TIP].y) * 0.5,
    )
}

/// A frame point as a fraction of the screen: mirrored, with the edge
/// zone removed, and held to the screen.
pub fn to_screen(at: (f32, f32)) -> (f32, f32) {
    let span = 1.0 - 2.0 * EDGE;
    (
        ((1.0 - at.0 - EDGE) / span).clamp(0.0, 1.0),
        ((at.1 - EDGE) / span).clamp(0.0, 1.0),
    )
}

fn wrist_gap(a: &[Landmark; 21], b: &[Landmark; 21]) -> f32 {
    let dx = a[WRIST].x - b[WRIST].x;
    let dy = a[WRIST].y - b[WRIST].y;
    dx * dx + dy * dy
}

/// The aspect ratio `CODEROS_CAMERA_CAPTURE` names, such as `1280x720`,
/// or the camera's default.
pub fn aspect_from(value: Option<String>) -> f32 {
    let parsed = value.and_then(|text| {
        let (w, h) = text.trim().split_once('x')?;
        let w: f32 = w.parse().ok()?;
        let h: f32 = h.parse().ok()?;
        (w > 0.0 && h > 0.0).then_some(w / h)
    });
    parsed.unwrap_or(16.0 / 9.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn corpus() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/golden/hands")
    }

    /// The acts of a run with their repeats collapsed, as the sidecar
    /// writes them: a run of pointer moves is one `point`.
    fn collapsed(acts: &[Act]) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for act in acts {
            let word = act.word().to_string();
            if out.last() != Some(&word) {
                out.push(word);
            }
        }
        out
    }

    #[derive(serde::Deserialize)]
    struct Expect {
        why: String,
        #[serde(default)]
        synthetic: bool,
        expect: Vec<String>,
        #[serde(default)]
        margin_under: Option<f32>,
    }

    #[test]
    fn the_corpus_produces_the_acts_each_case_owes() {
        let dir = corpus();
        let mut names: Vec<PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("{}: {err}", dir.display()))
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .collect();
        names.sort();
        assert!(!names.is_empty(), "no cases under {}", dir.display());
        let mut failures = Vec::new();
        let mut synthetic = 0;
        for path in &names {
            let sidecar = path.with_extension("expect.json");
            let expect: Expect = serde_json::from_str(
                &std::fs::read_to_string(&sidecar)
                    .unwrap_or_else(|err| panic!("{}: {err}", sidecar.display())),
            )
            .unwrap_or_else(|err| panic!("{}: {err}", sidecar.display()));
            synthetic += usize::from(expect.synthetic);
            let text = std::fs::read_to_string(path).expect("the case reads");
            let mut machine = Gestures::new(16.0 / 9.0);
            let mut acts = Vec::new();
            let mut thinnest = f32::MAX;
            for (number, row) in text
                .lines()
                .enumerate()
                .filter(|(_, row)| !row.trim().is_empty())
            {
                let line = Line::parse(row)
                    .unwrap_or_else(|err| panic!("{} row {}: {err}", path.display(), number + 1));
                let step = machine.feed(&line);
                if step.label != Label::None {
                    thinnest = thinnest.min(step.margin);
                }
                acts.extend(step.acts);
            }
            let got = collapsed(&acts);
            if got != expect.expect {
                failures.push(format!(
                    "{}: {}\n  expected {:?}\n  got      {:?}",
                    path.display(),
                    expect.why,
                    expect.expect,
                    got
                ));
            }
            if let Some(bound) = expect.margin_under
                && thinnest >= bound
            {
                failures.push(format!(
                    "{}: the thinnest margin was {thinnest:+.2}, not under {bound:+.2}",
                    path.display()
                ));
            }
        }
        assert!(failures.is_empty(), "{}", failures.join("\n"));
        assert!(synthetic < names.len(), "at least one case is recorded");
    }

    #[test]
    fn a_stream_with_no_hand_produces_no_input() {
        let mut machine = Gestures::new(16.0 / 9.0);
        for frame in 0..90 {
            let line = Line {
                timestamp: 1.0 + frame as f64 / 30.0,
                hands: Vec::new(),
                status: "No hands detected".into(),
                dropped: 0,
            };
            let step = machine.feed(&line);
            assert!(step.acts.is_empty(), "frame {frame}: {:?}", step.acts);
            assert!(step.log.is_empty(), "frame {frame}: {:?}", step.log);
            assert_eq!(step.label, Label::None);
        }
    }

    #[test]
    fn a_reading_carries_every_rule_with_its_margin() {
        let reading = Reading::none();
        assert_eq!(reading.rules.len(), RULE_COUNT);
        assert_eq!(reading.rules[PINCH_RULE].0, "pinch");
        assert_eq!(reading.rules[RULE_COUNT - 1].0, "point");
        assert!(reading.rules.iter().all(|(_, margin)| *margin == 0.0));
        assert!(reading.text().contains("pinch +0.00"), "{}", reading.text());
    }

    #[test]
    fn the_pointer_is_mirrored_and_the_edge_zone_reaches_the_screens_edge() {
        assert_eq!(to_screen((EDGE, EDGE)), (1.0, 0.0));
        assert_eq!(to_screen((1.0 - EDGE, 1.0 - EDGE)), (0.0, 1.0));
        let (x, y) = to_screen((0.5, 0.5));
        assert!((x - 0.5).abs() < 1e-5 && (y - 0.5).abs() < 1e-5);
        assert_eq!(to_screen((0.0, 0.0)), (1.0, 0.0));
    }

    /// One frame of a case, as the machine smooths it: the first row
    /// stands for itself, since smoothing starts at what it reads.
    fn first_hand(case: &str) -> [Landmark; 21] {
        let path = corpus().join(format!("{case}.jsonl"));
        let text = std::fs::read_to_string(&path).unwrap_or_else(|err| panic!("{case}: {err}"));
        let row = text
            .lines()
            .find(|row| !row.trim().is_empty())
            .expect("a row");
        let line = Line::parse(row).expect("the row parses");
        let mut machine = Gestures::new(16.0 / 9.0);
        machine.feed(&line).hand.expect("the row carries a hand")
    }

    #[test]
    fn a_pointing_hand_reads_as_a_point_and_a_pinched_one_as_a_pinch() {
        let pointing = classify(&first_hand("point-moves-the-pointer"), 16.0 / 9.0);
        assert_eq!(pointing.label, Label::Point);
        assert!(pointing.margin > 0.0, "{}", pointing.text());
        assert!(
            pointing.rules[PINCH_RULE].1 < 0.0,
            "a pointing hand is no pinch: {}",
            pointing.text()
        );
        let fist = classify(&first_hand("fist-held-sends-escape"), 16.0 / 9.0);
        assert_eq!(fist.label, Label::Fist);
        let palm = classify(&first_hand("palm-swipes-a-desk"), 16.0 / 9.0);
        assert_eq!(palm.label, Label::Palm);
    }

    #[test]
    fn a_pinch_that_flickers_at_the_threshold_keeps_the_button_down() {
        let closed = first_hand("pinch-presses-and-drags");
        let mut machine = Gestures::new(16.0 / 9.0);
        // A hand whose pinch rule sits just the wrong side of zero is
        // inside the hysteresis, so a press through it stays pressed.
        let reading = Reading {
            label: Label::Point,
            margin: -0.01,
            rules: RULES.map(|label| (label, -0.01)),
        };
        let mut step = Step::default();
        machine.held = Some(Held {
            origin: midpoint(&closed),
            pointer: (0.5, 0.5),
        });
        machine.pinch(&closed, &reading, &mut step);
        assert!(machine.holding(), "{:?}", step.acts);
        assert!(!step.acts.contains(&Act::Release), "{:?}", step.acts);
        let open = Reading {
            margin: -PINCH_RELEASE - 0.01,
            rules: RULES.map(|label| (label, -PINCH_RELEASE - 0.01)),
            ..reading
        };
        machine.pinch(&closed, &open, &mut step);
        assert!(!machine.holding());
        assert_eq!(step.acts.last(), Some(&Act::Release));
    }

    #[test]
    fn the_capture_size_gives_the_aspect_and_a_bad_one_the_default() {
        assert!((aspect_from(Some("1280x720".into())) - 16.0 / 9.0).abs() < 1e-5);
        assert!((aspect_from(Some("640x480".into())) - 4.0 / 3.0).abs() < 1e-5);
        assert!((aspect_from(Some("wide".into())) - 16.0 / 9.0).abs() < 1e-5);
        assert!((aspect_from(None) - 16.0 / 9.0).abs() < 1e-5);
    }
}
