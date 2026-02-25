//! Spacetime topic to Spacetime stream mapping and cursor continuity helpers.

/// Legacy topic cursor watermark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopicCursor {
    pub topic: String,
    pub after_seq: u64,
}

/// Spacetime stream cursor watermark.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamCursor {
    pub stream_id: String,
    pub after_seq: u64,
}

/// Stream window used for stale-cursor continuity checks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamWindow {
    pub stream_id: String,
    pub oldest_seq: u64,
    pub head_seq: u64,
    pub replay_budget_events: u64,
}

/// Cursor continuity result for resume bootstrap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CursorContinuity {
    Resume(StreamCursor),
    Rebootstrap {
        stream_id: String,
        requested_after_seq: u64,
        oldest_available_cursor: u64,
        head_cursor: u64,
        reason_codes: Vec<String>,
        replay_lag: u64,
        replay_budget_events: u64,
    },
}

/// Converts legacy runtime topic semantics into canonical Spacetime stream IDs.
#[must_use]
pub fn topic_to_stream_id(topic: &str) -> String {
    if topic == "runtime.codex_worker_events" {
        return "runtime.codex.worker.events".to_string();
    }
    if let Some(rest) = topic.strip_prefix("run:")
        && let Some(run_id) = rest.strip_suffix(":events")
    {
        return format!("runtime.run.{run_id}.events");
    }
    if let Some(rest) = topic.strip_prefix("worker:")
        && let Some(worker_id) = rest.strip_suffix(":lifecycle")
    {
        return format!("runtime.worker.{worker_id}.lifecycle");
    }
    if let Some(rest) = topic.strip_prefix("fleet:user:")
        && let Some(user_id) = rest.strip_suffix(":workers")
    {
        return format!("runtime.fleet.user.{user_id}.workers");
    }
    if let Some(rest) = topic.strip_prefix("fleet:guest:")
        && let Some(guest_id) = rest.strip_suffix(":workers")
    {
        return format!("runtime.fleet.guest.{guest_id}.workers");
    }
    format!("runtime.topic.{}", topic.replace(':', "."))
}

/// Converts canonical Spacetime stream IDs back into runtime topic keys where possible.
#[must_use]
pub fn stream_id_to_topic(stream_id: &str) -> Option<String> {
    if stream_id == "runtime.codex.worker.events" {
        return Some("runtime.codex_worker_events".to_string());
    }
    if let Some(rest) = stream_id.strip_prefix("runtime.run.")
        && let Some(run_id) = rest.strip_suffix(".events")
        && !run_id.trim().is_empty()
    {
        return Some(format!("run:{run_id}:events"));
    }
    if let Some(rest) = stream_id.strip_prefix("runtime.worker.")
        && let Some(worker_id) = rest.strip_suffix(".lifecycle")
        && !worker_id.trim().is_empty()
    {
        return Some(format!("worker:{worker_id}:lifecycle"));
    }
    if let Some(rest) = stream_id.strip_prefix("runtime.fleet.user.")
        && let Some(user_id) = rest.strip_suffix(".workers")
        && !user_id.trim().is_empty()
    {
        return Some(format!("fleet:user:{user_id}:workers"));
    }
    if let Some(rest) = stream_id.strip_prefix("runtime.fleet.guest.")
        && let Some(guest_id) = rest.strip_suffix(".workers")
        && !guest_id.trim().is_empty()
    {
        return Some(format!("fleet:guest:{guest_id}:workers"));
    }
    if let Some(rest) = stream_id.strip_prefix("runtime.topic.")
        && !rest.trim().is_empty()
    {
        return Some(rest.replace('.', ":"));
    }
    None
}

/// Converts a legacy topic cursor into the equivalent stream cursor.
#[must_use]
pub fn topic_cursor_to_stream(cursor: TopicCursor) -> StreamCursor {
    StreamCursor {
        stream_id: topic_to_stream_id(cursor.topic.as_str()),
        after_seq: cursor.after_seq,
    }
}

