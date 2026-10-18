//! The quiet moment: the app restarts only when the owner isn't typing and
//! no voice call is live, and only after a notice has been up for a while
//! with the owner still away.

use std::time::{Duration, Instant};

/// How long since the last keystroke counts as not typing.
pub const IDLE: Duration = Duration::from_secs(30);
/// How long the notice is up before the restart.
pub const NOTICE: Duration = Duration::from_secs(10);

/// What the app sees of the owner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Activity {
    /// The last keystroke in any text field.
    pub last_input: Option<Instant>,
    /// A voice call (or dictation) is under way.
    pub voice_live: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Not yet: the owner is busy, or the notice is still up.
    Wait,
    /// Quiet: show the notice now.
    Notice,
    /// The owner came back while the notice was up: take it down, wait again.
    Cancel,
    /// Quiet through the notice: restart now.
    Restart,
}

#[derive(Debug, Clone)]
pub struct QuietWait {
    idle: Duration,
    notice: Duration,
    noticed_at: Option<Instant>,
}

impl Default for QuietWait {
    fn default() -> Self {
        QuietWait::new(IDLE, NOTICE)
    }
}

impl QuietWait {
    #[must_use]
    pub fn new(idle: Duration, notice: Duration) -> QuietWait {
        QuietWait {
            idle,
            notice,
            noticed_at: None,
        }
    }

    /// Whether the owner is away right now.
    #[must_use]
    pub fn quiet(&self, now: Instant, activity: &Activity) -> bool {
        !activity.voice_live
            && activity
                .last_input
                .is_none_or(|at| now.saturating_duration_since(at) >= self.idle)
    }

    /// The next step at `now`.
    pub fn step(&mut self, now: Instant, activity: &Activity) -> Step {
        let quiet = self.quiet(now, activity);
        match self.noticed_at {
            None if quiet => {
                self.noticed_at = Some(now);
                Step::Notice
            }
            None => Step::Wait,
            Some(_) if !quiet => {
                self.noticed_at = None;
                Step::Cancel
            }
            Some(at) if now.saturating_duration_since(at) >= self.notice => Step::Restart,
            Some(_) => Step::Wait,
        }
    }

    /// Whether the notice is up.
    #[must_use]
    pub fn noticing(&self) -> bool {
        self.noticed_at.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typing_and_calls_hold_the_restart_and_cancel_the_notice() {
        let start = Instant::now();
        let at = |secs: u64| start + Duration::from_secs(secs);
        let mut wait = QuietWait::default();
        let typing = |secs: u64| Activity {
            last_input: Some(at(secs)),
            voice_live: false,
        };

        // Typing a moment ago: wait.
        assert_eq!(wait.step(at(10), &typing(5)), Step::Wait);
        // 30 s after the last keystroke: the notice goes up.
        assert_eq!(wait.step(at(35), &typing(5)), Step::Notice);
        assert_eq!(wait.step(at(40), &typing(5)), Step::Wait);
        // A keystroke while the notice is up takes it down.
        assert_eq!(wait.step(at(41), &typing(41)), Step::Cancel);
        assert!(!wait.noticing());
        assert_eq!(wait.step(at(50), &typing(41)), Step::Wait);
        // Quiet again: a fresh notice, and the restart only after it has
        // been up for the full 10 s.
        assert_eq!(wait.step(at(71), &typing(41)), Step::Notice);
        assert_eq!(wait.step(at(80), &typing(41)), Step::Wait);
        assert_eq!(wait.step(at(81), &typing(41)), Step::Restart);

        // A voice call holds it however long the keyboard has been idle.
        let mut wait = QuietWait::default();
        let call = Activity {
            last_input: None,
            voice_live: true,
        };
        assert_eq!(wait.step(at(1000), &call), Step::Wait);
        assert_eq!(wait.step(at(2000), &Activity::default()), Step::Notice);
        assert_eq!(wait.step(at(2005), &call), Step::Cancel);
        assert_eq!(wait.step(at(2100), &call), Step::Wait);
    }
}
