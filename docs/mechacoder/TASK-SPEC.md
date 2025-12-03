Right now you’ve basically built a really nice **local task substrate** (.openagents) and wired MechaCoder to it. The missing piece is: “How do *other* agents (Claude Code, Codex, raw shell scripts) plug into the same substrate without hand-editing JSON?”

Here’s a spec you can drop in as something like:

* `docs/openagents/OPENAGENTS-TASKS-SPEC.md`
* or `docs/mechacoder/OPENAGENTS-TASKS-SPEC.md`

so that *any* agent has a clear contract.

---

# OpenAgents Task System – Shared Substrate Specification

> **Goal:**
> Provide a single, shared task system for the `openagents` repo that **any agent** can use — MechaCoder, Claude Code, Codex, or humans — via:
>
> - A stable **data model** (`.openagents/tasks.jsonl`, `.openagents/project.json`)
> - A typed **Effect API** (TaskService / ProjectService)
> - A small **CLI surface** for non-TS agents (scripts that read/write JSON on stdin/stdout)

This replaces the old `bd`/`.beads` dependency **for this repo**. Other repos (e.g. `nostr-effect`) may still use their own systems; this spec is specifically for `openagents`.

---

## 1. Project Layout

Every repo that wants to participate in the OpenAgents task system has:

```text
repo/
└── .openagents/
    ├── project.json         # ProjectConfig (tests, branches, models, etc.)
    ├── tasks.jsonl          # Tasks, 1 per line (canonical task store)
    ├── models.json          # Optional model overrides
    └── agents.json          # Optional per-agent settings
````

In this repo (`openagents`), those files are **canonical** for all task/issue tracking.

---

## 2. Data Model

### 2.1. Task

Tasks live in `.openagents/tasks.jsonl`, one JSON object per line.

Shape (conceptual):

```jsonc
{
  "id": "oa-1a2b3c",          // short hash ID (prefix + 4–6 hex chars or hierarchical id like oa-1a2b3c.1)
  "title": "Implement Golden Loop desktop harness",
  "description": "Implement Bun/Effect harness to run Golden Loop e2e locally.",
  "type": "task",             // "bug" | "feature" | "task" | "epic" | "chore"
  "priority": 1,              // 0..4
  "status": "open",           // "open" | "in_progress" | "blocked" | "closed"
  "labels": ["testing", "golden-loop"],
  "assignee": "mechacoder",   // optional; agent/human name or null
  "deps": [
    {
      "id": "oa-abcd12",
      "type": "blocks"        // "blocks" | "related" | "parent-child" | "discovered-from"
    }
  ],
  "commits": [                // commit SHAs attached when closing
    "ed3b5a9...",
    "1baeac7..."
  ],
  "createdAt": "2025-12-02T15:46:00.000Z",
  "updatedAt": "2025-12-02T16:00:00.000Z",
  "closedAt": null,
  "source": {
    "repo": "openagents",     // repo name
    "discoveredFrom": null,   // parent task ID, if any
    "externalRef": null       // external system ID (GH issue, JIRA, etc.), if any
  },
  "notes": ""                 // optional freeform notes
}
```

Validation rules are encoded via `src/tasks/schema.ts` (Effect Schema). Any agent writing tasks **must** conform to that schema.

### 2.2. ProjectConfig

`project.json` holds per-project settings that agents must honor:

```jsonc
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "defaultModel": "x-ai/grok-4.1-fast:free",
  "rootDir": ".",
  "testCommands": ["bun test"],
  "e2eCommands": [],
  "allowPush": true,
  "allowForcePush": false,
  "maxTasksPerRun": 3,
  "maxRuntimeMinutes": 240,
  "cloud": {
    "useGateway": false,
    "sendTelemetry": false,
    "relayUrl": null
  }
}
```

The **ProjectService** in `src/tasks/project.ts` is the source of truth for this shape.

---

## 3. In-Process API (Effect TypeScript)

Agents **inside this repo** (MechaCoder, future local tools) should use the Effect-based services rather than touching the filesystem directly.

### 3.1. TaskService

Defined under `src/tasks/service.ts` (names illustrative; consult actual module):

Core operations:

* `TaskService.loadAll(): Effect<ReadonlyArray<Task>>`
* `TaskService.create(TaskCreate): Effect<Task>`
* `TaskService.update(id: TaskId, TaskUpdate): Effect<Task>`
* `TaskService.close(id: TaskId, ClosePayload): Effect<Task>`
* `TaskService.ready(options?): Effect<ReadonlyArray<Task>>`
* `TaskService.pickNextReady(options?): Effect<Option<Task>>`

Responsibilities:

* Read/write `.openagents/tasks.jsonl`.
* Apply schema validation (`Task`, `TaskCreate`, `TaskUpdate`).
* Filter/sort by:

  * `status`,
  * `priority`,
  * `createdAt` (age),
  * labels, assignee, etc.
* Compute “ready” tasks:

  * `status` in `"open" | "in_progress"`,
  * No open `blocks`/`parent-child` deps,
  * `related` and `discovered-from` deps do not block.

### 3.2. ProjectService

Defined under `src/tasks/project.ts`:

* `ProjectService.load(): Effect<Option<ProjectConfig>>`
* `ProjectService.save(ProjectConfig): Effect<void>`

Responsibilities:

* Read/write `.openagents/project.json`.
* Apply defaults (e.g. `testCommands` default to `["bun test"]`).
* Let agents know:

  * which branch to push,
  * which tests to run,
  * whether they’re allowed to push at all.

### 3.3. ID generator

Defined under `src/tasks/id.ts`:

* `generateShortId(prefix?: string, length?: number): TaskId`
* `generateChildId(parentId: TaskId): TaskId`
* `isChildOf(parent, child)`, `getParentId`, etc.

This is the only supported way to create new task IDs. Agents **must not** invent their own ID formats.

---

## 4. CLI Surface for External Agents

Agents that don’t run inside this repo’s Bun/TS runtime (e.g. Claude Code, Codex, shell scripts) need a **thin CLI** that:

* Wraps TaskService/ProjectService,
* Talks JSON over stdin/stdout,
* Follows the same schema.

We standardize on a **`tasks` CLI** exposed via `package.json` scripts.

### 4.1. Proposed scripts (to be implemented / refined)

In `package.json`:

```jsonc
{
  "scripts": {
    "tasks:init": "bun src/tasks/cli.ts init",
    "tasks:list": "bun src/tasks/cli.ts list",
    "tasks:ready": "bun src/tasks/cli.ts ready",
    "tasks:next": "bun src/tasks/cli.ts next",
    "tasks:create": "bun src/tasks/cli.ts create",
    "tasks:update": "bun src/tasks/cli.ts update"
  }
}
```

We design the CLI with **JSON in/out** so any agent can call it safely.

#### 4.1.1. Initialize project (`tasks:init`)

Initializes `.openagents` for a repo:

```bash
# Minimal, in repo root
bun run tasks:init

