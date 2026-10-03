//! Provider usage windows: how much of a login's allowance is used, and
//! when each window resets, as a usage probe read them.
//!
//! This is the book's data half: the typed readings, `usage.json` beside
//! the capacity book, the parsers for each provider's answer, and the
//! cache and backoff rules. Asking a provider needs its login's token, so
//! the fetch lives in `coder::task::usage`, in the `coder` host process
//! only, and reaches [`refresh`] as a [`Fetch`] function. That module
//! re-exports this one, so the host and the loop read one book.
//!
//! The capacity book reads it too: a refusal whose provider did not report
//! a reset takes the reset of the window a fresh reading shows at its
//! limit ([`crate::capacity::Refusal::with_probed_reset`]).
//!
//! | Provider | Endpoint | Windows |
//! | --- | --- | --- |
//! | Claude | `GET https://api.anthropic.com/api/oauth/usage` | `five_hour`, `seven_day`: `utilization` percent and `resets_at` |
//! | Codex | `GET https://chatgpt.com/backend-api/wham/usage` | `primary_window`, `secondary_window`: `used_percent`, `limit_window_seconds`, `reset_at`; and `limit_reached` |
//!
//! Probes are cached: a provider is asked at most once per
//! [`MIN_INTERVAL`], a failure waits [`FAILURE_BACKOFF`], and a
//! `Retry-After` is honored up to [`MAX_RETRY_AFTER`]. A reading older than
//! [`STALE_AFTER`] is not used.

use std::io::{Read, Seek, Write};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::capacity::{self, Provider};

/// The book in the task store directory, beside `capacity.json`.
pub const FILE: &str = "usage.json";
/// The book's format.
pub const SCHEMA: &str = "openagents.coder.provider-usage.v1";
/// A provider is probed at most once in this many seconds.
pub const MIN_INTERVAL: u64 = 60;
/// After a failed probe, wait this many seconds before the next.
pub const FAILURE_BACKOFF: u64 = 5 * 60;
/// The longest `Retry-After` honored, in seconds.
pub const MAX_RETRY_AFTER: u64 = 6 * 60 * 60;
/// A reading older than this many seconds is not used for routing.
pub const STALE_AFTER: u64 = 15 * 60;
/// The used share at or above which a reading counts a window as at its
/// limit, for the reset a refusal takes from a probe.
pub const AT_LIMIT: f64 = 0.95;
/// The utilization, in percent, at or above which routing prefers another
/// route when the policy names no threshold.
pub const DEFAULT_THRESHOLD_PERCENT: u8 = 90;
/// A start does not pass a provider over for capacity on a reading older
/// than this many seconds: the host probes it again first (#10105).
pub const RECHECK_AFTER: u64 = 60;
/// A provider asked for a fresh reading is not asked again within this
/// many seconds, so a burst of starts asks once.
pub const FRESH_WITHIN: u64 = 5;

/// A usage window a provider reports.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowName {
    /// Claude's rolling five-hour window.
    FiveHour,
    /// Claude's seven-day window.
    SevenDay,
    /// Codex's primary window (its length is in `length_seconds`).
    Primary,
    /// Codex's secondary window.
    Secondary,
}

impl WindowName {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            WindowName::FiveHour => "five_hour",
            WindowName::SevenDay => "seven_day",
            WindowName::Primary => "primary",
            WindowName::Secondary => "secondary",
        }
    }
}

/// One window's use.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Window {
    pub window: WindowName,
    /// The share of the window's allowance used, 0 to 1.
    pub used_fraction: f64,
    /// When the window resets, in Unix seconds, if the provider said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// The window's length in seconds, if the provider said.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub length_seconds: Option<u64>,
}

/// One successful probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reading {
    pub provider: Provider,
    /// When the probe answered, in Unix seconds.
    pub observed_at: u64,
    pub windows: Vec<Window>,
    /// The provider said, as a typed field, that the login is at its limit.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub limit_reached: bool,
    /// The plan the provider named, such as `pro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// The fingerprint of the login the probe read ([`crate::account`]),
    /// when it has one. A reader drops the reading once a different login
    /// is signed in (#10105).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

impl Reading {
    /// The window with the most use.
    #[must_use]
    pub fn fullest(&self) -> Option<&Window> {
        self.windows
            .iter()
            .max_by(|a, b| a.used_fraction.total_cmp(&b.used_fraction))
    }

    /// When the limit that refuses the login resets, as far as this reading
    /// says: the latest reset of the windows at [`AT_LIMIT`] or more, else,
    /// when the provider said its limit is reached, the fullest window's
    /// reset. `None` when no window is at its limit or none says when.
    #[must_use]
    pub fn limiting_reset(&self) -> Option<u64> {
        self.windows
            .iter()
            .filter(|window| window.used_fraction >= AT_LIMIT)
            .filter_map(|window| window.resets_at)
            .max()
            .or_else(|| {
                self.limit_reached
                    .then(|| self.fullest().and_then(|window| window.resets_at))
                    .flatten()
            })
    }

