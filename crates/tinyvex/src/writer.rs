use std::collections::HashMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use crate::{Tinyvex, ThreadRow};

fn now_ms() -> i64 {
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis()) as i64
}

#[derive(Debug, Clone)]
struct StreamEntry {
    item_id: String,
    last_text: String,
    seq: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WriterNotification {
    ThreadsUpsert { row: ThreadRow },
    MessagesUpsert {
        thread_id: String,
        item_id: String,
        kind: String,
        role: Option<String>,
        seq: i64,
        text_len: usize,
    },
    MessagesFinalize {
        thread_id: String,
        item_id: String,
        kind: String,
        text_len: usize,
    },
    ToolCallUpsert { thread_id: String, tool_call_id: String },
    ToolCallUpdate { thread_id: String, tool_call_id: String },
    PlanUpsert { thread_id: String },
    StateUpsert { thread_id: String },
}

pub struct Writer {
    tvx: Arc<Tinyvex>,
    stream_track: Mutex<HashMap<String, StreamEntry>>, // key: "<thread>|<kind>"
}

impl Writer {
    pub fn new(tvx: Arc<Tinyvex>) -> Self {
        Self { tvx, stream_track: Mutex::new(HashMap::new()) }
    }

    fn map_kind_role<'a>(kind: &'a str) -> (&'a str, Option<&'a str>) {
        if kind == "assistant" {
            ("message", Some("assistant"))
        } else if kind == "reason" {
            ("reason", None)
        } else if kind == "user" {
            ("message", Some("user"))
        } else {
            (kind, None)
        }
    }

    pub async fn stream_upsert_or_append(
        &self,
        provider: &str,
        thread_id: &str,
        kind: &str,
        full_text: &str,
    ) -> Vec<WriterNotification> {
        let key = format!("{}|{}", thread_id, kind);
        let mut guard = self.stream_track.lock().await;
        let entry = guard
            .entry(key.clone())
            .or_insert_with(|| StreamEntry { item_id: String::new(), last_text: String::new(), seq: 0 });
        entry.seq = entry.seq.saturating_add(1);
        entry.last_text = full_text.to_string();
        if entry.item_id.is_empty() {
            entry.item_id = format!("turn:{}:{}", now_ms(), kind);
        }
        let seq_now = entry.seq as i64;
        let item_id = entry.item_id.clone();
        drop(guard);

        let (out_kind, role) = Self::map_kind_role(kind);
        let t = now_ms();
        // Ensure thread row exists and is current
        let thr = ThreadRow {
            id: thread_id.to_string(),
            thread_id: Some(thread_id.to_string()),
            title: "Thread".into(),
            project_id: None,
            resume_id: Some(thread_id.to_string()),
            rollout_path: None,
            source: Some(provider.to_string()),
            created_at: t,
            updated_at: t,
            message_count: None,
            last_message_ts: None,
        };
        let _ = self.tvx.upsert_thread(&thr);
        let _ = self
            .tvx
            .upsert_streamed_message(thread_id, out_kind, role, full_text, &item_id, seq_now, t);

        vec![
            WriterNotification::ThreadsUpsert { row: thr },
            WriterNotification::MessagesUpsert {
                thread_id: thread_id.to_string(),
                item_id,
                kind: out_kind.to_string(),
                role: role.map(|s| s.to_string()),
                seq: seq_now,
                text_len: full_text.len(),
            },
        ]
    }

    pub async fn try_finalize_stream_kind(
        &self,
        thread_id: &str,
        kind: &str,
    ) -> Option<Vec<WriterNotification>> {
        let key = format!("{}|{}", thread_id, kind);
        let (item_id, final_text) = {
            let mut guard = self.stream_track.lock().await;
            if let Some(entry) = guard.remove(&key) {
                (entry.item_id, entry.last_text)
            } else {
                return None;
            }
        };
        let t = now_ms();
        let (out_kind, role) = Self::map_kind_role(kind);
        let _ = self
            .tvx
            .finalize_streamed_message_with_kind(thread_id, &item_id, &final_text, t, out_kind, role);
        Some(vec![WriterNotification::MessagesFinalize {
            thread_id: thread_id.to_string(),
            item_id,
            kind: out_kind.to_string(),
            text_len: final_text.len(),
        }])
    }

    pub async fn finalize_or_snapshot(
        &self,
        provider: &str,
        thread_id: &str,
        kind: &str,
        final_text: &str,
    ) -> Vec<WriterNotification> {
        if let Some(notifs) = self.try_finalize_stream_kind(thread_id, kind).await {
            return notifs;
        }

        // REMOVED: message_exists check was causing false positives
        // Short repeated text chunks like " the" or "\n\n" were incorrectly
        // flagged as duplicates, preventing ALL assistant messages from being stored.
        // The duplicate check in finalize_streamed_message_with_kind is more robust
        // as it only applies when finalizing without an active stream.

        let mut notifs = self
            .stream_upsert_or_append(provider, thread_id, kind, final_text)
            .await;
        if let Some(mut fin) = self.try_finalize_stream_kind(thread_id, kind).await {
            notifs.append(&mut fin);
        }
        notifs
    }

