//! One tool server for every harness that takes MCP
//! (`docs/coder/design/coder-sdk.md`, "Everything else", item 6).
//!
//! A [`ToolServer`] is declared once: its tools, their schemas and bodies.
//! It reaches a harness two ways:
//!
//! - **in process** for Claude Code: [`ToolServer::in_process`] gives the
//!   `claude_agent_sdk::SdkMcpServer` a session hands the CLI;
//! - **over stdio** for every other harness that takes MCP (Codex,
//!   OpenCode): [`serve`] answers MCP JSON-RPC, one message per line.
//!
//! **Hooks.** A call can pass through a session's [`Runner`] first
//! ([`ToolServer::gated`]): `PreToolUse` before the tool runs (a deny or an
//! ask stops it, a rewrite changes its input, and the floors judge the
//! final input), then `PostToolUse` with what it returned (context is added
//! to the reply). The tool is named as Claude Code names it,
//! `mcp__<server>__<tool>`, so one matcher fits every harness. Claude Code
//! already fires its own `PreToolUse`/`PostToolUse` hooks on these tools,
//! so its in-process server is left ungated and no hook fires twice; a
//! stdio harness whose hooks don't reach MCP calls gets the gated server.

use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use claude_agent_sdk::{SdkMcpServer, SdkMcpTool, ToolResult, mcp_tool_name};
use futures::future::BoxFuture;
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncWrite, AsyncWriteExt};

use super::compose::{Applied, Runner};
use super::events::{Call, Event};
use super::outcome::Permission;

/// A tool's body: the arguments in, an MCP result or an error out.
pub type ToolRun =
    Arc<dyn Fn(Value) -> BoxFuture<'static, Result<ToolResult, String>> + Send + Sync>;

/// One tool.
#[derive(Clone)]
pub struct Tool {
    pub name: String,
    pub description: String,
    /// JSON Schema of the arguments.
    pub schema: Value,
    /// Always in the prompt, never behind tool search.
    pub always_load: bool,
    pub annotations: Option<Value>,
    run: ToolRun,
}

impl std::fmt::Debug for Tool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tool")
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl Tool {
    pub fn new<F, Fut>(name: &str, description: &str, schema: Value, run: F) -> Self
    where
        F: Fn(Value) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<ToolResult, String>> + Send + 'static,
    {
        Tool {
            name: name.to_string(),
            description: description.to_string(),
            schema,
            always_load: false,
            annotations: None,
            run: Arc::new(move |input| Box::pin(run(input))),
        }
    }

    #[must_use]
    pub fn always_load(mut self, always_load: bool) -> Self {
        self.always_load = always_load;
        self
    }

    #[must_use]
    pub fn annotations(mut self, annotations: Value) -> Self {
        self.annotations = Some(annotations);
        self
    }

    /// Runs the tool's body on `input`, with no hooks.
    pub fn invoke(&self, input: Value) -> BoxFuture<'static, Result<ToolResult, String>> {
        (self.run)(input)
    }
}

/// A set of tools under one server name.
#[derive(Clone)]
pub struct ToolServer {
    pub name: String,
    pub version: String,
    pub instructions: Option<String>,
    /// How long the harness should wait for one call.
    pub timeout_ms: Option<u64>,
    tools: Vec<Tool>,
    gate: Option<Arc<Mutex<Runner>>>,
    fired: Arc<Mutex<Vec<Applied>>>,
}

impl std::fmt::Debug for ToolServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolServer")
            .field("name", &self.name)
            .field("tools", &self.names())
            .field("gated", &self.gate.is_some())
            .finish_non_exhaustive()
    }
}

impl ToolServer {
    #[must_use]
    pub fn new(name: &str, version: &str) -> Self {
        ToolServer {
            name: name.to_string(),
            version: version.to_string(),
            instructions: None,
            timeout_ms: None,
            tools: Vec::new(),
            gate: None,
            fired: Arc::default(),
        }
    }

    #[must_use]
    pub fn instructions(mut self, instructions: &str) -> Self {
        self.instructions = Some(instructions.to_string());
        self
    }

