# Context Preservation in Autopilot Compaction

## Overview

This document describes the context loss detection and prevention system implemented in the autopilot to improve compaction effectiveness.

## Problem

During long autopilot sessions, the conversation context is automatically compacted (summarized) to stay within token limits. However, critical context can be lost during this process, leading to:

- Re-reading files that were already read
- Re-searching for information that was already found
- Forgetting error messages and having to re-encounter the same errors
- Losing track of file paths, function names, and architectural decisions

## Solution

### 1. Context Loss Detection (`context_analysis.rs`)

The system now automatically detects context loss patterns by analyzing trajectory logs:

#### Detected Patterns

- **High Read frequency**: If >40% of tool calls are Read operations in sessions with >10 calls, likely forgetting file content
- **Post-error searches**: If 3+ search operations (Read/Glob/Grep) follow an error, likely forgot error details
- **Excessive Grep usage**: If >5 Grep calls in a session, likely forgetting search results

#### Context Types Tracked

- `FilePaths`: File locations and structure
- `SymbolNames`: Function, struct, and type names
- `ErrorDetails`: Error messages and failure reasons
- `WorkingDirectory`: Directory navigation
- `Dependencies`: Package names and versions
- `TestResults`: Test outcomes
- `TodoItems`: Pending tasks
- `GitContext`: Branch and commit info
- `IssueContext`: Issue/directive IDs
- `ArchitectureDecisions`: Design choices
- `Constraints`: Requirements

### 2. Improved Compaction Instructions (`compaction.rs`)

Based on detected patterns, the Autonomous compaction strategy now includes:

```
## Critical Context (MUST PRESERVE)
The following context is frequently lost and severely impacts task completion:

- **File paths**: ALWAYS include specific file paths with line numbers (e.g., `src/main.rs:142`)
- **Error messages**: ALWAYS include full error messages if any errors occurred
- **Issue/Directive IDs**: ALWAYS mention active issue numbers (#1234) and directive IDs (d-004)
- **Test results**: ALWAYS state which tests passed/failed and why
- **Function/type names**: ALWAYS preserve exact names of functions, structs, types being worked on
- **Architectural decisions**: ALWAYS explain why certain approaches were chosen over alternatives
```

### 3. Learning System Integration (`learning.rs`)

The context loss analyzer is integrated into the learning pipeline:

```rust
// Pattern 4: Context loss after compaction
let context_loss_improvements = self.analyze_context_loss(session_ids)?;
improvements.extend(context_loss_improvements);
```

When significant context loss is detected (≥3 instances), the system:
1. Generates a detailed report with evidence
2. Creates an improvement recommendation
3. Proposes updated compaction instructions based on actual loss patterns

## Usage

### Automatic Analysis

Context loss is automatically analyzed when running:

```bash
cargo autopilot metrics analyze
```

### Manual Testing

To test the improved compaction instructions:

```bash
# Run autopilot with full auto mode
cargo autopilot run "task description" --full-auto

# The improved instructions will be used during automatic compaction
```

### Viewing Reports

Context loss reports are included in improvement outputs:

```bash
cargo autopilot metrics analyze --json | jq '.improvements[] | select(.improvement_type == "ContextLoss")'
```

## Metrics

The system tracks:

- **Frequency by type**: How often each context type is lost
- **Average impact**: Severity score (1-10) for each context type
- **Critical types**: Context types with both high frequency (≥3) and high impact (≥6)

## Example Output

```json
{
  "improvement_type": "ContextLoss",
  "description": "Detected 5 instances of context loss, most frequently: File paths and locations",
  "evidence": [
    "session-123: 15 Read calls out of 30 total (50.0%) - likely forgetting file context",
    "session-456: Error at call 8, then 3 search operations - likely forgot error details"
  ],
  "severity": 8,
  "proposed_fix": "Create a handoff-ready summary for autonomous continuation:\n\n## Critical Context (MUST PRESERVE)...",
  "create_issue": false
}
```

## Testing

Run tests to verify the system:

```bash
# Context analysis tests
cargo test -p autopilot --lib context_analysis

# Compaction tests
cargo test -p autopilot --lib compaction

# Learning integration tests
cargo test -p autopilot --lib learning
```

All tests should pass with output like:

```
test context_analysis::tests::test_extract_file_paths ... ok
test context_analysis::tests::test_extract_error_messages ... ok
test context_analysis::tests::test_extract_issue_refs ... ok
test context_analysis::tests::test_generate_report ... ok
test context_analysis::tests::test_generate_improved_instructions ... ok
```

## Future Improvements

Potential enhancements:

1. **Before/after metrics**: Track compaction effectiveness over time
2. **Per-directive tuning**: Customize compaction instructions per directive type
3. **Real-time monitoring**: Alert when context loss is detected during a session
4. **Adaptive thresholds**: Automatically adjust detection thresholds based on outcomes
5. **Cross-session learning**: Identify patterns across multiple autopilot runs

## Related Files

- `crates/autopilot/src/context_analysis.rs` - Core analysis logic
- `crates/autopilot/src/compaction.rs` - Compaction strategies and instructions
- `crates/autopilot/src/learning.rs` - Integration with learning pipeline
- `crates/autopilot/src/main.rs` - PreCompact hook implementation

## References

- Issue #1014: "Refine compaction instructions based on what context gets lost during summarization"
- Directive d-004: "Continual Constant Improvement of Autopilot"
