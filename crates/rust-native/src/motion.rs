//! Motion: easing curves, panel open and close transitions, and working
//! indicators, as pure functions of a clock.
//!
//! An adapter owns the clock and the frames. This module answers two
//! questions for a given instant: what a panel or an indicator looks like,
//! and when the next frame is due. When nothing animates, the answer to the
//! second is `None`, so an adapter that asks schedules no frame work. Under
//! reduced motion, panels snap to their end state and indicators rest, and
//! neither asks for a frame.
//!
//! The durations, curves, and indicator keyframes reimplement the motion
//! catalog and loaders of Zeron (`crates/ui/src/motion.rs`,
//! `crates/ui/src/loaders.rs`, and `crates/proto/src/motion.rs` at commit
//! `9e1a1115`), which is MIT licensed:
//!
//! > Copyright (c) 2026 Wing. Permission is hereby granted, free of charge,
//! > to any person obtaining a copy of this software and associated
//! > documentation files (the "Software"), to deal in the Software without
//! > restriction, including without limitation the rights to use, copy,
//! > modify, merge, publish, distribute, sublicense, and/or sell copies of
//! > the Software, and to permit persons to whom the Software is furnished
//! > to do so, subject to the following conditions: The above copyright
//! > notice and this permission notice shall be included in all copies or
//! > substantial portions of the Software. THE SOFTWARE IS PROVIDED "AS
//! > IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT
//! > NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
//! > PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS
//! > OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
//! > LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
//! > FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
//! > DEALINGS IN THE SOFTWARE.

use std::time::{Duration, Instant};

/// How often a repeating indicator asks for a frame: about 30 frames a
/// second. The cells are coarse, so a display-rate clock adds redraws
/// without a visible difference.
pub const FRAME_INTERVAL: Duration = Duration::from_millis(33);

/// A CSS `cubic-bezier(x1, y1, x2, y2)` timing function, with its ends fixed
/// at (0, 0) and (1, 1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Curve {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Curve {
    pub const fn new(x1: f32, y1: f32, x2: f32, y2: f32) -> Self {
        Self { x1, y1, x2, y2 }
    }

    /// The eased value at progress `x`, both from 0 to 1. Progress outside
    /// that range is clamped, and so is the result.
    pub fn at(self, x: f32) -> f32 {
        if x.is_nan() || x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }
        let t = self.solve(x);
        cubic(self.y1, self.y2, t).clamp(0.0, 1.0)
    }

    /// The curve parameter whose horizontal coordinate is `x`: Newton's
    /// method, then bisection when the slope is too flat to converge.
    fn solve(self, x: f32) -> f32 {
        let mut t = x;
        for _ in 0..8 {
            let error = cubic(self.x1, self.x2, t) - x;
            if error.abs() < 1e-6 {
                return t;
            }
            let slope = slope(self.x1, self.x2, t);
            if slope.abs() < 1e-6 {
                break;
            }
            t = (t - error / slope).clamp(0.0, 1.0);
        }
        let (mut low, mut high) = (0.0f32, 1.0f32);
        for _ in 0..32 {
            let mid = (low + high) / 2.0;
            if cubic(self.x1, self.x2, mid) < x {
                low = mid;
            } else {
                high = mid;
            }
        }
        (low + high) / 2.0
    }
}

/// One coordinate of the curve at parameter `t`, given its two control
/// coordinates.
fn cubic(a: f32, b: f32, t: f32) -> f32 {
    let r = 1.0 - t;
    3.0 * r * r * t * a + 3.0 * r * t * t * b + t * t * t
}

fn slope(a: f32, b: f32, t: f32) -> f32 {
    let r = 1.0 - t;
    3.0 * r * r * a + 6.0 * r * t * (b - a) + 3.0 * t * t * (1.0 - b)
}

/// CSS `ease`: quick fades and panel pops.
pub const EASE: Curve = Curve::new(0.25, 0.1, 0.25, 1.0);
/// CSS `ease-out`: size changes.
pub const EASE_OUT: Curve = Curve::new(0.0, 0.0, 0.58, 1.0);
/// CSS `ease-in-out`: glides that start and land gently.
pub const EASE_IN_OUT: Curve = Curve::new(0.42, 0.0, 0.58, 1.0);
/// An exponential ease-out, `cubic-bezier(0.16, 1, 0.3, 1)`: entrances.
pub const EASE_OUT_EXPO: Curve = Curve::new(0.16, 1.0, 0.3, 1.0);