    #[must_use]
    pub fn timeout_ms(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = Some(timeout_ms);
        self
    }

    /// Adds a tool; a second tool with the same name replaces the first.
    #[must_use]
    pub fn add(mut self, tool: Tool) -> Self {
        self.tools.retain(|other| other.name != tool.name);
        self.tools.push(tool);
        self
    }

    /// Passes every call through `runner`'s `PreToolUse` and `PostToolUse`
    /// hooks.
    #[must_use]
    pub fn gated(mut self, runner: Arc<Mutex<Runner>>) -> Self {
        self.gate = Some(runner);
        self
    }

    #[must_use]
    pub fn tools(&self) -> &[Tool] {
        &self.tools
    }

    #[must_use]
    pub fn names(&self) -> Vec<String> {
        self.tools.iter().map(|tool| tool.name.clone()).collect()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }

    /// The hooks' outcomes on this server's calls so far, in order.
    #[must_use]
    pub fn fired(&self) -> Vec<Applied> {
        self.fired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    /// Runs `name` with `input`, through the hooks when the server is
    /// gated. An unknown tool, a denial and an unanswerable ask are error
    /// results the model reads.
    pub async fn call(&self, name: &str, input: Value) -> ToolResult {
        let Some(tool) = self.tools.iter().find(|tool| tool.name == name) else {
            return ToolResult::error(format!("There is no tool named {name}."));
        };
        let Some(gate) = self.gate.clone() else {
            return (tool.run)(input).await.unwrap_or_else(ToolResult::error);
        };
        let qualified = mcp_tool_name(&self.name, name);
        let pre = self
            .fire(&gate, Call::tool(Event::PreToolUse, &qualified, input))
            .await;
        let mut notes: Vec<String> = pre
            .context
            .iter()
            .map(|context| format!("[{}] {}", context.source, context.text))
            .collect();
        match &pre.permission {
            Permission::Deny { reason } => return ToolResult::error(reason.clone()),
            Permission::Ask { step } => {
                return ToolResult::error(format!(
                    "{step}: this needs the person's approval, and no one can answer from here. Hand it back to them."
                ));
            }
            Permission::Allow | Permission::NoOpinion => {}
        }
        let input = pre.input.clone();
        let mut result = (tool.run)(input.clone())
            .await
            .unwrap_or_else(ToolResult::error);
        let post = self
            .fire(
                &gate,
                Call::tool(
                    Event::PostToolUse,
                    &qualified,
                    json!({"tool_input": input, "tool_response": result.to_value()}),
                ),
            )
            .await;
        notes.extend(
            post.context
                .iter()
                .map(|context| format!("[{}] {}", context.source, context.text)),
        );
        for note in notes {
            result = result.with_text(note);
        }
        result
    }

    async fn fire(&self, gate: &Arc<Mutex<Runner>>, call: Call) -> Applied {
        let gate = gate.clone();
        // A hook may take its whole timeout: off the async threads.
        let applied = tokio::task::spawn_blocking(move || {
            gate.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .fire(&call)
        })
        .await;
        let applied = match applied {
            Ok(applied) => applied,
            Err(_) => Applied {
                event: Event::PreToolUse,
                tool: None,
                permission: Permission::deny("The hooks on this tool stopped without an answer."),
                decided_by: None,
                input: Value::Null,
                rewritten_by: Vec::new(),
                context: Vec::new(),
                dropped: Vec::new(),
                stop: super::outcome::Stop::Continue,
                failures: Vec::new(),
            },
        };
        self.fired
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(applied.clone());
        applied
    }

    /// The bytes of the server's `tools/list` answer: what a harness reads
    /// into its prompt. Two servers that offer the same tools give the
    /// same bytes, so a test can hold the parent's tool list fixed.
    pub async fn list_bytes(&self) -> Vec<u8> {
        let reply = self
            .in_process()
            .handle_message(&json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}))
            .await;
        serde_json::to_vec(&reply["result"]).unwrap_or_default()
    }

    /// The server in process, for Claude Code. Each call goes through
    /// [`ToolServer::call`].
    #[must_use]
    pub fn in_process(&self) -> SdkMcpServer {
        let mut server = SdkMcpServer::new(self.name.clone(), self.version.clone());
        if let Some(timeout) = self.timeout_ms {
            server = server.timeout_ms(timeout);
        }
        if let Some(instructions) = &self.instructions {
            server = server.instructions(instructions.clone());
        }
        let shared = Arc::new(self.clone());
        for tool in &self.tools {
            let shared = shared.clone();
            let name = tool.name.clone();
            let mut entry = SdkMcpTool::new(
                tool.name.clone(),
                tool.description.clone(),
                tool.schema.clone(),
                move |input: Value| {
                    let shared = shared.clone();
                    let name = name.clone();
                    async move { Ok::<_, claude_agent_sdk::Error>(shared.call(&name, input).await) }
                },
            )
            .always_load(tool.always_load);
            if let Some(annotations) = &tool.annotations {
                entry = entry.annotations(annotations.clone());
            }
            server = server.add_tool(entry);
        }
        server
    }
}

