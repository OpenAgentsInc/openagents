//! What a caller asks about a hand window the rules cannot settle, and
//! what it does with the answer.
//!
//! Two callers read it: `crates/coder-compositor`, which acts on the desk,
//! and `crates/coder-hands-measure`, which scores a recorded run. It sits
//! beside the rules in [`crate::gestures`] rather than in either one.
//!
//! The per-frame path is `crate::gestures` and nothing here changes it: a
//! clear gesture acts on the frame it is read on, and the pointer waits
//! on no network call. This module sits beside it. It pushes every frame
//! into `crate::judge::Window`, and on a transition the rules
//! decided on a thin margin, or in a window whose labels keep flipping,
//! it sends the window's features through `crate::seam` and records
//! what comes back. The design is `docs/os/hands-judge.md`.
//!
//! `CODEROS_HANDS_JUDGE` names the rung of the rollout this session runs,
//! which `os/modules/coderos/hands.nix` writes as `shadow` when
//! `coderos.desktop.hands.judge` is on. In shadow the seam asks, records,
//! and changes nothing: every record carries the answers with their
//! probabilities, whether the deadline was met, and what the rules did
//! instead, which is what the measurement in `docs/os/camera-and-hands.md` reads. The
//! seam acts only in `act`, which waits on that measurement.
//!
//! The key comes from `TYPESAFE_API_KEY` or `~/.openagents/jev.json`
//! through [`crate::seam::configured`], and a session
//! with no key runs the rules alone and says so once. The state carries
//! palm-relative features and pose labels, never an image and never a raw
//! coordinate, and `crate::judge::sendable` scans it before it
//! leaves the machine.

use crate::judge::{self, Counts, Judge, Missed, Mode, Record, Skip, window_frames};
use crate::judge::{DEADLINE, Trigger, Window};
use crate::seam::{Answer, Meta, Seam};
use crate::wire::Line;
use crate::{Frame, Hand, recognize};
use coder_wm::Dir;
use serde_json::Value;

use crate::gestures::{Act, Step};

/// The variable that names the rung this session runs, one of `off`,
/// `shadow`, `suggest`, and `act`. Unset reads as off.
pub const MODE_VAR: &str = "CODEROS_HANDS_JUDGE";

/// How many gaps between frames the rate is read from. The window is
/// `judge::WINDOW_SECONDS` of hand, so the caller has to know how fast
/// the camera answers, and it is not one number: three recordings of
/// 2026-09-18 ran at 30, 30, and 15 frames a second and the CoderOS
/// daemon reports 15 to 16 in its own room. Twenty gaps is over a second
/// of hand at either rate, and the median of them ignores the frame the
/// tracker took twice as long over.
const RATE_GAPS: usize = 20;

/// Gaps outside this many seconds are a dropout or a clock that went
/// backwards rather than a frame, and the rate does not read them.
const RATE_GAP_BOUNDS: std::ops::Range<f64> = 0.001..2.0;

/// The rate the last frames arrived at, which sizes the window.
///
/// The desk reads it off the frames the camera sends and
/// `crates/coder-hands-measure` reads it off the rows of a recorded run,
/// so a window holds the same span of hand in both.
#[derive(Debug, Default)]
pub struct Rate {
    last: Option<f64>,
    gaps: std::collections::VecDeque<f64>,
}

impl Rate {
    /// One frame's timestamp. Answers the frames a window should hold
    /// once enough gaps have arrived to read a rate, and `None` before
    /// that.
    pub fn push(&mut self, at: f64) -> Option<usize> {
        let gap = self.last.map(|last| at - last);
        self.last = Some(at);
        let gap = gap?;
        if !RATE_GAP_BOUNDS.contains(&gap) {
            return None;
        }
        self.gaps.push_back(gap);
        while self.gaps.len() > RATE_GAPS {
            self.gaps.pop_front();
        }
        if self.gaps.len() < RATE_GAPS {
            return None;
        }
        let mut sorted: Vec<f64> = self.gaps.iter().copied().collect();
        sorted.sort_by(f64::total_cmp);
        let middle = sorted.get(sorted.len() / 2).copied().unwrap_or_default();
        (middle > 0.0).then(|| window_frames(1.0 / middle))
    }
}

/// The seam beside the rules: the mode, the window it pushes frames into,
/// and the thread it asks on.
pub struct Watch {
    judge: Judge,
    window: Window,
    seam: Option<Seam>,
    rate: Rate,
    paced: u64,
}

impl Watch {
    /// The seam off: the rules run alone and nothing is asked or sent.
    pub fn off() -> Watch {
        Watch {
            judge: Judge::new(Mode::Off),
            window: Window::new(),
            seam: None,
            rate: Rate::default(),
            paced: 0,
        }
    }

    /// The seam this session's environment describes: the mode
    /// [`MODE_VAR`] names, and the key and the model
    /// [`crate::seam::configured`] resolves. Anything missing leaves the rules
    /// alone, with one line that says which.
    pub fn from_environment() -> Watch {
        let named = std::env::var(MODE_VAR).unwrap_or_default();
        let Some(mode) = Mode::named(&named) else {
            log::warn!("hands: {MODE_VAR} holds {named}, which names no mode; the rules run alone");
            return Watch::off();
        };
        Watch::start(mode)
    }

