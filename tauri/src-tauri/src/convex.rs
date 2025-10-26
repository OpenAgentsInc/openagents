//! Convex-facing commands and mapping helpers for the Tauri backend.
//!
//! This module isolates all Convex list/subscribe logic so it can be
//! unit-tested and refactored independently from window/bootstrap code.

use serde::Serialize;
use tauri::Emitter;

#[derive(Serialize)]
pub struct ThreadSummary {
    pub id: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub updated_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<i64>,
}

#[derive(Serialize)]
pub struct MessageRow {
    pub id: Option<String>,
    #[serde(rename = "threadId")]
    pub thread_id: Option<String>,
    pub role: Option<String>,
    pub kind: Option<String>,
    pub text: Option<String>,
    pub data: Option<serde_json::Value>,
    pub ts: f64,
}

// -- Mapping helpers (unit-testable) -----------------------------------------------------------

/// Prefer the Convex document id (`docId`) when present; otherwise fall back to `threadId`.
pub fn select_thread_key(thread_id: &str, doc_id: Option<&str>) -> String {
    doc_id.map(|s| s.to_string()).unwrap_or_else(|| thread_id.to_string())
}

/// Map a Convex thread JSON into a ThreadSummary. Tolerates integer or float counts.
pub fn map_thread_item(json: &serde_json::Value) -> Option<ThreadSummary> {
    let id = json.get("_id").and_then(|x| x.as_str())?.to_string();
    let title = json.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
    let updated_at = json.get("updatedAt").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let thread_id = json.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string());
    let count = json
        .get("messageCount")
        .and_then(|x| x.as_i64())
        .or_else(|| json.get("messageCount").and_then(|x| x.as_f64()).map(|f| f as i64));
    Some(ThreadSummary { id, thread_id, title, updated_at, count })
}

/// Returns true if a message JSON should be hidden from the UI (preface/system/meta).
pub fn should_hide_message(json: &serde_json::Value) -> bool {
    let text_s = json.get("text").and_then(|x| x.as_str()).unwrap_or("");
    let kind_s = json.get("kind").and_then(|x| x.as_str()).unwrap_or("");
    let role_s = json.get("role").and_then(|x| x.as_str()).unwrap_or("");
    let trimmed = text_s.trim_start();
    trimmed.starts_with("<user_instructions>")
        || trimmed.starts_with("<environment_context>")
        || matches!(kind_s, "preface" | "instructions" | "env" | "context")
        || (role_s == "system" && (text_s.contains("Repository Guidelines") || text_s.contains("<environment_context>")))
}

/// Convert a Convex message JSON into a MessageRow, applying hide rules.
pub fn map_message_row(json: &serde_json::Value) -> Option<MessageRow> {
    if should_hide_message(json) { return None; }
    let id = json.get("_id").and_then(|x| x.as_str()).map(|s| s.to_string());
    let thread_id = json.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string());
    let role = json.get("role").and_then(|x| x.as_str()).map(|s| s.to_string());
    let kind = json.get("kind").and_then(|x| x.as_str()).map(|s| s.to_string());
    let text = json.get("text").and_then(|x| x.as_str()).map(|s| s.to_string());
    let data = json.get("data").cloned();
    let ts = json.get("ts").and_then(|x| x.as_f64()).unwrap_or(0.0);
    Some(MessageRow { id, thread_id, role, kind, text, data, ts })
}

// -- Tauri commands ---------------------------------------------------------------------------

#[tauri::command]
pub async fn get_thread_count(convex_url: Option<String>) -> Result<usize, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    let mut client = convex::ConvexClient::new(&url).await.map_err(|e| format!("convex connect error: {e}"))?;
    let result = client.query("threads:list", BTreeMap::new()).await.map_err(|e| format!("convex query error: {e}"))?;
    match result { FunctionResult::Value(Value::Array(items)) => Ok(items.len()), FunctionResult::Value(_) => Ok(0), FunctionResult::ErrorMessage(msg) => Err(msg), FunctionResult::ConvexError(err) => Err(err.to_string()) }
}