/// Serves `server` over MCP stdio: one JSON-RPC message per line in, one
/// reply per request out. Notifications get no reply. Ends when the input
/// does.
///
/// # Errors
///
/// When the output can't be written.
pub async fn serve<R, W>(server: &ToolServer, reader: R, mut writer: W) -> Result<(), String>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mcp = server.in_process();
    let mut lines = reader.lines();
    while let Some(line) = lines.next_line().await.map_err(|error| error.to_string())? {
        if line.trim().is_empty() {
            continue;
        }
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(error) => {
                let reply = json!({
                    "jsonrpc": "2.0",
                    "id": Value::Null,
                    "error": {"code": -32700, "message": format!("Parse error: {error}")},
                });
                write_line(&mut writer, &reply).await?;
                continue;
            }
        };
        // A notification or a response to us: nothing to answer.
        if message.get("method").is_none() || message.get("id").is_none_or(Value::is_null) {
            continue;
        }
        let reply = mcp.handle_message(&message).await;
        write_line(&mut writer, &reply).await?;
    }
    Ok(())
}

/// [`serve`] on this process's standard input and output.
///
/// # Errors
///
/// When the output can't be written.
pub async fn serve_stdio(server: &ToolServer) -> Result<(), String> {
    let stdin = tokio::io::BufReader::new(tokio::io::stdin());
    serve(server, stdin, tokio::io::stdout()).await
}

