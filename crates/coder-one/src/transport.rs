//! Codex sessions that can't reach their provider.
//!
//! When `codex exec` can't connect, it doesn't fail: it reports each
//! attempt as an `error` event, such as `Reconnecting... waiting for
//! network (Connection failed: error sending request)`, and keeps
//! retrying until something ends it. In a task image with no CA roots,
//! every TLS handshake fails with `UnknownIssuer`, and a session idled for
//! 3 to 4 hours on its dispatch deadline (issue #9581).
//!
//! [`Watch`] reads a Codex stream line by line and reports when the
//! session has said nothing but connection errors for its bound. The
//! adapter then ends the session and records it as a transport failure,
//! so it doesn't use up the round's deadline. Any other progress, such as
//! a command, a message, or a finished turn, resets the watch; a usage
//! limit is not a connection error, because `coder_one::limit` handles it.

use std::time::Duration;

use serde_json::Value;

/// How long a Codex session may report only connection errors before the
/// host ends it: long enough for a network blip to clear, short against a
/// dispatch deadline measured in tens of minutes.
pub const BOUND: Duration = Duration::from_secs(180);

/// Text that marks a Codex `error` event as a failed connection.
const CONNECTION: [&str; 6] = [
    "reconnecting",
    "stream disconnected",
    "connection failed",
    "error sending request",
    "invalid peer certificate",
    "falling back from websockets",
];

/// What one Codex event says about the connection.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Signal {
    /// A connection error, with its message.
    Failed(String),
    /// Work the session did: the connection is up.
    Progress,
    /// Nothing about the connection, such as `turn.started`.
    Neutral,
}

fn signal(event: &Value) -> Signal {
    let kind = event.get("type").and_then(Value::as_str);
    let message = match kind {
        Some("error") => event.get("message").and_then(Value::as_str),
        Some("item.started" | "item.updated" | "item.completed")
            if event.pointer("/item/type").and_then(Value::as_str) == Some("error") =>
        {
            event.pointer("/item/message").and_then(Value::as_str)
        }
        Some("thread.started" | "turn.started") | None => return Signal::Neutral,
        Some(_) => return Signal::Progress,
    };
    let Some(message) = message else {
        return Signal::Neutral;
    };
    let lower = message.to_lowercase();
    if !crate::limit::says_limited(message) && CONNECTION.iter().any(|text| lower.contains(text)) {
        Signal::Failed(message.to_string())
    } else {
        Signal::Neutral
    }
}

/// Watches one Codex process's stream for a run of connection errors.
#[derive(Debug, Clone)]
pub struct Watch {
    bound_ms: u64,
    /// When the current run of connection errors began.
    since_ms: Option<u64>,
    /// Connection errors in the current run.
    errors: u64,
    /// The run's latest message.
    last: Option<String>,
    /// Whether the session ever made progress.
    reached: bool,
}

impl Watch {
    /// A watch that fires after `bound` of connection errors alone.
    #[must_use]
    pub fn new(bound: Duration) -> Self {
        Watch {
            bound_ms: u64::try_from(bound.as_millis()).unwrap_or(u64::MAX),
            since_ms: None,
            errors: 0,
            last: None,
            reached: false,
        }
    }

    /// Reads one line of the stream, seen at `at_ms`.
    pub fn line(&mut self, line: &str, at_ms: u64) {
        let Ok(event) = serde_json::from_str::<Value>(line) else {
            return;
        };
        match signal(&event) {
            Signal::Failed(message) => {
                self.since_ms.get_or_insert(at_ms);
                self.errors += 1;
                self.last = Some(message);
            }
            Signal::Progress => {
                self.reached = true;
                self.since_ms = None;
                self.errors = 0;
                self.last = None;
            }
            Signal::Neutral => {}
        }
    }

    /// Whether the session ever made progress, so a provider may have
    /// billed it.
    #[must_use]
    pub fn reached(&self) -> bool {
        self.reached
    }

    /// Why the session should end at `now_ms`: set once connection errors
    /// alone have filled the bound.
    #[must_use]
    pub fn expired(&self, now_ms: u64) -> Option<String> {
        let since = self.since_ms?;
        let lasted = now_ms.saturating_sub(since);
        (lasted >= self.bound_ms).then(|| {
            format!(
                "Codex reported only connection errors for {} s ({} errors); the last: {}",
                lasted / 1000,
                self.errors,
                self.last.as_deref().unwrap_or("none")
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TLS: &str = r#"{"type":"error","message":"Reconnecting... 2/5 (stream disconnected before completion: invalid peer certificate: UnknownIssuer)"}"#;
    const FALLBACK: &str = r#"{"type":"item.completed","item":{"id":"item_0","type":"error","message":"Falling back from WebSockets to HTTPS transport. stream disconnected before completion: invalid peer certificate: UnknownIssuer"}}"#;
    const OFFLINE: &str = r#"{"type":"error","message":"Reconnecting... waiting for network (Connection failed: error sending request)"}"#;
    const COMMAND: &str = r#"{"type":"item.completed","item":{"id":"i1","type":"command_execution","command":"ls","aggregated_output":"","exit_code":0,"status":"completed"}}"#;

    /// The #9581 stream: TLS failures, the WebSocket fallback, then
    /// `waiting for network` every few seconds, with no progress.
    fn offline(watch: &mut Watch, from_ms: u64, to_ms: u64) {
        watch.line(r#"{"type":"thread.started","thread_id":"t"}"#, from_ms);
        watch.line(r#"{"type":"turn.started"}"#, from_ms);
        watch.line(TLS, from_ms);
        watch.line(FALLBACK, from_ms + 1_000);
        let mut at = from_ms + 2_000;
        while at <= to_ms {
            watch.line(OFFLINE, at);
            at += 5_000;
        }
    }

    #[test]
    fn a_stream_of_only_connection_errors_ends_after_the_bound() {
        let mut watch = Watch::new(Duration::from_secs(180));
        offline(&mut watch, 0, 170_000);
        assert_eq!(watch.expired(170_000), None);
        offline(&mut watch, 172_000, 200_000);
        let why = watch.expired(180_000).expect("the bound passed");
        assert!(why.contains("only connection errors for 180 s"), "{why}");
        assert!(why.contains("waiting for network"), "{why}");
        assert!(!watch.reached());
    }

    #[test]
    fn progress_resets_the_watch_and_marks_the_provider_reached() {
        let mut watch = Watch::new(Duration::from_secs(180));
        offline(&mut watch, 0, 100_000);
        watch.line(COMMAND, 110_000);
        assert_eq!(watch.expired(250_000), None);
        assert!(watch.reached());
        watch.line(OFFLINE, 260_000);
        assert_eq!(watch.expired(439_000), None);
        assert!(watch.expired(440_000).is_some());
    }

    #[test]
    fn a_usage_limit_and_other_errors_are_not_connection_errors() {
        let mut watch = Watch::new(Duration::from_secs(1));
        watch.line(
            r#"{"type":"error","message":"exceeded retry limit, last status: 429 Too Many Requests"}"#,
            0,
        );
        watch.line(
            r#"{"type":"error","message":"The 'gpt-5.2-codex' model is not supported when using Codex with a ChatGPT account."}"#,
            0,
        );
        watch.line("not json", 0);
        assert_eq!(watch.expired(10_000), None);
        assert!(!watch.reached());
    }
}