# Custom projectId
bun run tasks:init --project-id openagents-desktop
```

Effects:

* Creates `.openagents/` directory.
* Writes `project.json` with defaults.
* Writes empty `tasks.jsonl`.

#### 4.1.2. List tasks (`tasks:list`)

Lists tasks, filterable via flags, outputs JSON array.

```bash
bun run tasks:list --status open --priority-max 2 --json
```

Example output:

```jsonc
[
  {
    "id": "oa-1a2b3c",
    "title": "Implement Golden Loop desktop harness",
    "status": "open",
    "priority": 1,
    "labels": ["testing", "golden-loop"],
    "deps": [],
    ...
  }
]
```

#### 4.1.3. Ready tasks (`tasks:ready`)

Lists tasks that are “ready” (no open blockers).

```bash
bun run tasks:ready --limit 10 --json
```

Same output shape as `tasks:list`.

#### 4.1.4. Take next ready task (`tasks:next`)

Atomically picks the top ready task, marks it `in_progress`, prints it.

```bash
bun run tasks:next --json
```

Output (single object or `null`):

```jsonc
{
  "id": "oa-1a2b3c",
  "title": "Implement Golden Loop desktop harness",
  "status": "in_progress",
  ...
}
```

External agents (Claude Code, etc.) should call this to **claim a task** rather than manually scanning `tasks.jsonl`.

#### 4.1.5. Create task (`tasks:create`)

Creates a new task. Accepts either flags or JSON via stdin.

**Flag-based (simple):**

```bash
bun run tasks:create \
  --title "Fix e2e live harness" \
  --type "bug" \
  --priority 1 \
  --labels "e2e,golden-loop" \
  --json