async fn write_line<W: AsyncWrite + Unpin>(writer: &mut W, reply: &Value) -> Result<(), String> {
    let mut line = reply.to_string();
    line.push('\n');
    writer
        .write_all(line.as_bytes())
        .await
        .map_err(|error| error.to_string())?;
    writer.flush().await.map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::super::compose::Budget;
    use super::super::outcome::{Context, HookOutcome};
    use super::super::registry::{Hook, Registry, Source};
    use super::*;

    fn echo() -> Tool {
        Tool::new(
            "echo",
            "Says the text back.",
            json!({"type": "object", "properties": {"text": {"type": "string"}}}),
            |input: Value| async move {
                Ok(ToolResult::text(
                    input["text"].as_str().unwrap_or_default().to_string(),
                ))
            },
        )
    }

    fn server() -> ToolServer {
        ToolServer::new("coder", "1.0.0")
            .instructions("Coder's tools.")
            .add(echo())
    }

    /// A recorded MCP client: initialize, the initialized notification,
    /// list, two calls (one to a tool that isn't there), and a ping.
    const RECORDED_CLIENT: &str = r#"{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"codex-mcp-client","version":"0.1.0"}}}
{"jsonrpc":"2.0","method":"notifications/initialized"}
{"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}
{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hello"}}}
{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"missing","arguments":{}}}
{"jsonrpc":"2.0","id":4,"method":"ping"}
"#;

    async fn replay(server: &ToolServer, client: &str) -> Vec<Value> {
        let mut out = Vec::new();
        serve(server, client.as_bytes(), &mut out).await.unwrap();
        String::from_utf8(out)
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect()
    }

    #[tokio::test]
    async fn a_recorded_stdio_client_lists_and_calls_the_tools() {
        let replies = replay(&server(), RECORDED_CLIENT).await;
        // One reply per request; the notification gets none.
        let ids: Vec<i64> = replies.iter().map(|r| r["id"].as_i64().unwrap()).collect();
        assert_eq!(ids, [0, 1, 2, 3, 4]);
        assert_eq!(replies[0]["result"]["serverInfo"]["name"], "coder");
        assert_eq!(replies[0]["result"]["instructions"], "Coder's tools.");
        assert_eq!(replies[1]["result"]["tools"][0]["name"], "echo");
        assert_eq!(replies[2]["result"]["content"][0]["text"], "hello");
        assert!(replies[3].get("error").is_some());
    }

    fn gate() -> Arc<Mutex<Runner>> {
        let mut registry = Registry::new();
        registry
            .register(
                Hook::new(
                    "shout",
                    Event::PreToolUse,
                    Source::BuiltIn,
                    |call: &Call| {
                        let text = call.input["text"].as_str().unwrap_or_default();
                        Ok(HookOutcome::rewrite(json!({"text": text.to_uppercase()})))
                    },
                )
                .matching("mcp__coder__echo"),
            )
            .unwrap();
        registry
            .register(
                Hook::floor(
                    "no-secrets",
                    Event::PreToolUse,
                    Permission::ask("confirm"),
                    |call: &Call| {
                        // The floor judges the rewritten input.
                        let text = call.input["text"].as_str().unwrap_or_default();
                        Ok(if text.contains("SECRET") {
                            HookOutcome::deny("That would print a secret.")
                        } else {
                            HookOutcome::none()
                        })
                    },
                )
                .matching("mcp__coder__echo"),
            )
            .unwrap();
        registry
            .register(Hook::new(
                "note",
                Event::PostToolUse,
                Source::BuiltIn,
                |call: &Call| {
                    let said = &call.input["tool_response"]["content"][0]["text"];
                    Ok(HookOutcome::context(Context::new(
                        "note",
                        &format!("the tool said {}", said.as_str().unwrap_or_default()),
                        0,
                    )))
                },
            ))
            .unwrap();
        Arc::new(Mutex::new(Runner::new(registry, Budget::default())))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn calls_pass_through_the_same_pre_and_post_tool_hooks() {
        let server = server().gated(gate());
        let result = server.call("echo", json!({"text": "hello"})).await;
        assert!(!result.is_error);
        let texts: Vec<Value> = result.to_value()["content"]
            .as_array()
            .unwrap()
            .iter()
            .map(|block| block["text"].clone())
            .collect();
        assert_eq!(texts, [json!("HELLO"), json!("[note] the tool said HELLO")]);

        let denied = server.call("echo", json!({"text": "my secret"})).await;
        assert!(denied.is_error);
        assert_eq!(
            denied.to_value()["content"][0]["text"],
            "That would print a secret."
        );
        let fired = server.fired();
        // Pre and post on the first call; only pre on the denied one.
        assert_eq!(fired.len(), 3);
        assert_eq!(fired[0].rewritten_by, ["shout"]);
        assert_eq!(fired[2].decided_by.as_deref(), Some("no-secrets"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_in_process_server_lists_the_same_tools_and_answers_the_same() {
        let server = server().timeout_ms(1_000);
        let mcp = server.in_process();
        assert_eq!(mcp.name(), "coder");
        assert_eq!(mcp.timeout(), Some(1_000));
        assert_eq!(mcp.allowed_tool_names(), ["mcp__coder__echo"]);
        let reply = mcp
            .handle_message(&json!({
                "jsonrpc": "2.0", "id": 7, "method": "tools/call",
                "params": {"name": "echo", "arguments": {"text": "hi"}},
            }))
            .await;
        assert_eq!(reply["result"]["content"][0]["text"], "hi");
    }
}
