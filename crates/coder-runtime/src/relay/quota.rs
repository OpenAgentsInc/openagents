//! A conversation worker's optional abuse brake: how an open worker can be
//! slowed in an emergency, and the one bound it always keeps.
//!
//! The OpenAgents app's chat is open: a fresh install has a device key no
//! operator has seen, and it is answered at once, with no daily or
//! per-minute allowance. The owner decided on 2026-10-01 (#10120) that the
//! product shows no usage limit anywhere, so the shipped configuration sets
//! no count at all; every job is recorded in the usage log instead
//! ([`super::usage`]).
//!
//! What stays is a size bound and an off-by-default brake:
//!
//! - a request whose ciphertext is longer than [`Policy::max_request_bytes`]
//!   is refused before anything is counted (it would not fit a relay event
//!   anyway);
//! - for an abuse emergency only, an operator may set any of
//!   [`Policy::per_key_minute`], [`Policy::per_key_day`], and
//!   [`Policy::total_day`]. Each is `None` (unlimited) unless named.
//!
//! A refusal carries the typed code and the wait until a job would be
//! admitted. Clients never show a limit for it: they say only that
//! OpenAgents could not be reached. The day's counts are written to a file
//! after every admission when one is named and a day count is set, so a
//! restart does not reset an emergency brake.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Seconds in a UTC day.
const DAY: u64 = 86_400;
/// The per-key rate window, in seconds.
const MINUTE: u64 = 60;

/// An open worker's bounds: the request size always, and counts only when
/// an operator names them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Policy {
    /// Jobs one caller key may start in a UTC day; `None` is unlimited.
    pub per_key_day: Option<u32>,
    /// Jobs one caller key may start in any sixty seconds; `None` is
    /// unlimited.
    pub per_key_minute: Option<u32>,
    /// Jobs every caller together may start in a UTC day; `None` is
    /// unlimited.
    pub total_day: Option<u32>,
    /// The longest request content, in bytes of NIP-44 ciphertext.
    pub max_request_bytes: usize,
}

impl Policy {
    /// The largest content a request may carry when the configuration
    /// does not say: room for a long conversation, far below the relay's
    /// own event bound.
    pub const DEFAULT_REQUEST_BYTES: usize = 96 * 1024;

    /// No count at all: the shipped behavior. Only the size bound holds.
    pub const UNLIMITED: Policy = Policy {
        per_key_day: None,
        per_key_minute: None,
        total_day: None,
        max_request_bytes: Self::DEFAULT_REQUEST_BYTES,
    };

    /// Whether any job count is set (an emergency brake is on).
    #[must_use]
    pub fn counts(&self) -> bool {
        self.per_key_day.is_some() || self.per_key_minute.is_some() || self.total_day.is_some()
    }

