//! The retry policy, its delay, and the server delay headers it honors.
//!
//! The defaults are the ones both official SDKs ship: two retries, 500 ms
//! doubling to 5 s with a quarter of each delay taken off at random, and the
//! statuses a server sends when a later attempt can succeed.

use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use reqwest::header::HeaderMap;

use crate::Result;
use crate::error::Error;

/// The header a server sends to ask for a wait in milliseconds. It wins over
/// `Retry-After`, as it does in both official SDKs.
const RETRY_AFTER_MS_HEADER: &str = "retry-after-ms";

/// The header a server sends to ask for a wait in seconds or as a date.
const RETRY_AFTER_HEADER: &str = "retry-after";

/// The header an attempt after the first carries.
pub(crate) const RETRY_COUNT_HEADER: &str = "x-typesafe-retry-count";

/// A caller's own test of whether a failure is worth another attempt.
pub type RetryPredicate = Arc<dyn Fn(&Error) -> bool + Send + Sync>;

/// What to retry, how long to wait, and when to stop.
///
/// Every field is public so a caller builds one with struct update syntax over
/// the default or over the client's own policy:
///
/// ```
/// use jev::RetryPolicy;
///
/// let policy = RetryPolicy {
///     max_retries: 4,
///     ..RetryPolicy::default()
/// };
/// assert_eq!(policy.backoff_jitter, 0.25);
/// ```
#[derive(Clone)]
pub struct RetryPolicy {
    /// Attempts after the first. Zero retries nothing.
    pub max_retries: u32,
    /// The first delay, doubled each attempt up to `backoff_max`.
    pub backoff_initial: Duration,
    /// The longest delay the doubling reaches.
    pub backoff_max: Duration,
    /// The fraction of each delay taken off at random, from 0 to 1.
    pub backoff_jitter: f64,
    /// The statuses a retry follows.
    pub http_statuses: HashSet<u16>,
    /// Whether a server delay header replaces the computed delay.
    pub respect_retry_after: bool,
    /// The longest server delay honored. A longer one falls back to the
    /// computed delay.
    pub max_retry_after: Duration,
    /// Whether a request that never reached the API is retried.
    pub connection_errors: bool,
    /// Whether an attempt that ran past its timeout is retried.
    pub timeouts: bool,
    /// The whole call's budget, including the first attempt and every delay.
    /// `None`, the default, sets no budget. A retry whose delay would reach
    /// the budget does not run, and the last failure is returned.
    pub budget: Option<Duration>,
    /// A caller's own test, asked in addition to the rules above.
    pub predicate: Option<RetryPredicate>,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        let mut http_statuses: HashSet<u16> = (500..=599).collect();
        http_statuses.insert(408);
        http_statuses.insert(429);
        Self {
            max_retries: 2,
            backoff_initial: Duration::from_millis(500),
            backoff_max: Duration::from_secs(5),
            backoff_jitter: 0.25,
            http_statuses,
            respect_retry_after: true,
            max_retry_after: Duration::from_secs(60),
            connection_errors: true,
            timeouts: true,
            budget: None,
            predicate: None,
        }
    }
}

impl fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut statuses: Vec<u16> = self.http_statuses.iter().copied().collect();
        statuses.sort_unstable();
        f.debug_struct("RetryPolicy")
            .field("max_retries", &self.max_retries)
            .field("backoff_initial", &self.backoff_initial)
            .field("backoff_max", &self.backoff_max)
            .field("backoff_jitter", &self.backoff_jitter)
            .field("http_statuses", &statuses)
            .field("respect_retry_after", &self.respect_retry_after)
            .field("max_retry_after", &self.max_retry_after)
            .field("connection_errors", &self.connection_errors)
            .field("timeouts", &self.timeouts)
            .field("budget", &self.budget)
            .field("predicate", &self.predicate.as_ref().map(|_| "set"))
            .finish()
    }
}

