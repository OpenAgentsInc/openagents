//! The frames both halves of the protocol read and write, typed.
//!
//! ACP is JSON-RPC 2.0, one object per line. A client sends requests
//! (`initialize`, `session/new`, `session/load`, `session/set_mode`,
//! `session/prompt`) and one notification (`session/cancel`); an agent
//! answers them, streams `session/update` notifications, and sends its own
//! requests back (`session/request_permission`). Every payload this crate
//! reads is deserialized into a type here; a field an agent leaves out or
//! adds is tolerated, and a payload of the wrong shape is an error, not a
//! guess.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The protocol version this crate speaks.
pub const PROTOCOL_VERSION: u64 = 1;

/// The method names this crate sends and answers.
pub mod method {
    pub const INITIALIZE: &str = "initialize";
    pub const SESSION_NEW: &str = "session/new";
    pub const SESSION_LOAD: &str = "session/load";
    pub const SESSION_LIST: &str = "session/list";
    pub const SESSION_SET_MODE: &str = "session/set_mode";
    pub const SESSION_PROMPT: &str = "session/prompt";
    pub const SESSION_CANCEL: &str = "session/cancel";
    pub const SESSION_UPDATE: &str = "session/update";
    pub const REQUEST_PERMISSION: &str = "session/request_permission";
}

/// The JSON-RPC error code for a method the receiver does not serve.
pub const METHOD_NOT_FOUND: i64 = -32601;
/// The JSON-RPC error code for a failure inside the receiver.
pub const INTERNAL_ERROR: i64 = -32603;

/// A JSON-RPC error, as the `error` member of a reply.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

/// The typed part of an error's `data` this crate reads.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
struct ErrorData {
    #[serde(default)]
    retryable: Option<bool>,
}

impl RpcError {
    #[must_use]
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    #[must_use]
    pub fn method_not_found(method: &str) -> Self {
        Self::new(METHOD_NOT_FOUND, format!("method not found: {method}"))
    }

    /// Whether the agent marked the refusal retryable with a boolean
    /// `data.retryable: true`. Nothing else counts.
    #[must_use]
    pub fn retryable(&self) -> bool {
        self.data
            .as_ref()
            .and_then(|data| serde_json::from_value::<ErrorData>(data.clone()).ok())
            .and_then(|data| data.retryable)
            == Some(true)
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// One incoming line, classified.
#[derive(Clone, Debug, PartialEq)]
pub enum Incoming {
    /// A reply to a request this side sent.
    Reply {
        id: Value,
        result: Result<Value, RpcError>,
    },
    /// A request the other side wants answered.
    Request {
        id: Value,
        method: String,
        params: Value,
    },
    /// A notification: a method and no id.
    Notification { method: String, params: Value },
}

/// The members of a JSON-RPC frame.
#[derive(Deserialize)]
struct Frame {
    #[serde(default)]
    id: Option<Value>,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<RpcError>,
}

/// Classify one line. A line that is not a JSON-RPC object is `None`: an
/// agent may write other text to its output, and a client skips it.
#[must_use]
pub fn classify(line: &str) -> Option<Incoming> {
    let trimmed = line.trim();
    if !trimmed.starts_with('{') {
        return None;
    }
    let frame: Frame = serde_json::from_str(trimmed).ok()?;
    let id = frame.id.filter(|id| !id.is_null());
    let params = frame.params.unwrap_or(Value::Null);
    match (frame.method, id) {
        (None, Some(id)) => Some(Incoming::Reply {
            id,
            result: match frame.error {
                Some(error) => Err(error),
                None => Ok(frame.result.unwrap_or(Value::Null)),
            },
        }),
        (Some(method), Some(id)) => Some(Incoming::Request { id, method, params }),
        (Some(method), None) => Some(Incoming::Notification { method, params }),
        (None, None) => None,
    }
}

/// A request line.
#[must_use]
pub fn request(id: u64, method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
}

/// A notification line.
#[must_use]
pub fn notification(method: &str, params: &Value) -> Value {
    json!({"jsonrpc": "2.0", "method": method, "params": params})
}

/// A reply line.
#[must_use]
pub fn reply(id: &Value, result: Result<Value, RpcError>) -> Value {
    match result {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(error) => json!({"jsonrpc": "2.0", "id": id, "error": error}),
    }
}

/// An authentication method an agent advertises.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AuthMethod {
    pub id: String,
}

/// What the agent can do, as `initialize` reports it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentCapabilities {
    #[serde(default)]
    pub load_session: bool,
}

/// The agent's name and version, when it reports them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AgentInfo {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
}