    pub async fn finalize_streaming_for_thread(&self, thread_id: &str) -> Vec<WriterNotification> {
        let keys: Vec<String> = {
            let guard = self.stream_track.lock().await;
            guard
                .keys()
                .filter_map(|k| {
                    let mut p = k.split('|');
                    let tid = p.next()?;
                    let kind = p.next()?;
                    if tid == thread_id { Some(kind.to_string()) } else { None }
                })
                .collect()
        };
        let mut out = Vec::new();
        for kind in keys {
            if let Some(mut v) = self.try_finalize_stream_kind(thread_id, &kind).await {
                out.append(&mut v);
            }
        }
        out
    }

    pub async fn mirror_acp_update_to_tinyvex(
        &self,
        provider: &str,
        thread_id: &str,
        update: &agent_client_protocol::SessionUpdate,
    ) -> Vec<WriterNotification> {
        use agent_client_protocol::SessionUpdate as SU;
        let t = now_ms();
        match update {
            SU::ToolCall(tc) => {
                let id = format!("{:?}", tc.id);
                let title = tc.title.as_str();
                let kind = format!("{:?}", tc.kind);
                let status = format!("{:?}", tc.status);
                let content_json = serde_json::to_string(&tc.content).unwrap_or("[]".into());
                let locations_json = serde_json::to_string(&tc.locations).unwrap_or("[]".into());
                let _ = self.tvx.upsert_acp_tool_call(
                    thread_id,
                    &id,
                    Some(title),
                    Some(&kind),
                    Some(&status),
                    Some(&content_json),
                    Some(&locations_json),
                    t,
                );
                vec![WriterNotification::ToolCallUpsert { thread_id: thread_id.to_string(), tool_call_id: id }]
            }
            SU::ToolCallUpdate(tc) => {
                let id = format!("{:?}", tc.id);
                let _title: &str = tc.fields.title.as_deref().unwrap_or("");
                let kind_s: Option<String> = tc
                    .fields
                    .kind
                    .as_ref()
                    .map(|k| format!("{:?}", k));
                let status_s: Option<String> = tc
                    .fields
                    .status
                    .as_ref()
                    .map(|s| format!("{:?}", s));
                let content_json_s: Option<String> = tc
                    .fields
                    .content
                    .as_ref()
                    .map(|c| serde_json::to_string(c).unwrap_or("[]".into()));
                let locations_json_s: Option<String> = tc
                    .fields
                    .locations
                    .as_ref()
                    .map(|l| serde_json::to_string(l).unwrap_or("[]".into()));
                let _ = self.tvx.upsert_acp_tool_call(
                    thread_id,
                    &id,
                    tc.fields.title.as_deref(),
                    kind_s.as_deref(),
                    status_s.as_deref(),
                    content_json_s.as_deref(),
                    locations_json_s.as_deref(),
                    t,
                );
                vec![WriterNotification::ToolCallUpdate { thread_id: thread_id.to_string(), tool_call_id: id }]
            }
            SU::Plan(p) => {
                let entries_json = serde_json::to_string(&p.entries).unwrap_or("[]".into());
                let _ = self.tvx.upsert_acp_plan(thread_id, &entries_json, t);
                vec![WriterNotification::PlanUpsert { thread_id: thread_id.to_string() }]
            }
            SU::AvailableCommandsUpdate(ac) => {
                let cmds_json = serde_json::to_string(&ac.available_commands).unwrap_or("[]".into());
                let _ = self.tvx.upsert_acp_state(thread_id, None, Some(&cmds_json), t);
                vec![WriterNotification::StateUpsert { thread_id: thread_id.to_string() }]
            }
            SU::CurrentModeUpdate(_cm) => {
                let _ = self.tvx.upsert_acp_state(thread_id, None, None, t);
                vec![WriterNotification::StateUpsert { thread_id: thread_id.to_string() }]
            }
            SU::UserMessageChunk(ch) => {
                let txt = content_to_text(&ch.content);
                if !txt.is_empty() {
                    self.finalize_or_snapshot(provider, thread_id, "user", &txt).await
                } else {
                    Vec::new()
                }
            }
            SU::AgentMessageChunk(ch) => {
                let txt = content_to_text(&ch.content);
                if !txt.is_empty() {
                    self.finalize_or_snapshot(provider, thread_id, "assistant", &txt).await
                } else {
                    Vec::new()
                }
            }
            SU::AgentThoughtChunk(ch) => {
                let txt = content_to_text(&ch.content);
                if !txt.is_empty() {
                    self.finalize_or_snapshot(provider, thread_id, "reason", &txt).await
                } else {
                    Vec::new()
                }
            }
        }
    }
}

