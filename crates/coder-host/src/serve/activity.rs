//! Whether the host is in use, so it starts again for an update only when
//! nobody would notice (2026-10-02: a rebuild restarted the host while the
//! owner's message was in flight, and the terminal said it could not reach
//! OpenAgents).
//!
//! In use means a request is being answered on any path (the control
//! socket, a relay, a direct channel), one finished within [`QUIET`] (a
//! client following a reply or a Coder run asks every fraction of a
//! second, so the gaps between its reads are not idle), a chat reply is
//! streaming, or a device holds a terminal.

use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::time::Instant;

/// How long after the last request the host still counts as in use.
pub(crate) const QUIET: Duration = Duration::from_secs(2);
/// How often a wait for idle looks again.
pub(crate) const LOOK_EVERY: Duration = Duration::from_millis(250);

/// The requests in flight and when the last one ended.
#[derive(Debug)]
pub(crate) struct Activity {
    in_flight: AtomicUsize,
    last: Mutex<Instant>,
}

impl Default for Activity {
    fn default() -> Self {
        Self {
            in_flight: AtomicUsize::new(0),
            // A host that just started has served nobody yet.
            last: Mutex::new(
                Instant::now()
                    .checked_sub(QUIET)
                    .unwrap_or_else(Instant::now),
            ),
        }
    }
}

impl Activity {
    /// A request begins; it ends when the guard drops.
    pub(crate) fn begin(&self) -> Busy<'_> {
        self.in_flight.fetch_add(1, Ordering::SeqCst);
        Busy(self)
    }

    /// Requests being answered now.
    pub(crate) fn in_flight(&self) -> usize {
        self.in_flight.load(Ordering::SeqCst)
    }

    /// No request in flight, and none ended within `quiet`.
    pub(crate) fn quiet_for(&self, quiet: Duration) -> bool {
        self.in_flight() == 0
            && self
                .last
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .elapsed()
                >= quiet
    }
}

/// One request being answered.
#[must_use]
pub(crate) struct Busy<'a>(&'a Activity);

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        *self
            .0
            .last
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = Instant::now();
        self.0.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn a_request_in_flight_and_the_quiet_after_it_are_in_use() {
        let activity = Activity::default();
        assert!(activity.quiet_for(QUIET), "a fresh host is idle");
        let busy = activity.begin();
        tokio::time::sleep(Duration::from_secs(10)).await;
        assert!(!activity.quiet_for(QUIET), "a long request is in use");
        drop(busy);
        assert!(!activity.quiet_for(QUIET), "just ended");
        tokio::time::sleep(QUIET).await;
        assert!(activity.quiet_for(QUIET));
    }
}
