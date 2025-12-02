Love where this is headed. You basically have:

* A **local** MechaCoder that already runs on a schedule and edits real code.
* A proven **issue model** (Beads) you like.
* A desire to **own the whole loop** on the user’s machine, with `.openagents` as your project brain.

Here’s a spec you can drop in as something like `docs/MECHACODER-DESKTOP-LOOP.md` (or `docs/OPENAGENTS-PROJECTS.md`) that ties all of this together and sets you up for Bun/Effect/Electrobun + `.openagents`.

---

````markdown
# OpenAgents Desktop Loop & .openagents Project Spec

> **Goal:**
> A local-first, Bun + Effect powered MechaCoder that can run autonomously on a user’s machine, against any project folder they point it at, using a `.openagents` directory as its brain (tasks, config, models, cloud toggles).

This document defines:

1. The **OpenAgents Desktop Loop** (goal-in-loop for the desktop agent)
2. The `.openagents` **project layout and schema**
3. How this integrates with existing **Beads (`bd`)** for the transition
4. Operational rules for **agentic iteration** in this new environment

---

## 1. OpenAgents Desktop Loop (Goal-in-Loop)

### 1.1. Core Loop

For any given project directory (a git repo with `.openagents/`):

> **Loop:**
> **Select project → pick ready task → understand → edit → test → commit/push → update tasks → log → repeat.**

Concrete steps:

1. **Select project**
   - User either:
     - runs CLI with `--dir /path/to/repo`, or
     - uses the desktop UI to pick a repo.
   - Agent finds `/path/to/repo/.openagents/project.json`.

2. **Locate tasks / issues**
   - Agent loads:
     - `.openagents/tasks.jsonl` (OpenAgents-native tasks), OR
     - falls back to `.beads` / `bd ready --json` during transition.
   - Filters tasks to **ready work**:
     - `status = "open"` or `"ready"`,
     - no open blockers (deps resolved),
     - priority ordering.

3. **Pick next task**
   - Choose highest-priority ready task using a simple policy:
     - P0s first,
     - then P1s,
     - within same priority: oldest first.
   - Mark task as `in_progress` in `.openagents/tasks.jsonl` (and/or `bd update --status in_progress` while still using Beads).

4. **Understand**
   - Read relevant files (e.g. via tools).
   - Read task description, deps, and notes from `.openagents/tasks.jsonl`.
   - Optionally read last few logs in `docs/logs/YYYYMMDD/` for context.

5. **Implement**
   - Plan minimal change to satisfy the task.
   - Edit files using the allowed tools (read/edit/write/bash).
   - Keep changes small and focused.

6. **Test**
   - Run project’s tests as defined in `.openagents/project.json`:
     - e.g. `"testCommands": ["pnpm test"]`
   - For projects with explicit e2e flows (like Golden Loop), run e2e commands:
     - e.g. `"e2eCommands": ["E2E_STUB=1 pnpm run test:golden-loop-e2e:local-stub"]`
   - If tests fail, **fix and re-run** until green or until a clear blocker is found.

7. **Commit & push**
   - Only if tests pass:
     - `git add` relevant files,
     - `git commit -m "<task-id>: <summary>"`,
     - `git push` to the configured branch (default `main` unless overridden).
   - If push fails (e.g., conflicts), back off, log, and optionally open a follow-up task instead of force-pushing.

8. **Update tasks**
   - Mark the task as `closed` with a reason and link to commits:
     - In `.openagents/tasks.jsonl`,
     - And/or `bd close` during transition.
   - If new work is discovered, create new tasks in `.openagents` with `discoveredFrom` pointing back.

9. **Log**
   - Append a per-run log under `docs/logs/YYYYMMDD/HHMMSS-agent-run.md` with:
     - Task ID, description,
     - Changes made,
     - Tests run and results,
     - Commit SHA(s),
     - Any follow-up tasks opened.

10. **Repeat**
   - If running in “overnight” mode, loop back to step 2 until:
     - No ready tasks remain, or
     - Config says “stop after N tasks / M minutes”.

### 1.2. Loop Modes

- **Single-run mode**
  Run once, process at most one task, then exit. Good for testing/debugging.

- **Overnight mode**
  Run until:
  - there are no more ready tasks,
  - or time limit is reached (e.g. max 4 hours),
  - or a critical error occurs (e.g. tests failing repeatedly for same task).

- **Interactive desktop mode (Electrobun)**
  User can:
  - See current task and status,
  - Pause/stop the loop,
  - Trigger “Run one task now”,
  - Inspect logs and diffs.

---

## 2. .openagents Project Layout

Each git repo that wants to be MechaCoder-ready includes a `.openagents/` directory at its root.

