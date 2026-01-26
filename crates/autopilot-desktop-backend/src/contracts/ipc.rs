use serde::{Deserialize, Serialize};
use ts_rs::{Dependency, TS};

use crate::agent::unified::{AgentId, UnifiedConversationItem, UnifiedEvent};

pub struct JsonValue;

impl TS for JsonValue {
    fn name() -> String {
        "JsonValue".to_string()
    }

    fn decl() -> String {
        "type JsonValue = unknown;".to_string()
    }

    fn inline() -> String {
        Self::name()
    }

    fn dependencies() -> Vec<Dependency> {
        Vec::new()
    }

    fn transparent() -> bool {
        false
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CodexDoctorRequest {
    pub codex_bin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct CodexDoctorResponse {
    pub ok: bool,
    pub codex_bin: Option<String>,
    pub version: Option<String>,
    pub app_server_ok: bool,
    pub details: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TestCodexConnectionRequest {
    pub workspace_path: String,
    pub codex_bin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct TestCodexConnectionResponse {
    pub success: bool,
    pub message: String,
    #[ts(type = "JsonValue | null")]
    pub models: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectWorkspaceRequest {
    pub workspace_id: String,
    pub workspace_path: String,
    pub codex_bin: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConnectionResponse {
    pub success: bool,
    pub message: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceConnectionStatusResponse {
    pub workspace_id: String,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectWorkspaceRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartThreadRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StartThreadResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListThreadsRequest {
    pub workspace_id: String,
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub sort_key: Option<String>,
    pub archived: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListThreadsResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ResumeThreadRequest {
    pub workspace_id: String,
    pub thread_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ResumeThreadResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUserMessageRequest {
    pub workspace_id: String,
    pub thread_id: String,
    pub text: String,
    pub model: Option<String>,
    pub access_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct SendUserMessageResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccountRateLimitsResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct AccountRateLimitsRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ListModelsResponse(#[ts(type = "JsonValue")] pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListModelsRequest {
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetFullAutoRequest {
    pub workspace_id: String,
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub continue_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SetFullAutoResponse {
    pub workspace_id: String,
    pub enabled: bool,
    pub thread_id: Option<String>,
    pub continue_prompt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CurrentDirectory(pub String);

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectUnifiedAgentRequest {
    pub agent_id_str: String,
    pub workspace_path: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ConnectUnifiedAgentResponse {
    pub success: bool,
    pub session_id: String,
    pub agent_id: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectUnifiedAgentRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DisconnectUnifiedAgentResponse {
    pub success: bool,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartUnifiedSessionRequest {
    pub session_id: String,
    pub workspace_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StartUnifiedSessionResponse {
    pub success: bool,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUnifiedMessageRequest {
    pub session_id: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct SendUnifiedMessageResponse {
    pub success: bool,
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUnifiedConversationItemsRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUnifiedConversationItemsResponse {
    pub success: bool,
    pub session_id: String,
    pub items: Vec<UnifiedConversationItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUnifiedAgentStatusRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetUnifiedAgentStatusResponse {
    pub session_id: String,
    pub agent_id: Option<String>,
    pub connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct DsrsSignatureInfo {
    pub name: String,
    pub instruction: String,
    #[ts(type = "JsonValue")]
    pub input_fields: serde_json::Value,
    #[ts(type = "JsonValue")]
    pub output_fields: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ListDsrsSignaturesResponse {
    pub signatures: Vec<DsrsSignatureInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetDsrsSignatureRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GetDsrsSignatureResponse {
    pub signature: DsrsSignatureInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct UiPatch {
    pub op: String,
    pub path: String,
    #[ts(type = "JsonValue | null")]
    pub value: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
pub enum UiEvent {
    UiTreeReset {
        session_id: String,
        #[ts(type = "JsonValue")]
        tree: serde_json::Value,
    },
    UiPatch {
        session_id: String,
        patch: UiPatch,
    },
    UiDataUpdate {
        session_id: String,
        path: String,
        #[ts(type = "JsonValue")]
        value: serde_json::Value,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct GreetRequest {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct GreetResponse(pub String);

pub fn export_ts(path: &std::path::Path) -> Result<(), std::io::Error> {
    let mut buffer = String::from(
        "// This file was generated by ts-rs. Do not edit this file manually.\n\n",
    );

    let decls = [
        JsonValue::decl(),
        CodexDoctorRequest::decl(),
        CodexDoctorResponse::decl(),
        TestCodexConnectionRequest::decl(),
        TestCodexConnectionResponse::decl(),
        ConnectWorkspaceRequest::decl(),
        WorkspaceConnectionResponse::decl(),
        WorkspaceConnectionStatusResponse::decl(),
        DisconnectWorkspaceRequest::decl(),
        StartThreadRequest::decl(),
        StartThreadResponse::decl(),
        ListThreadsRequest::decl(),
        ListThreadsResponse::decl(),
        ResumeThreadRequest::decl(),
        ResumeThreadResponse::decl(),
        SendUserMessageRequest::decl(),
        SendUserMessageResponse::decl(),
        AccountRateLimitsRequest::decl(),
        AccountRateLimitsResponse::decl(),
        ListModelsRequest::decl(),
        ListModelsResponse::decl(),
        SetFullAutoRequest::decl(),
        SetFullAutoResponse::decl(),
        CurrentDirectory::decl(),
        ConnectUnifiedAgentRequest::decl(),
        ConnectUnifiedAgentResponse::decl(),
        DisconnectUnifiedAgentRequest::decl(),
        DisconnectUnifiedAgentResponse::decl(),
        StartUnifiedSessionRequest::decl(),
        StartUnifiedSessionResponse::decl(),
        SendUnifiedMessageRequest::decl(),
        SendUnifiedMessageResponse::decl(),
        GetUnifiedConversationItemsRequest::decl(),
        GetUnifiedConversationItemsResponse::decl(),
        GetUnifiedAgentStatusRequest::decl(),
        GetUnifiedAgentStatusResponse::decl(),
        DsrsSignatureInfo::decl(),
        ListDsrsSignaturesResponse::decl(),
        GetDsrsSignatureRequest::decl(),
        GetDsrsSignatureResponse::decl(),
        UiPatch::decl(),
        UiEvent::decl(),
        GreetRequest::decl(),
        GreetResponse::decl(),
        AgentId::decl(),
        UnifiedEvent::decl(),
        UnifiedConversationItem::decl(),
    ];

    for decl in decls {
        buffer.push_str("export ");
        buffer.push_str(&decl);
        if !decl.ends_with('\n') {
            buffer.push('\n');
        }
        buffer.push('\n');
    }

    std::fs::write(path, buffer)?;
    Ok(())
}
