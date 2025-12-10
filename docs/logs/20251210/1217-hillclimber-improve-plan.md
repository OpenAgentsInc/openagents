# HillClimber: Smart Orchestration Harness

## The Problem

The Apple FM is too weak to follow workflow instructions. Despite explicit prompts saying "MUST call verify_progress after EVERY write_file", it doesn't. It writes files but never verifies, so it never gets feedback to improve.

## The Solution

**Don't rely on the FM to choose tools correctly. Build a smarter harness that enforces the workflow deterministically.**

Three strategies (implement in order of priority):

| Strategy | Effort | Impact |
|----------|--------|--------|
| 1. Auto-verify after write_file | 30 mins | Guarantees feedback loop |
| 2. Tool sequencing in monitor | 1-2 hours | Prevents bad sequences |
| 3. Dynamic tool filtering in prompt | 2-3 hours | Helps FM understand constraints |

---

## Strategy 1: Auto-Verify After Write (IMPLEMENT FIRST)

**File:** `crates/hillclimber/src/orchestrator.rs`

**Location:** After line 516 (inside main loop, after execute_action returns)

### Implementation

```rust
// AFTER line 516, add this block:

// Auto-run verify_progress after successful write_file
if action.tool_name == "write_file" && result.success {
    tracing::debug!("Auto-verifying after write_file");

    let auto_verify = FMAction {
        tool_name: "verify_progress".to_string(),
        tool_args: serde_json::json!({}),
        reasoning: Some("Automatic post-write verification".to_string()),
    };

    let verify_result = self.execute_action(&auto_verify, &task.verification).await?;
    let verify_display = format_action_for_display("verify_progress", &serde_json::json!({}));
    let verify_summary = if verify_result.success {
        format!("OK: {}", &verify_result.output[..verify_result.output.len().min(200)])
    } else {
        format!("FAILED: {}", verify_result.output)
    };
    state.previous_actions.push(format!("{} -> {}", verify_display, verify_summary));

    // Parse and handle verification result
    if let Some(eval) = self.parse_verification_result(&verify_result) {
        let progress = eval.progress;
        let passed = eval.passed;

        self.emitter.on_verify_complete(eval.tests_passing, eval.tests_total, progress);

        if progress > state.best_progress {
            state.best_progress = progress;
            state.turns_since_improvement = 0;
        } else {
            state.turns_since_improvement += 1;
        }

        state.last_evaluation = Some(eval);

        if passed {
            return Ok(MAPOrchestratorResult {
                passed: true,
                progress: 1.0,
                turns: state.total_turns,
                final_files: state.modified_files.clone(),
                evaluations: vec![],
            });
        }
    }
}
```

### Expected Behavior

**Before:**
```
Turn 1: write_file -> OK
Turn 2: write_file -> OK (same content)
Turn 3: write_file -> OK (same content)
... never calls verify_progress
```

**After:**
```
Turn 1: write_file -> OK
        AUTO: verify_progress -> 16/20 tests passing
Turn 2: (FM sees test results, can improve)
        write_file -> OK (improved solution)
        AUTO: verify_progress -> 18/20 tests passing
...
```

---

## Strategy 2: Tool Sequencing in Monitor (IMPLEMENT SECOND)

**File:** `crates/hillclimber/src/monitor.rs`

**Purpose:** Prevent bad tool sequences (e.g., two writes without verify, repeated reads of non-existent files)

### Implementation

Add new validation rule:

