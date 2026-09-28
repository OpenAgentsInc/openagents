//! The Jev seam beside the pose rules: a typed judgment over the windows
//! the deterministic classifier cannot settle, designed in
//! `docs/os/hands-judge.md`.
//!
//! The per-frame path stays in code: [`crate::recognize`], smoothing,
//! hysteresis, and a pointer that never waits on a network call. Jev
//! never enters the frame loop. When a transition's margin is thin, or a
//! window's labels keep flipping, the caller sends this module's state
//! and questions on a thread of its own under [`DEADLINE`]. Until an
//! answer arrives the rules' result stands, which for an ambiguous
//! window is [`crate::HandPose::None`]: Jev can turn a dropped gesture
//! into an acted one, and never the reverse except through `addressed`,
//! which only ever drops.
//!
//! The module lives in this crate, beside the rules it judges and the
//! [`crate::Frame`] type it reads. The caller is [`crate::watch`], which
//! `crates/coder-compositor` runs: it pushes each frame into
//! [`Window`], asks on each [`Window::ambiguous`] transition, runs
//! [`ask`] on the thread [`crate::seam`] owns, applies [`Judge::decide`]
//! to the answer, and writes a [`Record`] to the compositor's
//! transcript.

use std::collections::VecDeque;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::pose::{self, Margins};
use crate::{Frame, Hand, INDEX_MCP, INDEX_TIP, MIDDLE_MCP, MIDDLE_TIP};
use crate::{PINKY_MCP, PINKY_TIP, RING_MCP, RING_TIP};
use crate::{THUMB_MCP, THUMB_TIP, WRIST};

/// The seconds of hand a request reads. A window is sized in time rather
/// than in frames because the camera's rate is not a constant: the
/// recordings of 2026-09-18 ran at 30 frames a second twice and 15 once,
/// and the CoderOS daemon reports 15 to 16 in the room it sits in. A
/// fixed count of 30 frames is one second at the first rate and two at
/// the second.
///
/// One second is the span to read. Every labelled gesture run in those
/// recordings was over inside 0.87 seconds at the ninety-fifth
/// percentile, so a one-second window holds a whole gesture almost
/// always; it is the span the rules' own log writes
/// (`gestures::WINDOW`); and it is [`DEADLINE`], so the window a request
/// reads and the time it has to answer are the same length. Two seconds
/// is not: over the recordings a 2.04-second window covered more than
/// one gesture in 67% to 97% of its positions, against 55% to 78% at one
/// second.
pub const WINDOW_SECONDS: f64 = 1.0;

/// The rate a window assumes until a caller measures one, in frames a
/// second. The CoderOS camera daemon reports this.
pub const NOMINAL_RATE: f64 = 15.0;

/// The fewest frames a window holds, however slow the camera answers.
pub const WINDOW_FRAMES_MIN: usize = 8;

/// The most frames a window holds, however fast it answers. A window
/// this long is still [`WINDOW_SECONDS`] at 45 frames a second.
pub const WINDOW_FRAMES_MAX: usize = 45;

/// How many frames [`WINDOW_SECONDS`] is at `rate` frames a second,
/// within the two bounds. A rate that is not a positive number falls
/// back to [`NOMINAL_RATE`].
///
/// It counts up rather than rounding a product, because the bounds are
/// small and a float that converts to an integer needs an exception the
/// crate does not otherwise take.
#[must_use]
pub fn window_frames(rate: f64) -> usize {
    let rate = if rate.is_finite() && rate > 0.0 {
        rate
    } else {
        NOMINAL_RATE
    };
    let wanted = rate * WINDOW_SECONDS;
    let mut frames = WINDOW_FRAMES_MIN;
    let mut counted = MIN_COUNTED;
    while frames < WINDOW_FRAMES_MAX && counted + 0.5 < wanted {
        frames += 1;
        counted += 1.0;
    }
    frames
}

/// [`WINDOW_FRAMES_MIN`] as a number to count up from. A test holds the
/// two spellings together.
const MIN_COUNTED: f64 = 8.0;

/// The deadline a request runs under: one window. An answer that lands
/// after it is logged and discarded, because the desk has moved on.
pub const DEADLINE: Duration = Duration::from_millis(1_000);

/// A rule margin, in units of the constant it wraps, under which a
/// transition is ambiguous rather than clear.
///
/// Still a starting point. What the recordings of 2026-09-18 measure is
/// what it costs: 32% to 50% of frames with a hand fall inside it, and
/// `coder-hands-measure score` prints the ask rate at a sweep of it on
/// every run. What they cannot measure is what it buys, because no line
/// of them says what the hand meant and no answer has been kept beside
/// one. Moving it trades cost against coverage, and only the coverage
/// half is missing, so it has not moved.
pub const THIN_MARGIN: f32 = 0.15;

