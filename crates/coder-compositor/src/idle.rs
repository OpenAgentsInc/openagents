//! When the compositor tells `ext-idle-notify` that somebody is there.
//!
//! Smithay holds one timer per notification a client asked for, and resets
//! every one of them each time the compositor reports activity. A pointer
//! crossing the screen reports hundreds of times a second, and each report
//! removes and re-inserts every timer, so the compositor folds a run of
//! input into one report.
//!
//! The fold is bounded by [`REPORT_EVERY`]: a report goes out on the first
//! input after that long with none, which is the resolution a lock rule
//! needs, and the timers keep the rest.

use std::time::{Duration, Instant};

/// How long a run of input is folded into one report.
///
/// An `ext-idle-notify` client asks for a timeout in milliseconds and the
/// shortest a lock rule uses is seconds, so folding a tenth of a second of
/// input costs nothing a client can see.
pub const REPORT_EVERY: Duration = Duration::from_millis(100);

/// The last time the compositor reported activity.
#[derive(Clone, Copy, Debug)]
pub struct Activity {
    every: Duration,
    last: Option<Instant>,
}

impl Activity {
    /// An activity record that reports at most once every `every`.
    pub fn new(every: Duration) -> Self {
        Self { every, last: None }
    }

    /// Whether input at `now` is worth reporting. The first input always
    /// is, and the next one is once the fold has run out.
    pub fn report(&mut self, now: Instant) -> bool {
        let due = match self.last {
            None => true,
            Some(last) => now.saturating_duration_since(last) >= self.every,
        };
        if due {
            self.last = Some(now);
        }
        due
    }
}

impl Default for Activity {
    fn default() -> Self {
        Self::new(REPORT_EVERY)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_input_is_reported() {
        let mut activity = Activity::default();
        assert!(activity.report(Instant::now()));
    }

    #[test]
    fn a_run_of_input_inside_the_fold_is_one_report() {
        let start = Instant::now();
        let mut activity = Activity::new(Duration::from_millis(100));
        assert!(activity.report(start));
        for step in 1..10 {
            let now = start + Duration::from_millis(step * 10);
            assert!(!activity.report(now), "input at {step} reported twice");
        }
    }

    #[test]
    fn input_after_the_fold_is_reported_again() {
        let start = Instant::now();
        let mut activity = Activity::new(Duration::from_millis(100));
        assert!(activity.report(start));
        assert!(!activity.report(start + Duration::from_millis(99)));
        assert!(activity.report(start + Duration::from_millis(100)));
        assert!(!activity.report(start + Duration::from_millis(150)));
        assert!(activity.report(start + Duration::from_millis(200)));
    }
}
