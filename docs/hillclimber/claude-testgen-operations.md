# Claude TestGen Operations Guide

## Overview

This guide explains how to run Claude Code on Terminal-Bench 2 (TB2) tasks using the TestGen protocol. The TestGen protocol requires Claude to generate its own tests before solving tasks, ensuring it understands requirements before implementing.

## Architecture: Claude Agent SDK Integration

**As of December 2024, TB2 runs use the `claude-agent-sdk` Rust crate instead of spawning the `claude` CLI directly.**

### Why SDK Instead of CLI

| Aspect | Manual CLI Spawn | claude-agent-sdk |
|--------|------------------|------------------|
| **Code size** | ~200 lines manual JSONL parsing | ~100 lines (SDK handles protocol) |
| **Type safety** | ❌ Raw `serde_json::Value` | ✅ Typed `SdkMessage` enum |
| **Error handling** | ❌ Exit codes, parse errors | ✅ Rust `Result` types |
| **Streaming** | Manual line-by-line parsing | `Stream<Item = SdkMessage>` |
| **Maintenance** | Duplicate logic | Single source of truth |
| **Permission hooks** | N/A | `PermissionHandler` trait available |
| **Future features** | Manual implementation | SDK provides automatically |

### Architecture Flow

```
┌─────────────────────────────────────────────────────────────┐
│ HOST MACHINE (MechaCoder)                                    │
│                                                               │
│  1. DockerRunner::run_claude()                               │
│     └─ run_claude_with_sdk()                                 │
│        └─ query(instruction, QueryOptions)                   │
│           └─ Spawns: claude --output-format stream-json      │
│              (via SDK's ProcessTransport)                    │
│                                                               │
│  2. Claude SDK runs on HOST                                  │
│     └─ Working directory: /tmp/workspace_XXXXX               │
│     └─ Tool calls (Bash, Read, Write, etc.) → HOST filesystem│
│     └─ Creates: workspace_dir/regex.txt, testgen_tests.py   │
│                                                               │
│  3. After Claude completes:                                  │
│     └─ TB2Verifier::run_tests()                              │
│        └─ Docker run alexgshaw/regex-log:20251031            │
│           └─ Mount: workspace_dir → /app (read-only)         │
│           └─ Mount: logs_dir → /logs (read-write)            │
│           └─ Runs: bash /tests/test.sh                       │
│           └─ Writes: /logs/verifier/reward.txt (0 or 1)      │
│                                                               │
└─────────────────────────────────────────────────────────────┘

DOCKER CONTAINER (alexgshaw/regex-log:20251031)
┌─────────────────────────────────────────────────────────────┐
│  /app/             ← Volume mount (workspace_dir from host)  │
│    ├─ regex.txt    ← Claude's solution (created on host)     │
│    └─ testgen_tests.py ← Claude's tests (created on host)    │
│                                                               │
│  /tests/           ← Volume mount (TB2 task tests, read-only)│
│    ├─ test.sh      ← Verification script                     │
│    └─ test_outputs.py ← Official test cases                  │
│                                                               │
│  /logs/verifier/   ← Volume mount (logs_dir from host)       │
│    ├─ reward.txt   ← Result: "1" = pass, "0" = fail          │
│    └─ ctrf.json    ← Detailed test results                   │
└─────────────────────────────────────────────────────────────┘
```

### Key Benefits of Volume Mount Strategy

**Q: Why not run Claude CLI inside the Docker container?**

A: TB2 Docker images are minimal task environments (Python, bash, basic tools). They don't include:
- Node.js (required for Claude CLI)
- npm packages
- Claude Code CLI

Installing these at runtime would add ~30s overhead per run.

**Q: Why not execute tool calls inside the container?**

A: The volume mount strategy is functionally equivalent:
- Claude runs on host → writes to `workspace_dir/regex.txt`
- Container sees same file at `/app/regex.txt` (via volume mount)
- Tests run in isolated, reproducible TB2 environment
- No need for complex tool interception