/// Label flips inside one window past which the window is ambiguous.
pub const FLICKER: usize = 1;

/// Windows of hand between one ask and the next.
///
/// A request reads [`WINDOW_SECONDS`] of hand, so two asks less than a
/// window apart read mostly the same frames and carry nearly the same
/// state. The second one buys almost nothing and costs a whole request,
/// so the trigger holds it back and the record says it did.
///
/// One window caps the ask rate at one a second however fast a label
/// flickers. Without the cap the trigger fired 1.1 to 3.4 times a second
/// over the recordings of 2026-09-18, against one request in flight
/// under a one-second deadline, so most of what it asked was dropped
/// before it was answered.
pub const ASK_SPACING: usize = 1;

/// The integration map's default floor for a verification seam: the top
/// intent acts only at or above this probability.
pub const ACT_FLOOR: f64 = 0.60;

/// `press` carries a higher floor because a wrong press costs more than
/// a wrong pointer move.
pub const PRESS_FLOOR: f64 = 0.75;

/// `escape` carries the press floor for the same reason.
pub const ESCAPE_FLOOR: f64 = 0.75;

/// `addressed` below this floor drops the window, whatever `intent`
/// says.
pub const ADDRESSED_FLOOR: f64 = 0.50;

/// `continuing` at or above this floor holds the running gesture across
/// the window.
pub const CONTINUING_FLOOR: f64 = 0.50;

/// The overlay text when the seam is off or Jev is unreachable. It
/// names no model.
pub const RULES_ONLY: &str = "hands: rules only";

/// The id of the intent Choice.
pub const INTENT: &str = "intent";

/// The id of the addressed Noul.
pub const ADDRESSED: &str = "addressed";

/// The id of the continuing Noul.
pub const CONTINUING: &str = "continuing";

/// The instructions and criteria the request carries.
///
/// This is the selection policy the owner reviews. It sits in one place
/// rather than beside the code that sends it, because the text is the
/// parameter that gets tuned and the code around it is not.
pub mod policy {
    /// `intent`: what the hand is doing in this window.
    pub const INTENT: &str = "The state is a one-second window of a tracked hand over a desk: per frame, the pose the deterministic rules saw, the pinch distance and each finger's extension as ratios of palm size, which way the palm faces, and the index fingertip's velocity. `previous` names the last gesture the desk acted on and how many frames ago it ended. What is the hand doing in this window?";

    /// `point`: aim at a spot.
    pub const POINT: &str = "The hand aims at a spot on the screen: index finger extended toward the display, the others folded, the tip holding still or drifting toward a target. Not a pinch closing, not a sideways sweep.";

    /// `press`: activate what is under the pointer.
    pub const PRESS: &str = "The hand activates what the pointer is over: a pinch closing or a push forward, a deliberate contact. Not an incidental brush of thumb and finger while the hand rests.";

    /// `release`: let go of a held press or drag.
    pub const RELEASE: &str = "The hand lets go of a press or a drag it was holding: a pinch opening, fingers relaxing. Not a gesture starting.";

    /// `swipe_left`: sweep to the desk on the left.
    pub const SWIPE_LEFT: &str = "The hand sweeps toward screen left to move to the desk on that side: a fast sideways crossing, not a slow drift while pointing.";

    /// `swipe_right`: sweep to the desk on the right.
    pub const SWIPE_RIGHT: &str = "The hand sweeps toward screen right to move to the desk on that side: a fast sideways crossing, not a slow drift while pointing.";

    /// `escape`: dismiss what is up.
    pub const ESCAPE: &str = "The hand dismisses what is up: a flick away or a closed hand snapping open, backing out rather than choosing.";

    /// `rest`: deliberately idle, still addressed to the desk.
    pub const REST: &str = "The hand is deliberately still beside the desk: it stays in view but is not operating anything. Not a hand mid-gesture.";

    /// `none`: doing something off the desk.
    pub const NONE: &str = "The hand is doing something off the desk: scratching, holding a mug, waving while the person talks. Choose this rather than the nearest gesture.";

    /// `addressed`: is this input for the desk at all.
    pub const ADDRESSED: &str = "Is the hand in `frames` deliberately operating the screen?";

    /// What a yes to `addressed` means.
    pub const ADDRESSED_TRUE: &str = "The hand's shapes and motion are directed at the desk: held toward the display, paced like an input.";

    /// What a no to `addressed` means.
    pub const ADDRESSED_FALSE: &str = "The hand is on some other errand and its shapes are incidental: near the body, off-axis, or moving while the person talks.";

    /// `continuing`: same gesture still held.
    pub const CONTINUING: &str = "Is the gesture in this window the same one `previous.gesture` names, still held rather than ended and restarted?";

