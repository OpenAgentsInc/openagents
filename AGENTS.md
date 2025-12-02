### Agent Startup Checklist (for this repo)

Before you make any code changes in this repo, do the following:

1. **Read the core docs once per session:**
   - `AGENTS.md` (this file)
   - `docs/mechacoder/README.md`
   - `docs/mechacoder/GOLDEN-LOOP-v2.md`
   - `docs/mechacoder/spec.md`

2. **Inspect project/task config:**
   - If `.openagents/project.json` exists, read it to understand:
     - `defaultBranch`, `testCommands`, `e2eCommands`, `allowPush`, etc.
   - If `.openagents/tasks.jsonl` exists, skim a few tasks to see how work is structured.

3. **Check current health:**
   - Run `bun test` in this repo and make sure it passes.
   - If tests fail, treat fixing them as P0 before starting new work.

4. **Start a work log:**
   - Use the `DAY`/`TS` snippet under "Work Logs".
   - Note the bead/task ID you're about to work on and your intent for this session.

---

## Issue Tracking: bd (beads) and .openagents

This project uses two closely related tracking systems:

- **bd (beads)** — global issue/epic tracker for planning and cross-repo work.
- **.openagents/** — per-project task system (TaskService + ProjectService) used by MechaCoder and OpenAgents Desktop.

**IMPORTANT:**

- Do NOT use markdown TODOs, task lists, or other ad-hoc tracking methods.
- Use **bd** for high-level issues/epics and cross-repo planning.
- Use **.openagents** tasks for concrete, per-project work that MechaCoder can pick up.

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

### OpenAgents project tasks (.openagents)

For repos that have an `.openagents/` directory (e.g. this `openagents` repo and any others we migrate):

- Project metadata and task lists live under:

  ```bash
  .openagents/project.json   # ProjectConfig (branch, tests, model, etc.)
  .openagents/tasks.jsonl    # One task per line (TaskService schema)
  ```

- Tasks use the same conceptual schema as beads issues:

  - `id`, `title`, `description`, `status`, `priority`, `type`, `assignee`, `labels`, `deps`, `commits`, `createdAt`, `updatedAt`, `closedAt`.
  - `status` is `open | in_progress | blocked | closed`.
  - Dependencies have `type` `blocks | related | parent-child | discovered-from`.

**For AI agents:**

- Prefer using the existing **TaskService** and **ProjectService** under `src/tasks/` rather than manually editing `.openagents/*.json*`.
- When creating new work for MechaCoder to pick up:

  - If it's a **global epic/planning item**, create a **bd** issue.
  - If it's a **concrete implementation task for this repo**, add a `.openagents` task (or let MechaCoder/TaskService create it).

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
- When your work touches MechaCoder or `.openagents/` internals, include the bead/task IDs you touched in the log header (e.g. `openagents-42j.4`, `oa-1a2b3c`).

### Git and GitHub CLI Conventions

**Safety Protocol:**
- NEVER update git config
- NEVER run destructive commands (`push --force`, `reset --hard`) unless explicitly requested
- NEVER skip hooks (`--no-verify`) unless explicitly requested
- NEVER force push to main/master - warn if requested
- NEVER commit unless explicitly asked - users find proactive commits intrusive
- Avoid `git commit --amend` unless (1) user requested it or (2) fixing pre-commit hook changes
- Before amending: check authorship with `git log -1 --format='%an %ae'`
- NEVER use `-i` flag (interactive mode not supported)

> **Exception – MechaCoder:**  
> The MechaCoder autonomous agent is allowed to commit and push changes **without explicit user confirmation** as long as it:
> - Follows the Golden Loop v2 spec (`docs/mechacoder/GOLDEN-LOOP-v2.md`),
> - Runs the configured tests from `.openagents/project.json` and they pass,
> - Uses small, task-focused commits that reference the relevant bead/task ID.
>
> The "never commit unless explicitly asked" rule applies to interactive agents (e.g. chat-based assistants), not to MechaCoder's overnight loop.

**Commit Workflow:**

1. Run in parallel to understand state:
   ```bash
   git status
   git diff
   git log --oneline -5
   ```

2. Analyze changes and draft message:
   - Summarize nature (feature, fix, refactor, etc.)
   - Check for secrets (.env, credentials) - warn if found
   - Focus on "why" not "what"

3. Create commit with signature:
   ```bash
   git commit -m "$(cat <<'EOF'
   Your commit message here.

   🤖 Generated with [OpenAgents](https://openagents.com)

   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   ```

4. If pre-commit hook modifies files, retry ONCE. Only amend if:
   - You authored the commit (`git log -1 --format='%an %ae'`)
   - Not yet pushed (`git status` shows "ahead")

**GitHub CLI (`gh`) Usage:**

```bash
# View repo info
gh repo view

# List issues/PRs
gh issue list
gh pr list

# Create PR (use default branch as base)
gh pr create --title "Title" --body "Description"

# View PR/issue details
gh pr view <number>
gh issue view <number>

# Check CI status
gh run list
gh run view <run-id>
```

**PR Creation Protocol:**
1. Check `git status` and `git log` to understand branch state
2. Push branch if needed: `git push -u origin <branch>`
3. Create PR: `gh pr create`
4. Return the PR URL when done

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

### Standard "Next task" flow for .openagents repos

When the user says **"Next task."** for a repo that has `.openagents/`:

1) **Load project config:**
   - Read `.openagents/project.json` and honor:
     - `defaultBranch`
     - `testCommands`, `e2eCommands`
     - `allowPush`, `allowForcePush`
     - `maxTasksPerRun`, `maxRuntimeMinutes` (if relevant)

2) **Load tasks and find ready work:**
   - Use the existing TaskService/TaskPicker in `src/tasks/*` (or any provided CLI wrapper) to:
     - Load `.openagents/tasks.jsonl`
     - Filter to `status in ["open", "in_progress"]`
     - Determine which tasks are **ready** (no open `blocks`/`parent-child` deps)
     - Sort by priority (0..4) and age (oldest first)

3) **Select the top ready task:**
   - Pick the highest-priority, oldest ready task.
   - Mark it `in_progress` via TaskService and update `updatedAt`.

4) **Implement under Golden Loop v2:**
   - Follow `docs/mechacoder/GOLDEN-LOOP-v2.md`:
     - Understand → implement → run tests (`testCommands` + any `e2eCommands` if required) → commit/push if tests pass → update task → log.

5) **Update tasks and log:**
   - Set `status` to `closed` (or `blocked` with `reason`) when done.
   - Append commit SHA(s) to the task's `commits` list.
   - Write a per-run log under `docs/logs/YYYYMMDD/HHMM-*.md` summarizing:
     - Task ID, changes, tests run, results, follow-up tasks created.

Use this flow for **per-project work** in repos with `.openagents/`. Use the "Next bead" flow for **global epics/planning** via bd.

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

- ✅ Use **bd** for ALL high-level issue/epic tracking and cross-repo planning.
- ✅ Use **.openagents** tasks for per-project work in repos that have `.openagents/` (e.g. `openagents`), especially for MechaCoder.
- ✅ Always use `--json` flag for programmatic use.
- ✅ Link discovered work with `discovered-from` dependencies.
- ✅ Check `bd ready` (and/or `.openagents` ready tasks via TaskService) before asking "what should I work on?"
- ✅ Store AI planning docs in `history/` directory.
- ✅ Run `bd <cmd> --help` to discover available flags.
- ❌ Do NOT create markdown TODO lists.
- ❌ Do NOT use external issue trackers.
- ❌ Do NOT duplicate tracking systems.
- ❌ Do NOT clutter repo root with planning documents.

For more details, see README.md and QUICKSTART.md.

**Note:**  
- We use [bd (beads)](https://github.com/steveyegge/beads) for global issue/epic tracking.  
- We use `.openagents/` for per-project task tracking in repos that have it.  
- Do NOT use markdown TODOs or external trackers.

---

## Lessons Learned

Common mistakes and patterns future agents should know:

### Effect TypeScript Patterns

**Providing a Layer to an Effect:**
```typescript
// ✅ CORRECT - Effect.provide with pipe
Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)))

// ✅ CORRECT - Effect.provide as second arg
Effect.runPromise(Effect.provide(program, BunContext.layer))

// ❌ WRONG - Layer.provide is for composing layers, not providing to effects
Effect.runPromise(Layer.provide(BunContext.layer)(program))  // Runtime error!
Effect.runPromise(Layer.provide(BunContext.layer, program))  // Type error!
```

**Effect.gen Pattern (modern):**
```typescript
// ✅ CORRECT - No adapter parameter, direct yield*
Effect.gen(function* () {
  const fs = yield* FileSystem.FileSystem;
  const data = yield* fs.readFile(path);
  return data;
})

// ❌ OLD PATTERN - Adapter is deprecated and causes TypeScript warnings
Effect.gen(function* (_) {
  const fs = yield* _(FileSystem.FileSystem);
  const data = yield* _(fs.readFile(path));
  return data;
})
```

**Mapping Platform Errors to Tool Errors:**
```typescript
// ✅ CORRECT - Map PlatformError to ToolExecutionError
const content = yield* fs.readFileString(path).pipe(
  Effect.mapError((e) => new ToolExecutionError("command_failed", e.message)),
);

// ❌ WRONG - Letting PlatformError leak breaks Tool type signature
const content = yield* fs.readFileString(path);  // Type error!
```

**Context.Tag Pattern (class-based):**
```typescript
// ✅ CORRECT - Modern class-based pattern
export class MyService extends Context.Tag("MyService")<
  MyService,
  { doThing: () => Effect.Effect<void> }
>() {}

// ❌ OLD PATTERN - Function-based doesn't work with yield*
export const MyService = Context.Tag<MyService>()  // Broken!
```

### Test Patterns

**runWithBun Helper:**
```typescript
// ✅ CORRECT - Specific context type
const runWithBun = <A, E>(
  program: Effect.Effect<A, E, FileSystem.FileSystem | Path.Path>
) => Effect.runPromise(program.pipe(Effect.provide(BunContext.layer)));

// ❌ PROBLEMATIC - Using 'any' causes type errors with exactOptionalPropertyTypes
const runWithBun = <A, E>(program: Effect.Effect<A, E, any>) => ...
```

**ToolContent Type Guards:**
```typescript
// ✅ CORRECT - Use type guard for union types
import { isTextContent } from "./schema.js";
const textBlock = result.content.find(isTextContent);
expect(textBlock?.text).toContain("expected");

// ❌ WRONG - Direct property access on union type
expect(result.content[0]?.text).toContain("expected");  // Type error!
```

### Git Conventions

**Commit Message Format:**
```bash
git commit -m "$(cat <<'EOF'
Your commit message here.

🤖 Generated with [OpenAgents](https://openagents.com)

Co-Authored-By: MechaCoder <noreply@openagents.com>
EOF
)"
```

**Never Do:**
- `git push --force` to main/master
- `git commit --amend` on commits you didn't author
- `git config` updates
- Commit without explicit user request (exception: MechaCoder under Golden Loop v2)
- Use `-i` flag (interactive mode not supported)

### Bead Priority

When user says "pick top priority":
- P0 = Critical (do immediately)
- P1 = High (do next)
- P2+ = Lower priority

Don't pick P2/P3 beads when P0/P1 exist unless explicitly told to.

---

<!-- effect-solutions:start -->
## Effect Solutions Usage

The Effect Solutions CLI provides curated best practices and patterns for Effect TypeScript. Before working on Effect code, check if there's a relevant topic that covers your use case.

- `effect-solutions list` - List all available topics
- `effect-solutions show <slug...>` - Read one or more topics
- `effect-solutions search <term>` - Search topics by keyword

**Local Effect Source:** The Effect repository is cloned to `~/.local/share/effect-solutions/effect` for reference. Use this to explore APIs, find usage examples, and understand implementation details when the documentation isn't enough.

**Package versioning:** Never hand-edit `package.json` versions by guessing. Add deps via `bun add -E <pkg>@<version>` (or `bun add -E <pkg>` if you know the exact version) so the lockfile stays consistent.
<!-- effect-solutions:end -->


### Model Selection

**NEVER change the model from `x-ai/grok-4.1-fast`** - it is the only free model available on OpenRouter. Do not switch to Claude, GPT, or any other paid model.



### MechaCoder Autonomous Agent

MechaCoder is an autonomous coding agent that picks up tasks, implements code, runs tests, and commits - learning patterns and conventions over time.
By default, MechaCoder reads and writes tasks from `.openagents/tasks.jsonl` in the target repo (falling back to bd for planning/epics).

**See:** [`docs/mechacoder/`](docs/mechacoder/) for full documentation:
- [README.md](docs/mechacoder/README.md) - Overview and quick start
- [MECHACODER-OPS.md](docs/mechacoder/MECHACODER-OPS.md) - Operations guide (start/stop, logs, troubleshooting)
- [GOLDEN-LOOP-v2.md](docs/mechacoder/GOLDEN-LOOP-v2.md) - Golden Loop v2 spec (desktop agent loop)
- [spec.md](docs/mechacoder/spec.md) - .openagents project format and architecture
