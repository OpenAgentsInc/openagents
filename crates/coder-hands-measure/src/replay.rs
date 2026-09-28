//! A recorded run through the rules and the seam's window.
//!
//! The replay reads the rules rather than a copy of them: it feeds every
//! row of a run to `coder_hands::gestures::Gestures` and every frame to
//! `coder_hands::judge::Window`, in the order and with the calls
//! `coder_hands::watch` makes at the desk. What comes back is one beat a
//! row, and both the scorer and the pass that asks the seam read it.
//!
//! A constant that moves in `gestures.rs`, a floor that moves in
//! `coder_hands::judge`, or a trigger that paces the asks is measured by
//! replaying the recorded runs again. Nobody has to wave at the camera
//! twice.

use coder_hands::gestures::Gestures;
use coder_hands::judge::{Skip, Trigger, Window};
use coder_hands::watch::{self, Pending, Rate, Request};

use crate::run::{Phase, Run};

/// One row of a run after the rules and the window have read it.
#[derive(Debug)]
pub struct Beat {
    /// The row's place in the run, counting from zero.
    pub row: usize,
    /// The cue that row belongs to.
    pub cue: usize,
    /// That cue's label.
    pub label: String,
    /// Which part of the cue the row arrived in.
    pub phase: Phase,
    /// The frame's timestamp, in seconds since the Unix epoch.
    pub t: f64,
    /// Whether the frame carried a hand the rules could read.
    pub hand: bool,
    /// The rules' label for the frame.
    pub rule: &'static str,
    /// The margin that label was decided on.
    pub margin: f32,
    /// What the desk did on this frame, by act word.
    pub acts: Vec<String>,
    /// Frames the seam's window held after this one.
    pub window: usize,
    /// The request this frame owes the seam, when the transition was one
    /// the rules could not settle and the trigger let it through.
    pub ask: Option<Request>,
    /// Why this frame's ambiguous window sent no request. Only
    /// [`Skip::Paced`] is a property of the run: whether a window would
    /// have found a request in flight depends on the round trips of the
    /// session it ran in, which a file does not hold.
    pub skipped: Option<Skip>,
}

/// A whole run, read by the rules.
#[derive(Debug)]
pub struct Replay {
    /// One beat a row, in order.
    pub beats: Vec<Beat>,
    /// The trigger the beats were read at.
    pub trigger: Trigger,
}

impl Replay {
    /// The beats of a run at the trigger the desk runs.
    #[must_use]
    pub fn of(run: &Run) -> Replay {
        Replay::of_with(run, Trigger::default())
    }

    /// The beats of a run at a trigger the caller chose, with the rules
    /// and the window fed in the order the desk feeds them.
    ///
    /// The window is sized from the rate the run's own timestamps give,
    /// through the same `coder_hands::watch::Rate` the desk reads off
    /// the camera, so a replayed window holds the span of hand the desk
    /// would have held.
    #[must_use]
    pub fn of_with(run: &Run, trigger: Trigger) -> Replay {
        let mut machine = Gestures::new(run.header.aspect);
        let mut window = Window::new();
        let mut rate = Rate::default();
        let mut beats = Vec::with_capacity(run.rows.len());
        for (row, entry) in run.rows.iter().enumerate() {
            if let Some(frames) = rate.push(entry.line.timestamp)
                && frames != window.capacity()
            {
                window.set_rate(frames as f64 / coder_hands::judge::WINDOW_SECONDS);
            }
            let step = machine.feed(&entry.line);
            let pending =
                watch::pending_with(&mut window, &entry.line, &step, machine.pointer(), trigger);
            for act in &step.acts {
                if let Some(gesture) = watch::gesture_of(act) {
                    window.acted(gesture);
                }
            }
            let (ask, skipped) = match pending {
                Pending::Settled => (None, None),
                Pending::Ask(request) => (Some(request), None),
                Pending::Paced(_) => (None, Some(Skip::Paced)),
            };
            beats.push(Beat {
                row,
                cue: entry.cue,
                label: entry.label.clone(),
                phase: entry.phase,
                t: entry.line.timestamp,
                hand: step.hand.is_some(),
                rule: step.label.word(),
                margin: step.margin,
                acts: step.acts.iter().map(|act| act.word().to_string()).collect(),
                window: window.len(),
                ask,
                skipped,
            });
        }
        Replay { beats, trigger }
    }

    /// How many windows the seam would be asked about.
    #[must_use]
    pub fn asks(&self) -> usize {
        self.beats.iter().filter(|beat| beat.ask.is_some()).count()
    }

    /// How many ambiguous windows the trigger held back.
    #[must_use]
    pub fn paced(&self) -> usize {
        self.beats
            .iter()
            .filter(|beat| beat.skipped == Some(Skip::Paced))
            .count()
    }

    /// When each ask went out, in seconds since the Unix epoch.
    #[must_use]
    pub fn ask_times(&self) -> Vec<f64> {
        self.beats
            .iter()
            .filter(|beat| beat.ask.is_some())
            .map(|beat| beat.t)
            .collect()
    }

    /// The run's wall time, from the first row to the last.
    #[must_use]
    pub fn seconds(&self) -> f64 {
        match (self.beats.first(), self.beats.last()) {
            (Some(first), Some(last)) => (last.t - first.t).max(0.0),
            _ => 0.0,
        }
    }
}
