# 1226 Beads Removal Log

## Task
Remove all beads/bd references from the openagents repo and establish .openagents as the sole task system.

## Changes Made

### AGENTS.md (Complete Rewrite)
- Replaced entire bd/beads section with .openagents task system
- Added "Agent Startup Checklist" at the top
- Added "Next task" flow for .openagents repos
- Updated all rules to reference .openagents/tasks.jsonl
- Added MechaCoder exception for autonomous commits
- Removed all bd commands, workflows, and references

### src/agent/do-one-bead.ts
- Updated file header comment (Do One Task)
- Changed log file pattern from `*-bead-run.md` to `*-task-run.md`
- Updated system prompt: BEAD_COMPLETED -> TASK_COMPLETED
- Removed bd close instructions from prompt
- Updated all log messages (bead -> task)

### src/agent/overnight.ts
- Updated usage comment (--max-tasks instead of --max-beads)
- Rewrote OVERNIGHT_SYSTEM_PROMPT to remove bd commands
- Changed maxBeads -> maxTasks throughout
- Renamed runBeadCycle -> runTaskCycle
- Updated all log messages and variable names

### docs/mechacoder/README.md
- Removed beads quick start command
- Added .openagents task inspection command
- Updated document descriptions

### docs/mechacoder/MECHACODER-OPS.md
- Complete rewrite to remove all beads references
- Updated log file patterns (bead-run -> task-run)
- Removed legacy beads section (section 3.3)
- Updated recovery instructions for .openagents
- Updated quick reference table

### docs/mechacoder/GOLDEN-LOOP-v2.md
- Removed bd transition language from task selection

### docs/mechacoder/spec.md
- Removed entire beads intro paragraph
- Removed "Transition from Beads" section (was section 3)
- Removed beads transition strategy content
- Removed "Suggested Initial Beads" section
- Cleaned up all beads references in TL;DR

### scripts/start-mechacoder.sh
- Updated comment: "complete beads" -> "complete tasks from .openagents/tasks.jsonl"

### Deleted Files
- AGENTS-old.md

## Validation
- `bun test` - All tests pass
- `bun run typecheck` - No type errors

## Commit
`e599eb4a` - Remove beads/bd references from openagents repo

Pushed to main.