**Benefits:**
- ✅ **Fast startup** - No Node.js/npm install in containers
- ✅ **Simple credentials** - Use host's `~/.claude/.credentials.json`
- ✅ **Type safety** - SDK provides `SdkMessage` enum, not raw JSON
- ✅ **Clean separation** - Agent (host) vs Environment (Docker)
- ✅ **Same isolation** - Docker still used for verification

### Implementation Details

**File:** `crates/mechacoder/src/panels/docker_runner.rs`

**Key method:** `run_claude_with_sdk()`

```rust
async fn run_claude_with_sdk(
    &self,
    config: &DockerRunConfig,
    event_tx: mpsc::UnboundedSender<DockerEvent>,
) -> Result<DockerRunResult, DockerError> {
    let instruction = TestGenWrapper::wrap_instruction(&config.task.instruction);

    // Build query options
    let mut options = QueryOptions::new()
        .max_turns(config.max_turns)
        .cwd(config.workspace_dir.clone())
        .dangerously_skip_permissions(true);

    options.allowed_tools = Some(ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect());
    options.env = Some(self.build_env_vars(config));

    // Run query with streaming
    let mut stream = query(instruction, options).await?;

    // Process stream
    while let Some(msg) = stream.next().await {
        match msg? {
            SdkMessage::Assistant(msg) => { /* handle assistant message */ }
            SdkMessage::ToolProgress(p) => { /* handle tool progress */ }
            SdkMessage::Result(result) => { /* extract cost, turns, success */ }
            SdkMessage::StreamEvent(_) => { /* partial content */ }
            _ => {}
        }
    }

    Ok(DockerRunResult { success, turns, cost_usd, ... })
}
```

**Environment variables passed to SDK:**
- `ANTHROPIC_API_KEY` - API authentication
- `ANTHROPIC_MODEL` - Model override (if specified)
- `CLAUDE_CONFIG_DIR` - Session log directory (host path)
- `FORCE_AUTO_BACKGROUND_TASKS=1` - Enable background tasks
- `ENABLE_BACKGROUND_TASKS=1` - Enable background tasks

**Allowed tools:**
- Bash, Edit, Write, Read, Glob, Grep, LS, WebFetch
- NotebookEdit, NotebookRead, TodoRead, TodoWrite, Agent

### Migration from Manual CLI Spawning

**Before (removed in December 2024):**
```rust
// OLD: Manual claude CLI spawning
let mut child = Command::new("claude")
    .args(&["--verbose", "--output-format", "stream-json", ...])
    .current_dir(workspace_dir)
    .spawn()?;

// Manual JSONL parsing
let mut reader = BufReader::new(stdout).lines();
while let Some(line) = reader.next_line().await? {
    if let Ok(json) = serde_json::from_str::<Value>(&line) {
        // Extract fields manually from raw JSON
        if json.get("type") == Some("result") {
            cost_usd = json.get("total_cost_usd")?.as_f64()?;
        }
    }
}
```

**After (current implementation):**
```rust
// NEW: SDK-based approach
let mut stream = query(instruction, options).await?;

// Type-safe message processing
while let Some(msg) = stream.next().await {
    match msg? {
        SdkMessage::Result(result) => {
            match result {
                SdkResultMessage::Success(s) => {
                    cost_usd = s.total_cost_usd;
                    turns = s.num_turns;
                }
                _ => { /* handle errors */ }
            }
        }
        _ => {}
    }
}
```

### Testing

**Manual test (simulates what SDK does):**
```bash
cd /tmp/test-workspace
claude --verbose --output-format stream-json \
  -p "Write a regex that matches IP addresses" \
  --max-turns 5

# Then verify with Docker
docker run --rm \
  -v $(pwd):/app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh
```

