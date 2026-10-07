//! Model-provider capacity: which providers a host can reach, and which of
//! them refused work for a usage or rate limit, until when.
//!
//! The book lives in the Microcoder loop crate
//! ([`microcoder_loop::capacity`]), because the loop's failover writes it
//! and `coder` runs the loop in its delegate door. This module re-exports
//! it under the task store's name, so the auto-start policy, the usage
//! probes, and the loop read one `capacity.json`.
//!
//! It also holds the detector that turns what a delegate printed into a
//! refusal for the book ([`detect`], [`from_limit`]): Claude Code and Codex
//! sessions that Coder delegates to, ACP agents' refusal messages, and a
//! limit an orchestrator outside Coder saw (`openagents capacity record`).

use std::path::{Path, PathBuf};

pub use microcoder_loop::capacity::*;

use coder_delegate::limit::{self, Limit};

/// The task store directory that holds the book for this user:
/// `~/.openagents/tasks`, or `None` without a home.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| background::task_store(Path::new(&home)))
}

/// The refusal a delegate's limit carries, for `provider`'s login. A limit
/// that names a usage, session, or weekly allowance, or a window, is a
/// usage limit; any other (a bare 429) is a rate limit. A limit without a
/// reset holds for the provider's unknown-reset hold.
#[must_use]
pub fn from_limit(provider: Provider, limit: &Limit, now: u64) -> Refusal {
    let message = limit.message.to_lowercase();
    let usage = limit.window.is_some()
        || [
            "usage limit",
            "session limit",
            "weekly limit",
            "hit your limit",
        ]
        .iter()
        .any(|phrase| message.contains(phrase));
    let kind = if usage {
        Kind::UsageLimit
    } else {
        Kind::RateLimit
    };
    let mut refusal = Refusal::new(provider, kind, now, limit.resets_at);
    refusal.window_minutes = limit.window.as_deref().and_then(window_minutes);
    refusal
}

/// The refusal `output` reports for `provider`, if any line of it says a
/// usage or rate limit stopped the work. `output` is what a delegate
/// printed: Claude Code's or Codex's error text, a Codex
/// `usage_limit_reached` error body, or an ACP agent's refusal message.
/// Pass error text only, never what a model wrote, since a model can
/// quote a limit message.
#[must_use]
pub fn detect(provider: Provider, output: &str, now: u64) -> Option<Refusal> {
    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // A Codex error body, typed: the reset comes from its own fields.
        if provider == Provider::Codex
            && let Some(refusal) = Refusal::codex(429, line, now)
        {
            return Some(refusal);
        }
        if limit::says_limited(line) {
            let limit = Limit::from_message(provider.as_str(), line, now);
            return Some(from_limit(provider, &limit, now));
        }
    }
    None
}

/// Record the refusal `output` reports for `provider` in the book in
/// `dir`, and return it; `None` when `output` reports no limit.
///
/// # Errors
/// Reports a failed read or write of the book.
pub fn record_detected(
    dir: &Path,
    provider: Provider,
    output: &str,
    now: u64,
) -> Result<Option<Refusal>, String> {
    let Some(refusal) = detect(provider, output, now) else {
        return Ok(None);
    };
    record(dir, refusal.clone())?;
    Ok(Some(refusal))
}

/// The refusal an ACP agent's error carries for `provider`: one the agent
/// marked retryable or whose code or words say a limit
/// ([`acp_client::wire::RpcError::limited`]), with the reset its message
/// states, else a rate limit with the unknown-reset hold. `None` for any
/// other error.
#[must_use]
pub fn acp_refusal(
    provider: Provider,
    error: &acp_client::wire::RpcError,
    now: u64,
) -> Option<Refusal> {
    if !(error.limited() || error.retryable()) {
        return None;
    }
    Some(
        detect(provider, &error.message, now)
            .unwrap_or_else(|| Refusal::new(provider, Kind::RateLimit, now, None)),
    )
}

/// Minutes in a limit window a stream names, such as `five_hour`.
fn window_minutes(window: &str) -> Option<u64> {
    match window {
        "five_hour" => Some(5 * 60),
        "seven_day" | "seven_day_opus" | "seven_day_sonnet" => Some(7 * 24 * 60),
        _ => None,
    }
}

