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

## 2. Golden Loop v2 – Agent Architecture

### 2.0. Orchestrator / Subagent Split

Following Anthropic's ["Effective Harnesses for Long-Running Agents"](https://www.anthropic.com/engineering/effective-harnesses-for-long-running-agents), MechaCoder uses a two-agent architecture optimized for overnight automation:

| Agent | Runs | Responsibilities |
|-------|------|------------------|
| **Orchestrator** | Once per session | Orient, select task, decompose into subtasks, coordinate verification, commit/push, update task, write progress for next session |
| **Coding Subagent** | Per subtask | Implement one subtask with minimal prompt (~50 tokens), 4 tools (read/write/edit/bash) |

**Why this split?**

1. **Coding prompts stay minimal** - Models are RL-trained for coding; they don't need 10K tokens of instructions
2. **Orchestration is explicit** - Not hidden in a mega-prompt that confuses the model
3. **Subtask decomposition prevents "one-shot" failures** - Breaking work into pieces keeps each invocation focused
4. **Progress files bridge context windows** - Next session can orient quickly without re-exploring

**Coordination Artifacts:**

| File | Purpose |
|------|---------|
| `.openagents/progress.md` | Session summary for next session to read |
| `.openagents/subtasks/{taskId}.json` | Subtask list with status tracking |
| `.openagents/init.sh` | Startup script to verify clean state |

---

### 2.1. Implementation Contract

From the orchestrator's point of view, one iteration of the loop is:

> **Orient → select task → decompose → invoke subagent per subtask → verify → commit & push → update task → log for next session.**

### 2.2. Project discovery

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

### 2.3. Task selection

- Load tasks from `.openagents/tasks.jsonl`:
  - Filter to `status in ["open", "ready"]`.
  - Exclude tasks blocked by open `deps` with `type: "blocks"` or `parent-child`.
- Sort ready tasks by:
  - `priority` (0 → 4),
  - then `createdAt` (oldest first).
- Choose the top task and mark it `in_progress` with updated timestamp.
- `.openagents/tasks.jsonl` is the source of truth for this repo.

### 2.4. Understand

- Read:
  - Task `title`, `description`, `labels`, `deps`.
  - Any referenced files (paths mentioned in description or notes).
  - Recent logs (optional) for additional context on related work.

- Build a simple internal plan:
  - What to change,
  - Which files to touch,
  - What tests to run.

### 2.5. Implement

- Apply changes using the available code-editing tools (read/edit/write/bash).
- Keep changes tightly scoped to the task.
- If new follow-up work is discovered:
  - Add new entries to `.openagents/tasks.jsonl` with `discoveredFrom: <current-task-id>`.

### 2.6. Test

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

### 2.7. Commit & push

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

### 2.8. Update task

* Set `status: "closed"` if the task was fully completed, else `blocked`.
* Append commit SHA(s) under `commits`.
* Update `closedAt` and `updatedAt`.
* If new tasks were created in the process, ensure they are written to `tasks.jsonl` in the same run.

### 2.9. Log and exit (or loop)

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

## 4. Failure Modes & Recovery

This section documents known failure scenarios and how Golden Loop v2 handles them.

### 4.1. Test/Typecheck Failures

| Scenario | Behavior |
|----------|----------|
| Tests fail after code change | No commit. Task stays `in_progress`. Blocker logged in progress file. |
| Typecheck fails at session start | Orchestrator injects "fix-typecheck" subtask before proceeding. |
| Consecutive failures (3x) | Task blocked. `MAX_CONSECUTIVE_FAILURES` prevents infinite retry loops. |

**Recovery**: The next session reads blockers from `progress.md` and either retries or marks the task `blocked`.

### 4.2. Verification Command Issues

| Scenario | Behavior |
|----------|----------|
| Verification times out (120s default) | Treated as failure. Falls back to minimal subagent if enabled. |
| Verification throws error | Caught and treated as failure with empty outputs. |
| Empty/whitespace outputs | Default error message: "Verification failed (typecheck/tests)". |

**Recovery**: Fallback to minimal subagent (if `fallbackToMinimal: true`). Error details preserved in `verificationOutputs`.

### 4.3. Claude Code Subagent Failures

| Scenario | Behavior |
|----------|----------|
| Claude Code unavailable | Falls back to minimal subagent. |
| Claude Code detection fails | Falls back to minimal subagent. |
| Claude Code times out (50min default) | Retried up to 3x with exponential backoff. |
| Rate limit / auth errors | Falls back to minimal subagent (if enabled). |

**Recovery**: Session resume via `resumeSessionId` and `forkSession` options. Previous session ID tracked in progress file.

### 4.4. Progress File Issues

