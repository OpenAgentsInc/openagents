use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{
    AskForApproval, Personality, ReasoningEffort, ReasoningSummary, SandboxPolicy, ServiceTier,
    TextElement, deserialize_double_option, serialize_double_option,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnRef {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UserInput {
    Text {
        text: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        text_elements: Vec<TextElement>,
    },
    Image {
        url: String,
    },
    LocalImage {
        path: PathBuf,
    },
    Skill {
        name: String,
        path: PathBuf,
    },
    Mention {
        name: String,
        path: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(
        default,
        deserialize_with = "deserialize_double_option",
        serialize_with = "serialize_double_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub service_tier: Option<Option<ServiceTier>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<ReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub personality: Option<Personality>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub collaboration_mode: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartResponse {
    pub turn: TurnRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnStartedNotification {
    pub thread_id: String,
    pub turn: TurnRef,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ReviewDelivery {
    Inline,
    Detached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ReviewTarget {
    UncommittedChanges,
    #[serde(rename_all = "camelCase")]
    BaseBranch {
        branch: String,
    },
    #[serde(rename_all = "camelCase")]
    Commit {
        sha: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
    },
    #[serde(rename_all = "camelCase")]
    Custom {
        instructions: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartParams {
    pub thread_id: String,
    pub target: ReviewTarget,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery: Option<ReviewDelivery>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewStartResponse {
    pub turn: TurnRef,
    pub review_thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnCompletedNotification {
    pub thread_id: String,
    pub turn: TurnRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemStartedNotification {
    pub item: Value,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ItemCompletedNotification {
    pub item: Value,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnInterruptParams {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TurnInterruptResponse {}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentMessageDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningSummaryTextDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    pub summary_index: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReasoningTextDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
    pub content_index: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionOutputDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeOutputDeltaNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub delta: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnDiffUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub explanation: Option<String>,
    pub plan: Vec<TurnPlanStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnPlanStep {
    pub step: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsageUpdatedNotification {
    pub thread_id: String,
    pub turn_id: String,
    pub token_usage: ThreadTokenUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadTokenUsage {
    pub input_tokens: i32,
    pub cached_input_tokens: i32,
    pub output_tokens: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextCompactedNotification {
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnError {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorNotification {
    pub error: TurnError,
    pub will_retry: bool,
    pub thread_id: String,
    pub turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_id: Option<String>,
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub available_decisions: Option<Vec<ApprovalDecision>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApprovalDecision {
    Accept,
    AcceptForSession,
    Decline,
    Cancel,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalAcceptSettings {
    pub for_session: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalResponse {
    pub decision: ApprovalDecision,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub accept_settings: Option<ApprovalAcceptSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandExecutionRequestApprovalResponse {
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileChangeRequestApprovalResponse {
    pub decision: ApprovalDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallParams {
    pub thread_id: String,
    pub turn_id: String,
    pub call_id: String,
    pub tool: String,
    pub arguments: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DynamicToolCallOutputContentItem {
    #[serde(rename_all = "camelCase")]
    InputText { text: String },
    #[serde(rename_all = "camelCase")]
    InputImage { image_url: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DynamicToolCallResponse {
    pub content_items: Vec<DynamicToolCallOutputContentItem>,
    pub success: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputOption {
    pub label: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputQuestion {
    pub id: String,
    pub header: String,
    pub question: String,
    #[serde(default)]
    pub is_other: bool,
    #[serde(default)]
    pub is_secret: bool,
    pub options: Option<Vec<ToolRequestUserInputOption>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputParams {
    pub thread_id: String,
    pub turn_id: String,
    pub item_id: String,
    pub questions: Vec<ToolRequestUserInputQuestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputAnswer {
    pub answers: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRequestUserInputResponse {
    pub answers: HashMap<String, ToolRequestUserInputAnswer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptAuthTokensRefreshParams {
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_account_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatgptAuthTokensRefreshResponse {
    pub access_token: String,
    pub chatgpt_account_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chatgpt_plan_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerParams {
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub expected_turn_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnSteerResponse {
    pub turn_id: String,
}
