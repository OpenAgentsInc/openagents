use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, TS)]
#[allow(dead_code)]
#[ts(export, export_to = "../../../packages/tricoder/src/types/generated/")]
pub struct SyncWatchedDirTs {
    pub provider: String,
    pub base: String,
    pub files: i64,
    pub last_read: i64,
}

#[derive(Debug, Clone, Serialize, TS)]
#[allow(dead_code)]
#[ts(export, export_to = "../../../packages/tricoder/src/types/generated/")]
pub struct SyncStatusTs {
    pub enabled: bool,
    pub two_way: bool,
    pub watched: Vec<SyncWatchedDirTs>,
}

#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
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
#[allow(dead_code)]
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
#[allow(dead_code)]
#[ts(export, export_to = "../../../packages/tricoder/src/types/generated/")]
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
    #[serde(skip_serializing_if = "Option::is_none")]
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
#[ts(export, export_to = "../../../packages/tricoder/src/types/generated/")]
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
#[ts(export, export_to = "../../packages/tricoder/src/types/generated/")]
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

#[cfg(test)]
mod tests {
    use super::*;
    use ts_rs::TS;

    #[test]
    fn export_bridge_transport_types() {
        let out = std::path::Path::new(".");
        // Export transport rows and sync status (file paths come from #[ts(export_to = ...)])
        ThreadRowTs::export_all_to(out).unwrap();
        MessageRowTs::export_all_to(out).unwrap();
        ToolCallRowTs::export_all_to(out).unwrap();
        SyncStatusTs::export_all_to(out).unwrap();
    }

    /// Test that ThreadRowTs serializes with snake_case field names (ADR-0002 compliance)
    #[test]
    fn thread_row_serializes_snake_case() {
        let row = ThreadRowTs {
            id: "t-123".to_string(),
            thread_id: Some("thread-456".to_string()),
            title: "Test Thread".to_string(),
            project_id: Some("proj-1".to_string()),
            resume_id: Some("resume-789".to_string()),
            rollout_path: Some("/path/to/rollout".to_string()),
            source: Some("codex".to_string()),
            created_at: 1234567890,
            updated_at: 1234567900,
            message_count: Some(42),
            last_message_ts: Some(1234567950),
        };

        let json = serde_json::to_string(&row).expect("Failed to serialize ThreadRowTs");

        // Assert snake_case fields are present
        assert!(json.contains("\"thread_id\""), "Expected thread_id in snake_case");
        assert!(json.contains("\"project_id\""), "Expected project_id in snake_case");
        assert!(json.contains("\"resume_id\""), "Expected resume_id in snake_case");
        assert!(json.contains("\"rollout_path\""), "Expected rollout_path in snake_case");
        assert!(json.contains("\"created_at\""), "Expected created_at in snake_case");
        assert!(json.contains("\"updated_at\""), "Expected updated_at in snake_case");
        assert!(json.contains("\"message_count\""), "Expected message_count in snake_case");
        assert!(json.contains("\"last_message_ts\""), "Expected last_message_ts in snake_case");

        // Assert camelCase variants are NOT present (would violate ADR-0002)
        assert!(!json.contains("\"threadId\""), "Found camelCase threadId - violates ADR-0002");
        assert!(!json.contains("\"projectId\""), "Found camelCase projectId - violates ADR-0002");
        assert!(!json.contains("\"resumeId\""), "Found camelCase resumeId - violates ADR-0002");
        assert!(!json.contains("\"rolloutPath\""), "Found camelCase rolloutPath - violates ADR-0002");
        assert!(!json.contains("\"createdAt\""), "Found camelCase createdAt - violates ADR-0002");
        assert!(!json.contains("\"updatedAt\""), "Found camelCase updatedAt - violates ADR-0002");
        assert!(!json.contains("\"messageCount\""), "Found camelCase messageCount - violates ADR-0002");
        assert!(!json.contains("\"lastMessageTs\""), "Found camelCase lastMessageTs - violates ADR-0002");
    }

    /// Test that MessageRowTs serializes with snake_case field names (ADR-0002 compliance)
    #[test]
    fn message_row_serializes_snake_case() {
        let row = MessageRowTs {
            id: 1,
            thread_id: "t-123".to_string(),
            role: Some("assistant".to_string()),
            kind: "message".to_string(),
            text: Some("Hello world".to_string()),
            item_id: Some("item-456".to_string()),
            partial: Some(0),
            seq: Some(1),
            ts: 1234567890,
            created_at: 1234567890,
            updated_at: Some(1234567900),
        };

        let json = serde_json::to_string(&row).expect("Failed to serialize MessageRowTs");

        // Assert snake_case fields are present
        assert!(json.contains("\"thread_id\""), "Expected thread_id in snake_case");
        assert!(json.contains("\"item_id\""), "Expected item_id in snake_case");
        assert!(json.contains("\"created_at\""), "Expected created_at in snake_case");
        assert!(json.contains("\"updated_at\""), "Expected updated_at in snake_case");

        // Assert camelCase variants are NOT present
        assert!(!json.contains("\"threadId\""), "Found camelCase threadId - violates ADR-0002");
        assert!(!json.contains("\"itemId\""), "Found camelCase itemId - violates ADR-0002");
        assert!(!json.contains("\"createdAt\""), "Found camelCase createdAt - violates ADR-0002");
        assert!(!json.contains("\"updatedAt\""), "Found camelCase updatedAt - violates ADR-0002");
    }

