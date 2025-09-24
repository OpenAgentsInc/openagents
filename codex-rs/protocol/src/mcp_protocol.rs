use std::collections::HashMap;
use std::fmt::Display;
use std::path::PathBuf;

use crate::config_types::ReasoningEffort;
use crate::config_types::ReasoningSummary;
use crate::config_types::SandboxMode;
use crate::config_types::Verbosity;
use crate::protocol::AskForApproval;
use crate::protocol::EventMsg;
use crate::protocol::FileChange;
use crate::protocol::ReviewDecision;
use crate::protocol::SandboxPolicy;
use crate::protocol::TurnAbortReason;
use mcp_types::RequestId;
use serde::Deserialize;
use serde::Serialize;
use strum_macros::Display;
use ts_rs::TS;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, TS, Hash)]
#[ts(type = "string")]
pub struct ConversationId {
    uuid: Uuid,
}

impl ConversationId {
    pub fn new() -> Self {
        Self {
            uuid: Uuid::now_v7(),
        }
    }

    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self {
            uuid: Uuid::parse_str(s)?,
        })
    }
}

impl Default for ConversationId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ConversationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uuid)
    }
}

impl Serialize for ConversationId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.uuid)
    }
}

impl<'de> Deserialize<'de> for ConversationId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self { uuid })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(type = "string")]
pub struct GitSha(pub String);

impl GitSha {
    pub fn new(sha: &str) -> Self {
        Self(sha.to_string())
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, TS)]
#[serde(rename_all = "lowercase")]
pub enum AuthMode {
    ApiKey,
    ChatGPT,
}

