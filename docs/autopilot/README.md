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

### Resume a Crashed Session

If autopilot crashes or is interrupted, resume from where it left off:

```bash
# Resume from a JSON trajectory file (has session_id)
cargo autopilot resume docs/logs/20251219/2138-start-working.json

# Continue most recent session (no file needed)
cargo autopilot resume --continue-last

# Resume with additional instructions
cargo autopilot resume file.json --prompt "Continue the task"

# Resume with issue tracking enabled
cargo autopilot resume file.json --with-issues
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

### CLI Options (run)
| Option | Description |
|--------|-------------|
| `--model MODEL` | Model to use |
| `--max-turns N` | Maximum turns |
| `--max-budget USD` | Maximum budget |
| `--verbose` | Show all messages |
| `--dry-run` | Don't save logs |
| `--with-issues` | Enable issue tracking (default in wrapper) |

### CLI Options (resume)
| Option | Description |
|--------|-------------|
| `--continue-last` | Continue most recent session |
| `--prompt TEXT` | Additional prompt to send on resume |
| `--max-budget USD` | Maximum budget for resumed session |
| `--with-issues` | Enable issue tracking |

## Architecture

```
autopilot.sh          # Convenience wrapper
crates/autopilot/     # Main CLI
crates/issues/        # Issue tracking library
crates/issues-mcp/    # MCP server for issue tools
autopilot.db          # SQLite database for issues
docs/logs/            # Trajectory logs
```

## Plan Mode

Plan mode is a structured workflow for exploring and designing before executing:

### What is Plan Mode?

Plan mode creates a restricted environment where agents can:
- **Explore** the codebase (read files, search code)
- **Design** implementation approaches (analyze, plan)
- **Launch subagents** for parallel investigation
- **Ask questions** to clarify requirements

But **cannot**:
- Write files (except the plan file)
- Commit or push to git
- Execute destructive bash commands

### Using Plan Mode

Plan mode is entered via MCP tools:

```bash
# Enter plan mode
issue_enter_plan_mode(slug="feature-name", goal="Implement X")

# Exit plan mode (validates plan completeness)
issue_exit_plan_mode()
```

### Plan Phases

Plans progress through 4 phases:

1. **Explore** - Understand codebase and requirements
2. **Design** - Evaluate approaches and create plan
3. **Review** - Validate plan completeness
4. **Final** - Prepare for implementation

### Subagents in Plan Mode

The planmode module provides helper functions for launching specialized subagents:

```rust
use autopilot::planmode::{explore_agent_prompt, plan_agent_prompt};

// Launch an explore agent
let prompt = explore_agent_prompt(
    "authentication system",
    "Find all auth-related code and patterns"
);

// Launch a plan agent
let prompt = plan_agent_prompt(
    "add OAuth support",
    "Current auth uses JWT tokens",
    "simplicity" // or "performance", "maintainability", etc.
);
```

Agents can be launched in parallel (up to 3 recommended) and their findings incorporated into the plan file.

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
| `enter_plan_mode` | Enter structured planning mode |
| `exit_plan_mode` | Exit plan mode and validate plan |
