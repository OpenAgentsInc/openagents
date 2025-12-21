# Autopilot Learnings

This document tracks improvements applied to the autopilot system based on metrics analysis, trajectory review, and observed patterns. Every improvement should be documented here with evidence, implementation details, and measured impact.

## Purpose

This is a living document that serves as:

1. **Historical record** - What we've learned and how autopilot has improved over time
2. **Impact tracker** - Quantitative evidence that improvements work
3. **Pattern library** - Common issues and their solutions
4. **Knowledge base** - Institutional knowledge about autopilot behavior

## How to Document a Learning

When you apply an improvement based on metrics analysis, add an entry using this template:

```markdown
### [YYYY-MM-DD] Brief Title

**Context**: What pattern or issue was detected?

**Evidence**:
- Metric data showing the problem
- Link to issues or trajectories
- Specific examples

**Root Cause**: Why was this happening?

**Solution**: What was changed?

**Impact**:
- Before: [metric baseline]
- After: [metric after change]
- Improvement: [% or absolute change]

**Related**:
- Issue: #XXX
- Commit: [commit hash]
- Directive: [directive ID if applicable]
```

## Learnings

### [2024-12-20] Implemented Metrics Collection and Analysis Infrastructure

**Context**: Autopilot had no systematic way to measure its own performance or detect regressions. Hundreds of trajectory logs existed but were unused for improvement.

**Evidence**:
- 0 metrics collected from ~200+ historical autopilot runs
- No baseline measurements for key dimensions (error rates, token usage, completion rates)
- Improvements were anecdotal rather than data-driven

**Root Cause**: Missing infrastructure for metrics collection, storage, analysis, and automated issue creation.

**Solution**: Implemented Phase 1-4 of directive d-004:
1. **Metrics Collection** (`crates/autopilot/src/metrics.rs`):
   - SQLite database (`autopilot-metrics.db`) for session and tool-call metrics
   - Extract from trajectory JSON files
   - Track 15+ dimensions per session

2. **Anomaly Detection** (`crates/autopilot/src/analyze.rs`):
   - Statistical baselines using mean, median, p50, p90, p99
   - Z-score based anomaly detection (>2σ triggers warning)
   - Regression detection comparing periods

3. **Automated Issue Creation** (`crates/autopilot/src/auto_issues.rs`):
   - Pattern detection: group related anomalies (2+ occurrences)
   - Generate issues with evidence, root cause, proposed fix
   - Link to d-004 directive for tracking

4. **Tool Pattern Detection** (`crates/autopilot/src/tool_patterns.rs`):
   - Detect tool-specific error patterns (EISDIR, ENOENT, NonZeroExit)
   - Tool error rate analysis (>5% triggers pattern)
   - Root cause and fix recommendations per tool

**Impact**:
- Before: 0 metrics tracked, 0 automated improvements
- After: Full metrics pipeline operational
  - CLI command: `cargo autopilot metrics import <log-dir>`
  - CLI command: `cargo autopilot metrics analyze --period 7d`
  - CLI command: `cargo autopilot metrics create-issues`
- First automated detection: Bash tool 6.5% error rate (7 failures in 108 calls)
- Improvement capability: Can now detect and auto-create issues for patterns

**Measured Benefits** (expected):
- Faster problem detection: From manual review to automated in <1 minute
- Better prioritization: Issues ranked by severity and impact
- Institutional knowledge: Learnings preserved in issues and this document

**Related**:
- Issues: #273, #274, #275, #276, #277, #278, #279, #280, #281
- Commits: `a3a102102`, `814e3214c`, `0f782846a`
- Directive: d-004

---

### [2024-12-20] Tool Pattern Detection Detects Bash Error Rate

**Context**: First real-world test of tool pattern detection found Bash tool with 6.5% error rate.