/// Request from the client to the server.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum ClientRequest {
    NewConversation {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: NewConversationParams,
    },
    /// List recorded Codex conversations (rollouts) with optional pagination and search.
    ListConversations {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ListConversationsParams,
    },
    /// Resume a recorded Codex conversation from a rollout file.
    ResumeConversation {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ResumeConversationParams,
    },
    ArchiveConversation {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ArchiveConversationParams,
    },
    SendUserMessage {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: SendUserMessageParams,
    },
    SendUserTurn {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: SendUserTurnParams,
    },
    InterruptConversation {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: InterruptConversationParams,
    },
    AddConversationListener {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: AddConversationListenerParams,
    },
    RemoveConversationListener {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: RemoveConversationListenerParams,
    },
    GitDiffToRemote {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: GitDiffToRemoteParams,
    },
    LoginApiKey {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: LoginApiKeyParams,
    },
    LoginChatGpt {
        #[serde(rename = "id")]
        request_id: RequestId,
    },
    CancelLoginChatGpt {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: CancelLoginChatGptParams,
    },
    LogoutChatGpt {
        #[serde(rename = "id")]
        request_id: RequestId,
    },
    GetAuthStatus {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: GetAuthStatusParams,
    },
    GetUserSavedConfig {
        #[serde(rename = "id")]
        request_id: RequestId,
    },
    SetDefaultModel {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: SetDefaultModelParams,
    },
    GetUserAgent {
        #[serde(rename = "id")]
        request_id: RequestId,
    },
    UserInfo {
        #[serde(rename = "id")]
        request_id: RequestId,
    },
    /// Execute a command (argv vector) under the server's sandbox.
    ExecOneOffCommand {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ExecOneOffCommandParams,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, TS)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationParams {
    /// Optional override for the model name (e.g. "o3", "o4-mini").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,

    /// Configuration profile from config.toml to specify default options.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,

    /// Working directory for the session. If relative, it is resolved against
    /// the server process's current working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,

    /// Approval policy for shell commands generated by the model:
    /// `untrusted`, `on-failure`, `on-request`, `never`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,

    /// Sandbox mode: `read-only`, `workspace-write`, or `danger-full-access`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<SandboxMode>,

    /// Individual config settings that will override what is in
    /// CODEX_HOME/config.toml.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<HashMap<String, serde_json::Value>>,

    /// The set of instructions to use instead of the default ones.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_instructions: Option<String>,

    /// Whether to include the plan tool in the conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_plan_tool: Option<bool>,

    /// Whether to include the apply patch tool in the conversation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_apply_patch_tool: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct NewConversationResponse {
    pub conversation_id: ConversationId,
    pub model: String,
    /// Note this could be ignored by the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResumeConversationResponse {
    pub conversation_id: ConversationId,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_messages: Option<Vec<EventMsg>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Default, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsParams {
    /// Optional page size; defaults to a reasonable server-side value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
    /// Opaque pagination cursor returned by a previous call.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub conversation_id: ConversationId,
    pub path: PathBuf,
    pub preview: String,
    /// RFC3339 timestamp string for the session start, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsResponse {
    pub items: Vec<ConversationSummary>,
    /// Opaque cursor to pass to the next call to continue after the last item.
    /// if None, there are no more items to return.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResumeConversationParams {
    /// Absolute path to the rollout JSONL file.
    pub path: PathBuf,
    /// Optional overrides to apply when spawning the resumed session.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overrides: Option<NewConversationParams>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddConversationSubscriptionResponse {
    pub subscription_id: Uuid,
}

/// The [`ConversationId`] must match the `rollout_path`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConversationParams {
    pub conversation_id: ConversationId,
    pub rollout_path: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveConversationResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoveConversationSubscriptionResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginApiKeyParams {
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginApiKeyResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginChatGptResponse {
    pub login_id: Uuid,
    /// URL the client should open in a browser to initiate the OAuth flow.
    pub auth_url: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteResponse {
    pub sha: GitSha,
    pub diff: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelLoginChatGptParams {
    pub login_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffToRemoteParams {
    pub cwd: PathBuf,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct CancelLoginChatGptResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogoutChatGptParams {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LogoutChatGptResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusParams {
    /// If true, include the current auth token (if available) in the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include_token: Option<bool>,
    /// If true, attempt to refresh the token before returning status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecOneOffCommandParams {
    /// Command argv to execute.
    pub command: Vec<String>,
    /// Timeout of the command in milliseconds.
    /// If not specified, a sensible default is used server-side.
    pub timeout_ms: Option<u64>,
    /// Optional working directory for the process. Defaults to server config cwd.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<PathBuf>,
    /// Optional explicit sandbox policy overriding the server default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_policy: Option<SandboxPolicy>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct ExecArbitraryCommandResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetAuthStatusResponse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,

    // Indicates that auth method must be valid to use the server.
    // This can be false if using a custom provider that is configured
    // with requires_openai_auth == false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requires_openai_auth: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUserAgentResponse {
    pub user_agent: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserInfoResponse {
    /// Note: `alleged_user_email` is not currently verified. We read it from
    /// the local auth.json, which the user could theoretically modify. In the
    /// future, we may add logic to verify the email against the server before
    /// returning it.
    pub alleged_user_email: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUserSavedConfigResponse {
    pub config: UserSavedConfig,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultModelParams {
    /// If set to None, this means `model` should be cleared in config.toml.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// If set to None, this means `model_reasoning_effort` should be cleared
    /// in config.toml.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<ReasoningEffort>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetDefaultModelResponse {}

/// UserSavedConfig contains a subset of the config. It is meant to expose mcp
/// client-configurable settings that can be specified in the NewConversation
/// and SendUserTurn requests.
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UserSavedConfig {
    /// Approvals
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval_policy: Option<AskForApproval>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_mode: Option<SandboxMode>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_settings: Option<SandboxSettings>,

    /// Model-specific configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_reasoning_effort: Option<ReasoningEffort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_reasoning_summary: Option<ReasoningSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_verbosity: Option<Verbosity>,

    /// Tools
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Tools>,

    /// Profiles
    #[serde(skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
}

/// MCP representation of a [`codex_core::config_profile::ConfigProfile`].
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub model: Option<String>,
    /// The key in the `model_providers` map identifying the
    /// [`ModelProviderInfo`] to use.
    pub model_provider: Option<String>,
    pub approval_policy: Option<AskForApproval>,
    pub model_reasoning_effort: Option<ReasoningEffort>,
    pub model_reasoning_summary: Option<ReasoningSummary>,
    pub model_verbosity: Option<Verbosity>,
    pub chatgpt_base_url: Option<String>,
}
/// MCP representation of a [`codex_core::config::ToolsToml`].
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub web_search: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub view_image: Option<bool>,
}

/// MCP representation of a [`codex_core::config_types::SandboxWorkspaceWrite`].
#[derive(Deserialize, Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SandboxSettings {
    #[serde(default)]
    pub writable_roots: Vec<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_access: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_tmpdir_env_var: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude_slash_tmp: Option<bool>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageParams {
    pub conversation_id: ConversationId,
    pub items: Vec<InputItem>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserTurnParams {
    pub conversation_id: ConversationId,
    pub items: Vec<InputItem>,
    pub cwd: PathBuf,
    pub approval_policy: AskForApproval,
    pub sandbox_policy: SandboxPolicy,
    pub model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effort: Option<ReasoningEffort>,
    pub summary: ReasoningSummary,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserTurnResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct InterruptConversationParams {
    pub conversation_id: ConversationId,
}

#[derive(Serialize, Deserialize, Debug, Clone, TS)]
#[serde(rename_all = "camelCase")]
pub struct InterruptConversationResponse {
    pub abort_reason: TurnAbortReason,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageResponse {}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AddConversationListenerParams {
    pub conversation_id: ConversationId,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct RemoveConversationListenerParams {
    pub subscription_id: Uuid,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type", content = "data")]
pub enum InputItem {
    Text {
        text: String,
    },
    /// Pre‑encoded data: URI image.
    Image {
        image_url: String,
    },

    /// Local image path provided by the user.  This will be converted to an
    /// `Image` variant (base64 data URL) during request serialization.
    LocalImage {
        path: PathBuf,
    },
}

// TODO(mbolin): Need test to ensure these constants match the enum variants.

pub const APPLY_PATCH_APPROVAL_METHOD: &str = "applyPatchApproval";
pub const EXEC_COMMAND_APPROVAL_METHOD: &str = "execCommandApproval";

/// Request initiated from the server and sent to the client.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(tag = "method", rename_all = "camelCase")]
pub enum ServerRequest {
    /// Request to approve a patch.
    ApplyPatchApproval {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ApplyPatchApprovalParams,
    },
    /// Request to exec a command.
    ExecCommandApproval {
        #[serde(rename = "id")]
        request_id: RequestId,
        params: ExecCommandApprovalParams,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
pub struct ApplyPatchApprovalParams {
    pub conversation_id: ConversationId,
    /// Use to correlate this with [codex_core::protocol::PatchApplyBeginEvent]
    /// and [codex_core::protocol::PatchApplyEndEvent].
    pub call_id: String,
    pub file_changes: HashMap<PathBuf, FileChange>,
    /// Optional explanatory reason (e.g. request for extra write access).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// When set, the agent is asking the user to allow writes under this root
    /// for the remainder of the session (unclear if this is honored today).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant_root: Option<PathBuf>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
pub struct ExecCommandApprovalParams {
    pub conversation_id: ConversationId,
    /// Use to correlate this with [codex_core::protocol::ExecCommandBeginEvent]
    /// and [codex_core::protocol::ExecCommandEndEvent].
    pub call_id: String,
    pub command: Vec<String>,
    pub cwd: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
pub struct ExecCommandApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
pub struct ApplyPatchApprovalResponse {
    pub decision: ReviewDecision,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct LoginChatGptCompleteNotification {
    pub login_id: Uuid,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusChangeNotification {
    /// Current authentication method; omitted if signed out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_method: Option<AuthMode>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS, Display)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum ServerNotification {
    /// Authentication status changed
    AuthStatusChange(AuthStatusChangeNotification),

    /// ChatGPT login flow completed
    LoginChatGptComplete(LoginChatGptCompleteNotification),
}

impl ServerNotification {
    pub fn to_params(self) -> Result<serde_json::Value, serde_json::Error> {
        match self {
            ServerNotification::AuthStatusChange(params) => serde_json::to_value(params),
            ServerNotification::LoginChatGptComplete(params) => serde_json::to_value(params),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn serialize_new_conversation() {
        let request = ClientRequest::NewConversation {
            request_id: RequestId::Integer(42),
            params: NewConversationParams {
                model: Some("gpt-5".to_string()),
                profile: None,
                cwd: None,
                approval_policy: Some(AskForApproval::OnRequest),
                sandbox: None,
                config: None,
                base_instructions: None,
                include_plan_tool: None,
                include_apply_patch_tool: None,
            },
        };
        assert_eq!(
            json!({
                "method": "newConversation",
                "id": 42,
                "params": {
                    "model": "gpt-5",
                    "approvalPolicy": "on-request"
                }
            }),
            serde_json::to_value(&request).unwrap(),
        );
    }

    #[test]
    fn test_conversation_id_default_is_not_zeroes() {
        let id = ConversationId::default();
        assert_ne!(id.uuid, Uuid::nil());
    }

    #[test]
    fn conversation_id_serializes_as_plain_string() {
        let id = ConversationId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap();

        assert_eq!(
            json!("67e55044-10b1-426f-9247-bb680e5fe0c8"),
            serde_json::to_value(id).unwrap()
        );
    }

    #[test]
    fn conversation_id_deserializes_from_plain_string() {
        let id: ConversationId =
            serde_json::from_value(json!("67e55044-10b1-426f-9247-bb680e5fe0c8")).unwrap();

        assert_eq!(
            ConversationId::from_string("67e55044-10b1-426f-9247-bb680e5fe0c8").unwrap(),
            id,
        );
    }
}