    /// What a yes to `continuing` means.
    pub const CONTINUING_TRUE: &str = "The pose flickers across a threshold but the held shape and its drift continue the earlier gesture.";

    /// What a no to `continuing` means.
    pub const CONTINUING_FALSE: &str =
        "The earlier gesture ended, and this window is a new motion or no motion.";
}

/// One frame's features as the state carries them: palm-relative, so the
/// window reads the same at any distance from the camera. No raw
/// coordinates and no image leave the machine.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Features {
    /// The label the rules gave this frame.
    pub pose: String,
    /// Thumb-tip to index-tip distance, in palm lengths.
    pub pinch: f32,
    /// Each finger's knuckle-to-tip length, in palm lengths, in
    /// thumb-index-middle-ring-pinky order.
    pub extension: [f32; 5],
    /// The palm normal's depth component, normalized by palm size:
    /// positive when a right palm faces the camera. A left hand mirrors.
    pub facing: f32,
    /// The index tip's velocity, in normalized image units per frame.
    pub tip_velocity: [f32; 2],
    /// Where the index tip sits in the frame.
    pub x: f32,
    /// Where the index tip sits in the frame.
    pub y: f32,
}

/// The gesture the desk last acted on, and how long ago it ended.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Previous {
    /// The action's name, from [`Action::label`].
    pub gesture: String,
    /// Frames since it ended.
    pub ended_frames: usize,
}

/// What the trigger reads to tell a window it asks about from one it
/// does not. [`Trigger::default`] carries the constants above, which is
/// what the desk runs; a scorer that sweeps one builds its own and reads
/// the same code, so the trigger stays in one place while its numbers
/// are measured (`crates/coder-hands-measure`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trigger {
    /// The rule margin under which a transition is ambiguous.
    pub thin: f32,
    /// Label flips inside one window past which the window is ambiguous.
    pub flicker: usize,
    /// Windows of hand between one ask and the next. Zero paces nothing,
    /// which is what the trigger did before it was spaced.
    pub spacing: usize,
}

impl Default for Trigger {
    fn default() -> Trigger {
        Trigger {
            thin: THIN_MARGIN,
            flicker: FLICKER,
            spacing: ASK_SPACING,
        }
    }
}

/// What the trigger does with the newest transition.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ask {
    /// The rules settled it, or there was no transition. Nothing is
    /// sent and nothing is recorded.
    Settled,
    /// The rules could not settle it, and the seam asks.
    Now,
    /// The rules could not settle it, and the seam does not ask: it
    /// asked less than [`Trigger::spacing`] window(s) of hand ago, so an
    /// answer over the same frames is already on its way. The record
    /// says so.
    Paced,
}

/// Why an ambiguous window sent no request.
///
/// A skipped window is written to the transcript rather than counted,
/// because a record that holds only the windows that were asked about
/// describes the asks that happened to land.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    /// The trigger held it back: the seam asked less than
    /// [`Trigger::spacing`] window(s) of hand ago.
    Paced,
    /// A request was in flight, and the seam runs
    /// [`crate::seam::IN_FLIGHT`] at a time.
    InFlight,
    /// The seam's thread is gone, so nothing can be asked at all.
    Closed,
}

impl Skip {
    /// The word the record carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Skip::Paced => "paced",
            Skip::InFlight => "in flight",
            Skip::Closed => "closed",
        }
    }
}

/// What one session did with the ambiguous windows it saw. The line the
/// compositor writes when tracking stops names every field, so a session
/// says what it did not ask as well as what it did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// Windows a request went out for.
    pub asked: u64,
    /// Windows the trigger held back.
    pub paced: u64,
    /// Windows that found a request in flight.
    pub in_flight: u64,
    /// Windows that found the seam's thread gone.
    pub closed: u64,
}

impl Counts {
    /// Ambiguous windows no request went out for.
    #[must_use]
    pub fn skipped(&self) -> u64 {
        self.paced + self.in_flight + self.closed
    }

    /// Ambiguous windows the trigger saw.
    #[must_use]
    pub fn ambiguous(&self) -> u64 {
        self.asked + self.skipped()
    }

    /// Counts one skip.
    pub fn skip(&mut self, skip: Skip) {
        match skip {
            Skip::Paced => self.paced += 1,
            Skip::InFlight => self.in_flight += 1,
            Skip::Closed => self.closed += 1,
        }
    }
}

/// One window's entry: the features the state carries and the margins
/// the trigger reads, which never leave the machine.
#[derive(Clone, Debug)]
struct Beat {
    features: Features,
    margins: Option<Margins>,
}