### 2.1. Directory structure

```text
repo/
├── .openagents/
│   ├── project.json         # Project-level config (required)
│   ├── tasks.jsonl          # OpenAgents-native tasks (like beads issues)
│   ├── models.json          # Optional per-task/per-agent model config
│   ├── agents.json          # Optional per-agent/loop settings
│   └── metadata.json        # Internal versioning, migrations, etc.
└── (rest of repo...)
````

### 2.2. project.json (project-level config)

Minimal schema (v0):

```jsonc
{
  "version": 1,
  "projectId": "openagents",             // human-readable
  "defaultBranch": "main",
  "defaultModel": "x-ai/grok-4.1-fast",  // or other, per-project
  "rootDir": ".",                        // relative to repo root
  "testCommands": ["pnpm test"],
  "e2eCommands": [],                     // e.g. ["E2E_STUB=1 pnpm run test:golden-loop-e2e:local-stub"]
  "allowPush": true,                     // whether agent may push
  "allowForcePush": false,
  "maxTasksPerRun": 3,                   // overnight safety
  "maxRuntimeMinutes": 240,              // 4 hours cap
  "cloud": {
    "useGateway": false,                 // later: OpenAgents cloud integration
    "sendTelemetry": false,
    "relayUrl": null
  }
}
```

### 2.3. tasks.jsonl (OpenAgents-native tasks)

Each line is a JSON object representing a task, similar to Beads issues but simpler and under our control:

```jsonc
// Example line in tasks.jsonl
{
  "id": "oa-1a2b3c",              // short hash
  "title": "Add Golden Loop desktop harness",
  "description": "Implement Bun/Effect harness to run Golden Loop e2e locally.",
  "type": "task",                 // task | bug | feature | epic | chore
  "priority": 1,                  // 0..4 (0=P0, 1=P1, etc.)
  "status": "open",               // open | in_progress | blocked | closed
  "labels": ["testing", "golden-loop"],
  "createdAt": "2025-12-02T15:46:00Z",
  "updatedAt": "2025-12-02T16:00:00Z",
  "closedAt": null,
  "assignee": "mechacoder",       // human or agent
  "deps": [                       // dependencies (IDs)
    {
      "id": "oa-abc123",
      "type": "blocks"            // blocks | related | parent-child | discovered-from
    }
  ],
  "commits": [                    // filled when closed
    "ed3b5a9...",
    "1baeac7..."
  ],
  "source": {
    "repo": "openagents",
    "discoveredFrom": "oa-abc123",
    "externalRef": null           // e.g. GH issue, JIRA ID later
  }
}
```

**ID format:**

* `oa-xxxxxx` (hash-based, like beads)
* We can reuse beads’ birthday-paradox logic later, but v0 can be:

  * random 6-char hex after `oa-`.

### 2.4. agents.json (per-agent config)

Optional, but useful for specifying local loops:

```jsonc
{
  "agents": [
    {
      "id": "mechacoder-desktop",
      "enabled": true,
      "models": {
        "default": "x-ai/grok-4.1-fast"
      },
      "policy": {
        "maxTasksPerRun": 3,
        "maxRetriesPerTask": 3,
        "mustRunTests": true,
        "mustRunE2EForLabels": ["golden-loop", "e2e"]
      }
    }
  ]
}
```

---

## 3. Transition from Beads (`bd`) to .openagents

You love beads (and it’s great), but you want to **own the format** right away.

### 3.1. Short-term strategy

Per repo:

* **nostr-effect**: Keep using `bd` as-is for now (you already have a good setup).
* **openagents (Bun/desktop agent)**:

  * Introduce `.openagents/project.json` and `tasks.jsonl`.
  * Optionally import existing beads issues into `.openagents/tasks.jsonl` once (via a simple converter script).
  * Run OpenAgents Desktop **against `.openagents/tasks.jsonl` first**, and optionally still query `bd` while transitioning.

### 3.2. Converter script idea (later)

* `scripts/bd-to-openagents.ts`:

  * Reads `.beads/issues.jsonl`.
  * Outputs `.openagents/tasks.jsonl` with `id` renamed, fields normalized.
  * Preserves `discovered-from` and `blocks` deps.

### 3.3. Long-term

* `.openagents/tasks.jsonl` becomes the only source of truth.
* `bd` can still be used as a **tool** or **import/export format**, but your agent is not hard-dependent on it.
* OpenAgents Cloud can eventually import/export `.openagents/tasks.jsonl` as part of gateway integration.

---

## 4. Desktop Agent Architecture (Bun + Effect + Electrobun)

### 4.1. Layers

* **Core agent program (Effect)**

  * Lives under `src/agent-core/`.
  * Exports something like `AgentProgram: Effect<AgentEnv, Error, void>`.
  * Has no knowledge of launchd, Electrobun, or Cloudflare.

* **CLI host (Bun)**

  * Entrypoint: `src/cli/mechacoder.ts`.
  * Wires Effect layers for FS, Git, `.openagents`, LLM, logging.
  * Used by launchd, cron, or manual CLI runs.

* **Desktop host ("Electrobun")**

  * Bun-based UI host.
  * Starts a WebView (or equivalent) with a React/TanStack app.
  * UI:

    * Choose repo,
    * Start/stop loops,
    * View logs per run,
    * View current/next tasks.

This lets you run the **same core agent** in:

* Launchd (cron-loop),
* Desktop app (on-demand or continuous),
* Possible cloud worker in the future (if you ever want to).

---

## 5. Agent Behavior & Safety Rules (Desktop)

### 5.1. Safety defaults

* Only commit/push if:

  * tests defined in `project.json` pass,
  * and any configured e2e flows for that task pass (e.g. `golden-loop`-labeled tasks).
* Never force-push unless `allowForcePush` is set and the user explicitly opted in.
* Default to **pushing to a branch** (e.g. `mechacoder/<task-id>`) if you want to avoid touching `main` directly.

### 5.2. Concurrency

* Use a simple lock file per repo:

  * `.openagents/agent.lock` with PID + timestamp.
  * If lock exists and PID is alive, new agent instance should exit.
* This prevents overlapping runs from launchd + desktop.

### 5.3. Autonomy level

This is where your new **AGENTIC-TESTING** style instructions apply:

* Agent does **not** ask “do you want me to run tests?” — it just runs them.
* It does **not** ask “do you want me to push?” if `allowPush=true` and tests are green.
* It only asks you when:

  * Config/secrets are missing,
  * It must change project-level behavior (`project.json`), or
  * A spec-level decision is needed (changing task semantics or `.openagents` schema).

---

## 6. Suggested Initial Beads (.openagents-focused) for openagents

Before you migrate fully off beads, you can file a final set of beads in the **openagents** repo to bootstrap this:

1. **Epic**: “OpenAgents Desktop core (Bun + Effect)”

   * Task: Extract agent logic into `src/agent-core/AgentProgram.ts`.
   * Task: CLI host entry (`src/cli/mechacoder.ts`) with Bun.

2. **Epic**: “.openagents project metadata v0”

   * Task: Define `.openagents/project.json` for openagents.
   * Task: Define `.openagents/tasks.jsonl` schema and add a few seed tasks.
   * Task: Add loader/writer for `.openagents/tasks.jsonl` (Effect Layer).

3. **Epic**: “Desktop loop integration”

   * Task: Change MechaCoder loop to use `.openagents/tasks.jsonl` instead of `bd` (or in addition).
   * Task: Wire per-run logs to reference `.openagents` task IDs.

4. **Epic**: “Electrobun host (minimal UI)”

   * Task: Simple Bun+WebView window that shows:

     * Current repo,
     * Last N runs,
     * Current/next tasks.

Later, you can add epics for:

* Beads-to-OpenAgents converter,
* Cloud/gateway sync,
* Multi-repo orchestration.

---

## 7. Other Considerations

* **Secrets**: `.openagents/project.json` shouldn’t contain secrets. Use env vars or OS keychain, with references in project config (e.g. `"openRouterKeyEnv": "OPENROUTER_API_KEY"`).
* **Multi-project behavior**: Desktop app can maintain a list of “known projects” by scanning for `.openagents` in nested repos.
* **Metrics**: `.openagents/metadata.json` can track counts:

  * tasks completed,
  * last run time,
  * success/failure stats — gives you a way to measure MechaCoder over time.
* **Future cloud bridge**: `.openagents/project.json`’s `cloud` section is where you can later plug into OpenAgents Gateway, Nostr relays, Lightning, etc., without changing the local agent.

---

**TL;DR for the new setup:**

* Every project that wants agentic coding gets a `.openagents/` directory.
* OpenAgents Desktop (Bun + Effect) reads `.openagents/project.json` and `.openagents/tasks.jsonl`, then:

  * picks tasks,
  * edits,
  * tests,
  * commits/pushes,
  * updates tasks,
  * logs,
  * loops.
* Beads (`bd`) can coexist for now, but `.openagents/` becomes your **own, portable, local-first issue/memory format**, ready to be wired into your cloud gateway later.

You can now create a final round of beads in **openagents** to implement this spec, then let MechaCoder (and future agents) build the agentic future from there.