    /// Reads `[day=N][,minute=N][,total=N][,bytes=N]`. Every part is
    /// optional; a part left out is unlimited (`bytes` defaults to
    /// [`Policy::DEFAULT_REQUEST_BYTES`]), and an empty text is
    /// [`Policy::UNLIMITED`]. A part that is named must be a positive whole
    /// number, and a key's day cannot exceed the total when both are set:
    /// a typo in an emergency brake must stop the worker, not loosen it.
    ///
    /// # Errors
    ///
    /// A message naming the part that did not parse.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut fields: BTreeMap<&str, u64> = BTreeMap::new();
        for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
            let (name, value) = part
                .split_once('=')
                .ok_or_else(|| format!("{part} is not name=value"))?;
            let name = name.trim();
            if !matches!(name, "day" | "minute" | "total" | "bytes") {
                return Err(format!(
                    "{name} is not a quota part (day, minute, total, bytes)"
                ));
            }
            let value: u64 = value
                .trim()
                .parse()
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| format!("{name} must be a positive whole number"))?;
            if fields.insert(name, value).is_some() {
                return Err(format!("{name} is given twice"));
            }
        }
        let count = |name: &str| -> Result<Option<u32>, String> {
            fields
                .get(name)
                .map(|value| u32::try_from(*value).map_err(|_| format!("{name} is too large")))
                .transpose()
        };
        let policy = Policy {
            per_key_day: count("day")?,
            per_key_minute: count("minute")?,
            total_day: count("total")?,
            max_request_bytes: match fields.get("bytes") {
                Some(bytes) => {
                    usize::try_from(*bytes).map_err(|_| "bytes is too large".to_string())?
                }
                None => Self::DEFAULT_REQUEST_BYTES,
            },
        };
        if let (Some(day), Some(total)) = (policy.per_key_day, policy.total_day)
            && day > total
        {
            return Err("day cannot exceed total".into());
        }
        Ok(policy)
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

    /// The human message the refusal carries.
    #[must_use]
    pub fn message(&self) -> String {
        match self {
            Refusal::TooLarge { bytes, limit } => {
                format!("the request is {bytes} bytes; this worker takes at most {limit}")
            }
            Refusal::RateLimited { .. } => {
                "this key sent too many requests in the last minute".to_string()
            }
            Refusal::Exhausted {
                everyone: false, ..
            } => "this key used today's requests on this worker".to_string(),
            Refusal::Exhausted { everyone: true, .. } => {
                "this worker answered as many requests as it takes today".to_string()
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

/// The counts an open worker admits jobs against.
pub struct Ledger {
    policy: Policy,
    path: Option<PathBuf>,
    day: u64,
    total: u32,
    per_day: HashMap<String, u32>,
    /// Admission times inside the last minute, oldest first, per key.
    recent: HashMap<String, VecDeque<u64>>,
}

impl Ledger {
    /// A ledger for `policy`, continuing the day saved at `path` when there
    /// is one. A file for an earlier day starts the new day empty.
    ///
    /// # Errors
    ///
    /// A file that exists but does not read: a worker that cannot tell how
    /// much of today it spent must not guess zero.
    pub fn open(policy: Policy, path: Option<PathBuf>, now: u64) -> Result<Self, String> {
        let saved = match &path {
            Some(path) if path.exists() => {
                let text = fs::read_to_string(path)
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
            path,
            day,
            total,
            per_day,
            recent: HashMap::new(),
        })
    }

    /// The policy this ledger enforces.
    #[must_use]
    pub fn policy(&self) -> Policy {
        self.policy
    }

    /// Jobs admitted today, every caller together.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.total
    }

    /// Admit one job from `key` (the verified request signer) whose content
    /// is `bytes` long, at `now` in Unix seconds, and count it; or say why
    /// not, counting nothing.
    ///
    /// # Errors
    ///
    /// The refusal, with its code and wait.
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
        let window = self.recent.entry(key.to_string()).or_default();
        while window
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= MINUTE)
        {
            window.pop_front();
        }
        if let Some(minute) = self.policy.per_key_minute
            && window.len() >= minute as usize
        {
            let oldest = window.front().copied().unwrap_or(now);
            let wait = (oldest + MINUTE).saturating_sub(now).max(1);
            return Err(Refusal::RateLimited {
                retry_after_ms: wait * 1_000,
            });
        }
        window.push_back(now);
        *self.per_day.entry(key.to_string()).or_default() += 1;
        self.total += 1;
        // Keys with an empty window take no memory past their minute.
        self.recent.retain(|_, window| {
            window
                .back()
                .is_some_and(|at| now.saturating_sub(*at) < MINUTE)
        });
        self.save();
        Ok(())
    }

    /// Write the day's counts, best effort: a failed write is logged, and
    /// the counts in memory still bound this process.
    fn save(&self) {
        let Some(path) = &self.path else { return };
        if self.policy.per_key_day.is_none() && self.policy.total_day.is_none() {
            return;
        }
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
            eprintln!("quota: could not save {}: {error}", path.display());
        }
    }
}