/// The `initialize` reply.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Initialized {
    pub protocol_version: u64,
    #[serde(default)]
    pub agent_capabilities: AgentCapabilities,
    #[serde(default)]
    pub auth_methods: Vec<AuthMethod>,
    #[serde(default)]
    pub agent_info: Option<AgentInfo>,
}

/// One session mode an agent offers.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Mode {
    pub id: String,
}

/// The session's modes, as `session/new` reports them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Modes {
    pub current_mode_id: String,
    #[serde(default)]
    pub available_modes: Vec<Mode>,
}

/// One selectable value of a configuration option.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct ConfigValue {
    pub value: String,
}

/// A session configuration option, such as the model.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigOption {
    pub id: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub current_value: Option<String>,
    #[serde(default)]
    pub options: Vec<ConfigValue>,
}

/// The `session/new` reply, and the part of `session/load`'s this crate
/// reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Opened {
    /// Absent from a `session/load` reply, which names no new session.
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub modes: Option<Modes>,
    #[serde(default)]
    pub config_options: Vec<ConfigOption>,
}

impl Opened {
    /// The model the session reports, from its `model` configuration option.
    #[must_use]
    pub fn model(&self) -> Option<&str> {
        self.config_options
            .iter()
            .find(|option| option.id == "model" || option.category.as_deref() == Some("model"))
            .and_then(|option| option.current_value.as_deref())
    }
}

/// Why a prompt turn ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
    /// A stop reason this crate does not know.
    #[serde(other)]
    Unknown,
}

impl StopReason {
    /// The wire name.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            StopReason::EndTurn => "end_turn",
            StopReason::MaxTokens => "max_tokens",
            StopReason::MaxTurnRequests => "max_turn_requests",
            StopReason::Refusal => "refusal",
            StopReason::Cancelled => "cancelled",
            StopReason::Unknown => "unknown",
        }
    }
}

/// A turn's token totals, as a `session/prompt` reply reports them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnUsage {
    #[serde(default)]
    pub total_tokens: Option<u64>,
    #[serde(default)]
    pub input_tokens: Option<u64>,
    #[serde(default)]
    pub output_tokens: Option<u64>,
    /// Input tokens read from the provider's cache, when the agent says
    /// (OpenCode's `cachedReadTokens`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cached_read_tokens: Option<u64>,
}

/// The `session/prompt` reply.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompted {
    pub stop_reason: StopReason,
    #[serde(default)]
    pub usage: Option<TurnUsage>,
}

/// A content block's text: `{"type": "text", "text": "..."}`. Other block
/// types carry no text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Content {
    #[serde(default)]
    pub text: Option<String>,
}

/// One tool call content block: `{"type": "content", "content": {...}}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct ToolContent {
    #[serde(default)]
    pub content: Option<Content>,
}

/// A tool call's `_meta`, where agents name the tool itself.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct ToolMeta {
    #[serde(default)]
    pub tool: Option<String>,
}

/// One step of an agent's plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanEntry {
    pub content: String,
    #[serde(default)]
    pub status: Option<String>,
}

/// A `usage_update`'s cost.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Cost {
    pub amount: f64,
    #[serde(default)]
    pub currency: Option<String>,
}

/// The vendor-namespaced token counts agents put in a `usage_update`'s
/// `_meta`: Devin's `cognition.ai/*` keys, and the plain names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct UsageMeta {
    #[serde(default, rename = "cognition.ai/inputTokens")]
    pub devin_input: Option<u64>,
    #[serde(default, rename = "cognition.ai/outputTokens")]
    pub devin_output: Option<u64>,
    #[serde(default, rename = "inputTokens")]
    pub input: Option<u64>,
    #[serde(default, rename = "outputTokens")]
    pub output: Option<u64>,
    /// Present when a subagent, not the root agent, spent the tokens.
    #[serde(default, rename = "cognition.ai/subagent_context")]
    pub devin_subagent: Option<Value>,
}

/// What a `usage_update` reports.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
pub struct Usage {
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    /// Tokens in the context so far.
    pub used: Option<u64>,
    /// The context window.
    pub size: Option<u64>,
    /// The cost so far, when the agent reports one in US dollars.
    pub cost_usd: Option<f64>,
    /// Whether a subagent, rather than the root agent, reported it.
    pub subagent: bool,
}