#[tauri::command]
pub async fn list_recent_threads(limit: Option<u32>, convex_url: Option<String>) -> Result<Vec<ThreadSummary>, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    let mut client = convex::ConvexClient::new(&url).await.map_err(|e| format!("convex connect error: {e}"))?;
    let mut args: BTreeMap<String, Value> = BTreeMap::new(); if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }
    let res = client.query("threads:listWithCounts", args).await.map_err(|e| format!("convex query error: {e}"))?;
    match res {
        FunctionResult::Value(Value::Array(items)) => {
            let mut rows: Vec<ThreadSummary> = Vec::new();
            for item in items { let json: serde_json::Value = item.into(); if let Some(r) = map_thread_item(&json) { rows.push(r); } }
            Ok(rows)
        }
        FunctionResult::Value(_) => Ok(Vec::new()),
        FunctionResult::ErrorMessage(msg) => Err(msg),
        FunctionResult::ConvexError(err) => Err(err.to_string()),
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn list_messages_for_thread(threadId: String, docId: Option<String>, limit: Option<u32>, convex_url: Option<String>) -> Result<Vec<MessageRow>, String> {
    use convex::{FunctionResult, Value};
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    let mut client = convex::ConvexClient::new(&url).await.map_err(|e| format!("convex connect error: {e}"))?;
    let key = select_thread_key(&threadId, docId.as_deref());
    let mut args: BTreeMap<String, Value> = BTreeMap::new(); args.insert("threadId".into(), Value::from(key)); if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }
    let res = client.query("messages:forThread", args).await.map_err(|e| format!("convex query error: {e}"))?;
    match res {
        FunctionResult::Value(Value::Array(items)) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                let json: serde_json::Value = item.into();
                if let Some(row) = map_message_row(&json) { out.push(row); }
            }
            Ok(out)
        }
        FunctionResult::Value(_) => Ok(Vec::new()),
        FunctionResult::ErrorMessage(msg) => Err(msg),
        FunctionResult::ConvexError(err) => Err(err.to_string()),
    }
}

#[tauri::command]
#[allow(non_snake_case)]
pub async fn subscribe_thread_messages(window: tauri::WebviewWindow, threadId: String, docId: Option<String>, limit: Option<u32>, convex_url: Option<String>) -> Result<(), String> {
    use convex::{FunctionResult, Value};
    use futures::StreamExt;
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    tauri::async_runtime::spawn(async move {
        let mut client = match convex::ConvexClient::new(&url).await { Ok(c) => c, Err(e) => { eprintln!("[subscribe_thread_messages] convex connect error: {}", e); return; } };
        let key = select_thread_key(&threadId, docId.as_deref());
        let mut args: BTreeMap<String, Value> = BTreeMap::new(); args.insert("threadId".into(), Value::from(key)); if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }
        let mut sub = match client.subscribe("messages:forThread", args).await { Ok(s) => s, Err(e) => { eprintln!("[subscribe_thread_messages] convex subscribe error: {}", e); return; } };
        while let Some(result) = sub.next().await {
            if let FunctionResult::Value(Value::Array(items)) = result {
                let mut rows: Vec<MessageRow> = Vec::with_capacity(items.len());
                for item in items {
                    let json: serde_json::Value = item.into();
                    if let Some(row) = map_message_row(&json) { rows.push(row); }
                }
                let _ = window.emit("convex:messages", &rows);
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn subscribe_recent_threads(window: tauri::WebviewWindow, limit: Option<u32>, convex_url: Option<String>) -> Result<(), String> {
    use convex::{FunctionResult, Value};
    use futures::StreamExt;
    use std::collections::BTreeMap;
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    tauri::async_runtime::spawn(async move {
        let mut client = match convex::ConvexClient::new(&url).await { Ok(c) => c, Err(e) => { eprintln!("[subscribe_recent_threads] convex connect error: {}", e); return; } };
        let mut args: BTreeMap<String, Value> = BTreeMap::new(); if let Some(l) = limit { args.insert("limit".into(), Value::from(l as i64)); }
        let mut sub = match client.subscribe("threads:listWithCounts", args).await { Ok(s) => s, Err(e) => { eprintln!("[subscribe_recent_threads] convex subscribe error: {}", e); return; } };
        while let Some(result) = sub.next().await { if let FunctionResult::Value(Value::Array(items)) = result { let mut rows: Vec<ThreadSummary> = Vec::new(); for item in items { let json: serde_json::Value = item.into(); if let Some(r) = map_thread_item(&json) { rows.push(r); } } let _ = window.emit("convex:threads", &rows); } }
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn prefers_doc_id_when_present() {
        assert_eq!(select_thread_key("abc", Some("doc")), "doc");
        assert_eq!(select_thread_key("abc", None), "abc");
    }
    #[test]
    fn maps_count_int_and_float() {
        let v = serde_json::json!({"_id":"1","title":"t","updatedAt":1.0,"threadId":"tid","messageCount": 5});
        let r = map_thread_item(&v).unwrap();
        assert_eq!(r.count, Some(5));
        let v2 = serde_json::json!({"_id":"2","title":"t","updatedAt":1.0,"threadId":"tid","messageCount": 5.0});
        let r2 = map_thread_item(&v2).unwrap();
        assert_eq!(r2.count, Some(5));
    }
    #[test]
    fn hide_rules_filter_preface_and_system() {
        let v = serde_json::json!({"_id":"1","text":"<user_instructions> secret","role":"system","ts":1.0});
        assert!(should_hide_message(&v));
        let v2 = serde_json::json!({"_id":"2","text":"ok","role":"assistant","kind":"message","ts":2.0});
        assert!(!should_hide_message(&v2));
        let v3 = serde_json::json!({"_id":"3","text":"ctx","kind":"env","ts":3.0});
        assert!(should_hide_message(&v3));
    }
}
