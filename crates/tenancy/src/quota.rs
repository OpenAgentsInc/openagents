//! The quota ledger: durable reservations, retry-safe settlement.
//!
//! A call costs something before it answers, so the reservation exists
//! before dispatch and outlives the process that made it. The ledger is
//! an append-only `quota-ledger.jsonl` beside the registry — one writer
//! at a time, an exclusive lock file held for the ledger's open lifetime
//! — and every event lands on disk before the work it pays for begins.
//!
//! # The lifecycle
//!
//! `reserved` → `settled` or `released`, with `orphaned` as recovery's
//! answer to a reservation whose writer disappeared:
//!
//! - **reserved** — the budget held the units and the event is on disk.
//!   A reservation carries a deadline; past it, the ledger orphans it on
//!   the next read rather than holding the units forever.
//! - **settled** — the attempt resolved and its outcome was recorded:
//!   answered, refused, unavailable, or unknown. Which outcomes count
//!   against the budget is the settlement policy's decision (`quota-v1`:
//!   everything that ran counts; `unattempted` frees the reservation).
//! - **released** — the attempt was never dispatched. The units return;
//!   the reservation was real but the work was not.
//! - **orphaned** — recovery found a reservation whose deadline passed
//!   with no settlement. Its outcome is `unknown`: the work may or may
//!   not have run, and the ledger says so rather than guessing.
//!
//! # Idempotency
//!
//! `(request, attempt)` is the reservation's identity. Reserving the same
//! pair with the same request digest returns the reservation that exists
//! — a retry is not a second spend. Reserving the pair with a *different*
//! digest is refused: a caller that reuses an idempotency key for changed
//! content is making two requests and calling them one.
//!
//! # What a quota is not
//!
//! Units are resources — requests, questions, input bytes, concurrency —
//! not money. What a unit costs belongs to the pricing contract, and
//! nothing here names a price.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::manifest::{Manifest, Quota};

/// The ledger file and its lock, beside `registry.json`.
const LEDGER: &str = "quota-ledger.jsonl";

/// The settlement policy this module implements.
pub const POLICY_V1: &str = "quota-v1";

/// What an attempt's outcome counts as under `quota-v1`.
///
/// The receipt's outcome vocabulary, mapped to its accounting meaning:
/// work that ran is charged whether or not it answered, work that never
/// dispatched is freed, and work the service cannot account for is
/// charged conservatively — an `unknown` outcome cannot prove the compute
/// was not spent.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Outcome {
    /// The door produced an answer. Charged.
    Answered,
    /// The door declined. Charged — the work happened.
    Refused,
    /// Capacity or transport denied the call after dispatch. Charged —
    /// the compute was spent even though the caller received nothing.
    Unavailable,
    /// Never dispatched. Freed — the reservation returns to the budget.
    Unattempted,
    /// The service cannot say what happened. Charged — conservatively,
    /// because the alternative is spending nobody accounts for.
    Unknown,
}

/// The resources a reservation holds.
///
/// A request is not a constant amount of compute: ten questions over a
/// long state cost more than one over a short one, and `options` counts
/// the readout width a `choice` or `score` call pays for. The fields are
/// estimates at reserve time and the measured values at settlement.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Units {
    /// Questions in the request.
    #[serde(default)]
    pub questions: u64,
    /// Input bytes — the state plus the question text.
    #[serde(default)]
    pub input_bytes: u64,
    /// Total options across the request's questions.
    #[serde(default)]
    pub options: u64,
}

impl Units {
    /// The zero reservation.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }
}

/// The event the ledger appends.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
enum Event {
    /// A reservation was taken.
    Reserved {
        request: String,
        attempt: u32,
        request_digest: String,
        tenant: String,
        key: String,
        units: Units,
        reserved_unix: u64,
        expires_unix: u64,
        day: String,
    },
    /// A reservation resolved: its outcome and the measured units.
    Settled {
        request: String,
        attempt: u32,
        outcome: Outcome,
        units: Units,
        resolved_unix: u64,
    },
    /// A reservation was freed without dispatch.
    Released {
        request: String,
        attempt: u32,
        resolved_unix: u64,
    },
    /// Recovery found a reservation past its deadline, unsettled.
    Orphaned {
        request: String,
        attempt: u32,
        resolved_unix: u64,
    },
}