/// A reset time as a person or a script writes it, read against `now`:
/// Unix seconds (`1790164200`), an ISO 8601 UTC time
/// (`2026-10-06T15:00:00Z`), a duration from now (`90m`, `5h`, `2d`, or
/// `45s`), or a clock time the way a CLI prints it (`3pm`, `resets 11:50am
/// (UTC)`, `Sep 24, 5pm`), read as UTC. `None` when it is none of these.
#[must_use]
pub fn parse_reset(text: &str, now: u64) -> Option<u64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    if text.bytes().all(|b| b.is_ascii_digit()) {
        return text.parse().ok();
    }
    if let Some(at) = limit::parse_iso(text) {
        return Some(at);
    }
    let lower = text.to_ascii_lowercase();
    let unit = lower.chars().last()?;
    let number = &lower[..lower.len() - 1];
    if !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()) {
        let amount: u64 = number.parse().ok()?;
        let scale = match unit {
            's' => Some(1),
            'm' => Some(60),
            'h' => Some(3_600),
            'd' => Some(86_400),
            _ => None,
        };
        if let Some(scale) = scale {
            return Some(now.saturating_add(amount.saturating_mul(scale)));
        }
    }
    let phrased = if lower.contains("resets ") || lower.contains("try again at ") {
        text.to_owned()
    } else {
        format!("resets {text}")
    };
    limit::reset_from_message(&phrased, now)
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-09-23T11:30:58Z, when the retained sessions were throttled.
    const NOW: u64 = 1_790_163_058;
    // 2026-09-23T11:50:00Z.
    const RESET: u64 = 1_790_164_200;

    fn none(_: Provider) -> Option<String> {
        None
    }

    /// Claude Code's recorded limit messages, in each form it has printed,
    /// become a book entry with the reset each one states.
    #[test]
    fn claude_codes_recorded_limits_become_book_entries() {
        let dir = tempfile::tempdir().unwrap();
        for (said, reset) in [
            ("Claude AI usage limit reached|1790164200", Some(RESET)),
            (
                "You've hit your session limit · resets 11:50am (UTC)",
                Some(RESET),
            ),
            (
                "You've hit your limit · resets 3pm",
                limit::parse_iso("2026-09-23T15:00:00Z"),
            ),
            // A zone this crate can't read: the unknown-reset hold.
            (
                "You've hit your limit · resets 3pm (America/Los_Angeles)",
                None,
            ),
        ] {
            let refusal = detect(Provider::Claude, &format!("Error\n{said}\n"), NOW)
                .unwrap_or_else(|| panic!("no limit in {said}"));
            assert_eq!(refusal.kind, Kind::UsageLimit, "{said}");
            assert_eq!(refusal.resets_at, reset, "{said}");
            record_with(dir.path(), refusal, none).unwrap();
            let book = Book::load_with(dir.path(), none);
            let held = book.blocking(Provider::Claude, NOW).unwrap();
            assert_eq!(
                held.until,
                reset.unwrap_or(NOW + UNKNOWN_RESET_HOLD),
                "{said}"
            );
            assert!(book.has_capacity(Provider::Codex, NOW));
            assert!(book.has_capacity(Provider::Claude, held.until));
        }
    }

    /// Codex's recorded limits: the typed `usage_limit_reached` body with
    /// its reset, and the CLI's sentence with "try again at".
    #[test]
    fn codexs_recorded_limits_become_book_entries() {
        let dir = tempfile::tempdir().unwrap();
        let body = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1790164200,"resets_in_seconds":1142}}"#;
        let typed = detect(Provider::Codex, body, NOW).unwrap();
        assert_eq!(typed.kind, Kind::UsageLimit);
        assert_eq!(typed.resets_at, Some(RESET));
        assert_eq!(typed.plan.as_deref(), Some("pro"));
        let said = "ERROR: You've hit your usage limit. Upgrade to Pro (https://chatgpt.com/explore/pro), visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 3:04 PM.";
        let sentence = detect(Provider::Codex, said, NOW).unwrap();
        assert_eq!(sentence.resets_at, limit::parse_iso("2026-09-23T15:04:00Z"));
        record_with(dir.path(), sentence, none).unwrap();
        let book = Book::load_with(dir.path(), none);
        assert_eq!(
            book.blocking(Provider::Codex, NOW).unwrap().until,
            limit::parse_iso("2026-09-23T15:04:00Z").unwrap()
        );
    }

    /// An ACP agent's refusal message is read the same way, and a bare
    /// 429 is a rate limit, not a usage limit.
    #[test]
    fn an_acp_agents_rate_limit_is_a_rate_limit() {
        let refusal = detect(
            Provider::Grok,
            "exceeded retry limit, last status: 429 Too Many Requests",
            NOW,
        )
        .unwrap();
        assert_eq!(refusal.kind, Kind::RateLimit);
        assert_eq!(refusal.until, NOW + UNKNOWN_RESET_HOLD);
        assert_eq!(detect(Provider::Devin, "the turn failed", NOW), None);
        assert_eq!(detect(Provider::Codex, "", NOW), None);
    }

    #[test]
    fn an_acp_error_says_its_limit_and_reset() {
        use acp_client::wire::RpcError;
        let said = RpcError::new(
            -32603,
            "You've hit your session limit · resets 11:50am (UTC)",
        );
        let refusal = acp_refusal(Provider::Devin, &said, NOW).unwrap();
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.until, RESET);
        let mut retryable = RpcError::new(-32603, "busy");
        retryable.data = Some(serde_json::json!({"retryable": true}));
        let refusal = acp_refusal(Provider::Grok, &retryable, NOW).unwrap();
        assert_eq!(refusal.kind, Kind::RateLimit);
        assert_eq!(refusal.until, NOW + UNKNOWN_RESET_HOLD);
        assert_eq!(
            acp_refusal(Provider::Grok, &RpcError::new(-32603, "no such file"), NOW),
            None
        );
    }

    #[test]
    fn a_window_names_its_minutes() {
        let mut limit = Limit::from_message("anthropic", "rate_limit_error", NOW);
        limit.window = Some("seven_day".into());
        let refusal = from_limit(Provider::Claude, &limit, NOW);
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.window_minutes, Some(7 * 24 * 60));
    }

    #[test]
    fn reads_reset_times_as_people_write_them() {
        assert_eq!(parse_reset("1790164200", NOW), Some(RESET));
        assert_eq!(parse_reset("2026-09-23T11:50:00Z", NOW), Some(RESET));
        assert_eq!(parse_reset("90m", NOW), Some(NOW + 5_400));
        assert_eq!(parse_reset("5h", NOW), Some(NOW + 18_000));
        assert_eq!(parse_reset("2d", NOW), Some(NOW + 172_800));
        assert_eq!(parse_reset("11:50am", NOW), Some(RESET));
        assert_eq!(parse_reset("resets 11:50am (UTC)", NOW), Some(RESET));
        assert_eq!(parse_reset("soon", NOW), None);
        assert_eq!(parse_reset("", NOW), None);
    }
}
