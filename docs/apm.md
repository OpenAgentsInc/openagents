# APM (Actions Per Minute) Tracking System

The APM tracking system measures agent velocity and effectiveness by counting actions (messages and tool calls) over time. Inspired by StarCraft 2's competitive metric, APM provides objective performance measurement for both autonomous autopilot runs and interactive Claude Code sessions.

## Quick Start

```bash
# View current APM stats
autopilot apm stats

# Watch real-time APM during a run
autopilot apm watch

# List recent sessions
autopilot apm sessions

# Export data for analysis
autopilot apm export apm-data.json
```

## APM Tiers

APM values are classified into tiers that indicate performance levels:

| Tier | APM Range | Description |
|------|-----------|-------------|
| **Baseline** | 0-5 | Starting out, learning, or working on complex problems |
| **Active** | 5-10 | Steady progress, good momentum |
| **Productive** | 10-15 | High efficiency, strong workflow |
| **Elite** | 15-20 | Exceptional performance, rapid execution |
| **Superhuman** | 20+ | Peak performance, autonomous agent velocity |

## How APM is Calculated

```
APM = Total Actions / Duration (minutes)

Total Actions = Messages + Tool Calls
```

**Messages**: User inputs and Assistant responses
**Tool Calls**: Invocations of Read, Edit, Write, Bash, Grep, etc.

APM is calculated over different time windows:
- **Session**: Current/last run only
- **1h, 6h, 1d, 1w**: Rolling time windows
- **Lifetime**: All-time average

## Commands

### `autopilot apm stats`

Display APM statistics for different sources.

```bash
# Show stats for all sources
autopilot apm stats

# Show stats for autopilot only
autopilot apm stats --source autopilot

# Show stats for Claude Code sessions
autopilot apm stats --source claude_code
```

**Example output:**
```
APM Statistics
──────────────────────────────────────────────────────────────────────
Autopilot        19.2 APM  (Elite) - 142 messages, 89 tool calls
ClaudeCode        4.5 APM  (Active) - 38 messages, 12 tool calls
```

### `autopilot apm sessions`

List recent APM sessions.

```bash
# List last 20 sessions
autopilot apm sessions

# List last 50 sessions
autopilot apm sessions --limit 50

# List autopilot sessions only
autopilot apm sessions --source autopilot
```

**Example output:**
```
APM Sessions
──────────────────────────────────────────────────────────────────────
✓ apm-e4c7f2a1-8b3d-4 2025-12-22 23:45:12
✓ apm-b9a3e5d2-7c1f-3 2025-12-22 22:30:45
• apm-3f8a9b4e-2d6c-1 2025-12-22 21:15:33

Showing 3 of 3 sessions
```

Legend:
- `✓` = Complete
- `•` = Running

### `autopilot apm show <session-id>`

Show detailed breakdown for a specific session.

```bash
autopilot apm show apm-e4c7f2a1-8b3d-4
```

**Example output:**
```
APM Session: apm-e4c7f2a1-8b3d-4a7e-9f2b-6c8d1e3a5b7c
──────────────────────────────────────────────────────────────────────
Messages:            45
Tool Calls:          150
Total Actions:       195

APM: 19.5
```

### `autopilot apm export <output-file>`

Export APM data to JSON for external analysis.

```bash
# Export all autopilot sessions
autopilot apm export apm-data.json

# Export Claude Code sessions
autopilot apm export claude-apm.json --source claude_code
```

**Example output file:**
```json
{
  "source": "autopilot",
  "exported_at": "2025-12-22T23:54:00Z",
  "sessions": [
    {
      "id": "apm-e4c7f2a1-8b3d-4a7e-9f2b-6c8d1e3a5b7c",
      "start_time": "2025-12-22T23:45:12Z",
      "end_time": "2025-12-22T23:55:12Z"
    }
  ]
}
```

### `autopilot apm watch`

Monitor APM stats in real-time with auto-refresh.

```bash
# Watch with 2-second refresh (default)
autopilot apm watch

# Watch with custom interval
autopilot apm watch --interval 5

# Watch Claude Code sessions
autopilot apm watch --source claude_code
```

**Example display:**
```
══════════════════════════════════════════════════════════════════════
                    APM Dashboard - Autopilot
══════════════════════════════════════════════════════════════════════

Session:             apm-e4c7f2a1-8b3d-4a7e-9f2b-6c8
Status:              Running
Duration:            10.2 minutes

Messages:            45
Tool Calls:          150
Total Actions:       195

Current APM:         19.1 APM
Tier:                Elite

Tier Thresholds:
  Baseline 0-5   Active 5-10   Productive 10-15   Elite 15-20   Superhuman 20+

Updated: 23:54:32 | Refresh: 2s
```

