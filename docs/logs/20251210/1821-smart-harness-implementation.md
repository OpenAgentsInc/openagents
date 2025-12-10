# Smart Orchestration Harness Implementation

**Date:** 2025-12-10 18:21
**Author:** MechaCoder

## Problem

The Apple Foundation Model is too weak to follow workflow instructions. Despite explicit prompts saying "MUST call verify_progress after EVERY write_file", it doesn't. It writes files but never verifies, so it never gets feedback to improve.

The FM was:
- Writing the same regex repeatedly without checking results
- Never calling verify_progress voluntarily
- Wasting all 10 turns on redundant writes
- Never seeing test feedback to iterate

## Solution

**Don't rely on the FM to choose tools correctly. Build a smarter harness that enforces the workflow deterministically.**

The harness is now the smart part, not the FM.

## Implementation

### Strategy 1: Auto-Verify After Write (orchestrator.rs:530-616)

After every successful `write_file`, the orchestrator automatically runs `verify_progress`:

```rust
// AUTO-VERIFY: Automatically run verify_progress after write_file
// The FM is too weak to reliably call verify after writes, so we
// enforce it deterministically. This guarantees feedback loop.
if action.tool_name == "write_file" && result.success {
    tracing::debug!("Auto-verifying after write_file");

    let auto_verify = FMAction {
        tool_name: "verify_progress".to_string(),
        tool_args: serde_json::json!({}),
        reasoning: Some("Automatic post-write verification".to_string()),
    };

    let verify_result = self.execute_action(&auto_verify, &task.verification).await?;
    // ... handle result, update state, check completion
}
```

Key behaviors:
- Tracks auto-verify with `[AUTO]` prefix in previous_actions
- Handles verification result: updates progress, checks completion, emits events
- FM now always sees feedback immediately after writing
- Can still achieve early success if auto-verify passes all tests

### Strategy 2: Tool Sequencing Rules (monitor.rs:136-184)

Added `check_tool_sequence()` as highest priority validation rule:

```rust
fn check_tool_sequence(ctx: &ActionContext) -> Option<MonitorDecision> {
    // Rule 1: Can't call write_file twice without verify in between
    // (Backup for auto-verify - if FM somehow bypasses, this catches it)
    if ctx.tool_name == "write_file" && last_action.contains("write_file") {
        if !last_action.contains("verify_progress") && !last_action.contains("[AUTO]") {
            return Some(MonitorDecision::deny_with_suggestion(...));
        }
    }

    // Rule 2: Can't read_file more than twice for same path
    // Prevents FM from repeatedly trying to read a file that doesn't exist
    if ctx.tool_name == "read_file" {
        let read_count = ...;
        if read_count >= 2 {
            return Some(MonitorDecision::deny_with_suggestion(...));
        }
    }
}
```

## Test Results

Before (FM never verified):
```
Turn 1: write_file -> OK
Turn 2: write_file -> OK (same content)
Turn 3: write_file -> OK (same content)
... never calls verify_progress, never gets feedback
```

After (harness enforces verification):
```
Turn 4: write_file(/app/regex.txt) -> OK
        [AUTO] verify_progress() -> FAILED: 12/12 tests
Turn 7: write_file(/app/regex.txt) -> OK
        [AUTO] verify_progress() -> FAILED: 12/12 tests
```

FM now sees test results after every write without having to "choose" to call verify_progress.

## Files Changed

| File | Changes |
|------|---------|
| `crates/hillclimber/src/orchestrator.rs` | +86 lines: Auto-verify after write_file |
| `crates/hillclimber/src/monitor.rs` | +48 lines: Tool sequencing rules, 4 new tests |
| `crates/hillclimber/src/prompt.rs` | Workflow hints updates |
| `crates/hillclimber/src/decomposer.rs` | Subtask hint updates |

## Tests

All 59 hillclimber tests pass:
- `test_tool_sequence_double_write` - blocks double writes
- `test_tool_sequence_write_after_auto_verify` - allows writes after auto-verify
- `test_tool_sequence_read_limit` - blocks excessive reads
- `test_repetition_detection` - updated to use verify_progress

## Philosophy

> "The harness is the smart part, not the FM."

We're proving "architecture beats model size" - the FM doesn't need to be smart enough to follow instructions. The harness enforces the workflow deterministically.