| Scenario | Behavior |
|----------|----------|
| Missing progress file | Fresh session created. No previous context. |
| Malformed/corrupted progress file | Parser returns partial results with safe defaults. |
| Truncated file | Parses what's available, missing fields get defaults. |

**Recovery**: Progress parsing is fault-tolerant. Missing sections don't crash the orchestrator.

### 4.5. Git Operation Failures

| Scenario | Behavior |
|----------|----------|
| Push fails (conflicts) | Changes kept in working tree. Task stays `in_progress`. |
| Push fails (remote changes) | Option A: New task for conflict resolution. Option B: Rebase if policy allows. |
| Force push required | **Never** force-pushed unless `allowForcePush` explicitly set. |

**Recovery**: Conflict tasks created for human review. Changes preserved locally.

### 4.6. Task System Failures

| Scenario | Behavior |
|----------|----------|
| No ready tasks | Loop exits cleanly. |
| Task blocked by dependencies | Skipped. Next ready task selected. |
| Concurrent modification of tasks.jsonl | **Not fully handled** - single agent per repo assumed in v2. |

**Recovery**: Lock file (`.openagents/agent.lock`) prevents overlapping runs.

### 4.7. Init Script Failures

| Scenario | Behavior |
|----------|----------|
| Missing init.sh | Continues without init (non-fatal). |
| Init script fails | Failure logged. Continues with warning. |
| Init script times out (60s) | Treated as failure. Continues with warning. |

**Recovery**: Init failures are warnings, not blockers. Session continues.

---

## 5. Recovery Playbooks

This section provides step-by-step instructions for recovering from common failure scenarios. Use these playbooks when the orchestrator stops or encounters issues.

### 5.1. Playbook: Test/Typecheck Failure Recovery

**Symptoms:**
- Session ends with `testsPassingAfterWork: false` in progress.md
- Subtask marked as `failed` with `error` field populated
- Task remains `in_progress` (not committed)

**Recovery Steps:**

1. **Inspect the failure output:**
   ```bash
   # Check progress file for failure details
   cat .openagents/progress.md

   # Check subtask file for error and failure count
   cat .openagents/subtasks/<taskId>.json | jq '.subtasks[] | select(.status == "failed")'
   ```

2. **Understand what changed:**
   ```bash
   # See uncommitted changes from the failed attempt
   git status
   git diff

   # Check which files were modified
   jq '.subtasks[].filesModified' .openagents/subtasks/<taskId>.json
   ```

3. **Decide on recovery strategy:**

   **Option A: Retry (let orchestrator fix it)**
   - The orchestrator tracks `failureCount` per subtask
   - On next run, it will retry with `resumeStrategy: "fork"` (fresh context)
   - After 3 consecutive failures, task is marked `blocked`

   ```bash
   # Simply rerun - orchestrator will retry
   bun run src/cli/mechacoder.ts --dir . --once
   ```

   **Option B: Manual fix before retry**
   ```bash
   # Fix the failing tests/typecheck manually
   bun run typecheck
   bun test

   # Reset subtask status for fresh attempt
   # Edit .openagents/subtasks/<taskId>.json:
   # - Set failed subtask's status to "pending"
   # - Reset failureCount to 0
   # - Clear claudeCode.sessionId to force fresh start

   # Rerun orchestrator
   bun run src/cli/mechacoder.ts --dir . --once
   ```

   **Option C: Discard changes and skip**
   ```bash
   # Discard all uncommitted changes
   git checkout -- .
   git clean -fd

   # Mark task as blocked to skip it
   bun run tasks:update --id <taskId> --status blocked --reason "Manual: skipped due to persistent failures"
   ```

4. **Prevent infinite loops:**
   - After 3 consecutive failures (`MAX_CONSECUTIVE_FAILURES`), the task is automatically blocked
   - The orchestrator sets `resumeStrategy: "fork"` after each failure to try fresh context
   - Check `failureCount` in subtask JSON to see how many attempts have been made

---

### 5.2. Playbook: Agent Crash Recovery

**Symptoms:**
- Orchestrator process terminated unexpectedly (killed, OOM, power loss)
- `.openagents/progress.md` may be incomplete or missing `completedAt`
- Subtask status shows `in_progress` but no agent is running
- Git working tree may have uncommitted changes

**Recovery Steps:**

1. **Detect the crash state:**
   ```bash
   # Check if any agent process is running
   ps aux | grep mechacoder

   # Check for stale lock file
   cat .openagents/agent.lock 2>/dev/null && echo "Lock exists" || echo "No lock"

   # Check progress file completion status
   grep "Completed:" .openagents/progress.md
   ```

2. **Clean up stale state:**
   ```bash
   # Remove stale lock file if process is dead
   rm -f .openagents/agent.lock

   # Check progress for in-progress subtasks
   grep "Subtasks In Progress" .openagents/progress.md
   ```