Press `Ctrl+C` to exit.

## Data Collection

APM data is automatically collected during autopilot runs and stored in SQLite (`autopilot.db`).

### Database Schema

**apm_sessions**: Track session lifecycle
- `id`: Unique session identifier
- `source`: autopilot | claude_code
- `start_time`: When session began
- `end_time`: When session completed (NULL if running)

**apm_events**: Individual action records
- `id`: Unique event identifier
- `session_id`: Links to session
- `event_type`: message | tool_call | git_command | file_operation
- `timestamp`: When action occurred
- `metadata`: JSON data (tool name, etc.)

**apm_snapshots**: Pre-calculated metrics
- `id`: Unique snapshot identifier
- `timestamp`: When calculated
- `source`: autopilot | claude_code
- `window`: session | 1h | 6h | 1d | 1w | lifetime
- `apm`: Calculated APM value
- `actions`: Total action count
- `duration_minutes`: Time span
- `messages`: Message count
- `tool_calls`: Tool call count

### Disabling APM Tracking

APM tracking is enabled by default. To disable:

```bash
autopilot run "task description" --no-apm
```

APM tracking is automatically disabled in dry-run mode.

## Use Cases

### Performance Monitoring

Track your velocity over time to identify:
- Peak productivity periods
- Task complexity impact on speed
- Learning curves with new tools
- Effectiveness of different workflows

### Agent Comparison

Compare autonomous autopilot vs. interactive Claude Code:

```bash
# View both sources
autopilot apm stats

# Watch autopilot in one terminal
autopilot apm watch --source autopilot

# Watch Claude Code in another terminal
autopilot apm watch --source claude_code
```

### Optimization

Use APM data to:
- Identify bottlenecks (low APM periods)
- Optimize tool selection
- Improve prompting strategies
- Fine-tune agent configurations

### Historical Analysis

Export data for deeper analysis:

```bash
# Export all data
autopilot apm export all-sessions.json

# Analyze with external tools
python analyze_apm.py all-sessions.json
```

## Implementation Details

### TrajectoryCollector Integration

APM tracking is integrated into the `TrajectoryCollector`:

```rust
let mut collector = TrajectoryCollector::new(...);

// Enable APM tracking
collector.enable_apm_tracking("autopilot.db", APMSource::Autopilot)?;

// Events are automatically recorded during execution
// - User/Assistant messages
// - Tool call invocations
// - Git commands

// Session finalized and snapshot saved
let trajectory = collector.finish();
```

### Event Recording

APM events are recorded for:
- **Messages**: Every user input and assistant response
- **Tool Calls**: Read, Edit, Write, Bash, Grep, Glob, etc.
- **Git Commands**: When git operations are executed
- **File Operations**: Read/write/edit file actions

### Snapshot Calculation

APM snapshots are calculated at session end:
1. Count total messages and tool calls
2. Calculate session duration
3. Compute APM = actions / duration_minutes
4. Determine APM tier
5. Save to database

## Troubleshooting

### No sessions found

If `autopilot apm stats` shows "No data":
1. Run an autopilot task to generate APM data
2. Ensure `--no-apm` flag is not set
3. Check that you're not in dry-run mode
4. Verify database path is correct

### Watch command not updating

If `autopilot apm watch` freezes:
1. Press `Ctrl+C` to exit
2. Check database is not locked
3. Verify interval is reasonable (>= 1 second)
4. Ensure terminal supports ANSI escape codes

### Incorrect APM values

If APM seems wrong:
1. Check session duration (very short sessions = high APM)
2. Verify event counts with `autopilot apm show <session-id>`
3. Ensure clock is synchronized
4. Check for incomplete sessions (no end_time)

## Contributing

To extend APM tracking:

1. **Add new event types**: Update `APMEventType` in `crates/autopilot-core/src/apm_storage.rs`
2. **Add new windows**: Update `APMWindow` in `crates/autopilot-core/src/apm.rs`
3. **Add new sources**: Update `APMSource` (e.g., for codex agent)
4. **Add new metrics**: Extend `APMSnapshot` struct

See `crates/autopilot-core/src/apm_storage.rs` for implementation details.

## References

- Directive d-016: Measure Actions Per Minute (APM)
- `crates/autopilot-core/src/apm.rs`: APM calculation logic
- `crates/autopilot-core/src/apm_storage.rs`: Database storage layer
- `crates/autopilot-core/src/lib.rs`: TrajectoryCollector integration
- `crates/autopilot-core/src/main.rs`: CLI commands
