//! SDK-hosted (in-process) MCP servers: the Rust counterpart of the TS
//! SDK's `tool()` and `createSdkMcpServer()`.
//!
//! An [`SdkMcpServer`] holds tools whose handlers run inside the host
//! process. Registering one with [`QueryOptions::sdk_mcp_server`] names it
//! in the `initialize` request's `sdkMcpServers` field (it never goes into
//! `--mcp-config`, as in the TS SDK). The CLI then reaches the server with
//! `mcp_message` control requests, each carrying one MCP JSON-RPC message;
//! the reply goes back under `mcp_response`.
//!
//! The model sees each tool as `mcp__<server>__<tool>`
//! ([`SdkMcpServer::tool_name`]); use that name in `allowed_tools`.
//!
//! The CLI, not this crate, enforces the MCP description limits of
//! Claude Code 2.1.296: tool descriptions sent up front and server
//! instructions are cut at [`MCP_DESCRIPTION_LIMIT`] characters, and
//! descriptions loaded through tool search at
//! [`MCP_DEFERRED_DESCRIPTION_LIMIT`].
//!
//! [`QueryOptions::sdk_mcp_server`]: crate::QueryOptions::sdk_mcp_server

use crate::error::Result;
use futures::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::fmt;
use std::future::Future;
use std::sync::Arc;

/// The newest MCP protocol version this server speaks (the TS SDK's
/// client requests the same one).
pub const MCP_PROTOCOL_VERSION: &str = "2025-11-25";

/// Protocol versions the server accepts from a client's `initialize`.
pub const SUPPORTED_MCP_PROTOCOL_VERSIONS: &[&str] = &[
    "2025-11-25",
    "2025-06-18",
    "2025-03-26",
    "2024-11-05",
    "2024-10-07",
];

/// Characters the CLI keeps of a tool description sent up front, and of
/// server instructions (0.3.296).
pub const MCP_DESCRIPTION_LIMIT: usize = 4_096;

/// Characters the CLI keeps of a tool description loaded through tool
/// search (0.3.296).
pub const MCP_DEFERRED_DESCRIPTION_LIMIT: usize = 16_384;

const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// The `mcp__<server>__<tool>` name the model and `allowed_tools` use.
pub fn mcp_tool_name(server: &str, tool: &str) -> String {
    format!("mcp__{server}__{tool}")
}

/// One content block of a tool result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ToolContent {
    /// Plain text.
    Text { text: String },
    /// A base64-encoded image.
    Image {
        /// Base64 image bytes.
        data: String,
        /// For example `image/png`.
        #[serde(rename = "mimeType")]
        mime_type: String,
    },
}

/// What a tool handler returns (MCP `CallToolResult`).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ToolResult {
    /// Content blocks shown to the model.
    pub content: Vec<ToolContent>,
    /// The call failed; the model sees the content as an error.
    pub is_error: bool,
    /// Optional `structuredContent`.
    pub structured_content: Option<Value>,
}

impl ToolResult {
    /// A result with one text block.
    pub fn text(text: impl Into<String>) -> Self {
        Self::default().with_text(text)
    }

    /// An error result with one text block.
    pub fn error(text: impl Into<String>) -> Self {
        let mut result = Self::text(text);
        result.is_error = true;
        result
    }

    /// A result with one base64 image block.
    pub fn image(data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        Self::default().with_image(data, mime_type)
    }

    /// Append a text block.
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.content.push(ToolContent::Text { text: text.into() });
        self
    }

    /// Append a base64 image block.
    pub fn with_image(mut self, data: impl Into<String>, mime_type: impl Into<String>) -> Self {
        self.content.push(ToolContent::Image {
            data: data.into(),
            mime_type: mime_type.into(),
        });
        self
    }

    /// Set `structuredContent`.
    pub fn with_structured_content(mut self, value: Value) -> Self {
        self.structured_content = Some(value);
        self
    }

    /// Mark the result as an error.
    pub fn as_error(mut self) -> Self {
        self.is_error = true;
        self
    }

    /// The MCP `CallToolResult` JSON.
    pub fn to_value(&self) -> Value {
        let mut out = json!({ "content": self.content });
        if self.is_error {
            out["isError"] = Value::Bool(true);
        }
        if let Some(structured) = &self.structured_content {
            out["structuredContent"] = structured.clone();
        }
        out
    }
}

