# Full Auto Mode for Autopilot

## Problem Summary

1. **No looping**: Autopilot stops after completing one issue instead of continuing
2. **No discovery**: When issues run out, the agent stops instead of finding new work
3. **CLI output**: Progress messages overwrite each other using `\r` (carriage return)

## Vision: `--full-auto` Mode

A true autonomous agent that:
1. Executes all ready issues in the queue
2. When the queue is empty, analyzes the codebase to discover what should come next
3. Creates new issues for the logical next steps
4. Executes those new issues
5. Repeats until explicitly stopped or budget exhausted

## Solution

### Fix 1: Add `--full-auto` flag

**Files to modify:**
- `crates/autopilot/src/main.rs`

**Implementation:**

1. Add `#[arg(long)] full_auto: bool` to Run command

2. Modify the prompt when `--full-auto` is enabled to append:
```
FULL AUTO MODE: You are in autonomous mode. Your workflow:
1. Call issue_ready to get the next available issue
2. If an issue exists: claim it, implement it, test it, commit, complete it
3. After completing, call issue_ready again for the next issue
4. When issue_ready returns "No ready issues available":
   - Analyze the codebase to understand the project direction
   - Identify the most logical next improvement or feature
   - Create a new issue using issue_create with your recommendation
   - Immediately claim and implement it
5. Continue this loop until max_budget is reached

Never stop. Always keep working. Always keep improving.
```

3. Outer loop in Rust:
   - Run `query()` with the enhanced prompt
   - After each session completes, check budget remaining
   - If budget remains and agent didn't explicitly stop, restart with same prompt
   - Log each iteration to separate rlog files (iteration-1, iteration-2, etc.)

4. Environment variable support: `AUTOPILOT_FULL_AUTO=1`

### Fix 2: Scrolling CLI output

**Files to modify:**
- `crates/autopilot/src/main.rs` (lines 369-399)

**Implementation:**

1. Replace `print_progress()` function:
```rust
fn print_progress(msg: &SdkMessage) {
    match msg {
        SdkMessage::ToolProgress(p) => {
            println!("{} {} ({:.1}s)", "Working:".yellow(), p.tool_name, p.elapsed_time_seconds);
        }
        SdkMessage::Assistant(a) => {
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    if block.get("type").and_then(|t| t.as_str()) == Some("tool_use") {
                        let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        println!("{} {}", "Calling:".blue(), tool);
                    }
                }
            }
        }
        SdkMessage::Result(_) => {
            println!("{}", "Complete".green());
        }
        _ => {}
    }
}
```

2. Remove all `\r` carriage returns
3. Remove `stdout().flush()` calls (println handles this)

### Pre-implementation: Push existing commit

Push commit `2c5982a58` (AUTOPILOT_MODEL env var support) to main.

## Execution Order

1. Push existing commit to main
2. Implement scrolling CLI output (quick fix)
3. Implement `--full-auto` flag with enhanced prompt
4. Add outer loop logic for session restarts
5. Test with remaining issues (#4, #5)
6. Watch it discover and create issue #6
7. Commit and push