/// The last [`WINDOW_SECONDS`] of hand features, the state a request is
/// built from. How many frames that is depends on the rate the camera
/// answers at, which [`Window::at_rate`] sets.
#[derive(Clone, Debug)]
pub struct Window {
    beats: VecDeque<Beat>,
    previous: Option<Previous>,
    second_hand: bool,
    frames: usize,
    pushed: u64,
    asked: Option<u64>,
}

impl Default for Window {
    fn default() -> Self {
        Self::at_rate(NOMINAL_RATE)
    }
}

impl Window {
    /// An empty window at [`NOMINAL_RATE`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// An empty window holding [`WINDOW_SECONDS`] at `rate` frames a
    /// second.
    #[must_use]
    pub fn at_rate(rate: f64) -> Self {
        Window {
            beats: VecDeque::new(),
            previous: None,
            second_hand: false,
            frames: window_frames(rate),
            pushed: 0,
            asked: None,
        }
    }

    /// Resizes the window for a rate the caller has since measured. The
    /// frames already in it are kept, less any the new size drops.
    pub fn set_rate(&mut self, rate: f64) {
        self.frames = window_frames(rate);
        self.trim();
    }

    /// How many frames this window holds when it is full.
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.frames
    }

    fn trim(&mut self) {
        while self.beats.len() > self.frames {
            self.beats.pop_front();
        }
    }

    /// One tracker frame into the window. A frame with no hands adds no
    /// beat but ages the previous gesture, because the frames pass either
    /// way.
    pub fn push(&mut self, frame: &Frame) {
        if let Some(previous) = &mut self.previous {
            previous.ended_frames += 1;
        }
        self.second_hand = frame.hands.len() > 1;
        let Some(hand) = frame.hands.first() else {
            return;
        };
        let tip = hand.landmarks[INDEX_TIP];
        let velocity = self
            .beats
            .back()
            .map(|beat| [tip.x - beat.features.x, tip.y - beat.features.y])
            .unwrap_or([0.0, 0.0]);
        self.beats.push_back(Beat {
            features: features(hand, velocity),
            margins: pose::margins(&hand.landmarks),
        });
        self.pushed += 1;
        self.trim();
    }

    /// How many frames the window holds. A frame with no hand adds none,
    /// so this is what the request carries rather than how long the
    /// caller has been pushing.
    #[must_use]
    pub fn len(&self) -> usize {
        self.beats.len()
    }

    /// Whether the window holds no frame yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.beats.is_empty()
    }

    /// Forgets every frame and the previous gesture, which is what
    /// turning tracking off does.
    pub fn clear(&mut self) {
        self.beats.clear();
        self.previous = None;
        self.second_hand = false;
        self.pushed = 0;
        self.asked = None;
    }

    /// The gesture the desk just acted on. The caller names it when it
    /// applies an action, so `continuing` has a gesture to compare the
    /// window against.
    pub fn acted(&mut self, gesture: &str) {
        self.previous = Some(Previous {
            gesture: gesture.to_string(),
            ended_frames: 0,
        });
    }

    /// Whether the newest frame changed the label. The trigger asks only
    /// on a transition.
    #[must_use]
    pub fn transitioned(&self) -> bool {
        let mut beats = self.beats.iter().rev();
        let Some(newest) = beats.next() else {
            return false;
        };
        let Some(prior) = beats.next() else {
            return false;
        };
        newest.features.pose != prior.features.pose
    }

    /// How many times the label changed inside the window.
    #[must_use]
    pub fn flips(&self) -> usize {
        self.beats
            .iter()
            .zip(self.beats.iter().skip(1))
            .filter(|(a, b)| a.features.pose != b.features.pose)
            .count()
    }

    /// The margin the newest frame's label was decided on: the margin of
    /// the rule that produced the label, or, for `none`, the margin of
    /// the rule that came closest to firing. `None` when the frame had
    /// fewer than 21 joints.
    #[must_use]
    pub fn margin(&self) -> Option<f32> {
        let beat = self.beats.back()?;
        let margins = beat.margins?;
        Some(match beat.features.pose.as_str() {
            "pinch" => margins.pinch,
            "fist" => margins.fist,
            "two-finger v" => margins.two_finger_v,
            "open hand" => margins.open_hand,
            "flat hand" => margins.flat_hand,
            _ => margins.thinnest().1,
        })
    }

    /// Whether the newest transition is one the rules cannot settle: a
    /// label change decided on a thin margin, or a window whose labels
    /// keep flipping.
    #[must_use]
    pub fn ambiguous(&self) -> bool {
        self.ambiguous_with(Trigger::default())
    }

    /// [`Window::ambiguous`] at a trigger the caller chose, which is
    /// what a scorer sweeping a margin reads.
    #[must_use]
    pub fn ambiguous_with(&self, trigger: Trigger) -> bool {
        if !self.transitioned() {
            return false;
        }
        self.flips() > trigger.flicker || self.margin().is_some_and(|m| m.abs() < trigger.thin)
    }

    /// Whether the seam asks about the newest transition, and when it
    /// does not, why.
    ///
    /// An ambiguous transition asks only when the window has turned over
    /// since the last ask: [`Trigger::spacing`] window(s) of hand have
    /// arrived, so the request reads frames the last one did not. Every
    /// other ambiguous transition answers [`Ask::Paced`], which the
    /// caller records rather than drops.
    ///
    /// It takes `&mut self` because an ask that goes out is what the
    /// next one is spaced from.
    pub fn ask(&mut self, trigger: Trigger) -> Ask {
        if !self.ambiguous_with(trigger) {
            return Ask::Settled;
        }
        let apart = u64::try_from(self.frames.saturating_mul(trigger.spacing)).unwrap_or(u64::MAX);
        if let Some(asked) = self.asked
            && self.pushed.saturating_sub(asked) < apart
        {
            return Ask::Paced;
        }
        self.asked = Some(self.pushed);
        Ask::Now
    }

    /// The state one request evaluates: the window's features, the
    /// previous acted-on gesture, and whether a second hand is present.
    /// A few hundred tokens against the budget.
    #[must_use]
    pub fn state(&self) -> Value {
        json!({
            "frames": self.beats.iter().map(|beat| &beat.features).collect::<Vec<_>>(),
            "previous": self.previous,
            "second_hand": self.second_hand,
        })
    }
}

