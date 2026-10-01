//! The day's counts: runs per trainer, and agent turns for everyone
//! together, per UTC day, kept in one file so a restart doesn't start a
//! second day.
//!
//! There is no usage limit (owner decision, 2026-10-01, #10120 and
//! #10121): with the shipped configuration [`Quota::take`] never refuses,
//! and every job is recorded in the usage log ([`crate::usage`]). The
//! counts are kept so an operator's emergency brake
//! ([`Limits::runs_per_trainer`], [`Limits::turns_per_day`], both unset by
//! default) holds across a restart when one is set.
//!
//! A suite run takes one of the trainer's runs and `cases × runs × arms`
//! turns from the day's total. A check (a rerun of someone's published
//! result) takes turns but none of the trainer's runs. A run refused
//! before it started gives its share back.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Refusal;
use crate::config::Limits;

/// Seconds in a day.
const DAY: u64 = 86_400;

/// What an emergency brake's refusal says: no count, no reset time.
pub const BRAKED: &str = "the hosted runner can't take this run right now; try again later";

/// One UTC day's counts.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Day {
    /// Days since the Unix epoch.
    pub day: u64,
    /// Suite runs per trainer.
    pub runs: BTreeMap<String, u32>,
    /// Agent turns taken.
    pub turns: u64,
}

/// What one admission took, to give back on a refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ticket {
    day: u64,
    trainer: String,
    run: bool,
    turns: u64,
}

/// The quota, backed by a file.
#[derive(Debug)]
pub struct Quota {
    path: PathBuf,
    day: Day,
}

impl Quota {
    /// The quota in `path`, or an empty one.
    #[must_use]
    pub fn open(path: PathBuf) -> Self {
        let day = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self { path, day }
    }

    /// Today's counts, as of `now`.
    #[must_use]
    pub fn today(&self, now: u64) -> Day {
        if self.day.day == now / DAY {
            self.day.clone()
        } else {
            Day {
                day: now / DAY,
                ..Day::default()
            }
        }
    }

    /// Takes one run for `trainer` (none for a `check`) and `turns` turns,
    /// as of `now`.
    ///
    /// # Errors
    ///
    /// `over_quota` only when an emergency brake is set and spent; nothing
    /// is taken then. The refusal names no count: the phone shows it as
    /// "try again later".
    pub fn take(
        &mut self,
        trainer: &str,
        turns: u64,
        check: bool,
        now: u64,
        limits: &Limits,
    ) -> Result<Ticket, Refusal> {
        let mut day = self.today(now);
        let used = day.runs.get(trainer).copied().unwrap_or(0);
        let braked = (!check && limits.runs_per_trainer.is_some_and(|runs| used >= runs))
            || limits
                .turns_per_day
                .is_some_and(|ceiling| day.turns + turns > ceiling);
        if braked {
            return Err(Refusal::over_quota(BRAKED));
        }
        if !check {
            day.runs.insert(trainer.to_string(), used + 1);
        }
        day.turns += turns;
        self.day = day;
        self.save();
        Ok(Ticket {
            day: now / DAY,
            trainer: trainer.to_string(),
            run: !check,
            turns,
        })
    }

    /// Gives back what `ticket` took, if it's still the same day.
    pub fn give_back(&mut self, ticket: &Ticket) {
        if self.day.day != ticket.day {
            return;
        }
        if ticket.run
            && let Some(runs) = self.day.runs.get_mut(&ticket.trainer)
        {
            *runs = runs.saturating_sub(1);
        }
        self.day.turns = self.day.turns.saturating_sub(ticket.turns);
        self.save();
    }

    fn save(&self) {
        if let Ok(bytes) = serde_json::to_vec_pretty(&self.day)
            && let Err(error) = crate::write_atomic(&self.path, &bytes)
        {
            eprintln!("quota: {}: {error}", self.path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: u64 = 20_000 * DAY + 43_200;

    /// The shipped configuration counts but never refuses: a trainer's
    /// fourth, tenth, and hundredth run of a day are admitted.
    #[test]
    fn with_no_brake_a_trainer_runs_as_often_as_they_like() {
        let dir = tempfile::tempdir().unwrap();
        let mut quota = Quota::open(dir.path().join("quota.json"));
        let limits = Limits::default();
        assert_eq!(
            (limits.runs_per_trainer, limits.turns_per_day),
            (None, None)
        );
        for _ in 0..100 {
            quota.take("alice", 48, false, NOON, &limits).unwrap();
        }
        assert_eq!(quota.today(NOON).runs["alice"], 100);
        assert_eq!(quota.today(NOON).turns, 4_800);
    }

    /// An emergency brake, when an operator sets one: three runs a day per
    /// trainer; a check takes turns but no run; the day's turns cap
    /// everyone; the refusal names no count; a new UTC day starts over; a
    /// refund gives back exactly what was taken; and the counts survive a
    /// restart.
    #[test]
    fn an_emergency_brake_holds_and_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let limits = Limits {
            runs_per_trainer: Some(3),
            turns_per_day: Some(200),
            ..Limits::default()
        };
        let mut quota = Quota::open(path.clone());
        for _ in 0..3 {
            quota.take("alice", 48, false, NOON, &limits).unwrap();
        }
        let refused = quota.take("alice", 6, false, NOON, &limits).unwrap_err();
        assert_eq!(refused.code, "over_quota");
        assert_eq!(refused.message, BRAKED);
        assert!(!refused.message.chars().any(|c| c.is_ascii_digit()));
        // A check isn't a run.
        let check = quota.take("alice", 6, true, NOON, &limits).unwrap();
        assert_eq!(quota.today(NOON).runs["alice"], 3);
        // The day's turns: 150 used, 50 left.
        assert_eq!(
            quota
                .take("bob", 51, false, NOON, &limits)
                .unwrap_err()
                .code,
            "over_quota"
        );
        let bob = quota.take("bob", 50, false, NOON, &limits).unwrap();
        quota.give_back(&bob);
        quota.give_back(&check);
        assert_eq!(quota.today(NOON).turns, 144);
        assert_eq!(quota.today(NOON).runs.get("bob"), Some(&0));
        // A restart reads the same day.
        let reopened = Quota::open(path);
        assert_eq!(reopened.today(NOON), quota.today(NOON));
        // Tomorrow starts over.
        let mut tomorrow = reopened;
        tomorrow
            .take("alice", 48, false, NOON + DAY, &limits)
            .unwrap();
        assert_eq!(tomorrow.today(NOON + DAY).runs["alice"], 1);
    }
}
