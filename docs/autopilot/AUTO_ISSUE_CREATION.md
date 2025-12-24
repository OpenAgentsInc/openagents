# Automated Issue Creation from Metrics Analysis

## Overview

The autopilot system can automatically detect patterns of failures in metrics and create improvement issues with detailed evidence. This implements **Phase 4 of directive d-004**.

## Features

### Pattern Detection

The system detects two types of patterns:

#### 1. Anomaly Patterns
Metrics that deviate >2σ from baseline across multiple sessions:
- Tool error rate anomalies
- Token consumption anomalies
- Cost efficiency issues
- Completion rate problems
- Session duration anomalies

#### 2. Tool Error Patterns
Specific tool failure patterns:
- **EISDIR errors**: Attempting to read directories as files
- **ENOENT errors**: File not found errors
- **Permission errors**: Access denied failures
- **High error rates**: Tools with >10% failure rate

### Automatic Issue Creation

When patterns are detected, the system:
1. Groups similar anomalies/errors into patterns
2. Generates detailed issue descriptions with:
   - Evidence from affected sessions
   - Links to trajectory logs
   - Statistical analysis (deviation %, occurrence count)
   - Proposed fixes with specific steps
   - Investigation commands
3. Calculates priority based on:
   - Severity (Critical/Error/Warning)
   - Frequency (occurrence count)
   - Deviation magnitude
4. Links all issues to directive d-004
5. Marks anomalies as investigated to prevent duplicates

## Usage

### Basic Command

```bash
cargo autopilot metrics create-issues
```

This will:
- Scan the metrics database for uninvestigated anomalies
- Detect tool error patterns
- Generate and create improvement issues
- Print issue numbers and links

### Dry Run Mode

Preview what would be created without actually creating issues:

```bash
cargo autopilot metrics create-issues --dry-run
```

Output shows:
- Number and type of patterns detected
- Proposed issue titles and priorities
- Pattern details (sessions affected, error rates, etc.)

### Custom Database Paths

```bash
cargo autopilot metrics create-issues \
  --metrics-db /path/to/autopilot-metrics.db \
  --issues-db /path/to/autopilot.db
```

### Environment Variables

```bash
export ISSUES_DB=/workspace/autopilot.db
cargo autopilot metrics create-issues
```

## Example Output

```
================================================================================
🤖 Automated Issue Creation from Pattern Detection
================================================================================

🔍 Detecting patterns...
📊 Found 3 patterns:
  - 2 anomaly patterns
  - 1 tool error patterns

📝 Proposed Issues:

1. High tool error rate detected across 5 sessions [high]
   Type: Anomaly pattern
   Dimension: tool_error_rate (5 sessions, Error severity)

2. Read tool: 12 EISDIR errors (attempting to read directories) [medium]
   Type: Tool error pattern
   Tool: Read (15.0% error rate, 12 failures)

3. Token usage anomaly: 3 sessions with unusual token consumption [medium]
   Type: Anomaly pattern
   Dimension: tokens_per_issue (3 sessions, Warning severity)

🚀 Creating issues...
  ✓ Created issue #25: High tool error rate detected across 5 sessions [high]
  ✓ Created issue #26: Read tool: 12 EISDIR errors [medium]
  ✓ Created issue #27: Token usage anomaly: 3 sessions [medium]

================================================================================
✓ Created 3 improvement issues linked to d-004

Issue numbers: #25, #26, #27

View issues: cargo autopilot issue list
================================================================================
```

## Issue Template Structure

All auto-generated issues follow this structure:

### Title
- Dimension-specific and descriptive
- Includes occurrence count
- Examples:
  - "High tool error rate detected across 5 sessions"
  - "Read tool: 12 EISDIR errors (attempting to read directories)"
  - "Cost efficiency issue: 4 sessions with elevated costs"

### Description

#### Evidence Section
- Occurrence count
- Severity level
- Average deviation percentage
- Deviation range (min-max)
- List of affected sessions with metrics
- Commands to view each session

#### Trajectory Evidence
- Commands to find trajectory logs
- What to look for
- Specific error patterns to identify