/// A timed transition: a duration after an optional delay, along a curve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spec {
    pub duration: Duration,
    pub delay: Duration,
    pub curve: Curve,
}

impl Spec {
    pub const fn new(millis: u64, curve: Curve) -> Self {
        Self {
            duration: Duration::from_millis(millis),
            delay: Duration::ZERO,
            curve,
        }
    }

    pub const fn with_delay(mut self, millis: u64) -> Self {
        self.delay = Duration::from_millis(millis);
        self
    }

    /// The delay and the duration together.
    pub fn total(&self) -> Duration {
        self.delay + self.duration
    }

    /// The eased progress `elapsed` after the transition started: 0 through
    /// the delay, then along the curve to 1.
    pub fn progress(&self, elapsed: Duration) -> f32 {
        if self.duration.is_zero() {
            return if elapsed >= self.delay { 1.0 } else { 0.0 };
        }
        let running = elapsed.saturating_sub(self.delay);
        self.curve
            .at(running.as_secs_f32() / self.duration.as_secs_f32())
    }
}

/// A panel or a popover opening: 180 milliseconds.
pub const PANEL_OPEN: Spec = Spec::new(180, EASE);
/// A panel closing: 100 milliseconds, quicker than it opened, so it gets out
/// of the way.
pub const PANEL_CLOSE: Spec = Spec::new(100, EASE);
/// A larger surface entering: 500 milliseconds on the exponential ease-out.
pub const FADE_IN: Spec = Spec::new(500, EASE_OUT_EXPO);
/// A pane's width or height changing: 200 milliseconds.
pub const RESIZE: Spec = Spec::new(200, EASE_OUT);

/// How far, in points, a panel travels as it opens: it rises into place.
pub const PANEL_RISE: f32 = 4.0;

/// What a panel looks like at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PanelFrame {
    /// How open the panel is, from 0 (closed) to 1 (open), eased.
    pub openness: f32,
    /// The panel's opacity, from 0 to 1.
    pub opacity: f32,
    /// How far below its resting place to draw the panel, in points.
    pub offset: f32,
    /// Whether to draw the panel at all. A closed panel draws nothing.
    pub visible: bool,
}

/// A panel's open and close transition.
///
/// Reversing a transition part way through starts from where the panel is,
/// never from the far end, and takes the share of the full duration that
/// the remaining distance needs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    open: bool,
    /// Openness when the current transition started.
    from: f32,
    /// When the current transition started, and the span it runs; `None`
    /// when the panel is at rest.
    running: Option<(Instant, Spec)>,
}

impl Panel {
    /// A panel at rest, open or closed.
    pub const fn new(open: bool) -> Self {
        Self {
            open,
            from: if open { 1.0 } else { 0.0 },
            running: None,
        }
    }

    /// Whether the panel is open or opening.
    pub fn open(&self) -> bool {
        self.open
    }

    /// Opens or closes the panel at `now`. Under `reduced` motion the panel
    /// is at its end state at once.
    pub fn set_open(&mut self, open: bool, now: Instant, reduced: bool) {
        if open == self.open {
            return;
        }
        let from = self.openness(now);
        self.open = open;
        let target = if open { 1.0 } else { 0.0 };
        let distance = (target - from).abs();
        if reduced || distance <= f32::EPSILON {
            self.from = target;
            self.running = None;
            return;
        }
        let mut spec = if open { PANEL_OPEN } else { PANEL_CLOSE };
        spec.duration = spec.duration.mul_f32(distance);
        self.from = from;
        self.running = Some((now, spec));
    }

    /// Openness at `now`, from 0 to 1.
    pub fn openness(&self, now: Instant) -> f32 {
        let target = if self.open { 1.0 } else { 0.0 };
        let Some((start, spec)) = self.running else {
            return target;
        };
        let progress = spec.progress(now.saturating_duration_since(start));
        self.from + (target - self.from) * progress
    }

