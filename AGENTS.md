## Issue Tracking with bd (beads)

**IMPORTANT**: This project uses **bd (beads)** for ALL issue tracking. Do NOT use markdown TODOs, task lists, or other tracking methods.

### Why bd?

- Dependency-aware: Track blockers and relationships between issues
- Git-friendly: Auto-syncs to JSONL for version control
- Agent-optimized: JSON output, ready work detection, discovered-from links
- Prevents duplicate tracking systems and confusion

### Quick Start

**Check for ready work:**
```bash
bd ready --json
```

**Create new issues:**
```bash
bd create "Issue title" -t bug|feature|task -p 0-4 --json
bd create "Issue title" -p 1 --deps discovered-from:bd-123 --json
bd create "Subtask" --parent <epic-id> --json  # Hierarchical subtask (gets ID like epic-id.1)
```

**Claim and update:**
```bash
bd update bd-42 --status in_progress --json
bd update bd-42 --priority 1 --json
```

**Complete work:**
```bash
bd close bd-42 --reason "Completed" --json
```

### Issue Types

- `bug` - Something broken
- `feature` - New functionality
- `task` - Work item (tests, docs, refactoring)
- `epic` - Large feature with subtasks
- `chore` - Maintenance (dependencies, tooling)

### Priorities

- `0` - Critical (security, data loss, broken builds)
- `1` - High (major features, important bugs)
- `2` - Medium (default, nice-to-have)
- `3` - Low (polish, optimization)
- `4` - Backlog (future ideas)

### Workflow for AI Agents

1. **Check ready work**: `bd ready` shows unblocked issues
2. **Claim your task**: `bd update <id> --status in_progress`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create linked issue:
   - `bd create "Found bug" -p 1 --deps discovered-from:<parent-id>`
5. **Complete**: `bd close <id> --reason "Done"`
6. **Commit together**: Always commit the `.beads/issues.jsonl` file together with the code changes so issue state stays in sync with code state

### Auto-Sync

bd automatically syncs with git:
- Exports to `.beads/issues.jsonl` after changes (5s debounce)
- Imports from JSONL when newer (e.g., after `git pull`)
- No manual export/import needed!

### Work Logs (Required)

Always log your work as you go. Use a dated folder per day and timestamped filenames:

- Folder: `docs/logs/YYYYMMDD/` (Central Time)
- Filename: `HHMM-subject-log.md` (example: `0935-feature-start-log.md`)

Commands (copy/paste):

```bash
# Central US time helpers
DAY=$(TZ=America/Chicago date +%Y%m%d)
TS=$(TZ=America/Chicago date +%H%M)

# Ensure folder exists
mkdir -p docs/logs/$DAY

# Start a new log file
echo "# $TS Work Log\n" > docs/logs/$DAY/${TS}-your-subject-log.md
```

Guidelines:
- One or more log entries per meaningful step (create new file each time `TS` changes).
- Summarize what changed, why, and validation steps (typecheck/tests).
- Commit and push your work as you go, including each log file.
- When adding code files, prefer small, focused commits with matching logs.

### Standard "Next bead" flow (for future agents)

When the user says **"Next bead."**, run this exact loop:

1) Pick the bead: `bd ready --json`, choose the highest-priority ready item; if none, ask which epic to prioritize. Claim it: `bd update <id> --status in_progress --json`.
2) Quick triage: ensure it is linked to the correct epic via `discovered-from`; close duplicates; downgrade non-urgent side work to P4 unless the user says otherwise.
   - Concurrency guard: if a bead is already `in_progress` (claimed by another agent), do not take it—tell the user it's in flight. If nothing is ready because upstream beads are blocking, pause and inform the user to wait or unblock.
3) Review context: skim the latest logs for today (and yesterday if useful) under `docs/logs/YYYYMMDD/` to pick up recent agent actions/decisions.
4) Start a dated work log before coding: use the Central Time snippet under "Work Logs" (`DAY`, `TS`, and `docs/logs/$DAY/${TS}-<subject>-log.md`) and note the bead ID and intent. Add new log entries when the timestamp changes or when you finish a major sub-step.
5) Implement the bead: follow AGENTS rules (no `as any`, keep comments sparse). If you discover new work, create a `bd create ... --deps discovered-from:<bead>` entry instead of TODO comments. Opportunistic refactors/structure fixes are encouraged—just log what you moved and why. Triage adjacent beads when you spot mispriority/duplicates, and file updates via bd (or note them for a Bead Audit if the user didn't ask you to change them).
6) Validate with tests: add/extend coverage appropriate to the change, run the relevant tests, and note what ran in the log. If you truly cannot add tests, state why in the log. Fix typecheck/test failures before stopping.
7) Finish: close the bead with `bd close <id> --reason ... --json` (or leave in progress if truly not done), commit code plus `.beads/issues.jsonl` and any new log files together, then push. Do not leave work uncommitted/unpushed. Never use `--no-verify` unless the user explicitly instructs it.