**Evidence**:
```
📊 Found 1 patterns:
  - 1 tool error patterns

Bash tool: 7 non-zero exit errors [low]
Tool: Bash (6.5% error rate, 7 failures)
```

**Root Cause**: Commands exiting with non-zero status. Common causes:
- Commands failing due to invalid arguments
- Required tools or files missing
- Commands needing better error handling

**Solution**: Pattern detected and ready for automated issue creation. Issue would include:
- Investigation SQL commands to query specific failures
- Root cause analysis specific to Bash tool
- Fix recommendations (validation, error handling, retry logic)

**Impact**:
- Before: Would require manual trajectory review to discover this pattern
- After: Automated detection in <1 second
- Next: Issue creation will make this actionable for improvement

**Related**:
- Issue: #280
- Feature: Tool pattern detection
- Directive: d-004

---

## Improvement Categories

### 1. Error Reduction

Improvements that reduce tool errors, crashes, or failures.

### 2. Efficiency Gains

Improvements that reduce tokens, cost, or duration while maintaining quality.

### 3. Completion Rate

Improvements that increase the % of claimed issues that are successfully completed.

### 4. Code Quality

Improvements to output quality, test coverage, or adherence to best practices.

### 5. System Reliability

Improvements to stability, crash prevention, and graceful degradation.

### 6. Observability

Improvements to logging, metrics, and debugging capabilities.

## Baseline Metrics (as of 2024-12-20)

These baselines will be updated quarterly or when major changes occur.

### Current Baselines

*To be populated after first full week of metrics collection*

| Dimension | Mean | Median | P90 | P99 |
|-----------|------|--------|-----|-----|
| tool_error_rate | TBD | TBD | TBD | TBD |
| completion_rate | TBD | TBD | TBD | TBD |
| tokens_per_issue | TBD | TBD | TBD | TBD |
| cost_per_issue | TBD | TBD | TBD | TBD |
| duration_per_issue | TBD | TBD | TBD | TBD |
| session_duration | TBD | TBD | TBD | TBD |

### Historical Baselines

| Date | Version | Notes |
|------|---------|-------|
| 2024-12-20 | Initial | First metrics collection implementation |

## Regression Tracking

If a metric regresses (gets worse than baseline), document it here with investigation status.

### Active Regressions

*None currently*

### Resolved Regressions

*None yet*

## Anti-Patterns Learned

Document patterns that were tried but didn't work or made things worse.

### Don't: Manual Trajectory Review for Pattern Detection

**Why it doesn't work**: Too slow, misses patterns across sessions, not scalable.

**What works instead**: Automated pattern detection from metrics database.

---

## Future Improvement Areas

Based on analysis, these areas show opportunity for improvement but haven't been implemented yet:

1. **Read-Before-Edit Enforcement**: Still seeing "file has not been read" errors
2. **Directory Detection**: EISDIR errors when Read tool called on directories
3. **Command Validation**: Bash commands exiting with non-zero status
4. **Parallelization**: Independent tool calls not always parallelized
5. **Compaction Tuning**: May be triggering too aggressively or not aggressively enough

## Contributing

When you implement an improvement:

1. **Before changing code**: Capture baseline metrics
   ```bash
   cargo autopilot metrics analyze --period 7d > before.txt
   ```

2. **Implement the change**: Make your improvement

3. **After deploying**: Wait 1 week, then capture new metrics
   ```bash
   cargo autopilot metrics analyze --period 7d > after.txt
   ```

4. **Document the learning**: Add entry to this file with before/after comparison

5. **Update baselines**: If improvement is significant, update baseline table

## References

- **Directive d-004**: Continual Constant Improvement of Autopilot
- **IMPROVEMENT-DIMENSIONS.md**: Full list of 50+ measurable dimensions
- **Metrics Database**: `autopilot-metrics.db` (SQLite)
- **Anomalies Table**: Detected anomalies with investigation status
- **Tool Patterns**: `crates/autopilot/src/tool_patterns.rs`
