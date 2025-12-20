# Full-Auto Mode

Full-auto mode enables truly autonomous operation where the agent continuously works through issues until budget is exhausted.

## Quick Start

```bash
cargo fullauto
```

This runs the autopilot with:
- `--full-auto` flag enabled
- `--max-turns 99999` (effectively unlimited)
- `--max-budget 300` ($300 USD limit)
- Default prompt: "Call issue_ready NOW to get the first issue and begin working."

## How It Works

### The Autonomous Loop

1. **Start**: Agent calls `issue_ready` to get the next available issue
2. **Work**: Agent claims, implements, tests, commits, and pushes the issue
3. **Complete**: Agent calls `issue_complete` to mark the issue done
4. **Repeat**: Agent immediately calls `issue_ready` again
5. **Create**: If no issues are ready, agent creates new issues based on codebase analysis

### Continuation Enforcement

The agent has a natural tendency to stop and summarize after completing work. Full-auto mode prevents this by:

1. **Detecting premature stops**: When the agent outputs a "success" result without exhausting budget/turns
2. **Forcing continuation**: Sending a new prompt with `continue_session=true` to resume
3. **Reinforcing instructions**: Including the full-auto prompt in each continuation

The continuation prompt explicitly tells the agent:
- NEVER output session summaries
- NEVER stop to reflect on progress
- After EVERY `issue_complete`, immediately call `issue_ready`
- The ONLY valid stop reasons: budget exhausted, max turns, or system crash

## Memory Management

Long-running autonomous sessions can exhaust system memory. The autopilot includes aggressive memory monitoring and cleanup.

### Memory Threshold

```rust
const MIN_AVAILABLE_MEMORY_BYTES: u64 = 2 * 1024 * 1024 * 1024; // 2 GB
```

If available memory drops below 2GB, the autopilot takes action.

### Memory Checks

Memory is checked:
- At the start of each loop iteration
- Every 10 messages during execution
- Memory status is logged every 100 messages

### Memory Hog Detection

When memory is low, the autopilot:

1. **Lists top 15 processes** by memory usage
2. **Highlights Claude/Node processes** (likely culprits)
3. **Shows memory stats**: total, used, available

Example output:
```
============================================================
MEM: Memory Status
  Total:     16.0 GB
  Used:      14.2 GB
  Available: 1.8 GB

PROCS: Top 15 Memory Hogs:
   1.    2.1 GB  12345  node ← CLAUDE/NODE
   2.    1.8 GB  12346  node ← CLAUDE/NODE
   3.    1.2 GB  12347  Slack
   ...
============================================================
```

### Automatic Process Cleanup

The autopilot automatically kills:
- **Node processes using >500MB** (stale Claude CLI instances)
- Excludes the current process to avoid self-termination

After killing processes, it waits 2 seconds for cleanup, then rechecks memory. If memory is still insufficient, it aborts with a clear error.

### Why Node Processes?

Claude Code CLI runs on Node.js. Long-running sessions or multiple continuation attempts can leave orphaned Node processes that accumulate memory. The autopilot aggressively cleans these up.

## Configuration

### Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `AUTOPILOT_MODEL` | `sonnet` | Model to use (sonnet, opus, haiku) |
| `AUTOPILOT_MAX_TURNS` | `99999` | Maximum conversation turns |
| `AUTOPILOT_BUDGET` | `300.0` | Maximum cost in USD |
| `AUTOPILOT_FULL_AUTO` | - | Enable full-auto mode |

### Command Line

```bash
# Basic full-auto
cargo fullauto

# With custom model
cargo autopilot run --with-issues --full-auto --model opus "Start working"

# With custom budget
cargo autopilot run --with-issues --full-auto --max-budget 100 "Start working"
```

## Stopping Full-Auto

The autopilot stops when:

1. **Budget exhausted**: API costs reach the limit
2. **Max turns reached**: Conversation turns hit the limit
3. **Max continuations**: 1000 continuation attempts (safety limit)
4. **Memory critical**: Available memory below 2GB after cleanup attempts
5. **Manual interrupt**: Ctrl+C

## Logs

Session logs are saved to:
```
docs/logs/YYYYMMDD/HHMMSS-<slug>.rlog
docs/logs/YYYYMMDD/HHMMSS-<slug>.json
```

Monitor live progress with:
```bash
tail -f docs/logs/$(date +%Y%m%d)/*.rlog
```

## Troubleshooting

### Agent Stops After Few Issues

The continuation mechanism should prevent this. Check:
1. Is `--full-auto` flag being passed?
2. Are there errors in the log?
3. Is budget exhausted?

### Memory Crashes

If your system still crashes:
1. Lower the memory threshold (edit `MIN_AVAILABLE_MEMORY_BYTES`)
2. Close other applications
3. Monitor with `htop` or Activity Monitor
4. Check if logs are growing too large

### Stale Processes

If Node processes accumulate:
```bash
# Manually kill all Node processes (careful!)
pkill -f node

# Or kill specific Claude-related processes
pkill -f "claude-code"
```

## Architecture

```
cargo fullauto
    │
    ▼
run_full_auto_loop()
    │
    ├─► check_memory() ──► check_and_kill_memory_hogs()
    │
    ├─► query(prompt, options)
    │       │
    │       ▼
    │   Claude Agent works...
    │       │
    │       ▼
    │   Stream ends (Success/Error)
    │
    ├─► Check: budget/turns exhausted?
    │       │
    │       Yes ──► Exit loop
    │       │
    │       No
    │       │
    │       ▼
    ├─► Check: more issues available?
    │       │
    │       ▼
    └─► Set continue_session=true
        Set continuation prompt
        Loop back to query()
```

## Related

- [Issues System](../../issues/README.md)
- [Claude Agent SDK](../../claude-agent-sdk/README.md)
- [Trajectory Logging](./trajectory-format.md)