**Automated test (via MechaCoder UI):**
1. Build: `cargo build -p mechacoder`
2. Run: `./target/debug/MechaCoder`
3. Navigate to GYM panel
4. Click "Run TB2" on regex-log task
5. Observe:
   - Claude spawns via SDK ✅
   - Streaming events appear in UI ✅
   - Verification runs after completion ✅
   - Results displayed with cost/turns ✅

## Models

**ALWAYS use 4.5 model versions. Older versions are deprecated.**

| Model | Model ID | Use Case |
|-------|----------|----------|
| Haiku 4.5 | `claude-haiku-4-5-20251001` | Fast testing, iteration |
| Sonnet 4.5 | `claude-sonnet-4-5-20251022` | Balanced speed/quality |
| Opus 4.5 | `claude-opus-4-5-20251101` | Best quality, slowest |

**NEVER use these deprecated IDs:**
- ❌ `claude-sonnet-4-20250514`
- ❌ `claude-haiku-4-20250514`
- ❌ Any model ID without "4-5" in the name

## Directory Structure

For each test run, create this structure:

```
/tmp/<task-name>-test/
├── app/                    # Working directory (maps to /app in Docker)
│   ├── testgen_tests.py    # Claude-generated tests (TestGen creates this)
│   └── regex.txt           # Claude's solution (or whatever output)
└── logs/
    ├── agent/
    │   └── claude-code.txt # Full Claude output
    └── verifier/
        ├── reward.txt      # TB2 result: "1" = pass, "0" = fail
        └── ctrf.json       # Detailed test results
```

## Running Claude with TestGen

### Step 1: Setup Directories

```bash
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/app
mkdir -p /tmp/regex-log-test/logs/agent
mkdir -p /tmp/regex-log-test/logs/verifier
```

### Step 2: Run Claude

```bash
cd /tmp/regex-log-test/app

claude --verbose \
  --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "[TESTGEN WRAPPED INSTRUCTION]" \
  --max-turns 30 \
  --output-format stream-json \
  2>&1 | tee /tmp/regex-log-test/logs/agent/claude-code.txt
```

### Required Parameters

| Parameter | Value | Why |
|-----------|-------|-----|
| `--verbose` | (flag) | Required for stream-json output |
| `--dangerously-skip-permissions` | (flag) | Allows file writes without prompts |
| `--model` | `claude-haiku-4-5-20251001` | Use 4.5 models only |
| `-p` | TestGen-wrapped instruction | The task with TestGen protocol |
| `--max-turns` | 30 | Enough iterations for TestGen loop |
| `--output-format` | `stream-json` | Structured output for parsing |

### Optional Parameters

| Parameter | Default | Notes |
|-----------|---------|-------|
| `--allowedTools` | All tools | Can restrict to specific tools |
| `timeout` | None | Use `timeout 300` wrapper for 5min limit |

## TestGen Instruction Format

The instruction must be wrapped with TestGen protocol. The wrapper adds:

```
# TestGen Protocol Active

This task requires Test-Driven Development. You MUST generate your own tests
from the task description before writing the solution.

## YOUR TASK

[Original task instruction here]

## REQUIRED WORKFLOW: TestGen Protocol

### Step 1: DESCRIBE (Output this section first)
[Task analysis with acceptance criteria]

### Step 2: WRITE TESTS (Create /app/testgen_tests.py)
[Pytest tests for each criterion]

### Step 3: ITERATE (Loop until tests pass)
1. Write initial solution
2. Run: pytest /app/testgen_tests.py -v
3. If FAIL: fix solution, go to step 2
4. If PASS: you're done

## CRITICAL RULES
- NEVER read /tests/* - Those are for final verification only
- Derive tests from description ONLY
```

## Running TB2 Verification

After Claude completes, run the official TB2 tests:

```bash
# Build TB2 image if not exists
cd ~/code/terminal-bench-2/regex-log/environment
docker build -t alexgshaw/regex-log:20251031 .

# Run verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v ~/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh
```

### Check Results

