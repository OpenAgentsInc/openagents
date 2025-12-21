# Autopilot

Autonomous task runner with trajectory logging for the Claude Agent SDK.

Autopilot executes tasks using AI agents (Claude or Codex) and records complete execution traces for analysis, debugging, and continuous improvement.

## Features

- 🤖 **Autonomous Execution**: Run complex multi-step tasks with AI agents
- 📊 **Complete Trajectory Logging**: Record every step in JSON and human-readable formats
- 🔄 **Resume Support**: Pick up where you left off if interrupted
- 📈 **Analytics**: Analyze performance, costs, and tool usage patterns
- 🔍 **Replay Mode**: Debug by replaying saved trajectories
- 🎯 **Issue Management**: Track and complete issues with built-in project management
- 🚀 **Full-Auto Mode**: Continuously process issues until queue is empty
- 💾 **Session Tracking**: Maintain history of all runs with metrics

## Installation

```bash
# From the workspace root
cargo build --release -p autopilot

# The binary will be at target/release/autopilot
```

## Quick Start

### Basic Usage

```bash
# Run a simple task
cargo autopilot run "Fix all clippy warnings in the workspace"

# Run with specific model
cargo autopilot run --model opus "Implement the new feature"

# Run with budget limit
cargo autopilot run --max-budget 5.0 "Refactor the authentication module"
```

### Resume a Session

```bash
# Resume the most recent session
cargo autopilot resume --continue

# Resume from a specific trajectory file
cargo autopilot resume --trajectory docs/logs/20251220/103800-fix-warnings.json
```

### Analyze Trajectories

```bash
# Analyze a completed run
cargo autopilot analyze docs/logs/20251220/103800-fix-warnings.json

# Compare two runs
cargo autopilot compare run1.json run2.json
```

## Commands

### `run` - Execute a task

```bash
cargo autopilot run [OPTIONS] <PROMPT>

Options:
  -m, --model <MODEL>           AI model to use [default: sonnet]
  -b, --max-budget <BUDGET>     Maximum cost in USD [default: 10.0]
  -o, --output-dir <DIR>        Output directory for logs
  -s, --slug <SLUG>             Custom slug for filenames
  --ui                          Launch desktop UI
  --project <PROJECT>           Run within a project context
  --full-auto                   Continuously process issues
```

**Examples:**

```bash
# Basic task
cargo autopilot run "Add tests for the user module"

# With budget limit
cargo autopilot run --max-budget 2.0 "Fix the login bug"

# With custom output location
cargo autopilot run --output-dir ~/logs "Refactor database layer"

# Full-auto mode (process all issues)
cargo autopilot run --full-auto --project myproject "Process all issues"
```

### `resume` - Continue a previous session

```bash
cargo autopilot resume [OPTIONS]

Options:
  -t, --trajectory <FILE>       Path to trajectory file
  -c, --continue                Continue most recent session
  -p, --prompt <PROMPT>         Override the resume prompt
  -b, --max-budget <BUDGET>     Maximum additional cost
```

**Examples:**

```bash
# Continue the last session
cargo autopilot resume --continue

# Resume specific session with new budget
cargo autopilot resume --trajectory logs/session.json --max-budget 5.0
```

### `analyze` - Analyze trajectory metrics

```bash
cargo autopilot analyze <TRAJECTORY_FILE>
```

Displays:
- Total cost and token usage
- Duration and number of turns
- Tool usage breakdown
- Success/failure status
- Issues completed

### `replay` - Debug by replaying trajectories

```bash
cargo autopilot replay <TRAJECTORY_FILE>
```

Replays the session step-by-step for debugging.

### `compare` - Compare two trajectories

```bash
cargo autopilot compare <FILE1> <FILE2>
```

Side-by-side comparison of two runs showing differences in cost, performance, and approach.

## Trajectory Output

Every run produces two files:

1. **JSON** (`<timestamp>-<slug>.json`): Structured data for programmatic analysis
2. **rlog** (`<timestamp>-<slug>.rlog`): Human-readable text format

Default location: `docs/logs/YYYYMMDD/HHMMSS-slug.*`

See [Trajectory Format Documentation](docs/trajectory-format.md) for detailed format specification.

### Trajectory Structure

```json
{
  "session_id": "unique-id",
  "prompt": "Your task prompt",
  "model": "claude-sonnet-4",
  "steps": [...],
  "result": {
    "success": true,
    "duration_ms": 45000,
    "num_turns": 8,
    "issues_completed": 2
  },
  "usage": {
    "input_tokens": 1500,
    "output_tokens": 800,
    "cost_usd": 0.025
  }
}
```

## Project Management

Autopilot includes built-in issue and project management:

### Projects

```bash
# Create a project
cargo autopilot project create myproject

# List projects
cargo autopilot project list

# View project details
cargo autopilot project view myproject

# Run within project context
cargo autopilot run --project myproject "Fix all bugs"
```