    /// Whether the login is at or above `threshold_percent` in any window,
    /// or the provider said it reached its limit.
    #[must_use]
    pub fn near_limit(&self, threshold_percent: u8) -> bool {
        let threshold = f64::from(threshold_percent) / 100.0;
        self.limit_reached
            || self
                .fullest()
                .is_some_and(|window| window.used_fraction >= threshold)
    }
}

/// Why a probe gave no reading. None of these carries provider text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Failure {
    /// No login this probe can read.
    NoCredential,
    /// The login's access token has expired.
    Expired,
    /// The endpoint refused the token (HTTP 401 or 403).
    Unauthorized,
    /// The endpoint rate-limited the probe (HTTP 429).
    RateLimited,
    /// Another non-success HTTP status.
    Status,
    /// The body is not the typed shape this probe reads.
    Malformed,
    /// The request did not complete.
    Network,
    /// The provider has no usage endpoint to ask (Vertex).
    Unsupported,
}

impl Failure {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Failure::NoCredential => "no_credential",
            Failure::Expired => "expired",
            Failure::Unauthorized => "unauthorized",
            Failure::RateLimited => "rate_limited",
            Failure::Status => "status",
            Failure::Malformed => "malformed",
            Failure::Network => "network",
            Failure::Unsupported => "unsupported",
        }
    }
}

/// A provider's latest probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub provider: Provider,
    /// When the latest probe ran, in Unix seconds.
    pub attempted_at: u64,
    /// The earliest time the provider may be probed again.
    pub next_probe_at: u64,
    /// The latest successful reading, kept across later failures until it
    /// is stale.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<Reading>,
    /// Why the latest probe failed, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<Failure>,
}

/// Every provider's latest probe.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub entries: Vec<Entry>,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            schema: SCHEMA.into(),
            entries: Vec::new(),
        }
    }
}

impl Book {
    /// The book in `dir`, less each entry whose reading was taken on
    /// another login than the one signed in now ([`crate::account`],
    /// #10105): that provider is then due for a probe. Missing,
    /// unreadable, or malformed is empty.
    #[must_use]
    pub fn load(dir: &Path) -> Book {
        Book::load_with(dir, crate::account::identify)
    }

