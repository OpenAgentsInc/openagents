//! Injected monotonic time.
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// A point on a monotonic clock, in milliseconds from the clock's origin.
///
/// Moments from different clocks are not comparable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Moment(pub u64);

impl Moment {
    /// Returns the moment `duration` later, saturating at the end of the range.
    pub fn after(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(millis(duration)))
    }

    /// Returns the time from `earlier` to this moment, or zero if `earlier`
    /// is later.
    pub fn since(self, earlier: Moment) -> Duration {
        Duration::from_millis(self.0.saturating_sub(earlier.0))
    }
}

pub(crate) fn millis(duration: Duration) -> u64 {
    u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
}

/// A monotonic time source. The registry reads it on every signal, report,
/// and tick, so tests can advance time without sleeping.
pub trait Clock {
    /// Returns the current moment. Successive calls never go backward.
    fn now(&self) -> Moment;
}

/// A clock that moves only when a test advances it. Clones share one time.
#[derive(Clone, Debug, Default)]
pub struct ManualClock(Arc<AtomicU64>);

impl ManualClock {
    /// Creates a clock at `start`.
    pub fn new(start: Moment) -> Self {
        Self(Arc::new(AtomicU64::new(start.0)))
    }

    /// Moves the clock forward by `duration`.
    pub fn advance(&self, duration: Duration) {
        let step = millis(duration);
        let _ = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |now| {
                Some(now.saturating_add(step))
            });
    }
}

impl Clock for ManualClock {
    fn now(&self) -> Moment {
        Moment(self.0.load(Ordering::SeqCst))
    }
}

/// The process's monotonic clock, measured from when this value was created.
#[derive(Clone, Copy, Debug)]
pub struct SystemClock(Instant);

impl SystemClock {
    /// Starts a clock whose origin is now.
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Moment {
        Moment(millis(self.0.elapsed()))
    }
}
