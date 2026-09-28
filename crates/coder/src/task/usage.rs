//! Provider usage windows: how much of a login's allowance is used, and
//! when each window resets, read from the provider before a run starts.
//!
//! [`super::capacity`] learns that a provider is out only from a refusal.
//! When the owner turns usage probes on (`coder host autostart on
//! --probe-usage`), the host also asks each admitted provider's usage
//! endpoint and keeps the typed answer in `usage.json` beside
//! `capacity.json`:
//!
//! | Provider | Endpoint | Windows |
//! | --- | --- | --- |
//! | Claude | `GET https://api.anthropic.com/api/oauth/usage` (`anthropic-beta: oauth-2025-04-20`) | `five_hour`, `seven_day`: `utilization` percent and `resets_at` |
//! | Codex | `GET https://chatgpt.com/backend-api/wham/usage` | `primary_window`, `secondary_window`: `used_percent`, `limit_window_seconds`, `reset_at`; and `limit_reached` |
//!
//! Both endpoints are private and undocumented. A probe result is
//! advisory: routing prefers an admitted route whose provider is below the
//! owner's threshold, and a refusal in the capacity book stays the
//! authority. Any failure (no credential, an expired one, a refused or
//! rate-limited request, a malformed body, no network) is recorded as a
//! typed [`Failure`] and routing falls back to refusal-only capacity.
//!
//! Probes are cached: a provider is asked at most once per
//! [`MIN_INTERVAL`], a failure waits [`FAILURE_BACKOFF`], and a
//! `Retry-After` is honored up to [`MAX_RETRY_AFTER`]. A reading older than
//! [`STALE_AFTER`] is not used.
//!
//! **Credentials.** A probe reads the provider's OAuth access token: the
//! Codex login `codex_transport::codex::Login::load` reads, and Claude Code's
//! `claudeAiOauth.accessToken` from `~/.claude/.credentials.json` or, on
//! macOS, the `Claude Code-credentials` keychain item (read with
//! `/usr/bin/security`, as Claude Code itself reads it). The token is sent
//! only to that provider's own usage endpoint, is never written, logged,
//! or stored in `usage.json`, and no credential store is ever modified.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use super::capacity::{self, Provider};

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
/// The utilization, in percent, at or above which routing prefers another
/// route when the policy names no threshold.
pub const DEFAULT_THRESHOLD_PERCENT: u8 = 90;
/// Claude Code's OAuth usage endpoint.
pub const CLAUDE_USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// The beta header value the Claude endpoint requires.
pub const CLAUDE_OAUTH_BETA: &str = "oauth-2025-04-20";
/// The ChatGPT backend's Codex usage endpoint.
pub const CODEX_USAGE_URL: &str = "https://chatgpt.com/backend-api/wham/usage";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(10);
const KEYCHAIN_SERVICE: &str = "Claude Code-credentials";

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
}

impl Reading {
    /// The window with the most use.
    #[must_use]
    pub fn fullest(&self) -> Option<&Window> {
        self.windows
            .iter()
            .max_by(|a, b| a.used_fraction.total_cmp(&b.used_fraction))
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
    /// The book in `dir`. Missing, unreadable, or malformed is empty.
    #[must_use]
    pub fn load(dir: &Path) -> Book {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| Book::parse(&bytes))
            .unwrap_or_default()
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

    /// Whether `provider` may be probed at `now`.
    #[must_use]
    pub fn due(&self, provider: Provider, now: u64) -> bool {
        self.entry(provider)
            .is_none_or(|entry| now >= entry.next_probe_at)
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

    fn apply(&mut self, provider: Provider, now: u64, outcome: Outcome) {
        let kept = self.entry(provider).and_then(|entry| entry.reading.clone());
        let entry = match outcome.result {
            Ok(reading) => Entry {
                provider,
                attempted_at: now,
                next_probe_at: outcome.next_probe_at,
                reading: Some(reading),
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
    let before = Book::load(dir);
    let mut learned: Vec<(Provider, Outcome)> = Vec::new();
    for provider in providers {
        if learned.iter().any(|(p, _)| p == provider) || !before.due(*provider, now) {
            continue;
        }
        learned.push((*provider, outcome(*provider, fetch(*provider), now)));
    }
    if learned.is_empty() {
        return before;
    }
    match write(dir, &learned, now) {
        Ok(book) => book,
        Err(error) => {
            eprintln!("coder host: usage probe: {error}");
            let mut book = before;
            for (provider, outcome) in learned {
                book.apply(provider, now, outcome);
            }
            book
        }
    }
}

fn write(dir: &Path, learned: &[(Provider, Outcome)], now: u64) -> Result<Book, String> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| format!("cannot create {}", dir.display()))?;
    let path = dir.join(FILE);
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|_| format!("cannot open {}", path.display()))?;
    file.lock()
        .map_err(|_| format!("cannot lock {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {}", path.display()))?;
    let mut book = Book::parse(&bytes).unwrap_or_default();
    for (provider, outcome) in learned {
        book.apply(*provider, now, outcome.clone());
    }
    let bytes = serde_json::to_vec_pretty(&book).map_err(|e| e.to_string())?;
    file.set_len(0)
        .and_then(|()| file.rewind())
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", path.display()))?;
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

/// Ask `provider`'s usage endpoint with its local login. Runs on its own
/// thread with its own runtime, so it can be called from any context.
///
/// # Errors
/// A typed [`Failure`] when there is no usable credential or the request
/// does not complete.
pub fn fetch(provider: Provider) -> Result<Response, Failure> {
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Failure::Network)?;
        runtime.block_on(fetch_async(provider))
    })
    .join()
    .unwrap_or(Err(Failure::Network))
}