/// One hand's features for the window: the rule label, the pinch
/// distance and each finger's extension as ratios of palm size, the
/// palm's facing, and the index tip's velocity.
///
/// Every ratio divides by the palm the pinch rule divides by,
/// [`pose::palm_span`], the wrist to the index knuckle. It used to
/// divide by the wrist to the middle knuckle, which over the recordings
/// of 2026-09-18 ran 1.05 times the other at the median and from 0.82 to
/// 1.28 between the fifth and ninety-fifth percentiles. A `pinch` of
/// 0.42 in the state was then a quarter either side of the 0.35 the rule
/// tests, while the question text invites reading the two as one number.
fn features(hand: &Hand, velocity: [f32; 2]) -> Features {
    let landmarks = &hand.landmarks;
    let wrist = landmarks[WRIST];
    let unit = pose::palm_span(landmarks).unwrap_or(1.0);
    let index_mcp = landmarks[INDEX_MCP];
    let pinky_mcp = landmarks[PINKY_MCP];
    let facing = ((index_mcp.x - wrist.x) * (pinky_mcp.y - wrist.y)
        - (index_mcp.y - wrist.y) * (pinky_mcp.x - wrist.x))
        / (unit * unit);
    let tip = landmarks[INDEX_TIP];
    Features {
        pose: hand.pose.label().to_string(),
        pinch: pose::dist(landmarks[THUMB_TIP], tip) / unit,
        extension: [
            pose::dist(landmarks[THUMB_MCP], landmarks[THUMB_TIP]) / unit,
            pose::dist(index_mcp, tip) / unit,
            pose::dist(landmarks[MIDDLE_MCP], landmarks[MIDDLE_TIP]) / unit,
            pose::dist(landmarks[RING_MCP], landmarks[RING_TIP]) / unit,
            pose::dist(landmarks[PINKY_MCP], landmarks[PINKY_TIP]) / unit,
        ],
        facing,
        tip_velocity: velocity,
        x: tip.x,
        y: tip.y,
    }
}

/// The question set one request asks: the `intent` Choice and the
/// `addressed` and `continuing` Nouls, in a single request because the
/// window is small and the questions read the same state.
#[must_use]
pub fn questions() -> jev::Questions {
    let intent = jev::Choice::default()
        .option("point", policy::POINT)
        .option("press", policy::PRESS)
        .option("release", policy::RELEASE)
        .option("swipe_left", policy::SWIPE_LEFT)
        .option("swipe_right", policy::SWIPE_RIGHT)
        .option("escape", policy::ESCAPE)
        .option("rest", policy::REST)
        .option("none", policy::NONE);
    jev::Questions::new()
        .with(INTENT, jev::Choice::new(policy::INTENT, intent.criteria))
        .with(
            ADDRESSED,
            jev::Noul::with_criteria(
                policy::ADDRESSED,
                jev::NoulCriteria::new()
                    .when_true(policy::ADDRESSED_TRUE)
                    .when_false(policy::ADDRESSED_FALSE),
            ),
        )
        .with(
            CONTINUING,
            jev::Noul::with_criteria(
                policy::CONTINUING,
                jev::NoulCriteria::new()
                    .when_true(policy::CONTINUING_TRUE)
                    .when_false(policy::CONTINUING_FALSE),
            ),
        )
}

