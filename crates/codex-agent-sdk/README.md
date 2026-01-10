# Codex Agent SDK

Rust SDK for programmatically interacting with OpenAI's Codex CLI agent.

## Installation

Add to your Cargo.toml:

```toml
[dependencies]
codex-agent-sdk = { path = "../codex-agent-sdk" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## Quick Start

```rust
use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions};

#[tokio::main]
async fn main() -> Result<(), codex_agent_sdk::Error> {
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions::default());

    let turn = thread.run("What files are here?", TurnOptions::default()).await?;
    println!("{}", turn.final_response);
    Ok(())
}
```

## Configuration

### ThreadOptions

Configure how the Codex agent operates:

```rust
use codex_agent_sdk::{ThreadOptions, SandboxMode, ModelReasoningEffort};
use std::path::PathBuf;

let options = ThreadOptions {
    model: Some("gpt-4o".to_string()),
    sandbox_mode: Some(SandboxMode::WorkspaceWrite),
    working_directory: Some(PathBuf::from("/path/to/project")),
    skip_git_repo_check: false,
    model_reasoning_effort: Some(ModelReasoningEffort::Medium),
    network_access_enabled: Some(true),
    web_search_enabled: Some(false),
    approval_policy: None,
    additional_directories: vec![],
};
```

**Available options:**

- **model**: Model to use (e.g., `"gpt-4o"`, `"gpt-4o-mini"`)
- **sandbox_mode**: File system access level
  - `ReadOnly`: Can read files but not modify
  - `WorkspaceWrite`: Can write within the working directory
  - `DangerFullAccess`: Unrestricted file system access
- **working_directory**: Working directory for the agent
- **skip_git_repo_check**: Allow running outside git repos
- **model_reasoning_effort**: Control model reasoning depth
  - `Minimal`, `Low`, `Medium`, `High`, `Xhigh`
- **network_access_enabled**: Enable network requests
- **web_search_enabled**: Enable web search capability
- **approval_policy**: Control when to request user approval
  - `Never`: Never ask for approval
  - `OnRequest`: Ask when agent thinks it's needed
  - `OnFailure`: Ask after failures
  - `Untrusted`: Ask for all operations
- **additional_directories**: Extra directories to grant access to

### TurnOptions

Configure individual turns within a thread:

```rust
use codex_agent_sdk::TurnOptions;
use serde_json::json;

let options = TurnOptions {
    output_schema: Some(json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" },
            "files_modified": { "type": "array", "items": { "type": "string" } }
        },
        "required": ["summary"]
    })),
};
```

**Available options:**

- **output_schema**: JSON schema for structured output validation

## Streaming

Process events as they arrive for real-time feedback:

```rust
use codex_agent_sdk::{Codex, ThreadOptions, TurnOptions, ThreadEvent};

#[tokio::main]
async fn main() -> Result<(), codex_agent_sdk::Error> {
    let codex = Codex::new();
    let mut thread = codex.start_thread(ThreadOptions::default());

    let mut streamed = thread.run_streamed("Analyze code", TurnOptions::default()).await?;

    while let Some(event) = streamed.next().await {
        match event? {
            ThreadEvent::ItemStarted(item) => {
                println!("Started: {:?}", item.item.details);
            }
            ThreadEvent::ItemCompleted(item) => {
                println!("Completed: {:?}", item.item.details);
            }
            ThreadEvent::TurnCompleted(tc) => {
                println!("Turn finished. Tokens: {} in, {} out",
                    tc.usage.input_tokens, tc.usage.output_tokens);
            }
            _ => {}
        }
    }

    Ok(())
}
```

## Thread Management

Continue conversations across multiple turns:

```rust
let codex = Codex::new();
let mut thread = codex.start_thread(ThreadOptions::default());

// First turn
let turn1 = thread.run("Create a hello.txt file", TurnOptions::default()).await?;
println!("Response: {}", turn1.final_response);

// Second turn (in same thread context)
let turn2 = thread.run("Now read that file back to me", TurnOptions::default()).await?;
println!("Response: {}", turn2.final_response);