    /// [`Book::load`] with `identify` naming each provider's login now.
    #[must_use]
    pub fn load_with(dir: &Path, identify: crate::account::Identify) -> Book {
        let mut book = std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| Book::parse(&bytes))
            .unwrap_or_default();
        book.forget_other_logins(dir, identify);
        book
    }

    /// Drop each entry whose reading belongs to another login than the
    /// one signed in now.
    fn forget_other_logins(&mut self, dir: &Path, identify: crate::account::Identify) {
        self.entries.retain(|entry| {
            entry.reading.as_ref().is_none_or(|reading| {
                crate::account::applies(
                    reading.account.as_deref(),
                    crate::account::current(dir, entry.provider, identify).as_deref(),
                )
            })
        });
    }

    fn parse(bytes: &[u8]) -> Option<Book> {
        serde_json::from_slice::<Book>(bytes)
            .ok()
            .filter(|book| book.schema == SCHEMA)
    }

    #[must_use]
    pub fn entry(&self, provider: Provider) -> Option<&Entry> {
        self.entries.iter().find(|entry| entry.provider == provider)
    }

    /// The provider's reading, if one is fresh at `now`.
    #[must_use]
    pub fn reading(&self, provider: Provider, now: u64) -> Option<&Reading> {
        self.entry(provider)
            .and_then(|entry| entry.reading.as_ref())
            .filter(|reading| now < reading.observed_at.saturating_add(STALE_AFTER))
    }

    /// Whether a fresh reading puts `provider` at or above
    /// `threshold_percent`. No fresh reading is not near the limit: the
    /// refusal book decides.
    #[must_use]
    pub fn near_limit(&self, provider: Provider, threshold_percent: u8, now: u64) -> bool {
        self.reading(provider, now)
            .is_some_and(|reading| reading.near_limit(threshold_percent))
    }

    /// Whether a probe reading taken after `refusal` shows the login back
    /// under its allowance, so the refusal no longer holds (#10073): the
    /// refusal is a usage limit, the reading was observed after it, the
    /// provider did not say its limit is reached, and no window is at
    /// [`AT_LIMIT`]. Codex resets a week's allowance on its own schedule,
    /// sometimes before the reset its refusal named; the reading is the
    /// newer evidence. A rate limit is per minute and no window shows it,
    /// so a reading never lifts one.
    #[must_use]
    pub fn lifts(&self, refusal: &capacity::Refusal) -> bool {
        refusal.kind == capacity::Kind::UsageLimit
            && self
                .entry(refusal.provider)
                .and_then(|entry| entry.reading.as_ref())
                .is_some_and(|reading| {
                    reading.observed_at > refusal.observed_at
                        && !reading.windows.is_empty()
                        && !reading.limit_reached
                        && reading.limiting_reset().is_none()
                })
    }

    /// Whether `provider` may be probed at `now`.
    #[must_use]
    pub fn due(&self, provider: Provider, now: u64) -> bool {
        self.entry(provider)
            .is_none_or(|entry| now >= entry.next_probe_at)
    }

    /// Whether `provider` may be probed at `now` for a fresh reading: when
    /// [`Book::due`], or when its latest probe ran [`FRESH_WITHIN`] or more
    /// ago, unless the provider rate-limited the probe (its `Retry-After`
    /// is honored).
    #[must_use]
    pub fn due_fresh(&self, provider: Provider, now: u64) -> bool {
        self.due(provider, now)
            || self.entry(provider).is_some_and(|entry| {
                entry.failure != Some(Failure::RateLimited)
                    && now >= entry.attempted_at.saturating_add(FRESH_WITHIN)
            })
    }

    /// Whether a start must read `provider` again before passing it over
    /// for capacity at `now` (#10105): it has no reading, or its reading is
    /// [`RECHECK_AFTER`] seconds old or older.
    #[must_use]
    pub fn needs_recheck(&self, provider: Provider, now: u64) -> bool {
        self.entry(provider)
            .and_then(|entry| entry.reading.as_ref())
            .is_none_or(|reading| now >= reading.observed_at.saturating_add(RECHECK_AFTER))
    }

    /// One provider's windows as text, for host output and the journal:
    /// `codex 100% primary until 2026-10-03 18:07 UTC (limit reached)`.
    #[must_use]
    pub fn describe(&self, provider: Provider, now: u64) -> String {
        match (self.reading(provider, now), self.entry(provider)) {
            (Some(reading), entry) => {
                let mut windows: Vec<String> = reading
                    .windows
                    .iter()
                    .map(|window| {
                        let mut text = format!(
                            "{} {:.0}%",
                            window.window.as_str(),
                            window.used_fraction * 100.0
                        );
                        if let Some(at) = window.resets_at {
                            text.push_str(&format!(" until {}", capacity::utc(at)));
                        }
                        text
                    })
                    .collect();
                if windows.is_empty() {
                    windows.push("no windows".into());
                }
                let mut text = windows.join(", ");
                if reading.limit_reached {
                    text.push_str(" (limit reached)");
                }
                if let Some(failure) = entry.and_then(|entry| entry.failure) {
                    text.push_str(&format!(" (latest probe: {})", failure.as_str()));
                }
                text
            }
            (
                None,
                Some(Entry {
                    failure: Some(failure),
                    ..
                }),
            ) => format!("unknown (probe: {})", failure.as_str()),
            (None, _) => "not probed".into(),
        }
    }

    fn apply(&mut self, provider: Provider, now: u64, outcome: Outcome, account: Option<String>) {
        // A reading kept across a failure must be the same login's.
        let kept = self
            .entry(provider)
            .and_then(|entry| entry.reading.clone())
            .filter(|kept| crate::account::applies(kept.account.as_deref(), account.as_deref()));
        let entry = match outcome.result {
            Ok(mut reading) => Entry {
                provider,
                attempted_at: now,
                next_probe_at: outcome.next_probe_at,
                reading: Some({
                    reading.account = account;
                    reading
                }),
                failure: None,
            },
            Err(failure) => Entry {
                provider,
                attempted_at: now,
                next_probe_at: outcome.next_probe_at,
                reading: kept.filter(|r| now < r.observed_at.saturating_add(STALE_AFTER)),
                failure: Some(failure),
            },
        };
        self.entries.retain(|kept| kept.provider != provider);
        self.entries.push(entry);
        self.entries.sort_by_key(|entry| entry.provider);
    }
}

/// An HTTP answer, before parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    pub status: u16,
    /// `Retry-After` in seconds, when the header held a number.
    pub retry_after: Option<u64>,
    pub body: Vec<u8>,
}

/// Asks a provider's usage endpoint. [`fetch`] is the live one.
pub type Fetch = fn(Provider) -> Result<Response, Failure>;

/// What one probe learned, and when the provider may be probed again.
#[derive(Clone, Debug, PartialEq)]
pub struct Outcome {
    pub result: Result<Reading, Failure>,
    pub next_probe_at: u64,
}