    /// Test that ToolCallRowTs serializes with snake_case field names (ADR-0002 compliance)
    #[test]
    fn tool_call_row_serializes_snake_case() {
        let row = ToolCallRowTs {
            thread_id: "t-123".to_string(),
            tool_call_id: "tc-456".to_string(),
            title: Some("Execute command".to_string()),
            kind: Some("execute".to_string()),
            status: Some("completed".to_string()),
            content_json: Some(r#"{"type":"terminal"}"#.to_string()),
            locations_json: Some(r#"[{"path":"file.rs"}]"#.to_string()),
            created_at: 1234567890,
            updated_at: 1234567900,
        };

        let json = serde_json::to_string(&row).expect("Failed to serialize ToolCallRowTs");

        // Assert snake_case fields are present
        assert!(json.contains("\"thread_id\""), "Expected thread_id in snake_case");
        assert!(json.contains("\"tool_call_id\""), "Expected tool_call_id in snake_case");
        assert!(json.contains("\"content_json\""), "Expected content_json in snake_case");
        assert!(json.contains("\"locations_json\""), "Expected locations_json in snake_case");
        assert!(json.contains("\"created_at\""), "Expected created_at in snake_case");
        assert!(json.contains("\"updated_at\""), "Expected updated_at in snake_case");

        // Assert camelCase variants are NOT present
        assert!(!json.contains("\"threadId\""), "Found camelCase threadId - violates ADR-0002");
        assert!(!json.contains("\"toolCallId\""), "Found camelCase toolCallId - violates ADR-0002");
        assert!(!json.contains("\"contentJson\""), "Found camelCase contentJson - violates ADR-0002");
        assert!(!json.contains("\"locationsJson\""), "Found camelCase locationsJson - violates ADR-0002");
        assert!(!json.contains("\"createdAt\""), "Found camelCase createdAt - violates ADR-0002");
        assert!(!json.contains("\"updatedAt\""), "Found camelCase updatedAt - violates ADR-0002");
    }

    /// Test that SyncStatusTs serializes with snake_case field names (ADR-0002 compliance)
    #[test]
    fn sync_status_serializes_snake_case() {
        let watched_dir = SyncWatchedDirTs {
            provider: "codex".to_string(),
            base: "/home/user/.codex".to_string(),
            files: 42,
            last_read: 1234567890,
        };

        let status = SyncStatusTs {
            enabled: true,
            two_way: false,
            watched: vec![watched_dir],
        };

        let json = serde_json::to_string(&status).expect("Failed to serialize SyncStatusTs");

        // Assert snake_case fields are present
        assert!(json.contains("\"two_way\""), "Expected two_way in snake_case");
        assert!(json.contains("\"last_read\""), "Expected last_read in snake_case");

        // Assert camelCase variants are NOT present
        assert!(!json.contains("\"twoWay\""), "Found camelCase twoWay - violates ADR-0002");
        assert!(!json.contains("\"lastRead\""), "Found camelCase lastRead - violates ADR-0002");
    }

    /// Test that TinyvexSnapshot envelope uses snake_case for thread_id
    #[test]
    fn tinyvex_snapshot_envelope_snake_case() {
        let snapshot = TinyvexSnapshot {
            type_name: "tinyvex.snapshot",
            stream: "messages".to_string(),
            thread_id: Some("t-123".to_string()),
            rows: vec![MessageRowTs {
                id: 1,
                thread_id: "t-123".to_string(),
                role: Some("user".to_string()),
                kind: "message".to_string(),
                text: Some("Test".to_string()),
                item_id: None,
                partial: None,
                seq: None,
                ts: 1234567890,
                created_at: 1234567890,
                updated_at: None,
            }],
            rev: 1,
        };

        let json = serde_json::to_string(&snapshot).expect("Failed to serialize TinyvexSnapshot");

        // Assert snake_case thread_id in envelope
        assert!(json.contains("\"thread_id\""), "Expected thread_id in snake_case");
        assert!(!json.contains("\"threadId\""), "Found camelCase threadId in envelope - violates ADR-0002");
    }

    /// Test that TinyvexQueryResult envelope uses snake_case for thread_id
    #[test]
    fn tinyvex_query_result_envelope_snake_case() {
        let result = TinyvexQueryResult {
            type_name: "tinyvex.query_result",
            name: "messages.list".to_string(),
            thread_id: Some("t-123".to_string()),
            rows: vec![MessageRowTs {
                id: 1,
                thread_id: "t-123".to_string(),
                role: Some("assistant".to_string()),
                kind: "message".to_string(),
                text: Some("Response".to_string()),
                item_id: None,
                partial: None,
                seq: None,
                ts: 1234567890,
                created_at: 1234567890,
                updated_at: None,
            }],
        };

        let json = serde_json::to_string(&result).expect("Failed to serialize TinyvexQueryResult");

        // Assert snake_case thread_id in envelope
        assert!(json.contains("\"thread_id\""), "Expected thread_id in snake_case");
        assert!(!json.contains("\"threadId\""), "Found camelCase threadId in envelope - violates ADR-0002");
    }
}
