# Migrate TB2 Runner from claude-agent-sdk to Harbor

## The Problem

**Current implementation** (`crates/mechacoder/src/panels/docker_runner.rs`):
- Uses `claude-agent-sdk` to run Claude CLI directly
- ❌ **No ATIF trajectories saved** - can't submit to Terminal-Bench leaderboard
- ❌ **Not using official TB2 harness** (Harbor)
- ❌ **Lost the 110-turn trajectory** when it crashed

**What we need:**
- Use Harbor's `tbench` binary (the official TB2 harness)
- Automatically saves ATIF v1.4 trajectories in standard format
- Required for Terminal-Bench leaderboard submissions

## Harbor Architecture (Already Exists!)

We already have `crates/harbor/` with:

**`crates/harbor/src/bin/tbench.rs`** - The TB2 wrapper CLI:
- Runs Claude CLI with `--output-format stream-json`
- Parses output into ATIF steps in real-time
- Saves three files:
  - `trajectory.json` - ATIF v1.4 complete trajectory (THIS IS WHAT WE NEED)
  - `events.jsonl` - Streaming events during execution
  - `metrics.json` - Token usage, cost, timing stats
- Has `--stream` flag to emit JSON events to stdout for UI consumption

**`crates/harbor/src/lib.rs`** - ATIF types and event handling:
- Complete ATIF v1.4 schema in Rust
- `TrajectoryBuilder` for constructing trajectories
- `EventRecorder` for writing events.jsonl
- `StreamEvent` enum for UI consumption

## Architecture Decision

**Keep TWO runners:**
1. **docker_runner.rs** - For non-TB2 runs, future use cases, custom agents
2. **harbor_runner.rs** - NEW, specifically for Terminal-Bench tasks

