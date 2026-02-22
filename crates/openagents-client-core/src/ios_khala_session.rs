use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::{json, Value};

use crate::khala_protocol::{
    build_phoenix_frame, decode_khala_payload, parse_phoenix_frame, KhalaEventPayload,
    PhoenixFrame, SyncErrorPayload,
};

const DEFAULT_CHANNEL_TOPIC: &str = "sync:v1";
const DEFAULT_WORKER_EVENTS_TOPIC: &str = "runtime.codex_worker_events";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SessionEvent {
    pub topic: String,
    pub seq: Option<u64>,
    pub payload: Value,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SessionTopicWatermark {
    pub topic: String,
    pub watermark: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SessionStep {
    Ignore,
    Outbound {
        frame: String,
    },
    Live,
    Events {
        events: Vec<SessionEvent>,
        watermark: u64,
        topic_watermarks: Vec<SessionTopicWatermark>,
    },
    Error {
        code: String,
        message: String,
        status: Option<u16>,
        stale_cursor: bool,
        unauthorized: bool,
        forbidden: bool,
    },
}

#[derive(Debug, Clone)]
pub struct IosKhalaSession {
    channel_topic: String,
    subscribed_topics: Vec<String>,
    subscribed_topic_set: HashSet<String>,
    topic_watermarks: HashMap<String, u64>,
    next_ref: u64,
    join_ref: Option<String>,
    pending_join_ref: Option<String>,
    pending_subscribe_ref: Option<String>,
    live: bool,
}

impl IosKhalaSession {
    pub fn new(
        _worker_id: impl Into<String>,
        worker_events_topic: impl Into<String>,
        resume_after: u64,
    ) -> Self {
        let worker_events_topic = normalize_topic(worker_events_topic.into())
            .unwrap_or_else(|| DEFAULT_WORKER_EVENTS_TOPIC.to_string());
        let mut resume_after_map = HashMap::new();
        resume_after_map.insert(worker_events_topic.clone(), resume_after);
        Self::new_multi(vec![worker_events_topic], resume_after_map)
    }

    pub fn new_multi(topics: Vec<String>, resume_after_by_topic: HashMap<String, u64>) -> Self {
        let mut subscribed_topics = normalize_topics(topics);
        if subscribed_topics.is_empty() {
            subscribed_topics.push(DEFAULT_WORKER_EVENTS_TOPIC.to_string());
        }

        let mut subscribed_topic_set = HashSet::new();
        let mut topic_watermarks = HashMap::new();
        for topic in &subscribed_topics {
            subscribed_topic_set.insert(topic.clone());
            topic_watermarks.insert(
                topic.clone(),
                resume_after_by_topic.get(topic).copied().unwrap_or(0),
            );
        }

        Self {
            channel_topic: DEFAULT_CHANNEL_TOPIC.to_string(),
            subscribed_topics,
            subscribed_topic_set,
            topic_watermarks,
            next_ref: 1,
            join_ref: None,
            pending_join_ref: None,
            pending_subscribe_ref: None,
            live: false,
        }
    }

    pub fn start(&mut self) -> SessionStep {
        let join_ref = self.next_ref_string();
        self.pending_join_ref = Some(join_ref.clone());
        let frame = build_phoenix_frame(
            None,
            Some(join_ref.as_str()),
            self.channel_topic.as_str(),
            "phx_join",
            json!({}),
        );
        SessionStep::Outbound { frame }
    }

    pub fn heartbeat_frame(&mut self) -> Option<String> {
        if !self.live {
            return None;
        }

        let join_ref = self.join_ref.clone()?;
        let reference = self.next_ref_string();
        Some(build_phoenix_frame(
            Some(join_ref.as_str()),
            Some(reference.as_str()),
            self.channel_topic.as_str(),
            "sync:heartbeat",
            json!({}),
        ))
    }

    pub fn latest_watermark(&self) -> u64 {
        self.topic_watermarks.values().copied().max().unwrap_or(0)
    }

    pub fn handle_frame_raw(&mut self, raw_frame: &str) -> SessionStep {
        let Some(frame) = parse_phoenix_frame(raw_frame) else {
            return SessionStep::Ignore;
        };

        if frame.topic != self.channel_topic {
            return SessionStep::Ignore;
        }

        match frame.event.as_str() {
            "phx_reply" => self.handle_reply(frame),
            "sync:update_batch" | "sync:frame" => self.handle_update(frame),
            "sync:error" => self.handle_sync_error(frame.payload),
            "phx_error" => SessionStep::Error {
                code: "khala_channel_error".to_string(),
                message: "khala channel error".to_string(),
                status: None,
                stale_cursor: false,
                unauthorized: false,
                forbidden: false,
            },
            _ => SessionStep::Ignore,
        }
    }

    fn handle_reply(&mut self, frame: PhoenixFrame) -> SessionStep {
        let payload = match frame.payload.as_object() {
            Some(payload) => payload,
            None => {
                return SessionStep::Error {
                    code: "khala_invalid_reply".to_string(),
                    message: "khala invalid reply".to_string(),
                    status: None,
                    stale_cursor: false,
                    unauthorized: false,
                    forbidden: false,
                };
            }
        };

        let status = payload
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("error");
        let response = payload
            .get("response")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        if status == "ok" {
            if frame.reference == self.pending_join_ref {
                self.pending_join_ref = None;
                self.join_ref = frame.reference.clone();

                let subscribe_ref = self.next_ref_string();
                self.pending_subscribe_ref = Some(subscribe_ref.clone());
                let frame = build_phoenix_frame(
                    self.join_ref.as_deref(),
                    Some(subscribe_ref.as_str()),
                    self.channel_topic.as_str(),
                    "sync:subscribe",
                    json!({
                        "topics": self.subscribed_topics,
                        "resume_after": self.topic_watermarks,
                        "replay_batch_size": 200,
                    }),
                );
                return SessionStep::Outbound { frame };
            }

            if frame.reference == self.pending_subscribe_ref {
                self.pending_subscribe_ref = None;
                self.live = true;
                return SessionStep::Live;
            }

            return SessionStep::Ignore;
        }

        let code = response
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or("sync_error")
            .to_string();
        let message = response
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("khala request failed")
            .to_string();

        Self::error_step(code, message)
    }

    fn handle_update(&mut self, frame: PhoenixFrame) -> SessionStep {
        match decode_khala_payload(&frame) {
            Some(KhalaEventPayload::UpdateBatch(batch)) => {
                let previous_topic_watermarks = self.topic_watermarks.clone();
                let mut events = Vec::new();

                for update in batch.updates {
                    if !self.subscribed_topic_set.contains(update.topic.as_str()) {
                        continue;
                    }

                    self.topic_watermarks
                        .entry(update.topic.clone())
                        .and_modify(|watermark| *watermark = (*watermark).max(update.watermark))
                        .or_insert(update.watermark);

                    let Some(payload) = update.payload else {
                        continue;
                    };

                    let seq = payload
                        .get("seq")
                        .and_then(Value::as_u64)
                        .or(Some(update.watermark));
                    events.push(SessionEvent {
                        topic: update.topic,
                        seq,
                        payload,
                    });
                }

                if events.is_empty() && self.topic_watermarks == previous_topic_watermarks {
                    return SessionStep::Ignore;
                }

                SessionStep::Events {
                    events,
                    watermark: self.latest_watermark(),
                    topic_watermarks: self.topic_watermarks_snapshot(),
                }
            }
            Some(KhalaEventPayload::Heartbeat(_)) | Some(KhalaEventPayload::Other) | None => {
                SessionStep::Ignore
            }
            Some(KhalaEventPayload::Error(sync_error)) => Self::from_sync_error(sync_error),
        }
    }

    fn topic_watermarks_snapshot(&self) -> Vec<SessionTopicWatermark> {
        let mut watermarks: Vec<SessionTopicWatermark> = self
            .topic_watermarks
            .iter()
            .map(|(topic, watermark)| SessionTopicWatermark {
                topic: topic.clone(),
                watermark: *watermark,
            })
            .collect();
        watermarks.sort_by(|lhs, rhs| lhs.topic.cmp(&rhs.topic));
        watermarks
    }

    fn handle_sync_error(&mut self, payload: Value) -> SessionStep {
        match payload.as_object() {
            Some(object) => {
                let code = object
                    .get("code")
                    .and_then(Value::as_str)
                    .unwrap_or("sync_error")
                    .to_string();
                let message = object
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("khala sync error")
                    .to_string();
                Self::error_step(code, message)
            }
            None => SessionStep::Error {
                code: "sync_error".to_string(),
                message: "khala sync error".to_string(),
                status: None,
                stale_cursor: false,
                unauthorized: false,
                forbidden: false,
            },
        }
    }

    fn from_sync_error(error: SyncErrorPayload) -> SessionStep {
        Self::error_step(error.code, error.message)
    }

    fn error_step(code: String, message: String) -> SessionStep {
        match code.as_str() {
            "unauthorized" => SessionStep::Error {
                code,
                message,
                status: Some(401),
                stale_cursor: false,
                unauthorized: true,
                forbidden: false,
            },
            "forbidden_topic" => SessionStep::Error {
                code,
                message,
                status: Some(403),
                stale_cursor: false,
                unauthorized: false,
                forbidden: true,
            },
            "stale_cursor" => SessionStep::Error {
                code,
                message,
                status: Some(409),
                stale_cursor: true,
                unauthorized: false,
                forbidden: false,
            },
            _ => SessionStep::Error {
                code,
                message,
                status: None,
                stale_cursor: false,
                unauthorized: false,
                forbidden: false,
            },
        }
    }

    fn next_ref_string(&mut self) -> String {
        let value = self.next_ref;
        self.next_ref = self.next_ref.saturating_add(1);
        value.to_string()
    }
}

fn normalize_topic(raw: String) -> Option<String> {
    let normalized = raw.trim();
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.to_string())
    }
}