/// What one request answered: the intent distribution and the two Nouls.
#[derive(Clone, Debug, PartialEq)]
pub struct Report {
    /// The option `intent` picked.
    pub intent: String,
    /// Its confidence, as the API reports it.
    pub confidence: f64,
    /// A probability for each option, in the order the response sent them.
    pub probabilities: Vec<(String, f64)>,
    /// The probability the hand addresses the desk.
    pub addressed: f64,
    /// The probability the window continues the previous gesture.
    pub continuing: f64,
    /// The tokens the request read and wrote, when the API reported them.
    pub usage: jev::Usage,
    /// The request id the API returned.
    pub request_id: Option<String>,
}

/// The part of an answer the compositor's transcript keeps, and the
/// part a recorded run keeps beside it, so a floor that moves is scored
/// against answers that are already in hand.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct ReportView {
    /// The option `intent` picked.
    pub intent: String,
    /// Its confidence.
    pub confidence: f64,
    /// A probability for each option.
    pub probabilities: Vec<(String, f64)>,
    /// The `addressed` Noul's probability.
    pub addressed: f64,
    /// The `continuing` Noul's probability.
    pub continuing: f64,
    /// Tokens in, when the API reported them.
    pub input_tokens: Option<u64>,
    /// Tokens out, when the API reported them.
    pub output_tokens: Option<u64>,
}

impl Report {
    /// The transcript view of this answer.
    #[must_use]
    pub fn view(&self) -> ReportView {
        ReportView {
            intent: self.intent.clone(),
            confidence: self.confidence,
            probabilities: self.probabilities.clone(),
            addressed: self.addressed,
            continuing: self.continuing,
            input_tokens: self.usage.input_tokens,
            output_tokens: self.usage.output_tokens,
        }
    }
}

/// What the Jev read failed at.
#[derive(Debug, Error)]
pub enum Error {
    /// The System One request failed.
    #[error("{0}")]
    Jev(#[from] jev::Error),
}

/// Ask Jev over `state` with [`questions`] and read the answers back.
/// The caller runs this on a thread of its own under [`DEADLINE`].
///
/// # Errors
///
/// Returns [`Error::Jev`] when the request or the decode fails.
pub async fn ask(
    client: &jev::Client,
    model: &str,
    state: Value,
    questions: jev::Questions,
) -> Result<Report, Error> {
    let request = jev::SystemOneRequest::new(jev::Entry::from(state), questions).model(model);
    let response = client.system_one(request).await?;
    let intent = response.choice(INTENT)?;
    Ok(Report {
        intent: intent.choice.clone(),
        confidence: intent.confidence,
        probabilities: intent
            .probabilities
            .iter()
            .map(|(name, probability)| (name.clone(), *probability))
            .collect(),
        addressed: response.noul(ADDRESSED)?.noul,
        continuing: response.noul(CONTINUING)?.noul,
        usage: response.usage,
        request_id: response.request_id().map(str::to_string),
    })
}

/// What the desk does, the answers-to-action table's output.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Action {
    /// Move the pointer to the hand's aim.
    Point,
    /// Press what the pointer is over.
    Press,
    /// Let go of a held press or drag.
    Release,
    /// Move to the desk on the left.
    SwitchLeft,
    /// Move to the desk on the right.
    SwitchRight,
    /// Dismiss what is up.
    Escape,
    /// The hand rests; nothing happens and nothing ends.
    Rest,
    /// The running gesture continues across the window; emit nothing.
    Hold,
    /// The window carries no input; the rules' `None` stands.
    Drop,
}

impl Action {
    /// The name the state and the transcript carry.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Action::Point => "point",
            Action::Press => "press",
            Action::Release => "release",
            Action::SwitchLeft => "swipe_left",
            Action::SwitchRight => "swipe_right",
            Action::Escape => "escape",
            Action::Rest => "rest",
            Action::Hold => "hold",
            Action::Drop => "drop",
        }
    }
}

/// The floors one decision reads. [`Floors::default`] carries the
/// constants above, which is what the desk runs; a scorer that sweeps a
/// floor builds its own and reads the same table, so the table stays in
/// one place while the numbers are measured
/// (`crates/coder-hands-measure`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Floors {
    /// The floor an action clears to act.
    pub act: f64,
    /// The floor `press` clears.
    pub press: f64,
    /// The floor `escape` clears.
    pub escape: f64,
    /// The floor `addressed` clears before the window counts at all.
    pub addressed: f64,
    /// The floor `continuing` clears to hold the running gesture.
    pub continuing: f64,
}