    /// The seam in `mode`, over the Jev configuration this machine
    /// holds.
    fn start(mode: Mode) -> Watch {
        if mode == Mode::Off {
            return Watch::off();
        }
        match crate::seam::configured() {
            Ok((seam, model)) => {
                log::info!(
                    "hands: the seam judges what the rules cannot settle, in {} mode, over {model}; \
                     the desk acts on the rules",
                    mode.word()
                );
                Watch {
                    judge: Judge::new(mode),
                    window: Window::new(),
                    rate: Rate::default(),
                    paced: 0,
                    seam: Some(seam),
                }
            }
            Err(error) => {
                log::info!("hands: {error} The rules run alone.");
                Watch::off()
            }
        }
    }

    /// Whether the seam asks anything at all.
    pub fn asks(&self) -> bool {
        self.judge.asks() && self.seam.is_some()
    }

    /// One frame, after the rules have read it: the window takes it, and
    /// an ambiguous transition asks. `at` is where the hand aims, so an
    /// answer that acts acts where the window pointed.
    pub fn feed(&mut self, line: &Line, step: &Step, at: Option<(f32, f32)>) {
        if !self.asks() {
            return;
        }
        if let Some(frames) = self.rate.push(line.timestamp)
            && frames != self.window.capacity()
        {
            log::info!(
                "hands: the camera answers {:.1} times a second, so the seam's window \
                     holds {frames} frame(s)",
                frames as f64 / judge::WINDOW_SECONDS
            );
            self.window.set_rate(frames as f64 / judge::WINDOW_SECONDS);
        }
        let request = match pending(&mut self.window, line, step, at) {
            Pending::Settled => return,
            Pending::Paced(meta) => {
                self.paced += 1;
                log::info!("hands judge: {}", missed(&meta, Skip::Paced));
                return;
            }
            Pending::Ask(request) => request,
        };
        if request.findings > 0 {
            log::warn!(
                "hands: the window's state carried {} finding(s), which the scan took out",
                request.findings
            );
        }
        if let Some(seam) = self.seam.as_mut() {
            let meta = request.meta.clone();
            if let Err(skip) = seam.ask(request.meta, request.state) {
                log::info!("hands judge: {}", missed(&meta, skip));
            }
        }
    }

    /// The gestures the desk just acted on, so the window's `previous`
    /// names one and the seam can be asked whether a window continues it.
    pub fn acted(&mut self, acts: &[Act]) {
        if !self.asks() {
            return;
        }
        for act in acts {
            if let Some(gesture) = gesture_of(act) {
                self.window.acted(gesture);
            }
        }
    }

    /// Every answer that arrived, written to the transcript, with the
    /// acts it applies. In shadow and suggest that is none: the seam
    /// records what it would have done and the rules keep the desk.
    pub fn take(&mut self) -> Vec<Act> {
        let answers = match self.seam.as_mut() {
            Some(seam) => seam.take(),
            None => return Vec::new(),
        };
        let mut acts = Vec::new();
        for answer in answers {
            let (record, act) = judged(&self.judge, &answer);
            log::info!("hands judge: {}", transcript(&record));
            acts.extend(act);
        }
        acts
    }

    /// Forgets the window, which is what turning tracking off does. A
    /// request already in flight still answers, and its record says which
    /// window it was asked over.
    pub fn reset(&mut self) {
        self.window.clear();
        self.rate = Rate::default();
    }

    /// What the seam did this run, for the line the compositor writes
    /// when tracking stops: the asks that went out, and the ambiguous
    /// windows no request went out for, by reason.
    pub fn counts(&self) -> Counts {
        let mut counts = match &self.seam {
            Some(seam) => seam.counts(),
            None => Counts::default(),
        };
        counts.paced = self.paced;
        counts
    }
}

/// What an ambiguous window owes the seam: the state to send, the meta
/// the answer comes back beside, and how many findings the redaction
/// scan took out of the state before it left.
#[derive(Clone, Debug)]
pub struct Request {
    /// The state one request evaluates, after the scan.
    pub state: Value,
    /// What the transcript keeps about the window.
    pub meta: Meta,
    /// Findings the scan took out.
    pub findings: usize,
}

/// What one frame owes the seam.
#[derive(Clone, Debug)]
pub enum Pending {
    /// The rules settled the transition, or there was none.
    Settled,
    /// The request this window owes.
    Ask(Request),
    /// An ambiguous window the trigger held back, and what the record
    /// keeps about it. The caller writes it rather than dropping it.
    Paced(Meta),
}

/// One frame into `window`, and what an ambiguous transition owes at the
/// trigger the desk runs.
#[must_use]
pub fn pending(window: &mut Window, line: &Line, step: &Step, at: Option<(f32, f32)>) -> Pending {
    pending_with(window, line, step, at, Trigger::default())
}

