use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub struct PhoenixFrame {
    pub join_ref: Option<String>,
    pub reference: Option<String>,
    pub topic: String,
    pub event: String,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TopicWatermark {
    pub topic: String,
    pub watermark: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct KhalaUpdate {
    pub topic: String,
    pub watermark: u64,
    pub payload: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UpdateBatchPayload {
    pub updates: Vec<KhalaUpdate>,
    pub replay_complete: bool,
    pub head_watermarks: Vec<TopicWatermark>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaleTopic {
    pub topic: String,
    pub resume_after: u64,
    pub retention_floor: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyncErrorPayload {
    pub code: String,
    pub message: String,
    pub full_resync_required: bool,
    pub stale_topics: Vec<StaleTopic>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum KhalaEventPayload {
    UpdateBatch(UpdateBatchPayload),
    Heartbeat(Vec<TopicWatermark>),
    Error(SyncErrorPayload),
    Other,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeStreamEvent {
    pub id: Option<u64>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WatermarkDecision {
    Advanced { next: u64 },
    Duplicate,
    OutOfOrder { current: u64, incoming: u64 },
}

#[must_use]
pub fn build_phoenix_frame(
    join_ref: Option<&str>,
    reference: Option<&str>,
    topic: &str,
    event: &str,
    payload: Value,
) -> String {
    let frame = Value::Array(vec![
        join_ref.map_or(Value::Null, |value| Value::String(value.to_string())),
        reference.map_or(Value::Null, |value| Value::String(value.to_string())),
        Value::String(topic.to_string()),
        Value::String(event.to_string()),
        payload,
    ]);

    serde_json::to_string(&frame).unwrap_or_else(|_| "[]".to_string())
}

pub fn parse_phoenix_frame(raw: &str) -> Option<PhoenixFrame> {
    let parsed: Value = serde_json::from_str(raw).ok()?;
    let frame = parsed.as_array()?;
    if frame.len() != 5 {
        return None;
    }

    let join_ref = frame[0].as_str().map(ToString::to_string);
    let reference = frame[1].as_str().map(ToString::to_string);
    let topic = frame[2].as_str()?.to_string();
    let event = frame[3].as_str()?.to_string();
    let payload = frame[4].clone();

    Some(PhoenixFrame {
        join_ref,
        reference,
        topic,
        event,
        payload,
    })
}

pub fn decode_khala_payload(frame: &PhoenixFrame) -> Option<KhalaEventPayload> {
    match frame.event.as_str() {
        "sync:update_batch" => {
            parse_update_batch(&frame.payload).map(KhalaEventPayload::UpdateBatch)
        }
        "sync:heartbeat" => Some(KhalaEventPayload::Heartbeat(parse_watermarks(
            &frame.payload,
        ))),
        "sync:error" => parse_sync_error(&frame.payload).map(KhalaEventPayload::Error),
        "sync:frame" => decode_sync_frame_payload(&frame.payload),
        _ => Some(KhalaEventPayload::Other),
    }
}

#[must_use]
pub fn apply_watermark(current: u64, incoming: u64) -> WatermarkDecision {
    if incoming > current {
        WatermarkDecision::Advanced { next: incoming }
    } else if incoming == current {
        WatermarkDecision::Duplicate
    } else {
        WatermarkDecision::OutOfOrder { current, incoming }
    }
}

pub fn sync_error_code(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|object| object.get("code"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

pub fn extract_runtime_stream_events(
    payload: &Value,
    expected_topic: &str,
    worker_id: &str,
) -> Vec<RuntimeStreamEvent> {
    let updates = payload
        .as_object()
        .and_then(|object| object.get("updates"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    updates
        .into_iter()
        .filter_map(|update| {
            let update_object = update.as_object()?;
            let topic = update_object.get("topic")?.as_str()?;
            if topic != expected_topic {
                return None;
            }

            let stream_payload = update_object.get("payload")?.as_object()?;
            let event_worker_id = stream_payload
                .get("workerId")
                .and_then(Value::as_str)
                .or_else(|| stream_payload.get("worker_id").and_then(Value::as_str))?;
            if event_worker_id != worker_id {
                return None;
            }

            let payload = Value::Object(stream_payload.clone());
            let seq = stream_payload
                .get("seq")
                .and_then(Value::as_u64)
                .or_else(|| update_object.get("watermark").and_then(Value::as_u64));

            Some(RuntimeStreamEvent { id: seq, payload })
        })
        .collect()
}

pub fn runtime_stream_event_seq(event: &RuntimeStreamEvent) -> Option<u64> {
    event
        .id
        .or_else(|| event.payload.get("seq").and_then(Value::as_u64))
}

#[must_use]
pub fn merge_retry_cursor(current: Option<u64>, failed_seq: u64) -> u64 {
    let replay_cursor = failed_seq.saturating_sub(1);
    current
        .map(|cursor| cursor.min(replay_cursor))
        .unwrap_or(replay_cursor)
}

fn decode_sync_frame_payload(payload: &Value) -> Option<KhalaEventPayload> {
    let object = payload.as_object()?;
    let kind = object.get("kind")?.as_str()?;
    let payload_bytes = object.get("payload_bytes")?.as_str()?;
    let decoded_bytes = STANDARD.decode(payload_bytes).ok()?;
    let decoded_json: Value = serde_json::from_slice(&decoded_bytes).ok()?;

    match kind {
        "KHALA_FRAME_KIND_UPDATE_BATCH" => {
            parse_update_batch(&decoded_json).map(KhalaEventPayload::UpdateBatch)
        }
        "KHALA_FRAME_KIND_HEARTBEAT" => Some(KhalaEventPayload::Heartbeat(parse_watermarks(
            &decoded_json,
        ))),
        "KHALA_FRAME_KIND_ERROR" => parse_sync_error(&decoded_json).map(KhalaEventPayload::Error),
        _ => Some(KhalaEventPayload::Other),
    }
}

fn parse_update_batch(payload: &Value) -> Option<UpdateBatchPayload> {
    let object = payload.as_object()?;
    let replay_complete = object
        .get("replay_complete")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let updates = object
        .get("updates")
        .and_then(Value::as_array)
        .map(|updates| {
            updates
                .iter()
                .filter_map(|value| {
                    let item = value.as_object()?;
                    let topic = item.get("topic")?.as_str()?.to_string();
                    let watermark = item.get("watermark")?.as_u64()?;
                    let update_payload = item.get("payload").cloned();
                    Some(KhalaUpdate {
                        topic,
                        watermark,
                        payload: update_payload,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let head_watermarks = object
        .get("head_watermarks")
        .map(parse_watermarks)
        .unwrap_or_default();

    Some(UpdateBatchPayload {
        updates,
        replay_complete,
        head_watermarks,
    })
}

fn parse_sync_error(payload: &Value) -> Option<SyncErrorPayload> {
    let object = payload.as_object()?;
    let code = object
        .get("code")
        .and_then(Value::as_str)
        .unwrap_or("sync_error")
        .to_string();
    let message = object
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or("khala_sync_error")
        .to_string();
    let full_resync_required = object
        .get("full_resync_required")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let stale_topics = object
        .get("stale_topics")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| {
                    let item = entry.as_object()?;
                    let topic = item.get("topic")?.as_str()?.to_string();
                    let resume_after = item.get("resume_after")?.as_u64()?;
                    let retention_floor = item.get("retention_floor")?.as_u64()?;
                    Some(StaleTopic {
                        topic,
                        resume_after,
                        retention_floor,
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Some(SyncErrorPayload {
        code,
        message,
        full_resync_required,
        stale_topics,
    })
}

fn parse_watermarks(payload: &Value) -> Vec<TopicWatermark> {
    let entries = payload
        .as_object()
        .and_then(|object| object.get("watermarks"))
        .and_then(Value::as_array)
        .or_else(|| payload.as_array());

    entries
        .map(|watermarks| {
            watermarks
                .iter()
                .filter_map(|entry| {
                    let item = entry.as_object()?;
                    let topic = item.get("topic")?.as_str()?.to_string();
                    let watermark = item.get("watermark")?.as_u64()?;
                    Some(TopicWatermark { topic, watermark })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parse_sync_frame_update_batch_payload() {
        let payload = json!({
            "updates": [
                {
                    "topic": "runtime.codex_worker_events",
                    "watermark": 12,
                    "payload": { "seq": 12, "method": "turn/started" }
                }
            ],
            "replay_complete": true,
            "head_watermarks": [
                { "topic": "runtime.codex_worker_events", "watermark": 12 }
            ]
        });
        let encoded_payload =
            STANDARD.encode(serde_json::to_vec(&payload).expect("payload should serialize"));
        let frame = PhoenixFrame {
            join_ref: Some("1".to_string()),
            reference: Some("2".to_string()),
            topic: "sync:v1".to_string(),
            event: "sync:frame".to_string(),
            payload: json!({
                "topic": "runtime.codex_worker_events",
                "seq": 12,
                "kind": "KHALA_FRAME_KIND_UPDATE_BATCH",
                "payload_bytes": encoded_payload,
                "schema_version": 1
            }),
        };

        let decoded = decode_khala_payload(&frame).expect("frame should decode");
        let batch = if let KhalaEventPayload::UpdateBatch(batch) = decoded {
            batch
        } else {
            panic!("expected update batch payload");
        };
        assert_eq!(batch.updates.len(), 1);
        assert_eq!(batch.updates[0].watermark, 12);
        assert!(batch.replay_complete);
    }

    #[test]
    fn parse_stale_cursor_error_payload() {
        let raw = build_phoenix_frame(
            Some("1"),
            Some("9"),
            "sync:v1",
            "sync:error",
            json!({
                "code": "stale_cursor",
                "message": "cursor is older than retention floor",
                "full_resync_required": true,
                "stale_topics": [
                    {
                        "topic": "runtime.codex_worker_events",
                        "resume_after": 11,
                        "retention_floor": 20
                    }
                ]
            }),
        );

        let frame = parse_phoenix_frame(&raw).expect("frame should parse");
        let payload = decode_khala_payload(&frame).expect("payload should decode");

        let error = if let KhalaEventPayload::Error(error) = payload {
            error
        } else {
            panic!("expected error payload");
        };

        assert_eq!(error.code, "stale_cursor");
        assert!(error.full_resync_required);
        assert_eq!(error.stale_topics.len(), 1);
        assert_eq!(error.stale_topics[0].resume_after, 11);
    }

    #[test]
    fn apply_watermark_advances_only_on_increase() {
        assert_eq!(
            apply_watermark(4, 9),
            WatermarkDecision::Advanced { next: 9 }
        );
        assert_eq!(apply_watermark(9, 9), WatermarkDecision::Duplicate);
        assert_eq!(
            apply_watermark(9, 7),
            WatermarkDecision::OutOfOrder {
                current: 9,
                incoming: 7,
            }
        );
    }

    #[test]
    fn extract_runtime_stream_events_filters_by_topic_and_worker() {
        let payload = json!({
            "updates": [
                {
                    "topic": "runtime.codex_worker_events",
                    "watermark": 12,
                    "payload": {
                        "workerId": "desktopw:shared",
                        "seq": 12,
                        "eventType": "worker.event"
                    }
                },
                {
                    "topic": "runtime.codex_worker_events",
                    "watermark": 13,
                    "payload": {
                        "workerId": "desktopw:other",
                        "seq": 13
                    }
                }
            ]
        });

        let events = extract_runtime_stream_events(
            &payload,
            "runtime.codex_worker_events",
            "desktopw:shared",
        );
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, Some(12));
        assert_eq!(runtime_stream_event_seq(&events[0]), Some(12));
    }

    #[test]
    fn merge_retry_cursor_prefers_oldest_replay_position() {
        assert_eq!(merge_retry_cursor(None, 50), 49);
        assert_eq!(merge_retry_cursor(Some(32), 50), 32);
        assert_eq!(merge_retry_cursor(Some(49), 12), 11);
        assert_eq!(merge_retry_cursor(Some(0), 0), 0);
    }
}