/// One reservation's current state, folded from the event log.
#[derive(Clone, Debug)]
pub struct Reservation {
    /// The logical request.
    pub request: String,
    /// The attempt number.
    pub attempt: u32,
    /// Digest of the canonical request envelope — what the idempotency
    /// check compares.
    pub request_digest: String,
    /// The tenant paying.
    pub tenant: String,
    /// The key that reserved, for audit — the id, never the secret.
    pub key: String,
    /// The units held.
    pub units: Units,
    /// Where the reservation stands.
    pub state: State,
    /// The outcome settlement recorded, when one did.
    pub outcome: Option<Outcome>,
    /// The UTC day the reservation counts against, `YYYY-MM-DD`.
    pub day: String,
    /// Unix seconds when the reservation was taken.
    pub reserved_unix: u64,
    /// Unix seconds after which an unsettled reservation is orphaned.
    pub expires_unix: u64,
}

/// Where a reservation stands.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State {
    /// Held, unsettled.
    Reserved,
    /// Resolved with a recorded outcome.
    Settled,
    /// Freed without dispatch.
    Released,
    /// Recovery closed it with outcome unknown.
    Orphaned,
}

/// A tenant's position: what the budget charged today and what is still
/// held.
#[derive(Clone, Debug, Default)]
pub struct Usage {
    /// Reservations settled today and charged under the policy.
    pub settled_today: u64,
    /// Questions charged today.
    pub questions_today: u64,
    /// Input bytes charged today.
    pub input_bytes_today: u64,
    /// Reservations held and unsettled right now.
    pub outstanding: u64,
    /// Reservations orphaned today — the unknown-completion count an
    /// operator watches.
    pub orphaned_today: u64,
}

/// Why a reservation was refused.
#[derive(Debug)]
pub enum Refusal {
    /// The idempotency pair exists with different content.
    ContentConflict { request: String, attempt: u32 },
    /// The reservation was already resolved — settled, released, or
    /// orphaned — and cannot be taken again.
    Resolved { request: String, attempt: u32 },
    /// The tenant's quota does not cover the request; the field names
    /// which bound failed.
    Exhausted { tenant: String, bound: &'static str },
    /// The tenant carries a settlement policy this build does not
    /// implement.
    UnknownPolicy { tenant: String, policy: String },
    /// No reservation exists under this request and attempt — a settle
    /// that arrives before any reserve cannot be honored.
    Unreserved { request: String, attempt: u32 },
    /// The ledger itself failed — not a quota answer.
    Ledger(LedgerTrouble),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ContentConflict { request, attempt } => write!(
                f,
                "request `{request}` attempt {attempt} was already used with \
                 different content; use a new idempotency key for a new request"
            ),
            Self::Resolved { request, attempt } => write!(
                f,
                "request `{request}` attempt {attempt} has already finished"
            ),
            Self::Exhausted { tenant, bound } => {
                write!(f, "account `{tenant}` has reached its `{bound}` limit")
            }
            Self::UnknownPolicy { tenant, policy } => write!(
                f,
                "tenant `{tenant}` uses billing policy `{policy}`, which this \
                 version of the service doesn't support"
            ),
            Self::Unreserved { request, attempt } => write!(
                f,
                "no quota is held for request `{request}` attempt {attempt}"
            ),
            Self::Ledger(trouble) => write!(f, "{trouble}"),
        }
    }
}

impl std::error::Error for Refusal {}

/// What a reservation asks for — the parameters of one call.
///
/// `tenant` is the stable identity the budget binds to; `key` is the
/// credential id the reservation was taken under, recorded for audit.
/// `request` is the caller's logical request name and `attempt` its
/// retry number: together they are the idempotency pair. `request_digest`
/// is the digest of the canonical request envelope — the pair held with
/// a different digest is a changed request, not a retry. `ttl_secs` is
/// how long the reservation may stand unsettled before recovery orphans
/// it.
#[derive(Clone, Debug)]
pub struct Call<'a> {
    /// The tenant paying.
    pub tenant: &'a str,
    /// The credential id reserving, for audit — never the secret.
    pub key: &'a str,
    /// The logical request.
    pub request: &'a str,
    /// The retry number.
    pub attempt: u32,
    /// The digest of the canonical request envelope.
    pub request_digest: &'a str,
    /// The units the reservation holds.
    pub units: &'a Units,
    /// Seconds before an unsettled reservation is orphaned.
    pub ttl_secs: u64,
}

