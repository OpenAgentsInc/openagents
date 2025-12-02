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

## Quick Start

```bash
# Check if MechaCoder is running
launchctl list | grep mechacoder

# View latest agent log
cat $(ls -t ~/code/openagents/docs/logs/$(date +%Y%m%d)/*.md | head -1)

# Inspect tasks in a repo
cd ~/code/openagents && cat .openagents/tasks.jsonl | jq '.'
```

## Related

- [AGENTS.md](../../AGENTS.md) - Project-wide coding guidelines and conventions
- [CLAUDE.md](../../CLAUDE.md) - Same as AGENTS.md (for Claude compatibility)
