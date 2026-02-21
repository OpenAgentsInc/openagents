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
            assert!(false, "expected update batch payload");
            return;
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
        let decoded = decode_khala_payload(&frame).expect("payload should decode");
        let error = if let KhalaEventPayload::Error(error) = decoded {
            error
        } else {
            assert!(false, "expected error payload");
            return;
        };
        assert_eq!(error.code, "stale_cursor");
        assert!(error.full_resync_required);
        assert_eq!(error.stale_topics.len(), 1);
        assert_eq!(error.stale_topics[0].retention_floor, 20);
    }

    #[test]
    fn watermark_decision_detects_duplicate_and_out_of_order() {
        assert_eq!(
            apply_watermark(5, 6),
            WatermarkDecision::Advanced { next: 6 }
        );
        assert_eq!(apply_watermark(5, 5), WatermarkDecision::Duplicate);
        assert_eq!(
            apply_watermark(5, 3),
            WatermarkDecision::OutOfOrder {
                current: 5,
                incoming: 3
            }
        );
    }
}