```bash
# Check pass/fail (1 = pass, 0 = fail)
cat /tmp/regex-log-test/logs/verifier/reward.txt

# Check if testgen tests were created
ls -la /tmp/regex-log-test/app/testgen_tests.py

# Check solution
cat /tmp/regex-log-test/app/regex.txt
```

## Example: Full regex-log Test

```bash
# 1. Setup
rm -rf /tmp/regex-log-test
mkdir -p /tmp/regex-log-test/{app,logs/agent,logs/verifier}

# 2. Run Claude with Haiku 4.5
cd /tmp/regex-log-test/app
timeout 180 claude --verbose --dangerously-skip-permissions \
  --model claude-haiku-4-5-20251001 \
  -p "# TestGen Protocol Active
...
[full TestGen-wrapped instruction]
...
" \
  --max-turns 20 \
  --output-format stream-json \
  2>&1 | tee /tmp/regex-log-test/logs/agent/claude-code.txt

# 3. Check TestGen compliance
ls /tmp/regex-log-test/app/testgen_tests.py  # Should exist
cat /tmp/regex-log-test/app/regex.txt        # Solution

# 4. Run TB2 verification
docker run --rm \
  -v /tmp/regex-log-test/app:/app \
  -v /tmp/regex-log-test/logs:/logs \
  -v /home/christopherdavid/code/terminal-bench-2/regex-log/tests:/tests:ro \
  -w /app \
  alexgshaw/regex-log:20251031 \
  bash /tests/test.sh

# 5. Check result
cat /tmp/regex-log-test/logs/verifier/reward.txt
```

## Anti-Cheating Rules

**Claude MUST NOT:**
- Read `/tests/*` directory (TB2 official tests)
- Read `test_outputs.py`
- Hardcode solutions based on task ID
- Use knowledge from benchmark test files

**Claude MUST:**
- Derive tests from task description ONLY
- Follow DESCRIBE → WRITE TESTS → ITERATE workflow
- Create `/app/testgen_tests.py` before solution
- Iterate until self-generated tests pass

## Troubleshooting

### "model not found" or API errors
- Check model ID is exactly `claude-haiku-4-5-20251001`
- Verify ANTHROPIC_API_KEY is set

### Claude doesn't create testgen_tests.py
- Instruction wrapper may be missing
- Check TestGen protocol is in the prompt

### Timeout
- Increase timeout (default 180s may not be enough)
- Use faster model (Haiku)
- Reduce max-turns

### Docker image not found
- Build locally: `cd ~/code/terminal-bench-2/regex-log/environment && docker build -t alexgshaw/regex-log:20251031 .`
- TB2 images are NOT on Docker Hub

## File Locations

| File | Purpose |
|------|---------|
| `crates/gym/src/mechacoder/testgen_wrapper.rs` | Wraps instructions |
| `crates/gym/src/mechacoder/testgen_validator.rs` | Validates protocol |
| `.claude/skills/testgen-protocol/SKILL.md` | TestGen methodology |
| `~/code/terminal-bench-2/` | TB2 tasks directory |

## Cost Estimates

| Model | ~Cost per regex-log run |
|-------|------------------------|
| Haiku 4.5 | $0.01 - $0.05 |
| Sonnet 4.5 | $0.10 - $0.30 |
| Opus 4.5 | $0.50 - $2.00 |

Start with Haiku for testing, use Sonnet/Opus for production runs.

## WARNING: Deprecated Models in Codebase

The codebase has hardcoded `claude-sonnet-4-20250514` in many places:

```
crates/llm/src/anthropic.rs:15      - DEFAULT_MODEL
crates/cli/src/mechacoder_cmd.rs:26 - CLI default
crates/llm/src/models.rs:149        - Anthropic default
crates/harbor/src/lib.rs:47         - Harbor config
crates/gym/src/tbcc/types.rs:224    - TBCC types
crates/orchestrator/src/session.rs  - Session default
```

When running manually, ALWAYS specify `--model claude-haiku-4-5-20251001` explicitly to override these defaults.

TODO: Update all hardcoded models to 4.5 versions.