/// Turn a fetched answer into a typed outcome at `now`.
#[must_use]
pub fn outcome(provider: Provider, fetched: Result<Response, Failure>, now: u64) -> Outcome {
    let backoff = |retry_after: Option<u64>, default: u64| {
        now + retry_after.map_or(default, |s| s.clamp(MIN_INTERVAL, MAX_RETRY_AFTER))
    };
    match fetched {
        Err(failure) => Outcome {
            result: Err(failure),
            next_probe_at: now + FAILURE_BACKOFF,
        },
        Ok(response) => match response.status {
            200..=299 => {
                let result = match provider {
                    Provider::Claude => parse_claude(&response.body, now),
                    Provider::Codex => parse_codex(&response.body, now),
                    Provider::Vertex | Provider::Devin | Provider::OpenCode | Provider::Grok => {
                        Err(Failure::Unsupported)
                    }
                };
                let next_probe_at = if result.is_ok() {
                    now + MIN_INTERVAL
                } else {
                    now + FAILURE_BACKOFF
                };
                Outcome {
                    result,
                    next_probe_at,
                }
            }
            401 | 403 => Outcome {
                result: Err(Failure::Unauthorized),
                next_probe_at: backoff(response.retry_after, FAILURE_BACKOFF),
            },
            429 => Outcome {
                result: Err(Failure::RateLimited),
                next_probe_at: backoff(response.retry_after, FAILURE_BACKOFF),
            },
            _ => Outcome {
                result: Err(Failure::Status),
                next_probe_at: backoff(response.retry_after, FAILURE_BACKOFF),
            },
        },
    }
}

/// Probe each of `providers` that is due at `now` with `fetch`, record
/// what each probe learned in `dir`, and return the book. Requests run
/// without the book's lock; the write merges under an exclusive lock.
/// A failed write still returns what was learned.
pub fn refresh(dir: &Path, providers: &[Provider], now: u64, fetch: Fetch) -> Book {
    refresh_with(dir, providers, &[], now, fetch, crate::account::identify)
}

/// [`refresh`], where each of `providers` also in `fresh` is probed when
/// [`Book::due_fresh`] rather than only when [`Book::due`]: a start about
/// to pass it over for capacity, or one the person asked for, reads it
/// now (#10105). `identify` names each provider's login now; each reading
/// keeps that login's fingerprint.
pub fn refresh_with(
    dir: &Path,
    providers: &[Provider],
    fresh: &[Provider],
    now: u64,
    fetch: Fetch,
    identify: crate::account::Identify,
) -> Book {
    let before = Book::load_with(dir, identify);
    let mut learned: Vec<(Provider, Outcome, Option<String>)> = Vec::new();
    for provider in providers {
        let due = if fresh.contains(provider) {
            before.due_fresh(*provider, now)
        } else {
            before.due(*provider, now)
        };
        if learned.iter().any(|(p, _, _)| p == provider)
            || !due
            || !Provider::PROBED.contains(provider)
        {
            continue;
        }
        // The store holds the salt each reading's fingerprint is made with.
        if learned.is_empty() {
            let _ = crate::capacity::open_private(dir, &dir.join(FILE));
        }
        let account = crate::account::current(dir, *provider, identify);
        learned.push((
            *provider,
            outcome(*provider, fetch(*provider), now),
            account,
        ));
    }
    if learned.is_empty() {
        return before;
    }
    match write(dir, &learned, now, identify) {
        Ok(book) => book,
        Err(error) => {
            eprintln!("openagents host: usage probe: {error}");
            let mut book = before;
            for (provider, outcome, account) in learned {
                book.apply(provider, now, outcome, account);
            }
            book
        }
    }
}

fn write(
    dir: &Path,
    learned: &[(Provider, Outcome, Option<String>)],
    now: u64,
    identify: crate::account::Identify,
) -> Result<Book, String> {
    let path = dir.join(FILE);
    let mut file = crate::capacity::open_private(dir, &path)?;
    file.lock()
        .map_err(|_| format!("cannot lock {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {}", path.display()))?;
    let mut book = Book::parse(&bytes).unwrap_or_default();
    for (provider, outcome, account) in learned {
        book.apply(*provider, now, outcome.clone(), account.clone());
    }
    let bytes = serde_json::to_vec_pretty(&book).map_err(|e| e.to_string())?;
    file.set_len(0)
        .and_then(|()| file.rewind())
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", path.display()))?;
    book.forget_other_logins(dir, identify);
    Ok(book)
}

/// A percent the provider reported, as a fraction from 0 to 1. A value
/// outside 0 to 100 (with a little slack for rounding) is malformed.
fn fraction(percent: f64) -> Result<f64, Failure> {
    if percent.is_finite() && (0.0..=100.5).contains(&percent) {
        Ok((percent / 100.0).min(1.0))
    } else {
        Err(Failure::Malformed)
    }
}