#### Proposed Fix
- Severity indicators (⚠️ CRITICAL / HIGH IMPACT)
- Step-by-step fix recommendations
- SQL queries for deeper investigation
- Specific guardrails or validations to add
- Target metrics post-fix

#### Investigation Steps
- Commands to review metrics
- How to analyze the pattern
- Steps to verify fix effectiveness

## Priority Calculation

Issues are prioritized using a composite score:

| Component | Score Range | Criteria |
|-----------|-------------|----------|
| Severity | 1-3 | Warning=1, Error=2, Critical=3 |
| Frequency | 0-2 | <5=0, 5-9=1, 10+=2 |
| Deviation | 0-2 | <25%=0, 25-50%=1, >50%=2 |

**Total Score → Priority Mapping:**
- 6-7 = urgent
- 4-5 = high
- 2-3 = medium
- 0-1 = low

## Workflow Integration

### Weekly Analysis

Add to cron or GitHub Actions:

```bash
# Every Monday at 9 AM
0 9 * * 1 cd /path/to/openagents && cargo autopilot metrics create-issues
```

### After Metrics Collection

In daemon post-run hook:

```bash
# After collecting metrics
cargo autopilot metrics create-issues
```

### Manual Investigation

When you notice issues:

```bash
# 1. Check for patterns
cargo autopilot metrics create-issues --dry-run

# 2. Review what would be created
# 3. Create issues if patterns look valid
cargo autopilot metrics create-issues
```

## Database Schema

### Anomalies Table
```sql
CREATE TABLE anomalies (
    session_id TEXT,
    dimension TEXT,
    expected_value REAL,
    actual_value REAL,
    severity TEXT,  -- 'warning', 'error', 'critical'
    investigated BOOLEAN DEFAULT FALSE,
    issue_number INTEGER,  -- NULL until issue created
    PRIMARY KEY (session_id, dimension)
);
```

### Issues Table (auto_created column)
```sql
ALTER TABLE issues ADD COLUMN auto_created BOOLEAN DEFAULT FALSE;
```

All auto-created issues have:
- `auto_created = TRUE`
- `directive_id = "d-004"`
- `agent = "claude"`
- `issue_type = "task"`

## Preventing Duplicates

The system prevents duplicate issues by:
1. Marking anomalies as `investigated = TRUE` after issue creation
2. Linking anomalies to `issue_number` for traceability
3. Only detecting patterns from uninvestigated anomalies
4. Requiring 2+ similar anomalies to form a pattern

## Viewing Auto-Created Issues

```bash
# List all auto-created issues
cargo autopilot issue list-auto

# List only open auto-created issues
cargo autopilot issue list-auto --status open
```

## Metrics Commands for Investigation

After issues are created, use these commands to investigate:

```bash
# View specific session metrics
cargo autopilot metrics show <session-id>

# Analyze trends
cargo autopilot metrics analyze --period 7d

# Show anomalies
cargo autopilot metrics analyze --anomalies

# Find tool error breakdown
cargo autopilot metrics analyze --period 7d | grep "Top Error Tools"

# Export for external analysis
cargo autopilot metrics export --period 7d --format csv --output metrics.csv
```

## Example Investigation Workflow

1. **Automated detection finds pattern:**
   ```bash
   cargo autopilot metrics create-issues
   ```

2. **Review created issue #25 "High tool error rate":**
   ```bash
   cargo autopilot issue show 25
   ```

3. **Investigate affected sessions:**
   ```bash
   cargo autopilot metrics show session-abc123
   ```

4. **Find trajectory logs:**
   ```bash
   find docs/logs -name '*abc123*.json' -o -name '*abc123*.rlog'
   ```

5. **Analyze tool errors:**
   ```bash
   cargo autopilot metrics analyze --period 7d
   ```

6. **Implement fix based on findings**

7. **Verify improvement:**
   ```bash
   # After fix, check if error rate decreased
   cargo autopilot metrics analyze --period 7d
   ```

8. **Complete issue:**
   ```bash
   cargo autopilot issue complete 25
   ```

