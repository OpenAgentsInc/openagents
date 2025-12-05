# Terminal-Bench 2.0 Evaluation

This directory documents how to run Terminal-Bench 2.0 evaluations with MechaCoder.

## Overview

Terminal-Bench 2.0 is a benchmark for evaluating AI coding agents on real-world programming tasks. We support two modes of operation:

| Mode | Use Case | Authentication | Environment |
|------|----------|----------------|-------------|
| **Local** | Internal testing, development | Claude Max subscription (local Claude Code CLI) | macOS/Linux host |
| **Harbor** | Official leaderboard submission | Anthropic API key | Docker containers |

## Quick Start (Local Mode)

```bash
# 1. Clone Terminal-Bench 2.0 tasks (if not already done)
git clone https://github.com/laude-institute/terminal-bench-2.git ~/code/terminal-bench-2

# 2. Import tasks into our suite format (already done - tasks/terminal-bench-2.json exists)
bun src/cli/import-tasks.ts --source ~/code/terminal-bench-2 --output tasks/terminal-bench-2.json

# 3. Install pytest (needed for verification)
pip3 install pytest
```

## Running a Single Task

**This is the most common usage** - run one task and inspect the output:

```bash
# Run a single task
bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output /tmp/tbench-test \
  --tasks regex-log

# View the results
cat /tmp/tbench-test/report.md

# See what files the agent created
ls -la /tmp/tbench-test/regex-log/workspace/

# View agent conversation
cat /tmp/tbench-test/regex-log/output.txt

# View test results
cat /tmp/tbench-test/regex-log/verification.txt
```

## Running Multiple Tasks

```bash
# Run specific tasks (comma-separated)
bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output results/ \
  --tasks "regex-log,large-scale-text-editing,chess-best-move"

# Run all 89 tasks (takes many hours, ~5min per task average)
bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output results/
```

## List Available Tasks

```bash
# See all task IDs
cat tasks/terminal-bench-2.json | jq -r '.tasks[].id'

# See tasks by difficulty
cat tasks/terminal-bench-2.json | jq -r '.tasks[] | "\(.difficulty): \(.id)"' | sort
```

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        tbench-local.ts                          │
│  - Loads suite JSON                                             │
│  - Creates workspace per task                                   │
│  - Copies tests with /app/ → workspace path replacement        │
│  - Runs Claude Code SDK (runClaudeCodeSubagent)                │
│  - Runs pytest for verification                                 │
│  - Generates results + reports                                  │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Claude Code SDK                              │
│  - Uses local Claude Code CLI authentication                    │
│  - Runs with bypassPermissions mode                             │
│  - Outputs ATIF trajectory                                      │
└─────────────────────────────────────────────────────────────────┘
```

## CLI Tools

### import-tasks.ts

Imports tasks from the Terminal-Bench 2.0 repository into our suite JSON format.

```bash
# Import from local clone
bun src/cli/import-tasks.ts --source ~/code/terminal-bench-2 --output tasks/terminal-bench-2.json

# Clone and import
bun src/cli/import-tasks.ts --clone --output tasks/terminal-bench-2.json

# Import specific dataset from Harbor repo
bun src/cli/import-tasks.ts --source ~/code/harbor --output tasks/hello-world.json --dataset hello-world
```

**What it does:**
1. Reads `task.toml` files for metadata (difficulty, category, timeouts)
2. Reads `instruction.md` for task description
3. Records source paths for workspace setup
4. Outputs a suite JSON compatible with `tbench-local.ts`

### tbench-local.ts

Runs Terminal-Bench tasks locally using the Claude Code SDK.

```bash
bun src/cli/tbench-local.ts \
  --suite tasks/terminal-bench-2.json \
  --output results/$(date +%Y%m%d) \
  --tasks task1,task2 \
  --timeout 3600 \
  --max-turns 300
