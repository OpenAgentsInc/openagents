# codex-mcp

Minimal Rust helpers for building **MCP (Model Context Protocol)** servers that speak **JSON-RPC 2.0 over stdio**, in the style expected by Codex CLI / app-server integrations.

This crate intentionally stays small and focused on the “tools” path (`tools/list`, `tools/call`) so OpenAgents binaries can expose capabilities to Codex with minimal boilerplate.

`codex-mcp` is published as a normal Rust library crate and its crate-level docs are this README (via `#![doc = include_str!("../README.md")]`).

## Status / scope

`codex-mcp` implements a pragmatic subset of MCP suitable for Codex-style stdio servers:

- ✅ JSON-RPC request/response envelope
- ✅ `initialize`
- ✅ `tools/list`, `tools/call`
- ✅ newline-delimited JSON framing over stdio
- ❌ MCP resources/prompts/sampling and other extended surface area (by design)

If you need a broader MCP implementation, treat this crate as a small foundation (or introduce a richer MCP crate) rather than expecting full protocol coverage here.

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

# Runtime / utilities you’ll typically want in an MCP server binary:
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

Configure via Codex settings (example):

```json
{
  "mcpServers": {
    "echo": {
      "type": "stdio",
      "command": "echo-mcp-server"
    }
  }
}
```

Or via CLI (example):

```bash
codex --mcp-server "echo:stdio:echo-mcp-server"
```

The name before `:stdio:` is the server identifier that will show up in tool call events.

### Important: keep stdout clean

For stdio MCP servers, **stdout is the protocol channel**. Any extra output (println debug, logs written to stdout, progress bars, etc.) will corrupt the JSON stream.

- Write logs to stderr (`eprintln!`, or `tracing_subscriber` defaults).
- Only JSON-RPC responses should be written to stdout.

## Manual smoke test (stdio)

You can also test your server without Codex by piping JSON-RPC lines into stdin.

1) Run your server (replace `./echo-mcp-server`):

```bash
./echo-mcp-server
```

2) In another terminal, send JSON-RPC requests (each request must be on a single line):

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"message":"hi"}}}' \
| ./echo-mcp-server
```

Responses are written one-per-line to stdout.

## Concepts

- **Server**: a subprocess Codex launches (stdio transport).
- **Tool**: a named capability listed by `tools/list` and invoked by `tools/call`.
- **Tool input schema**: `Tool::input_schema` is arbitrary JSON Schema that Codex can use for UI/validation hints (your server should still validate inputs).
- **Tool call arguments**: `ToolCallParams::arguments` is arbitrary JSON (often an object).
- **Tool result content**: `ToolCallResult::content` is a list of typed items. This crate currently supports text content only (`ToolResultContent::Text`).

## JSON-RPC methods handled

`codex-mcp` routes these methods:

- `initialize` (optional handler override via `McpHandler::initialize`)
- `tools/list`
- `tools/call`

Unknown methods return JSON-RPC `-32601 (Method not found)`.

## Wire format (stdio)

`StdioTransport` uses **newline-delimited JSON**:

- Each JSON-RPC request must be a single line of JSON.
- Each JSON-RPC response is emitted as a single line of JSON followed by `\n`.
- EOF cleanly shuts down the server loop.

This framing is simple and works well for subprocess integration, but it means callers must not pretty-print or send multi-line JSON.

## Protocol shapes (subset)

This crate implements the shapes it needs for the tools flow. In practice that means:

- `initialize`:
  - Request params: any JSON (passed through to your override).
  - Default result:
    - `capabilities: { tools: {} }`
    - `serverInfo: { name: "codex-mcp", version: <crate version> }`
- `tools/list`:
  - Result: `{ "tools": [ { "name": "...", "description": "...?", "input_schema": <json> } ] }`
- `tools/call`:
  - Params: `{ "name": "<tool name>", "arguments": <json> }`
  - Result: `{ "content": [ { "type": "text", "text": "..." } ], "is_error": <bool> }`

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

### Tool naming

Codex will typically address tools by `name` as returned from `tools/list`. Prefer stable, lowercase names and avoid whitespace. If you need namespacing, encode it in the tool name (for example `fs.read_file`), not in the server name.

### Validation

`codex-mcp` does not enforce JSON Schema at runtime. If you publish an `input_schema`, you should still validate `ToolCallParams::arguments`:

- For small servers, decode to a struct with `serde` (see example below).
- For stricter validation, you can use a JSON Schema validator in your binary crate.

## Transport

The transport layer is a tiny abstraction so you can swap framing/IO later:

- `transport::Transport`:
  - `read_request() -> anyhow::Result<Option<protocol::Request>>`
  - `write_response(&protocol::Response) -> anyhow::Result<()>`
- `transport::StdioTransport`:
  - Reads stdin line-by-line (newline-delimited JSON).
  - Writes one JSON-RPC response per line to stdout and flushes.
  - Empty lines are tolerated and ignored by `serve()` (they become a request with an empty `method`).
  - EOF cleanly shuts down the server loop.

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

### Notes on IDs and notifications

- If a request comes in with `id = null` or no `id`, it is treated as a JSON-RPC notification.
- `server::serve()` still calls your handler for notifications, but will emit a response with `id = None`.
  - If your caller is strict about “no response for notifications”, wrap `Transport` and drop responses where `response.id.is_none()`.

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

## Logging / tracing

This crate does not emit logs by itself, but it depends on `tracing` so downstream crates can instrument handlers and transports in a consistent way. A typical server binary sets up a subscriber and then logs inside `tools_call`:

```rust,no_run
use codex_mcp::protocol::ToolCallParams;

fn log_tools_call(params: &ToolCallParams) {
    tracing::info!(tool = %params.name, "tools/call");
}
```

## Troubleshooting

- **Codex can’t parse responses**: ensure *nothing* else writes to stdout (logs, debug prints, banners). Use stderr for logs.
- **Hangs / no responses**: requests must be one JSON object per line; pretty-printed JSON will block `read_line`.
- **`tools/call` fails to decode**: confirm the request uses `{ "name": "...", "arguments": ... }` (and that `arguments` is valid JSON).

## Versioning and compatibility

- The crate is intended to be “boring” and stable, but it is not a complete MCP implementation.
- The on-the-wire format is JSON-RPC 2.0. The method names and result shapes are those commonly used by Codex-style MCP tool servers.

## License

This crate inherits the workspace license (see the repo root `Cargo.toml`).

## Design notes / non-goals

- **Framing**: newline-delimited JSON (one JSON-RPC message per line).
- **Errors**: returned as JSON-RPC `error` objects; handler failures map to `-32000` with error text in `data`.
- **Scope**: minimal “tool server” building blocks; not a complete MCP spec implementation.
- **Surface area**: intentionally no resources/prompts/sampling; add them in your handler + routing if you need them.
