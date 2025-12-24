# Automated Issue Creation from Anomaly Detection

**Status**: ✅ FULLY IMPLEMENTED

## Overview

The autopilot system automatically creates improvement issues when patterns of failures are detected in metrics analysis. This implements Phase 4 of directive d-004 (Continual Constant Improvement).

## How It Works

### 1. Detection (PostRun Hook)

After each autopilot run, the PostRun hook:
- Extracts metrics from the trajectory log
- Detects anomalies (>2 std dev from baseline)
- Stores anomalies in the database
- Calls `auto_create_issues_from_patterns()`

**Code Location**: `crates/autopilot/src/main.rs:321`

```rust
if let Err(e) = auto_create_issues_from_patterns(&db, &workdir) {
    eprintln!("Warning: PostRun hook failed to auto-create issues: {}", e);
}
```

### 2. Pattern Detection

The system detects two types of patterns:

#### Anomaly Patterns
Groups anomalies by dimension (e.g., `tool_error_rate`, `tokens_per_issue`).
- Requires 2+ occurrences to identify as a real pattern
- Calculates severity (worst seen across all occurrences)
- Computes average deviation from expected values

#### Tool Error Patterns
Identifies tools with high error rates:
- Requires minimum 10 calls for statistical significance
- Calculates error rate per tool
- Groups similar error types

**Code Location**: `crates/autopilot/src/auto_issues.rs:122-207`

### 3. Issue Generation

For each pattern, generates:

**Title**: Descriptive with occurrence count
- "High tool error rate detected across 5 sessions"
- "Token usage anomaly: 3 sessions with unusual token consumption"

**Description**: Comprehensive markdown with:
- Evidence section (occurrences, severity, deviation stats)
- Affected sessions with actual vs expected values
- Trajectory log commands for investigation
- Proposed fix with specific action items
- Investigation steps with SQL queries

**Priority Calculation**: Composite score based on:
- Severity (Critical=3, Error=2, Warning=1)
- Frequency (10+=2, 5-9=1, <5=0)
- Deviation magnitude (>50%=2, >25%=1, <25%=0)
- Total score 6-7=urgent, 4-5=high, 2-3=medium, 0-1=low

**Code Location**: `crates/autopilot/src/auto_issues.rs:238-664`

### 4. Issue Creation

Issues are created in the database with:
- `directive_id = "d-004"` (linked to improvement directive)
- `auto_created = true` (flag for automated detection tracking)
- Proper UUID and issue number assignment
- Anomaly records updated with `issue_number` for traceability

**Code Location**: `crates/autopilot/src/auto_issues.rs:676-726`

## Manual Triggering

You can manually trigger issue creation:

```bash
# Dry run (preview without creating)
cargo run -p autopilot -- metrics create-issues --dry-run

# Actually create issues
cargo run -p autopilot -- metrics create-issues

# Specify custom database paths
cargo run -p autopilot -- metrics create-issues \
  --metrics-db /path/to/metrics.db \
  --issues-db /path/to/autopilot.db
```

## Testing

Comprehensive test coverage in `crates/autopilot/src/auto_issues.rs`:

- `test_detect_patterns` - Pattern detection from anomalies
- `test_generate_issues` - Issue content generation
- `test_generate_issues_urgent_priority` - Urgent priority calculation
- `test_generate_issues_high_priority` - High priority calculation
- `test_create_issues_end_to_end` - Full integration test

Run tests:
```bash
cargo test -p autopilot --test '*' auto_issues
```

## Example Generated Issue

**Title**: "High tool error rate detected across 5 sessions"

**Priority**: high