type Handler = Arc<dyn Fn(Value) -> BoxFuture<'static, Result<ToolResult>> + Send + Sync>;

/// One tool of an [`SdkMcpServer`] (TS `SdkMcpToolDefinition`).
#[derive(Clone)]
pub struct SdkMcpTool {
    /// Tool name, unique within the server.
    pub name: String,
    /// What the tool does, shown to the model.
    pub description: String,
    /// JSON Schema for the arguments. The CLI validates calls against it;
    /// the handler gets the arguments object as sent.
    pub input_schema: Value,
    /// MCP tool annotations (`readOnlyHint`, `title`, ...).
    pub annotations: Option<Value>,
    /// Always include this tool in the prompt instead of deferring it
    /// behind tool search (`_meta["anthropic/alwaysLoad"]`).
    pub always_load: bool,
    /// Hint for tool search (`_meta["anthropic/searchHint"]`).
    pub search_hint: Option<String>,
    handler: Handler,
}

impl SdkMcpTool {
    /// A tool with an async handler (TS `tool()`). The handler gets the
    /// call's `arguments` object. An `Err` becomes an `isError` result
    /// carrying the error text, as the TS `McpServer` does for a throw.
    pub fn new<F, Fut>(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult>> + Send + 'static,
    {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
            annotations: None,
            always_load: false,
            search_hint: None,
            handler: Arc::new(move |args| Box::pin(handler(args))),
        }
    }

    /// Set MCP tool annotations.
    pub fn annotations(mut self, annotations: Value) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Never defer this tool behind tool search.
    pub fn always_load(mut self, always_load: bool) -> Self {
        self.always_load = always_load;
        self
    }

    /// Set the tool-search hint.
    pub fn search_hint(mut self, hint: impl Into<String>) -> Self {
        self.search_hint = Some(hint.into());
        self
    }

    /// The tool's `tools/list` entry.
    fn listing(&self, server_always_load: bool) -> Value {
        let mut entry = json!({
            "name": self.name,
            "description": self.description,
            "inputSchema": self.input_schema,
        });
        if let Some(annotations) = &self.annotations {
            entry["annotations"] = annotations.clone();
        }
        let mut meta = Map::new();
        if let Some(hint) = &self.search_hint {
            meta.insert("anthropic/searchHint".into(), Value::String(hint.clone()));
        }
        if self.always_load || server_always_load {
            meta.insert("anthropic/alwaysLoad".into(), Value::Bool(true));
        }
        if !meta.is_empty() {
            entry["_meta"] = Value::Object(meta);
        }
        entry
    }
}

impl fmt::Debug for SdkMcpTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SdkMcpTool")
            .field("name", &self.name)
            .field("description", &self.description)
            .field("input_schema", &self.input_schema)
            .finish_non_exhaustive()
    }
}

/// An in-process MCP server (TS `createSdkMcpServer`).
///
/// ```rust
/// use claude_agent_sdk::{SdkMcpServer, ToolResult};
/// use serde_json::json;
///
/// let calc = SdkMcpServer::new("calc", "1.0.0").tool(
///     "add",
///     "Add two numbers",
///     json!({
///         "type": "object",
///         "properties": {"a": {"type": "number"}, "b": {"type": "number"}},
///         "required": ["a", "b"]
///     }),
///     |args| async move {
///         let a = args["a"].as_f64().unwrap_or_default();
///         let b = args["b"].as_f64().unwrap_or_default();
///         Ok(ToolResult::text((a + b).to_string()))
///     },
/// );
/// assert_eq!(calc.tool_name("add"), "mcp__calc__add");
/// ```
#[derive(Debug, Clone)]
pub struct SdkMcpServer {
    name: String,
    version: String,
    instructions: Option<String>,
    always_load: bool,
    timeout_ms: Option<u64>,
    tools: Vec<SdkMcpTool>,
}