3. **Assess working tree state:**
   ```bash
   # Check for uncommitted changes
   git status --porcelain

   # If changes exist, decide:
   # - Keep them (they may be partial work)
   # - Discard them (start fresh)
   ```

4. **Reset subtask for resumption:**
   ```bash
   # Find the in-progress subtask
   cat .openagents/subtasks/<taskId>.json | jq '.subtasks[] | select(.status == "in_progress")'

   # Option A: Resume from where it left off
   # Leave subtask as-is; orchestrator will continue using sessionId

   # Option B: Force fresh start
   # Edit subtask JSON:
   # - Set status to "pending"
   # - Set claudeCode.resumeStrategy to "fork"
   # - Optionally clear claudeCode.sessionId
   ```

5. **Restart the orchestrator:**
   ```bash
   # The orchestrator will read progress.md and resume appropriately
   bun run src/cli/mechacoder.ts --dir . --once
   ```

**Session Resumption Behavior:**
- If `claudeCode.sessionId` exists, orchestrator attempts to resume that session
- If `resumeStrategy: "fork"`, it creates a new session forked from the previous one
- If resumption fails, it falls back to minimal subagent or fresh start

---

### 5.3. Playbook: Dirty Workspace Detection & Cleanup

**Goal:** Ensure the working tree is in a clean, known state before starting work.

**Detection (via init.sh or manual):**

Create `.openagents/init.sh` to automate detection:

```bash
#!/bin/bash
# .openagents/init.sh - Workspace health check

set -e

echo "=== Workspace Health Check ==="

# 1. Check for uncommitted changes
if [ -n "$(git status --porcelain)" ]; then
    echo "WARNING: Uncommitted changes detected:"
    git status --short

    # Option: Fail init to prevent work on dirty tree
    # exit 1

    # Option: Continue with warning (current behavior)
    echo "Proceeding with dirty working tree..."
fi

# 2. Check for stale lock files
if [ -f ".openagents/agent.lock" ]; then
    LOCK_PID=$(cat .openagents/agent.lock | head -1)
    if ! kill -0 "$LOCK_PID" 2>/dev/null; then
        echo "WARNING: Stale lock file from PID $LOCK_PID (process not running)"
        rm -f .openagents/agent.lock
        echo "Removed stale lock file"
    else
        echo "ERROR: Another agent is running (PID $LOCK_PID)"
        exit 1
    fi
fi

# 3. Check for in-progress subtasks from crashed sessions
for subtask_file in .openagents/subtasks/*.json; do
    if [ -f "$subtask_file" ]; then
        IN_PROGRESS=$(jq -r '.subtasks[] | select(.status == "in_progress") | .id' "$subtask_file" 2>/dev/null || true)
        if [ -n "$IN_PROGRESS" ]; then
            echo "WARNING: Found in-progress subtask: $IN_PROGRESS"
            echo "Previous session may have crashed. Will attempt resumption."
        fi
    fi
done

# 4. Verify tests pass at session start
echo "Running quick typecheck..."
if ! bun run typecheck 2>&1; then
    echo "WARNING: Typecheck failing at session start"
    echo "Orchestrator will inject fix-typecheck subtask"
fi

echo "=== Health check complete ==="
```

**Manual Cleanup Procedure:**

```bash
# 1. Identify dirty state
git status
git stash list  # Check for stashed changes

# 2. Decide what to do with uncommitted changes
# Option A: Commit them (if they're valid work)
git add -A
git commit -m "WIP: save uncommitted work from crashed session"

# Option B: Stash them (to review later)
git stash push -m "Uncommitted work from crashed session $(date +%Y%m%d-%H%M)"

# Option C: Discard them (if they're invalid/partial)
git checkout -- .
git clean -fd

# 3. Clean up orchestrator state
rm -f .openagents/agent.lock

# 4. Reset in-progress subtasks
# Edit .openagents/subtasks/<taskId>.json as needed

# 5. Verify clean state
git status  # Should show clean working tree
bun run typecheck
bun test
```

---

### 5.4. Playbook: Safe Task Resumption

**Goal:** Resume a partially-completed task without losing progress or creating duplicate work.

**Understanding Resumption State:**

The orchestrator tracks resumption through:
1. `.openagents/progress.md` - Session summary with blockers and next steps
2. `.openagents/subtasks/<taskId>.json` - Per-subtask status and Claude Code session IDs
3. `.openagents/tasks.jsonl` - Overall task status

**Resumption Flow:**