/// The `session/update` payloads, as the wire carries them.
#[derive(Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
enum Raw {
    AgentMessageChunk {
        content: Content,
    },
    AgentThoughtChunk {
        content: Content,
    },
    UserMessageChunk {
        content: Content,
    },
    #[serde(rename_all = "camelCase")]
    ToolCall {
        #[serde(default)]
        tool_call_id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        raw_input: Option<Value>,
        #[serde(default, rename = "_meta")]
        meta: Option<ToolMeta>,
    },
    #[serde(rename_all = "camelCase")]
    ToolCallUpdate {
        #[serde(default)]
        tool_call_id: String,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        title: Option<String>,
        #[serde(default)]
        content: Option<Vec<ToolContent>>,
        #[serde(default)]
        raw_output: Option<Value>,
    },
    Plan {
        #[serde(default)]
        entries: Vec<PlanEntry>,
    },
    UsageUpdate {
        #[serde(default)]
        used: Option<u64>,
        #[serde(default)]
        size: Option<u64>,
        #[serde(default)]
        cost: Option<Cost>,
        #[serde(default, rename = "_meta")]
        meta: Option<UsageMeta>,
    },
    #[serde(rename_all = "camelCase")]
    CurrentModeUpdate {
        current_mode_id: String,
    },
    SessionInfoUpdate {
        #[serde(default)]
        title: Option<String>,
    },
    #[serde(other)]
    Other,
}

/// One `session/update`, typed.
#[derive(Clone, Debug, PartialEq)]
pub enum Update {
    /// A piece of the answer, as it is written.
    AgentText(String),
    /// A piece of the agent's reasoning.
    Thought(String),
    /// A piece of a user message, as a loaded session replays it.
    UserText(String),
    /// A tool the agent started.
    ToolCall {
        id: String,
        title: String,
        kind: String,
        status: String,
        raw_input: Value,
        /// The tool's own name, from `_meta.tool`.
        tool: Option<String>,
    },
    /// A change on a tool call.
    ToolCallUpdate {
        id: String,
        status: Option<String>,
        title: Option<String>,
        /// The update's text content blocks joined with newlines, or the
        /// `rawOutput` string when there are none.
        text: Option<String>,
    },
    /// The agent's plan, as it stands.
    Plan(Vec<PlanEntry>),
    /// Tokens and cost so far.
    Usage(Usage),
    /// The session's mode changed.
    CurrentMode(String),
    /// The session's title changed.
    Title(String),
    /// A `sessionUpdate` this crate does not type.
    Other,
}

/// The `params` of a `session/update` notification.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateParams {
    #[serde(default)]
    session_id: Option<String>,
    update: Raw,
}

/// Type the `params` of a `session/update` notification: the session it
/// names, when it names one, and the update. `None` for a payload of the
/// wrong shape.
#[must_use]
pub fn parse_update(params: &Value) -> Option<(Option<String>, Update)> {
    let params: UpdateParams = serde_json::from_value(params.clone()).ok()?;
    let update = match params.update {
        Raw::AgentMessageChunk { content } => Update::AgentText(content.text?),
        Raw::AgentThoughtChunk { content } => Update::Thought(content.text?),
        Raw::UserMessageChunk { content } => Update::UserText(content.text?),
        Raw::ToolCall {
            tool_call_id,
            title,
            kind,
            status,
            raw_input,
            meta,
        } => Update::ToolCall {
            id: tool_call_id,
            title,
            kind: kind.unwrap_or_else(|| "other".into()),
            status: status.unwrap_or_else(|| "pending".into()),
            raw_input: raw_input.unwrap_or(Value::Null),
            tool: meta.and_then(|meta| meta.tool),
        },
        Raw::ToolCallUpdate {
            tool_call_id,
            status,
            title,
            content,
            raw_output,
        } => {
            let blocks: Vec<String> = content
                .unwrap_or_default()
                .into_iter()
                .filter_map(|block| block.content.and_then(|content| content.text))
                .collect();
            let text = if blocks.is_empty() {
                raw_output.and_then(|output| output.as_str().map(str::to_owned))
            } else {
                Some(blocks.join("\n"))
            };
            Update::ToolCallUpdate {
                id: tool_call_id,
                status,
                title,
                text,
            }
        }
        Raw::Plan { entries } => Update::Plan(entries),
        Raw::UsageUpdate {
            used,
            size,
            cost,
            meta,
        } => {
            let meta = meta.unwrap_or_default();
            Update::Usage(Usage {
                input_tokens: meta.devin_input.or(meta.input),
                output_tokens: meta.devin_output.or(meta.output),
                used,
                size,
                cost_usd: cost
                    .filter(|cost| cost.currency.as_deref().is_none_or(|c| c == "USD"))
                    .map(|cost| cost.amount),
                subagent: meta.devin_subagent.is_some(),
            })
        }
        Raw::CurrentModeUpdate { current_mode_id } => Update::CurrentMode(current_mode_id),
        Raw::SessionInfoUpdate { title } => Update::Title(title.unwrap_or_default()),
        Raw::Other => Update::Other,
    };
    Some((params.session_id, update))
}

