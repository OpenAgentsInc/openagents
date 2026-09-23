//! One monotonic episode deadline, shared by everything that waits.
//!
//! The episode starts the clock once. Each dispatch, Jev request, retry,
//! wait, setup command, probe, and check asks the deadline for its
//! allowance before it starts: the smaller of what it asked for and what
//! is left once the reserve for final checks, cleanup, and recording is
//! kept back. A grant shorter than the request is a cut, and a request
//! with nothing left is skipped; the deadline records both, so the episode
//! record says what was cut short and why.
//!
//! An episode with no deadline (the policy's
//! `protected.ceilings.episode_deadline_sec` is `null`) grants every
//! request in full, and the harness's own timeout stays the outer limit.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The least time worth starting something with. A request left with
/// less is skipped rather than started to be killed.
pub const MINIMUM_GRANT: Duration = Duration::from_secs(1);

/// One request the deadline shortened or refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cut {
    /// What asked, such as `delegate-1` or `jev_survey`.
    pub what: String,
    /// When it asked, in milliseconds since the episode started.
    pub at_ms: u64,
    pub requested_ms: u64,
    /// What it got; zero when it was skipped.
    pub granted_ms: u64,
}

impl Cut {
    /// Whether the request was skipped outright.
    #[must_use]
    pub fn skipped(&self) -> bool {
        self.granted_ms == 0
    }
}

struct Inner {
    started: Instant,
    total: Option<Duration>,
    reserve: Duration,
    cuts: RefCell<Vec<Cut>>,
}

/// The episode's deadline. Clones share one clock and one record.
#[derive(Clone)]
pub struct Deadline(Rc<Inner>);

impl Default for Deadline {
    fn default() -> Self {
        Self::unbounded()
    }
}

impl std::fmt::Debug for Deadline {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deadline")
            .field("total", &self.0.total)
            .field("reserve", &self.0.reserve)
            .field("elapsed", &self.elapsed())
            .finish()
    }
}

fn ms(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

impl Deadline {
    /// A deadline `total` from now, keeping `reserve` back at the end.
    #[must_use]
    pub fn new(total: Option<Duration>, reserve: Duration) -> Self {
        Self::starting(Instant::now(), total, reserve)
    }

    /// A deadline whose clock started at `started`.
    #[must_use]
    pub fn starting(started: Instant, total: Option<Duration>, reserve: Duration) -> Self {
        Deadline(Rc::new(Inner {
            started,
            total,
            reserve,
            cuts: RefCell::new(Vec::new()),
        }))
    }

    /// No deadline: every request is granted in full.
    #[must_use]
    pub fn unbounded() -> Self {
        Self::new(None, Duration::ZERO)
    }

    /// Whether the episode has a deadline at all.
    #[must_use]
    pub fn bounded(&self) -> bool {
        self.0.total.is_some()
    }

    /// Time since the episode started.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.0.started.elapsed()
    }

    /// Time left before the deadline itself, reserve included.
    #[must_use]
    pub fn remaining(&self) -> Option<Duration> {
        self.0
            .total
            .map(|total| total.saturating_sub(self.elapsed()))
    }

    /// Time left for work: what remains less the reserve.
    #[must_use]
    pub fn allowance(&self) -> Option<Duration> {
        self.remaining()
            .map(|remaining| remaining.saturating_sub(self.0.reserve))
    }

    /// Grants `what` at most `requested`, bounded by the allowance. `None`
    /// when less than [`MINIMUM_GRANT`] is left, and the request should
    /// not start. A shortened or refused request is recorded.
    pub fn grant(&self, what: &str, requested: Duration) -> Option<Duration> {
        let Some(allowance) = self.allowance() else {
            return Some(requested);
        };
        if requested <= allowance {
            return Some(requested);
        }
        let granted = if allowance >= MINIMUM_GRANT.min(requested) {
            Some(allowance)
        } else {
            None
        };
        self.0.cuts.borrow_mut().push(Cut {
            what: what.to_string(),
            at_ms: ms(self.elapsed()),
            requested_ms: ms(requested),
            granted_ms: granted.map_or(0, ms),
        });
        granted
    }

    /// Every cut so far, in order.
    #[must_use]
    pub fn cuts(&self) -> Vec<Cut> {
        self.0.cuts.borrow().clone()
    }

    /// The deadline as the episode manifest records it.
    #[must_use]
    pub fn record(&self) -> Value {
        let cuts: Vec<Value> = self
            .cuts()
            .iter()
            .map(|cut| {
                json!({
                    "what": cut.what,
                    "at_ms": cut.at_ms,
                    "requested_ms": cut.requested_ms,
                    "granted_ms": cut.granted_ms,
                    "skipped": cut.skipped(),
                })
            })
            .collect();
        match self.0.total {
            None => json!({
                "kind": "none",
                "note": "no episode deadline: the harness's exec timeout is the only limit",
                "elapsed_ms": ms(self.elapsed()),
                "cuts": cuts,
            }),
            Some(total) => json!({
                "kind": "hard",
                "total_ms": ms(total),
                "reserve_ms": ms(self.0.reserve),
                "elapsed_ms": ms(self.elapsed()),
                "remaining_ms": self.remaining().map(ms),
                "used_fraction": self.elapsed().as_secs_f64() / total.as_secs_f64().max(f64::MIN_POSITIVE),
                "cuts": cuts,
                "note": "one monotonic deadline from the episode's start; every dispatch, Jev request, retry, wait, setup command, probe, and check is granted at most what is left after the reserve",
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unbounded_deadline_grants_everything_and_records_nothing() {
        let deadline = Deadline::unbounded();
        assert_eq!(
            deadline.grant("delegate-1", Duration::from_secs(600)),
            Some(Duration::from_secs(600))
        );
        assert!(deadline.cuts().is_empty());
        assert_eq!(deadline.record()["kind"], "none");
    }

    #[test]
    fn a_request_longer_than_the_allowance_is_cut_and_recorded() {
        let started = Instant::now() - Duration::from_secs(100);
        let deadline = Deadline::starting(
            started,
            Some(Duration::from_secs(400)),
            Duration::from_secs(30),
        );
        // About 270 seconds are left for work.
        let granted = deadline
            .grant("delegate-1", Duration::from_secs(600))
            .unwrap();
        assert!(granted <= Duration::from_secs(270) && granted > Duration::from_secs(260));
        assert_eq!(
            deadline.grant("jev_close", Duration::from_secs(60)),
            Some(Duration::from_secs(60))
        );
        let cuts = deadline.cuts();
        assert_eq!(cuts.len(), 1);
        assert_eq!(cuts[0].what, "delegate-1");
        assert_eq!(cuts[0].requested_ms, 600_000);
        assert!(!cuts[0].skipped());
        let record = deadline.record();
        assert_eq!(record["kind"], "hard");
        assert_eq!(record["reserve_ms"], 30_000);
        assert_eq!(record["cuts"][0]["what"], "delegate-1");
    }

    #[test]
    fn nothing_starts_inside_the_reserve() {
        let started = Instant::now() - Duration::from_secs(95);
        let deadline = Deadline::starting(
            started,
            Some(Duration::from_secs(120)),
            Duration::from_secs(30),
        );
        assert_eq!(deadline.allowance(), Some(Duration::ZERO));
        assert_eq!(deadline.grant("jev_step", Duration::from_secs(60)), None);
        assert!(deadline.cuts()[0].skipped());
        assert_eq!(deadline.record()["cuts"][0]["skipped"], true);
    }
}