9. **Update baselines:**
   ```bash
   cargo autopilot metrics analyze --update-baselines
   ```

## Code Organization

### Module Structure

```
crates/autopilot/src/
├── auto_issues.rs          # Anomaly pattern detection & issue generation
├── tool_patterns.rs        # Tool-specific error pattern detection
├── analyze.rs              # Regression detection & trend analysis
└── metrics/
    ├── mod.rs              # Metrics database & storage
    └── baseline.rs         # Baseline tracking
```

### Key Functions

**auto_issues.rs:**
- `detect_patterns()` - Group anomalies into patterns
- `detect_all_patterns()` - Detect both anomaly and tool patterns
- `generate_issues()` - Generate issue descriptions
- `create_issues()` - Create issues in database
- `calculate_priority()` - Calculate issue priority

**tool_patterns.rs:**
- `detect_tool_patterns()` - Find tool-specific failure patterns
- `detect_error_type_patterns()` - Group by error type (EISDIR, etc.)
- `generate_tool_pattern_title()` - Generate issue titles
- `generate_tool_pattern_description()` - Generate issue descriptions

**analyze.rs:**
- `detect_regressions()` - Find metrics worse than baseline
- `store_regressions_as_anomalies()` - Convert regressions to anomalies
- `calculate_velocity()` - Track improvement rate

## Testing

Run tests for the automated issue creation system:

```bash
# Test pattern detection
cargo test -p autopilot auto_issues::tests

# Test tool pattern detection
cargo test -p autopilot tool_patterns::tests

# End-to-end test
cargo test -p autopilot test_create_issues_end_to_end
```

## Related Documentation

- [docs/autopilot/IMPROVEMENT-DIMENSIONS.md](./IMPROVEMENT-DIMENSIONS.md) - All 50+ trackable dimensions
- [docs/autopilot/METRICS.md](./METRICS.md) - Metrics collection system
- [docs/autopilot/LEARNINGS.md](./LEARNINGS.md) - Documented improvements
- [.openagents/directives/d-004.md](../../.openagents/directives/d-004.md) - Full directive

## Troubleshooting

### No patterns detected

**Possible causes:**
- No anomalies in metrics database
- All anomalies already investigated
- Baselines not set (run with `--update-baselines`)

**Solution:**
```bash
# Check if anomalies exist
cargo autopilot metrics analyze --anomalies

# Update baselines first
cargo autopilot metrics analyze --update-baselines

# Try detection again
cargo autopilot metrics create-issues --dry-run
```

### Issues not linked to d-004

This should never happen (it's hardcoded), but if it does:

```sql
-- Manually fix in database
UPDATE issues
SET directive_id = 'd-004'
WHERE auto_created = TRUE AND directive_id IS NULL;
```

### Duplicate issues created

Check if anomalies were properly marked:

```sql
-- Find anomalies without issue numbers
SELECT * FROM anomalies
WHERE investigated = FALSE;

-- Find anomalies that should have been marked
SELECT a.*, i.number
FROM anomalies a
LEFT JOIN issues i ON i.auto_created = TRUE
WHERE a.investigated = FALSE
AND i.number IS NOT NULL;
```

## Future Enhancements

Potential improvements (not yet implemented):

- [ ] Automatic scheduling (weekly cron)
- [ ] Slack/Discord notifications when issues are created
- [ ] Machine learning to predict which patterns are most impactful
- [ ] Auto-prioritization based on directive urgency
- [ ] Pattern trend analysis (are patterns getting worse over time?)
- [ ] Integration with dashboard for visual pattern browsing

## Summary

The automated issue creation system closes the loop on continual improvement:

1. **Metrics collected** during autopilot runs
2. **Patterns detected** from anomalies and tool errors
3. **Issues created** automatically with detailed evidence
4. **Developers investigate** using provided commands
5. **Fixes implemented** based on proposed solutions
6. **Improvement verified** with next week's metrics
7. **Baselines updated** to reflect new performance

This creates a **perpetual flywheel** where every run contributes to making autopilot better.
