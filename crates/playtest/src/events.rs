//! Named playtest steps the website records (`/playtest`, the tutorial, the
//! survey).
//!
//! The pages call [`record`] with the playtest session's id, a step name,
//! and a few small properties. Until the server installs a [`Sink`], each
//! call does nothing, so the pages and their tests need no storage. The
//! website's sink is its playtest research stream
//! (`openagents_web::research`), which applies the visitor's opt-out and
//! the stream's bounds before it keeps anything.

use std::sync::OnceLock;

/// Where recorded steps go.
pub trait Sink: Send + Sync {
    fn record(&self, session_id: &str, step: &str, props: &[(&str, &str)]);
}

static SINK: OnceLock<Box<dyn Sink>> = OnceLock::new();

/// Installs the process's sink. The first install wins; returns whether
/// this one did.
pub fn install(sink: Box<dyn Sink>) -> bool {
    SINK.set(sink).is_ok()
}

/// Records one step of a playtest session. Does nothing when no sink is
/// installed.
pub fn record(session_id: &str, step: &str, props: &[(&str, &str)]) {
    if let Some(sink) = SINK.get() {
        sink.record(session_id, step, props);
    }
}

/// Whether a sink is installed.
pub fn installed() -> bool {
    SINK.get().is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_without_a_sink_does_nothing() {
        record("s", "playtest_started", &[("from", "home")]);
    }
}