// Get thread ID for later resumption
if let Some(id) = thread.id() {
    println!("Thread ID: {}", id);

    // Resume later
    let mut resumed = codex.resume_thread(id, ThreadOptions::default());
    let turn3 = resumed.run("What did we do earlier?", TurnOptions::default()).await?;
}
```

## Events

The SDK emits various events during execution:

### ThreadEvent Variants

- **ThreadStarted**: Thread initialized with an ID
- **TurnStarted**: New turn beginning
- **TurnCompleted**: Turn finished successfully (includes usage stats)
- **TurnFailed**: Turn failed with error
- **ItemStarted**: Agent started a new action (tool call, reasoning, etc.)
- **ItemUpdated**: Agent action in progress
- **ItemCompleted**: Agent action finished
- **Error**: Error occurred during execution

### Item Types

Actions the agent can perform:

- **AgentMessage**: Text response from the agent
- **Reasoning**: Internal reasoning/thinking
- **CommandExecution**: Shell command execution
- **FileChange**: File modifications (create, update, delete)
- **McpToolCall**: MCP (Model Context Protocol) tool invocation
- **WebSearch**: Web search query
- **TodoList**: Task list management
- **Error**: Error during execution

## Examples

See the `examples/` directory:

- **simple.rs**: Basic query and response
- **streaming.rs**: Streaming events in real-time

Run examples:

```bash
cargo run --example simple
cargo run --example streaming
```

## Comparison with Claude Agent SDK

| Feature | Claude Agent SDK | Codex Agent SDK |
|---------|------------------|-----------------|
| CLI Command | `claude --output-format stream-json` | `codex exec --experimental-json` |
| Event Type | `SdkMessage` | `ThreadEvent` |
| Streaming | `impl Stream<Item = SdkMessage>` | `StreamedTurn` with `next()` |
| Thread Resumption | Via thread ID | Via thread ID |
| Structured Output | `output_schema` | `output_schema` |
| Sandbox Control | Not available | `SandboxMode` enum |
| Approval Policy | Not available | `ApprovalMode` enum |

Both SDKs follow similar patterns:

```rust
// Claude
let claude = ClaudeAgent::new(ClaudeOptions::default());
let stream = claude.run_stream("prompt").await?;

// Codex
let codex = Codex::new();
let mut thread = codex.start_thread(ThreadOptions::default());
let streamed = thread.run_streamed("prompt", TurnOptions::default()).await?;
```

## Bi-directional Delegation

Claude and Codex can delegate to each other:

### Claude → Codex

Claude can invoke Codex via the `/codex` skill:

```bash
# From within a Claude session
codex exec --sandbox workspace-write "Refactor the authentication module"
```

### Codex → Claude

Codex can invoke Claude via MCP tools (if configured):

```json
{
  "tool": "claude_query",
  "arguments": {
    "prompt": "Review this code for security issues"
  }
}
```

This enables:
- Claude delegating complex refactoring to Codex
- Codex delegating analysis/review to Claude
- Multi-agent workflows with different strengths

## Error Handling

```rust
use codex_agent_sdk::Error;

match thread.run("prompt", TurnOptions::default()).await {
    Ok(turn) => println!("Success: {}", turn.final_response),
    Err(Error::ExecutableNotFound(msg)) => {
        eprintln!("Codex not found: {}", msg);
        eprintln!("Install from: https://github.com/openai/codex");
    }
    Err(Error::TurnFailed(msg)) => {
        eprintln!("Turn failed: {}", msg);
    }
    Err(e) => eprintln!("Error: {}", e),
}
```

## Coder Integration

The Coder desktop application (GPU-accelerated AI coding terminal) uses this SDK for Codex backend support. When you select Codex as your backend in Coder, it uses this SDK to:

1. Start and manage Codex threads
2. Stream events in real-time
3. Map Codex events to the UI (tool calls, text responses, errors)
4. Handle permission modes and sandbox settings

**Switch backends in Coder:**
```bash
/backend codex      # Switch to Codex
/backend claude     # Switch back to Claude
/backend            # Toggle between backends
```

The status bar shows the current backend. Both backends share the same streaming UI infrastructure.

**Event mapping (Codex → Coder UI):**

| Codex Event | Coder Response |
|-------------|----------------|
| `ThreadEvent::ItemUpdated(AgentMessage)` | Text streaming chunk |
| `ThreadEvent::ItemStarted(CommandExecution)` | Tool call card (Bash) |
| `ThreadEvent::ItemCompleted(CommandExecution)` | Tool result with output |
| `ThreadEvent::ItemStarted(FileChange)` | Tool call card (Edit) |
| `ThreadEvent::ItemCompleted(FileChange)` | File modification result |
| `ThreadEvent::TurnCompleted` | Completion with token usage |
| `ThreadEvent::Error` | Error display |

## Requirements

- Codex CLI installed and in PATH
- OpenAI API key configured (via `OPENAI_API_KEY` environment variable or Codex config)

Install Codex:

```bash
# See Codex documentation for installation
# Typically: npm install -g @openai/codex
```

Verify installation:

```bash
codex --version
```

## License

MIT