/// One option a permission request offers.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    #[serde(default)]
    pub kind: Option<String>,
}

impl PermissionOption {
    /// Whether the option lets the tool run: `allow_once`, `allow_always`.
    #[must_use]
    pub fn allows(&self) -> bool {
        matches!(self.kind.as_deref(), Some("allow_once" | "allow_always"))
    }

    /// Whether the option refuses the tool: `reject_once`, `reject_always`.
    #[must_use]
    pub fn rejects(&self) -> bool {
        matches!(self.kind.as_deref(), Some("reject_once" | "reject_always"))
    }
}

/// The tool call a permission request names.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionTool {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub raw_input: Option<Value>,
}

/// A `session/request_permission` request's parameters.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRequest {
    #[serde(default)]
    pub tool_call: PermissionTool,
    #[serde(default)]
    pub options: Vec<PermissionOption>,
}

impl PermissionRequest {
    /// The first option that allows the tool.
    #[must_use]
    pub fn allow(&self) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.allows())
            .map(|option| option.option_id.as_str())
    }

    /// The first option that refuses it.
    #[must_use]
    pub fn reject(&self) -> Option<&str> {
        self.options
            .iter()
            .find(|option| option.rejects())
            .map(|option| option.option_id.as_str())
    }
}

/// The answer a client gives to `session/request_permission`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PermissionAnswer {
    Selected(String),
    Cancelled,
}

impl PermissionAnswer {
    /// The reply's `result`.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            PermissionAnswer::Selected(option) => {
                json!({"outcome": {"outcome": "selected", "optionId": option}})
            }
            PermissionAnswer::Cancelled => json!({"outcome": {"outcome": "cancelled"}}),
        }
    }
}

/// One session a `session/list` reply names.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedSession {
    pub session_id: String,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub updated_at: Option<String>,
    /// The `_meta` the session was opened with, and the agent's own keys.
    #[serde(default, rename = "_meta")]
    pub meta: Option<serde_json::Map<String, Value>>,
}

