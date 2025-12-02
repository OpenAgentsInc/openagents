# GOLDEN LOOP v2 – OpenAgents Desktop / MechaCoder

> **Vision (v2):**
> On a single machine, with no cloud dependencies, it should feel this simple:
>
> **OpenAgents Desktop → select a repo → click “Run MechaCoder” → watch a task get implemented, tested, committed, pushed, and marked done.**

This is the **local-first Golden Loop** for the **OpenAgents Desktop** app, running MechaCoder against any project folder the user chooses.

v1 was “Cloud MechaCoder” (`openagents.com` + Cloudflare + `/sessions`).
v2 is “Desktop MechaCoder” (`openagents` repo + Bun + Effect) with tasks stored locally in `.openagents/`.

---

## 0. Scope & Non-Goals

**In scope for Golden Loop v2:**

- Single user, single machine.
- One git repo at a time (user chooses which).
- Tasks tracked in the local project:
  - `.openagents/tasks.jsonl` (OpenAgents-native), and/or
  - `bd`/`.beads` as a transitional backend.
- Git remotes assumed to be already configured (SSH or HTTPS).
- MechaCoder:
  - Picks a ready task,
  - Edits code,
  - Runs tests,
  - Commits & pushes,
  - Marks the task done,
  - Logs a summary.

**Explicitly _not_ in scope for v2:**

- Cloud/OpenAgents Gateway job queues (`/sessions`).
- Cloudflare Workers / Durable Objects / Nostr relay as hard dependencies.
- Multi-machine coordination (only one desktop agent per repo in v2).
- Multi-repo orchestration in a single loop (one repo per run).
- Wallet, nostr identity, payments, etc. (those are later surfaces in the Desktop app).

If something requires external job routing, multi-agent coordination, or wallets, it belongs in a **future loop** (Gateway/Wallet loops), not in **Golden Loop v2**.

---

## 1. Golden Loop v2 – User Experience

From the user’s perspective:

1. **Launch OpenAgents Desktop**
   - The app shows a home screen with:
     - A list of known projects (repos with `.openagents/`), and
     - A “Add Project” button to point at a new folder.

2. **Select a project**
   - User selects a repo (e.g. `~/code/openagents` or `~/code/nostr-effect`).
   - The app finds `.openagents/project.json` (or offers to create one if missing).
   - The app shows:
     - Project name,
     - Default branch,
     - Summary of open tasks (from `.openagents/tasks.jsonl` and/or beads).

3. **Start MechaCoder**
   - User clicks one of:
     - **“Run one task”** – do a single task then stop.
     - **“Run overnight”** – loop until:
       - no ready tasks remain, or
       - a time/iteration limit is reached.
   - User can see:
     - Current task ID/title,
     - A live log stream (or at least “running / idle / failed / done”).

4. **Watch one loop complete**
   - For each task, the user sees (in logs or UI):
     - Task chosen,
     - Files touched,
     - Tests run and their results,
     - Commit message + SHA,
     - Task marked as done.

5. **Review work in the morning**
   - In the repo:
     - Git history shows new commits on the expected branch.
   - In `.openagents/tasks.jsonl`:
     - Completed tasks have `status: "closed"` and `commits: [...]`.
   - In `docs/logs/YYYYMMDD/`:
     - Per-run logs show what happened each cycle.

That’s the **human** Golden Loop: open app → choose project → run → see real work land.

---

## 2. Golden Loop v2 – Agent Loop (Implementation Contract)

From the agent’s point of view, one iteration of the loop is:

> **Find project → load config → choose ready task → understand → implement → test → commit & push → update task → log.**

### 2.1. Project discovery

Given a working directory:

- Find `.openagents/project.json` and load:
  - `defaultBranch`,
  - `testCommands`,
  - `e2eCommands` (if any),
  - `allowPush`,
  - safety limits (`maxTasksPerRun`, `maxRuntimeMinutes`, etc.).
- If `.openagents/project.json` does not exist:
  - Do **not** invent behavior.
  - Log the issue and either:
    - Prompt the user via Desktop UI to initialize `.openagents`, or
    - Use a CLI flow to generate a minimal project.json (v2+).

### 2.2. Task selection

- Load tasks from `.openagents/tasks.jsonl`:
  - Filter to `status in ["open", "ready"]`.
  - Exclude tasks blocked by open `deps` with `type: "blocks"` or `parent-child`.
- Sort ready tasks by:
  - `priority` (0 → 4),
  - then `createdAt` (oldest first).
- Choose the top task and mark it `in_progress` with updated timestamp.
- `.openagents/tasks.jsonl` is the source of truth for this repo.

### 2.3. Understand

- Read:
  - Task `title`, `description`, `labels`, `deps`.
  - Any referenced files (paths mentioned in description or notes).
  - Recent logs (optional) for additional context on related work.

- Build a simple internal plan:
  - What to change,
  - Which files to touch,
  - What tests to run.

