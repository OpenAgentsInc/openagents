use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
pub struct SyncWatchedDirTs {
    pub provider: String,
    pub base: String,
    pub files: i64,
    pub last_read: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SyncStatusTs {
    pub enabled: bool,
    pub two_way: bool,
    pub watched: Vec<SyncWatchedDirTs>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TinyvexSnapshot<T: Serialize + Clone> {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub stream: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub rows: Vec<T>,
    pub rev: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct TinyvexQueryResult<T: Serialize + Clone> {
    #[serde(rename = "type")]
    pub type_name: &'static str,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_id: Option<String>,
    pub rows: Vec<T>,
}

// Canonical transport row types (TS export lives here, not in Tinyvex)

#[derive(Debug, Clone, Serialize, TS)]
pub struct ThreadRowTs {
    pub id: String,
    pub thread_id: Option<String>,
    pub title: String,
    pub project_id: Option<String>,
    pub resume_id: Option<String>,
    pub rollout_path: Option<String>,
    pub source: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    #[serde(rename = "messageCount", skip_serializing_if = "Option::is_none")]
    pub message_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_message_ts: Option<i64>,
}

impl From<&tinyvex::ThreadRow> for ThreadRowTs {
    fn from(r: &tinyvex::ThreadRow) -> Self {
        ThreadRowTs {
            id: r.id.clone(),
            thread_id: r.thread_id.clone(),
            title: r.title.clone(),
            project_id: r.project_id.clone(),
            resume_id: r.resume_id.clone(),
            rollout_path: r.rollout_path.clone(),
            source: r.source.clone(),
            created_at: r.created_at,
            updated_at: r.updated_at,
            message_count: r.message_count,
            last_message_ts: r.last_message_ts,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MessageRowTs {
    pub id: i64,
    pub thread_id: String,
    pub role: Option<String>,
    pub kind: String,
    pub text: Option<String>,
    pub item_id: Option<String>,
    pub partial: Option<i64>,
    pub seq: Option<i64>,
    pub ts: i64,
    pub created_at: i64,
    pub updated_at: Option<i64>,
}

impl From<&tinyvex::MessageRow> for MessageRowTs {
    fn from(r: &tinyvex::MessageRow) -> Self {
        MessageRowTs {
            id: r.id,
            thread_id: r.thread_id.clone(),
            role: r.role.clone(),
            kind: r.kind.clone(),
            text: r.text.clone(),
            item_id: r.item_id.clone(),
            partial: r.partial,
            seq: r.seq,
            ts: r.ts,
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ToolCallRowTs {
    pub thread_id: String,
    pub tool_call_id: String,
    pub title: Option<String>,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub content_json: Option<String>,
    pub locations_json: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl From<&tinyvex::ToolCallRow> for ToolCallRowTs {
    fn from(r: &tinyvex::ToolCallRow) -> Self {
        ToolCallRowTs {
            thread_id: r.thread_id.clone(),
            tool_call_id: r.tool_call_id.clone(),
            title: r.title.clone(),
            kind: r.kind.clone(),
            status: r.status.clone(),
            content_json: r.content_json.clone(),
            locations_json: r.locations_json.clone(),
            created_at: r.created_at,
            updated_at: r.updated_at,
        }
    }
}
