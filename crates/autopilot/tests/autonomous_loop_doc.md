# Autonomous Loop Integration Tests

This document describes the autonomous loop execution flow and test coverage.

## Autonomous Loop Flow

```
START LOOP
  ↓
issue_ready()
  ↓
┌─────────────────┐
│ Issue found?    │
└────┬────────────┘
     │ Yes        │ No
     ↓            ↓
issue_claim()    directive_get()
     ↓            ↓
Implement task   issue_create()
     ↓            ↓
Git commit       issue_claim()
     ↓            ↓
Git push         Implement task
     ↓            ↓
issue_complete() Git commit
     ↓            ↓
LOOP BACK        Git push
                 ↓
             issue_complete()
                 ↓
             LOOP BACK
```

## Test Coverage

### 1. Complete Issue Cycle (`test_complete_issue_cycle`)
Tests the happy path: `issue_ready` → `issue_claim` → `issue_complete`

**Verified:**
- All tool calls recorded in trajectory
- Trajectory result contains correct num_turns
- Tool call sequence is correct

**Implementation:** `tests/autonomous_loop.rs:78`

### 2. No Ready Issues Scenario (`test_no_ready_issues_scenario`)
Tests handling when `issue_ready` returns no issues

**Verified:**
- Agent calls `directive_get` to find work
- Agent creates new issue via `issue_create`
- New issue is linked to directive

**Implementation:** `tests/autonomous_loop.rs:153`

### 3. Git Workflow Tracking (`test_git_workflow_tracking`)
Tests tracking of git operations in trajectory

**Verified:**
- `git commit` tracked as Bash tool call
- `git push` tracked as Bash tool call
- Commit message format verified

**Implementation:** `tests/autonomous_loop.rs:240`

### 4. Multiple Issues Completion (`test_multiple_issues_completion`)
Tests completing multiple issues in one autonomous run

**Verified:**
- Multiple `issue_complete` calls tracked
- Counter increments correctly
- All issues recorded in trajectory

**Implementation:** `tests/autonomous_loop.rs:293`

### 5. Trajectory Persistence (`test_trajectory_persistence`)
Tests that trajectory data persists correctly to disk

**Verified:**
- Trajectory written as JSON to .rlog file
- File contains session_id and all steps
- JSON is valid and parseable

**Implementation:** `tests/autonomous_loop.rs:359`

### 6. Error Handling in Loop (`test_error_handling_in_loop`)
Tests that errors during execution are properly tracked

**Verified:**
- Tool errors recorded in trajectory
- Result marked as failure (success=false)
- Error messages captured

**Implementation:** `tests/autonomous_loop.rs:413`

### 7. State Management Across Iterations (`test_state_management_across_iterations`)
Tests that state persists across loop iterations

**Note:** Each trajectory collector tracks independently. State persistence happens via the autopilot database, not in-memory.

**Implementation:** `tests/autonomous_loop.rs:474`

## Running Tests

```bash
# Run all autonomous loop tests
cargo test -p autopilot --test autonomous_loop

# Run specific test
cargo test -p autopilot --test autonomous_loop test_complete_issue_cycle

# Run with output
cargo test -p autopilot --test autonomous_loop -- --nocapture
```

## Test Data Requirements

Tests use mock messages from `claude-agent-sdk`:
- `SdkMessage::System(Init)` - Initialize session
- `SdkMessage::Assistant` - Tool calls
- `SdkMessage::Result(Success)` - Successful completion
- `SdkMessage::Result(ErrorDuringExecution)` - Errors

## Coverage Gaps

The following scenarios are NOT currently tested:

1. **NIP-SA trajectory publishing** - Real Nostr relay integration
2. **Multi-agent coordination** - Multiple agents working on same issue
3. **Actual git operations** - Tests use mocked Bash calls, not real git
4. **Database persistence** - Tests don't verify autopilot.db state
5. **Crash recovery** - Tests don't simulate daemon restarts
6. **Token budget exhaustion** - Tests don't verify budget limit handling
7. **Permission denials** - Tests don't simulate user rejections
8. **Hook execution** - Pre/post hooks not tested in isolation

These would require:
- Test Nostr relay (or mock relay server)
- Temporary git repositories
- Database fixtures
- Process lifecycle management

## Related Files

- `crates/autopilot/src/trajectory.rs` - Trajectory data structures
- `crates/autopilot/tests/trajectory_tracking.rs` - Trajectory tracking tests
- `crates/autopilot/src/main.rs` - Main autonomous loop implementation
- `.openagents/CLAUDE.md` - Autonomous loop behavior documentation

## Future Improvements

1. Add end-to-end test with real issue database
2. Add test with actual git repository (using tempdir)
3. Add test with mock Nostr relay for trajectory publishing
4. Add test for daemon crash recovery
5. Add performance benchmarks for trajectory operations
