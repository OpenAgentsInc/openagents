//! The decision worker's open lane quota: how a worker that answers keys
//! no operator provisioned spends its one server-held door key.
//!
//! Coder on a computer with no TypeSafe key signs its Jev judgments with a
//! key nobody has seen before, so the hosted decision worker admits any key
//! on its open lane — metered, the way the chat worker meters its callers
//! (`coder::relay::quota`; this crate does not depend on `coder`, so the
//! same design is kept here):
//!
//! - each caller key gets at most `per_key_minute` jobs in any sixty
//!   seconds and `per_key_day` jobs in a UTC day;
//! - every caller together gets at most `total_day` jobs in a UTC day,
//!   which bounds the day's spend however many keys a caller mints;
//! - a request whose ciphertext is longer than `max_request_bytes` is
//!   refused before anything is counted.
//!
//! A job counts when it is admitted, whether or not the door answers. The
//! day's counts are written to `file` after every admission, so a restart
//! does not hand out a second day; the per-minute window is kept in memory
//! only, so a restart resets at most one minute. A file that exists but
//! does not read stops the worker rather than loosening the limit.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Seconds in a UTC day.
const DAY: u64 = 86_400;
/// The per-key rate window, in seconds.
const MINUTE: u64 = 60;

/// The open lane's limits, as `decision-worker.json` names them.
#[derive(Clone, Debug, Deserialize, PartialEq, Eq)]
pub struct Policy {
    /// Jobs one caller key may start in a UTC day.
    pub per_key_day: u32,
    /// Jobs one caller key may start in any sixty seconds.
    pub per_key_minute: u32,
    /// Jobs every caller together may start in a UTC day.
    pub total_day: u32,
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

impl Policy {
    /// Every limit positive, and no key's allowance larger than the whole:
    /// a typo in the one setting that bounds the spend must stop the
    /// worker, not loosen it.
    ///
    /// # Errors
    ///
    /// A message naming the limit that is out of shape.
    pub fn check(&self) -> Result<(), String> {
        if self.per_key_day == 0 || self.per_key_minute == 0 || self.total_day == 0 {
            return Err("every quota limit must be positive".into());
        }
        if self.max_request_bytes == 0 {
            return Err("max_request_bytes must be positive".into());
        }
        if self.per_key_day > self.total_day {
            return Err("per_key_day cannot exceed total_day".into());
        }
        if self.per_key_minute > self.per_key_day {
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

    /// The message the refusal carries.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Refusal::TooLarge { bytes, limit } => {
                format!("The request is {bytes} bytes; this worker takes at most {limit}.")
            }
            Refusal::RateLimited { .. } => {
                "This key sent too many decision jobs in the last minute.".to_string()
            }
            Refusal::Exhausted {
                everyone: false, ..
            } => "This key used today's decision jobs on this worker.".to_string(),
            Refusal::Exhausted { everyone: true, .. } => {
                "This worker answered as many decision jobs as it takes today.".to_string()
            }
        }
    }
}

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
        if self.total >= self.policy.total_day {
            return Err(Refusal::Exhausted {
                retry_after_ms: until_tomorrow,
                everyone: true,
            });
        }
        if self.per_day.get(key).copied().unwrap_or(0) >= self.policy.per_key_day {
            return Err(Refusal::Exhausted {
                retry_after_ms: until_tomorrow,
                everyone: false,
            });
        }
        let window = self.recent.entry(key.to_string()).or_default();
        while window
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= MINUTE)
        {
            window.pop_front();
        }
        if window.len() >= self.policy.per_key_minute as usize {
            let oldest = window.front().copied().unwrap_or(now);
            let wait = (oldest + MINUTE).saturating_sub(now).max(1);
            return Err(Refusal::RateLimited {
                retry_after_ms: wait * 1_000,
            });
        }
        window.push_back(now);
        *self.per_day.entry(key.to_string()).or_default() += 1;
        self.total += 1;
        self.recent.retain(|_, window| {
            window
                .back()
                .is_some_and(|at| now.saturating_sub(*at) < MINUTE)
        });
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

    fn policy(file: Option<PathBuf>) -> Policy {
        Policy {
            per_key_day: 5,
            per_key_minute: 2,
            total_day: 7,
            max_request_bytes: 100,
            file,
        }
    }

    #[test]
    fn loose_limits_stop_the_worker() {
        let mut loose = policy(None);
        loose.per_key_day = 8;
        assert!(Ledger::open(loose, NOON).is_err());
        let mut zero = policy(None);
        zero.total_day = 0;
        assert!(Ledger::open(zero, NOON).is_err());
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