### 2.4. Implement

- Apply changes using the available code-editing tools (read/edit/write/bash).
- Keep changes tightly scoped to the task.
- If new follow-up work is discovered:
  - Add new entries to `.openagents/tasks.jsonl` with `discoveredFrom: <current-task-id>`.

### 2.5. Test

- Run all commands in `project.json.testCommands` (e.g. `["pnpm test"]`).
- If task or labels require additional tests:
  - e.g. tasks labeled `["golden-loop", "e2e"]` should run `e2eCommands`.
- If tests fail:
  - Diagnose and fix if possible within reasonable scope.
  - If blocked by external constraints (e.g. missing secrets), log clearly and:
    - Leave task `in_progress` and open a new blocking task (`blocked`), or
    - Mark as `blocked` with a `reason` and `blockedBy`.

**Golden Loop v2 acceptance rule:**
**No commit or push is allowed if configured tests fail.**

### 2.6. Commit & push

If tests pass:

- Stage all relevant files (code, tests, docs, `.openagents/tasks.jsonl`).
- Commit with a message including the task ID, e.g.:

  ```text
  oa-1a2b3c: add Golden Loop desktop harness
````

* If `allowPush = true`, push to the configured branch (`defaultBranch` or a configured work branch).
* If push fails (e.g., remote changes):

  * Perform a **safe** resolution:

    * Option A: open a new task “Resolve conflicts for <sha>/<task>”.
    * Option B: if policy allows, rebase & retry push with care.
  * Do **not** force-push unless explicitly configured.

### 2.7. Update task

* Set `status: "closed"` if the task was fully completed, else `blocked`.
* Append commit SHA(s) under `commits`.
* Update `closedAt` and `updatedAt`.
* If new tasks were created in the process, ensure they are written to `tasks.jsonl` in the same run.

### 2.8. Log and exit (or loop)

* Write a per-run log under `docs/logs/YYYYMMDD/HHMMSS-agent-run.md` with:

  * Task ID, title,
  * Summary of changes,
  * Tests run + results,
  * Commit SHA(s),
  * Any follow-up tasks created.

* In **single-run** mode:

  * Exit after one task.

* In **overnight** mode:

  * Continue until:

    * No ready tasks remain,
    * `maxTasksPerRun` is reached, or
    * `maxRuntimeMinutes` is exceeded.

---

## 3. Acceptance Criteria (Golden Loop v2 Done)

Golden Loop v2 is considered **implemented** when the following are true for at least one project (e.g. `openagents` repo):

1. **Config & tasks**

   * `.openagents/project.json` exists with:

     * `defaultBranch`,
     * `testCommands`,
     * `allowPush` set appropriately.
   * `.openagents/tasks.jsonl` exists with ≥ 1 task in `status: "open"`.

2. **Agent loop**

   * From CLI:

     ```bash
     cd ~/code/openagents
     bun run src/cli/mechacoder.ts --dir . --once
     ```

     results in:

     * One task picked from `.openagents/tasks.jsonl`,
     * Code changes applied,
     * `pnpm test` (or configured tests) run,
     * Commit created on the configured branch (if tests pass),
     * Task updated to `closed` with commit SHA,
     * A new `docs/logs/YYYYMMDD/HHMMSS-agent-run.md` file created.

3. **Desktop integration**

   * From OpenAgents Desktop UI:

     * User can select the `openagents` repo,
     * Click “Run one task”,
     * See status as the same loop executes,
     * Verify the same end state (commit + task closed + log).

4. **Safety**

   * If tests fail, Golden Loop v2 does **not**:

     * create a commit,
     * nor push changes.
   * At least one failure scenario has been tested and handled gracefully:

     * e.g. test fails → changes kept in working tree, task remains `in_progress` or `blocked`, log explains why.

5. **Autonomy**

   * At no point does the agent require human confirmation for:

     * Running tests,
     * Committing/pushing (within configured safety settings),
     * Updating tasks/logs.
   * The only required human actions are:

     * Selecting the repo,
     * Starting/stopping the loop,
     * Configuring `.openagents/project.json` / initial tasks.

---

## 4. Future Extensions (Out of Scope for v2)

These belong in future loops or specs, not in Golden Loop v2:

* **Multi-repo orchestration** (e.g. coordinating changes across `openagents`, `nostr-effect`, `openagents.com`).
* **Cloud job intake** from OpenAgents Gateway (Nostr/Lightning-backed jobs).
* **Wallet integration** (Lightning, Nostr zaps, etc.) inside the Desktop app.
* **Team multi-agent coordination** across machines.

---

If you’re updating or extending MechaCoder:

> **Golden rule for v2:**
> **If a change affects how MechaCoder chooses tasks, edits code, runs tests, commits, or updates `.openagents/tasks.jsonl`, you must re-run the Desktop Golden Loop and ensure it still matches this spec.**