async fn fetch_async(provider: Provider) -> Result<Response, Failure> {
    let http = reqwest::Client::builder()
        .user_agent(concat!("openagents-coder/", env!("CARGO_PKG_VERSION")))
        .timeout(REQUEST_TIMEOUT)
        .build()
        .map_err(|_| Failure::Network)?;
    let request = match provider {
        Provider::Codex => {
            let path =
                codex_transport::codex::Login::default_path().ok_or(Failure::NoCredential)?;
            let login =
                codex_transport::codex::Login::load(&path).map_err(|error| match error {
                    codex_transport::codex::LoginError::Expiring { .. } => Failure::Expired,
                    _ => Failure::NoCredential,
                })?;
            login.authorize(http.get(CODEX_USAGE_URL))
        }
        Provider::Claude => {
            let token = claude_token(now_millis())?;
            http.get(CLAUDE_USAGE_URL)
                .bearer_auth(&token.0)
                .header("anthropic-beta", CLAUDE_OAUTH_BETA)
        }
    };
    let response = request.send().await.map_err(|_| Failure::Network)?;
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = response.bytes().await.map_err(|_| Failure::Network)?;
    Ok(Response {
        status,
        retry_after,
        body: body.to_vec(),
    })
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Claude Code's OAuth access token. Its `Debug` output hides it.
struct ClaudeToken(String);

impl std::fmt::Debug for ClaudeToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ClaudeToken(<redacted>)")
    }
}

/// The access token in Claude Code's credential record, checked for
/// expiry at `now_ms`.
fn claude_credential(bytes: &[u8], now_ms: u64) -> Result<ClaudeToken, Failure> {
    #[derive(Deserialize)]
    struct Record {
        #[serde(rename = "claudeAiOauth")]
        oauth: Option<OAuth>,
    }
    #[derive(Deserialize)]
    struct OAuth {
        #[serde(rename = "accessToken")]
        access_token: Option<String>,
        #[serde(default, rename = "expiresAt")]
        expires_at: Option<u64>,
    }
    let record: Record = serde_json::from_slice(bytes).map_err(|_| Failure::NoCredential)?;
    let oauth = record.oauth.ok_or(Failure::NoCredential)?;
    let token = oauth
        .access_token
        .filter(|token| !token.is_empty())
        .ok_or(Failure::NoCredential)?;
    if oauth.expires_at.is_some_and(|at| at <= now_ms) {
        return Err(Failure::Expired);
    }
    Ok(ClaudeToken(token))
}

/// Claude Code's token: `~/.claude/.credentials.json`, else on macOS the
/// keychain item Claude Code writes, for this account.
fn claude_token(now_ms: u64) -> Result<ClaudeToken, Failure> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or(Failure::NoCredential)?;
    if let Ok(bytes) = std::fs::read(home.join(".claude/.credentials.json")) {
        return claude_credential(&bytes, now_ms);
    }
    if cfg!(target_os = "macos") {
        let bytes = keychain_record().ok_or(Failure::NoCredential)?;
        return claude_credential(&bytes, now_ms);
    }
    Err(Failure::NoCredential)
}

/// The `Claude Code-credentials` generic password for this account, read
/// with `/usr/bin/security` and a timeout, so a locked keychain cannot
/// hold the host.
fn keychain_record() -> Option<Vec<u8>> {
    use std::process::{Command, Stdio};
    let account = std::env::var_os("USER")
        .filter(|user| !user.is_empty())
        .or_else(account_name)?;
    let mut child = Command::new("/usr/bin/security")
        .arg("find-generic-password")
        .arg("-s")
        .arg(KEYCHAIN_SERVICE)
        .arg("-a")
        .arg(account)
        .arg("-w")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes).map(|_| bytes)
    });
    let deadline = std::time::Instant::now() + KEYCHAIN_TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
        }
    };
    let bytes = reader.join().ok()?.ok()?;
    status.filter(std::process::ExitStatus::success)?;
    Some(bytes)
}

/// This process's account name from the account database.
fn account_name() -> Option<std::ffi::OsString> {
    use std::os::unix::ffi::OsStrExt;
    // SAFETY: getpwuid returns a pointer into static storage or null; the
    // name is copied before any other call that could reuse it.
    unsafe {
        let entry = libc::getpwuid(libc::getuid());
        if entry.is_null() || (*entry).pw_name.is_null() {
            return None;
        }
        let name = std::ffi::CStr::from_ptr((*entry).pw_name);
        Some(std::ffi::OsStr::from_bytes(name.to_bytes()).to_os_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAUDE: &str = include_str!("../../fixtures/usage/claude-oauth-usage.json");
    const CODEX: &str = include_str!("../../fixtures/usage/codex-wham-usage.json");
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
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(dir.path().join(FILE)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        // A malformed book reads as empty.
        std::fs::write(dir.path().join(FILE), b"{").unwrap();
        assert_eq!(Book::load(dir.path()), Book::default());
    }

    #[test]
    fn a_claude_credential_is_read_typed_and_never_printed() {
        let record = br#"{"claudeAiOauth":{"accessToken":"sk-ant-oat-secret","expiresAt":2000,"refreshToken":"r"},"mcpOAuth":{}}"#;
        let token = claude_credential(record, 1_000).unwrap();
        assert!(!format!("{token:?}").contains("secret"));
        assert_eq!(
            claude_credential(record, 2_000).unwrap_err(),
            Failure::Expired
        );
        assert_eq!(
            claude_credential(br#"{"mcpOAuth":{}}"#, 1_000).unwrap_err(),
            Failure::NoCredential
        );
        assert_eq!(
            claude_credential(b"not json", 1_000).unwrap_err(),
            Failure::NoCredential
        );
    }
}