fn write_atomically(path: &Path, saved: &Saved) -> Result<(), String> {
    let text = serde_json::to_string(saved).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, text).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const POLICY: Policy = Policy {
        per_key_day: Some(5),
        per_key_minute: Some(2),
        total_day: Some(7),
        max_request_bytes: 100,
    };

    /// Noon on a UTC day.
    const NOON: u64 = 20_000 * DAY + DAY / 2;

    #[test]
    fn the_policy_reads_each_part_and_refuses_loose_ones() {
        assert_eq!(
            Policy::parse("day=40, minute=6,total=3000").unwrap(),
            Policy {
                per_key_day: Some(40),
                per_key_minute: Some(6),
                total_day: Some(3000),
                max_request_bytes: Policy::DEFAULT_REQUEST_BYTES,
            }
        );
        assert_eq!(
            Policy::parse("day=1,minute=1,total=1,bytes=10")
                .unwrap()
                .max_request_bytes,
            10
        );
        // A flood limit above the day is allowed; the day binds first.
        assert_eq!(
            Policy::parse("day=40,minute=600,total=3000")
                .unwrap()
                .per_key_minute,
            Some(600)
        );
        // Every part is optional: nothing named is unlimited (#10120).
        assert_eq!(Policy::parse("").unwrap(), Policy::UNLIMITED);
        assert!(!Policy::UNLIMITED.counts());
        let bytes_only = Policy::parse("bytes=98304").unwrap();
        assert!(!bytes_only.counts());
        assert_eq!(bytes_only.max_request_bytes, 98_304);
        assert_eq!(
            Policy::parse("minute=6").unwrap(),
            Policy {
                per_key_minute: Some(6),
                ..Policy::UNLIMITED
            }
        );
        for bad in [
            "day=40,minute=6,total=0",
            "day=40,minute=6,total=-1",
            "day=40,minute=6,total=10",
            "day=40,minute=6,total=3000,hours=2",
            "day=40,day=41,minute=6,total=3000",
            "day",
        ] {
            assert!(Policy::parse(bad).is_err(), "{bad:?}");
        }
    }

    /// The shipped policy counts nothing: a key sends as many jobs as it
    /// likes, in any minute and any day; only the size bound holds.
    #[test]
    fn the_unlimited_policy_admits_every_job_but_an_oversized_one() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let mut ledger = Ledger::open(Policy::UNLIMITED, Some(path.clone()), NOON).unwrap();
        for at in 0..2_000 {
            assert_eq!(ledger.admit("a", 10, NOON + at / 100), Ok(()));
        }
        assert_eq!(ledger.total(), 2_000);
        assert_eq!(
            ledger
                .admit("a", Policy::DEFAULT_REQUEST_BYTES + 1, NOON)
                .unwrap_err()
                .code(),
            "limit_exceeded"
        );
        // Nothing to keep across a restart when nothing is counted.
        assert!(!path.exists());
    }

    #[test]
    fn a_key_is_held_to_its_minute_then_its_day() {
        let mut ledger = Ledger::open(POLICY, None, NOON).unwrap();
        assert_eq!(ledger.admit("a", 10, NOON), Ok(()));
        assert_eq!(ledger.admit("a", 10, NOON + 1), Ok(()));
        // The third in the same minute waits for the first to age out.
        assert_eq!(
            ledger.admit("a", 10, NOON + 20),
            Err(Refusal::RateLimited {
                retry_after_ms: 40_000
            })
        );
        // Another key has its own minute.
        assert_eq!(ledger.admit("b", 10, NOON + 20), Ok(()));
        assert_eq!(ledger.admit("a", 10, NOON + 60), Ok(()));
        assert_eq!(ledger.admit("a", 10, NOON + 120), Ok(()));
        assert_eq!(ledger.admit("a", 10, NOON + 180), Ok(()));
        // Five today: the sixth waits for tomorrow, whatever the minute.
        let refused = ledger.admit("a", 10, NOON + 240).unwrap_err();
        assert_eq!(refused.code(), "quota_exhausted");
        assert_eq!(refused.retry_after_ms(), Some((DAY / 2 - 240) * 1_000));
        assert_eq!(ledger.total(), 6);
        // Tomorrow the key starts again.
        assert_eq!(ledger.admit("a", 10, NOON + DAY), Ok(()));
        assert_eq!(ledger.total(), 1);
    }

    #[test]
    fn every_key_together_is_held_to_the_total() {
        let mut ledger = Ledger::open(POLICY, None, NOON).unwrap();
        for index in 0..7 {
            assert_eq!(ledger.admit(&format!("k{index}"), 10, NOON), Ok(()));
        }
        // Minting a new key does not buy more of the day.
        let refused = ledger.admit("fresh", 10, NOON).unwrap_err();
        assert_eq!(
            refused,
            Refusal::Exhausted {
                retry_after_ms: DAY / 2 * 1_000,
                everyone: true
            }
        );
        assert!(refused.message().contains("today"));
    }

    #[test]
    fn a_large_request_is_refused_and_counts_nothing() {
        let mut ledger = Ledger::open(POLICY, None, NOON).unwrap();
        let refused = ledger.admit("a", 101, NOON).unwrap_err();
        assert_eq!(refused.code(), "limit_exceeded");
        assert_eq!(refused.retry_after_ms(), None);
        assert_eq!(ledger.total(), 0);
    }

    #[test]
    fn the_day_survives_a_restart_and_ends_at_midnight() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("quota.json");
        let mut ledger = Ledger::open(POLICY, Some(path.clone()), NOON).unwrap();
        for at in [NOON, NOON + 60, NOON + 120, NOON + 180, NOON + 240] {
            assert_eq!(ledger.admit("a", 10, at), Ok(()));
        }
        drop(ledger);
        let mut again = Ledger::open(POLICY, Some(path.clone()), NOON + 300).unwrap();
        assert_eq!(again.total(), 5);
        assert_eq!(
            again.admit("a", 10, NOON + 300).unwrap_err().code(),
            "quota_exhausted"
        );
        let tomorrow = Ledger::open(POLICY, Some(path.clone()), NOON + DAY).unwrap();
        assert_eq!(tomorrow.total(), 0);
        // A file that does not read stops the worker rather than reading
        // as an unspent day.
        fs::write(&path, "not json").unwrap();
        assert!(Ledger::open(POLICY, Some(path), NOON).is_err());
    }
}