fn content_to_text(content: &agent_client_protocol::ContentBlock) -> String {
    match content {
        agent_client_protocol::ContentBlock::Text(agent_client_protocol::TextContent { text, .. }) => text.clone(),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol as acp;

    #[tokio::test]
    async fn mirror_user_and_assistant_updates_last_message_ts_and_roles() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-1";

        // user chunk then assistant chunk
        let u = acp::SessionUpdate::UserMessageChunk(acp::ContentChunk { content: acp::ContentBlock::Text(acp::TextContent{ annotations: None, text: "hi".into(), meta: None }), meta: None });
        let a = acp::SessionUpdate::AgentMessageChunk(acp::ContentChunk { content: acp::ContentBlock::Text(acp::TextContent{ annotations: None, text: "hello".into(), meta: None }), meta: None });
        writer.mirror_acp_update_to_tinyvex("codex", tid, &u).await;
        writer.mirror_acp_update_to_tinyvex("codex", tid, &a).await;

        let threads = tvx.list_threads(10).unwrap();
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].id, tid);
        assert!(threads[0].last_message_ts.unwrap_or(0) > 0);

        let msgs = tvx.list_messages(tid, 50).unwrap();
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].role.as_deref(), Some("user"));
        assert_eq!(msgs[1].role.as_deref(), Some("assistant"));
    }

    #[tokio::test]
    async fn finalize_or_snapshot_preserves_role_for_assistant_messages() {
        // Tests that when finalize_or_snapshot is called with "assistant" kind,
        // and no prior stream exists (direct insert path), the role is correctly set.
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-finalize-test";

        // Directly finalize an assistant message without prior stream
        // This exercises the code path where finalize_or_snapshot creates a new message
        writer.finalize_or_snapshot("codex", tid, "assistant", "Hey there!").await;

        let msgs = tvx.list_messages(tid, 50).unwrap();
        assert_eq!(msgs.len(), 1, "expected exactly one message");
        assert_eq!(msgs[0].kind.as_str(), "message", "kind should be 'message'");
        assert_eq!(msgs[0].role.as_deref(), Some("assistant"), "role should be 'assistant'");
        assert_eq!(msgs[0].text.as_deref(), Some("Hey there!"));
        assert_eq!(msgs[0].partial, Some(0), "message should be finalized");
    }

    #[tokio::test]
    async fn finalize_or_snapshot_preserves_role_for_user_messages() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-user-test";

        writer.finalize_or_snapshot("codex", tid, "user", "Hi").await;

        let msgs = tvx.list_messages(tid, 50).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].kind.as_str(), "message");
        assert_eq!(msgs[0].role.as_deref(), Some("user"));
        assert_eq!(msgs[0].text.as_deref(), Some("Hi"));
    }

    #[tokio::test]
    async fn finalize_or_snapshot_preserves_role_for_reasoning() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-reason-test";

        writer.finalize_or_snapshot("codex", tid, "reason", "**Thinking...**").await;

        let msgs = tvx.list_messages(tid, 50).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].kind.as_str(), "reason");
        assert_eq!(msgs[0].role, None, "reasoning messages should have NULL role");
        assert_eq!(msgs[0].text.as_deref(), Some("**Thinking...**"));
    }

    #[tokio::test]
    async fn multiple_assistant_chunks_with_short_repeated_text_all_stored() {
        // Tests that short repeated text like " the" doesn't trigger false duplicate detection
        // This was the bug: message_exists() would find " the" from chunk 1 and skip chunk 2
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-chunks";

        // Simulate ACP AgentMessageChunk stream with accumulated text
        writer.finalize_or_snapshot("codex", tid, "assistant", "Hello").await;
        writer.finalize_or_snapshot("codex", tid, "assistant", "Hello there").await;
        writer.finalize_or_snapshot("codex", tid, "assistant", "Hello there!").await;

        let msgs = tvx.list_messages(tid, 50).unwrap();
        // Should have 1 finalized message with the final text
        assert_eq!(msgs.len(), 1, "should have exactly one finalized message");
        assert_eq!(msgs[0].kind.as_str(), "message", "assistant kind should map to 'message'");
        assert_eq!(msgs[0].role.as_deref(), Some("assistant"), "assistant role should be set");
        assert_eq!(msgs[0].text.as_deref(), Some("Hello there!"), "should have final accumulated text");
        assert_eq!(msgs[0].partial, Some(0), "message should be finalized (partial=0)");
    }

    #[tokio::test]
    async fn streaming_updates_then_finalize_preserves_role() {
        // Tests the full streaming flow: stream_upsert_or_append -> finalize
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("tvx.sqlite3");
        let tvx = std::sync::Arc::new(crate::Tinyvex::open(&db).unwrap());
        let writer = Writer::new(tvx.clone());
        let tid = "t-stream-role";

        // Simulate streaming chunks
        writer.stream_upsert_or_append("codex", tid, "assistant", "Hey").await;
        writer.stream_upsert_or_append("codex", tid, "assistant", "Hey there").await;

        // Finalize the stream
        writer.try_finalize_stream_kind(tid, "assistant").await;

        let msgs = tvx.list_messages(tid, 50).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].kind.as_str(), "message");
        assert_eq!(msgs[0].role.as_deref(), Some("assistant"), "role must be 'assistant' not NULL");
        assert_eq!(msgs[0].text.as_deref(), Some("Hey there"));
        assert_eq!(msgs[0].partial, Some(0), "should be finalized");
    }

}
