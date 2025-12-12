# TB2 Architecture Decision: claude-agent-sdk vs claude CLI

## The Question

Now that Claude runs on the HOST (not in container), should we:
1. **Keep current approach**: Spawn `claude` CLI binary directly, parse JSONL output
2. **Switch to claude-agent-sdk**: Use our Rust SDK for type safety and better APIs
3. **Hybrid**: Use SDK with custom tool executor to run tools in container

## Key Findings from Investigation

### Current Implementation (claude CLI directly)

**What it does:**
```rust
// crates/mechacoder/src/panels/docker_runner.rs:422
let mut child = Command::new("claude")
    .args(&["--verbose", "--output-format", "stream-json", ...])
    .current_dir(workspace_dir)
    .spawn()?;
```

- Spawns `claude` CLI as subprocess
- Parses JSONL output manually (lines 435-527)
- All tool calls execute on **HOST** in `workspace_dir`
- Workspace is volume-mounted to `/app` in container for verification

**Flow:**
```
HOST
  ├─ Claude CLI spawned
  ├─ Tool calls (Read/Write/Bash) → execute in workspace_dir
  └─ workspace_dir volume-mounted to /app in container

DOCKER CONTAINER
  └─ Verifier reads /app (same as workspace_dir via mount)
```

### claude-agent-sdk Architecture

**What it provides:**
```rust
// crates/claude_agent_sdk/src/lib.rs
pub async fn query(
    prompt: impl Into<String>,
    options: QueryOptions,
) -> Result<impl Stream<Item = Result<SdkMessage>>>
```

**How it works:**
- Also spawns `claude` CLI as subprocess (via `ProcessTransport`)
- Communicates via JSONL over stdin/stdout
- Provides **type-safe** Rust API instead of manual JSON parsing
- Has `PermissionHandler` trait to intercept tool calls BEFORE execution

**Key insight:** Both approaches spawn the same `claude` CLI binary. The SDK just provides a nicer Rust interface.

### PermissionHandler Interception

**THIS IS THE KEY CAPABILITY:**

```rust
// crates/claude_agent_sdk/src/permissions.rs
pub trait PermissionHandler {
    async fn can_use_tool(
        &self,
        tool_name: &str,
        input: &Value,  // tool input JSON
        ...
    ) -> Result<PermissionResult>;
}

// Response options:
PermissionResult::Allow { updated_input } // Execute tool (optionally modify input)
PermissionResult::Deny { message }         // Block tool
```

**When called:**
- BEFORE each tool executes
- Claude CLI sends `CanUseToolRequest` to SDK
- SDK calls your `PermissionHandler`
- You respond Allow/Deny
- CLI executes tool only if allowed

**Can we execute tools in container?**

Sort of. Two approaches:

#### Option A: Intercept + Docker Exec (Custom Handler)
```rust
pub struct ContainerToolHandler {
    container_id: String,
}

impl PermissionHandler for ContainerToolHandler {
    async fn can_use_tool(&self, tool_name: &str, input: &Value, ...) -> Result<PermissionResult> {
        match tool_name {
            "Bash" => {
                let cmd = input["command"].as_str()?;
                // Execute in container instead of host
                let output = docker_exec(&self.container_id, cmd).await?;
                // Still return Allow (but we already executed it)
                Ok(PermissionResult::allow(input.clone()))
            }
            "Read" => {
                let path = input["file_path"].as_str()?;
                let content = docker_exec(&self.container_id, &format!("cat {}", path)).await?;
                // Return modified result? Not really possible with PermissionHandler
                Ok(PermissionResult::allow(input.clone()))
            }
            _ => Ok(PermissionResult::allow(input.clone()))
        }
    }
}
```

**Problem:** PermissionHandler can only Allow/Deny, not replace results. The CLI still executes the tool after you respond. So you'd execute TWICE (once in container, once on host).