/// The `session/list` reply.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Listed {
    #[serde(default)]
    pub sessions: Vec<ListedSession>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_classify_as_reply_request_or_notification() {
        assert_eq!(
            classify(r#"{"jsonrpc":"2.0","id":3,"result":{"ok":true}}"#),
            Some(Incoming::Reply {
                id: json!(3),
                result: Ok(json!({"ok": true}))
            })
        );
        assert!(matches!(
            classify(r#"{"jsonrpc":"2.0","id":4,"error":{"code":-32601,"message":"nope"}}"#),
            Some(Incoming::Reply {
                result: Err(RpcError {
                    code: METHOD_NOT_FOUND,
                    ..
                }),
                ..
            })
        ));
        assert!(matches!(
            classify(
                r#"{"jsonrpc":"2.0","id":"p1","method":"session/request_permission","params":{}}"#
            ),
            Some(Incoming::Request { .. })
        ));
        assert!(matches!(
            classify(r#"{"jsonrpc":"2.0","method":"session/update","params":{"a":1}}"#),
            Some(Incoming::Notification { .. })
        ));
        assert_eq!(classify("plain log line"), None);
        assert_eq!(classify("   "), None);
        assert_eq!(classify("[1,2]"), None);
        assert_eq!(classify("{not json"), None);
    }

    #[test]
    fn a_retryable_refusal_needs_a_boolean() {
        let mut error = RpcError::new(-32000, "busy");
        assert!(!error.retryable());
        error.data = Some(json!({"retryable": "true"}));
        assert!(!error.retryable());
        error.data = Some(json!({"retryable": true}));
        assert!(error.retryable());
    }

    #[test]
    fn updates_are_typed() {
        let chunk = json!({"sessionId":"s","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"hi"}}});
        assert_eq!(
            parse_update(&chunk),
            Some((Some("s".into()), Update::AgentText("hi".into())))
        );
        let image =
            json!({"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"image"}}});
        assert_eq!(parse_update(&image), None);
        let call = json!({"update":{"sessionUpdate":"tool_call","toolCallId":"t1","title":"Read README.md","kind":"read","rawInput":{"path":"README.md"},"_meta":{"tool":"read_file"}}});
        assert_eq!(
            parse_update(&call).map(|(_, update)| update),
            Some(Update::ToolCall {
                id: "t1".into(),
                title: "Read README.md".into(),
                kind: "read".into(),
                status: "pending".into(),
                raw_input: json!({"path": "README.md"}),
                tool: Some("read_file".into()),
            })
        );
        let done = json!({"update":{"sessionUpdate":"tool_call_update","toolCallId":"t1","status":"completed","content":[{"type":"content","content":{"type":"text","text":"line 1"}},{"type":"content","content":{"type":"text","text":"line 2"}}]}});
        assert_eq!(
            parse_update(&done).map(|(_, update)| update),
            Some(Update::ToolCallUpdate {
                id: "t1".into(),
                status: Some("completed".into()),
                title: None,
                text: Some("line 1\nline 2".into()),
            })
        );
        let raw = json!({"update":{"sessionUpdate":"tool_call_update","toolCallId":"t2","rawOutput":"out"}});
        assert!(matches!(
            parse_update(&raw),
            Some((_, Update::ToolCallUpdate { text: Some(ref text), .. })) if text == "out"
        ));
        let unknown =
            json!({"update":{"sessionUpdate":"available_commands_update","availableCommands":[]}});
        assert_eq!(parse_update(&unknown).map(|(_, u)| u), Some(Update::Other));
        assert_eq!(parse_update(&json!({})), None);
    }

    #[test]
    fn usage_reads_devin_vendor_keys_and_a_dollar_cost() {
        let devin = json!({"update":{"sessionUpdate":"usage_update","used":10992,"size":262000,"_meta":{"cognition.ai/inputTokens":10934,"cognition.ai/outputTokens":58}}});
        let Some((_, Update::Usage(usage))) = parse_update(&devin) else {
            panic!("not usage");
        };
        assert_eq!(usage.input_tokens, Some(10934));
        assert_eq!(usage.output_tokens, Some(58));
        assert_eq!(usage.size, Some(262_000));
        assert!(!usage.subagent);
        let sub = json!({"update":{"sessionUpdate":"usage_update","_meta":{"cognition.ai/inputTokens":1,"cognition.ai/subagent_context":{"parentAgentId":"root"}}}});
        assert!(matches!(
            parse_update(&sub),
            Some((_, Update::Usage(Usage { subagent: true, .. })))
        ));
        let priced = json!({"update":{"sessionUpdate":"usage_update","cost":{"amount":0.25,"currency":"USD"}}});
        assert!(
            matches!(parse_update(&priced), Some((_, Update::Usage(Usage { cost_usd: Some(c), .. }))) if (c - 0.25).abs() < 1e-9)
        );
        let euro = json!({"update":{"sessionUpdate":"usage_update","cost":{"amount":1.0,"currency":"EUR"}}});
        assert!(matches!(
            parse_update(&euro),
            Some((_, Update::Usage(Usage { cost_usd: None, .. })))
        ));
    }

    #[test]
    fn a_prompt_reply_and_an_opened_session_are_typed() {
        let reply: Prompted = serde_json::from_value(json!({"stopReason":"end_turn","usage":{"totalTokens":10992,"inputTokens":10934,"outputTokens":58},"_meta":{}})).unwrap();
        assert_eq!(reply.stop_reason, StopReason::EndTurn);
        assert_eq!(reply.usage.unwrap().output_tokens, Some(58));
        let future: Prompted = serde_json::from_value(json!({"stopReason":"paused"})).unwrap();
        assert_eq!(future.stop_reason, StopReason::Unknown);
        assert!(serde_json::from_value::<Prompted>(json!({})).is_err());
        let opened: Opened = serde_json::from_value(json!({"sessionId":"cookie-marmot","modes":{"currentModeId":"accept-edits","availableModes":[{"id":"bypass","name":"Bypass Permissions"}]},"configOptions":[{"id":"mode","category":"mode","currentValue":"accept-edits"},{"id":"model","category":"model","currentValue":"swe-2-high","options":[{"value":"swe-2-high"}]}]})).unwrap();
        assert_eq!(opened.model(), Some("swe-2-high"));
        assert_eq!(opened.modes.unwrap().available_modes[0].id, "bypass");
    }

    #[test]
    fn permission_options_are_read_by_kind() {
        let request: PermissionRequest = serde_json::from_value(json!({"toolCall":{"kind":"execute","title":"run ls","rawInput":{"command":"ls"}},"options":[
            {"optionId":"deny-1","kind":"reject_once"},
            {"optionId":"ok-1","kind":"allow_once"}
        ]})).unwrap();
        assert_eq!(request.allow(), Some("ok-1"));
        assert_eq!(request.reject(), Some("deny-1"));
        assert_eq!(request.tool_call.kind.as_deref(), Some("execute"));
        assert_eq!(
            PermissionAnswer::Selected("ok-1".into()).to_value()["outcome"]["optionId"],
            "ok-1"
        );
    }
}
