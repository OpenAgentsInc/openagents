//! Telemetry batch endpoint
//!
//! Receives batched telemetry events from the client and stores in D1.
//! Called via sendBeacon on page unload for reliable delivery.

use serde::{Deserialize, Serialize};
use wasm_bindgen::JsValue;
use worker::{Env, Error, Response, Result};

/// A single telemetry event from the client
#[derive(Debug, Deserialize)]
struct ClientEvent {
    event: serde_json::Value,
    timestamp_ms: f64,
    page_path: String,
}

/// Batch payload from the client
#[derive(Debug, Deserialize)]
struct TelemetryBatchRequest {
    session_id: String,
    events: Vec<ClientEvent>,
    user_agent: String,
    user_id: Option<String>,
}

/// Response for successful batch insert
#[derive(Serialize)]
struct BatchResponse {
    ok: bool,
    inserted: usize,
}

/// Handle POST /api/telemetry/batch
pub async fn handle_batch(env: Env, body: String) -> Result<Response> {
    let batch: TelemetryBatchRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    if batch.events.is_empty() {
        return Response::from_json(&BatchResponse { ok: true, inserted: 0 });
    }

    // Validate session_id format (32 hex chars)
    if batch.session_id.len() != 32 || !batch.session_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Response::error("Invalid session_id", 400);
    }

    let db = env.d1("DB")?;
    let mut statements = Vec::with_capacity(batch.events.len());

    for event in &batch.events {
        let event_type = event
            .event
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("unknown");

        let event_name = extract_event_name(&event.event, event_type);

        let payload =
            serde_json::to_string(&event.event).unwrap_or_else(|_| "{}".to_string());

        let stmt = db
            .prepare(
                "INSERT INTO telemetry_events
                 (session_id, event_type, event_name, page_path, payload, user_agent, user_id, timestamp_ms)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&[
                JsValue::from_str(&batch.session_id),
                JsValue::from_str(event_type),
                JsValue::from_str(&event_name),
                JsValue::from_str(&event.page_path),
                JsValue::from_str(&payload),
                JsValue::from_str(&batch.user_agent),
                batch
                    .user_id
                    .as_ref()
                    .map(|u| JsValue::from_str(u))
                    .unwrap_or(JsValue::NULL),
                JsValue::from_f64(event.timestamp_ms),
            ])?;

        statements.push(stmt);
    }

    let results = db.batch(statements).await?;

    let inserted = results.iter().filter(|r| r.success()).count();

    Response::from_json(&BatchResponse { ok: true, inserted })
}

/// Extract event name based on event type
fn extract_event_name(event: &serde_json::Value, event_type: &str) -> String {
    match event_type {
        "page_view" => event
            .get("path")
            .and_then(|p| p.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "interaction" => event
            .get("action")
            .and_then(|a| a.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "performance" => event
            .get("metric")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown")
            .to_string(),
        "error" => event
            .get("error_type")
            .and_then(|e| e.as_str())
            .unwrap_or("unknown")
            .to_string(),
        _ => "unknown".to_string(),
    }
}
