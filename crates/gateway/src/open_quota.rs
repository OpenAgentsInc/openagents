//! The decision worker's open lane: the one bound it always keeps, and an
//! off-by-default emergency brake.
//!
//! Coder on a computer with no TypeSafe key signs its Jev judgments with a
//! key nobody has seen before, so the hosted decision worker admits any key
//! on its open lane. The owner decided on 2026-10-01 (#10120, #10121) that
//! nothing in the app has a usage limit, so the shipped configuration sets
//! no count: every job is answered and recorded in the usage log
//! ([`crate::decision_usage`]) instead. This is the chat worker's design
//! (`coder::relay::quota`; this crate does not depend on `coder`, so the
//! same design is kept here):
//!
//! - a request whose ciphertext is longer than `max_request_bytes` is
//!   refused before anything is counted (it would not fit a relay event);
//! - for an abuse emergency only, an operator may set `per_key_minute`,
//!   `per_key_day`, or `total_day`. Each is unlimited unless named.
//!
//! A job counts when it is admitted, whether or not the door answers. The
//! day's counts are written to `file` after every admission, so a restart
//! does not reset an emergency brake; the per-minute window is kept in
//! memory only. A file that exists but does not read stops the worker.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Seconds in a UTC day.
const DAY: u64 = 86_400;
/// The per-key rate window, in seconds.
const MINUTE: u64 = 60;

/// The open lane's bounds, as `decision-worker.json` names them: the
/// request size always, and counts only when an operator names them.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// Emergency brake: jobs one caller key may start in a UTC day.
    /// Absent, the default, is unlimited.
    #[serde(default)]
    pub per_key_day: Option<u32>,
    /// Emergency brake: jobs one caller key may start in any sixty
    /// seconds. Absent, the default, is unlimited.
    #[serde(default)]
    pub per_key_minute: Option<u32>,
    /// Emergency brake: jobs every caller together may start in a UTC day.
    /// Absent, the default, is unlimited.
    #[serde(default)]
    pub total_day: Option<u32>,
    /// The longest request content, in bytes of NIP-44 ciphertext.
    #[serde(default = "default_request_bytes")]
    pub max_request_bytes: usize,
    /// Where the day's counts are kept across restarts.
    #[serde(default)]
    pub file: Option<PathBuf>,
}

fn default_request_bytes() -> usize {
    256 * 1024
}

impl Default for Policy {
    /// No count at all: the shipped behavior. Only the size bound holds.
    fn default() -> Self {
        Self {
            per_key_day: None,
            per_key_minute: None,
            total_day: None,
            max_request_bytes: default_request_bytes(),
            file: None,
        }
    }
}

impl Policy {
    /// Whether any job count is set (an emergency brake is on).
    #[must_use]
    pub fn counts(&self) -> bool {
        self.per_key_day.is_some() || self.per_key_minute.is_some() || self.total_day.is_some()
    }

    /// The startup log's words for the lane's bounds: `no usage limit`, or
    /// the brake an operator set.
    #[must_use]
    pub fn describe(&self) -> String {
        if !self.counts() {
            return "no usage limit".to_string();
        }
        let part = |value: Option<u32>| value.map_or_else(|| "any".to_string(), |n| n.to_string());
        format!(
            "emergency brake on: {}/key/day {}/key/min {}/day total",
            part(self.per_key_day),
            part(self.per_key_minute),
            part(self.total_day)
        )
    }

    /// Every count that is set positive, and no key's allowance larger
    /// than the whole when both are set: a typo in an emergency brake must
    /// stop the worker, not loosen it.
    ///
    /// # Errors
    ///
    /// A message naming the limit that is out of shape.
    pub fn check(&self) -> Result<(), String> {
        if [self.per_key_day, self.per_key_minute, self.total_day].contains(&Some(0)) {
            return Err("a count that is set must be positive; leave it out for no limit".into());
        }
        if self.max_request_bytes == 0 {
            return Err("max_request_bytes must be positive".into());
        }
        if let (Some(day), Some(total)) = (self.per_key_day, self.total_day)
            && day > total
        {
            return Err("per_key_day cannot exceed total_day".into());
        }
        if let (Some(minute), Some(day)) = (self.per_key_minute, self.per_key_day)
            && minute > day
        {
            return Err("per_key_minute cannot exceed per_key_day".into());
        }
        Ok(())
    }
}

/// Why a metered job was not admitted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The request is larger than the policy allows; waiting does not help.
    TooLarge { bytes: usize, limit: usize },
    /// The caller sent too many jobs in the last minute.
    RateLimited { retry_after_ms: u64 },
    /// The caller, or every caller together, used the day's jobs.
    Exhausted { retry_after_ms: u64, everyone: bool },
}

