# Automatic Metrics Emission Verification

## Issue #585 Resolution

Issue #585 claimed that "autopilot should automatically emit metrics to metrics.db at the end of each run" and that "currently metrics must be imported manually".

**This is INCORRECT.** Automatic metrics emission has been implemented since the metrics module was created.

## Implementation Location

`crates/autopilot/src/main.rs:2033-2035`:

```rust
// Extract and store metrics
store_trajectory_metrics(&trajectory);
```

This function (defined at line 2433) automatically:
1. Extracts metrics from the trajectory using `extract_metrics_from_trajectory`
2. Opens the metrics database at `autopilot-metrics.db`
3. Stores session metrics
4. Stores tool call metrics
5. Detects and reports anomalies
6. Prints success confirmation

## Code Flow

Every autopilot run goes through `run_task()`:
1. Line 1897: Creates `TrajectoryCollector`
2. Line 2000-2026: Runs the agent (claude or codex)
3. Line 2028: Finalizes trajectory with `collector.finish()`
4. **Line 2035: Automatically stores metrics**
5. Line 2038-2090: Saves trajectory files (.rlog, .json)

## Verification

All metrics tests pass:
```
test metrics::tests::test_extract_metrics_from_trajectory ... ok
test metrics::tests::test_store_and_retrieve_session ... ok
test metrics::tests::test_store_and_retrieve_tool_calls ... ok
test metrics::tests::test_detect_anomalies_with_baseline ... ok
```

## Database Location

Metrics are automatically stored to `autopilot-metrics.db` in the current working directory.

## PostRun Hook Behavior

The `store_trajectory_metrics` function implements the PostRun hook behavior:
- Line 2469: Detects anomalies after storing metrics
- Line 2472-2490: Reports anomalies to console with color-coded severity
- This provides immediate feedback without requiring manual analysis

## Conclusion

- ✅ Automatic metrics emission is ALREADY IMPLEMENTED
- ✅ Works for all run completions (success, crash, budget exhausted)
- ✅ Anomaly detection runs automatically
- ✅ No additional work needed

Issue #585 should be marked as complete (already implemented).
Issue #622 (duplicate) should be closed.