/// What went wrong with the ledger itself.
#[derive(Debug)]
pub enum LedgerTrouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// An event line did not parse — the ledger is corrupt.
    Corrupt(String),
    /// Another ledger writer holds the lock.
    Locked(String),
}

impl std::fmt::Display for LedgerTrouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Corrupt(message) => write!(f, "{message}"),
            Self::Locked(path) => write!(
                f,
                "another process is writing {path}. Only one process can write the \
                 ledger at a time: wait for it to finish, or remove the lock file \
                 if no other process is running"
            ),
        }
    }
}

impl std::error::Error for LedgerTrouble {}

impl From<std::io::Error> for LedgerTrouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// The exclusive lock one writer holds for the ledger's open lifetime.
///
/// Same shape as the store's: `create_new` makes the lock atomic, the
/// file's absence is the release, and a dropped guard removes it. A
/// crashed writer leaves the file; the holder line inside says who it
/// was.
struct Lock {
    path: PathBuf,
}

impl Lock {
    fn acquire(ledger: &Path) -> Result<Self, LedgerTrouble> {
        let path = ledger.with_extension("lock");
        match std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&path)
        {
            Ok(mut file) => {
                writeln!(file, "pid {}", std::process::id()).ok();
                Ok(Self { path })
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                Err(LedgerTrouble::Locked(path.display().to_string()))
            }
            Err(error) => Err(LedgerTrouble::Io(error)),
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
    }
}

/// The open ledger: the file's events folded into current state.
///
/// Open once per serving process — the lock is held for the lifetime, so
/// a second `Ledger::open` on the same directory is refused rather than
/// interleaved.
pub struct Ledger {
    path: PathBuf,
    reservations: BTreeMap<(String, u32), Reservation>,
    _lock: Lock,
}

impl Ledger {
    /// Open the ledger at the current time.
    pub fn open(dir: &Path) -> Result<Self, LedgerTrouble> {
        Self::open_at(dir, unix_now())
    }

    /// Open at an explicit time — the seam tests and recovery share.
    pub(crate) fn open_at(dir: &Path, now: u64) -> Result<Self, LedgerTrouble> {
        let path = dir.join(LEDGER);
        let lock = Lock::acquire(&path)?;
        let mut ledger = Self {
            path,
            reservations: BTreeMap::new(),
            _lock: lock,
        };
        ledger.replay()?;
        ledger.sweep(now)?;
        Ok(ledger)
    }

