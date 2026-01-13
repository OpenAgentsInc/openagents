# codex-mcp

Minimal Rust helpers for building **MCP (Model Context Protocol)** servers that speak **JSON-RPC 2.0 over stdio**, in the style expected by Codex CLI / app-server integrations.

This crate intentionally stays small and focused on the “tools” path (`tools/list`, `tools/call`) so OpenAgents binaries can expose capabilities to Codex with minimal boilerplate.

`codex-mcp` is published as a normal Rust library crate and its crate-level docs are this README (via `#![doc = include_str!("../README.md")]`).

## What you get

- **Protocol types**: `Request`, `Response`, `Tool`, `ToolsListResult`, `ToolCallParams`, `ToolCallResult`
- **Transport**: newline-delimited JSON over stdio via `StdioTransport`
- **Server loop**: `server::serve()` that routes JSON-RPC methods to a handler

## When to use this

Use `codex-mcp` when you want a small Rust binary that:

- runs as a subprocess under Codex (`--mcp-server "...:stdio:your-bin"`)
- exposes one or more “tools” that Codex can invoke
- doesn’t need a full web server, sockets, or a larger framework

If you need resources/prompts/sampling or other MCP surface area, treat this crate as a starting point and extend it (or introduce a richer implementation).

## Install

Add it to your binary crate:

```toml
[dependencies]
# If you’re inside this workspace:
codex-mcp = { workspace = true }

# Or, if you’re in a separate repo and want a path dependency:
# codex-mcp = { path = "../openagents/crates/codex-mcp" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
anyhow = "1"
async-trait = "0.1"
```

In this repo, it’s already part of the workspace (`crates/codex-mcp`).

## Quickstart (minimal server)

Add a binary crate that depends on `codex-mcp`, implement `McpHandler`, and call `serve()` with `StdioTransport`:

```rust
use codex_mcp::{
    protocol::{Tool, ToolCallParams, ToolCallResult, ToolResultContent, ToolsListResult},
    server::{serve, McpHandler},
    transport::StdioTransport,
};

struct Echo;

#[async_trait::async_trait]
impl McpHandler for Echo {
    async fn tools_list(&self) -> anyhow::Result<ToolsListResult> {
        Ok(ToolsListResult {
            tools: vec![Tool {
                name: "echo".to_string(),
                description: Some("Echo input arguments as text".to_string()),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": { "message": { "type": "string" } },
                    "required": ["message"]
                }),
            }],
        })
    }

    async fn tools_call(&self, params: ToolCallParams) -> anyhow::Result<ToolCallResult> {
        match params.name.as_str() {
            "echo" => {
                let message = params
                    .arguments
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                Ok(ToolCallResult {
                    content: vec![ToolResultContent::Text {
                        text: message.to_string(),
                    }],
                    is_error: false,
                })
            }
            _ => Ok(ToolCallResult {
                content: vec![ToolResultContent::Text {
                    text: format!("unknown tool: {}", params.name),
                }],
                is_error: true,
            }),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    serve(StdioTransport::new(), &Echo).await
}
```

### Wire into Codex

Codex supports stdio MCP servers by launching a subprocess and speaking JSON-RPC over stdin/stdout.

Example (conceptual):

```bash
codex --mcp-server "echo:stdio:echo-mcp-server"
```

The name before `:stdio:` is the server identifier that will show up in tool call events.

## JSON-RPC methods handled

`codex-mcp` routes these methods:

- `initialize` (optional handler override via `McpHandler::initialize`)
- `tools/list`
- `tools/call`

Unknown methods return JSON-RPC `-32601 (Method not found)`.

## Handler API

Implement `server::McpHandler`:

- `initialize(params: serde_json::Value) -> anyhow::Result<serde_json::Value>`
  - Default implementation returns:
    - `capabilities: { tools: {} }`
    - `serverInfo: { name: "codex-mcp", version: <crate version> }`
  - Override to add fields Codex expects from your server (custom `serverInfo`, extra capabilities, etc).
- `tools_list() -> anyhow::Result<ToolsListResult>`
  - Return the tool catalog. Each `Tool` includes:
    - `name`: tool name (string)
    - `description`: optional
    - `input_schema`: JSON Schema (arbitrary `serde_json::Value`)
- `tools_call(params: ToolCallParams) -> anyhow::Result<ToolCallResult>`
  - Dispatch on `params.name`.
  - `params.arguments` is arbitrary JSON.
  - Set `ToolCallResult::is_error = true` for “tool-level” failures (unknown tool, validation failures, domain errors), even if the JSON-RPC request itself was valid.

## Transport

The transport layer is a tiny abstraction so you can swap framing/IO later:

- `transport::Transport`:
  - `read_request() -> anyhow::Result<Option<protocol::Request>>`
  - `write_response(&protocol::Response) -> anyhow::Result<()>`
- `transport::StdioTransport`:
  - Reads stdin line-by-line (newline-delimited JSON).
  - Writes one JSON-RPC response per line to stdout and flushes.
  - Empty lines are tolerated and ignored by `serve()` (they become a request with an empty `method`).

If you need TCP/WebSocket/HTTP, implement `Transport` and keep `server::serve()` unchanged.

## Protocol types

The `protocol` module defines the subset of JSON-RPC/MCP that this crate uses:

- `Request { jsonrpc, id, method, params }`
  - `id` is `Option<Id>` (notifications have `id = None`).
  - `params` defaults to `null` if missing.
- `Response { jsonrpc, id, result, error }`
  - Use `Response::ok(id, result)` or `Response::err(id, code, message, data)`.
- `ToolCallResult { content, is_error }`
  - `content` is a list of `ToolResultContent` items (currently just `text`).

## Error behavior

`server::serve()` maps errors like this:

- Invalid or unknown JSON-RPC method: `-32601 (Method not found)`
- Handler failures (initialize/tools/list/tools/call): `-32000` with:
  - `error.message`: fixed string like `"tools/call failed"`
  - `error.data`: `{ "error": "<stringified anyhow error>" }`
- `tools/call` param decoding failures:
  - `-32000` with `"tools/call failed"` and `error.data` from the decode error

If you want structured error codes for tool failures, encode them in `ToolCallResult` and set `is_error = true`.

## Example: typed arguments (recommended)

Instead of manually poking at `params.arguments`, decode it into a struct:

```rust,no_run
use codex_mcp::protocol::{ToolCallParams, ToolCallResult, ToolResultContent};
use codex_mcp::server::McpHandler;

#[derive(serde::Deserialize)]
struct EchoArgs {
    message: String,
}

struct Echo;

#[async_trait::async_trait]
impl McpHandler for Echo {
    async fn tools_list(&self) -> anyhow::Result<codex_mcp::protocol::ToolsListResult> {
        unimplemented!("not shown")
    }

    async fn tools_call(&self, params: ToolCallParams) -> anyhow::Result<ToolCallResult> {
        if params.name != "echo" {
            return Ok(ToolCallResult {
                content: vec![ToolResultContent::Text { text: "unknown tool".into() }],
                is_error: true,
            });
        }
        let args: EchoArgs = serde_json::from_value(params.arguments)?;
        Ok(ToolCallResult {
            content: vec![ToolResultContent::Text { text: args.message }],
            is_error: false,
        })
    }
}
```

## Design notes / non-goals

- **Framing**: newline-delimited JSON (one JSON-RPC message per line).
- **Errors**: returned as JSON-RPC `error` objects; handler failures map to `-32000` with error text in `data`.
- **Scope**: minimal “tool server” building blocks; not a complete MCP spec implementation.
- **Surface area**: intentionally no resources/prompts/sampling; add them in your handler + routing if you need them.