impl Default for Floors {
    fn default() -> Floors {
        Floors {
            act: ACT_FLOOR,
            press: PRESS_FLOOR,
            escape: ESCAPE_FLOOR,
            addressed: ADDRESSED_FLOOR,
            continuing: CONTINUING_FLOOR,
        }
    }
}

impl Floors {
    /// The floor an `intent` option's confidence clears before it can
    /// act. `press` and `escape` carry higher floors because a wrong
    /// press costs more than a wrong pointer move.
    #[must_use]
    pub fn of(&self, intent: &str) -> f64 {
        match intent {
            "press" => self.press,
            "escape" => self.escape,
            _ => self.act,
        }
    }
}

impl ReportView {
    /// The answers-to-action table at these floors. `addressed` below
    /// its floor drops the window whatever `intent` says — it is the
    /// only answer that drops. `continuing` above its floor holds the
    /// running gesture. An `intent` under its floor drops too: for an
    /// ambiguous window the rules' answer is nothing, and a guess under
    /// the floor is nothing as well.
    #[must_use]
    pub fn verdict_with(&self, floors: Floors) -> Action {
        if self.addressed < floors.addressed {
            return Action::Drop;
        }
        if self.continuing >= floors.continuing {
            return Action::Hold;
        }
        let action = match self.intent.as_str() {
            "point" => Action::Point,
            "press" => Action::Press,
            "release" => Action::Release,
            "swipe_left" => Action::SwitchLeft,
            "swipe_right" => Action::SwitchRight,
            "escape" => Action::Escape,
            "rest" => Action::Rest,
            _ => Action::Drop,
        };
        if action == Action::Drop || self.confidence < floors.of(&self.intent) {
            return Action::Drop;
        }
        action
    }
}

/// The answers-to-action table at the floors the desk runs.
#[must_use]
pub fn verdict(report: &Report) -> Action {
    report.view().verdict_with(Floors::default())
}

/// How far the seam goes. The rollout is shadow first: the seam collects
/// the corpus that measures it before it suggests, and suggests before
/// it acts (`docs/os/hands-judge.md`, "Rollout").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The seam asks nothing; the rules run alone. The default, and the
    /// mode a host runs until `coderos.desktop.hands.judge` turns it on.
    #[default]
    Off,
    /// Ask and record; the desk acts on the rules alone. The stage that
    /// builds the corpus the floors are measured on.
    Shadow,
    /// The overlay shows the top intent beside the rule label; the desk
    /// still acts on the rules alone.
    Suggest,
    /// Above the floors, an ambiguous window acts on the answer.
    Act,
}

impl Mode {
    /// The mode a word names, or `None` when it names none.
    /// `os/modules/coderos/hands.nix` writes `shadow` when
    /// `coderos.desktop.hands.judge` is on, because the rollout starts
    /// there; the other rungs are for a run by hand, and `Act` waits on
    /// the measurement that says the seam may act.
    #[must_use]
    pub fn named(value: &str) -> Option<Mode> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "0" | "off" | "false" | "no" => Some(Mode::Off),
            "1" | "on" | "true" | "yes" | "shadow" => Some(Mode::Shadow),
            "suggest" => Some(Mode::Suggest),
            "act" => Some(Mode::Act),
            _ => None,
        }
    }

    /// The word [`Mode::named`] reads back, which the transcript keeps.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Mode::Off => "off",
            Mode::Shadow => "shadow",
            Mode::Suggest => "suggest",
            Mode::Act => "act",
        }
    }
}

/// The seam's owner: its mode, and through it the fallback and the
/// overlay text.
#[derive(Clone, Debug)]
pub struct Judge {
    mode: Mode,
}

impl Judge {
    /// A judge in `mode`. A host with the option off builds
    /// `Judge::new(Mode::Off)`, and so does a host whose Jev
    /// configuration resolved off — the fallback is the rules alone.
    #[must_use]
    pub fn new(mode: Mode) -> Self {
        Self { mode }
    }

    /// The mode the judge runs in.
    #[must_use]
    pub fn mode(&self) -> Mode {
        self.mode
    }

    /// Whether an ambiguous transition sends a request. Every mode but
    /// `Off` asks, because the shadow stage is where the corpus comes
    /// from.
    #[must_use]
    pub fn asks(&self) -> bool {
        self.mode != Mode::Off
    }