    /// Fold the event log into current reservation state.
    fn replay(&mut self) -> Result<(), LedgerTrouble> {
        let text = match std::fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(LedgerTrouble::Io(error)),
        };
        for (line, text) in text.lines().enumerate() {
            let event: Event = serde_json::from_str(text).map_err(|error| {
                LedgerTrouble::Corrupt(format!(
                    "{}: line {}: {error}",
                    self.path.display(),
                    line + 1
                ))
            })?;
            match event {
                Event::Reserved {
                    request,
                    attempt,
                    request_digest,
                    tenant,
                    key,
                    units,
                    reserved_unix,
                    expires_unix,
                    day,
                } => {
                    self.reservations.insert(
                        (request.clone(), attempt),
                        Reservation {
                            request,
                            attempt,
                            request_digest,
                            tenant,
                            key,
                            units,
                            state: State::Reserved,
                            outcome: None,
                            day,
                            reserved_unix,
                            expires_unix,
                        },
                    );
                }
                Event::Settled {
                    request,
                    attempt,
                    outcome,
                    units,
                    resolved_unix: _,
                } => {
                    if let Some(reservation) = self.reservations.get_mut(&(request, attempt)) {
                        reservation.state = State::Settled;
                        reservation.outcome = Some(outcome);
                        reservation.units = units;
                    }
                }
                Event::Released {
                    request, attempt, ..
                } => {
                    if let Some(reservation) = self.reservations.get_mut(&(request, attempt)) {
                        reservation.state = State::Released;
                        reservation.outcome = Some(Outcome::Unattempted);
                    }
                }
                Event::Orphaned {
                    request, attempt, ..
                } => {
                    if let Some(reservation) = self.reservations.get_mut(&(request, attempt)) {
                        reservation.state = State::Orphaned;
                        reservation.outcome = Some(Outcome::Unknown);
                    }
                }
            }
        }
        Ok(())
    }

    /// Append one event.
    fn append(&mut self, event: &Event) -> Result<(), LedgerTrouble> {
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        let line = serde_json::to_string(event)
            .map_err(|error| LedgerTrouble::Corrupt(error.to_string()))?;
        writeln!(file, "{line}")?;
        file.sync_all()?;
        Ok(())
    }

    /// Orphan every reservation whose deadline has passed — on open and
    /// before each reservation check, so a crashed attempt stops holding
    /// budget it may never settle.
    fn sweep(&mut self, now: u64) -> Result<(), LedgerTrouble> {
        let expired: Vec<(String, u32)> = self
            .reservations
            .iter()
            .filter(|(_, reservation)| {
                reservation.state == State::Reserved && reservation.expires_unix <= now
            })
            .map(|(key, _)| key.clone())
            .collect();
        for (request, attempt) in expired {
            let event = Event::Orphaned {
                request: request.clone(),
                attempt,
                resolved_unix: now,
            };
            self.append(&event)?;
            let reservation = self.reservations.get_mut(&(request, attempt)).unwrap();
            reservation.state = State::Orphaned;
            reservation.outcome = Some(Outcome::Unknown);
        }
        Ok(())
    }

    /// Take a reservation.
    ///
    /// The same `(request, attempt)` reserved twice with the same digest
    /// returns the existing reservation — a retry is not a second spend.
    /// The same pair with a different digest is a new request wearing an
    /// old name and is refused.
    pub fn reserve(
        &mut self,
        manifest: &Manifest,
        call: &Call<'_>,
    ) -> Result<Reservation, Refusal> {
        let now = unix_now();
        self.sweep(now).map_err(Refusal::Ledger)?;
        if let Some(held) = self
            .reservations
            .get(&(call.request.to_string(), call.attempt))
        {
            if held.state != State::Reserved {
                return Err(Refusal::Resolved {
                    request: call.request.to_string(),
                    attempt: call.attempt,
                });
            }
            if held.request_digest != call.request_digest || held.tenant != call.tenant {
                return Err(Refusal::ContentConflict {
                    request: call.request.to_string(),
                    attempt: call.attempt,
                });
            }
            return Ok(held.clone());
        }

        let quota = manifest
            .tenants
            .get(call.tenant)
            .and_then(|record| record.quota.clone());
        if let Some(quota) = &quota {
            self.check(call.tenant, quota, call.units)?;
        }

        let day = day_of(now);
        let event = Event::Reserved {
            request: call.request.to_string(),
            attempt: call.attempt,
            request_digest: call.request_digest.to_string(),
            tenant: call.tenant.to_string(),
            key: call.key.to_string(),
            units: call.units.clone(),
            reserved_unix: now,
            expires_unix: now + call.ttl_secs,
            day: day.clone(),
        };
        self.append(&event).map_err(Refusal::Ledger)?;
        let reservation = Reservation {
            request: call.request.to_string(),
            attempt: call.attempt,
            request_digest: call.request_digest.to_string(),
            tenant: call.tenant.to_string(),
            key: call.key.to_string(),
            units: call.units.clone(),
            state: State::Reserved,
            outcome: None,
            day,
            reserved_unix: now,
            expires_unix: now + call.ttl_secs,
        };
        self.reservations.insert(
            (call.request.to_string(), call.attempt),
            reservation.clone(),
        );
        Ok(reservation)
    }

    /// Check the tenant's quota against the units a reservation wants.
    fn check(&self, tenant: &str, quota: &Quota, units: &Units) -> Result<(), Refusal> {
        let policy = quota.policy.as_deref().unwrap_or(POLICY_V1);
        if policy != POLICY_V1 {
            return Err(Refusal::UnknownPolicy {
                tenant: tenant.to_string(),
                policy: policy.to_string(),
            });
        }
        let usage = self.usage(tenant);
        let outstanding: u64 = self
            .reservations
            .values()
            .filter(|reservation| {
                reservation.tenant == tenant && reservation.state == State::Reserved
            })
            .count() as u64;
        let bounds = [
            (
                quota.requests_per_day,
                usage.settled_today + 1,
                "requests_per_day",
            ),
            (
                quota.questions_per_day,
                usage.questions_today + units.questions,
                "questions_per_day",
            ),
            (
                quota.input_bytes_per_day,
                usage.input_bytes_today + units.input_bytes,
                "input_bytes_per_day",
            ),
            (quota.concurrency, outstanding + 1, "concurrency"),
        ];
        for (limit, needed, bound) in bounds {
            if limit.is_some_and(|limit| needed > limit) {
                return Err(Refusal::Exhausted {
                    tenant: tenant.to_string(),
                    bound,
                });
            }
        }
        Ok(())
    }

    /// Settle a reservation with its outcome and measured units.
    ///
    /// Settling twice is a no-op that returns the settled record — the
    /// answer arrived twice is not the answer twice. Settling a
    /// reservation that was never taken is refused: the ledger does not
    /// invent holds.
    pub fn settle(
        &mut self,
        request: &str,
        attempt: u32,
        outcome: Outcome,
        units: &Units,
    ) -> Result<Reservation, Refusal> {
        let key = (request.to_string(), attempt);
        let Some(reservation) = self.reservations.get(&key) else {
            return Err(Refusal::Unreserved {
                request: request.to_string(),
                attempt,
            });
        };
        if reservation.state == State::Settled {
            return Ok(reservation.clone());
        }
        if reservation.state != State::Reserved {
            return Err(Refusal::Resolved {
                request: request.to_string(),
                attempt,
            });
        }
        let now = unix_now();
        if outcome == Outcome::Unattempted {
            let event = Event::Released {
                request: request.to_string(),
                attempt,
                resolved_unix: now,
            };
            self.append(&event).map_err(Refusal::Ledger)?;
            let reservation = self.reservations.get_mut(&key).unwrap();
            reservation.state = State::Released;
            reservation.outcome = Some(Outcome::Unattempted);
            return Ok(reservation.clone());
        }
        let event = Event::Settled {
            request: request.to_string(),
            attempt,
            outcome,
            units: units.clone(),
            resolved_unix: now,
        };
        self.append(&event).map_err(Refusal::Ledger)?;
        let reservation = self.reservations.get_mut(&key).unwrap();
        reservation.state = State::Settled;
        reservation.outcome = Some(outcome);
        reservation.units = units.clone();
        Ok(reservation.clone())
    }

    /// A tenant's position: charged today, held now, orphaned today.
    #[must_use]
    pub fn usage(&self, tenant: &str) -> Usage {
        let today = day_of(unix_now());
        let mut usage = Usage::default();
        for reservation in self.reservations.values() {
            if reservation.tenant != tenant {
                continue;
            }
            match reservation.state {
                State::Reserved => {
                    usage.outstanding += 1;
                }
                State::Settled => {
                    if reservation.day == today && charged(reservation.outcome) {
                        usage.settled_today += 1;
                        usage.questions_today += reservation.units.questions;
                        usage.input_bytes_today += reservation.units.input_bytes;
                    }
                }
                State::Released => {}
                State::Orphaned => {
                    if reservation.day == today {
                        usage.orphaned_today += 1;
                        // Unknown completion is charged under quota-v1:
                        // the ledger cannot prove the compute was not
                        // spent, so it counts rather than forgets.
                        usage.settled_today += 1;
                        usage.questions_today += reservation.units.questions;
                        usage.input_bytes_today += reservation.units.input_bytes;
                    }
                }
            }
        }
        usage
    }

    /// One reservation by its idempotency pair — the join a usage read
    /// makes from a receipt's `(request, attempt)` to the units and
    /// state the ledger recorded. Read-only.
    #[must_use]
    pub fn reservation(&self, request: &str, attempt: u32) -> Option<&Reservation> {
        self.reservations.get(&(request.to_string(), attempt))
    }

    /// The reservations still held — what an operator's `outstanding`
    /// reads.
    #[must_use]
    pub fn outstanding(&self, tenant: &str) -> Vec<&Reservation> {
        self.reservations
            .values()
            .filter(|reservation| {
                reservation.tenant == tenant && reservation.state == State::Reserved
            })
            .collect()
    }
}