/// A reported reset, held to the capacity book's bound.
fn bounded(at: u64, now: u64) -> u64 {
    at.min(now.saturating_add(capacity::MAX_HOLD))
}

/// Read Claude's `/api/oauth/usage` body: the `five_hour` and `seven_day`
/// windows. Other windows and fields are ignored.
///
/// # Errors
/// [`Failure::Malformed`] for a body without either window, a
/// utilization outside 0 to 100, or a reset that is not RFC 3339.
pub fn parse_claude(body: &[u8], now: u64) -> Result<Reading, Failure> {
    #[derive(Deserialize)]
    struct Usage {
        #[serde(default)]
        five_hour: Option<Limit>,
        #[serde(default)]
        seven_day: Option<Limit>,
    }
    #[derive(Deserialize)]
    struct Limit {
        utilization: Option<f64>,
        #[serde(default)]
        resets_at: Option<String>,
    }
    let usage: Usage = serde_json::from_slice(body).map_err(|_| Failure::Malformed)?;
    let mut windows = Vec::new();
    for (name, length, limit) in [
        (WindowName::FiveHour, 5 * 3600, usage.five_hour),
        (WindowName::SevenDay, 7 * 86_400, usage.seven_day),
    ] {
        let Some(limit) = limit else { continue };
        let Some(utilization) = limit.utilization else {
            continue;
        };
        let resets_at = match limit.resets_at {
            Some(text) => Some(bounded(rfc3339(&text).ok_or(Failure::Malformed)?, now)),
            None => None,
        };
        windows.push(Window {
            window: name,
            used_fraction: fraction(utilization)?,
            resets_at,
            length_seconds: Some(length),
        });
    }
    if windows.is_empty() {
        return Err(Failure::Malformed);
    }
    Ok(Reading {
        provider: Provider::Claude,
        observed_at: now,
        windows,
        limit_reached: false,
        plan: None,
        account: None,
    })
}

/// Read the ChatGPT backend's `/wham/usage` body: `rate_limit`'s primary
/// and secondary windows and its `limit_reached` and `allowed` flags.
/// Account identifiers in the body are not read.
///
/// # Errors
/// [`Failure::Malformed`] for a body that is not that shape or a percent
/// outside 0 to 100.
pub fn parse_codex(body: &[u8], now: u64) -> Result<Reading, Failure> {
    #[derive(Deserialize)]
    struct Usage {
        #[serde(default)]
        plan_type: Option<String>,
        #[serde(default)]
        rate_limit: Option<RateLimit>,
    }
    #[derive(Deserialize)]
    struct RateLimit {
        #[serde(default)]
        allowed: Option<bool>,
        #[serde(default)]
        limit_reached: Option<bool>,
        #[serde(default)]
        primary_window: Option<Limit>,
        #[serde(default)]
        secondary_window: Option<Limit>,
    }
    #[derive(Deserialize)]
    struct Limit {
        used_percent: f64,
        #[serde(default)]
        limit_window_seconds: Option<u64>,
        #[serde(default)]
        reset_after_seconds: Option<u64>,
        #[serde(default)]
        reset_at: Option<u64>,
    }
    let usage: Usage = serde_json::from_slice(body).map_err(|_| Failure::Malformed)?;
    let mut reading = Reading {
        provider: Provider::Codex,
        observed_at: now,
        windows: Vec::new(),
        limit_reached: false,
        plan: usage
            .plan_type
            .filter(|plan| !plan.is_empty() && plan.len() <= 32),
        account: None,
    };
    let Some(limit) = usage.rate_limit else {
        return Ok(reading);
    };
    reading.limit_reached = limit.limit_reached == Some(true) || limit.allowed == Some(false);
    for (name, window) in [
        (WindowName::Primary, limit.primary_window),
        (WindowName::Secondary, limit.secondary_window),
    ] {
        let Some(window) = window else { continue };
        let resets_at = window
            .reset_at
            .or_else(|| window.reset_after_seconds.map(|s| now.saturating_add(s)))
            .map(|at| bounded(at, now));
        reading.windows.push(Window {
            window: name,
            used_fraction: fraction(window.used_percent)?,
            resets_at,
            length_seconds: window.limit_window_seconds,
        });
    }
    Ok(reading)
}

