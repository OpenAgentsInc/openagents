//! The feature-card carousel on a phone's new chat: which card it opens on
//! and when it moves on by itself. Rust picks the start card; the hosts
//! (`Shell.swift`, `Shell.kt`) run the same timing rules as [`AutoScroll`],
//! with the times the packet carries ([`Timing`]).
//!
//! - It opens on a random card, never the one it opened on last time.
//! - It moves one card every [`DWELL_MS`], smoothly, looping past the end.
//! - A touch or drag stops it; it starts again [`RESUME_MS`] after the
//!   finger lifts.
//! - With Reduce Motion on it never moves by itself.

use serde::Serialize;

/// How long each card stays before the carousel moves on, milliseconds.
pub const DWELL_MS: u64 = 3_500;
/// How long after a touch ends the carousel starts moving again.
pub const RESUME_MS: u64 = 4_000;

/// The carousel's start and timing, as the packet carries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct Timing {
    /// The card the carousel opens on, by its index in the cards.
    pub start: usize,
    pub dwell_ms: u64,
    pub resume_ms: u64,
}

/// The card to open on among `count`, never `last` when there is a
/// choice. `roll` is a random number; any value works.
#[must_use]
pub fn pick_start(count: usize, last: Option<usize>, roll: u64) -> usize {
    match (count, last) {
        (0, _) => 0,
        (1, _) => 0,
        (count, Some(last)) if last < count => {
            // One of the other `count - 1` cards, evenly.
            let pick = (roll % (count as u64 - 1)) as usize;
            if pick >= last { pick + 1 } else { pick }
        }
        (count, _) => (roll % count as u64) as usize,
    }
}

/// A fresh random roll for [`pick_start`].
#[must_use]
pub fn roll() -> u64 {
    use secp256k1::rand::RngCore;
    secp256k1::rand::rng().next_u64()
}

/// When the carousel moves on by itself, in milliseconds on any clock that
/// only goes forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AutoScroll {
    dwell_ms: u64,
    resume_ms: u64,
    reduce_motion: bool,
    touching: bool,
    /// When the card on view arrived, or when the pause after a touch ends.
    since: u64,
}

impl AutoScroll {
    #[must_use]
    pub fn new(now: u64, reduce_motion: bool) -> Self {
        Self {
            dwell_ms: DWELL_MS,
            resume_ms: RESUME_MS,
            reduce_motion,
            touching: false,
            since: now,
        }
    }

    /// A finger is down on the cards: nothing moves until it lifts.
    pub fn touch(&mut self) {
        self.touching = true;
    }

    /// The finger lifted at `now`: the next move waits [`RESUME_MS`] and
    /// then a full dwell on the card the person left on view.
    pub fn release(&mut self, now: u64) {
        self.touching = false;
        self.since = now.saturating_add(self.resume_ms);
    }

    /// Whether to move one card on at `now`; when it does, the dwell on the
    /// new card starts.
    pub fn step(&mut self, now: u64) -> bool {
        if self.reduce_motion || self.touching || now < self.since.saturating_add(self.dwell_ms) {
            return false;
        }
        self.since = now;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_start_is_never_the_last_one_and_reaches_every_other_card() {
        for last in 0..4 {
            let starts: std::collections::BTreeSet<_> = (0..64)
                .map(|roll| pick_start(4, Some(last), roll))
                .collect();
            assert!(!starts.contains(&last), "{last}: {starts:?}");
            assert_eq!(starts.len(), 3);
            assert!(starts.iter().all(|&start| start < 4));
        }
        // The first open may be any card.
        let first: std::collections::BTreeSet<_> =
            (0..64).map(|roll| pick_start(4, None, roll)).collect();
        assert_eq!(first.len(), 4);
        // No choice with one card; none at all without cards; a stale last
        // index is ignored.
        assert_eq!(pick_start(1, Some(0), 7), 0);
        assert_eq!(pick_start(0, None, 7), 0);
        assert!(pick_start(3, Some(9), 7) < 3);
    }

    #[test]
    fn it_moves_each_dwell_pauses_while_touched_and_resumes_after() {
        let mut auto = AutoScroll::new(0, false);
        assert!(!auto.step(DWELL_MS - 1));
        assert!(auto.step(DWELL_MS));
        assert!(!auto.step(DWELL_MS + 10));
        assert!(auto.step(2 * DWELL_MS));
        // A finger down stops it, however long it stays.
        auto.touch();
        assert!(!auto.step(10 * DWELL_MS));
        // Lifted at 10 dwells: nothing until the resume pause and a dwell.
        auto.release(10 * DWELL_MS);
        assert!(!auto.step(10 * DWELL_MS + RESUME_MS));
        assert!(!auto.step(10 * DWELL_MS + RESUME_MS + DWELL_MS - 1));
        assert!(auto.step(10 * DWELL_MS + RESUME_MS + DWELL_MS));
    }

    #[test]
    fn reduce_motion_never_moves_it() {
        let mut auto = AutoScroll::new(0, true);
        assert!(!auto.step(100 * DWELL_MS));
        auto.release(0);
        assert!(!auto.step(100 * DWELL_MS));
    }
}