```rust
// Add new function before VALIDATION_RULES

/// Enforce tool sequencing rules
fn check_tool_sequence(ctx: &ActionContext) -> Option<MonitorDecision> {
    if ctx.previous_actions.is_empty() {
        return None;
    }

    let last_action = ctx.previous_actions.last().unwrap();

    // Rule 1: Can't call write_file twice without verify in between
    // (This is backup if auto-verify fails for some reason)
    if ctx.tool_name == "write_file" && last_action.contains("write_file") {
        if !last_action.contains("verify_progress") && !last_action.contains("AUTO_VERIFY") {
            return Some(MonitorDecision::deny_with_suggestion(
                "Cannot write_file twice without verifying".to_string(),
                "Call verify_progress() to check your previous changes first".to_string(),
            ));
        }
    }

    // Rule 2: Can't read_file more than twice for same path
    if ctx.tool_name == "read_file" {
        let path = ctx.args.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let read_count = ctx.previous_actions.iter()
            .filter(|a| a.contains("read_file") && a.contains(path))
            .count();

        if read_count >= 2 {
            return Some(MonitorDecision::deny_with_suggestion(
                format!("Already read {} twice", path),
                "The file either doesn't exist or you've seen its contents. Try write_file instead.".to_string(),
            ));
        }
    }

    None
}

// Add to VALIDATION_RULES array (as FIRST item for priority)
const VALIDATION_RULES: &[ValidationRule] = &[
    check_tool_sequence,        // NEW - highest priority
    check_workspace_bounds,
    check_dangerous_commands,
    check_repetition,
    check_test_before_submit,
];
```

---

## Strategy 3: Dynamic Tool Filtering (OPTIONAL)

**Files:**
- `crates/hillclimber/src/prompt.rs` - modify `build_user_prompt()`
- `crates/hillclimber/src/orchestrator.rs` - pass state to prompt builder

### Implementation

```rust
// In prompt.rs, add new function:

fn compute_available_tools(state: &ExecutionState) -> Vec<&'static str> {
    // If no file written yet, allow all tools
    if state.modified_files.is_empty() {
        return vec!["read_file", "write_file", "run_command", "verify_progress"];
    }

    // If last action was write, only allow verify
    if let Some(last) = state.previous_actions.last() {
        if last.contains("write_file") && !last.contains("verify_progress") {
            return vec!["verify_progress"];
        }
    }

    // If no progress for 3+ turns, restrict to verify and write
    if state.turns_since_improvement >= 3 {
        return vec!["write_file", "verify_progress"];
    }

    // Default: full set
    vec!["read_file", "write_file", "run_command", "verify_progress"]
}

// In build_user_prompt(), add tools section:

pub fn build_user_prompt(context: &FMContext, turn: u32, max_turns: u32, state: &ExecutionState) -> String {
    // ... existing code ...

    // Add available tools section
    let tools = compute_available_tools(state);
    sections.push(format!(
        "## Available Tools (this turn)\n\n{}",
        tools.iter().map(|t| format!("- {}", t)).collect::<Vec<_>>().join("\n")
    ));

    sections.join("\n\n")
}
```

---

## Files to Modify

| File | Change | Lines |
|------|--------|-------|
| `crates/hillclimber/src/orchestrator.rs` | Add auto-verify after write_file | ~520 |
| `crates/hillclimber/src/monitor.rs` | Add tool sequencing rule | ~165 |
| `crates/hillclimber/src/prompt.rs` | (Optional) Dynamic tools | ~300 |

---

## Testing

```bash
# Test with 10 turns - should see AUTO_VERIFY after every write
RUST_LOG=debug cargo run -p hillclimber -- \
  --tasks regex-log \
  --max-runs 1 \
  --max-turns 10 \
  --workspace /Users/christopherdavid/code/terminal-bench-2/regex-log \
  -v 2>&1 | grep -E "(write_file|verify_progress|AUTO)"

# Expected output should show:
# write_file(/app/regex.txt, N chars) -> OK
# verify_progress() -> OK: Tests: X/20 passing
```

---

## Success Criteria

1. Every write_file is AUTOMATICALLY followed by verify_progress
2. FM sees test results after EVERY change
3. No turns wasted on repeated writes without feedback
4. Progress tracked correctly (turns_since_improvement updates)

---

## Why This Works

The FM is weak, but it CAN improve its solution IF it sees test feedback. The problem was it never called verify_progress, so it never got feedback.

By making verify_progress automatic after every write:
- FM always sees test results
- FM can learn from failures
- FM iterates toward solution
- We don't rely on FM to "choose" the right tool

**The harness is the smart part, not the FM.**