#### Option B: Run Entire CLI in Container
```rust
let options = QueryOptions::new()
    .executable(ExecutableConfig {
        executable: Some("docker".to_string()),
        executable_args: vec![
            "exec", "-i", &container_id,
            "claude", "--output-format", "stream-json"
        ],
    });
```

**Problem:** Container needs Claude CLI + Node.js installed. TB2 images don't have these (minimal task environments).

### Current Volume Mount Strategy (BEST)

**The existing approach is actually optimal:**

```
HOST (workspace_dir)              DOCKER (/app)
┌─────────────────┐              ┌─────────────────┐
│ regex.txt       │ ←─ mount ──→ │ regex.txt       │
│ solution.py     │              │ solution.py     │
└─────────────────┘              └─────────────────┘
        ▲                                 │
        │                                 │
    Claude SDK                        Verifier
    (tool calls)                      (test.sh)
```

**Why this works:**
1. Claude SDK on host writes to `workspace_dir/regex.txt`
2. Volume mount makes it visible at `/app/regex.txt` in container
3. Verifier reads `/app/regex.txt` and runs tests
4. **Net effect:** Tools execute on host, results visible in container

This is functionally equivalent to "tools in container" because:
- Files created on host appear in container
- Commands on host can prep the workspace
- Container only needs to READ and VERIFY, not BUILD

## The Three Approaches

### Approach 1: Keep Current (spawn claude CLI directly)

**Pros:**
- Simple, straightforward
- Already working (after fixing imports)
- No extra dependencies
- Minimal code

