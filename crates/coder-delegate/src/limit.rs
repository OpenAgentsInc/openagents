//! Usage and rate limits a delegate session reports.
//!
//! A subscription account shares one quota across every session that
//! uses it. When the quota runs out, Claude Code ends its session with a
//! `result` event whose `api_error_status` is 429 and whose text reads
//! "You've hit your session limit · resets 11:50am (UTC)", after a
//! `rate_limit_event` whose status is `rejected`. Codex ends its turn with
//! an `error` event and a `turn.failed` event that say "You've hit your
//! usage limit" and, when it knows, "Try again at 3:04 PM".
//!
//! A throttled session did no work worth grading, and every later session
//! on the same account is throttled too. So the episode stops at the
//! first limited session, ends with [`EXIT_CODE`] and [`OUTCOME`], and
//! records the limit with its reset time, so the harness can requeue the
//! trial and pause until the quota resets.

use serde_json::{Value, json};

/// The schema of a limit's record.
pub const SCHEMA: &str = "coder-one.usage-limit.v1";

/// The episode's exit code when a delegate session hit a usage limit.
pub const EXIT_CODE: i32 = 6;

/// The episode's outcome when a delegate session hit a usage limit.
pub const OUTCOME: &str = "usage_limited";

/// The key a delegate call's `extra` and the episode manifest carry the
/// limit's record under.
pub const KEY: &str = "usage_limit";

pub use coder_engine_status::limit::{parse_iso, reset_from_message, says_limited};

/// One usage or rate limit a session hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Limit {
    /// The account's provider: `anthropic` or `openai`.
    pub provider: String,
    /// What the session said, verbatim.
    pub message: String,
    /// When the limit resets, in seconds since the epoch, when known.
    pub resets_at: Option<u64>,
    /// Where the reset time came from: `rate_limit_event` or `message`.
    pub reset_source: Option<String>,
    /// The limit's window, such as `five_hour`, when the stream named it.
    pub window: Option<String>,
    /// The HTTP status the session reported, when it did.
    pub status: Option<u64>,
}

impl Limit {
    /// A limit read from `message` alone, with the reset time it states
    /// read against `now` (seconds since the epoch).
    #[must_use]
    pub fn from_message(provider: &str, message: &str, now: u64) -> Self {
        let resets_at = reset_from_message(message, now);
        Limit {
            provider: provider.to_string(),
            message: message.trim().to_string(),
            resets_at,
            reset_source: resets_at.map(|_| "message".to_string()),
            window: None,
            status: None,
        }
    }

    /// The limit as the call's `extra`, the manifest, and the composition
    /// record it.
    #[must_use]
    pub fn record(&self) -> Value {
        json!({
            "schema": SCHEMA,
            "provider": self.provider,
            "message": self.message,
            "resets_at": self.resets_at,
            "resets_at_iso": self.resets_at.map(|at| atif::document::iso(at * 1_000)),
            "reset_source": self.reset_source,
            "window": self.window,
            "status": self.status,
        })
    }
}

/// The provider an executor's agent bills: `anthropic` for Claude Code,
/// `openai` for Codex.
#[must_use]
pub fn provider_of(agent: &str) -> &'static str {
    if agent.contains("codex") {
        "openai"
    } else if agent.contains("claude") {
        "anthropic"
    } else {
        "unknown"
    }
}

/// The first usage limit any delegate call in `steps` recorded, as its
/// record. The first one is the one that stopped the episode.
#[must_use]
pub fn from_steps(steps: &[atif::document::Step]) -> Option<Value> {
    steps
        .iter()
        .filter_map(|step| step.call.as_ref())
        .filter(|call| call.name == "delegate")
        .find_map(|call| call.extra.get(KEY).filter(|v| v.is_object()).cloned())
}

/// Seconds since the epoch now.
#[must_use]
pub fn now() -> u64 {
    atif::document::now_ms() / 1_000
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-23T11:30:58Z, when the retained sessions were throttled.
    const NOW: u64 = 1_790_163_058;
    // 2026-09-23T11:50:00Z, the `resetsAt` their streams carried.
    const RESET: u64 = 1_790_164_200;

    #[test]
    fn a_limit_records_its_reset_in_both_forms() {
        let limit = Limit::from_message(
            "anthropic",
            "You've hit your session limit · resets 11:50am (UTC)",
            NOW,
        );
        let record = limit.record();
        assert_eq!(record["schema"], SCHEMA);
        assert_eq!(record["resets_at"], RESET);
        assert_eq!(record["resets_at_iso"], "2026-09-23T11:50:00.000Z");
        assert_eq!(record["reset_source"], "message");
        assert_eq!(provider_of("claude-code"), "anthropic");
        assert_eq!(provider_of("codex"), "openai");
    }
}
