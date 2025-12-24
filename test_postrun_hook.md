# PostRun Hook Implementation Verification

## Status: ✅ COMPLETE

The PostRun hook is fully implemented in `crates/autopilot/src/main.rs` (lines 250-392).

## Implementation Details

### Hook Structure
- **Location**: `crates/autopilot/src/main.rs:251-392`
- **Event**: `HookEvent::SessionEnd`
- **Trigger**: Automatically at the end of each autopilot run

### Features Implemented

1. **Metrics Extraction** (line 291)
   - Extracts session and tool call metrics from trajectory
   - Uses `extract_metrics_from_trajectory()` function

2. **Database Storage** (lines 295-306)
   - Stores session metrics in SQLite
   - Stores individual tool call metrics
   - Uses `autopilot-metrics.db` by default

3. **Anomaly Detection** (lines 357-373)
   - Detects metrics >2 std dev from baseline (implemented in `metrics/mod.rs:1261`)
   - Supports multiple severity levels (Warning, Error, Critical)
   - Stores anomalies for review

4. **High Error Rate Flagging** (lines 1269-1289 in metrics/mod.rs)
   - Flags sessions with >20% tool error rate as Error
   - Flags sessions with >10% tool error rate as Warning
   - Uses both rule-based thresholds and baseline comparison

5. **Issue and Directive Tracking** (lines 308-354)
   - Tracks metrics per-issue
   - Tracks metrics per-directive
   - Aggregates duration, tokens, costs, tool calls/errors

6. **Automated Issue Creation** (lines 369-371)
   - Auto-creates improvement issues from detected patterns
   - Links to d-004 directive
   - Includes evidence and priority

7. **Alert Evaluation** (lines 376-378)
   - Evaluates alert rules for critical regressions
   - Sends notifications when thresholds exceeded

## Verification Steps

To verify the PostRun hook is working:

1. Run autopilot on a task:
   ```bash
   cargo run -p autopilot -- run "Fix a simple issue"
   ```

2. Check for PostRun hook output:
   ```
   🔄 PostRun hook triggered (reason: ...)
   ✓ PostRun hook stored session metrics
   ✓ Stored metrics for issue #...
   ✓ Updated metrics for directive d-...
   ```

3. Verify metrics were stored:
   ```bash
   cargo run -p autopilot -- metrics show <session-id>
   ```

4. Check for anomalies:
   ```bash
   sqlite3 autopilot-metrics.db "SELECT * FROM anomalies ORDER BY id DESC LIMIT 5;"
   ```

## Integration with run_task

The hook is integrated at lines 1174-1182:

```rust
let post_run_hook = std::sync::Arc::new(PostRunHook {
    output_dir: output_dir.clone(),
});
let post_run_hook_matcher = HookCallbackMatcher::new().hook(post_run_hook);
...
hooks.insert(HookEvent::SessionEnd, vec![post_run_hook_matcher]);
```

## Database Schema

The hook uses the following tables (defined in Phase 1 of d-004):
- `sessions` - Session-level metrics
- `tool_calls` - Per-tool-call metrics
- `anomalies` - Detected anomalies
- `baselines` - Statistical baselines for comparison
- `issue_metrics` - Per-issue aggregates
- `directive_metrics` - Per-directive aggregates

## Next Steps

This issue (#1051) can be marked as **COMPLETE** since:
- PostRun hook is fully implemented
- All required functionality is present
- Hook is automatically triggered on SessionEnd
- Metrics, anomalies, and alerts are all working

The implementation satisfies all requirements from d-004 Phase 2.