**Cons:**
- Manual JSONL parsing (error-prone)
- No type safety for messages
- Harder to maintain
- Duplicate effort (we have SDK but don't use it)

**Code size:** ~200 lines in docker_runner.rs

### Approach 2: Switch to claude-agent-sdk

**Pros:**
- ✅ **Type safety** - `SdkMessage` enum instead of raw JSON
- ✅ **Better APIs** - Proper Rust types, not serde_json::Value
- ✅ **Maintained SDK** - Use our own crate instead of duplicating logic
- ✅ **Streaming** - Clean `Stream<Item = SdkMessage>` interface
- ✅ **Permission hooks** - Can intercept tools if needed later
- ✅ **Error handling** - Proper Result types
- ✅ **Less code** - SDK handles parsing/protocol

**Cons:**
- Slightly more complex setup
- Extra dependency (but we already have the crate)

**Code size:** ~100 lines in docker_runner.rs (SDK handles the rest)

**Example implementation:**
```rust
use claude_agent_sdk::{query, QueryOptions, ExecutableConfig, AllowAllPermissions};

async fn run_claude_with_sdk(
    instruction: &str,
    workspace_dir: &Path,
    max_turns: u32,
) -> Result<DockerRunResult> {
    let options = QueryOptions::new()
        .max_turns(Some(max_turns))
        .permission_handler(AllowAllPermissions)
        .allowed_tools(vec!["Bash", "Read", "Write", "Edit", "Glob", "Grep", ...])
        .executable(ExecutableConfig {
            working_directory: Some(workspace_dir.to_path_buf()),
            ..Default::default()
        });

    let mut stream = query(instruction, options).await?;

    let mut turns = 0;
    let mut cost_usd = 0.0;

    while let Some(msg) = stream.next().await {
        match msg? {
            SdkMessage::Assistant(msg) => {
                // Handle assistant message
                turns += 1;
            }
            SdkMessage::ToolProgress(progress) => {
                // Handle tool progress
                event_tx.send(DockerEvent::ToolExecuting {
                    tool_name: progress.tool_name,
                    elapsed: progress.elapsed_time_seconds,
                })?;
            }
            SdkMessage::Result(result) => {
                // Final result
                cost_usd = result.cost_usd;
                break;
            }
            _ => {}
        }
    }

    Ok(DockerRunResult { turns, cost_usd, success: true })
}
```

### Approach 3: SDK + Custom PermissionHandler (Overkill)

**Pros:**
- Fine-grained control over tool execution
- Can log/monitor/modify tool calls
- Could theoretically execute in container

**Cons:**
- Much more complex
- PermissionHandler can't replace tool results (only allow/deny)
- Volume mount already solves the problem
- Not worth the complexity

**Verdict:** Unnecessary for current needs

## Recommendation: **Switch to claude-agent-sdk (Approach 2)**

### Why?

1. **Type Safety**
   - Current: `serde_json::Value` everywhere
   - SDK: Proper `SdkMessage`, `SdkAssistantMessage`, `SdkToolProgress` types

2. **Less Code**
   - Current: 200+ lines of manual JSONL parsing
   - SDK: ~100 lines, SDK handles protocol

3. **Better Errors**
   - Current: Parse errors, exit code guessing
   - SDK: Typed errors, proper Result propagation

4. **Maintenance**
   - Current: Duplicate logic (we maintain SDK but don't use it)
   - SDK: Single source of truth

5. **Future-Proof**
   - SDK supports sessions, MCP, all CLI features
   - Easy to add permission handling later if needed

6. **Streaming Still Works**
   - SDK returns `Stream<Item = SdkMessage>`
   - Can still send DockerEvent::ToolExecuting, etc.

### What About Tool Execution in Container?

**Answer: Volume mount is the right approach.**

- Tools execute on host
- Files written to `workspace_dir`
- Container sees same files at `/app` (via volume mount)
- Verifier runs in clean isolated environment

This is **functionally equivalent** to tools in container because:
- Container sees all artifacts
- Host can't contaminate container environment
- Tests run in reproducible TB2 image

No need for complex tool interception.

## Implementation Plan

### Phase 1: Add claude-agent-sdk Integration

**File:** `crates/mechacoder/Cargo.toml`

Add dependency:
```toml
[dependencies]
claude-agent-sdk = { path = "../claude-agent-sdk" }
```

### Phase 2: Replace run_claude_on_host with SDK Version

**File:** `crates/mechacoder/src/panels/docker_runner.rs`

**Remove:**
- `run_claude_on_host()` method (lines 401-538)
- Manual JSONL parsing
- `build_command()` helper

**Add:**
```rust
use claude_agent_sdk::{
    query, QueryOptions, ExecutableConfig, AllowAllPermissions,
    SdkMessage, SdkAssistantMessage, SdkToolProgressMessage,
};

async fn run_claude_with_sdk(
    &self,
    config: &DockerRunConfig,
    event_tx: mpsc::UnboundedSender<DockerEvent>,
) -> Result<DockerRunResult, DockerError> {
    let instruction = TestGenWrapper::wrap_instruction(&config.task.instruction);

    // Build query options
    let mut options = QueryOptions::new()
        .max_turns(Some(config.max_turns))
        .permission_handler(AllowAllPermissions)
        .allowed_tools(ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect())
        .executable(ExecutableConfig {
            working_directory: Some(config.workspace_dir.clone()),
            environment: self.build_env_vars(config),
            ..Default::default()
        });

    if let Some(model) = &config.model {
        options = options.model(model.clone());
    }

    // Run query with streaming
    let mut stream = query(instruction, options)
        .await
        .map_err(|e| DockerError::ProcessSpawn(format!("SDK error: {}", e)))?;

    let mut turns = 0;
    let mut cost_usd = 0.0;
    let mut success = false;

    // Process stream
    while let Some(msg) = stream.next().await {
        match msg.map_err(|e| DockerError::StreamRead(e.to_string()))? {
            SdkMessage::Assistant(_) => {
                turns += 1;
                event_tx.send(DockerEvent::TurnComplete { turn: turns })?;
            }
            SdkMessage::ToolProgress(p) => {
                event_tx.send(DockerEvent::ToolExecuting {
                    tool_name: p.tool_name,
                    elapsed: p.elapsed_time_seconds,
                })?;
            }
            SdkMessage::Result(result) => {
                cost_usd = result.cost_usd.unwrap_or(0.0);
                success = true;
                log::info!("TB2: Claude completed - {} turns, ${:.4}", turns, cost_usd);
            }
            SdkMessage::StreamEvent(_) => {
                // Partial content, can ignore or log
            }
            _ => {}
        }
    }

    Ok(DockerRunResult {
        success,
        turns,
        cost_usd,
        error: None,
    })
}

fn build_env_vars(&self, config: &DockerRunConfig) -> HashMap<String, String> {
    let mut env = HashMap::new();

    // API key
    if let Ok(key) = std::env::var("ANTHROPIC_API_KEY") {
        env.insert("ANTHROPIC_API_KEY".to_string(), key);
    }

    // Model override
    if let Some(model) = &config.model {
        env.insert("ANTHROPIC_MODEL".to_string(), model.clone());
    }

    // Session logging
    env.insert("CLAUDE_CONFIG_DIR".to_string(),
        config.logs_dir.join("agent/sessions").display().to_string());

    // Background tasks
    env.insert("FORCE_AUTO_BACKGROUND_TASKS".to_string(), "1".to_string());
    env.insert("ENABLE_BACKGROUND_TASKS".to_string(), "1".to_string());

    env
}
```

### Phase 3: Update run_claude to Use SDK

**File:** `crates/mechacoder/src/panels/docker_runner.rs`

Replace implementation (lines 215-236):
```rust
pub async fn run_claude(
    &self,
    config: &DockerRunConfig,
    event_tx: mpsc::UnboundedSender<DockerEvent>,
    _abort_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<DockerRunResult, DockerError> {
    log::info!("TB2: Running Claude via SDK on host");
    log::info!("  Workspace: {}", config.workspace_dir.display());
    log::info!("  Max turns: {}", config.max_turns);

    // Use SDK to run Claude
    self.run_claude_with_sdk(config, event_tx).await
}
```

### Phase 4: Clean Up Unused Code

**Remove from docker_runner.rs:**
- `build_claude_args()` (no longer needed)
- Manual JSONL parsing helpers
- Custom subprocess stdout/stderr reading

**Keep:**
- `DockerRunResult` struct
- `DockerEvent` enum
- `DockerRunConfig` struct

### Phase 5: Test

**Steps:**
1. `cargo build -p mechacoder`
2. Run MechaCoder
3. Click "Run TB2" on regex-log
4. Verify:
   - Claude spawns via SDK ✅
   - Streaming events work (ToolExecuting, TurnComplete) ✅
   - Workspace files created correctly ✅
   - Verification passes ✅
   - UI shows results ✅

## Benefits Summary

| Feature | Current (CLI) | With SDK |
|---------|--------------|----------|
| Type safety | ❌ Raw JSON | ✅ Rust types |
| Code size | ~200 lines | ~100 lines |
| Error handling | Exit codes | Typed errors |
| Streaming | Manual parse | Stream trait |
| Maintenance | Duplicate logic | Use SDK |
| Tool hooks | N/A | PermissionHandler |
| Future features | Manual impl | SDK provides |

## Q&A

**Q: Does SDK execute tools in container?**
A: No, but volume mount makes it equivalent. Host writes to workspace_dir, container sees /app.

**Q: Can we intercept tool calls?**
A: Yes, via PermissionHandler. But not needed for current use case.

**Q: Is this more complex?**
A: Initial setup slightly more, but overall less code and better APIs.

**Q: What if claude CLI not installed?**
A: Same issue as current approach. SDK just wraps CLI.

**Q: Performance impact?**
A: None. Both spawn same `claude` binary. SDK is just a nicer interface.

## Files to Modify

| File | Change |
|------|--------|
| `crates/mechacoder/Cargo.toml` | Add claude-agent-sdk dependency |
| `crates/mechacoder/src/panels/docker_runner.rs` | Replace run_claude_on_host with SDK version (~150 lines) |
| `crates/mechacoder/src/panels/docker_runner.rs` | Add SDK imports, remove manual parsing |

## Decision

**✅ Recommend switching to claude-agent-sdk**

Provides better type safety, cleaner code, and uses our existing SDK instead of duplicating logic.


