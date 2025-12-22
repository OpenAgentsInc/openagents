# Autopilot Examples and Workflows

This directory contains practical examples and workflows for using Autopilot in real-world scenarios.

## Table of Contents

- [Getting Started](#getting-started)
- [Common Workflows](#common-workflows)
- [Dashboard Usage](#dashboard-usage)
- [Metrics and Analysis](#metrics-and-analysis)
- [Working with Issues and Directives](#working-with-issues-and-directives)
- [Data Export](#data-export)
- [Advanced Scenarios](#advanced-scenarios)

## Getting Started

### First-Time Setup

1. **Initialize the database**:
   ```bash
   cd /path/to/your/project
   # The database will be created automatically on first use
   ```

2. **Verify installation**:
   ```bash
   cargo run --bin openagents -- autopilot --help
   ```

3. **Set your API key**:
   ```bash
   export ANTHROPIC_API_KEY="your-api-key-here"
   ```

### Your First Autopilot Run

```bash
# Start with a simple task
cargo run --bin openagents -- autopilot run "Add a hello world function to src/lib.rs"

# Output will be saved to docs/logs/YYYYMMDD/HHMMSS-slug.{json,rlog}
```

## Common Workflows

### Workflow 1: Processing a Queue of Issues

```bash
# Create several issues
cargo run --bin openagents -- autopilot issue create \
  "Fix clippy warnings in workspace" --priority high

cargo run --bin openagents -- autopilot issue create \
  "Add unit tests for authentication module" --priority medium

cargo run --bin openagents -- autopilot issue create \
  "Update README with new features" --priority low

# Run autopilot to process the queue
cargo run --bin openagents -- autopilot run \
  --full-auto \
  "Process all ready issues"
```

### Workflow 2: Directive-Driven Development

```bash
# Create issues linked to a directive
cargo run --bin openagents -- autopilot issue create \
  "Implement NIP-01 event signing" \
  --directive-id d-002 \
  --priority urgent

cargo run --bin openagents -- autopilot issue create \
  "Add NIP-01 signature verification tests" \
  --directive-id d-002 \
  --priority high

# Process all issues for this directive
cargo run --bin openagents -- autopilot run \
  --full-auto \
  "Complete all d-002 directive issues"
```

### Workflow 3: Incremental Development with Budget Control

```bash
# Work on a feature with budget constraints
cargo run --bin openagents -- autopilot run \
  --max-budget 2.0 \
  --model sonnet \
  "Implement basic user registration endpoint"

# If interrupted, resume later
cargo run --bin openagents -- autopilot resume --continue

# Add more budget if needed
cargo run --bin openagents -- autopilot resume \
  --continue \
  --max-budget 1.0
```

### Workflow 4: Test-Driven Development

```bash
# Create the test first
cargo run --bin openagents -- autopilot run \
  "Write comprehensive tests for the authentication module"

# Then implement to make tests pass
cargo run --bin openagents -- autopilot run \
  "Implement authentication functions to pass all tests"
```

### Workflow 5: Bug Fix Workflow

```bash
# Create a bug issue with detailed description
cargo run --bin openagents -- autopilot issue create \
  "Users can't login with special characters in password" \
  --priority urgent \
  --issue-type bug

# Run autopilot to investigate and fix
cargo run --bin openagents -- autopilot run \
  "Fix issue #42 - investigate and resolve the login bug"
```

## Dashboard Usage

### Starting the Dashboard

```bash
# Start the web dashboard
cargo run --bin openagents -- autopilot dashboard

# The dashboard will open at http://localhost:8080
```

### Dashboard Features

The dashboard provides:

1. **Real-time Monitoring**
   - Active sessions and their progress
   - Current APM (Actions Per Minute)
   - Budget consumption
   - Issue completion rate

2. **Historical Analysis**
   - Session history with filtering
   - Performance trends over time
   - Cost analysis
   - Success/failure rates

3. **Issue Management**
   - Browse all issues by status
   - Filter by priority, directive, agent
   - Create and update issues
   - View issue dependencies

4. **Trajectory Viewer**
   - Inspect individual runs
   - Step-by-step execution replay
   - Tool usage visualization
   - Error inspection

## Metrics and Analysis

### Analyzing a Completed Run

```bash
# View metrics for a specific trajectory
cargo run --bin openagents -- autopilot metrics \
  docs/logs/20251222/134500-fix-warnings.json

# Output shows:
# - Total cost and tokens used
# - Duration and APM
# - Tool usage breakdown
# - Issues completed
# - Success/failure status
```

### Comparing Multiple Runs

```bash
# Compare two approaches to the same problem
cargo run --bin openagents -- autopilot metrics \
  --compare \
  docs/logs/20251222/run1.json \
  docs/logs/20251222/run2.json

# Shows side-by-side comparison of:
# - Cost efficiency
# - Time to completion
# - Tool usage patterns
# - Code quality metrics
```

### Tracking APM Over Time

```bash
# View APM trends for recent sessions
cargo run --bin openagents -- autopilot metrics --apm-history

# Export APM data for external analysis
cargo run --bin openagents -- autopilot metrics \
  --export-apm apm-data.csv
```

## Working with Issues and Directives

### Creating Issues

```bash
# Basic issue creation
cargo run --bin openagents -- autopilot issue create \
  "Your issue title here"

# With full options
cargo run --bin openagents -- autopilot issue create \
  "Implement feature X" \
  --description "Detailed description here" \
  --priority high \
  --issue-type feature \
  --agent claude \
  --directive-id d-001
```

### Querying Issues

```bash
# List all open issues
cargo run --bin openagents -- autopilot issue list --status open

# List issues for a specific directive
cargo run --bin openagents -- autopilot issue list --directive d-002

# Show issues by priority
cargo run --bin openagents -- autopilot issue list --priority urgent
```

### Working with Directives

```bash
# List all active directives
cargo run --bin openagents -- autopilot directive list

# Get detailed directive information
cargo run --bin openagents -- autopilot directive get d-002

# Check directive progress
cargo run --bin openagents -- autopilot directive progress d-002
```

### Issue Lifecycle

```bash
# 1. Create issue
ISSUE_ID=$(cargo run --bin openagents -- autopilot issue create \
  "Fix performance issue in query handler" \
  --json | jq -r '.number')

# 2. Claim issue (usually done automatically in full-auto mode)
cargo run --bin openagents -- autopilot issue claim $ISSUE_ID

# 3. Work on issue
cargo run --bin openagents -- autopilot run "Fix issue #$ISSUE_ID"

# 4. Complete issue (usually done automatically)
cargo run --bin openagents -- autopilot issue complete $ISSUE_ID

# If blocked, document the blocker
cargo run --bin openagents -- autopilot issue block $ISSUE_ID \
  --reason "Waiting for upstream API changes"
```

## Data Export

### Exporting Trajectory Data

```bash
# Export a single trajectory to CSV
cargo run --bin openagents -- autopilot export \
  docs/logs/20251222/session.json \
  --format csv \
  --output trajectory.csv

# Export all trajectories for a date range
cargo run --bin openagents -- autopilot export \
  --start-date 2025-12-01 \
  --end-date 2025-12-31 \
  --format json \
  --output december-runs.json
```

### Exporting Metrics

```bash
# Export aggregated metrics
cargo run --bin openagents -- autopilot metrics \
  --export metrics.json \
  --format json

# Export APM history
cargo run --bin openagents -- autopilot metrics \
  --export-apm apm-history.csv
```

### Integration with External Tools

```bash
# Export for Jupyter notebook analysis
cargo run --bin openagents -- autopilot export \
  --all \
  --format json \
  --output analysis-data.json

# Then in Python:
# import json
# import pandas as pd
#
# with open('analysis-data.json') as f:
#     data = json.load(f)
# df = pd.DataFrame(data['sessions'])
# df.plot(x='timestamp', y='apm')
```

## Advanced Scenarios

### Scenario 1: Multi-Agent Collaboration

```bash
# Create issues for different agents
cargo run --bin openagents -- autopilot issue create \
  "Write comprehensive test suite" \
  --agent codex

cargo run --bin openagents -- autopilot issue create \
  "Implement core business logic" \
  --agent claude

# Run with agent-specific filtering
cargo run --bin openagents -- autopilot run \
  --full-auto \
  --agent claude \
  "Process all Claude-assigned issues"
```

### Scenario 2: Continuous Integration Workflow

```bash
#!/bin/bash
# ci-autopilot.sh - Run in CI pipeline

set -e

# Create issues from CI failures
if ! cargo test; then
  cargo run --bin openagents -- autopilot issue create \
    "Fix failing CI tests" \
    --priority urgent \
    --agent claude
fi

if ! cargo clippy -- -D warnings; then
  cargo run --bin openagents -- autopilot issue create \
    "Fix clippy warnings" \
    --priority high \
    --agent claude
fi

# Process issues with budget limit
cargo run --bin openagents -- autopilot run \
  --full-auto \
  --max-budget 5.0 \
  "Fix all CI issues"
```

### Scenario 3: Benchmark-Driven Optimization

```bash
# Run benchmarks first
cargo run --bin openagents -- autopilot benchmark run

# Create optimization issues based on results
cargo run --bin openagents -- autopilot issue create \
  "Optimize slow database queries identified in benchmarks" \
  --priority high

# Run autopilot to optimize
cargo run --bin openagents -- autopilot run \
  "Complete all optimization issues, verify with benchmarks"

# Re-run benchmarks to verify improvements
cargo run --bin openagents -- autopilot benchmark run --compare
```

### Scenario 4: Documentation Sprint

```bash
# Create doc issues for all undocumented modules
cargo run --bin openagents -- autopilot issue create \
  "Add comprehensive module docs to all crates" \
  --priority medium \
  --directive-id d-013

# Process with model optimized for writing
cargo run --bin openagents -- autopilot run \
  --model opus \
  --full-auto \
  "Complete all documentation issues"
```

### Scenario 5: Refactoring Large Codebases

```bash
# Break refactoring into phases
cargo run --bin openagents -- autopilot issue create \
  "Phase 1: Extract interfaces" \
  --directive-id d-012

cargo run --bin openagents -- autopilot issue create \
  "Phase 2: Update implementations" \
  --directive-id d-012

cargo run --bin openagents -- autopilot issue create \
  "Phase 3: Remove deprecated code" \
  --directive-id d-012

# Process incrementally with commits between phases
cargo run --bin openagents -- autopilot run \
  --full-auto \
  "Complete refactoring phases in order, commit after each"
```

## Sample Configuration Files

See the `workflows/` directory for example configuration files:

- `workflows/ci-config.toml` - CI/CD integration settings
- `workflows/dev-workflow.toml` - Development workflow presets
- `workflows/review-workflow.toml` - Code review automation

## Tips and Best Practices

### Budget Management

- Start with lower budgets ($2-5) for exploratory tasks
- Use higher budgets ($10+) for complex features
- Monitor spending with `--max-budget` flags
- Resume sessions to add budget incrementally

### Issue Organization

- Link issues to directives for strategic alignment
- Use priorities to guide autonomous work
- Keep issue descriptions specific and actionable
- Block issues when dependencies are unmet

### Monitoring and Debugging

- Use the dashboard for real-time monitoring
- Check APM to gauge agent effectiveness
- Review rlog files for human-readable traces
- Use replay mode to debug unexpected behavior

### Performance Optimization

- Run benchmarks before and after changes
- Track APM trends to identify slowdowns
- Use the metrics system to find bottlenecks
- Export data for deeper analysis

## Additional Resources

- [Autopilot README](../crates/autopilot/README.md) - Comprehensive command reference
- [Trajectory Format](../docs/trajectory-format.md) - Detailed format specification
- [Directive System](../DIRECTIVES.md) - Strategic goals and phases
- [APM Metrics](../docs/apm-metrics.md) - Actions Per Minute tracking

## Getting Help

If you encounter issues or have questions:

1. Check the main README and documentation
2. Review example workflows in this directory
3. Inspect trajectory logs for debugging
4. Open an issue on GitHub with relevant logs