```

**Options:**
- `-s, --suite` - Path to suite JSON (required)
- `-o, --output` - Output directory for results (required)
- `-t, --tasks` - Comma-separated task IDs (default: all)
- `--timeout` - Task timeout in seconds (default: 3600)
- `--max-turns` - Max agent turns per task (default: 300)
- `-b, --baseline` - Baseline results for comparison

**Output files:**
- `results.json` - Machine-readable results
- `report.md` - Human-readable summary
- `<task-id>/workspace/` - Task workspace with agent output
- `<task-id>/output.txt` - Agent conversation log
- `<task-id>/verification.txt` - Test output

## Task Format

Tasks are defined in a suite JSON file:

```json
{
  "name": "Terminal-Bench 2.0",
  "version": "2.0.0",
  "source_repo": "/path/to/terminal-bench-2",
  "tasks": [
    {
      "id": "regex-log",
      "name": "Regex Log",
      "description": "Write a regex expression that...",
      "difficulty": "medium",
      "category": "data-processing",
      "verification": {
        "type": "test",
        "command": "pytest tests/ -v"
      },
      "timeout_seconds": 900,
      "max_turns": 100,
      "tags": ["regex"],
      "source_path": "/path/to/terminal-bench-2/regex-log"
    }
  ]
}
```

## How Workspace Setup Works

For each task, `tbench-local.ts`:

1. **Creates workspace directory**: `/output/<task-id>/workspace/`

2. **Copies environment files**: From `<source_path>/environment/` (excluding Dockerfile)

3. **Copies and transforms tests**: From `<source_path>/tests/`
   - Replaces `/app/` paths with workspace paths
   - This allows tests to run locally without Docker

4. **Runs agent**: Claude Code SDK with `cwd` set to workspace

5. **Runs verification**: `python3 -m pytest tests/ -v` in workspace

## Verification

Terminal-Bench tasks use pytest for verification. The tests check that the agent produced the correct output.

**Requirements:**
```bash
# Install pytest
pip3 install pytest

# Some tasks may need additional Python packages
pip3 install numpy pandas  # example
```

**How verification works:**
1. Tests are copied from `source_path/tests/` to workspace
2. `/app/` paths in tests are replaced with workspace path
3. pytest runs in the workspace directory
4. Exit code 0 = pass, non-zero = fail

## HUD Visualization (Electrobun)

The desktop HUD lets you watch and control TB runs live:

- **View modes:** Run summary (pass/fail counts, timing) and per-task detail panes (phases, turns, stdout/logs). Click a task row to focus its detail view; the summary stays pinned. A collapsible live log stream is available for long-running tasks.
- **Triggering runs from UI:** Open the TB pane in Electrobun, load a suite (defaults to `tasks/terminal-bench-2.json`), pick tasks or leave “all”, set an output directory, then click **Run**. The HUD auto-connects to the TB WebSocket; connection status appears in the header.
- **Keyboard:** No dedicated TB shortcuts yet; use the on-screen controls. Electron defaults still work (`Cmd/Ctrl+R` reloads the window, `Esc` closes dialogs).
- **Screenshots:** Capture during a run to share progress; the layout shows a header with progress, a task list with status chips, and a right-hand detail panel with logs/trajectory for the selected task. Add labels like “Summary” and “Task detail” when sharing.
- **Troubleshooting:** If the HUD shows “Disconnected”, restart Electrobun (`bun run dev`) and rerun TB. Ensure the WebSocket port (default 4242) is free, no stale `.worktrees/` directories remain from prior runs, and the suite path is valid. When in doubt, re-run a single easy task first (e.g., `hello-world`) to confirm connectivity.

## Results Format

```json
{
  "suite_name": "Terminal-Bench 2.0",
  "suite_version": "2.0.0",
  "model": "claude-code",
  "timestamp": "2025-12-04T08:00:00.000Z",
  "results": [
    {
      "task_id": "regex-log",
      "status": "pass",
      "duration_ms": 297500,
      "turns": 14,
      "tokens_used": 5545
    }
  ],
  "summary": {
    "total": 1,
    "passed": 1,
    "failed": 0,
    "pass_rate": 1.0,
    "avg_duration_ms": 297500,
    "avg_turns": 14,
    "total_tokens": 5545
  }
}
```

## Task Categories (89 tasks)

| Category | Count |
|----------|-------|
| software-engineering | 26 |
| system-administration | 9 |
| security | 8 |
| data-science | 8 |
| scientific-computing | 8 |
| file-operations | 5 |
| debugging | 5 |
| mathematics | 4 |
| data-processing | 4 |
| model-training | 4 |
| machine-learning | 3 |
| video-processing | 1 |
| games | 1 |
| personal-assistant | 1 |
| data-querying | 1 |
| optimization | 1 |

## Task Difficulties

| Difficulty | Count |
|------------|-------|
| easy | 4 |
| medium | 55 |
| hard | 30 |

## Harbor Mode (Official Leaderboard)

For official Terminal-Bench leaderboard submission, use Harbor with a real Anthropic API key:

```bash
# Install Harbor
cd src/harbor
uv venv && source .venv/bin/activate
uv pip install -e ".[dev]"

