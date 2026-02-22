use serde::Serialize;
use serde_json::{Value, json};

use crate::khala_protocol::{
    KhalaEventPayload, PhoenixFrame, SyncErrorPayload, build_phoenix_frame, decode_khala_payload,
    parse_phoenix_frame,
};

const DEFAULT_CHANNEL_TOPIC: &str = "sync:v1";

#[derive(Debug, Clone, Serialize)]
pub struct SessionEvent {
    pub seq: Option<u64>,
    pub payload: Value,
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
    worker_events_topic: String,
    worker_id: String,
    resume_after: u64,
    next_ref: u64,
    join_ref: Option<String>,
    pending_join_ref: Option<String>,
    pending_subscribe_ref: Option<String>,
    live: bool,
    latest_watermark: u64,
}

impl IosKhalaSession {
    pub fn new(
        worker_id: impl Into<String>,
        worker_events_topic: impl Into<String>,
        resume_after: u64,
    ) -> Self {
        Self {
            channel_topic: DEFAULT_CHANNEL_TOPIC.to_string(),
            worker_events_topic: worker_events_topic.into(),
            worker_id: worker_id.into(),
            resume_after,
            next_ref: 1,
            join_ref: None,
            pending_join_ref: None,
            pending_subscribe_ref: None,
            live: false,
            latest_watermark: resume_after,
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
        self.latest_watermark
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
                        "topics": [self.worker_events_topic.clone()],
                        "resume_after": {
                            self.worker_events_topic.clone(): self.latest_watermark.max(self.resume_after),
                        },
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
                let previous = self.latest_watermark;
                let mut events = Vec::new();

                for update in batch.updates {
                    if update.topic != self.worker_events_topic {
                        continue;
                    }
                    self.latest_watermark = self.latest_watermark.max(update.watermark);

                    let Some(payload) = update.payload else {
                        continue;
                    };
                    let worker_id = payload
                        .get("workerId")
                        .and_then(Value::as_str)
                        .or_else(|| payload.get("worker_id").and_then(Value::as_str));
                    if worker_id != Some(self.worker_id.as_str()) {
                        continue;
                    }

                    let seq = payload
                        .get("seq")
                        .and_then(Value::as_u64)
                        .or(Some(update.watermark));
                    events.push(SessionEvent { seq, payload });
                }

                if events.is_empty() && self.latest_watermark <= previous {
                    return SessionStep::Ignore;
                }

                SessionStep::Events {
                    events,
                    watermark: self.latest_watermark,
                }
            }
            Some(KhalaEventPayload::Heartbeat(_)) | Some(KhalaEventPayload::Other) | None => {
                SessionStep::Ignore
            }
            Some(KhalaEventPayload::Error(sync_error)) => Self::from_sync_error(sync_error),
        }
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

#[cfg(test)]
mod tests {
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
    fn update_batch_emits_filtered_events_and_watermark() {
        let mut session = IosKhalaSession::new("worker-1", "runtime.codex_worker_events", 0);
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
                    }
                ]
            }
        ]);
        let step =
            session.handle_frame_raw(&serde_json::to_string(&frame).expect("serialize frame"));
        match step {
            SessionStep::Events { events, watermark } => {
                assert_eq!(watermark, 12);
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].seq, Some(11));
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