/// [`pending`] at a trigger the caller chose, which is what a scorer
/// sweeping a margin or a spacing reads. The trigger, the state, and the
/// meta all live here, so the desk and the measurement in
/// `crates/coder-hands-measure` ask about the same windows and send the
/// same thing.
#[must_use]
pub fn pending_with(
    window: &mut Window,
    line: &Line,
    step: &Step,
    at: Option<(f32, f32)>,
    trigger: Trigger,
) -> Pending {
    window.push(&frame_of(line, step));
    let asking = window.ask(trigger);
    if asking == judge::Ask::Settled {
        return Pending::Settled;
    }
    let meta = Meta {
        window: window.len(),
        pose: step.label.word().to_string(),
        margin: Some(step.margin),
        rules: step.acts.iter().map(|act| act.word().to_string()).collect(),
        at,
    };
    if asking == judge::Ask::Paced {
        return Pending::Paced(meta);
    }
    let (state, findings) = judge::sendable(window.state());
    Pending::Ask(Request {
        state,
        meta,
        findings,
    })
}

/// One skipped window as the transcript writes it, on the same kind of
/// line as a [`Record`] and told apart by its `skipped` field.
fn missed(meta: &Meta, skip: Skip) -> String {
    let missed = Missed {
        window: meta.window,
        pose: meta.pose.clone(),
        margin: meta.margin,
        rules: meta.rules.clone(),
        skipped: skip.word(),
    };
    serde_json::to_string(&missed).unwrap_or_else(|error| format!("{{\"broken\":\"{error}\"}}"))
}

/// The frame the window reads: the hand the rules read, smoothed, with
/// the pose set's own label, and every other hand the line carried, so
/// the state says whether a second hand is in the picture.
fn frame_of(line: &Line, step: &Step) -> Frame {
    let mut frame = line.frame();
    if let Some(landmarks) = step.hand {
        let followed = Hand {
            landmarks,
            pose: recognize(&landmarks),
        };
        if frame.hands.is_empty() {
            frame.hands.push(followed);
        } else {
            frame.hands[0] = followed;
        }
    }
    frame
}

/// The name the window keeps for one act the desk took. A drag is the
/// press it continues, and a swipe that is neither left nor right is no
/// gesture this seam names.
#[must_use]
pub fn gesture_of(act: &Act) -> Option<&'static str> {
    Some(match act {
        Act::Point(..) => judge::Action::Point.label(),
        Act::Press(..) | Act::Drag(..) => judge::Action::Press.label(),
        Act::Release => judge::Action::Release.label(),
        Act::Swipe(Dir::Left) => judge::Action::SwitchLeft.label(),
        Act::Swipe(Dir::Right) => judge::Action::SwitchRight.label(),
        Act::Swipe(_) => return None,
        Act::Escape => judge::Action::Escape.label(),
    })
}

/// One answer as the transcript keeps it and as the desk takes it: the
/// record, and the act it applies under [`Mode::Act`] alone.
fn judged(judge: &Judge, answer: &Answer) -> (Record, Option<Act>) {
    let meta = &answer.meta;
    match &answer.report {
        Ok(report) => {
            let decision = judge.decide(report, answer.met_deadline);
            let record = Record {
                window: meta.window,
                pose: meta.pose.clone(),
                margin: meta.margin,
                deadline_ms: DEADLINE.as_millis() as u64,
                met_deadline: answer.met_deadline,
                report: Some(report.view()),
                rules: meta.rules.clone(),
                action: decision.action.label().to_string(),
                failed: None,
            };
            let act = if decision.apply {
                act_of(decision.action, meta.at)
            } else {
                None
            };
            (record, act)
        }
        Err(failure) => (
            Record {
                window: meta.window,
                pose: meta.pose.clone(),
                margin: meta.margin,
                deadline_ms: DEADLINE.as_millis() as u64,
                met_deadline: answer.met_deadline,
                report: None,
                rules: meta.rules.clone(),
                action: judge::Action::Drop.label().to_string(),
                failed: Some(failure.clone()),
            },
            None,
        ),
    }
}

/// The act one answer applies, where the window pointed. An action that
/// needs a point and has none applies nothing, because a press at the
/// wrong place is worse than a press the desk never made.
#[must_use]
pub fn act_of(action: judge::Action, at: Option<(f32, f32)>) -> Option<Act> {
    match action {
        judge::Action::Point => at.map(|(x, y)| Act::Point(x, y)),
        judge::Action::Press => at.map(|(x, y)| Act::Press(x, y)),
        judge::Action::Release => Some(Act::Release),
        judge::Action::SwitchLeft => Some(Act::Swipe(Dir::Left)),
        judge::Action::SwitchRight => Some(Act::Swipe(Dir::Right)),
        judge::Action::Escape => Some(Act::Escape),
        judge::Action::Rest | judge::Action::Hold | judge::Action::Drop => None,
    }
}

/// One record as the transcript writes it: JSON on one line, so the
/// measurement reads the log with `jq` rather than a parser of its own.
fn transcript(record: &Record) -> String {
    serde_json::to_string(record).unwrap_or_else(|error| format!("{{\"broken\":\"{error}\"}}"))
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