# Run with API key
export ANTHROPIC_API_KEY="sk-ant-api03-..."
harbor run \
  --agent-import-path openagents_harbor:MechaCoderAgent \
  --dataset terminal-bench@2.0 \
  -k 1 \
  -o results/
```

**Note:** Harbor requires a real Anthropic API key from [console.anthropic.com](https://console.anthropic.com). OAuth tokens from Claude Max subscription do not work in Docker containers.

## Troubleshooting

### "No module named pytest"
```bash
pip3 install pytest
```

### Task fails but should pass
Check the verification output:
```bash
cat results/<task-id>/verification.txt
```

Manually run the test:
```bash
cd results/<task-id>/workspace
python3 -m pytest tests/ -v
```

### Agent creates files in wrong location
The agent should create files relative to the workspace (its cwd). Check:
```bash
ls -la results/<task-id>/workspace/
```

### Tests reference /app/ paths
If you see errors about `/app/` paths, the test transformation didn't work. Check:
```bash
cat results/<task-id>/workspace/tests/test_*.py | grep '/app/'
```

## Overnight Iteration System

For running repeated benchmark iterations overnight with learning:

```bash
# Run 10 iterations with Claude Code
bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --iterations 10

# Run with Ollama (any model)
bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --model ollama:codellama:34b --iterations 20

# Mixed: 90% Ollama, 10% Claude for validation
bun src/cli/tbench-iterate.ts --suite ./tasks/tb-2.0.json --model ollama:codellama:34b \
  --claude-validation-rate 0.1 --iterations 20

# Resume interrupted run
bun src/cli/tbench-iterate.ts --resume ./results/20251205/state.json
```

**Features:**
- Support for Claude Code and local Ollama models
- Episode tracking in `.openagents/gym/episodes.jsonl`
- Resume capability for interrupted runs
- Baseline comparison
- HUD integration for real-time monitoring

**Output structure:**
```
results/YYYYMMDD/
├── config.json           # Run configuration
├── state.json            # For resume capability
├── summary.md            # Overall summary
├── episodes.json         # All episodes
└── iterations/
    ├── 001/
    │   ├── results.json
    │   ├── report.md
    │   └── <task-id>/workspace/
    └── ...
```

See [overnight-runs.md](./overnight-runs.md) for detailed usage.
See [model-configuration.md](./model-configuration.md) for Ollama setup.

## Related Files

- `src/cli/tbench-local.ts` - Local runner (single run)
- `src/cli/tbench-iterate.ts` - Overnight iteration runner
- `src/cli/import-tasks.ts` - Task importer
- `src/bench/terminal-bench.ts` - Schemas and adapters
- `src/bench/model-adapter.ts` - Claude Code / Ollama abstraction
- `src/bench/episode-store.ts` - Episode storage
- `src/llm/ollama.ts` - Ollama HTTP client
- `tasks/terminal-bench-2.json` - Imported task suite
- `src/harbor/` - Harbor adapter (for official leaderboard)
