### Agent Startup Checklist (for this repo)

> **This repo (`openagents`) uses `.openagents/` as its sole task system. Beads (`bd`) is not used here anymore.**

Before you make any code changes in this repo, do the following:

1. **Read the core docs once per session:**
   - `AGENTS.md` (this file)
   - `docs/mechacoder/README.md`
   - `docs/mechacoder/GOLDEN-LOOP-v2.md`
   - `docs/mechacoder/spec.md`

2. **Inspect project/task config:**
   - Read `.openagents/project.json` to understand:
     - `defaultBranch`, `testCommands`, `e2eCommands`, `allowPush`, etc.
   - Skim `.openagents/tasks.jsonl` to see how work is structured.

3. **Check current health:**
   - Run `bun test` in this repo to see current status.
   - If tests fail, note the failures in your log; continue with your task but re-check after your changes before committing.

4. **Start a work log:**
   - Use the `DAY`/`TS` snippet under "Work Logs".
   - Note the task ID you're about to work on and your intent for this session.

---

## Task Tracking with .openagents

This repo uses `.openagents/` as its **sole task system**:

```bash
.openagents/project.json   # ProjectConfig (branch, tests, model, etc.)
.openagents/tasks.jsonl    # One task per line (TaskService schema)
```

**IMPORTANT:**

- Do NOT use markdown TODOs, task lists, or other ad-hoc tracking methods.
- Do NOT use `bd` or `.beads/` in this repo - that system is not used here.
- All task operations go through `.openagents/tasks.jsonl`.

### Task Schema

Tasks in `.openagents/tasks.jsonl` follow this schema:

- `id`, `title`, `description`, `status`, `priority`, `type`, `assignee`, `labels`, `deps`, `commits`, `createdAt`, `updatedAt`, `closedAt`.
- `status` is `open | in_progress | blocked | closed`.
- `priority` is `0..4` (0=P0 critical, 4=backlog).
- `type` is `bug | feature | task | epic | chore`.
- Dependencies have `type`: `blocks | related | parent-child | discovered-from`.

### Task Types

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

### For AI Agents

- Use **TaskService** and **ProjectService** under `src/tasks/` rather than manually editing `.openagents/*.json*`.
- When creating new work:
  - Add a `.openagents` task via TaskService (or let MechaCoder create it).
  - Link discovered work with `discovered-from` dependencies.

### Using the OpenAgents Task CLI

For external agents (Claude Code, Codex, scripts) working in this repo:

```bash
# List all tasks
bun run tasks:list --json

# See ready tasks (no blockers)
bun run tasks:ready --json

# Claim the next ready task (marks it in_progress)
bun run tasks:next --json

# Create a task
bun run tasks:create --title "Fix live e2e harness" --type bug --priority 1 --labels "e2e,golden-loop" --json

# Update/close a task (via JSON stdin)
echo '{"id":"oa-abc123","status":"closed","reason":"Done","commits":["abc123"]}' | bun run tasks:update --json-input --json
```

For full CLI documentation, run `bun src/tasks/cli.ts --help`.
See `docs/mechacoder/TASK-SPEC.md` for the complete task system specification.

### Workflow for AI Agents

1. **Check ready work**: Use TaskService to find tasks with `status: "open"` and no blocking deps
2. **Claim your task**: Update task to `status: "in_progress"`
3. **Work on it**: Implement, test, document
4. **Discover new work?** Create new task with `discoveredFrom` pointing to current task
5. **Complete**: Close task with reason, append commit SHAs
6. **Commit together**: Always commit `.openagents/tasks.jsonl` together with code changes

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
- When your work touches MechaCoder or `.openagents/` internals, include the task IDs you touched in the log header (e.g. `oa-1a2b3c`).

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
> - Uses small, task-focused commits that reference the relevant task ID.
>
> The "never commit unless explicitly asked" rule applies to interactive agents (e.g. chat-based assistants), not to MechaCoder's autonomous loop.

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

### Standard "Next task" flow

When the user says **"Next task."**:

1) **Load project config:**
   - Read `.openagents/project.json` and honor:
     - `defaultBranch`
     - `testCommands`, `e2eCommands`
     - `allowPush`, `allowForcePush`
     - `maxTasksPerRun`, `maxRuntimeMinutes` (if relevant)

2) **Load tasks and find ready work:**
   - Use the existing TaskService/TaskPicker in `src/tasks/*` to:
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
- Clean repository root
- Clear separation between ephemeral and permanent documentation
- Easy to exclude from version control if desired
- Preserves planning history for archeological research
- Reduces noise when browsing the project

### Important Rules

- ✅ Use `.openagents/tasks.jsonl` for ALL task tracking in this repo.
- ✅ Use TaskService/ProjectService under `src/tasks/` for programmatic access.
- ✅ Link discovered work with `discovered-from` dependencies.
- ✅ Check ready tasks via TaskService before asking "what should I work on?"
- ✅ Store AI planning docs in `history/` directory.
- ❌ Do NOT use `bd` or `.beads/` in this repo.
- ❌ Do NOT create markdown TODO lists.
- ❌ Do NOT use external issue trackers.
- ❌ Do NOT duplicate tracking systems.
- ❌ Do NOT clutter repo root with planning documents.

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

### Task Priority

When user says "pick top priority":
- P0 = Critical (do immediately)
- P1 = High (do next)
- P2+ = Lower priority

Don't pick P2/P3 tasks when P0/P1 exist unless explicitly told to.

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
By default, MechaCoder reads and writes tasks from `.openagents/tasks.jsonl` in the target repo.

**See:** [`docs/mechacoder/`](docs/mechacoder/) for full documentation:
- [README.md](docs/mechacoder/README.md) - Overview and quick start
- [MECHACODER-OPS.md](docs/mechacoder/MECHACODER-OPS.md) - Operations guide (start/stop, logs, troubleshooting)
- [GOLDEN-LOOP-v2.md](docs/mechacoder/GOLDEN-LOOP-v2.md) - Golden Loop v2 spec (desktop agent loop)
- [spec.md](docs/mechacoder/spec.md) - .openagents project format and architecture