```
Orchestrator Start
    │
    ├── Read progress.md
    │   └── Extract: previous blockers, completed subtasks, session IDs
    │
    ├── Run init.sh (if exists)
    │   └── Verify workspace health
    │
    ├── Check task status
    │   ├── If task is "closed" → skip, pick next task
    │   ├── If task is "blocked" → skip, pick next task
    │   └── If task is "in_progress" → resume
    │
    ├── Load subtasks/<taskId>.json
    │   ├── Skip subtasks with status "done" or "verified"
    │   ├── Resume subtasks with status "in_progress" or "pending"
    │   └── Check claudeCode.sessionId for session resumption
    │
    └── For each pending/in-progress subtask:
        ├── If claudeCode.sessionId exists:
        │   ├── If resumeStrategy == "fork" → fork from that session
        │   └── If resumeStrategy == "continue" → resume session directly
        └── If no sessionId → start fresh
```

**Step-by-Step Resumption:**

1. **Check what was completed:**
   ```bash
   # View completed subtasks
   cat .openagents/subtasks/<taskId>.json | jq '.subtasks[] | select(.status == "done") | .id'

   # View what's still pending
   cat .openagents/subtasks/<taskId>.json | jq '.subtasks[] | select(.status != "done") | {id, status, failureCount}'
   ```

2. **Check for blockers from last session:**
   ```bash
   grep -A 5 "### Blockers" .openagents/progress.md
   ```

3. **Decide on resumption strategy:**

   **Option A: Continue where left off (default)**
   ```bash
   # Just run - orchestrator handles resumption automatically
   bun run src/cli/mechacoder.ts --dir . --once
   ```

   **Option B: Force fresh start on specific subtask**
   ```bash
   # Edit .openagents/subtasks/<taskId>.json:
   # - Set claudeCode.resumeStrategy to "fork"
   # - Or remove claudeCode.sessionId entirely

   bun run src/cli/mechacoder.ts --dir . --once
   ```

   **Option C: Skip problematic subtask**
   ```bash
   # Edit .openagents/subtasks/<taskId>.json:
   # - Set subtask status to "done" (mark as complete even if not)
   # - Add a note in the description

   bun run src/cli/mechacoder.ts --dir . --once
   ```

4. **Handle merge conflicts or remote changes:**
   ```bash
   # If push failed due to remote changes
   git fetch origin
   git log --oneline HEAD..origin/main

   # Rebase local work
   git rebase origin/main

   # If conflicts, resolve manually then continue
   git rebase --continue

   # Push the resolved changes
   git push origin HEAD
   ```

5. **Verify resumption was successful:**
   ```bash
   # Check progress file was updated
   cat .openagents/progress.md | head -20

   # Check subtask status
   cat .openagents/subtasks/<taskId>.json | jq '.subtasks[] | {id, status}'

   # Check task was closed
   grep <taskId> .openagents/tasks.jsonl | jq '.status'
   ```

---

### 5.5. Quick Reference: Recovery Decision Tree

```
Agent stopped unexpectedly?
    │
    ├── Check: Is there a stale lock file?
    │   └── Yes → Remove .openagents/agent.lock
    │
    ├── Check: Are there uncommitted changes?
    │   ├── Yes, valid work → git stash or commit
    │   └── Yes, garbage → git checkout -- . && git clean -fd
    │
    ├── Check: What does progress.md say?
    │   ├── Has blockers → Address blockers first
    │   └── In progress → Resumption will continue
    │
    └── Restart: bun run src/cli/mechacoder.ts --dir . --once

Tests failing after changes?
    │
    ├── Check: failureCount in subtask JSON
    │   ├── < 3 → Orchestrator will retry automatically
    │   └── >= 3 → Task blocked, needs manual intervention
    │
    ├── Fix option: Manual code fix
    │   └── Fix code → Reset subtask status → Retry
    │
    └── Skip option: Mark task as blocked
        └── bun run tasks:update --id <id> --status blocked

Can't resume Claude Code session?
    │
    ├── Session expired or unavailable
    │   └── Set claudeCode.resumeStrategy = "fork" in subtask JSON
    │
    └── Want completely fresh start
        └── Remove claudeCode.sessionId from subtask JSON
```

---

## 6. Future Extensions (Out of Scope for v2)

These belong in future loops or specs, not in Golden Loop v2:

* **Multi-repo orchestration** (e.g. coordinating changes across `openagents`, `nostr-effect`, `openagents.com`).
* **Cloud job intake** from OpenAgents Gateway (Nostr/Lightning-backed jobs).
* **Wallet integration** (Lightning, Nostr zaps, etc.) inside the Desktop app.
* **Team multi-agent coordination** across machines.

---

If you're updating or extending MechaCoder:

> **Golden rule for v2:**
> **If a change affects how MechaCoder chooses tasks, edits code, runs tests, commits, or updates `.openagents/tasks.jsonl`, you must re-run the Desktop Golden Loop and ensure it still matches this spec.**
