# Autopilot Sessions

This document describes how Autopilot manages session state and checkpoints.

## Overview

Autopilot sessions are resumable multi-phase workflows that execute code changes. Each session progresses through phases:

1. **Plan** - Generate implementation plan with Claude
2. **Execute** - Apply changes to codebase
3. **Review** - Verify changes work correctly
4. **Fix** - Iterate on failures (up to N attempts)

Sessions are checkpointed to disk, enabling:
- Resume after crashes
- Resume after user abort
- Resume after system restart
- Session history and replay

## Directory Structure

```
~/.openagents/sessions/
├── abc123-def456/
│   └── checkpoint.json
├── xyz789-uvw012/
│   └── checkpoint.json
└── ...
```

Each session has a unique ID directory containing its checkpoint file.

## Checkpoint Format

```json
{
  "version": 1,
  "session_id": "abc123-def456",
  "checkpoint_time": "2024-01-15T10:30:00Z",
  "original_start_time": "2024-01-15T10:00:00Z",
  "phase": "execute",
  "phase_started_offset": 1800.0,
  "iteration": 0,
  "model": "claude-sonnet-4-20250514",

  "claude_session_id": "sess_abc...",
  "exec_session_id": "sess_def...",
  "review_session_id": null,
  "fix_session_id": null,

  "claude_events": [...],
  "claude_full_text": "Plan text...",
  "exec_events": [...],
  "exec_full_text": "Execution text...",
  "review_events": [],
  "review_full_text": "",
  "fix_events": [],
  "fix_full_text": "",

  "plan_cursor": 42,
  "exec_cursor": 17,
  "review_cursor": 0,
  "fix_cursor": 0,

  "lines": [...],
  "plan_path": "/path/to/plan.md",
  "last_checklist": {...},
  "working_dir": "/path/to/repo",
  "force_stopped": false,
  "force_stop_reason": null
}
```

### Key Fields

| Field | Type | Description |
|-------|------|-------------|
| `version` | u32 | Checkpoint format version (currently 1) |
| `session_id` | String | Unique session identifier |
| `phase` | StartupPhase | Current phase (plan/execute/review/fix) |
| `iteration` | u32 | Fix loop iteration count |
| `model` | ClaudeModel | Claude model being used |
| `working_dir` | PathBuf | Repository working directory |

### Session IDs

Session IDs are stored for API resume:

| Field | Purpose |
|-------|---------|
| `claude_session_id` | Plan phase API session |
| `exec_session_id` | Execute phase API session |
| `review_session_id` | Review phase API session |
| `fix_session_id` | Fix phase API session |

### Event History

Each phase accumulates events:

| Field | Contents |
|-------|----------|
| `claude_events` | Plan phase Claude events |
| `exec_events` | Execution phase events |
| `review_events` | Review phase events |
| `fix_events` | Fix phase events |

Cursors track delivery position for resuming event streams.

## Checkpoint API

### Path Resolution

```rust
use autopilot_core::checkpoint::SessionCheckpoint;

// Get sessions directory
let dir = SessionCheckpoint::sessions_dir();
// ~/.openagents/sessions/

// Get specific checkpoint path
let path = SessionCheckpoint::checkpoint_path("abc123");
// ~/.openagents/sessions/abc123/checkpoint.json
```

### Saving Checkpoints

```rust
// Checkpoints are saved automatically during phase transitions
// and periodically during long operations

checkpoint.save()?;
```

### Loading Checkpoints

```rust
// Load specific session
let checkpoint = SessionCheckpoint::load("abc123")?;

// List all sessions
let sessions = SessionCheckpoint::list_sessions()?;
for (id, checkpoint) in sessions {
    println!("{}: {:?}", id, checkpoint.phase);
}
```

### Stale Detection

Sessions older than 24 hours are considered stale:

```rust
if checkpoint.is_stale() {
    println!("Session is stale (>24 hours old)");
}
```

### Validity Checking

Sessions are invalid if the working directory no longer exists:

```rust
if !checkpoint.is_valid() {
    println!("Working directory no longer exists");
}
```

## Session Lifecycle

### Starting a New Session

```
autopilot run "implement feature X"
       │
       ▼
┌─────────────────────────┐
│  Generate session ID    │
│  Create checkpoint dir  │
│  Initialize checkpoint  │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  Plan Phase             │
│  - Start Claude session │
│  - Save checkpoint      │
│  - Generate plan        │
└───────────┬─────────────┘
            │
            ▼
        [Continue...]
```

### Resuming a Session

```
autopilot resume abc123
       │
       ▼
┌─────────────────────────┐
│  Load checkpoint        │
│  Verify working_dir     │
│  Check API sessions     │
└───────────┬─────────────┘
            │
            ▼
┌─────────────────────────┐
│  Resume at saved phase  │
│  - Replay events up to  │
│    cursor position      │
│  - Continue execution   │
└───────────┬─────────────┘
            │
            ▼
        [Continue...]
```

### Force Stopping

```rust
checkpoint.force_stopped = true;
checkpoint.force_stop_reason = Some("User requested stop".to_string());
checkpoint.save()?;
```

## Integration with Pylon

Autopilot can query Pylon for daemon status:

```rust
use autopilot_core::pylon_integration::*;

// Check if pylon daemon is running
if is_pylon_running() {
    println!("Pylon daemon is active");
}

// Get PID
if let Some(pid) = get_pylon_pid() {
    println!("Pylon PID: {}", pid);
}

// Send commands via control socket
let response = send_pylon_command(DaemonCommand::Status)?;
```

### Pylon Paths Used

| Path | Purpose |
|------|---------|
| `~/.openagents/pylon/pylon.pid` | Check daemon status |
| `~/.openagents/pylon/control.sock` | Send IPC commands |

## Cleanup

### Manual Cleanup

```bash
# Remove specific session
rm -rf ~/.openagents/sessions/abc123/

# Remove all sessions
rm -rf ~/.openagents/sessions/*
```

### Programmatic Cleanup

```rust
// Remove stale sessions
for (id, checkpoint) in SessionCheckpoint::list_sessions()? {
    if checkpoint.is_stale() {
        std::fs::remove_dir_all(
            SessionCheckpoint::checkpoint_path(&id).parent().unwrap()
        )?;
    }
}
```

## Backup

### Backup Sessions

```bash
# Backup all sessions
cp -r ~/.openagents/sessions/ ~/sessions-backup-$(date +%Y%m%d)/
```

### Restore Sessions

```bash
# Restore from backup
cp -r ~/sessions-backup-20240115/* ~/.openagents/sessions/
```

## Troubleshooting

### "Session not found"

The checkpoint file doesn't exist:

```bash
ls ~/.openagents/sessions/
# Check if session ID exists
```

### "Working directory not found"

The repo has been moved or deleted:

```bash
# Check the working_dir in checkpoint
cat ~/.openagents/sessions/abc123/checkpoint.json | jq .working_dir
```

### "Session is stale"

Session is >24 hours old. Either:
- Resume anyway: `autopilot resume abc123 --force`
- Delete and start fresh: `rm -rf ~/.openagents/sessions/abc123/`

### Corrupted Checkpoint

If JSON is invalid:

```bash
# Try to validate JSON
cat ~/.openagents/sessions/abc123/checkpoint.json | jq .

# If invalid, remove and start fresh
rm -rf ~/.openagents/sessions/abc123/
```

## Security Considerations

### Sensitive Data

Checkpoints may contain:
- Claude API responses (potentially sensitive)
- Code snippets from the repository
- File paths and project structure

### Permissions

```bash
chmod 700 ~/.openagents/sessions/
chmod 600 ~/.openagents/sessions/*/checkpoint.json
```

### Cleanup

Remove sessions after completion if they contain sensitive data:

```bash
# Remove completed sessions
autopilot cleanup --completed
```