**Why Harbor for TB2:**
- ✅ Guarantees isolation (no contamination from host computer)
- ✅ Official TB2 harness (required for leaderboard)
- ✅ ATIF trajectory saving (automatic, correct format)
- ✅ Better credential handling (through Harbor's agent framework)

## Implementation Plan

### Phase 1: Create HarborRunner for TB2

**NEW File:** `crates/mechacoder/src/panels/harbor_runner.rs`

**Spawns Harbor's tbench binary:**
```rust
// Spawn tbench CLI with --stream flag
let mut child = tokio::process::Command::new("cargo")
    .args(&[
        "run", "--bin", "tbench", "--",
        "--instruction", &instruction,
        "--output-dir", &output_dir,
        "--max-turns", &config.max_turns.to_string(),
        "--stream",  // Emits JSON events to stdout
    ])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()?;

// Parse JSON events from stdout
let stdout = BufReader::new(child.stdout.take().unwrap());
let mut lines = stdout.lines();

while let Some(line) = lines.next_line().await? {
    let event: StreamEvent = serde_json::from_str(&line)?;
    match event {
        StreamEvent::RunStart { session_id, .. } => { /* ... */ }
        StreamEvent::Assistant { turn, content, .. } => {
            event_tx.send(DockerEvent::AssistantMessage { text: content, turn })?;
        }
        StreamEvent::ToolUse { name, id, .. } => {
            event_tx.send(DockerEvent::ToolUse { tool_name: name, tool_id: id })?;
        }
        StreamEvent::Complete { metrics, .. } => {
            // Run finished, trajectory.json saved
            cost_usd = metrics.total_cost_usd;
            turns = metrics.total_turns;
        }
    }
}
```

**Benefits:**
- ✅ ATIF trajectory automatically saved to `output_dir/trajectory.json`
- ✅ Can submit to leaderboard
- ✅ Official TB2 harness (matches what leaderboard uses)
- ✅ Streaming events still work for UI

### Phase 2: Update output directory structure

**Current:**
```
/tmp/.tmpXXXXXX/
  ├── regex.txt          # Solution
  └── logs/
      └── agent/
```

**New with Harbor:**
```
/tmp/.tmpXXXXXX/
  ├── regex.txt          # Solution (still here)
  ├── trajectory.json    # ATIF v1.4 trajectory ✅ THIS IS NEW
  ├── events.jsonl       # Streaming events log
  └── metrics.json       # Summary stats
```

**For leaderboard submission:**
```
results/trajectories/
  └── {task-id}/
      └── {session-id}/
          ├── trajectory.json    # Copy here for git commit
          ├── events.jsonl
          └── metrics.json
```

### Phase 3: Add result copying to persistent location

After run completes, copy trajectory to git-tracked location:

```rust
// After run completes
let results_dir = PathBuf::from("results/trajectories")
    .join(&config.task.id)
    .join(&session_id);

std::fs::create_dir_all(&results_dir)?;
std::fs::copy(
    workspace_dir.join("trajectory.json"),
    results_dir.join("trajectory.json")
)?;
std::fs::copy(
    workspace_dir.join("metrics.json"),
    results_dir.join("metrics.json")
)?;
```

### Phase 4: Update GymPanel to use HarborRunner for TB2

**File:** `crates/mechacoder/src/screen.rs`

**Current:**
```rust
match event {
    GymPanelEvent::StartTB2Run { run_id, task, model } => {
        // Uses DockerRunner
        let docker_runner = DockerRunner::new();
        docker_runner.run_claude(&config, event_tx, abort_rx).await
    }
}
```

**New:**
```rust
match event {
    GymPanelEvent::StartTB2Run { run_id, task, model } => {
        // Use HarborRunner for TB2 tasks
        let harbor_runner = HarborRunner::new();
        harbor_runner.run_tbench(&config, event_tx, abort_rx).await
        // Trajectory saved to results/trajectories/{task}/{session}/
    }
}
```

### Phase 5: Update event types

**Keep both:**
- `DockerEvent` - for docker_runner.rs (non-TB2 runs)
- `HarborEvent` - NEW, wraps Harbor's `StreamEvent`

**TB2RunnerEvent** stays the same - both runners convert to it:
```rust
// docker_runner.rs
DockerEvent → TB2RunnerEvent

// harbor_runner.rs (NEW)
HarborEvent (StreamEvent) → TB2RunnerEvent
```

## Files to Modify

| File | Change |
|------|--------|
| `crates/mechacoder/src/panels/harbor_runner.rs` | NEW - Harbor-based TB2 runner (~150 lines) |
| `crates/mechacoder/src/panels/mod.rs` | Export HarborRunner |
| `crates/mechacoder/src/screen.rs` | Use HarborRunner for TB2, add trajectory copying |
| `crates/harbor/Cargo.toml` | Ensure tbench binary is built |
| `crates/harbor/src/bin/tbench.rs` | Verify --stream flag exists |

**Keep unchanged:**
| File | Status |
|------|--------|
| `crates/mechacoder/src/panels/docker_runner.rs` | KEEP - for non-TB2 runs |
| `crates/mechacoder/src/panels/runner_event.rs` | KEEP - TB2RunnerEvent works for both |

## Testing

1. Build: `cargo build --bin tbench`
2. Run MechaCoder, click "Run TB2" on regex-log
3. Verify:
   - ✅ `trajectory.json` created in workspace
   - ✅ ATIF format is valid (check schema)
   - ✅ UI shows streaming messages
   - ✅ Trajectory copied to `results/trajectories/`
   - ✅ Can git commit the trajectory

## Q&A

**Q: Why not just keep claude-agent-sdk and save ATIF ourselves?**
A: Harbor's `tbench` is the **official TB2 harness**. The leaderboard uses it. We need to match their exact format and behavior.

**Q: What about the SDK's nice type-safe API?**
A: Harbor has its own Rust types (ATIF schema). We'll use those instead.

**Q: Will this break existing code?**
A: Minimal breakage - we're just changing HOW we spawn Claude, not the event flow to UI.

**Q: What about session logs from SDK?**
A: Harbor's `events.jsonl` replaces SDK session logs. It's line-delimited JSON of every event.

**Q: How does Harbor handle credentials better?**
A: Harbor's `tbench` manages its own environment. We just pass ANTHROPIC_API_KEY as env var, Harbor handles the rest. No complex credential mounting like with Docker.

**Q: How does Harbor prevent contamination?**
A: Harbor spawns Claude in a clean subprocess with controlled environment. Our computer's files/config don't leak into the TB2 task workspace. This matches the official TB2 evaluation environment.

## Key Insight

Harbor's `tbench` is **not a replacement for our docker_runner** - it's a replacement for **how we invoke Claude**.

Instead of:
```rust
claude-agent-sdk::query() → Stream<SdkMessage>
```

We do:
```rust
spawn("tbench --stream") → Stream<StreamEvent>
```

The rest of our code (event handling, verification, UI updates) stays the same.

## Decision

**✅ Migrate to Harbor's `tbench` binary**

This is **required** for Terminal-Bench leaderboard submission. The 110-turn trajectory you asked about would have been saved if we were using Harbor.

