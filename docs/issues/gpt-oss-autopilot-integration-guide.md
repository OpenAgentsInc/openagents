# GPT-OSS Autopilot Integration Guide - Issue #2

## Status

**BLOCKED**: Cannot compile/test in runtime-only Docker container (see issue #1).

This guide documents the required changes to wire GPT-OSS into autopilot. These changes must be implemented and tested in a development environment with full Rust toolchain.

## Current State

### Already Complete ✓

1. **Dependencies**: `gpt-oss-agent` is already listed in `crates/autopilot/Cargo.toml:27`
2. **CLI Documentation**: Help text at `crates/autopilot/src/cli.rs:31` already mentions "gpt-oss"
3. **Infrastructure**:
   - `crates/local-inference/` - LocalModelBackend trait ✓
   - `crates/gpt-oss/` - HTTP client for GPT-OSS API ✓
   - `crates/gpt-oss-agent/` - Agent wrapper with tools ✓
   - `crates/acp-adapter/src/agents/gpt_oss.rs` - ACP integration ✓

### Missing ✗

Runtime support in autopilot CLI:
- Agent matching logic doesn't handle "gpt-oss" case
- No `run_gpt_oss_agent()` function
- Error message at `main.rs:1255` only mentions "claude" or "codex"

## Required Changes

### 1. Add GPT-OSS Match Case

**File**: `crates/autopilot/src/main.rs`
**Location**: Around line 1243-1257

**Current Code**:
```rust
"codex" => {
    run_codex_agent(
        &prompt,
        &cwd,
        max_turns,
        max_budget,
        &mut collector,
        verbose,
    )
    .await?;
}
_ => {
    anyhow::bail!("Unknown agent: {}. Use 'claude' or 'codex'", agent);
}
```

**Change To**:
```rust
"codex" => {
    run_codex_agent(
        &prompt,
        &cwd,
        max_turns,
        max_budget,
        &mut collector,
        verbose,
    )
    .await?;
}
"gpt-oss" => {
    run_gpt_oss_agent(
        &prompt,
        &cwd,
        &model, // Pass model explicitly for GPT-OSS
        max_turns,
        max_budget,
        &mut collector,
        verbose,
    )
    .await?;
}
_ => {
    anyhow::bail!("Unknown agent: {}. Use 'claude', 'codex', or 'gpt-oss'", agent);
}
```

### 2. Implement run_gpt_oss_agent Function

**File**: `crates/autopilot/src/main.rs`
**Location**: After `run_codex_agent()` function (around line 450-500)

**Add This Function**:
```rust
async fn run_gpt_oss_agent(
    prompt: &str,
    cwd: &Path,
    model: &str,
    max_turns: u32,
    max_budget: f64,
    collector: &mut TrajectoryCollector,
    verbose: bool,
) -> Result<()> {
    use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig};

    // Build agent configuration
    let config = GptOssAgentConfig {
        base_url: std::env::var("GPT_OSS_SERVER_URL")
            .unwrap_or_else(|_| "http://localhost:8000".to_string()),
        model: model.to_string(),
        workspace_root: cwd.to_path_buf(),
        record_trajectory: true,
    };

    // Create agent
    let agent = GptOssAgent::new(config).await?;

    // Execute prompt
    // TODO: Implement streaming/multi-turn support
    // For now, single completion:
    let response = agent.complete(prompt).await?;

    // Record in trajectory
    collector.add_assistant_message(&response);

    if verbose {
        println!("{}", response);
    }

    Ok(())
}
```

**Note**: This is a minimal implementation. Full implementation should:
- Support multi-turn conversations
- Stream responses chunk-by-chunk
- Handle tool calls
- Respect max_turns and max_budget limits
- Integrate with TrajectoryCollector properly

### 3. Add Imports

**File**: `crates/autopilot/src/main.rs`
**Location**: Top of file with other imports

**Add**:
```rust
use gpt_oss_agent::{GptOssAgent, GptOssAgentConfig};
```

### 4. Update Error Message

Already shown in change #1 - update the bail! message to include 'gpt-oss'.

## Testing Plan

Once implemented in a dev environment with build tools:

```bash
# 1. Start GPT-OSS server (llama-server)
~/code/llama.cpp/build/bin/llama-server \
  -m ~/models/gpt-oss/gpt-oss-20b-mxfp4.gguf \
  --port 8000

# 2. Test basic execution
autopilot run --agent gpt-oss --model gpt-oss-20b "List files in current directory"

# Expected: No "Unknown agent" error, task executes

# 3. Test with custom server URL
GPT_OSS_SERVER_URL=http://localhost:8081 \
autopilot run --agent gpt-oss --model gpt-oss-120b "Explain Rust ownership"

# 4. Test trajectory recording
autopilot run --agent gpt-oss --model gpt-oss-20b "Create hello.txt" --verbose

# Verify: docs/logs/YYYYMMDD/*.rlog contains GPT-OSS responses

# 5. Test with issues integration
autopilot run --agent gpt-oss --with-issues "Process issues from database"
```

## Architecture Notes

### GptOssAgent vs Claude/Codex

| Feature | Claude/Codex | GPT-OSS |
|---------|--------------|---------|
| SDK | claude-agent-sdk, codex-agent-sdk | gpt-oss-agent |
| Protocol | ACP (Agent Client Protocol) | Direct HTTP |
| Streaming | SSE via SDK | SSE via Responses API |
| Tools | Built into SDK | Native Rust (browser, python, apply_patch) |
| Config | Via SDK options | Via GptOssAgentConfig |

### Multi-turn Support

GPT-OSS requires stateful conversation management:

```rust
// Pseudocode for multi-turn
let mut session = GptOssSession::new(config);
for turn in 0..max_turns {
    let response = session.send(user_message).await?;

    // Handle tool calls if present
    if let Some(tool_call) = response.tool_call {
        let result = agent.execute_tool(tool_call).await?;
        session.add_tool_result(result);
        continue;
    }

    // Check if done
    if response.finish_reason == Some("stop") {
        break;
    }
}
```

See `crates/gpt-oss-agent/src/session.rs` for session management.

## Dependencies

All required crates are already in Cargo.toml:
- ✓ `gpt-oss-agent = { path = "../gpt-oss-agent" }` (line 27)
- ✓ `acp-adapter = { path = "../acp-adapter" }` (line 28)

No new dependencies needed.

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `GPT_OSS_SERVER_URL` | `http://localhost:8000` | URL for llama-server |
| `GPT_OSS_MODEL_PATH` | - | Path to GGUF model file (for reference) |

## Related Files

- `crates/autopilot/src/main.rs` - Main integration point
- `crates/autopilot/src/cli.rs` - CLI definition (already mentions gpt-oss)
- `crates/gpt-oss-agent/src/agent.rs` - Agent implementation
- `crates/gpt-oss-agent/src/session.rs` - Multi-turn session support
- `crates/gpt-oss/src/client.rs` - HTTP client
- `crates/local-inference/src/backend.rs` - Shared trait
- `crates/acp-adapter/src/agents/gpt_oss.rs` - ACP integration

## Next Steps

This issue is **BLOCKED** in the current environment because:
1. No C compiler (see issue #1 analysis)
2. No cargo for compilation
3. No testing possible

**Recommended approach**:
1. Implement changes in host development environment (not Docker)
2. Test with real GPT-OSS server
3. Verify multi-turn conversations work
4. Ensure trajectory recording captures GPT-OSS responses
5. Once working, rebuild Docker images with updated binary

## Success Criteria

- [ ] `autopilot run --agent gpt-oss` does NOT error with "Unknown agent"
- [ ] Single-turn completions work
- [ ] Multi-turn conversations work
- [ ] Tool calls execute correctly
- [ ] Trajectory logs contain GPT-OSS activity
- [ ] Respects max_turns and max_budget
- [ ] Environment variable GPT_OSS_SERVER_URL is honored