impl RetryPolicy {
    /// Check every field, the way both official SDKs check theirs.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Config`] when the jitter falls outside 0 to 1, a
    /// status falls outside 100 to 999, or the budget is zero.
    pub fn validate(&self) -> Result<()> {
        if !(0.0..=1.0).contains(&self.backoff_jitter) {
            return Err(Error::Config(format!(
                "`retry.backoff_jitter` must be between 0 and 1, got {}",
                self.backoff_jitter
            )));
        }
        if let Some(status) = self
            .http_statuses
            .iter()
            .find(|status| !(100..=999).contains(*status))
        {
            return Err(Error::Config(format!(
                "`retry.http_statuses` must hold HTTP status codes, got {status}"
            )));
        }
        if self.budget == Some(Duration::ZERO) {
            return Err(Error::Config(
                "`retry.budget` must be a positive duration".to_string(),
            ));
        }
        Ok(())
    }

    /// Whether a status is one this policy retries.
    #[must_use]
    pub fn retries_status(&self, status: u16) -> bool {
        self.http_statuses.contains(&status)
    }

    /// Whether a failure is one this policy retries.
    #[must_use]
    pub fn retries_error(&self, error: &Error) -> bool {
        let built_in = match error {
            Error::Timeout { .. } => self.timeouts,
            Error::Connection { .. } => self.connection_errors,
            Error::Api(api) => self.retries_status(api.status),
            _ => false,
        };
        built_in
            || self
                .predicate
                .as_ref()
                .is_some_and(|predicate| predicate(error))
    }

    /// The wait before the retry that follows a zero-based attempt.
    #[must_use]
    pub fn delay(&self, attempt: u32, headers: Option<&HeaderMap>) -> Duration {
        self.delay_with(attempt, headers, rand::random::<f64>())
    }

    /// [`RetryPolicy::delay`] with the random draw supplied, so a test reads a
    /// fixed delay.
    #[must_use]
    pub fn delay_with(&self, attempt: u32, headers: Option<&HeaderMap>, random: f64) -> Duration {
        if self.respect_retry_after
            && let Some(asked) = headers.and_then(parse_retry_after)
            && asked <= self.max_retry_after
        {
            return asked;
        }
        let doubled = match 1u32.checked_shl(attempt) {
            Some(factor) => self.backoff_initial.saturating_mul(factor),
            None => self.backoff_max,
        }
        .min(self.backoff_max);
        let scale = 1.0 - random.clamp(0.0, 1.0) * self.backoff_jitter;
        Duration::try_from_secs_f64(doubled.as_secs_f64() * scale).unwrap_or(doubled)
    }
}

/// The wait a response asks for, read from `retry-after-ms` first and
/// `Retry-After` second.
///
/// `Retry-After` carries either a count of seconds or an HTTP date. A negative
/// count and an unreadable value name no wait.
#[must_use]
pub fn parse_retry_after(headers: &HeaderMap) -> Option<Duration> {
    parse_retry_after_at(headers, SystemTime::now())
}

/// [`parse_retry_after`] against a fixed `now`, so a test reads a date header
/// without waiting for the clock.
#[must_use]
pub fn parse_retry_after_at(headers: &HeaderMap, now: SystemTime) -> Option<Duration> {
    if let Some(raw) = header(headers, RETRY_AFTER_MS_HEADER)
        && let Some(delay) = parse_seconds(raw, 0.001)
    {
        return Some(delay);
    }
    let raw = header(headers, RETRY_AFTER_HEADER)?;
    if let Some(delay) = parse_seconds(raw, 1.0) {
        return Some(delay);
    }
    if raw.trim().parse::<f64>().is_ok() {
        // A readable count that is negative asks for no wait.
        return None;
    }
    let at = httpdate::parse_http_date(raw.trim()).ok()?;
    Some(at.duration_since(now).unwrap_or(Duration::ZERO))
}

/// One header as text, when the response carries it and it is text.
fn header<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

/// A count of units, scaled to seconds. A blank value counts as zero, the way
/// the Python SDK reads it.
fn parse_seconds(raw: &str, unit: f64) -> Option<Duration> {
    let trimmed = raw.trim();
    let count: f64 = if trimmed.is_empty() {
        0.0
    } else {
        trimmed.parse().ok()?
    };
    if !count.is_finite() || count < 0.0 {
        return None;
    }
    Duration::try_from_secs_f64(count * unit).ok()
}