impl Refusal {
    /// The NIP-CJ refusal code.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Refusal::TooLarge { .. } => "limit_exceeded",
            Refusal::RateLimited { .. } => "rate_limited",
            Refusal::Exhausted { .. } => "quota_exhausted",
        }
    }

    /// The wait before a job would be admitted, when waiting helps.
    #[must_use]
    pub fn retry_after_ms(&self) -> Option<u64> {
        match self {
            Refusal::TooLarge { .. } => None,
            Refusal::RateLimited { retry_after_ms } | Refusal::Exhausted { retry_after_ms, .. } => {
                Some(*retry_after_ms)
            }
        }
    }

    /// The message the refusal carries. An emergency brake's refusal
    /// names no count: it says only to try again later.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Refusal::TooLarge { bytes, limit } => {
                format!("The request is {bytes} bytes; this worker takes at most {limit}.")
            }
            Refusal::RateLimited { .. } | Refusal::Exhausted { .. } => BRAKED.to_string(),
        }
    }
}

/// What an emergency brake's refusal says.
pub const BRAKED: &str = "This worker can't take this job right now. Try again later.";

/// What the file keeps: the UTC day, the day's total, and each key's count.
#[derive(Default, Serialize, Deserialize)]
struct Saved {
    day: u64,
    total: u32,
    keys: BTreeMap<String, u32>,
}

/// The counts the open lane admits jobs against.
pub struct Ledger {
    policy: Policy,
    day: u64,
    total: u32,
    per_day: HashMap<String, u32>,
    recent: HashMap<String, VecDeque<u64>>,
}

