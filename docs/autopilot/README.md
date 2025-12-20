# OpenAgents Autopilot

Autonomous task runner with issue tracking and trajectory logging.

## Quick Start

```bash
# Run a task
./autopilot.sh "Implement feature X"

# Use a specific model
./autopilot.sh --model opus "Complex architectural task"

# Verbose output
./autopilot.sh --verbose "Debug this issue"
```

## Issue Workflow

Autopilot integrates with a local issue tracker for task management.

### Check Issue Queue
```bash
sqlite3 autopilot.db "SELECT number, title, status, priority FROM issues ORDER BY priority, number;"
```

### Create Issues
```bash
./autopilot.sh "Use issue_create to add a new task: Fix the login bug (high priority, bug type)"
```

### Work on Issues
```bash
./autopilot.sh "Use issue_ready to get the next task. Claim it, implement it, test it, commit, and complete it."
```

## Trajectories

All runs are logged to `docs/logs/YYYYMMDD/`:

- `.rlog` - Human-readable log with YAML header
- `.json` - Machine-readable full trajectory

### View Recent Runs
```bash
ls -la docs/logs/$(date +%Y%m%d)/
```

### Analyze a Run
```bash
# View summary
head -50 docs/logs/20251219/1916-use-issue-ready-to-get.rlog

# Parse JSON
jq '.result' docs/logs/20251219/1916-use-issue-ready-to-get.json
```

## Git Workflow

Autopilot follows this workflow for code changes:

1. Creates branch: `git checkout -b autopilot/issue-N`
2. Makes changes and commits
3. Opens PR via `gh pr create`
4. Human reviews and squash-merges
5. Autopilot pulls main and continues

## Configuration

### Environment Variables
| Variable | Default | Description |
|----------|---------|-------------|
| `AUTOPILOT_MODEL` | sonnet | Default model (sonnet/opus/haiku) |
| `AUTOPILOT_BUDGET` | 3.0 | Max budget in USD |
| `AUTOPILOT_MAX_TURNS` | 30 | Max conversation turns |

### CLI Options
| Option | Description |
|--------|-------------|
| `--model MODEL` | Model to use |
| `--max-turns N` | Maximum turns |
| `--max-budget USD` | Maximum budget |
| `--verbose` | Show all messages |
| `--dry-run` | Don't save logs |
| `--with-issues` | Enable issue tracking (default in wrapper) |

## Architecture

```
autopilot.sh          # Convenience wrapper
crates/autopilot/     # Main CLI
crates/issues/        # Issue tracking library
crates/issues-mcp/    # MCP server for issue tools
autopilot.db          # SQLite database for issues
docs/logs/            # Trajectory logs
```

## MCP Tools Available

When running with `--with-issues`:

| Tool | Description |
|------|-------------|
| `issue_list` | List issues (filter by status) |
| `issue_create` | Create new issue |
| `issue_get` | Get issue by number |
| `issue_claim` | Claim issue for work |
| `issue_complete` | Mark issue done |
| `issue_block` | Block issue with reason |
| `issue_ready` | Get next ready issue |