**Description**:
```markdown
Detected pattern of error anomalies in **tool_error_rate** across 5 sessions.

## Evidence

- **Occurrences**: 5 sessions
- **Severity**: Error
- **Average deviation**: 24.5%
- **Deviation range**: 18.2% to 31.7%

### Affected Sessions

1. Session `a1b2c3d4...`
   - Expected: 0.050, Actual: 0.066 (+32.0%)
   - View session: `cargo autopilot metrics show a1b2c3d4`

2. Session `e5f6g7h8...`
   - Expected: 0.045, Actual: 0.058 (+28.9%)
   - View session: `cargo autopilot metrics show e5f6g7h8`

...and 3 more sessions

### Trajectory Evidence

Review trajectory logs for affected sessions in `docs/logs/` to identify specific:
- Tool calls that failed repeatedly
- Error messages and stack traces
- Patterns in agent reasoning before failures
- Context that might explain the anomaly

Example command:
```bash
# Find trajectory for session
find docs/logs -name '*a1b2c3d4*.json' -o -name '*a1b2c3d4*.rlog'
```

## Proposed Fix

⚠️  **HIGH IMPACT**: This pattern significantly affects autopilot performance.

Investigate common tool errors (detected in 5 sessions with 24.5% avg deviation):

1. **Identify error types**:
   ```sql
   SELECT tool_name, error_type, COUNT(*) as count
   FROM tool_calls tc
   WHERE tc.session_id IN ('a1b2c3d4', 'e5f6g7h8', ...) AND tc.success = 0
   GROUP BY tool_name, error_type
   ORDER BY count DESC;
   ```

2. **Review specific failures**: Focus on tools with >10 errors

3. **Add targeted guardrails**:
   - If EISDIR: Add directory detection before Read tool
   - If ENOENT: Validate file existence before operations
   - If permission errors: Check file permissions in hooks

4. **Update system prompts**: Add examples of correct tool usage

5. **Implement pre-call validation**: Block invalid tool calls before execution

## Investigation Steps

1. Review detailed metrics for affected sessions:
   ```bash
   cargo autopilot metrics show a1b2c3d4  # Session 1
   cargo autopilot metrics show e5f6g7h8  # Session 2
   ```

2. Query tool error breakdown:
   ```bash
   cargo autopilot metrics analyze --period 7d
   ```

3. Review trajectory logs for common patterns:
   ```bash
   grep -r "error" docs/logs/*/
   ```

4. After implementing fix:
   - Mark this issue as complete
   - Update baselines: `cargo autopilot metrics analyze --period 7d`
   - Monitor next week's metrics to confirm improvement

---

*This issue was automatically generated by autopilot metrics analysis (d-004).*
*Pattern detected from 5 sessions with 24.5% average deviation.*
```

## Success Criteria (Phase 4 of d-004)

- [x] When a pattern of failures is detected, auto-create improvement issue
- [x] Issue includes: evidence from trajectories, proposed fix, priority
- [x] Link improvement issues to d-004 directive
- [x] Track which improvements came from automated detection (`auto_created` flag)
- [x] Example: "Tool error rate >10% this week - investigate Read tool EISDIR errors"

## Related Issues

- #1012 - Marked this as "partially implemented" but actually it's complete
- #1013 - Same, marked partially but fully functional
- #974 - Original tracking issue for this feature (should be marked done)

## Verification

To verify the system is working:

1. Run autopilot with errors to generate anomalies
2. Check PostRun hook output for "Auto-created X improvement issues"
3. Query the database:
   ```sql
   SELECT number, title, priority, auto_created
   FROM issues
   WHERE directive_id = 'd-004' AND auto_created = 1
   ORDER BY created_at DESC LIMIT 10;
   ```

## Next Steps

The automated issue creation system is production-ready. Future enhancements could include:

1. **Trend-based issue creation**: Create issues for gradual degradation (not just anomalies)
2. **Auto-prioritization updates**: Adjust priority if pattern worsens
3. **Auto-close stale issues**: If metric returns to normal for 2+ weeks
4. **Pattern clustering**: Group related anomalies across dimensions
5. **Root cause linking**: Connect issues if they share common sessions

These are nice-to-haves, not blockers for the current implementation.
