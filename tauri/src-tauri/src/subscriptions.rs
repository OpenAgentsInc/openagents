//! Live subscription commands for Convex streams.
//!
//! These commands establish server‑pushed subscriptions from Convex to the
//! desktop webview and re‑emit rows as compact payloads via `window.emit`.

use crate::convex::{map_message_row, map_thread_item, select_thread_key, MessageRow, ThreadSummary};
use convex::{FunctionResult, Value};
use futures::StreamExt;
use std::collections::BTreeMap;
use tauri::Emitter;

#[tauri::command]
#[allow(non_snake_case)]
pub async fn subscribe_thread_messages(
    window: tauri::WebviewWindow,
    threadId: String,
    docId: Option<String>,
    limit: Option<u32>,
    convex_url: Option<String>,
) -> Result<(), String> {
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    tauri::async_runtime::spawn(async move {
        let mut client = match convex::ConvexClient::new(&url).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[subscribe_thread_messages] convex connect error: {}", e);
                return;
            }
        };
        let key = select_thread_key(&threadId, docId.as_deref());
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        args.insert("threadId".into(), Value::from(key));
        if let Some(l) = limit {
            args.insert("limit".into(), Value::from(l as i64));
        }
        let mut sub = match client.subscribe("messages:forThread", args).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[subscribe_thread_messages] convex subscribe error: {}", e);
                return;
            }
        };
        while let Some(result) = sub.next().await {
            if let FunctionResult::Value(Value::Array(items)) = result {
                let mut rows: Vec<MessageRow> = Vec::with_capacity(items.len());
                for item in items {
                    let json: serde_json::Value = item.into();
                    if let Some(row) = map_message_row(&json) {
                        rows.push(row);
                    }
                }
                let _ = window.emit("convex:messages", &rows);
            }
        }
    });
    Ok(())
}

#[tauri::command]
pub async fn subscribe_recent_threads(
    window: tauri::WebviewWindow,
    limit: Option<u32>,
    convex_url: Option<String>,
) -> Result<(), String> {
    let default_port: u16 = std::env::var("OPENAGENTS_CONVEX_PORT").ok().and_then(|s| s.parse().ok()).unwrap_or(7788);
    let url = convex_url.or_else(|| std::env::var("CONVEX_URL").ok()).unwrap_or_else(|| format!("http://127.0.0.1:{}", default_port));
    tauri::async_runtime::spawn(async move {
        let mut client = match convex::ConvexClient::new(&url).await {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[subscribe_recent_threads] convex connect error: {}", e);
                return;
            }
        };
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        if let Some(l) = limit {
            args.insert("limit".into(), Value::from(l as i64));
        }
        let mut sub = match client.subscribe("threads:listWithCounts", args).await {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[subscribe_recent_threads] convex subscribe error: {}", e);
                return;
            }
        };
        while let Some(result) = sub.next().await {
            if let FunctionResult::Value(Value::Array(items)) = result {
                let mut rows: Vec<ThreadSummary> = Vec::new();
                for item in items {
                    let json: serde_json::Value = item.into();
                    if let Some(r) = map_thread_item(&json) {
                        rows.push(r);
                    }
                }
                let _ = window.emit("convex:threads", &rows);
            }
        }
    });
    Ok(())
}
