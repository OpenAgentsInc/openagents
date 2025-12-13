# Building a Rust Claude Agent SDK

> How to build `crates/claude_agent_sdk` - a Rust implementation of the Claude Agent SDK

## Status: IMPLEMENTED

The Rust SDK is now implemented at `crates/claude_agent_sdk/`.

**Quick Start:**

```rust
use claude_agent_sdk::{query, QueryOptions, SdkMessage};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<(), claude_agent_sdk::Error> {
    // Simple one-shot query
    let mut stream = query("What files are in this directory?", QueryOptions::new()).await?;

    while let Some(message) = stream.next().await {
        match message? {
            SdkMessage::Assistant(msg) => {
                println!("Claude: {:?}", msg.message);
            }
            SdkMessage::Result(result) => {
                println!("Query completed: {:?}", result);
            }
            _ => {}
        }
    }

    Ok(())
}
```

**With Custom Permissions:**

```rust
use claude_agent_sdk::{query_with_permissions, QueryOptions, PermissionRules};
use std::sync::Arc;

let permissions = PermissionRules::new()
    .allow("Read")
    .allow("Glob")
    .deny("Bash")
    .default_allow(false)
    .build();

let stream = query_with_permissions(
    "List all Rust files",
    QueryOptions::new()
        .model("claude-sonnet-4-5-20250929")
        .max_turns(10),
    Arc::new(permissions),
).await?;
```

**All 18 tests pass:**
```
cargo test -p claude_agent_sdk
```

---

## Legal Considerations

**Important:** Before building this, review Anthropic's terms.