impl Ledger {
    /// The ledger for `policy`, with today's counts from its file when it
    /// holds today's.
    ///
    /// # Errors
    ///
    /// The policy is out of shape, or its file exists but does not read.
    pub fn open(policy: Policy, now: u64) -> Result<Self, String> {
        policy.check()?;
        let saved = match &policy.file {
            Some(path) if path.exists() => {
                let text = std::fs::read_to_string(path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                serde_json::from_str::<Saved>(&text)
                    .map_err(|error| format!("{}: {error}", path.display()))?
            }
            _ => Saved::default(),
        };
        let day = now / DAY;
        let (total, per_day) = if saved.day == day {
            (saved.total, saved.keys.into_iter().collect())
        } else {
            (0, HashMap::new())
        };
        Ok(Self {
            policy,
            day,
            total,
            per_day,
            recent: HashMap::new(),
        })
    }

    /// The limits this ledger admits under.
    #[must_use]
    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// Admit one job for `key` whose request is `bytes` long, or say why
    /// not. Nothing is counted for a refused job.
    ///
    /// # Errors
    ///
    /// The [`Refusal`] the caller is owed.
    pub fn admit(&mut self, key: &str, bytes: usize, now: u64) -> Result<(), Refusal> {
        if bytes > self.policy.max_request_bytes {
            return Err(Refusal::TooLarge {
                bytes,
                limit: self.policy.max_request_bytes,
            });
        }
        let day = now / DAY;
        if day != self.day {
            self.day = day;
            self.total = 0;
            self.per_day.clear();
        }
        let until_tomorrow = ((day + 1) * DAY).saturating_sub(now).max(1) * 1_000;
        if self
            .policy
            .total_day
            .is_some_and(|total| self.total >= total)
        {
            return Err(Refusal::Exhausted {
                retry_after_ms: until_tomorrow,
                everyone: true,
            });
        }
        if self
            .policy
            .per_key_day
            .is_some_and(|day| self.per_day.get(key).copied().unwrap_or(0) >= day)
        {
            return Err(Refusal::Exhausted {
                retry_after_ms: until_tomorrow,
                everyone: false,
            });
        }
        if let Some(minute) = self.policy.per_key_minute {
            let window = self.recent.entry(key.to_string()).or_default();
            while window
                .front()
                .is_some_and(|at| now.saturating_sub(*at) >= MINUTE)
            {
                window.pop_front();
            }
            if window.len() >= minute as usize {
                let oldest = window.front().copied().unwrap_or(now);
                let wait = (oldest + MINUTE).saturating_sub(now).max(1);
                return Err(Refusal::RateLimited {
                    retry_after_ms: wait * 1_000,
                });
            }
            window.push_back(now);
            self.recent.retain(|_, window| {
                window
                    .back()
                    .is_some_and(|at| now.saturating_sub(*at) < MINUTE)
            });
        }
        *self.per_day.entry(key.to_string()).or_default() += 1;
        self.total += 1;
        self.save();
        Ok(())
    }

    fn save(&self) {
        let Some(path) = &self.policy.file else {
            return;
        };
        let saved = Saved {
            day: self.day,
            total: self.total,
            keys: self
                .per_day
                .iter()
                .map(|(key, count)| (key.clone(), *count))
                .collect(),
        };
        if let Err(error) = write_atomically(path, &saved) {
            eprintln!(
                "decision-worker: quota: could not save {}: {error}",
                path.display()
            );
        }
    }
}

fn write_atomically(path: &Path, saved: &Saved) -> Result<(), String> {
    let text = serde_json::to_string(saved).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    std::fs::write(&temporary, text).map_err(|error| error.to_string())?;
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: u64 = 20_000 * DAY + DAY / 2;

    /// An emergency brake, as an operator would set one.
    fn policy(file: Option<PathBuf>) -> Policy {
        Policy {
            per_key_day: Some(5),
            per_key_minute: Some(2),
            total_day: Some(7),
            max_request_bytes: 100,
            file,
        }
    }

    #[test]
    fn loose_limits_stop_the_worker() {
        let mut loose = policy(None);
        loose.per_key_day = Some(8);
        assert!(Ledger::open(loose, NOON).is_err());
        let mut zero = policy(None);
        zero.total_day = Some(0);
        assert!(Ledger::open(zero, NOON).is_err());
    }

    /// The shipped lane (owner decision, 2026-10-01, #10121): no count, so
    /// one key's thousands of jobs in a minute and the day are all
    /// admitted; only the size bound refuses.
    #[test]
    fn with_no_brake_the_open_lane_admits_every_job() {
        let shipped: Policy = serde_json::from_str(r#"{"max_request_bytes": 100}"#).unwrap();
        assert_eq!(shipped.describe(), "no usage limit");
        let mut ledger = Ledger::open(shipped, NOON).unwrap();
        for at in 0..25_000 {
            ledger.admit("a", 10, NOON + at / 1_000).unwrap();
        }
        ledger.admit("b", 10, NOON).unwrap();
        assert_eq!(ledger.total, 25_001);
        assert_eq!(
            ledger.admit("a", 101, NOON).unwrap_err().code(),
            "limit_exceeded"
        );
        assert_eq!(Policy::default().describe(), "no usage limit");
    }

    #[test]
    fn a_key_is_rate_limited_then_capped_and_the_total_caps_everyone() {
        let mut ledger = Ledger::open(policy(None), NOON).unwrap();
        assert_eq!(
            ledger.admit("a", 101, NOON).unwrap_err().code(),
            "limit_exceeded"
        );
        ledger.admit("a", 10, NOON).unwrap();
        ledger.admit("a", 10, NOON).unwrap();
        let limited = ledger.admit("a", 10, NOON + 1).unwrap_err();
        assert_eq!(limited.code(), "rate_limited");
        assert_eq!(limited.message(), BRAKED);
        assert_eq!(limited.retry_after_ms(), Some(59_000));
        for minute in 1..=3 {
            ledger.admit("a", 10, NOON + 60 * minute).unwrap();
        }
        let exhausted = ledger.admit("a", 10, NOON + 600).unwrap_err();
        assert_eq!(exhausted.code(), "quota_exhausted");
        assert_eq!(exhausted.retry_after_ms(), Some((DAY / 2 - 600) * 1_000));
        ledger.admit("b", 10, NOON + 600).unwrap();
        ledger.admit("c", 10, NOON + 600).unwrap();
        assert_eq!(
            ledger.admit("d", 10, NOON + 600).unwrap_err(),
            Refusal::Exhausted {
                retry_after_ms: (DAY / 2 - 600) * 1_000,
                everyone: true
            }
        );
        // A new UTC day starts over.
        ledger.admit("a", 10, NOON + DAY).unwrap();
    }

    #[test]
    fn a_restart_keeps_the_day_and_a_broken_file_stops_it() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("quota.json");
        let mut ledger = Ledger::open(policy(Some(file.clone())), NOON).unwrap();
        for key in ["a", "b", "c", "d", "e", "f", "g"] {
            ledger.admit(key, 1, NOON).unwrap();
        }
        let mut again = Ledger::open(policy(Some(file.clone())), NOON + 5).unwrap();
        assert_eq!(
            again.admit("h", 1, NOON + 5).unwrap_err().code(),
            "quota_exhausted"
        );
        std::fs::write(&file, "not json").unwrap();
        assert!(Ledger::open(policy(Some(file)), NOON).is_err());
    }
}