/// Unix seconds for an RFC 3339 timestamp such as
/// `2026-09-28T15:49:59.742050+00:00` or `2026-10-03T18:07:04Z`. The
/// fraction is dropped.
#[must_use]
pub fn rfc3339(text: &str) -> Option<u64> {
    let bytes = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = bytes.get(range)?;
        if part.is_empty() || !part.iter().all(u8::is_ascii_digit) {
            return None;
        }
        std::str::from_utf8(part).ok()?.parse().ok()
    };
    if bytes.len() < 20
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !matches!(bytes[10], b'T' | b't' | b' ')
        || bytes[13] != b':'
        || bytes[16] != b':'
    {
        return None;
    }
    let (year, month, day) = (digits(0..4)?, digits(5..7)?, digits(8..10)?);
    let (hour, minute, second) = (digits(11..13)?, digits(14..16)?, digits(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    if second > 60 {
        return None;
    }
    let mut rest = 19;
    if bytes[rest] == b'.' {
        rest += 1;
        let start = rest;
        while bytes.get(rest).is_some_and(u8::is_ascii_digit) {
            rest += 1;
        }
        if rest == start {
            return None;
        }
    }
    let offset = match bytes.get(rest..)? {
        [b'Z' | b'z'] => 0,
        [sign @ (b'+' | b'-'), _, _, b':', _, _] => {
            let hours = digits(rest + 1..rest + 3)?;
            let minutes = digits(rest + 4..rest + 6)?;
            if hours > 23 || minutes > 59 {
                return None;
            }
            let offset = hours * 3600 + minutes * 60;
            if *sign == b'+' { offset } else { -offset }
        }
        _ => return None,
    };
    // Days from 1970-01-01 in the proleptic Gregorian calendar.
    let (y, m) = if month <= 2 {
        (year - 1, month + 9)
    } else {
        (year, month - 3)
    };
    let era = y.div_euclid(400);
    let year_of_era = y - era * 400;
    let day_of_year = (153 * m + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    let seconds = days * 86_400 + hour * 3600 + minute * 60 + second - offset;
    u64::try_from(seconds).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: &str = include_str!("../fixtures/usage/claude-oauth-usage.json");
    const CODEX: &str = include_str!("../fixtures/usage/codex-wham-usage.json");
    const NOW: u64 = 1_790_572_210;

    fn ok(body: &str) -> Result<Response, Failure> {
        Ok(Response {
            status: 200,
            retry_after: None,
            body: body.as_bytes().to_vec(),
        })
    }

    #[test]
    fn rfc3339_reads_offsets_and_fractions() {
        assert_eq!(rfc3339("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(rfc3339("2026-10-03T18:07:04.513720Z"), Some(1_791_050_824));
        assert_eq!(
            rfc3339("2026-09-28T15:49:59.742050+00:00"),
            Some(1_790_610_599)
        );
        assert_eq!(
            rfc3339("2026-09-28T10:49:59-05:00"),
            rfc3339("2026-09-28T15:49:59Z")
        );
        for bad in [
            "",
            "2026-09-28",
            "2026-13-28T15:49:59Z",
            "2026-09-28T15:49:59",
            "2026-09-28T15:49:59.Z",
            "2026-09-28T15:49:59+0000",
            "yesterday at noon, give or take",
        ] {
            assert_eq!(rfc3339(bad), None, "{bad}");
        }
    }

    #[test]
    fn a_recorded_claude_answer_reads_both_windows() {
        let reading = parse_claude(CLAUDE.as_bytes(), NOW).unwrap();
        assert_eq!(reading.provider, Provider::Claude);
        assert_eq!(reading.windows.len(), 2);
        let five = &reading.windows[0];
        assert_eq!(five.window, WindowName::FiveHour);
        assert!((five.used_fraction - 0.04).abs() < 1e-9);
        assert_eq!(five.resets_at, rfc3339("2026-09-28T15:49:59Z"));
        let seven = &reading.windows[1];
        assert_eq!(seven.window, WindowName::SevenDay);
        assert!((seven.used_fraction - 0.66).abs() < 1e-9);
        assert_eq!(reading.fullest().unwrap().window, WindowName::SevenDay);
        assert!(!reading.limit_reached);
        assert!(!reading.near_limit(90));
        assert!(reading.near_limit(66));
    }

    #[test]
    fn the_limiting_reset_is_the_full_windows_or_the_reached_limits() {
        let mut reading = parse_claude(CLAUDE.as_bytes(), NOW).unwrap();
        assert_eq!(reading.limiting_reset(), None);
        reading.windows[1].used_fraction = 0.97;
        assert_eq!(reading.limiting_reset(), reading.windows[1].resets_at);
        reading.windows[0].used_fraction = 1.0;
        let latest = reading.windows.iter().filter_map(|w| w.resets_at).max();
        assert_eq!(reading.limiting_reset(), latest);
        // Codex says its limit is reached: its fullest window's reset.
        let codex = parse_codex(CODEX.as_bytes(), NOW).unwrap();
        assert_eq!(codex.limiting_reset(), Some(1_791_050_824));
    }

    #[test]
    fn a_recorded_codex_answer_reads_its_window_and_limit() {
        let reading = parse_codex(CODEX.as_bytes(), NOW).unwrap();
        assert_eq!(reading.provider, Provider::Codex);
        assert!(reading.limit_reached);
        assert_eq!(reading.plan.as_deref(), Some("pro"));
        assert_eq!(reading.windows.len(), 1);
        let primary = &reading.windows[0];
        assert_eq!(primary.window, WindowName::Primary);
        assert!((primary.used_fraction - 1.0).abs() < 1e-9);
        assert_eq!(primary.resets_at, Some(1_791_050_824));
        assert_eq!(primary.length_seconds, Some(604_800));
        assert!(reading.near_limit(100));
        // The account identifiers in the body are not kept.
        let text = serde_json::to_string(&reading).unwrap();
        assert!(!text.contains("redacted") && !text.contains("example.invalid"));
    }

    #[test]
    fn malformed_answers_are_typed_failures() {
        for body in [
            "",
            "{",
            "[]",
            r#"{"five_hour":null,"seven_day":null}"#,
            r#"{"five_hour":{"utilization":140.0,"resets_at":null}}"#,
            r#"{"five_hour":{"utilization":4.0,"resets_at":"soon"}}"#,
        ] {
            assert_eq!(
                parse_claude(body.as_bytes(), NOW),
                Err(Failure::Malformed),
                "{body}"
            );
        }
        for body in [
            "",
            r#"{"rate_limit":{"primary_window":{"used_percent":-3}}}"#,
            r#"{"rate_limit":{"primary_window":{"used_percent":"most"}}}"#,
        ] {
            assert_eq!(
                parse_codex(body.as_bytes(), NOW),
                Err(Failure::Malformed),
                "{body}"
            );
        }
        // No rate limit at all is a reading without windows.
        let reading = parse_codex(br#"{"plan_type":"pro"}"#, NOW).unwrap();
        assert!(reading.windows.is_empty() && !reading.near_limit(1));
    }

    #[test]
    fn http_failures_back_off_and_honor_retry_after() {
        let answer = |status, retry_after| {
            outcome(
                Provider::Claude,
                Ok(Response {
                    status,
                    retry_after,
                    body: b"{\"error\":{\"type\":\"rate_limit_error\"}}".to_vec(),
                }),
                NOW,
            )
        };
        let limited = answer(429, Some(1704));
        assert_eq!(limited.result, Err(Failure::RateLimited));
        assert_eq!(limited.next_probe_at, NOW + 1704);
        assert_eq!(answer(429, Some(1)).next_probe_at, NOW + MIN_INTERVAL);
        assert_eq!(
            answer(429, Some(10_000_000)).next_probe_at,
            NOW + MAX_RETRY_AFTER
        );
        assert_eq!(answer(401, None).result, Err(Failure::Unauthorized));
        assert_eq!(answer(401, None).next_probe_at, NOW + FAILURE_BACKOFF);
        assert_eq!(answer(500, None).result, Err(Failure::Status));
        let malformed = answer(200, None);
        assert_eq!(malformed.result, Err(Failure::Malformed));
        assert_eq!(malformed.next_probe_at, NOW + FAILURE_BACKOFF);
        let offline = outcome(Provider::Codex, Err(Failure::Network), NOW);
        assert_eq!(offline.next_probe_at, NOW + FAILURE_BACKOFF);
        let fine = outcome(Provider::Codex, ok(CODEX), NOW);
        assert!(fine.result.is_ok());
        assert_eq!(fine.next_probe_at, NOW + MIN_INTERVAL);
    }

    fn login_a(_: Provider) -> Option<String> {
        Some("acct-a".into())
    }
    fn login_b(_: Provider) -> Option<String> {
        Some("acct-b".into())
    }

    /// A reading keeps its login's fingerprint; once another login is
    /// signed in, readers drop it and the provider is due for a probe at
    /// once, which reads the new login (#10105).
    #[test]
    fn a_reading_from_another_login_is_dropped_and_probed_again() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        fn full(_: Provider) -> Result<Response, Failure> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            ok(
                r#"{"five_hour":{"utilization":100.0,"resets_at":"2026-09-28T15:49:59Z"},"seven_day":{"utilization":100.0,"resets_at":null}}"#,
            )
        }
        fn calm(_: Provider) -> Result<Response, Failure> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            ok(
                r#"{"five_hour":{"utilization":6.0,"resets_at":null},"seven_day":{"utilization":2.0,"resets_at":null}}"#,
            )
        }
        let dir = tempfile::tempdir().unwrap();
        let claude = [Provider::Claude];
        let book = refresh_with(dir.path(), &claude, &[], NOW, full, login_a);
        assert!(book.near_limit(Provider::Claude, 90, NOW));
        assert!(!book.due(Provider::Claude, NOW + 10));
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains("acct-a"));
        // The same login: cached. Another login: the old reading is gone
        // and the provider is due now.
        let same = refresh_with(dir.path(), &claude, &[], NOW + 10, calm, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert!(same.near_limit(Provider::Claude, 90, NOW + 10));
        let switched = Book::load_with(dir.path(), login_b);
        assert!(switched.entry(Provider::Claude).is_none());
        assert!(switched.due(Provider::Claude, NOW + 10));
        let fresh = refresh_with(dir.path(), &claude, &[], NOW + 10, calm, login_b);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert!(!fresh.near_limit(Provider::Claude, 90, NOW + 10));
        assert!(fresh.reading(Provider::Claude, NOW + 10).is_some());
        // A reading kept across a failure is never another login's.
        let mut book = fresh.clone();
        book.apply(
            Provider::Claude,
            NOW + 80,
            outcome(Provider::Claude, Err(Failure::Network), NOW + 80),
            Some("other".into()),
        );
        assert!(book.entry(Provider::Claude).unwrap().reading.is_none());
    }

    /// A start about to pass a provider over, or one the person asked for,
    /// reads it now: within [`MIN_INTERVAL`], and past a failure's backoff,
    /// but never against a provider's own `Retry-After`.
    #[test]
    fn a_fresh_probe_skips_the_cache_but_not_a_retry_after() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static LIMITED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn fake(_: Provider) -> Result<Response, Failure> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            if LIMITED.load(Ordering::SeqCst) {
                return Ok(Response {
                    status: 429,
                    retry_after: Some(600),
                    body: Vec::new(),
                });
            }
            ok(CLAUDE)
        }
        let dir = tempfile::tempdir().unwrap();
        let claude = [Provider::Claude];
        refresh_with(dir.path(), &claude, &claude, NOW, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        // Within FRESH_WITHIN a burst asks once.
        refresh_with(dir.path(), &claude, &claude, NOW + 2, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        // Not due by the cache, but asked fresh.
        let book = refresh_with(dir.path(), &claude, &[], NOW + 20, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 1);
        assert!(!book.needs_recheck(Provider::Claude, NOW + 20));
        assert!(book.needs_recheck(Provider::Claude, NOW + RECHECK_AFTER));
        refresh_with(dir.path(), &claude, &claude, NOW + 20, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        LIMITED.store(true, Ordering::SeqCst);
        refresh_with(dir.path(), &claude, &claude, NOW + 40, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 3);
        refresh_with(dir.path(), &claude, &claude, NOW + 100, fake, login_a);
        assert_eq!(CALLS.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn refresh_caches_records_privately_and_keeps_a_reading_across_failures() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static CALLS: AtomicUsize = AtomicUsize::new(0);
        static FAIL: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        fn fake(provider: Provider) -> Result<Response, Failure> {
            CALLS.fetch_add(1, Ordering::SeqCst);
            if FAIL.load(Ordering::SeqCst) {
                return Err(Failure::Network);
            }
            match provider {
                Provider::Claude => ok(CLAUDE),
                Provider::Codex => Err(Failure::NoCredential),
                Provider::Vertex | Provider::Devin | Provider::OpenCode | Provider::Grok => {
                    Err(Failure::Unsupported)
                }
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let both = [Provider::Codex, Provider::Claude, Provider::Claude];
        let book = refresh(dir.path(), &both, NOW, fake);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert!(book.reading(Provider::Claude, NOW).is_some());
        assert_eq!(
            book.entry(Provider::Codex).unwrap().failure,
            Some(Failure::NoCredential)
        );
        assert!(!book.near_limit(Provider::Codex, 90, NOW));
        // Cached: nothing is due within the interval.
        let again = refresh(dir.path(), &both, NOW + 30, fake);
        assert_eq!(CALLS.load(Ordering::SeqCst), 2);
        assert_eq!(again, Book::load(dir.path()));
        // A later failure keeps the fresh reading and says why.
        FAIL.store(true, Ordering::SeqCst);
        let failed = refresh(dir.path(), &[Provider::Claude], NOW + MIN_INTERVAL, fake);
        let entry = failed.entry(Provider::Claude).unwrap();
        assert_eq!(entry.failure, Some(Failure::Network));
        assert!(entry.reading.is_some());
        assert!(
            failed
                .describe(Provider::Claude, NOW + MIN_INTERVAL)
                .contains("latest probe: network")
        );
        // A stale reading is not used.
        assert!(
            failed
                .reading(Provider::Claude, NOW + STALE_AFTER)
                .is_none()
        );
        assert_eq!(
            failed.describe(Provider::Codex, NOW),
            "unknown (probe: no_credential)"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(dir.path().join(FILE)).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        // A malformed book reads as empty.
        std::fs::write(dir.path().join(FILE), b"{").unwrap();
        assert_eq!(Book::load(dir.path()), Book::default());
    }
}
