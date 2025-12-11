# Claude Agent SDK for Rust

A Rust SDK for programmatically building AI agents with Claude Code's capabilities. Create autonomous agents that can understand codebases, edit files, run commands, and execute complex workflows.

This SDK is a Rust implementation of Anthropic's official [Claude Agent SDK](https://platform.claude.com/docs/en/agent-sdk/overview), providing the same functionality with native Rust ergonomics.

## Parity with TypeScript SDK

This crate has **~100% parity** with the official TypeScript SDK. All features from the TypeScript version are implemented:

- Query streaming and message types
- Permission handling (rules-based and callback)
- Query options (model, max_turns, budget, MCP servers, etc.)
- Session management (continue, resume)
- Control methods (interrupt, set_permission_mode, set_model, etc.)

### Rust-Only Extensions

We've added the following methods that are **not in the TypeScript SDK**:

| Method | Description |
|--------|-------------|
| `query.abort()` | Hard-kill the Claude CLI process. Unlike `interrupt()` which sends a graceful stop signal via the protocol, `abort()` immediately terminates the underlying process. Use this when you need to forcefully stop execution (e.g., user clicks "Stop" in a UI). |

```rust
// Graceful stop - sends interrupt signal via protocol
query.interrupt().await?;

// Hard stop - kills the process immediately
query.abort().await?;
```

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

Set your API key:

```bash
export ANTHROPIC_API_KEY=your_api_key_here
```

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

// Interrupt execution (graceful - sends protocol message)
query.interrupt().await?;

// Abort execution (hard - kills the process immediately)
query.abort().await?;

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

- `ANTHROPIC_API_KEY` - Your Anthropic API key (required)
- `CLAUDE_CODE_EXECUTABLE` - Path to Claude executable (optional)
- `DEBUG_CLAUDE_AGENT_SDK` - Enable debug logging (optional)

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
│   │  ├── abort()        ← Rust-only extension           │  │
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
