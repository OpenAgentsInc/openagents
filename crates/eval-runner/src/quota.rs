//! The daily quota: runs per trainer, and agent turns for everyone
//! together, per UTC day, kept in one file so a restart doesn't start a
//! second day.
//!
//! A suite run takes one of the trainer's runs and `cases × runs × arms`
//! turns from the day's total. A check (a rerun of someone's published
//! result) takes turns but none of the trainer's runs: checking others is
//! what the Gym wants more of. A run refused before it started gives its
//! share back.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Refusal;
use crate::config::Limits;

/// Seconds in a day.
const DAY: u64 = 86_400;

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
    /// `over_quota` when the trainer's runs or the day's turns are spent;
    /// nothing is taken then.
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
        if !check && used >= limits.runs_per_trainer {
            return Err(Refusal::over_quota(format!(
                "you've used today's {} test runs; they reset at midnight UTC, and checking someone else's result doesn't count",
                limits.runs_per_trainer
            )));
        }
        if day.turns + turns > limits.turns_per_day {
            return Err(Refusal::over_quota(
                "the hosted runner has used today's budget; it resets at midnight UTC",
            ));
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

    /// Three runs a day per trainer; a check takes turns but no run; the
    /// day's turns cap everyone; a new UTC day starts over; a refund gives
    /// back exactly what was taken; and the counts survive a restart.
    #[test]
    fn runs_per_trainer_and_turns_per_day_are_held_and_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let limits = Limits {
            runs_per_trainer: 3,
            turns_per_day: 200,
            ..Limits::default()
        };
        let mut quota = Quota::open(path.clone());
        for _ in 0..3 {
            quota.take("alice", 48, false, NOON, &limits).unwrap();
        }
        let refused = quota.take("alice", 6, false, NOON, &limits).unwrap_err();
        assert_eq!(refused.code, "over_quota");
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