    /// What the panel looks like at `now`.
    pub fn frame(&self, now: Instant) -> PanelFrame {
        let openness = self.openness(now);
        PanelFrame {
            openness,
            opacity: openness,
            offset: PANEL_RISE * (1.0 - openness),
            visible: openness > 0.0,
        }
    }

    /// When the next frame is due, or `None` when the panel is at rest at
    /// `now`. A finished transition settles here, so later calls ask for
    /// nothing.
    pub fn next_frame(&mut self, now: Instant) -> Option<Instant> {
        let (start, spec) = self.running?;
        if now.saturating_duration_since(start) >= spec.total() {
            self.from = if self.open { 1.0 } else { 0.0 };
            self.running = None;
            return None;
        }
        Some(now + FRAME_INTERVAL)
    }
}

/// The working indicator's wave period: 750 milliseconds.
pub const WORKING_PERIOD: Duration = Duration::from_millis(750);
/// Cells on a side of the working indicator's square grid.
pub const WORKING_SIDE: usize = 3;
/// The opacity a cell rests at between pulses.
pub const WORKING_DIM: f32 = 0.1;
/// The opacity every cell holds under reduced motion: still, and clearly
/// present.
pub const WORKING_REST: f32 = 0.6;

/// The working indicator: a square grid of cells whose brightness travels
/// upward, from the bottom edge to the top center, once a period.
///
/// Every indicator shares the epoch it was made with, so two indicators
/// made from one `Working` stay in step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Working {
    epoch: Instant,
}

impl Working {
    pub const fn new(epoch: Instant) -> Self {
        Self { epoch }
    }

    /// How far through its period the wave is at `now`, from 0 to 1.
    pub fn phase(&self, now: Instant) -> f32 {
        let elapsed = now.saturating_duration_since(self.epoch);
        let period = WORKING_PERIOD.as_nanos();
        (elapsed.as_nanos() % period) as f32 / period as f32
    }

    /// Each cell's opacity at `now`, row by row from the top. Under
    /// `reduced` motion every cell rests at [`WORKING_REST`].
    pub fn cells(&self, now: Instant, reduced: bool) -> [f32; WORKING_SIDE * WORKING_SIDE] {
        let mut cells = [WORKING_REST; WORKING_SIDE * WORKING_SIDE];
        if reduced {
            return cells;
        }
        let phase = self.phase(now);
        for (index, cell) in cells.iter_mut().enumerate() {
            let (row, column) = (index / WORKING_SIDE, index % WORKING_SIDE);
            *cell = working_opacity(phase + cell_phase(row, column), WORKING_DIM);
        }
        cells
    }

    /// When the next frame is due: the next tick of the shared clock after
    /// `now`. Under `reduced` motion the cells rest, so no frame is due.
    pub fn next_frame(&self, now: Instant, reduced: bool) -> Option<Instant> {
        if reduced {
            return None;
        }
        let elapsed = now.saturating_duration_since(self.epoch).as_nanos();
        let tick = FRAME_INTERVAL.as_nanos();
        let next = (elapsed / tick + 1) * tick;
        Some(self.epoch + Duration::from_nanos(next as u64))
    }
}

/// A cell's offset into the wave: cells on the bottom edge lead, and the
/// wave converges on the top center, so it reads as travelling upward.
pub fn cell_phase(row: usize, column: usize) -> f32 {
    let side = WORKING_SIDE as f32;
    let center = (side - 1.0) / 2.0;
    let farthest = side - 1.0 + center;
    let distance = side - 1.0 - row as f32 + (column as f32 - center).abs();
    distance / (farthest + 1.0)
}

/// A cell's opacity at phase `t` of the period: full at the start, easing to
/// `dim` by 45 percent, resting there until 92 percent, then rising back.
pub fn working_opacity(t: f32, dim: f32) -> f32 {
    let t = t.rem_euclid(1.0);
    let mix = |from: f32, to: f32, s: f32| from + (to - from) * s;
    if t < 0.45 {
        mix(1.0, dim, t / 0.45)
    } else if t < 0.92 {
        dim
    } else {
        mix(dim, 1.0, (t - 0.92) / 0.08)
    }
}