### Issues

```bash
# List issues
cargo autopilot issue list

# Create an issue
cargo autopilot issue create "Fix authentication bug" --priority high

# View issue details
cargo autopilot issue view 42

# Run a specific issue
cargo autopilot run --issue 42
```

### Sessions

```bash
# List all sessions
cargo autopilot session list

# View session details
cargo autopilot session view <session-id>

# Show recent sessions for a project
cargo autopilot session list --project myproject
```

## Full-Auto Mode

Full-auto mode continuously processes issues from the queue:

```bash
cargo autopilot run --full-auto --project myproject
```

The agent will:
1. Call `issue_ready` to get the next issue
2. Claim and implement it
3. Test and commit changes
4. Push to remote
5. Complete the issue
6. Immediately get the next issue
7. Repeat until no issues remain or budget exhausted

This creates a fully autonomous development workflow.

## Configuration

### Environment Variables

- `AUTOPILOT_MODEL`: Default model (sonnet/opus/haiku)
- `ANTHROPIC_API_KEY`: Claude API key
- `HOME`: Used for default paths

### Model Selection

- `sonnet` - Claude Sonnet 4.5 (default, balanced)
- `opus` - Claude Opus 4.5 (most capable)
- `haiku` - Claude Haiku 4 (fast and economical)

## Advanced Features

### Memory Management

Autopilot monitors system memory and will:
- Kill stale Claude/Node processes if memory is low
- Prevent crashes by checking available memory before starting
- Display memory usage statistics

### Secret Redaction

Automatically redacts secrets from logs:
- API keys
- Passwords
- Private keys
- Tokens
- Connection strings

### Signal Handling

Gracefully handles SIGINT/SIGTERM:
- Saves trajectory before exit
- Cleans up temporary files
- Preserves session state for resume

## Examples

### Fix All Warnings

```bash
cargo autopilot run "Fix all compiler warnings in the workspace"
```

### Implement Feature

```bash
cargo autopilot run --model opus --max-budget 10.0 \
  "Implement OAuth2 authentication with Google and GitHub providers"
```

### Code Review

```bash
cargo autopilot run "Review the changes in PR #123 and add comments"
```

### Autonomous Issue Processing

```bash
# Process all open issues in a project
cargo autopilot run --full-auto --project backend \
  "Work through all open issues"
```

## Output Example

```
Task: Fix clippy warnings across workspace
Model: claude-sonnet-4
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

Step 1: Running cargo clippy...
Step 2: Found 12 warnings
Step 3: Fixing unused imports...
Step 4: Fixing unnecessary clones...
...

━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
✓ Completed in 2m 34s
Cost: $0.45
Tokens: 5,420 in / 2,180 out
Issues: 1 completed

Logs saved to:
  docs/logs/20251220/103800-fix-warnings.json
  docs/logs/20251220/103800-fix-warnings.rlog
```

## Troubleshooting

### Out of Memory

If autopilot crashes with OOM:
- Reduce `--max-budget` to limit execution time
- Close other applications
- Check for stale processes: `ps aux | grep claude`

### API Rate Limits

If you hit rate limits:
- Use `--model haiku` for faster, cheaper requests
- Add delays between operations
- Split large tasks into smaller chunks

### Resume Not Working

If resume fails:
- Check trajectory file path is correct
- Verify session_id exists in the file
- Try `--continue` flag for automatic resume

## Performance Profiling

Autopilot supports CPU profiling via flamegraphs to identify performance bottlenecks.

### Install Flamegraph

```bash
cargo install flamegraph
```

On Linux, enable perf access:
```bash
echo -1 | sudo tee /proc/sys/kernel/perf_event_paranoid
```

### Profile a Run

```bash
# Profile a single run
cargo flamegraph --bin autopilot -- run "Your task here"

# Save to timestamped file
cargo flamegraph --bin autopilot \
  --output ./docs/profiles/flamegraph-$(date +%Y%m%d-%H%M%S).svg \
  -- run "Your task here"

# View the flamegraph
firefox flamegraph.svg
```

### What to Look For

- **Wide bars**: Functions consuming significant CPU time
- **Tall stacks**: Deep call chains (potential optimization targets)
- **Repeated patterns**: Code that could be memoized or cached

See [Performance Profiling Guide](../../docs/profiles/README.md) for detailed profiling documentation, best practices, and archived results.

## Development

### Running Tests

```bash
# All tests
cargo test -p autopilot

# Specific test
cargo test -p autopilot test_trajectory_tracking
```

### Building Docs

```bash
cargo doc -p autopilot --no-deps --open
```

## Related Documentation

- [Trajectory Format](docs/trajectory-format.md) - Detailed format specification
- [Full-Auto Mode](docs/full-auto-mode.md) - Autonomous execution guide
- [Claude Agent SDK](../claude-agent-sdk/README.md) - Underlying SDK documentation

## License

MIT
