# Claude Agent SDK for Rust

A Rust SDK for programmatically building AI agents with Claude Code's capabilities. Create autonomous agents that can understand codebases, edit files, run commands, and execute complex workflows.

This SDK is a Rust implementation of Anthropic's official [Claude Agent SDK](https://platform.claude.com/docs/en/agent-sdk/overview), providing the same functionality with native Rust ergonomics.

Status, October 4, 2026: an Agent Studio seat on `claude/sdk:MODEL` runs its
turn on this SDK, through `microcoder`'s `claude_sdk` engine
(`crates/microcoder/src/repository/claude_sdk.rs`,
[#10571](https://github.com/OpenAgentsInc/openagents/issues/10571)). The
engine uses the owner's Claude Code login and reads no API key. Its
permission callback turns each tool request outside the task's worktree into
a studio approval: **Allow once** runs that exact request, **Deny** refuses
it, and the seat's standing rules apply. The engine runs under full access
only. See the [Agent Studio audit](../../docs/verse/agent-studio-audit.md).

The crate tracks SDK 0.3.296 (Claude Code 2.1.296) as of October 9, 2026;
[PARITY.md](PARITY.md) lists what each release added and what is deferred,
and `scripts/check-claude-sdk-parity.sh latest` compares it with the newest
npm release.

On Unix the CLI leads a process group of its own; `Query::kill`, or dropping
the `Query`, stops the whole group.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
claude_agent_sdk = { path = "../claude_agent_sdk" }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
futures = "0.3"
```

## Prerequisites

You need the Claude Code CLI installed. Install it via:

```bash
# Via npm
npm install -g @anthropic-ai/claude-code

# Or via Homebrew (macOS)
brew install anthropic/tap/claude-code
```

Sign in to Claude Code. The SDK runs the `claude` CLI as a child process
with your environment, so the CLI's own sign-in applies: a Claude
subscription login (`claude`, then `/login`) works, and the SDK needs no API
key. An `ANTHROPIC_API_KEY` in the environment also works, and Claude Code
uses it instead of the subscription.

## Quick Start

### Simple Query

```rust
use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    let mut stream = query(
        "What files are in this directory?",
        QueryOptions::new()
    ).await?;

    while let Some(message) = stream.next().await {
        match message? {
            SdkMessage::Assistant(msg) => {
                println!("Claude: {:?}", msg.message);
            }
            SdkMessage::Result(result) => {
                match result {
                    claude_agent_sdk::SdkResultMessage::Success(s) => {
                        println!("Result: {}", s.result);
                        println!("Cost: ${:.4}", s.total_cost_usd);
                        println!("Turns: {}", s.num_turns);
                    }
                    _ => println!("Query ended with error"),
                }
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

### With Options

```rust
use claude_agent_sdk::{query, QueryOptions, PermissionMode};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    let options = QueryOptions::new()
        .model("claude-sonnet-4-5-20250929")
        .permission_mode(PermissionMode::AcceptEdits)
        .max_turns(10)
        .max_budget_usd(1.0)
        .cwd("/path/to/project");

    let mut stream = query("Refactor the main function", options).await?;

    while let Some(msg) = stream.next().await {
        // Process messages...
    }

    Ok(())
}
```

### Custom Permission Handler

Control which tools Claude can use:

```rust
use claude_agent_sdk::{query_with_permissions, QueryOptions, PermissionRules};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    // Allow only safe tools
    let permissions = PermissionRules::new()
        .allow("Read")
        .allow("Glob")
        .allow("Grep")
        .deny("Bash")
        .deny("Write")
        .deny("Edit")
        .default_allow(false)
        .build();

    let stream = query_with_permissions(
        "Find all TODO comments in the codebase",
        QueryOptions::new(),
        Arc::new(permissions),
    ).await?;

    // Process stream...
    Ok(())
}
```

### Advanced Permission Handler with Callbacks

For fine-grained control, implement the `PermissionHandler` trait:

```rust
use claude_agent_sdk::{
    query_with_permissions, QueryOptions, PermissionHandler,
    PermissionResult, PermissionRequest, PermissionUpdate
};
use async_trait::async_trait;
use std::sync::Arc;
use serde_json::Value;

struct MyPermissionHandler;

#[async_trait]
impl PermissionHandler for MyPermissionHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,
        _suggestions: Option<Vec<PermissionUpdate>>,
        _blocked_path: Option<String>,
        _decision_reason: Option<String>,
        _tool_use_id: &str,
        _agent_id: Option<String>,
    ) -> claude_agent_sdk::Result<PermissionResult> {
        // Allow read-only tools
        if matches!(tool_name, "Read" | "Glob" | "Grep") {
            return Ok(PermissionResult::allow(input.clone()));
        }

        // Allow bash only for safe commands
        if tool_name == "Bash" {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                // Deny dangerous commands
                if cmd.contains("rm ") || cmd.contains("sudo") {
                    return Ok(PermissionResult::deny("Dangerous command not allowed"));
                }
                // Allow safe commands
                if cmd.starts_with("ls") || cmd.starts_with("cat") || cmd.starts_with("echo") {
                    return Ok(PermissionResult::allow(input.clone()));
                }
            }
        }

        // Deny everything else
        Ok(PermissionResult::deny(format!("Tool '{}' not allowed", tool_name)))
    }
}

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    let stream = query_with_permissions(
        "List files and show their contents",
        QueryOptions::new(),
        Arc::new(MyPermissionHandler),
    ).await?;

    // Process stream...
    Ok(())
}
```

### Using the Callback Helper

For simpler cases, use the `permission_handler` function:

```rust
use claude_agent_sdk::{query_with_permissions, QueryOptions, permission_handler, PermissionResult};

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    let handler = permission_handler(|request| async move {
        // Allow all Read operations
        if request.tool_name == "Read" {
            return Ok(PermissionResult::allow(request.input));
        }
        // Deny everything else
        Ok(PermissionResult::deny("Only Read is allowed"))
    });

    let stream = query_with_permissions(
        "Read the README",
        QueryOptions::new(),
        handler,
    ).await?;

    Ok(())
}
```

## Message Types

The SDK streams various message types:

```rust
use claude_agent_sdk::SdkMessage;

match message {
    // Claude's response
    SdkMessage::Assistant(msg) => {
        // msg.message contains the API response
        // msg.uuid is the message ID
        // msg.session_id is the session ID
    }

    // User message echo
    SdkMessage::User(msg) => {
        // Echoed user message
    }

    // Query result (success or error)
    SdkMessage::Result(result) => {
        match result {
            SdkResultMessage::Success(s) => {
                println!("Result: {}", s.result);
                println!("Cost: ${}", s.total_cost_usd);
                println!("Turns: {}", s.num_turns);
            }
            SdkResultMessage::ErrorDuringExecution(e) => {
                println!("Errors: {:?}", e.errors);
            }
            SdkResultMessage::ErrorMaxTurns(e) => {
                println!("Max turns exceeded");
            }
            SdkResultMessage::ErrorMaxBudget(e) => {
                println!("Budget exceeded");
            }
            _ => {}
        }
    }

    // System messages (init, status, hooks)
    SdkMessage::System(sys) => {
        match sys {
            SdkSystemMessage::Init(init) => {
                println!("Session: {}", init.session_id);
                println!("Model: {}", init.model);
                println!("Tools: {:?}", init.tools);
            }
            SdkSystemMessage::Status(status) => {
                // Status update (e.g., "compacting")
            }
            _ => {}
        }
    }

    // Streaming partial response (if include_partial_messages is true)
    SdkMessage::StreamEvent(event) => {
        // Partial assistant message
    }

    // Tool progress updates
    SdkMessage::ToolProgress(progress) => {
        println!("Tool {} running for {}s",
            progress.tool_name,
            progress.elapsed_time_seconds);
    }

    // Authentication status
    SdkMessage::AuthStatus(auth) => {
        if auth.is_authenticating {
            println!("Authenticating...");
        }
    }
}
```

## Query Options

Full list of available options:

```rust
let options = QueryOptions::new()
    // Model selection
    .model("claude-sonnet-4-5-20250929")

    // Working directory
    .cwd("/path/to/project")

    // Permission mode
    .permission_mode(PermissionMode::Default)
    // Available modes:
    // - Default: Standard prompts for dangerous operations
    // - AcceptEdits: Auto-accept file edits
    // - BypassPermissions: Skip all checks (requires allow_dangerously_skip_permissions)
    // - Plan: Planning mode, no tool execution
    // - DontAsk: Deny if not pre-approved

    // Limits
    .max_turns(10)
    .max_budget_usd(5.0)

    // Include streaming partial messages
    .include_partial_messages(true)

    // Session management
    .continue_session()  // Continue most recent session
    .resume("session-id-here")  // Resume specific session

    // MCP servers
    .mcp_server("my-server", McpServerConfig::Stdio {
        command: "node".to_string(),
        args: Some(vec!["./my-mcp-server.js".to_string()]),
        env: None,
    });
```

## Query Control Methods

The `Query` struct provides methods to control execution:

```rust
let query = query("Do something", QueryOptions::new()).await?;

// Interrupt execution
query.interrupt().await?;

// Change permission mode mid-query
query.set_permission_mode(PermissionMode::AcceptEdits).await?;

// Change model mid-query
query.set_model(Some("claude-opus-4-20250514".to_string())).await?;

// Set max thinking tokens
query.set_max_thinking_tokens(Some(10000)).await?;

// Get MCP server status
let status = query.mcp_server_status().await?;

// Rewind files to a specific message (requires enable_file_checkpointing)
query.rewind_files("message-uuid").await?;

// Check if query completed
if query.is_completed() {
    println!("Query finished");
}

// Get session ID
if let Some(session_id) = query.session_id() {
    println!("Session: {}", session_id);
}
```

## Custom Tools (In-Process MCP Servers)

Tools written in Rust can run inside the host process, as with the TS
SDK's `tool()` and `createSdkMcpServer()`. Build an `SdkMcpServer`, add
tools with a JSON Schema and an async handler, and register it with
`sdk_mcp_server`. The server is named in the `initialize` request; the CLI
then sends each MCP JSON-RPC message (`initialize`, `tools/list`,
`tools/call`, notifications) as an `mcp_message` control request, and the
crate answers from the server. The model sees each tool as
`mcp__<server>__<tool>`, which is also the name `allowed_tools` takes.

```rust,no_run
use claude_agent_sdk::{QueryOptions, SdkMcpServer, ToolResult, query};
use serde_json::json;

# async fn example() -> Result<(), claude_agent_sdk::Error> {
let calc = SdkMcpServer::new("calc", "1.0.0").tool(
    "add",
    "Add two numbers and return the sum.",
    json!({
        "type": "object",
        "properties": {"a": {"type": "number"}, "b": {"type": "number"}},
        "required": ["a", "b"]
    }),
    |args| async move {
        let (Some(a), Some(b)) = (args["a"].as_f64(), args["b"].as_f64()) else {
            return Ok(ToolResult::error("a and b must be numbers"));
        };
        Ok(ToolResult::text((a + b).to_string()))
    },
);
let mut options = QueryOptions::new().sdk_mcp_server(calc);
options.allowed_tools = Some(vec!["mcp__calc__add".into()]);
let stream = query("Use the add tool to add 1234 and 5678.", options).await?;
# Ok(())
# }
```

`ToolResult` carries text and base64 image blocks (`text`, `image`,
`with_text`, `with_image`), optional `structuredContent`, and `is_error`
(`ToolResult::error`). A handler that returns `Err` becomes an `isError`
result with the error text. `SdkMcpTool` adds annotations, `always_load`,
and a tool-search hint; the server takes `instructions`, `always_load`, and
a per-server `timeout_ms`. `sdk_mcp_manifests(true)` sends each server's
`initialize` and `tools/list` results in `initialize`, saving the CLI those
round trips. The CLI cuts tool descriptions at 4,096 characters (16,384
when loaded through tool search). The live example, which has Claude call
`add` on the owner's login:

```bash
cargo run -p claude_agent_sdk --example sdk_mcp_tools
```

## Hooks, Elicitation, and Dialogs

Hook callbacks run in the host. The CLI calls them over the control
protocol when the hook fires, and the callback's output is the reply:

```rust,no_run
use claude_agent_sdk::{HookEvent, HookMatcher, QueryOptions, SyncHookJSONOutput, hook_fn};

let deny_shell = hook_fn(|_input, _tool_use_id| async {
    Ok(SyncHookJSONOutput {
        hook_specific_output: Some(serde_json::json!({
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "no shell in this session"
        })),
        ..Default::default()
    }
    .into())
});
let options = QueryOptions::new().hook(HookEvent::PreToolUse, HookMatcher::new(Some("Bash"), deny_shell));
```

Set `on_elicitation` to answer MCP elicitation (without it, the SDK
declines) and `on_user_dialog` to answer `request_user_dialog` (without
it, the request gets no reply). [PARITY.md](PARITY.md) lists what the
crate covers at SDK 0.3.296 and what remains.

To check the crate against the installed CLI, run the live smoke. It
saves no session:

```bash
cargo run -p claude_agent_sdk --example smoke
```

## Custom Executable Path

If Claude isn't in your PATH:

```rust
use claude_agent_sdk::{QueryOptions, ExecutableConfig};
use std::path::PathBuf;

let options = QueryOptions {
    executable: ExecutableConfig {
        path: Some(PathBuf::from("/custom/path/to/claude")),
        ..Default::default()
    },
    ..QueryOptions::new()
};

// Or for cli.js with a specific runtime:
let options = QueryOptions {
    executable: ExecutableConfig {
        path: Some(PathBuf::from("/path/to/cli.js")),
        executable: Some("bun".to_string()),  // or "node", "deno"
        executable_args: vec!["--smol".to_string()],
    },
    ..QueryOptions::new()
};
```

## Error Handling

```rust
use claude_agent_sdk::Error;

match result {
    Err(Error::ExecutableNotFound(msg)) => {
        eprintln!("Claude not found: {}", msg);
        eprintln!("Install with: npm install -g @anthropic-ai/claude-code");
    }
    Err(Error::SpawnFailed(e)) => {
        eprintln!("Failed to start Claude: {}", e);
    }
    Err(Error::PermissionDenied { tool }) => {
        eprintln!("Permission denied for tool: {}", tool);
    }
    Err(Error::Aborted) => {
        eprintln!("Query was aborted");
    }
    Err(e) => {
        eprintln!("Error: {}", e);
    }
    Ok(_) => {}
}
```

## Environment Variables

The SDK reads no settings from the environment. The CLI inherits your
environment plus `QueryOptions::env`, so the CLI's variables apply:

- `ANTHROPIC_API_KEY` - Optional. Claude Code uses this key instead of your
  subscription login.

The SDK finds `claude` on `PATH`, then in `~/.claude/local/claude`,
`/usr/local/bin/claude`, and `/opt/homebrew/bin/claude`. To use another
binary, set `path` in `QueryOptions::executable`.

## Architecture

This SDK spawns the Claude Code CLI as a child process and communicates via JSONL over stdin/stdout:

```
┌─────────────────────────────────────────────────────────────┐
│                  Your Rust Application                      │
│                                                             │
│   let query = query("Fix the bug", options).await;         │
│                         │                                   │
│                         ▼                                   │
│   ┌─────────────────────────────────────────────────────┐  │
│   │              claude_agent_sdk crate                  │  │
│   │                                                      │  │
│   │  ProcessTransport                                    │  │
│   │  ├── spawn(claude binary)                           │  │
│   │  ├── write JSONL to stdin                           │  │
│   │  └── read JSONL from stdout                         │  │
│   │                                                      │  │
│   │  Query (implements Stream<SdkMessage>)              │  │
│   │  ├── interrupt()                                     │  │
│   │  ├── set_permission_mode()                          │  │
│   │  └── set_model()                                     │  │
│   └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │ JSONL over stdio
                          ▼
┌─────────────────────────────────────────────────────────────┐
│           Claude Code CLI (native binary)                   │
│                                                             │
│  - Anthropic API integration                                │
│  - Tool execution (Bash, Read, Edit, Glob, Grep, etc.)     │
│  - MCP server management                                    │
│  - Permission system                                        │
│  - Session persistence                                      │
│  - Context compaction                                       │
└─────────────────────────────────────────────────────────────┘
```

## Testing

Run tests:

```bash
cargo test -p claude_agent_sdk
```

## License

MIT

## Related

- [Claude Agent SDK (TypeScript)](https://www.npmjs.com/package/@anthropic-ai/claude-agent-sdk)
- [Claude Code Documentation](https://docs.anthropic.com/en/docs/claude-code)
- [Agent SDK Documentation](https://platform.claude.com/docs/en/agent-sdk/overview)