impl SdkMcpServer {
    /// A server with no tools. `name` is the key the CLI and the
    /// `mcp__<name>__<tool>` names use.
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            instructions: None,
            always_load: false,
            timeout_ms: None,
            tools: Vec::new(),
        }
    }

    /// Add a tool with an async handler. A tool with the same name is
    /// replaced.
    pub fn tool<F, Fut>(
        self,
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: Value,
        handler: F,
    ) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult>> + Send + 'static,
    {
        self.add_tool(SdkMcpTool::new(name, description, input_schema, handler))
    }

    /// Add a prepared [`SdkMcpTool`]. A tool with the same name is
    /// replaced.
    pub fn add_tool(mut self, tool: SdkMcpTool) -> Self {
        self.tools.retain(|existing| existing.name != tool.name);
        self.tools.push(tool);
        self
    }

    /// Server instructions returned from `initialize`.
    pub fn instructions(mut self, instructions: impl Into<String>) -> Self {
        self.instructions = Some(instructions.into());
        self
    }

    /// Never defer any of this server's tools behind tool search.
    pub fn always_load(mut self, always_load: bool) -> Self {
        self.always_load = always_load;
        self
    }

    /// Per-server tool-call timeout in milliseconds, sent in
    /// `initialize.sdkMcpServerConfigs`. The CLI ignores values below
    /// 1000.
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// The server name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The server version.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The tools, in registration order.
    pub fn tools(&self) -> &[SdkMcpTool] {
        &self.tools
    }

    /// The configured tool-call timeout.
    pub fn timeout(&self) -> Option<u64> {
        self.timeout_ms
    }

    /// `mcp__<server>__<tool>` for one tool of this server.
    pub fn tool_name(&self, tool: &str) -> String {
        mcp_tool_name(&self.name, tool)
    }

    /// `mcp__<server>__<tool>` for every tool, for `allowed_tools`.
    pub fn allowed_tool_names(&self) -> Vec<String> {
        self.tools.iter().map(|t| self.tool_name(&t.name)).collect()
    }

    /// The `initialize` result for a client requesting `requested`.
    pub fn initialize_result(&self, requested: Option<&str>) -> Value {
        let version = requested
            .filter(|v| SUPPORTED_MCP_PROTOCOL_VERSIONS.contains(v))
            .unwrap_or(MCP_PROTOCOL_VERSION);
        let mut result = json!({
            "protocolVersion": version,
            "capabilities": { "tools": { "listChanged": false } },
            "serverInfo": { "name": self.name, "version": self.version },
        });
        if let Some(instructions) = &self.instructions {
            result["instructions"] = Value::String(instructions.clone());
        }
        result
    }

    /// The `tools/list` result.
    pub fn tools_list_result(&self) -> Value {
        let tools: Vec<Value> = self
            .tools
            .iter()
            .map(|t| t.listing(self.always_load))
            .collect();
        json!({ "tools": tools })
    }

    /// Answer one MCP JSON-RPC message from the CLI. A request (it has a
    /// `method` and a non-null `id`) gets a JSON-RPC response; a
    /// notification or a response gets the empty acknowledgement the TS
    /// SDK sends (`{"jsonrpc":"2.0","result":{},"id":0}`).
    pub async fn handle_message(&self, message: &Value) -> Value {
        let id = message.get("id").filter(|id| !id.is_null());
        let (Some(method), Some(id)) = (message.get("method").and_then(Value::as_str), id) else {
            return json!({ "jsonrpc": "2.0", "result": {}, "id": 0 });
        };
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let outcome = match method {
            "initialize" => {
                Ok(self.initialize_result(params.get("protocolVersion").and_then(Value::as_str)))
            }
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools_list_result()),
            "tools/call" => self.call_tool(&params).await,
            other => Err((METHOD_NOT_FOUND, format!("Method not found: {other}"))),
        };
        match outcome {
            Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
            Err((code, message)) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": code, "message": message },
            }),
        }
    }

    async fn call_tool(&self, params: &Value) -> std::result::Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or((INVALID_PARAMS, "tools/call needs a tool name".to_string()))?;
        let tool = self
            .tools
            .iter()
            .find(|t| t.name == name)
            .ok_or_else(|| (INVALID_PARAMS, format!("Tool {name} not found")))?;
        let args = match params.get("arguments") {
            None | Some(Value::Null) => json!({}),
            Some(args) => args.clone(),
        };
        let result = match (tool.handler)(args).await {
            Ok(result) => result,
            Err(error) => ToolResult::error(error.to_string()),
        };
        Ok(result.to_value())
    }

    /// The `initialize.sdkMcpServerManifests` entry: the server's own
    /// `initialize` and `tools/list` results, so the CLI can skip those
    /// round trips.
    pub fn manifest(&self) -> Value {
        json!({
            "initializeResult": self.initialize_result(None),
            "toolsListResult": self.tools_list_result(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Error;

    fn calc() -> SdkMcpServer {
        SdkMcpServer::new("calc", "1.2.3")
            .instructions("Use add for sums.")
            .tool(
                "add",
                "Add two numbers",
                json!({
                    "type": "object",
                    "properties": {"a": {"type": "number"}, "b": {"type": "number"}},
                    "required": ["a", "b"]
                }),
                |args| async move {
                    let a = args["a"].as_f64().unwrap_or_default();
                    let b = args["b"].as_f64().unwrap_or_default();
                    Ok(ToolResult::text(format!("{}", a + b)))
                },
            )
            .add_tool(
                SdkMcpTool::new(
                    "fail",
                    "Always fails",
                    json!({"type": "object"}),
                    |_| async { Err(Error::McpError("disk on fire".into())) },
                )
                .always_load(true)
                .search_hint("failure")
                .annotations(json!({"readOnlyHint": true})),
            )
            .tool("pixel", "One pixel", json!({"type": "object"}), |_| async {
                Ok(ToolResult::image("iVBORw0KGgo=", "image/png").with_text("a pixel"))
            })
    }

    fn request(id: Value, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    #[tokio::test]
    async fn initialize_echoes_a_supported_version_and_names_the_server() {
        let server = calc();
        let reply = server
            .handle_message(&request(
                json!(0),
                "initialize",
                json!({"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "claude-code", "version": "2.1.296"}}),
            ))
            .await;
        assert_eq!(reply["jsonrpc"], "2.0");
        assert_eq!(reply["id"], 0);
        assert_eq!(reply["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(reply["result"]["serverInfo"]["name"], "calc");
        assert_eq!(reply["result"]["serverInfo"]["version"], "1.2.3");
        assert!(reply["result"]["capabilities"]["tools"].is_object());
        assert_eq!(reply["result"]["instructions"], "Use add for sums.");

        let unknown = server
            .handle_message(&request(
                json!("x"),
                "initialize",
                json!({"protocolVersion": "1999-01-01"}),
            ))
            .await;
        assert_eq!(unknown["id"], "x");
        assert_eq!(unknown["result"]["protocolVersion"], MCP_PROTOCOL_VERSION);
    }

    #[tokio::test]
    async fn tools_list_carries_schema_annotations_and_meta() {
        let reply = calc()
            .handle_message(&request(json!(1), "tools/list", json!({})))
            .await;
        let tools = reply["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 3);
        assert_eq!(tools[0]["name"], "add");
        assert_eq!(tools[0]["description"], "Add two numbers");
        assert_eq!(tools[0]["inputSchema"]["required"][1], "b");
        assert!(tools[0].get("_meta").is_none());
        assert_eq!(tools[1]["_meta"]["anthropic/alwaysLoad"], true);
        assert_eq!(tools[1]["_meta"]["anthropic/searchHint"], "failure");
        assert_eq!(tools[1]["annotations"]["readOnlyHint"], true);

        let all = calc()
            .always_load(true)
            .handle_message(&request(json!(2), "tools/list", Value::Null))
            .await;
        assert_eq!(
            all["result"]["tools"][0]["_meta"]["anthropic/alwaysLoad"],
            true
        );
    }

    #[tokio::test]
    async fn tools_call_runs_the_handler() {
        let reply = calc()
            .handle_message(&request(
                json!(7),
                "tools/call",
                json!({"name": "add", "arguments": {"a": 2, "b": 40}}),
            ))
            .await;
        assert_eq!(reply["id"], 7);
        assert_eq!(
            reply["result"],
            json!({"content": [{"type": "text", "text": "42"}]})
        );
    }

    #[tokio::test]
    async fn tools_call_image_content_and_handler_errors() {
        let server = calc();
        let image = server
            .handle_message(&request(json!(8), "tools/call", json!({"name": "pixel"})))
            .await;
        assert_eq!(
            image["result"]["content"],
            json!([
                {"type": "image", "data": "iVBORw0KGgo=", "mimeType": "image/png"},
                {"type": "text", "text": "a pixel"}
            ])
        );
        let failed = server
            .handle_message(&request(
                json!(9),
                "tools/call",
                json!({"name": "fail", "arguments": {}}),
            ))
            .await;
        assert_eq!(failed["result"]["isError"], true);
        assert!(
            failed["result"]["content"][0]["text"]
                .as_str()
                .unwrap()
                .contains("disk on fire")
        );
    }

    #[tokio::test]
    async fn unknown_tool_and_method_are_json_rpc_errors() {
        let server = calc();
        let tool = server
            .handle_message(&request(json!(3), "tools/call", json!({"name": "mul"})))
            .await;
        assert_eq!(tool["id"], 3);
        assert_eq!(tool["error"]["code"], INVALID_PARAMS);
        assert!(tool["error"]["message"].as_str().unwrap().contains("mul"));
        assert!(tool.get("result").is_none());

        let method = server
            .handle_message(&request(json!(4), "resources/list", json!({})))
            .await;
        assert_eq!(method["error"]["code"], METHOD_NOT_FOUND);

        let ping = server
            .handle_message(&request(json!(5), "ping", Value::Null))
            .await;
        assert_eq!(ping["result"], json!({}));
    }

    #[tokio::test]
    async fn notifications_and_responses_get_the_empty_ack() {
        let server = calc();
        let ack = json!({"jsonrpc": "2.0", "result": {}, "id": 0});
        let notification = json!({"jsonrpc": "2.0", "method": "notifications/initialized"});
        assert_eq!(server.handle_message(&notification).await, ack);
        let null_id = json!({"jsonrpc": "2.0", "id": null, "method": "tools/list"});
        assert_eq!(server.handle_message(&null_id).await, ack);
        let response = json!({"jsonrpc": "2.0", "id": 1, "result": {}});
        assert_eq!(server.handle_message(&response).await, ack);
    }

    #[test]
    fn names_manifest_and_replacement() {
        let server = calc().tool("add", "Add again", json!({"type": "object"}), |_| async {
            Ok(ToolResult::text("0"))
        });
        assert_eq!(server.tools().len(), 3);
        assert_eq!(server.tools()[2].description, "Add again");
        assert_eq!(
            server.allowed_tool_names(),
            vec!["mcp__calc__fail", "mcp__calc__pixel", "mcp__calc__add"]
        );
        let manifest = server.manifest();
        assert_eq!(
            manifest["initializeResult"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(
            manifest["toolsListResult"]["tools"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        assert_eq!(
            ToolResult::error("no").to_value(),
            json!({"content": [{"type": "text", "text": "no"}], "isError": true})
        );
    }
}