/// Whether an outcome counts against the budget under `quota-v1`.
fn charged(outcome: Option<Outcome>) -> bool {
    matches!(
        outcome,
        Some(Outcome::Answered | Outcome::Refused | Outcome::Unavailable | Outcome::Unknown)
    )
}

/// Unix seconds now.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

/// The UTC day a unix second falls on, `YYYY-MM-DD`.
fn day_of(unix: u64) -> String {
    let (year, month, day) = civil_from_days((unix / 86400) as i64);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Days since the epoch to a calendar date, by Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Tenant;

    fn quota(requests: u64, questions: u64, concurrency: u64) -> Quota {
        Quota {
            requests_per_day: Some(requests),
            questions_per_day: Some(questions),
            input_bytes_per_day: None,
            concurrency: Some(concurrency),
            policy: Some(POLICY_V1.to_string()),
        }
    }

    fn manifest(quota: Option<Quota>) -> Manifest {
        let mut tenants = BTreeMap::new();
        tenants.insert(
            "acme".to_string(),
            Tenant {
                credential: "key-ref:acme".to_string(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota,
            },
        );
        tenants.insert(
            "globex".to_string(),
            Tenant {
                credential: "key-ref:globex".to_string(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        );
        Manifest {
            v: crate::SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants,
            digest: String::new(),
        }
    }

    fn units(questions: u64, bytes: u64) -> Units {
        Units {
            questions,
            input_bytes: bytes,
            options: 0,
        }
    }

    fn call<'a>(
        tenant: &'a str,
        request: &'a str,
        attempt: u32,
        digest: &'a str,
        units: &'a Units,
        ttl_secs: u64,
    ) -> Call<'a> {
        Call {
            tenant,
            key: "key-1",
            request,
            attempt,
            request_digest: digest,
            units,
            ttl_secs,
        }
    }

    #[test]
    fn reserve_settle_and_usage_count_what_ran() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(Some(quota(10, 100, 4)));
        let mut ledger = Ledger::open(dir.path()).unwrap();
        let reservation = ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "digest-1", &units(3, 400), 300),
            )
            .unwrap();
        assert_eq!(reservation.state, State::Reserved);
        assert_eq!(ledger.usage("acme").outstanding, 1);

        let settled = ledger
            .settle("req-1", 1, Outcome::Answered, &units(3, 410))
            .unwrap();
        assert_eq!(settled.state, State::Settled);
        let usage = ledger.usage("acme");
        assert_eq!(usage.settled_today, 1);
        assert_eq!(usage.questions_today, 3);
        assert_eq!(usage.input_bytes_today, 410);
        assert_eq!(usage.outstanding, 0);
    }

    #[test]
    fn a_retry_is_idempotent_and_changed_content_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(None);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        let first = ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "digest-1", &units(1, 10), 300),
            )
            .unwrap();
        let again = ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "digest-1", &units(1, 10), 300),
            )
            .unwrap();
        assert_eq!(first.request_digest, again.request_digest);
        assert_eq!(ledger.usage("acme").outstanding, 1);

        assert!(matches!(
            ledger.reserve(
                &manifest,
                &call("acme", "req-1", 1, "digest-2", &units(1, 10), 300)
            ),
            Err(Refusal::ContentConflict { .. })
        ));
        // A different attempt of the same request reserves separately.
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 2, "digest-1", &units(1, 10), 300),
            )
            .unwrap();
        assert_eq!(ledger.usage("acme").outstanding, 2);
    }

    #[test]
    fn an_exhausted_quota_refuses_before_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(Some(quota(2, 100, 2)));
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "d1", &units(1, 10), 300),
            )
            .unwrap();
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-2", 1, "d2", &units(1, 10), 300),
            )
            .unwrap();
        // Concurrency: two held is the bound.
        assert!(matches!(
            ledger.reserve(
                &manifest,
                &call("acme", "req-3", 1, "d3", &units(1, 10), 300)
            ),
            Err(Refusal::Exhausted {
                bound: "concurrency",
                ..
            })
        ));
        // Settle one, and the concurrency bound opens — but the request
        // budget of 2 still refuses the third.
        ledger
            .settle("req-1", 1, Outcome::Answered, &units(1, 10))
            .unwrap();
        ledger
            .settle("req-2", 1, Outcome::Refused, &units(1, 10))
            .unwrap();
        assert!(matches!(
            ledger.reserve(
                &manifest,
                &call("acme", "req-3", 1, "d3", &units(1, 10), 300)
            ),
            Err(Refusal::Exhausted {
                bound: "requests_per_day",
                ..
            })
        ));
    }

    #[test]
    fn a_crash_orphans_the_reservation_without_losing_it() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(Some(quota(10, 100, 4)));
        {
            let mut ledger = Ledger::open(dir.path()).unwrap();
            ledger
                .reserve(&manifest, &call("acme", "req-1", 1, "d1", &units(5, 50), 0))
                .unwrap();
            // The writer is dropped mid-flight — no settle, no release.
        }
        // Recovery reopens past the deadline: the reservation is orphaned
        // and counted unknown, not silently freed.
        let ledger = Ledger::open_at(dir.path(), unix_now() + 60).unwrap();
        let usage = ledger.usage("acme");
        assert_eq!(usage.outstanding, 0);
        assert_eq!(usage.orphaned_today, 1);
        assert_eq!(usage.settled_today, 1);
        assert_eq!(usage.questions_today, 5);
    }

    #[test]
    fn unattempted_work_frees_the_reservation() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(Some(quota(1, 100, 4)));
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "d1", &units(1, 10), 300),
            )
            .unwrap();
        ledger
            .settle("req-1", 1, Outcome::Unattempted, &Units::none())
            .unwrap();
        let usage = ledger.usage("acme");
        assert_eq!(usage.settled_today, 0);
        assert_eq!(usage.questions_today, 0);
        // And the freed budget admits the next request.
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-2", 1, "d2", &units(1, 10), 300),
            )
            .unwrap();
    }

    #[test]
    fn settlement_is_idempotent_and_never_double_counts() {
        let dir = tempfile::tempdir().unwrap();
        let manifest = manifest(None);
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "d1", &units(2, 20), 300),
            )
            .unwrap();
        ledger
            .settle("req-1", 1, Outcome::Answered, &units(2, 20))
            .unwrap();
        // The answer arrived twice; the second settle is the same record.
        let again = ledger
            .settle("req-1", 1, Outcome::Answered, &units(2, 20))
            .unwrap();
        assert_eq!(again.state, State::Settled);
        assert_eq!(ledger.usage("acme").settled_today, 1);
    }

    #[test]
    fn a_second_writer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let _ledger = Ledger::open(dir.path()).unwrap();
        assert!(matches!(
            Ledger::open(dir.path()),
            Err(LedgerTrouble::Locked(_))
        ));
    }

    #[test]
    fn a_rotated_key_spends_against_the_same_tenant() {
        let dir = tempfile::tempdir().unwrap();
        // The budget binds to the tenant identity, not the credential:
        // key-1 settles what it reserved, key-2 keeps spending against
        // the same ledger, and usage sees one tenant's position.
        let manifest = manifest(Some(quota(3, 100, 4)));
        let mut ledger = Ledger::open(dir.path()).unwrap();
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-1", 1, "d1", &units(1, 10), 300),
            )
            .unwrap();
        ledger
            .settle("req-1", 1, Outcome::Answered, &units(1, 10))
            .unwrap();
        let two = units(1, 10);
        let mut rotated = call("acme", "req-2", 1, "d2", &two, 300);
        rotated.key = "key-2";
        ledger.reserve(&manifest, &rotated).unwrap();
        ledger
            .settle("req-2", 1, Outcome::Answered, &units(1, 10))
            .unwrap();
        let usage = ledger.usage("acme");
        assert_eq!(usage.settled_today, 2);
        // Two spent of three — the next reservation still fits.
        ledger
            .reserve(
                &manifest,
                &call("acme", "req-3", 1, "d3", &units(1, 10), 300),
            )
            .unwrap();
    }

    #[test]
    fn an_unknown_policy_is_refused_rather_than_guessed() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = manifest(Some(quota(10, 100, 4)));
        manifest.tenants.get_mut("acme").unwrap().quota = Some(Quota {
            policy: Some("quota-v2".to_string()),
            ..quota(10, 100, 4)
        });
        let mut ledger = Ledger::open(dir.path()).unwrap();
        assert!(matches!(
            ledger.reserve(
                &manifest,
                &call("acme", "req-1", 1, "d1", &units(1, 10), 300)
            ),
            Err(Refusal::UnknownPolicy { .. })
        ));
    }
}