fn normalize_topics(topics: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    topics
        .into_iter()
        .filter_map(normalize_topic)
        .filter(|topic| seen.insert(topic.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use serde_json::json;

    use super::{IosKhalaSession, SessionStep};

    #[test]
    fn session_starts_with_join_and_subscribe_on_reply() {
        let mut session = IosKhalaSession::new("worker-1", "runtime.codex_worker_events", 8);
        let start = session.start();
        let SessionStep::Outbound { frame } = start else {
            panic!("expected outbound join frame");
        };
        assert!(frame.contains("\"phx_join\""));

        let join_reply = r#"[null,"1","sync:v1","phx_reply",{"status":"ok","response":{}}]"#;
        let subscribe = session.handle_frame_raw(join_reply);
        let SessionStep::Outbound { frame } = subscribe else {
            panic!("expected subscribe frame");
        };
        assert!(frame.contains("\"sync:subscribe\""));
        assert!(frame.contains("\"runtime.codex_worker_events\""));
        assert!(frame.contains("\"resume_after\""));
    }

    #[test]
    fn session_subscribe_payload_supports_multi_topic_resume_map() {
        let mut resume_after = HashMap::new();
        resume_after.insert("runtime.codex_worker_events".to_string(), 19);
        resume_after.insert("runtime.codex_worker_summaries".to_string(), 7);
        let mut session = IosKhalaSession::new_multi(
            vec![
                "runtime.codex_worker_events".to_string(),
                "runtime.codex_worker_summaries".to_string(),
                "runtime.codex_worker_events".to_string(),
            ],
            resume_after,
        );

        let _ = session.start();
        let subscribe = session
            .handle_frame_raw(r#"[null,"1","sync:v1","phx_reply",{"status":"ok","response":{}}]"#);
        let SessionStep::Outbound { frame } = subscribe else {
            panic!("expected subscribe frame");
        };
        assert!(frame.contains("\"runtime.codex_worker_events\""));
        assert!(frame.contains("\"runtime.codex_worker_summaries\""));
        assert!(frame.contains("\"runtime.codex_worker_events\":19"));
        assert!(frame.contains("\"runtime.codex_worker_summaries\":7"));
    }

    #[test]
    fn subscribe_reply_transitions_live() {
        let mut session = IosKhalaSession::new("worker-1", "runtime.codex_worker_events", 0);
        let _ = session.start();
        let _ = session
            .handle_frame_raw(r#"[null,"1","sync:v1","phx_reply",{"status":"ok","response":{}}]"#);
        let live = session
            .handle_frame_raw(r#"["1","2","sync:v1","phx_reply",{"status":"ok","response":{}}]"#);
        assert!(matches!(live, SessionStep::Live));
    }

    #[test]
    fn update_batch_emits_multi_topic_events_without_worker_filtering() {
        let mut session = IosKhalaSession::new_multi(
            vec![
                "runtime.codex_worker_events".to_string(),
                "runtime.codex_worker_summaries".to_string(),
            ],
            HashMap::new(),
        );
        let frame = json!([
            "1",
            "9",
            "sync:v1",
            "sync:update_batch",
            {
                "updates": [
                    {
                        "topic": "runtime.codex_worker_events",
                        "watermark": 11,
                        "payload": {
                            "workerId": "worker-1",
                            "seq": 11,
                            "method": "turn/started"
                        }
                    },
                    {
                        "topic": "runtime.codex_worker_events",
                        "watermark": 12,
                        "payload": {
                            "workerId": "worker-other",
                            "seq": 12,
                            "method": "turn/completed"
                        }
                    },
                    {
                        "topic": "runtime.codex_worker_summaries",
                        "watermark": 20,
                        "payload": {
                            "worker_id": "worker-other",
                            "status": "running",
                            "latest_seq": 12,
                            "adapter": "desktop_bridge"
                        }
                    },
                    {
                        "topic": "runtime.other_topic",
                        "watermark": 30,
                        "payload": {
                            "ignored": true
                        }
                    }
                ]
            }
        ]);
        let step =
            session.handle_frame_raw(&serde_json::to_string(&frame).expect("serialize frame"));
        match step {
            SessionStep::Events {
                events,
                watermark,
                topic_watermarks,
            } => {
                assert_eq!(watermark, 20);
                assert_eq!(events.len(), 3);
                assert_eq!(events[0].topic, "runtime.codex_worker_events");
                assert_eq!(events[1].topic, "runtime.codex_worker_events");
                assert_eq!(events[2].topic, "runtime.codex_worker_summaries");
                assert_eq!(events[1].seq, Some(12));
                assert_eq!(
                    topic_watermarks,
                    vec![
                        super::SessionTopicWatermark {
                            topic: "runtime.codex_worker_events".to_string(),
                            watermark: 12,
                        },
                        super::SessionTopicWatermark {
                            topic: "runtime.codex_worker_summaries".to_string(),
                            watermark: 20,
                        },
                    ]
                );
            }
            _ => panic!("expected events step"),
        }
    }

    #[test]
    fn stale_cursor_maps_conflict_error() {
        let mut session = IosKhalaSession::new("worker-1", "runtime.codex_worker_events", 0);
        let frame = r#"["1","2","sync:v1","sync:error",{"code":"stale_cursor","message":"cursor expired"}]"#;
        let step = session.handle_frame_raw(frame);
        match step {
            SessionStep::Error {
                code,
                status,
                stale_cursor,
                ..
            } => {
                assert_eq!(code, "stale_cursor");
                assert_eq!(status, Some(409));
                assert!(stale_cursor);
            }
            _ => panic!("expected error step"),
        }
    }
}