/// The earlier of two optional frame deadlines: an adapter folds every
/// animation's [`Panel::next_frame`] and [`Working::next_frame`] into one.
pub fn earliest(a: Option<Instant>, b: Option<Instant>) -> Option<Instant> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(millis: u64) -> Duration {
        Duration::from_millis(millis)
    }

    fn close(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn curves_start_at_zero_end_at_one_and_never_fall() {
        for curve in [EASE, EASE_OUT, EASE_IN_OUT, EASE_OUT_EXPO] {
            assert_eq!(curve.at(0.0), 0.0);
            assert_eq!(curve.at(1.0), 1.0);
            assert_eq!(curve.at(-1.0), 0.0);
            assert_eq!(curve.at(2.0), 1.0);
            assert_eq!(curve.at(f32::NAN), 0.0);
            let mut previous = 0.0;
            for step in 0..=100 {
                let value = curve.at(step as f32 / 100.0);
                assert!((0.0..=1.0).contains(&value));
                assert!(value + 1e-5 >= previous, "{curve:?} falls at {step}");
                previous = value;
            }
        }
    }

    #[test]
    fn curves_match_their_css_values() {
        // Reference values from the CSS definitions of these curves.
        assert!(close(EASE_IN_OUT.at(0.5), 0.5));
        assert!(close(EASE.at(0.5), 0.8024));
        assert!(close(EASE_OUT.at(0.5), 0.6847));
        assert!(EASE_OUT_EXPO.at(0.2) > 0.6, "the entrance front-loads");
        let linear = Curve::new(0.0, 0.0, 1.0, 1.0);
        assert!(close(linear.at(0.25), 0.25));
    }

    #[test]
    fn a_spec_holds_through_its_delay_then_eases_over_its_duration() {
        let spec = Spec::new(500, EASE).with_delay(150);
        assert_eq!(spec.total(), ms(650));
        assert_eq!(spec.progress(ms(0)), 0.0);
        assert_eq!(spec.progress(ms(150)), 0.0);
        assert!(close(spec.progress(ms(400)), EASE.at(0.5)));
        assert_eq!(spec.progress(ms(650)), 1.0);
        assert_eq!(spec.progress(ms(5_000)), 1.0);
        assert_eq!(Spec::new(0, EASE).progress(ms(0)), 1.0);
    }

    #[test]
    fn a_panel_opens_and_closes_along_its_curve_over_a_clock() {
        let start = Instant::now();
        let mut panel = Panel::new(false);
        assert_eq!(panel.next_frame(start), None);
        assert!(!panel.frame(start).visible);

        panel.set_open(true, start, false);
        assert_eq!(panel.frame(start).opacity, 0.0);
        assert_eq!(panel.frame(start).offset, PANEL_RISE);
        let half = start + ms(90);
        assert!(close(panel.openness(half), EASE.at(0.5)));
        assert!(close(
            panel.frame(half).offset,
            PANEL_RISE * (1.0 - EASE.at(0.5))
        ));
        assert_eq!(panel.next_frame(half), Some(half + FRAME_INTERVAL));

        let done = start + ms(180);
        assert_eq!(panel.frame(done).opacity, 1.0);
        assert_eq!(panel.frame(done).offset, 0.0);
        assert_eq!(panel.next_frame(done), None);
        assert_eq!(panel.next_frame(done + ms(1_000)), None);

        panel.set_open(false, done, false);
        let closing = done + ms(50);
        assert!(close(panel.openness(closing), 1.0 - EASE.at(0.5)));
        let closed = done + ms(100);
        assert!(!panel.frame(closed).visible);
        assert_eq!(panel.next_frame(closed), None);
    }

    #[test]
    fn reversing_a_panel_starts_from_where_it_is() {
        let start = Instant::now();
        let mut panel = Panel::new(false);
        panel.set_open(true, start, false);
        let turn = start + ms(90);
        let there = panel.openness(turn);
        panel.set_open(false, turn, false);
        assert!(close(panel.openness(turn), there), "no jump on reversal");
        // The close covers only the distance left: its share of 100 ms.
        let back = turn + PANEL_CLOSE.duration.mul_f32(there);
        assert_eq!(panel.openness(back), 0.0);
        assert_eq!(panel.next_frame(back), None);
        // Asking again for the state already set changes nothing.
        panel.set_open(false, back, false);
        assert_eq!(panel.next_frame(back), None);
    }

    #[test]
    fn reduced_motion_snaps_a_panel_and_asks_for_no_frame() {
        let now = Instant::now();
        let mut panel = Panel::new(false);
        panel.set_open(true, now, true);
        assert_eq!(panel.frame(now).opacity, 1.0);
        assert_eq!(panel.frame(now).offset, 0.0);
        assert_eq!(panel.next_frame(now), None);
        panel.set_open(false, now, true);
        assert!(!panel.frame(now).visible);
        assert_eq!(panel.next_frame(now), None);
    }

    #[test]
    fn the_working_wave_travels_upward_once_a_period() {
        let epoch = Instant::now();
        let working = Working::new(epoch);
        assert_eq!(working.phase(epoch), 0.0);
        assert!(close(working.phase(epoch + ms(375)), 0.5));
        assert_eq!(
            working.cells(epoch + WORKING_PERIOD, false),
            working.cells(epoch, false)
        );
        // The bottom edge leads and the top center trails.
        assert!(cell_phase(2, 1) < cell_phase(1, 1));
        assert!(cell_phase(1, 1) < cell_phase(0, 1));
        assert_eq!(cell_phase(2, 1), 0.0);
        // At the start of the period, the bottom center is at full and the
        // others are fading toward the dim floor.
        let cells = working.cells(epoch, false);
        assert_eq!(cells[7], 1.0);
        assert!(cells.iter().all(|cell| (WORKING_DIM..=1.0).contains(cell)));
        assert!(cells[1] < cells[7]);
    }

    #[test]
    fn working_opacity_follows_its_keyframes() {
        assert_eq!(working_opacity(0.0, 0.1), 1.0);
        assert!(close(working_opacity(0.225, 0.1), 0.55));
        assert!(close(working_opacity(0.45, 0.1), 0.1));
        assert!(close(working_opacity(0.7, 0.1), 0.1));
        assert!(close(working_opacity(0.96, 0.1), 0.55));
        assert!(close(working_opacity(1.0, 0.1), 1.0));
        assert!(close(working_opacity(-0.04, 0.1), 0.55));
    }

    #[test]
    fn the_working_clock_ticks_on_shared_boundaries() {
        let epoch = Instant::now();
        let working = Working::new(epoch);
        assert_eq!(
            working.next_frame(epoch, false),
            Some(epoch + FRAME_INTERVAL)
        );
        assert_eq!(
            working.next_frame(epoch + ms(10), false),
            Some(epoch + FRAME_INTERVAL)
        );
        assert_eq!(
            working.next_frame(epoch + FRAME_INTERVAL, false),
            Some(epoch + FRAME_INTERVAL * 2)
        );
        // A second indicator made from the same epoch is in step.
        let other = Working::new(epoch);
        assert_eq!(
            other.cells(epoch + ms(123), false),
            working.cells(epoch + ms(123), false)
        );
    }

    #[test]
    fn reduced_motion_rests_the_working_indicator_without_frames() {
        let epoch = Instant::now();
        let working = Working::new(epoch);
        assert_eq!(working.next_frame(epoch + ms(10), true), None);
        let rest = working.cells(epoch + ms(10), true);
        assert!(rest.iter().all(|cell| *cell == WORKING_REST));
        assert_eq!(rest, working.cells(epoch + ms(400), true));
    }

    #[test]
    fn nothing_animating_means_no_frame_is_due() {
        let now = Instant::now();
        let mut panel = Panel::new(true);
        assert_eq!(earliest(panel.next_frame(now), None), None);
        let working = Working::new(now);
        let due = working.next_frame(now, false);
        assert_eq!(earliest(panel.next_frame(now), due), due);
        assert_eq!(
            earliest(Some(now + ms(5)), Some(now + ms(3))),
            Some(now + ms(3))
        );
    }
}
