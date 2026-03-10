use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AskForApproval, Personality, ReasoningEffort, SandboxMode, SandboxPolicy, ServiceTier,
    deserialize_double_option, serialize_double_option,
};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_double_option",
        serialize_with = "serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<ServiceTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dynamic_tools: Option<Vec<DynamicToolSpec>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolSpec {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRef {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartResponse {
    pub thread: ThreadRef,
    pub model: String,
    #[serde(default)]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub approval_policy: Option<AskForApproval>,
    #[serde(default, rename = "sandbox")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_double_option",
        serialize_with = "serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<ServiceTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadResumeResponse {
    pub thread: ThreadSnapshot,
    pub model: String,
    #[serde(default)]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub approval_policy: Option<AskForApproval>,
    #[serde(default, rename = "sandbox")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadParams {
    pub thread_id: String,
    #[serde(default)]
    pub include_turns: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadReadResponse {
    pub thread: ThreadSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSnapshot {
    pub id: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub turns: Vec<ThreadTurn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTurn {
    pub id: String,
    #[serde(default)]
    pub items: Vec<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadArchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadArchiveResponse {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort_key: Option<ThreadSortKey>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_providers: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_kinds: Option<Vec<ThreadSourceKind>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    #[serde(default)]
    pub preview: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub model_provider: String,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub created_at: i64,
    #[serde(default)]
    pub updated_at: i64,
    #[serde(default)]
    pub status: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadListResponse {
    pub data: Vec<ThreadSummary>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ModelListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_hidden: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningEffortOption {
    pub reasoning_effort: ReasoningEffort,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub model: String,
    pub display_name: String,
    pub description: String,
    #[serde(default)]
    pub hidden: bool,
    #[serde(default)]
    pub supported_reasoning_efforts: Vec<ReasoningEffortOption>,
    pub default_reasoning_effort: ReasoningEffort,
    #[serde(default)]
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelListResponse {
    pub data: Vec<ModelInfo>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecParams {
    pub command: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadStartedNotification {
    pub thread: ThreadRef,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSortKey {
    CreatedAt,
    UpdatedAt,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThreadSourceKind {
    Cli,
    #[serde(rename = "vscode")]
    VsCode,
    Exec,
    AppServer,
    SubAgent,
    SubAgentReview,
    SubAgentCompact,
    SubAgentThreadSpawn,
    SubAgentOther,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListParams {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadLoadedListResponse {
    pub data: Vec<String>,
    #[serde(default)]
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkParams {
    pub thread_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_provider: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_double_option",
        serialize_with = "serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<ServiceTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub developer_instructions: Option<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub persist_extended_history: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadForkResponse {
    pub thread: ThreadSnapshot,
    pub model: String,
    #[serde(default)]
    pub model_provider: Option<String>,
    #[serde(default)]
    pub service_tier: Option<ServiceTier>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub approval_policy: Option<AskForApproval>,
    #[serde(default, rename = "sandbox")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(default)]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnsubscribeParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ThreadUnsubscribeStatus {
    NotLoaded,
    NotSubscribed,
    Unsubscribed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnsubscribeResponse {
    pub status: ThreadUnsubscribeStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSetNameParams {
    pub thread_id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadSetNameResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchiveParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadUnarchiveResponse {
    pub thread: ThreadSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactStartParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadCompactStartResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminalsCleanParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadBackgroundTerminalsCleanResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackParams {
    pub thread_id: String,
    pub num_turns: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRollbackResponse {
    pub thread: ThreadSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeStartParams {
    pub thread_id: String,
    pub prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeStartResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeAudioChunk {
    pub data: String,
    pub sample_rate: u32,
    pub num_channels: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub samples_per_channel: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeAppendAudioParams {
    pub thread_id: String,
    pub audio: ThreadRealtimeAudioChunk,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeAppendAudioResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeAppendTextParams {
    pub thread_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeAppendTextResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeStopParams {
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ThreadRealtimeStopResponse {}