```

**JSON-based (advanced):**

```bash
cat << 'EOF' | bun run tasks:create --json-input
{
  "title": "Add desktop settings UI",
  "description": "Basic settings panel for OpenAgents Desktop.",
  "type": "feature",
  "priority": 2,
  "labels": ["ui", "desktop"]
}
EOF
```

Output: full task object with generated `id`.

#### 4.1.6. Update / close task (`tasks:update`)

Update fields or close a task. Accepts JSON input.

```bash
# Mark task in_progress
printf '{"id":"oa-1a2b3c","status":"in_progress","assignee":"claude"}' \
  | bun run tasks:update --json-input

# Close task with commits:
printf '{
  "id":"oa-1a2b3c",
  "status":"closed",
  "commits":["ed3b5a9...", "1baeac7..."],
  "closedAt":"2025-12-02T17:00:00.000Z"
}' | bun run tasks:update --json-input
```

**Agents must not edit `.openagents/tasks.jsonl` by hand.** They should always go through `tasks:*` scripts or, if running in-process, through TaskService.

---

## 5. Standard Flows for Different Agent Types

### 5.1. MechaCoder (in-process, autonomous)

* Reads `ProjectConfig` + `TaskService` directly from TS.
* Golden Loop v2 path:

  * uses TaskPicker to claim tasks,
  * runs `testCommands`/`e2eCommands`,
  * commits/pushes,
  * updates tasks via TaskService.

No CLI required for MechaCoder; it lives inside this repo.

### 5.2. Claude Code / Codex / other LSP-like agents

Assume these agents have:

* Shell access,
* But don’t necessarily run TS/Effect code directly.

**Recommended behavior:**

* To find work:

  * `bun run tasks:ready --json`
  * or `bun run tasks:next --json` to claim one.
* To create new tasks:

  * `bun run tasks:create` with flags or JSON.
* To update/close:

  * Use `bun run tasks:update` with JSON input.

**No bd commands, no direct JSON editing.**

### 5.3. Human operators

* May inspect tasks via:

  ```bash
  cat .openagents/tasks.jsonl | jq '.'
  ```

* May create/update tasks either by:

  * running `bun run tasks:create` / `tasks:update`, or
  * using a future OpenAgents Desktop UI that calls the same APIs.

---

## 6. Concurrency & Locking

Multiple agents (and humans) may interact with `.openagents/tasks.jsonl`.

To stay safe:

* TaskService should:

  * Use file-level locking (or atomic write patterns) when writing.
  * Re-load from disk before mutating when in doubt.
* The `tasks:next` script is the **only supported way** to atomically claim a task for external agents:

  * It reads current tasks,
  * Picks the top ready,
  * Marks it `in_progress`,
  * Writes back and returns it.

Agents should **never**:

* Mark an `open` task as `in_progress` by directly editing JSON,
* Claim the same task ID from two different processes.

---

## 7. Safety & Evolution

* `.openagents/schema.ts` (Task/Project schemas) is the **only place** where the data model should be extended.
* Any evolution of the schema must:

  * Update Effect schemas,
  * Update CLI,
  * Update tests,
  * Remain backward-compatible with existing `tasks.jsonl` where possible.

Agents that depend on the CLI interface should:

* Treat the JSON they receive as canonical,
* Be defensive about unrecognized fields (ignore them rather than failing).

---

## 8. Summary

* `.openagents/project.json` + `.openagents/tasks.jsonl` form a **shared substrate** for tasks in this repo.
* **In-process agents** (MechaCoder, internal tools) use TaskService/ProjectService directly.
* **External agents** (Claude Code, Codex, shell scripts) use a small set of `bun run tasks:*` scripts that read/write JSON.
* **No beads (`bd`) are needed in this repo anymore.** All tracking is done via `.openagents/*`.

If you are an agent working in this repo:

> Use `.openagents` and the TaskService/CLI described here.
> Do not invent new task stores or use markdown TODOs.
> This is the API contract for work management in `openagents`.

```

---

Implementation-wise, you can:

- Add this spec under `docs/mechacoder/` or `docs/openagents/`.
- Then add a one-liner in AGENTS.md:
  “For the full task system contract and CLI, see `docs/openagents/OPENAGENTS-TASKS-SPEC.md`.”

That gives MechaCoder **and** any other agent the same substrate, with one clear contract.