The [Commercial Terms](https://www.anthropic.com/legal/commercial-terms) Section D.4 states you cannot:
- "reverse engineer or duplicate the Services"
- "access the Services to build a competing product or service"

**Our interpretation:** A Rust SDK wrapper is likely **permitted** because:

1. **We're NOT reverse engineering** - We're using the official CLI as-is, spawning it as a child process. The protocol (JSONL over stdio) is documented and intended for programmatic use.

2. **We're NOT duplicating the Services** - The "service" is Claude Code's functionality (tools, API calls, session management). Our SDK just provides a different language binding to invoke the same CLI.

3. **We're NOT building a competing product** - We're building a client library, like building a Redis client in Rust. The official TypeScript SDK does the same thing (spawns the CLI).

4. **Precedent exists** - Anthropic explicitly provides:
   - The `--output-format stream-json` and `--input-format stream-json` flags
   - The `spawnClaudeCodeProcess` option for custom spawning
   - Documentation of the SDK protocol

**However:** If you plan to distribute this commercially or at scale, consult with Anthropic directly or seek legal advice. The terms could be interpreted differently.

**Safe approach:** Frame this as "language bindings for the Claude Code CLI" rather than "an alternative SDK implementation."

---

## Overview

The official Claude Agent SDK (`@anthropic-ai/claude-agent-sdk`) is a thin wrapper around Claude Code's CLI. It spawns `cli.js` as a child process and communicates via JSONL over stdio. We can build an equivalent Rust SDK that does the same thing.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                  Your Rust Application                      │
│                                                             │
│   let query = sdk.query("Fix the bug", options).await;     │
│                         │                                   │
│                         ▼                                   │
│   ┌─────────────────────────────────────────────────────┐  │
│   │              claude_agent_sdk crate                  │  │
│   │                                                      │  │
│   │  ProcessTransport                                    │  │
│   │  ├── spawn(cli.js | claude binary)                  │  │
│   │  ├── write JSONL to stdin                           │  │
│   │  └── read JSONL from stdout                         │  │
│   │                                                      │  │
│   │  Query (async Stream<SDKMessage>)                   │  │
│   │  ├── interrupt()                                     │  │
│   │  ├── set_permission_mode()                          │  │
│   │  └── set_model()                                     │  │
│   └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
                          │ JSONL over stdio
                          ▼
┌─────────────────────────────────────────────────────────────┐
│           Claude Code CLI (cli.js or native binary)         │
│                                                             │
│  - Anthropic API integration                                │
│  - Tool execution (Bash, Read, Edit, Glob, Grep, etc.)     │
│  - MCP server management                                    │
│  - Permission system                                        │
│  - Session persistence                                      │
│  - Context compaction                                       │
└─────────────────────────────────────────────────────────────┘
```

## The Protocol

Claude Code CLI uses a bidirectional JSONL protocol over stdio:

### Messages FROM CLI (stdout)

```rust
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum StdoutMessage {
    #[serde(rename = "assistant")]
    Assistant(SDKAssistantMessage),

    #[serde(rename = "user")]
    User(SDKUserMessage),

    #[serde(rename = "result")]
    Result(SDKResultMessage),

    #[serde(rename = "system")]
    System(SDKSystemMessage),

    #[serde(rename = "stream_event")]
    StreamEvent(SDKPartialAssistantMessage),

    #[serde(rename = "control_request")]
    ControlRequest(SDKControlRequest),

    #[serde(rename = "control_response")]
    ControlResponse(SDKControlResponse),

    #[serde(rename = "keep_alive")]
    KeepAlive,

    #[serde(rename = "tool_progress")]
    ToolProgress(SDKToolProgressMessage),

    #[serde(rename = "auth_status")]
    AuthStatus(SDKAuthStatusMessage),
}
```

### Messages TO CLI (stdin)

```rust
#[derive(Debug, Serialize)]
#[serde(tag = "type")]
pub enum StdinMessage {
    #[serde(rename = "user")]
    User(SDKUserMessage),

    #[serde(rename = "control_request")]
    ControlRequest(SDKControlRequest),

    #[serde(rename = "control_response")]
    ControlResponse(SDKControlResponse),

    #[serde(rename = "keep_alive")]
    KeepAlive,
}
```

### Control Requests

The SDK and CLI exchange control messages for things like permission prompts:

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct SDKControlRequest {
    pub request_id: String,
    pub request: ControlRequestType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "subtype")]
pub enum ControlRequestType {
    /// CLI asking: "Can I use this tool?"
    #[serde(rename = "can_use_tool")]
    CanUseTool {
        tool_name: String,
        input: serde_json::Value,
        permission_suggestions: Option<Vec<PermissionUpdate>>,
        blocked_path: Option<String>,
        decision_reason: Option<String>,
        tool_use_id: String,
        agent_id: Option<String>,
    },

    /// SDK telling CLI to initialize
    #[serde(rename = "initialize")]
    Initialize {
        hooks: Option<HashMap<String, Vec<SDKHookCallbackMatcher>>>,
        sdk_mcp_servers: Option<Vec<String>>,
        json_schema: Option<serde_json::Value>,
        system_prompt: Option<String>,
        append_system_prompt: Option<String>,
        agents: Option<HashMap<String, AgentDefinition>>,
    },

    /// SDK telling CLI to interrupt
    #[serde(rename = "interrupt")]
    Interrupt,

    /// SDK changing permission mode
    #[serde(rename = "set_permission_mode")]
    SetPermissionMode { mode: PermissionMode },

    /// SDK changing model
    #[serde(rename = "set_model")]
    SetModel { model: Option<String> },

    // ... more control types
}
```

## Crate Structure

```
crates/claude_agent_sdk/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Public API
│   ├── query.rs            # Query struct (async stream)
│   ├── options.rs          # QueryOptions
│   ├── transport/
│   │   ├── mod.rs
│   │   ├── process.rs      # ProcessTransport (spawns CLI)
│   │   └── types.rs        # SpawnedProcess, SpawnOptions
│   ├── protocol/
│   │   ├── mod.rs
│   │   ├── messages.rs     # All message types
│   │   ├── control.rs      # Control request/response types
│   │   └── anthropic.rs    # Anthropic API message types
│   ├── permissions.rs      # CanUseTool callback, PermissionResult
│   ├── hooks.rs            # Hook types and callbacks
│   ├── mcp.rs              # MCP server config types
│   └── error.rs            # Error types
```

## Core Types

### QueryOptions

```rust
#[derive(Debug, Default)]
pub struct QueryOptions {
    /// Current working directory
    pub cwd: Option<PathBuf>,

    /// Model to use (e.g., "claude-sonnet-4-5-20250929")
    pub model: Option<String>,

    /// Fallback model if primary fails
    pub fallback_model: Option<String>,

    /// Permission mode
    pub permission_mode: Option<PermissionMode>,

    /// Skip permission checks (dangerous!)
    pub allow_dangerously_skip_permissions: bool,

    /// Maximum conversation turns
    pub max_turns: Option<u32>,

    /// Maximum budget in USD
    pub max_budget_usd: Option<f64>,

    /// Allowed tools
    pub allowed_tools: Vec<String>,

    /// Disallowed tools
    pub disallowed_tools: Vec<String>,

    /// MCP server configurations
    pub mcp_servers: HashMap<String, McpServerConfig>,

    /// Custom permission handler
    pub can_use_tool: Option<Box<dyn CanUseTool>>,

    /// System prompt (custom or preset)
    pub system_prompt: Option<SystemPrompt>,

    /// Include partial streaming messages
    pub include_partial_messages: bool,

    /// Session to resume
    pub resume: Option<String>,

    /// Continue most recent session
    pub continue_session: bool,

    /// Setting sources to load
    pub setting_sources: Vec<SettingSource>,

    /// Path to Claude Code executable
    pub path_to_executable: Option<PathBuf>,

    /// Custom process spawner (for VMs, containers, etc.)
    pub spawn_process: Option<Box<dyn SpawnProcess>>,

    /// Programmatic subagent definitions
    pub agents: HashMap<String, AgentDefinition>,

    /// Sandbox settings
    pub sandbox: Option<SandboxSettings>,

    /// Abort controller
    pub abort_signal: Option<tokio::sync::watch::Receiver<bool>>,
}
```

### Query (Async Stream)

```rust
pub struct Query {
    transport: ProcessTransport,
    control_tx: mpsc::Sender<ControlMessage>,
}

impl Query {
    /// Interrupt the current query
    pub async fn interrupt(&self) -> Result<()>;

    /// Change permission mode
    pub async fn set_permission_mode(&self, mode: PermissionMode) -> Result<()>;

    /// Change model
    pub async fn set_model(&self, model: Option<String>) -> Result<()>;

    /// Get available slash commands
    pub async fn supported_commands(&self) -> Result<Vec<SlashCommand>>;

    /// Get available models
    pub async fn supported_models(&self) -> Result<Vec<ModelInfo>>;

    /// Get MCP server status
    pub async fn mcp_server_status(&self) -> Result<Vec<McpServerStatus>>;
}

impl Stream for Query {
    type Item = Result<SDKMessage>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        // Read next JSONL line from stdout, parse as SDKMessage
    }
}
```

### Permission Callback

```rust
#[async_trait]
pub trait CanUseTool: Send + Sync {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        options: CanUseToolOptions,
    ) -> PermissionResult;
}

pub struct CanUseToolOptions {
    pub signal: tokio::sync::watch::Receiver<bool>,
    pub suggestions: Option<Vec<PermissionUpdate>>,
    pub blocked_path: Option<String>,
    pub decision_reason: Option<String>,
    pub tool_use_id: String,
    pub agent_id: Option<String>,
}

pub enum PermissionResult {
    Allow {
        updated_input: serde_json::Value,
        updated_permissions: Option<Vec<PermissionUpdate>>,
    },
    Deny {
        message: String,
        interrupt: bool,
    },
}
```

## Implementation Plan

### Phase 1: Core Transport (2-3 days)

1. **ProcessTransport** - Spawn CLI, manage stdin/stdout
2. **JSONL reader/writer** - Line-based JSON parsing
3. **Basic message types** - SDKUserMessage, SDKAssistantMessage, SDKResultMessage

```rust
// Example: Basic query
let mut query = sdk::query("What files are in src/?", QueryOptions::default()).await?;

while let Some(msg) = query.next().await {
    match msg? {
        SDKMessage::Assistant(m) => println!("{:?}", m.message),
        SDKMessage::Result(r) => {
            println!("Done! Cost: ${}", r.total_cost_usd);
            break;
        }
        _ => {}
    }
}
```

### Phase 2: Control Flow (1-2 days)

1. **Control requests/responses** - Bidirectional communication
2. **Permission callback** - Handle `can_use_tool` requests
3. **Query methods** - interrupt(), set_model(), etc.

```rust
// Example: Custom permission handler
struct MyPermissions;

#[async_trait]
impl CanUseTool for MyPermissions {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &serde_json::Value,
        _options: CanUseToolOptions,
    ) -> PermissionResult {
        // Deny all file deletions
        if tool_name == "Bash" {
            if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
                if cmd.contains("rm ") {
                    return PermissionResult::Deny {
                        message: "File deletion not allowed".into(),
                        interrupt: false,
                    };
                }
            }
        }
        PermissionResult::Allow {
            updated_input: input.clone(),
            updated_permissions: None,
        }
    }
}

let query = sdk::query("Clean up temp files", QueryOptions {
    can_use_tool: Some(Box::new(MyPermissions)),
    ..Default::default()
}).await?;
```

### Phase 3: Full Protocol (2-3 days)

1. **All message types** - System, StreamEvent, ToolProgress, etc.
2. **Hooks** - PreToolUse, PostToolUse, etc.
3. **MCP server config** - stdio, SSE, HTTP types
4. **Session management** - resume, continue, fork

### Phase 4: Polish (1-2 days)

1. **Error handling** - Proper error types, recovery
2. **Logging** - Debug output like DEBUG_CLAUDE_AGENT_SDK
3. **Tests** - Unit tests, integration tests with mock CLI
4. **Documentation** - Rustdoc, examples

## CLI Arguments

The SDK builds CLI arguments from QueryOptions:

```rust
fn build_args(options: &QueryOptions) -> Vec<String> {
    let mut args = vec![
        "--output-format".into(), "stream-json".into(),
        "--input-format".into(), "stream-json".into(),
        "--verbose".into(),
    ];

    if let Some(model) = &options.model {
        args.extend(["--model".into(), model.clone()]);
    }

    if let Some(mode) = &options.permission_mode {
        args.extend(["--permission-mode".into(), mode.to_string()]);
    }

    if let Some(turns) = options.max_turns {
        args.extend(["--max-turns".into(), turns.to_string()]);
    }

    if let Some(budget) = options.max_budget_usd {
        args.extend(["--max-budget-usd".into(), budget.to_string()]);
    }

    if !options.allowed_tools.is_empty() {
        args.extend(["--allowedTools".into(), options.allowed_tools.join(",")]);
    }

    if !options.disallowed_tools.is_empty() {
        args.extend(["--disallowedTools".into(), options.disallowed_tools.join(",")]);
    }

    if !options.mcp_servers.is_empty() {
        let config = serde_json::json!({ "mcpServers": options.mcp_servers });
        args.extend(["--mcp-config".into(), config.to_string()]);
    }

    if options.can_use_tool.is_some() {
        args.extend(["--permission-prompt-tool".into(), "stdio".into()]);
    }

    if let Some(resume) = &options.resume {
        args.extend(["--resume".into(), resume.clone()]);
    }

    if options.continue_session {
        args.push("--continue".into());
    }

    if !options.setting_sources.is_empty() {
        let sources: Vec<_> = options.setting_sources.iter().map(|s| s.to_string()).collect();
        args.extend(["--setting-sources".into(), sources.join(",")]);
    }

    if options.include_partial_messages {
        args.push("--include-partial-messages".into());
    }

    // ... more args

    args
}
```

## Finding the Executable

```rust
fn find_executable(options: &QueryOptions) -> Result<PathBuf> {
    // 1. Explicit path
    if let Some(path) = &options.path_to_executable {
        return Ok(path.clone());
    }

    // 2. Environment variable
    if let Ok(path) = std::env::var("CLAUDE_CODE_EXECUTABLE") {
        return Ok(PathBuf::from(path));
    }

    // 3. npx (if node available)
    if which::which("npx").is_ok() {
        // Use: npx @anthropic-ai/claude-agent-sdk/cli.js
        return Ok(PathBuf::from("npx"));
    }

    // 4. claude in PATH
    if let Ok(path) = which::which("claude") {
        return Ok(path);
    }

    Err(Error::ExecutableNotFound)
}
```

## Difference from Existing ACP Crate

We already have `crates/acp/` which implements the Agent Client Protocol for Zed integration. The key differences:

| Aspect | `crates/acp/` | `crates/claude_agent_sdk/` |
|--------|---------------|---------------------------|
| **Purpose** | Zed editor integration | General-purpose SDK |
| **Protocol** | ACP (Agent Client Protocol) | SDK JSONL protocol |
| **API Style** | Low-level, connection-based | High-level, query-based |
| **Permission handling** | Delegated to Zed | Callback-based |
| **Use case** | Editor with permission UI | Headless automation |

The `claude_agent_sdk` crate would be simpler and more focused on the SDK use case, while `acp` handles the more complex editor integration.

## Usage Example

```rust
use claude_agent_sdk::{query, QueryOptions, PermissionMode, SDKMessage};
use futures::StreamExt;

#[tokio::main]
async fn main() -> Result<()> {
    // Simple query
    let mut q = query("Explain the main function in src/main.rs", QueryOptions {
        model: Some("claude-sonnet-4-5-20250929".into()),
        permission_mode: Some(PermissionMode::AcceptEdits),
        max_turns: Some(10),
        ..Default::default()
    }).await?;

    while let Some(msg) = q.next().await {
        match msg? {
            SDKMessage::Assistant(m) => {
                // Print assistant response
                for block in &m.message.content {
                    if let ContentBlock::Text { text } = block {
                        print!("{}", text);
                    }
                }
            }
            SDKMessage::Result(r) => {
                println!("\n\nCost: ${:.4}", r.total_cost_usd);
                println!("Turns: {}", r.num_turns);
                break;
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Dependencies

```toml
[package]
name = "claude_agent_sdk"
version = "0.1.0"
edition = "2024"

[dependencies]
tokio = { version = "1", features = ["process", "io-util", "sync", "macros"] }
tokio-stream = "0.1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
futures = "0.3"
async-trait = "0.1"
thiserror = "2"
tracing = "0.1"
uuid = { version = "1", features = ["v4"] }
which = "7"

[dev-dependencies]
tokio-test = "0.4"
```

## Testing Strategy

1. **Unit tests** - Test serialization/deserialization of all message types
2. **Mock CLI** - Create a mock CLI that responds with canned responses
3. **Integration tests** - Real CLI tests (require ANTHROPIC_API_KEY)

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_result_message_parsing() {
        let json = r#"{
            "type": "result",
            "subtype": "success",
            "duration_ms": 1234,
            "duration_api_ms": 800,
            "is_error": false,
            "num_turns": 3,
            "result": "Done!",
            "total_cost_usd": 0.003,
            "usage": {...},
            "session_id": "abc123",
            "uuid": "..."
        }"#;

        let msg: StdoutMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, StdoutMessage::Result(_)));
    }
}
```

## Timeline

| Phase | Duration | Deliverable |
|-------|----------|-------------|
| Phase 1: Core Transport | 2-3 days | Basic query() that streams messages |
| Phase 2: Control Flow | 1-2 days | Permission callbacks, interrupt() |
| Phase 3: Full Protocol | 2-3 days | All message types, hooks, MCP |
| Phase 4: Polish | 1-2 days | Tests, docs, error handling |
| **Total** | **~1-2 weeks** | Production-ready SDK |

## Open Questions

1. **Should we support in-process MCP servers?** The TS SDK can run MCP tools in the same process. For Rust, we'd need a way to handle MCP requests coming from the CLI.

2. **Native binary vs Node?** Claude Code now has a native binary option. Should we prefer it over node/bun?

3. **Relationship to `crates/acp/`?** Should they share types? The protocol is different but some types overlap.

4. **Streaming input?** The TS SDK supports streaming input (AsyncIterable<SDKUserMessage>). Do we need this for multi-turn conversations?

5. **Do we even need this?** We already have `crates/acp/` for ACP protocol. The SDK JSONL protocol is simpler but different. Options:
   - Use SDK protocol for headless automation (simpler)
   - Use ACP protocol for interactive UI integration (more features)
   - Support both

## Alternatives to Building This

### Option 1: Just Use the TypeScript SDK via FFI/IPC

Instead of building a native Rust SDK, we could:
- Spawn a Node.js process running the TS SDK
- Communicate with it via IPC or stdio
- Less work, but adds Node.js dependency

### Option 2: Use the CLI Directly

For simple use cases, just spawn `claude -p "prompt" --output-format json`:
```rust
let output = Command::new("claude")
    .args(["-p", prompt, "--output-format", "json"])
    .output()?;
let result: SDKResultMessage = serde_json::from_slice(&output.stdout)?;
```

This works for one-shot queries but doesn't support:
- Streaming
- Permission callbacks
- Session resume
- Multi-turn conversations

### Option 3: Contribute to Anthropic

If Anthropic open-sources the SDK or publishes the protocol spec, we could contribute official Rust bindings. Worth asking them!

## Conclusion

Building a Rust Claude Agent SDK is feasible and relatively straightforward (~1-2 weeks). The main work is:

1. **Type definitions** - Port all the serde types from the TypeScript `.d.ts` files
2. **ProcessTransport** - Spawn CLI, handle stdin/stdout
3. **Query stream** - Async iterator over messages
4. **Permission flow** - Handle bidirectional control messages

The legal situation appears fine for building language bindings that use the official CLI, but verify with Anthropic if distributing commercially.
