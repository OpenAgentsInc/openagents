# Autopilot Crate Implementation Plan

**Date:** 2025-12-19
**Task:** Create `crates/autopilot/` CLI to run tasks via Claude Agent SDK and log trajectories

---

## Overview

Build a CLI tool that:
1. Accepts a prompt/task
2. Runs it via Claude Agent SDK
3. Streams messages and collects trajectory
4. Saves output as both `.rlog` and `.json` to `docs/logs/YYYYMMDD/HHMM-slug.{rlog,json}`
5. Generates analysis after running

---

## Crate Structure

```
crates/autopilot/
├── Cargo.toml
└── src/
    ├── main.rs          # CLI entry point with clap
    ├── lib.rs           # TrajectoryCollector (SdkMessage → Steps)
    ├── trajectory.rs    # Data structures
    ├── rlog.rs          # rlog format writer
    └── timestamp.rs     # Central US time utilities
```

---

## CLI Interface

```bash
cargo autopilot run "Fix the failing test" --model claude-sonnet-4-5-20250929
cargo autopilot run "List all Rust files" --max-turns 5 --verbose
```

**Arguments:**
- `prompt` (required) - The task to execute
- `--cwd` - Working directory (default: current)
- `--model` - Model name (default: claude-sonnet-4-5-20250929)
- `--max-turns` - Max turns (default: 50)
- `--max-budget` - Max budget USD (default: 5.0)
- `--output-dir` - Output dir (default: docs/logs/YYYYMMDD/)
- `--slug` - Custom filename slug (auto-generated if not provided)
- `--verbose` - Show all messages
- `--dry-run` - Don't save files

---

## Output Formats

### .rlog (recorder format)
```yaml
---
format: rlog/1
id: sess_abc123
repo_sha: 7e7a980b5
model: claude-sonnet-4-5-20250929
cwd: /Users/christopherdavid/code/openagents
---

@start id=sess_abc ts=2025-12-19T20:36:00Z
u: Fix the failing test
th: Let me analyze... sig=Ep4E... tokens_in=100 tokens_out=50
t!:Read id=toolu_01 file_path=src/lib.rs → [running]
o: id=toolu_01 → [ok]
a: I found the issue...
@end tokens_in=12345 tokens_out=5678 cost_usd=0.0523
```

### .json (simple trajectory - NOT ATIF)
```json
{
  "session_id": "sess_abc123",
  "prompt": "Fix the failing test",
  "model": "claude-sonnet-4-5-20250929",
  "cwd": "/Users/christopherdavid/code/openagents",
  "repo_sha": "7e7a980b5",
  "started_at": "2025-12-19T20:36:00Z",
  "steps": [
    { "type": "user", "content": "Fix the failing test" },
    { "type": "thinking", "content": "Let me analyze...", "tokens_in": 100 },
    { "type": "tool_call", "tool": "Read", "input": {"file_path": "src/lib.rs"} },
    { "type": "tool_result", "tool_id": "toolu_01", "success": true },
    { "type": "assistant", "content": "I found the issue..." }
  ],
  "usage": { "input_tokens": 12345, "output_tokens": 5678, "cost_usd": 0.0523 },
  "result": { "success": true, "duration_ms": 40000 }
}
```

---

## Files to Create/Modify

| File | Action |
|------|--------|
| `Cargo.toml` (root) | Add `crates/autopilot` to workspace members |
| `.cargo/config.toml` | Add `autopilot = "run -p autopilot --"` alias |
| `crates/autopilot/Cargo.toml` | Create with dependencies |
| `crates/autopilot/src/main.rs` | CLI with clap, execution flow |
| `crates/autopilot/src/lib.rs` | TrajectoryCollector |
| `crates/autopilot/src/trajectory.rs` | Trajectory, Step, Usage structs |
| `crates/autopilot/src/rlog.rs` | RlogWriter |
| `crates/autopilot/src/timestamp.rs` | Central US time helpers |

---

## Key Dependencies

```toml
[dependencies]
claude-agent-sdk = { path = "../claude-agent-sdk" }
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
futures = "0.3"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
chrono-tz = "0.10"
anyhow = "1"
colored = "2"
```

---

## Timestamp Format

Files in `docs/logs/20251219/` use HHMM format in Central US time:
- `2036-fix-test.rlog`
- `2036-fix-test.json`

```rust
use chrono_tz::America::Chicago;

fn time_prefix() -> String {
    let ct = Utc::now().with_timezone(&Chicago);
    format!("{:02}{:02}", ct.hour(), ct.minute())
}
```

---

## SdkMessage Mapping

| SdkMessage | rlog line | JSON step type |
|------------|-----------|----------------|
| `System::Init` | `@start id=...` | `system_init` |
| `User` | `u: content` | `user` |
| `Assistant` (thinking) | `th: content sig=...` | `thinking` |
| `Assistant` (text) | `a: content` | `assistant` |
| `Assistant` (tool_use) | `t!:Tool id=... → [running]` | `tool_call` |
| `User` (tool_result) | `o: id=... → [ok/error]` | `tool_result` |
| `Result` | `@end tokens_in=... cost_usd=...` | (in result field) |

---

## Implementation Order

1. Create `crates/autopilot/Cargo.toml`
2. Add to workspace `Cargo.toml`
3. Add alias to `.cargo/config.toml`
4. Implement `timestamp.rs` - Central US time utilities
5. Implement `trajectory.rs` - Data structures
6. Implement `rlog.rs` - rlog format writer
7. Implement `lib.rs` - TrajectoryCollector
8. Implement `main.rs` - CLI and execution
9. Test with `cargo autopilot run "say hi"`
10. Generate test logs in `docs/logs/20251219/`
11. Write analysis of logs in same folder with current timestamp
12. Commit and create PR

---

## Test Runs to Generate

After implementation, run these to generate logs:
1. `cargo autopilot run "Say hello and list files in current directory" --max-turns 3`
2. `cargo autopilot run "Read the README.md file and summarize it" --max-turns 5`

Review generated `.rlog` and `.json` files, then write analysis as `HHMM-autopilot-analysis.md`.

---

## Critical Reference Files

- `/Users/christopherdavid/code/openagents/crates/claude-agent-sdk/examples/test_query.rs` - Example SDK usage
- `/Users/christopherdavid/code/openagents/crates/claude-agent-sdk/src/protocol/messages.rs` - SdkMessage types
- `/Users/christopherdavid/code/openagents/crates/recorder/src/lib.rs` - rlog format spec
