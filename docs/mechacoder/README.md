# MechaCoder Documentation

MechaCoder is an autonomous coding agent that picks up tasks, implements code, runs tests, and commits - learning patterns and conventions over time.

By default, MechaCoder reads and writes tasks from `.openagents/tasks.jsonl` in the target repo.

## Documents

### [GOLDEN-LOOP-v2.md](./GOLDEN-LOOP-v2.md)
The **Golden Loop v2** specification for OpenAgents Desktop. Defines the core loop:

> Select repo → pick ready task → understand → implement → test → commit & push → update task → log → repeat

Covers:
- User experience (what the desktop app shows)
- Agent loop contract (what MechaCoder does each iteration)
- Acceptance criteria for v2
- Safety rules (no commit/push if tests fail)

### [spec.md](./spec.md)
The **OpenAgents Desktop Loop & .openagents Project Spec**. Defines:

- `.openagents/` directory structure (`project.json`, `tasks.jsonl`, `agents.json`)
- Schema for project config and tasks
- Desktop agent architecture (Bun + Effect + Electrobun)

### [MECHACODER-OPS.md](./MECHACODER-OPS.md)
The **operations guide** for running MechaCoder locally. Covers:

- Where logs are stored (`docs/logs/YYYYMMDD/`, `logs/*.log`)
- How to start/stop/restart the agent
- Task management via `.openagents/tasks.jsonl`
- Troubleshooting (uncommitted changes, stuck tasks, API errors)
- Quick reference table of common commands

### [golden-loop-regression.md](./golden-loop-regression.md)
Regression fixture + test matrix spanning CLI do-one-task, overnight loop, and Electrobun/Playwright desktop harnesses. Recommends using `createGoldenLoopFixture` for consistent stub repos.

### [TASK-SPEC.md](./TASK-SPEC.md)
The **task system specification**. Defines:

- Task schema (id, title, status, priority, type, labels, deps, commits)
- Status flow (`open` → `in_progress` → `closed`/`blocked`)
- Dependency types (`blocks`, `related`, `parent-child`, `discovered-from`)
- CLI interface for external agents (`bun run tasks:*`)

### Maintenance

- `bun run health` runs the configured typecheck/test/e2e commands from `.openagents/project.json`. Use `--json` for machine-readable output; non-zero exit when any command fails.

## Quick Start

```bash
# Run MechaCoder once against this repo
cd ~/code/openagents
bun src/agent/do-one-task.ts --dir .

# Run overnight loop (limited)
bun src/agent/overnight.ts --dir . --max-tasks 3

# Check if MechaCoder is running (launchd)
launchctl list | grep mechacoder

# View latest agent log
cat $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md | head -1)

# Inspect tasks in a repo
cat .openagents/tasks.jsonl | jq '.'

# Use the tasks CLI (for external agents)
bun run tasks:ready --json     # List ready tasks
bun run tasks:next --json      # Claim next task
```

## Related

- [AGENTS.md](../../AGENTS.md) - Project-wide coding guidelines and conventions
- [CLAUDE.md](../../CLAUDE.md) - Same as AGENTS.md (for Claude compatibility)