    /// What the hands overlay says. The seam never names a model.
    #[must_use]
    pub fn status(&self) -> &'static str {
        match self.mode {
            Mode::Off => RULES_ONLY,
            Mode::Shadow => "hands: shadow",
            Mode::Suggest => "hands: suggest",
            Mode::Act => "hands: act",
        }
    }

    /// What the desk does with an answer under this mode.
    ///
    /// `met_deadline` says whether the round trip finished inside
    /// [`DEADLINE`]. An answer that missed it never acts, because the
    /// desk has moved on by then, and it is still recorded, because the
    /// measurement counts an answer against what the hand meant rather
    /// than against when it arrived. That is the whole difference
    /// between the shadow rung and the acting one on a late answer.
    #[must_use]
    pub fn decide(&self, report: &Report, met_deadline: bool) -> Decision {
        let action = verdict(report);
        Decision {
            action,
            apply: self.mode == Mode::Act
                && met_deadline
                && matches!(
                    action,
                    Action::Point
                        | Action::Press
                        | Action::Release
                        | Action::SwitchLeft
                        | Action::SwitchRight
                        | Action::Escape
                ),
            hint: (self.mode == Mode::Suggest).then(|| report.intent.clone()),
        }
    }
}

/// What the desk does with one answer: the action the table gives, and
/// under `Act` whether it applies.
#[derive(Clone, Debug, PartialEq)]
pub struct Decision {
    /// The answers-to-action table's result.
    pub action: Action,
    /// Whether the desk applies it. False in `Shadow` and `Suggest`,
    /// where the answer is recorded but never moves the desk, and false
    /// for `Drop`, `Hold`, and `Rest`, which emit nothing.
    pub apply: bool,
    /// The intent the overlay shows in `Suggest` mode.
    pub hint: Option<String>,
}

/// What the compositor's transcript keeps for one ask: the window's
/// label and margin, the answers with their probabilities, the deadline
/// met or missed, and the action taken.
#[derive(Clone, Debug, Serialize)]
pub struct Record {
    /// Frames the asked window carried.
    pub window: usize,
    /// The rules' label on the newest frame.
    pub pose: String,
    /// The thinnest margin on that frame, when it had 21 joints.
    pub margin: Option<f32>,
    /// The deadline the request ran under, in milliseconds.
    pub deadline_ms: u64,
    /// Whether the answer arrived inside the deadline.
    pub met_deadline: bool,
    /// The answers, when one arrived in time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report: Option<ReportView>,
    /// What the rules did on the window's newest frame, by name, which is
    /// what the seam is measured against: the answer agrees with what the
    /// person did next more often than this, or the seam does not act.
    pub rules: Vec<String>,
    /// The action the desk took: the applied one, or `drop` when the
    /// rules stood.
    pub action: String,
    /// Why no answer arrived, when the request failed. The sentence
    /// carries no key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed: Option<String>,
}

/// What the compositor's transcript keeps for an ambiguous window the
/// seam did not ask about: the same window fields a [`Record`] carries,
/// and the reason no request went out.
///
/// It is written on its own line beside the records, and a reader tells
/// the two apart by the `skipped` field. Counting these rather than
/// writing them is what made the ask rate untrustworthy: a record that
/// holds only the windows that were asked about describes the asks that
/// happened to land.
#[derive(Clone, Debug, Serialize)]
pub struct Missed {
    /// Frames the window carried.
    pub window: usize,
    /// The rules' label on the newest frame.
    pub pose: String,
    /// The thinnest margin on that frame, when it had 21 joints.
    pub margin: Option<f32>,
    /// What the rules did on that frame, by name.
    pub rules: Vec<String>,
    /// Why no request went out, from [`Skip::word`].
    pub skipped: &'static str,
}

/// The longest string the state may carry: a pose label or a gesture's
/// name.
const WORD_LIMIT: usize = 32;

/// What a withheld string is replaced with.
const WITHHELD: &str = "[withheld]";

/// The state one request sends, and how many strings the scan withheld.
///
/// Hand features are numbers, pose labels, and gesture names, so the scan
/// should never find anything, and it runs anyway, the way every seam
/// that sends state off the machine scans it. A string passes when it is
/// at most [`WORD_LIMIT`] bytes of ASCII letters, digits, `_`, `-`, and
/// spaces. Any other string is replaced with `[withheld]` and counted.
#[must_use]
pub fn sendable(mut state: Value) -> (Value, usize) {
    let findings = withhold(&mut state);
    (state, findings)
}

/// Replaces every string that is not a short word, and counts them.
fn withhold(value: &mut Value) -> usize {
    match value {
        Value::String(text) => {
            let word = text.len() <= WORD_LIMIT
                && text
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"_- ".contains(&byte));
            if word {
                0
            } else {
                *text = WITHHELD.to_string();
                1
            }
        }
        Value::Array(items) => items.iter_mut().map(withhold).sum(),
        Value::Object(fields) => fields.values_mut().map(withhold).sum(),
        _ => 0,
    }
}

#[cfg(test)]
#[path = "judge_tests.rs"]
mod tests;
