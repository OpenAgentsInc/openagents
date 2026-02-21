use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use autopilot_inbox_domain::{
    DraftQualityReport, DraftStatus, PolicyDecision, RiskTier, ThreadCategory,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyMode {
    LocalOnly,
    #[default]
    Hybrid,
    Cloud,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentStorageMode {
    None,
    #[default]
    Metadata,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateRequest {
    pub client_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionCreateResponse {
    pub session_token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub connected_gmail: bool,
    pub connected_chatgpt: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailAuthUrlResponse {
    pub url: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmailAuthRequest {
    pub code: String,
    pub redirect_uri: String,
    pub code_verifier: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatGptAuthRequest {
    pub api_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthStatusResponse {
    pub connected: bool,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillRequest {
    pub days: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackfillResponse {
    pub imported_threads: usize,
    pub imported_messages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub subject: String,
    pub snippet: String,
    pub from_address: String,
    pub category: Option<ThreadCategory>,
    pub risk: Option<RiskTier>,
    pub policy: Option<PolicyDecision>,
    pub last_message_at: DateTime<Utc>,
    pub has_pending_draft: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadListResponse {
    pub threads: Vec<ThreadSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub id: String,
    pub thread_id: String,
    pub sender: String,
    pub recipient: String,
    pub body: String,
    pub snippet: String,
    pub inbound: bool,
    pub sent_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftRecord {
    pub id: String,
    pub thread_id: String,
    pub body: String,
    pub status: DraftStatus,
    pub source_summary: String,
    pub model_used: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetailResponse {
    pub thread: ThreadSummary,
    pub messages: Vec<MessageRecord>,
    pub draft: Option<DraftRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DraftListResponse {
    pub drafts: Vec<DraftRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GenerateDraftResponse {
    pub draft: DraftRecord,
    pub category: ThreadCategory,
    pub risk: RiskTier,
    pub policy: PolicyDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApproveSendResponse {
    pub draft_id: String,
    pub gmail_message_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResponse {
    pub category: Option<ThreadCategory>,
    pub risk: Option<RiskTier>,
    pub policy: Option<PolicyDecision>,
    pub similar_thread_ids: Vec<String>,
    pub external_model_used: bool,
    pub events: Vec<EventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: String,
    pub thread_id: Option<String>,
    pub event_type: String,
    pub payload: Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListResponse {
    pub events: Vec<EventRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSettingsRequest {
    pub privacy_mode: PrivacyMode,
    pub backfill_days: u32,
    pub allowed_recipient_domains: Vec<String>,
    pub attachment_storage_mode: AttachmentStorageMode,
    pub signature: Option<String>,
    pub template_scheduling: Option<String>,
    pub template_report_delivery: Option<String>,
    pub sync_interval_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsResponse {
    pub privacy_mode: PrivacyMode,
    pub backfill_days: u32,
    pub allowed_recipient_domains: Vec<String>,
    pub attachment_storage_mode: AttachmentStorageMode,
    pub signature: Option<String>,
    pub template_scheduling: Option<String>,
    pub template_report_delivery: Option<String>,
    pub sync_interval_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncNowResponse {
    pub imported_threads: usize,
    pub imported_messages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportAuditResponse {
    pub path: String,
    pub exported_events: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSuggestion {
    pub id: String,
    pub category: ThreadCategory,
    pub template_text: String,
    pub occurrences: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateMineResponse {
    pub suggestions: Vec<TemplateSuggestion>,
}
