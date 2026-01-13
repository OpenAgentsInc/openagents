# codex-mcp

Minimal Rust helpers for building **MCP (Model Context Protocol)** servers that speak **JSON-RPC 2.0 over stdio**, in the style expected by Codex CLI / app-server integrations.

This crate intentionally stays small and focused on the “tools” path (`tools/list`, `tools/call`) so OpenAgents binaries can expose capabilities to Codex with minimal boilerplate.

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

## Design notes

- **Framing**: newline-delimited JSON (one JSON-RPC message per line).
- **Errors**: returned as JSON-RPC `error` objects; handler failures map to `-32000` with error text in `data`.
- **Scope**: minimal “tool server” building blocks; not a complete MCP spec implementation.