### Bead Audit protocol

When the user says **"Bead Audit."**, do this:
1) Run `bd list --json` (filter by priority/epic if directed) to gather current beads and statuses.
2) Create a dated log in `docs/logs/YYYYMMDD/` (Central Time; use `DAY`/`TS` snippet) named `${TS}-bead-review-log.md`.
   - Start with 3–5 bullets summarizing overall state: top priorities, blockers, duplicates, or stale items.
   - Follow with a few paragraphs explaining what the beads are, why they exist, and the recommended execution order/next steps.
   - Refer to beads by title/description (and full ID if needed); avoid using 3-letter abbreviations when communicating to users.
3) If you see duplicates/outdated beads, note the proposed updates/closures in the report. Do not change beads unless the user asked—this is a review-only action.
4) Save the log and report back with a brief summary and the log path. Do not start implementation unless instructed.

### GitHub Copilot Integration

If using GitHub Copilot, also create `.github/copilot-instructions.md` for automatic instruction loading.
Run `bd onboard` to get the content, or see step 2 of the onboard instructions.

### MCP Server (Recommended)

If using Claude or MCP-compatible clients, install the beads MCP server:

```bash
pip install beads-mcp
```

Add to MCP config (e.g., `~/.config/claude/config.json`):
```json
{
  "beads": {
    "command": "beads-mcp",
    "args": []
  }
}
```

Then use `mcp__beads__*` functions instead of CLI commands.

### Managing AI-Generated Planning Documents

AI assistants often create planning and design documents during development:
- PLAN.md, IMPLEMENTATION.md, ARCHITECTURE.md
- DESIGN.md, CODEBASE_SUMMARY.md, INTEGRATION_PLAN.md
- TESTING_GUIDE.md, TECHNICAL_DESIGN.md, and similar files

**Best Practice: Use a dedicated directory for these ephemeral files**

**Recommended approach:**
- Create a `history/` directory in the project root
- Store ALL AI-generated planning/design docs in `history/`
- Keep the repository root clean and focused on permanent project files
- Only access `history/` when explicitly asked to review past planning

**Example .gitignore entry (optional):**
```
# AI planning documents (ephemeral)
history/
```

**Benefits:**
- ✅ Clean repository root
- ✅ Clear separation between ephemeral and permanent documentation
- ✅ Easy to exclude from version control if desired
- ✅ Preserves planning history for archeological research
- ✅ Reduces noise when browsing the project

### CLI Help

Run `bd <command> --help` to see all available flags for any command.
For example: `bd create --help` shows `--parent`, `--deps`, `--assignee`, etc.

### Important Rules

- ✅ Use bd for ALL task tracking
- ✅ Always use `--json` flag for programmatic use
- ✅ Link discovered work with `discovered-from` dependencies
- ✅ Check `bd ready` before asking "what should I work on?"
- ✅ Store AI planning docs in `history/` directory
- ✅ Run `bd <cmd> --help` to discover available flags
- ❌ Do NOT create markdown TODO lists
- ❌ Do NOT use external issue trackers
- ❌ Do NOT duplicate tracking systems
- ❌ Do NOT clutter repo root with planning documents

For more details, see README.md and QUICKSTART.md.

**Note**: This project uses [bd (beads)](https://github.com/steveyegge/beads) for issue tracking. Use `bd` commands instead of markdown TODOs. See AGENTS.md for workflow details.

<!-- effect-solutions:start -->
## Effect Solutions Usage

The Effect Solutions CLI provides curated best practices and patterns for Effect TypeScript. Before working on Effect code, check if there's a relevant topic that covers your use case.

- `effect-solutions list` - List all available topics
- `effect-solutions show <slug...>` - Read one or more topics
- `effect-solutions search <term>` - Search topics by keyword

**Local Effect Source:** The Effect repository is cloned to `~/.local/share/effect-solutions/effect` for reference. Use this to explore APIs, find usage examples, and understand implementation details when the documentation isn't enough.
<!-- effect-solutions:end -->
