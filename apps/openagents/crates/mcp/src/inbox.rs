//! New messages for the calling chat while one of its tools blocks (#11969).
//!
//! A blocking call (`wait_for_turn`, or a send/create with `wait`) races its
//! wait against this watch and returns early when the chat it speaks for gets
//! a new message: a user message written into its transcript (a mid-turn
//! steer from the owner or another chat's `send_message`) or a row added to
//! its queue (a turn-boundary harness holds messages there until the turn
//! ends). Everything already there when the watch opened is the baseline.

use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;

use openagents_doc::{MessagePart, MessageRole, TranscriptFrame, apply_transcript_frame};
use openagents_rpc::methods;
use serde_json::{Value, json};
use tokio::sync::{oneshot, watch};

use crate::openagents::OpenAgents;

/// How long a blocking call waits for the watch to read its baseline before
/// it starts waiting anyway (the watch keeps going in the background).
const READY_TIMEOUT: Duration = Duration::from_secs(5);
/// Gap between resubscribes when a watch stream ends at a lifecycle boundary.
const RESUBSCRIBE_DELAY: Duration = Duration::from_millis(300);
/// Longest message text a result carries.
const TEXT_MAX: usize = 4000;

pub struct Inbox {
    arrived: watch::Receiver<Option<Vec<Value>>>,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Inbox {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Inbox {
    /// Watch the chat this server speaks for. `None` outside a chat.
    pub async fn open(openagents: Arc<OpenAgents>) -> Option<Self> {
        let chat_id = openagents.origin().chat_id.clone()?;
        let device_id = openagents.origin().device_id.clone();
        let (tx, arrived) = watch::channel(None);
        let (transcript_ready, transcript_ready_rx) = oneshot::channel();
        let (queue_ready, queue_ready_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            let found = tokio::select! {
                found = watch_transcript(&openagents, &chat_id, device_id.as_deref(), transcript_ready) => found,
                found = watch_queue(&openagents, &chat_id, queue_ready) => found,
            };
            let _ = tx.send(Some(found));
        });
        let _ = tokio::time::timeout(READY_TIMEOUT, async {
            let _ = transcript_ready_rx.await;
            let _ = queue_ready_rx.await;
        })
        .await;
        Some(Self { arrived, task })
    }

    /// Whether a new message has already arrived.
    pub fn fired(&self) -> bool {
        self.arrived.borrow().is_some()
    }

    /// Resolves with the new messages once one arrives; never otherwise.
    pub async fn arrived(&self) -> Vec<Value> {
        let mut rx = self.arrived.clone();
        let found = rx
            .wait_for(Option::is_some)
            .await
            .map(|found| found.clone().unwrap_or_default());
        match found {
            Ok(found) => found,
            Err(_) => std::future::pending().await,
        }
    }
}

/// User messages that were not in the transcript when the watch opened.
async fn watch_transcript(
    openagents: &OpenAgents,
    chat_id: &str,
    device_id: Option<&str>,
    ready: oneshot::Sender<()>,
) -> Vec<Value> {
    let mut ready = Some(ready);
    let mut baseline: Option<HashSet<String>> = None;
    let mut params = json!({ "chatId": chat_id });
    if let Some(device) = device_id {
        params["targetDeviceId"] = json!(device);
    }
    loop {
        let mut rx = match openagents
            .subscribe(methods::WATCH_DOC_MESSAGES, params.clone())
            .await
        {
            Ok(rx) => rx,
            Err(err) => {
                tracing::warn!(error = %err, "inbox: transcript watch unavailable");
                drop(ready.take());
                return std::future::pending().await;
            }
        };
        let mut entries = Vec::new();
        let mut reset = false;
        while let Some(item) = rx.recv().await {
            reset |= item.get("reset").is_some();
            let Ok(frame) = serde_json::from_value::<TranscriptFrame>(item) else {
                continue;
            };
            if apply_transcript_frame(&mut entries, frame).is_err() || !reset {
                continue;
            }
            let users = entries.iter().filter(|e| e.role == MessageRole::User);
            let Some(seen) = &baseline else {
                baseline = Some(users.map(|e| e.id.clone()).collect());
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
                continue;
            };
            let new: Vec<Value> = users
                .filter(|e| !seen.contains(&e.id))
                .map(|e| {
                    let text: String = e
                        .parts
                        .iter()
                        .filter_map(|p| match p {
                            MessagePart::Text { text, .. } => Some(text.as_str()),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    message(&e.id, &text, false)
                })
                .collect();
            if !new.is_empty() {
                return new;
            }
        }
        // Don't hold the caller for a stream that ended before its baseline.
        drop(ready.take());
        tokio::time::sleep(RESUBSCRIBE_DELAY).await;
    }
}

/// Queue rows that were not there when the watch opened.
async fn watch_queue(
    openagents: &OpenAgents,
    chat_id: &str,
    ready: oneshot::Sender<()>,
) -> Vec<Value> {
    let mut ready = Some(ready);
    let mut baseline: Option<HashSet<String>> = None;
    loop {
        let mut rx = match openagents
            .subscribe(methods::WATCH_QUEUE, json!({ "chatId": chat_id }))
            .await
        {
            Ok(rx) => rx,
            Err(err) => {
                tracing::warn!(error = %err, "inbox: queue watch unavailable");
                drop(ready.take());
                return std::future::pending().await;
            }
        };
        while let Some(item) = rx.recv().await {
            let Some(items) = item.get("items").and_then(Value::as_array) else {
                continue;
            };
            let rows = items
                .iter()
                .filter_map(|row| Some((row.get("id")?.as_str()?, row.get("text")?.as_str()?)));
            let Some(seen) = &baseline else {
                baseline = Some(rows.map(|(id, _)| id.to_owned()).collect());
                if let Some(ready) = ready.take() {
                    let _ = ready.send(());
                }
                continue;
            };
            let new: Vec<Value> = rows
                .filter(|(id, _)| !seen.contains(*id))
                .map(|(id, text)| message(id, text, true))
                .collect();
            if !new.is_empty() {
                return new;
            }
        }
        // Don't hold the caller for a stream that ended before its baseline.
        drop(ready.take());
        tokio::time::sleep(RESUBSCRIBE_DELAY).await;
    }
}

fn message(id: &str, text: &str, queued: bool) -> Value {
    let mut text = text.trim().to_owned();
    if let Some((cut, _)) = text.char_indices().nth(TEXT_MAX) {
        text.truncate(cut);
        text.push('…');
    }
    json!({ "id": id, "text": text, "queued": queued })
}