/// Validates whether a stream cursor can resume against a current stream window.
#[must_use]
pub fn evaluate_cursor_continuity(
    cursor: StreamCursor,
    window: Option<StreamWindow>,
) -> CursorContinuity {
    let Some(window) = window else {
        return CursorContinuity::Resume(cursor);
    };

    let oldest_available_cursor = window.oldest_seq.saturating_sub(1);
    let replay_lag = window.head_seq.saturating_sub(cursor.after_seq);
    let replay_budget_events = window.replay_budget_events.max(1);
    let mut reason_codes = Vec::new();
    if cursor.after_seq < oldest_available_cursor {
        reason_codes.push("retention_floor_breach".to_string());
    }
    if cursor.after_seq < window.head_seq && replay_lag > replay_budget_events {
        reason_codes.push("replay_budget_exceeded".to_string());
    }

    if reason_codes.is_empty() {
        CursorContinuity::Resume(cursor)
    } else {
        CursorContinuity::Rebootstrap {
            stream_id: window.stream_id,
            requested_after_seq: cursor.after_seq,
            oldest_available_cursor,
            head_cursor: window.head_seq,
            reason_codes,
            replay_lag,
            replay_budget_events,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CursorContinuity, StreamCursor, StreamWindow, TopicCursor, evaluate_cursor_continuity,
        stream_id_to_topic, topic_cursor_to_stream, topic_to_stream_id,
    };

    #[test]
    fn topic_mapping_covers_retained_runtime_topics() {
        assert_eq!(
            topic_to_stream_id("run:abc123:events"),
            "runtime.run.abc123.events"
        );
        assert_eq!(
            topic_to_stream_id("worker:desktop:owner:lifecycle"),
            "runtime.worker.desktop:owner.lifecycle"
        );
        assert_eq!(
            topic_to_stream_id("fleet:user:42:workers"),
            "runtime.fleet.user.42.workers"
        );
        assert_eq!(
            topic_to_stream_id("fleet:guest:anon-1:workers"),
            "runtime.fleet.guest.anon-1.workers"
        );
        assert_eq!(
            topic_to_stream_id("runtime.codex_worker_events"),
            "runtime.codex.worker.events"
        );
    }

    #[test]
    fn stream_mapping_roundtrips_for_retained_topics() {
        let source = [
            "run:abc123:events",
            "worker:desktop:owner:lifecycle",
            "fleet:user:42:workers",
            "fleet:guest:anon-1:workers",
            "runtime.codex_worker_events",
        ];
        for topic in source {
            let stream_id = topic_to_stream_id(topic);
            let back = stream_id_to_topic(stream_id.as_str());
            assert_eq!(back.as_deref(), Some(topic));
        }
    }

    #[test]
    fn topic_cursor_migrates_to_stream_cursor_without_seq_change() {
        let stream_cursor = topic_cursor_to_stream(TopicCursor {
            topic: "run:r1:events".to_string(),
            after_seq: 88,
        });
        assert_eq!(
            stream_cursor,
            StreamCursor {
                stream_id: "runtime.run.r1.events".to_string(),
                after_seq: 88
            }
        );
    }

    #[test]
    fn continuity_rebootstraps_when_cursor_falls_below_retention_floor() {
        let result = evaluate_cursor_continuity(
            StreamCursor {
                stream_id: "runtime.run.r1.events".to_string(),
                after_seq: 2,
            },
            Some(StreamWindow {
                stream_id: "runtime.run.r1.events".to_string(),
                oldest_seq: 10,
                head_seq: 25,
                replay_budget_events: 200,
            }),
        );
        assert!(matches!(result, CursorContinuity::Rebootstrap { .. }));
    }

    #[test]
    fn continuity_rebootstraps_when_replay_budget_is_exceeded() {
        let result = evaluate_cursor_continuity(
            StreamCursor {
                stream_id: "runtime.run.r1.events".to_string(),
                after_seq: 5,
            },
            Some(StreamWindow {
                stream_id: "runtime.run.r1.events".to_string(),
                oldest_seq: 6,
                head_seq: 600,
                replay_budget_events: 100,
            }),
        );
        assert!(matches!(result, CursorContinuity::Rebootstrap { .. }));
    }

    #[test]
    fn continuity_resumes_when_cursor_is_inside_window() {
        let cursor = StreamCursor {
            stream_id: "runtime.run.r1.events".to_string(),
            after_seq: 21,
        };
        let result = evaluate_cursor_continuity(
            cursor.clone(),
            Some(StreamWindow {
                stream_id: "runtime.run.r1.events".to_string(),
                oldest_seq: 10,
                head_seq: 25,
                replay_budget_events: 100,
            }),
        );
        assert_eq!(result, CursorContinuity::Resume(cursor));
    }
}
