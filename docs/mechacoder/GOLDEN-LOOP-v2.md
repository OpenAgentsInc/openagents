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

**HUD Event Mapping:**

The orchestrator emits events during each phase of the Golden Loop. These events are filtered and forwarded to the HUD UI for real-time progress display.

| Phase | Events Emitted | Forwarded to HUD |
|-------|----------------|------------------|
| Session Start | `session_start` | ✅ |
| Orient | `lock_acquired`, `init_script_*`, `orientation_complete` | ❌ (internal) |
| Select Task | `task_selected` | ✅ |
| Decompose | `task_decomposed` | ✅ |
| Execute | `subtask_start`, `subtask_complete`, `subtask_failed` | ✅ |
| Verify | `verification_start`, `verification_complete` | ✅ |
| Commit | `commit_created`, `push_complete` | ✅ |
| Update | `task_updated`, `progress_written` | ❌ (internal) |
| Session End | `session_complete` | ✅ |
| Errors | `error` | ✅ |

Internal events (lock management, init scripts, task updates, progress writes) are filtered out – they're bookkeeping that doesn't need UI display. The HUD receives only user-visible state changes.

**Implementation:** See `src/hud/emit.ts` for the mapping function and `src/hud/emit.test.ts` for sample event sequences used in testing.

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

### 2.2.1. Preflight Checklist (init.sh)

Before selecting a task, the orchestrator runs a **preflight checklist** to verify the environment is ready for work. This checklist is defined in `.openagents/init.sh` (optional but recommended).

**Purpose:** Fail fast if the environment is broken, rather than discovering issues mid-task and leaving partial work behind.

#### Recommended Preflight Checks

| Check | Why | Fail-Fast Behavior |
|-------|-----|-------------------|
| **Git clean** | Uncommitted changes may conflict with new work | Warning (continue) or Error (abort) based on policy |
| **Smoke test** | Codebase must compile/pass basic tests | Error (abort) – no point working on broken code |
| **API keys present** | Claude Code / LLM APIs need credentials | Error (abort) if Claude Code enabled and no keys |
| **Lock file absent** | Another agent may be running | Error (abort) – prevent concurrent modifications |
| **Disk space** | Builds may fail with insufficient space | Warning (continue) with logged alert |
| **Network reachable** | Push/fetch and API calls need connectivity | Warning (continue) if `offlineMode: "allow"`, else Error (abort) |

#### Reference init.sh Template

```bash
#!/bin/bash
# .openagents/init.sh - Preflight checklist for Golden Loop v2
#
# Exit codes:
#   0 = All checks passed
#   1 = Fatal error (abort session)
#   2 = Warnings only (continue with caution)
#
# Logged to: docs/logs/YYYYMMDD/HHMM-preflight.log

set -o pipefail

DAY=$(TZ=America/Chicago date +%Y%m%d)
TS=$(TZ=America/Chicago date +%H%M)
LOG_DIR="docs/logs/$DAY"
LOG_FILE="$LOG_DIR/${TS}-preflight.log"

mkdir -p "$LOG_DIR"

log() {
    echo "[$(date -Iseconds)] $1" | tee -a "$LOG_FILE"
}

warn() {
    log "WARNING: $1"
    WARNINGS=$((WARNINGS + 1))
}

fatal() {
    log "FATAL: $1"
    exit 1
}

WARNINGS=0

log "=== Golden Loop v2 Preflight Checklist ==="
log "Working directory: $(pwd)"
log "Project: $(jq -r '.projectId // "unknown"' .openagents/project.json 2>/dev/null || echo 'unknown')"

# 1. Check for stale lock file
log "Checking agent lock..."
if [ -f ".openagents/agent.lock" ]; then
    LOCK_PID=$(head -1 .openagents/agent.lock)
    if kill -0 "$LOCK_PID" 2>/dev/null; then
        fatal "Another agent is running (PID $LOCK_PID). Aborting."
    else
        log "Removing stale lock from dead process $LOCK_PID"
        rm -f .openagents/agent.lock
    fi
fi

# 2. Check git status (uncommitted changes)
log "Checking git status..."
if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    warn "Uncommitted changes detected:"
    git status --short >> "$LOG_FILE"
    # Policy: continue with warning (change to 'fatal' for strict mode)
fi

# 3. Smoke test: typecheck
log "Running smoke test (typecheck)..."
if ! bun run typecheck >> "$LOG_FILE" 2>&1; then
    fatal "Typecheck failed at preflight. Fix errors before running agent."
fi
log "Typecheck passed."

# 4. Smoke test: quick test run (optional, based on project config)
SMOKE_TEST=$(jq -r '.smokeTestCommand // empty' .openagents/project.json 2>/dev/null)
if [ -n "$SMOKE_TEST" ]; then
    log "Running smoke test: $SMOKE_TEST"
    if ! eval "$SMOKE_TEST" >> "$LOG_FILE" 2>&1; then
        fatal "Smoke test failed: $SMOKE_TEST"
    fi
    log "Smoke test passed."
else
    log "No smokeTestCommand configured, skipping test smoke."
fi

# 5. Check API keys (if Claude Code enabled)
CLAUDE_ENABLED=$(jq -r '.claudeCode.enabled // false' .openagents/project.json 2>/dev/null)
if [ "$CLAUDE_ENABLED" = "true" ]; then
    log "Checking API credentials for Claude Code..."
    if [ -z "$ANTHROPIC_API_KEY" ] && [ ! -f ~/.config/claude/credentials.json ]; then
        fatal "Claude Code enabled but no API credentials found. Set ANTHROPIC_API_KEY or configure ~/.config/claude/credentials.json"
    fi
    log "API credentials present."
fi

# 6. Check network connectivity (if not offline mode)
OFFLINE_MODE=$(jq -r '.offlineMode // "block"' .openagents/project.json 2>/dev/null)
log "Checking network connectivity..."
if ! curl -s --connect-timeout 5 https://api.github.com >/dev/null 2>&1; then
    if [ "$OFFLINE_MODE" = "allow" ]; then
        warn "Network unreachable. Continuing in offline mode."
    else
        fatal "Network unreachable and offlineMode is not 'allow'. Aborting."
    fi
else
    log "Network connectivity confirmed."
fi

# 7. Check disk space (warn if < 1GB free)
log "Checking disk space..."
FREE_KB=$(df -k . | tail -1 | awk '{print $4}')
if [ "$FREE_KB" -lt 1048576 ]; then
    warn "Low disk space: $(($FREE_KB / 1024))MB free"
fi

# 8. Sync with remote (optional, fetch only)
ALLOW_PUSH=$(jq -r '.allowPush // false' .openagents/project.json 2>/dev/null)
if [ "$ALLOW_PUSH" = "true" ]; then
    log "Fetching from remote..."
    if git fetch origin >> "$LOG_FILE" 2>&1; then
        BEHIND=$(git rev-list --count HEAD..origin/$(git rev-parse --abbrev-ref HEAD) 2>/dev/null || echo 0)
        if [ "$BEHIND" -gt 0 ]; then
            warn "Local branch is $BEHIND commits behind remote."
        fi
    else
        warn "git fetch failed (continuing with local state)"
    fi
fi

# Summary
log "=== Preflight Complete ==="
if [ "$WARNINGS" -gt 0 ]; then
    log "Completed with $WARNINGS warning(s). Review $LOG_FILE for details."
    exit 2
else
    log "All checks passed. Ready for Golden Loop."
    exit 0
fi
```

#### Fail-Fast Policy Options

Configure fail-fast behavior in `.openagents/project.json`:

```json
{
  "preflight": {
    "abortOnDirtyWorkTree": false,      // true = abort if uncommitted changes
    "abortOnTypecheckFail": true,       // true = abort if typecheck fails
    "abortOnNetworkUnavailable": true,  // false = allow offline work
    "smokeTestCommand": "bun test --bail",  // Optional quick test
    "timeoutSeconds": 120               // Max time for all preflight checks
  }
}
```

#### Logging Preflight Results

Preflight logs are written to `docs/logs/YYYYMMDD/HHMM-preflight.log` alongside other session logs. This provides:

1. **Audit trail** – see what state the repo was in when the agent started
2. **Debugging** – understand why a session was aborted
3. **Metrics** – track how often preflight catches issues vs. mid-task failures

The orchestrator reads the init.sh exit code:
- `0` = proceed with task selection
- `1` = abort session (fatal error)
- `2` = proceed with warnings logged

If init.sh is missing, the orchestrator proceeds without preflight (equivalent to exit 0).

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

* Write a per-run log under `docs/logs/YYYYMMDD/HHMM-overnight-{sessionId}.md` with:

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

### 2.10. Log Retention and Rotation

Run logs are stored in `docs/logs/` with the following structure and policies:

#### Directory Structure

```
docs/logs/
├── 20251201/
│   ├── 0935-overnight-orchestrator-1234567890.md
│   ├── 1420-overnight-orchestrator-1234567891.md
│   └── 2135-init.md
├── 20251202/
│   ├── 0813-overnight-orchestrator-1234567892.md
│   └── 0956-supervisor-handoff.md
└── 20251203/
    └── 1056-overnight-orchestrator-1234567893.md
```

#### Naming Convention

| Component | Format | Example |
|-----------|--------|---------|
| Date folder | `YYYYMMDD` | `20251203` |
| Time prefix | `HHMM` | `1056` |
| Type | `overnight` or descriptive | `overnight`, `preflight`, `init` |
| Session ID | `orchestrator-{timestamp}` | `orchestrator-1764780987762` |
| Extension | `.md` | `.md` |

Full path: `docs/logs/YYYYMMDD/HHMM-overnight-orchestrator-{timestamp}.md`

#### Log Content

Each run log contains:

1. **Header**: Session ID and start timestamp
2. **Configuration**: Work directory, max tasks, enabled features
3. **Task cycles**: Per-task entries with:
   - Task ID and title
   - Subtask execution logs
   - Verification results (typecheck, tests)
   - Commit SHA and message
   - Success/failure status
4. **Summary**: Tasks completed, final status

#### Retention Policy (Manual)

Log rotation is currently **manual**. Recommendations:

| Age | Action |
|-----|--------|
| < 7 days | Keep all logs |
| 7-30 days | Keep logs with failures or notable events |
| > 30 days | Archive or delete |

**Cleanup command:**
```bash
# Remove log folders older than 30 days
find docs/logs -type d -name "20*" -mtime +30 -exec rm -rf {} +
```

#### Size Guidelines

- Individual logs: Typically 5-50 KB depending on session length
- Daily folders: Up to 1 MB for active days
- Total `docs/logs/`: Monitor if exceeding 100 MB

**Note:** Logs are committed to git. Large log accumulation will increase repository size. Consider:
1. Adding `docs/logs/` to `.gitignore` for repositories with frequent overnight runs
2. Using a separate logging system for production deployments
3. Periodically pruning old log folders

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

### 4.3. Failed Subtask Cleanup Guardrails

When a subtask fails (tests/typecheck don't pass), the orchestrator implements safety guardrails to prevent broken code from being committed.

#### Revert on Failure

Immediately after a subtask fails, the orchestrator reverts all uncommitted changes:

```bash
# Revert tracked file modifications
git checkout -- .

# Remove untracked files and directories (preserves .gitignore'd files)
git clean -fd
```

**Why this is important:**
- Failed subtasks may leave broken code in the working tree
- Without this guardrail, the final cleanup commit could accidentally commit broken code
- This ensures the repository always remains in a buildable state

**What is preserved:**
- All committed work (even WIP commits)
- Files in `.gitignore` (node_modules, .env, etc.)
- Progress files that were already committed

**What is discarded:**
- Uncommitted modifications to tracked files
- New untracked files created during the failed subtask

#### Selective Add in Cleanup Commit

At the end of each orchestrator session, instead of `git add -A` (which would stage everything), the cleanup commit only stages specific paths:

```bash
# Only add progress/log files - NOT broken code
git add .openagents/progress.md .openagents/subtasks/ docs/logs/ 2>/dev/null || true
```

**Rationale:**
- Progress files should always be committed (they track session state)
- Logs should always be committed (they provide audit trail)
- Code changes should only be committed via per-task commits after verification passes

**Files staged by cleanup commit:**
| Path | Purpose |
|------|---------|
| `.openagents/progress.md` | Session summary for next run |
| `.openagents/subtasks/*.json` | Subtask decomposition and status |
| `docs/logs/**/*.md` | Session logs and preflight results |

**Files NOT staged by cleanup commit:**
- Source code (`src/**`)
- Test files (`*.test.ts`)
- Configuration files (`package.json`, `tsconfig.json`)
- Any file not in the explicitly listed paths

This two-layer guardrail (revert + selective add) ensures that even if the revert fails for some reason, the cleanup commit won't include source code changes.

### 4.4. Claude Code Subagent Failures

| Scenario | Behavior |
|----------|----------|
| Claude Code unavailable | Falls back to minimal subagent. |
| Claude Code detection fails | Falls back to minimal subagent. |
| Claude Code times out (50min default) | Retried up to 3x with exponential backoff. |
| Rate limit / auth errors | Falls back to minimal subagent (if enabled). |

**Recovery**: Session resume via `resumeSessionId` and `forkSession` options. Previous session ID tracked in progress file.

### 4.5. Progress File Issues

| Scenario | Behavior |
|----------|----------|
| Missing progress file | Fresh session created. No previous context. |
| Malformed/corrupted progress file | Parser returns partial results with safe defaults. |
| Truncated file | Parses what's available, missing fields get defaults. |

**Recovery**: Progress parsing is fault-tolerant. Missing sections don't crash the orchestrator.

### 4.6. Git Operation Failures

| Scenario | Behavior |
|----------|----------|
| Push fails (conflicts) | Changes kept in working tree. Task stays `in_progress`. |
| Push fails (remote changes) | Option A: New task for conflict resolution. Option B: Rebase if policy allows. |
| Force push required | **Never** force-pushed unless `allowForcePush` explicitly set. |

**Recovery**: Conflict tasks created for human review. Changes preserved locally. See **Section 5.5: Playbook: Git Conflict & Push Failure Handling** for detailed recovery procedures.

### 4.7. Task System Failures

| Scenario | Behavior |
|----------|----------|
| No ready tasks | Loop exits cleanly. |
| Task blocked by dependencies | Skipped. Next ready task selected. |
| Concurrent modification of tasks.jsonl | Prevented by agent lock (see Section 4.7.1). |

**Recovery**: Lock file (`.openagents/agent.lock`) prevents overlapping runs.

#### 4.7.1. Agent Lock Enforcement

The orchestrator uses `.openagents/agent.lock` to prevent concurrent runs on the same repository.

**Lock File Format:**
```
<PID>
<ISO-8601-timestamp>
<optional-session-id>
```

Example:
```
12345
2025-12-03T10:30:00.000Z
orchestrator-1733227800123
```

**Lock Lifecycle:**

1. **Acquisition** (at session start):
   - If no lock exists → create lock with current PID
   - If lock exists and PID is running → abort with error
   - If lock exists but PID is dead (stale) → remove stale lock, create new one

2. **Release** (at session end):
   - Only the owning process can release the lock
   - Lock is released on normal exit
   - Lock remains if process crashes (becomes stale)

**Programmatic API** (`src/agent/orchestrator/agent-lock.ts`):

```typescript
import { acquireLock, releaseLock, checkLock, forceRemoveLock } from "./orchestrator/agent-lock.js";

// Acquire lock (returns detailed result)
const result = acquireLock(openagentsDir, sessionId);
if (!result.acquired && result.reason === "already_running") {
  console.error(`Agent already running: PID ${result.existingLock.pid}`);
  process.exit(1);
}

// Check lock status without modifying
const status = checkLock(openagentsDir);
if (status.locked && status.isStale) {
  console.log("Stale lock detected");
}

// Release lock (only if we own it)
releaseLock(openagentsDir);

// Force remove (for manual recovery)
forceRemoveLock(openagentsDir);
```

**Manual Recovery:**

```bash
# Check if lock exists and who owns it
cat .openagents/agent.lock

# Check if the PID is actually running
kill -0 <PID> 2>/dev/null && echo "Running" || echo "Not running"

# Force remove stale lock
rm -f .openagents/agent.lock
```

**Test Harness:**

Tests for lock behavior are in `src/agent/orchestrator/agent-lock.test.ts`:
- Lock acquisition when none exists
- Blocking when lock held by running process
- Stale lock detection and cleanup
- Lock release semantics
- Guard pattern for automatic cleanup

### 4.8. Init Script Failures

The init.sh preflight checklist (see Section 2.2.1) uses exit codes to signal severity:

| Exit Code | Meaning | Orchestrator Behavior |
|-----------|---------|----------------------|
| `0` | All checks passed | Proceed with task selection |
| `1` | Fatal error | **Abort session** – do not proceed |
| `2` | Warnings only | Proceed with caution, warnings logged |

| Scenario | Behavior |
|----------|----------|
| Missing init.sh | Continues without preflight (non-fatal). |
| Init script exits with `1` | **Session aborted.** Fatal error logged to `docs/logs/YYYYMMDD/HHMM-preflight.log`. |
| Init script exits with `2` | Warnings logged. Session continues. |
| Init script times out (default 120s) | Treated as exit `1` (fatal). Session aborted. |
| Init script throws unexpected error | Logged. Session aborted (fail-safe default). |

**Recovery**:
- For exit `1` (fatal): Fix the underlying issue (e.g., failing typecheck, missing API keys) and rerun.
- For exit `2` (warning): Review `docs/logs/YYYYMMDD/HHMM-preflight.log` to understand warnings.
- For missing init.sh: Consider adding the reference template from Section 2.2.1.

### 4.9. Network & External Service Failures

| Scenario | Behavior |
|----------|----------|
| Network unavailable at startup | Orchestrator detects via ping/fetch. Logs warning. Falls back to local-only mode if `offlineMode: "allow"` in project.json. |
| Network lost mid-session | Current operation times out. Uncommitted work preserved. Task marked `blocked` with reason. |
| Claude Code API unreachable | Falls back to minimal subagent (if `fallbackToMinimal: true`). |
| Claude Code rate-limited | Exponential backoff (3 retries). Then fallback to minimal subagent. |
| OpenRouter / LLM provider unavailable | Same as Claude Code - fallback to minimal or block task. |
| Git push fails (network) | Work committed locally. Task stays `in_progress`. Push retried on next session. |

**Recovery**: See **Section 5.7: Playbook: Network & Offline Recovery** for detailed procedures.

**Offline-Capable Operations:**
- Reading/writing local files
- Running local tests (`bun test`, `bun run typecheck`)
- Committing to local git (no push)
- Updating `.openagents/tasks.jsonl`
- Writing progress files and logs

**Operations Requiring Network:**
- Git push/fetch
- Claude Code subagent invocation
- LLM API calls (OpenRouter, Anthropic)
- Any external API integrations

**Fallback Hierarchy:**
1. **Claude Code** (preferred) - Full agentic coding with tools
2. **Minimal Subagent** (fallback) - Basic read/edit/bash, lower capability
3. **Block Task** (last resort) - Mark task blocked, preserve state for resumption

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

**Automated Detection (via init.sh):**

Use the comprehensive preflight checklist from **Section 2.2.1** which includes:
- Git dirty state detection
- Stale lock file cleanup
- Typecheck verification
- API key validation
- Network connectivity check
- Disk space monitoring

The preflight script logs all findings to `docs/logs/YYYYMMDD/HHMM-preflight.log` for debugging.

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

### 5.5. Playbook: Git Conflict & Push Failure Handling

**Goal:** Handle merge conflicts, rejected pushes, and diverged branches without losing work or corrupting state.

#### Understanding Git Failure Modes

| Failure Type | Cause | Symptoms |
|--------------|-------|----------|
| **Rejected push** | Remote has commits not in local | `! [rejected] main -> main (fetch first)` |
| **Merge conflict** | Concurrent changes to same lines | `CONFLICT (content): Merge conflict in <file>` |
| **Rebase conflict** | Local commits conflict with upstream | `Could not apply <sha>... <message>` |
| **Diverged branches** | Local and remote both advanced | `Your branch and 'origin/main' have diverged` |

#### Core Principle: Never Lose Work

**CRITICAL:** The orchestrator must **never discard uncommitted changes** or **lose committed work** during conflict resolution. The working tree represents potentially hours of agent work.

**Safe Operations:**
- `git fetch origin` - Always safe
- `git status` - Always safe
- `git stash push` - Safe, preserves changes
- `git rebase --abort` - Safe, returns to pre-rebase state
- `git merge --abort` - Safe, returns to pre-merge state

**Dangerous Operations (require explicit policy):**
- `git reset --hard` - Loses uncommitted changes
- `git checkout -- .` - Loses uncommitted changes
- `git clean -fd` - Removes untracked files
- `git push --force` - Overwrites remote history

#### Recovery Flow: Rejected Push

When `git push` fails with "rejected (fetch first)":

```
Push Rejected
    │
    ├── 1. Preserve current state
    │   └── Commit any uncommitted changes to a WIP commit
    │
    ├── 2. Fetch remote changes
    │   └── git fetch origin
    │
    ├── 3. Assess divergence
    │   ├── git log --oneline HEAD..origin/main  # What's new on remote
    │   └── git log --oneline origin/main..HEAD  # What's local-only
    │
    ├── 4. Choose resolution strategy
    │   │
    │   ├── Option A: Rebase (preferred for small changes)
    │   │   └── git rebase origin/main
    │   │       ├── Success → git push
    │   │       └── Conflict → See "Handling Rebase Conflicts"
    │   │
    │   ├── Option B: Merge (preserves commit history)
    │   │   └── git merge origin/main
    │   │       ├── Success → git push
    │   │       └── Conflict → See "Handling Merge Conflicts"
    │   │
    │   └── Option C: Create follow-up task (safe backoff)
    │       └── See "Safe Backoff with Follow-up Task"
    │
    └── 5. Update task status appropriately
```

#### Handling Rebase Conflicts

```bash
# 1. Start rebase
git rebase origin/main

# 2. If conflicts occur, for each conflicted file:
#    - Open the file and find conflict markers (<<<<<<, =======, >>>>>>)
#    - Resolve by choosing correct version or merging both
#    - Stage the resolved file
git add <resolved-file>

# 3. Continue rebase
git rebase --continue

# 4. If unable to resolve (complex conflicts):
git rebase --abort  # Return to pre-rebase state
# Then use "Safe Backoff" strategy below
```

#### Handling Merge Conflicts

```bash
# 1. Start merge
git merge origin/main

# 2. If conflicts occur:
#    - Resolve conflicts in each file
#    - Stage resolved files
git add <resolved-file>

# 3. Complete merge
git commit  # Merge commit message auto-generated

# 4. If unable to resolve:
git merge --abort  # Return to pre-merge state
# Then use "Safe Backoff" strategy below
```

#### Safe Backoff with Follow-up Task

When conflicts are too complex for automatic resolution, the orchestrator should:

1. **Preserve all local work:**
   ```bash
   # Ensure all changes are committed locally
   git add -A
   git commit -m "WIP: $(cat <<'EOF'
   oa-<taskId>: work in progress (push blocked by conflicts)

   Remote has diverged. This commit preserves local work.
   Requires manual conflict resolution before pushing.

   🤖 Generated with [OpenAgents](https://openagents.com)

   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   ```

2. **Create a follow-up task for conflict resolution:**
   ```bash
   bun run tasks:create \
     --title "Resolve git conflicts for oa-<taskId>" \
     --type task \
     --priority 1 \
     --labels "git,conflicts,manual" \
     --description "$(cat <<'EOF'
   The push for oa-<taskId> was rejected due to remote changes.

   Local commits to resolve:
   - <list commit SHAs>

   Remote commits causing conflict:
   - <list remote commit SHAs>

   Steps:
   1. git fetch origin
   2. git rebase origin/main (or merge)
   3. Resolve conflicts
   4. git push

   Original task work is preserved in local commits.
   EOF
   )" \
     --json
   ```

3. **Update original task status:**
   ```bash
   # Mark as blocked, not closed
   bun run tasks:update \
     --id oa-<taskId> \
     --status blocked \
     --reason "Push rejected - conflicts with remote. See follow-up task for resolution."
   ```

4. **Log the situation in progress.md:**
   ```markdown
   ### Git Conflict Encountered

   - Task: oa-<taskId>
   - Local commits: <sha1>, <sha2>
   - Conflict cause: Remote advanced with <N> new commits
   - Resolution: Follow-up task created (oa-<newTaskId>)
   - Local work: Preserved in commits on local branch
   ```

#### Orchestrator Decision Matrix

| Situation | Automatic Resolution | Manual Task |
|-----------|---------------------|-------------|
| 1-2 files conflicted, simple changes | ✅ Try rebase | Fallback |
| Many files conflicted | ❌ | ✅ Create task |
| Conflicts in generated/lock files | ✅ Regenerate | Fallback |
| Conflicts in core logic | ❌ | ✅ Create task |
| Same file modified by both sides | ❌ | ✅ Create task |
| Only additions on both sides | ✅ Try merge | Fallback |

#### Preventing Conflicts

To minimize conflicts during overnight runs:

1. **Work on isolated branches:**
   ```json
   // .openagents/project.json
   {
     "workBranch": "mechacoder/work",
     "defaultBranch": "main"
   }
   ```

2. **Fetch before starting work:**
   ```bash
   # In .openagents/init.sh
   git fetch origin
   LOCAL=$(git rev-parse HEAD)
   REMOTE=$(git rev-parse origin/main)
   if [ "$LOCAL" != "$REMOTE" ]; then
       echo "WARNING: Local is behind remote. Pulling..."
       git pull --rebase origin main
   fi
   ```

3. **Keep tasks small and focused:**
   - Smaller changes = fewer conflict opportunities
   - Complete and push quickly

#### Working Tree Preservation Rules

During any git operation that might fail:

1. **Never run `git reset --hard` or `git checkout -- .`** unless explicitly recovering from a known-bad state
2. **Always check for uncommitted changes** before any destructive operation
3. **Use `git stash` as insurance** before complex operations:
   ```bash
   git stash push -m "Pre-operation backup $(date +%Y%m%d-%H%M%S)"
   # ... do risky operation ...
   # If successful, can drop stash
   # If failed, can restore: git stash pop
   ```
4. **Keep WIP commits on the branch** rather than discarding partial work
5. **Log the state** in `.openagents/progress.md` before and after

---

### 5.6. Quick Reference: Recovery Decision Tree

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

Git push rejected / conflicts?
    │
    ├── Check: Are there uncommitted changes?
    │   └── Yes → Commit them first (WIP commit is fine)
    │
    ├── Fetch and assess: git fetch origin
    │   └── git log --oneline HEAD..origin/main
    │
    ├── Simple divergence (few commits, no overlap)?
    │   └── git rebase origin/main && git push
    │
    ├── Conflicts during rebase/merge?
    │   ├── Can resolve? → Fix conflicts → git rebase --continue → push
    │   └── Too complex? → git rebase --abort → Safe Backoff
    │
    └── Safe Backoff:
        ├── Commit local work (WIP)
        ├── Create follow-up task for conflict resolution
        ├── Mark original task as blocked
        └── Log in progress.md

Can't resume Claude Code session?
    │
    ├── Session expired or unavailable
    │   └── Set claudeCode.resumeStrategy = "fork" in subtask JSON
    │
    └── Want completely fresh start
        └── Remove claudeCode.sessionId from subtask JSON

Network / external service unavailable?
    │
    ├── Check: Can reach external services?
    │   └── ping 8.8.8.8 || curl -s https://api.anthropic.com/health
    │
    ├── Network down at startup
    │   ├── offlineMode: "allow" → Continue with local-only work
    │   └── offlineMode: "block" → Exit, wait for network
    │
    ├── Claude Code unavailable
    │   ├── fallbackToMinimal: true → Use minimal subagent
    │   └── fallbackToMinimal: false → Block task
    │
    └── Network lost mid-session
        ├── Commit work locally (no push)
        ├── Mark task as blocked with reason
        └── Retry push on next session
```

---

### 5.7. Playbook: Network & Offline Recovery

**Goal:** Handle network outages and external service unavailability without losing work.

#### Detecting Network State

```bash
# Quick connectivity check
check_network() {
    # Try multiple endpoints
    if curl -s --connect-timeout 5 https://api.anthropic.com/health >/dev/null 2>&1; then
        echo "claude-api: online"
    else
        echo "claude-api: offline"
    fi

    if curl -s --connect-timeout 5 https://api.github.com >/dev/null 2>&1; then
        echo "github: online"
    else
        echo "github: offline"
    fi

    if git ls-remote origin HEAD >/dev/null 2>&1; then
        echo "git-remote: online"
    else
        echo "git-remote: offline"
    fi
}
```

#### Scenario A: Network Unavailable at Startup

**Symptoms:**
- `git fetch` fails with connection error
- Claude Code invocation times out
- Orchestrator logs network-related errors

**Recovery Options:**

**Option 1: Wait for network (recommended for short outages)**
```bash
# Check network status
ping -c 1 8.8.8.8 || echo "Network unreachable"

# Wait and retry
while ! ping -c 1 8.8.8.8 >/dev/null 2>&1; do
    echo "Waiting for network..."
    sleep 30
done

# Resume orchestrator
bun run src/cli/mechacoder.ts --dir . --once
```

**Option 2: Enable offline mode (for extended outages)**
```bash
# Edit .openagents/project.json to allow offline work
# Add: "offlineMode": "allow"

# In offline mode, orchestrator will:
# - Skip git fetch/push
# - Fall back to minimal subagent (no Claude Code API)
# - Commit changes locally only
# - Mark tasks for "push pending" on network return
```

**Option 3: Work on local-only tasks**
```bash
# Filter to tasks that don't require network
# (e.g., documentation, refactoring, tests that run locally)

# Mark network-dependent tasks as blocked
bun run tasks:update --id oa-<taskId> --status blocked --reason "Requires network - offline"
```

#### Scenario B: Claude Code Unavailable (API Down/Rate Limited)

**Symptoms:**
- Claude Code invocation returns 503/429
- Timeout waiting for Claude Code response
- Authentication errors

**Recovery Flow:**

```
Claude Code Fails
    │
    ├── Is fallbackToMinimal enabled?
    │   │
    │   ├── Yes
    │   │   └── Switch to minimal subagent
    │   │       ├── Minimal has: read, write, edit, bash
    │   │       ├── Minimal lacks: full reasoning, multi-step planning
    │   │       └── Best for: simple, well-specified subtasks
    │   │
    │   └── No
    │       └── Block task with reason
    │           ├── Set status: "blocked"
    │           ├── Set reason: "Claude Code unavailable"
    │           └── Preserve progress for later resumption
    │
    └── Update progress.md with service status
```

**Manual fallback configuration:**
```bash
# Edit .openagents/project.json
{
  "fallbackToMinimal": true,  # Enable minimal subagent fallback
  "minimalSubagentModel": "gpt-4o-mini",  # Alternative model for minimal
  "claudeCodeRetries": 3,  # Retries before fallback
  "claudeCodeTimeout": 300000  # 5 min timeout
}
```

**Forcing minimal subagent:**
```bash
# To force a subtask to use minimal subagent (bypass Claude Code)
# Edit .openagents/subtasks/<taskId>.json:
{
  "subtasks": [
    {
      "id": "sub-001",
      "forceMinimal": true,  # Skip Claude Code entirely
      ...
    }
  ]
}
```

#### Scenario C: Network Lost Mid-Session

**Symptoms:**
- Git push fails after successful commit
- Claude Code session disconnects
- Timeouts during file operations

**Immediate Response:**

1. **Preserve all work locally:**
   ```bash
   # Ensure changes are committed locally
   git status
   # If uncommitted changes exist:
   git add -A
   git commit -m "WIP: $(cat <<'EOF'
   oa-<taskId>: work in progress (network lost)

   Session interrupted due to network failure.
   Push pending when connectivity restored.

   🤖 Generated with [OpenAgents](https://openagents.com)

   Co-Authored-By: MechaCoder <noreply@openagents.com>
   EOF
   )"
   ```

2. **Update task status:**
   ```bash
   # Mark task appropriately - NOT closed (work not pushed)
   bun run tasks:update --id oa-<taskId> --status in_progress --reason "Work complete locally; push pending (network lost)"
   ```

3. **Record state in progress.md:**
   ```markdown
   ### Network Failure - Session Interrupted

   - Time: <timestamp>
   - Task: oa-<taskId>
   - Local commits: <sha1>, <sha2>
   - Push status: PENDING (network unavailable)
   - Subtask status: work complete, verification pending push

   On network restore:
   1. git push origin HEAD
   2. Verify CI passes
   3. Close task
   ```

4. **Set up resumption:**
   ```bash
   # In subtask JSON, mark for push-on-resume:
   {
     "subtasks": [
       {
         "id": "sub-001",
         "status": "pending_push",  # Custom status for this scenario
         "localCommits": ["abc123", "def456"],
         "pushAttempts": 1
       }
     ]
   }
   ```

#### Scenario D: Resuming After Network Restored

**Steps:**

1. **Verify connectivity:**
   ```bash
   git ls-remote origin HEAD && echo "Git remote accessible"
   curl -s https://api.anthropic.com/health && echo "Claude API accessible"
   ```

2. **Push pending commits:**
   ```bash
   # Check for unpushed commits
   git log --oneline origin/main..HEAD

   # Push if any
   git push origin HEAD
   ```

3. **Resume Claude Code session (if applicable):**
   ```bash
   # If sessionId exists in subtask JSON, orchestrator will attempt resume
   # To force fresh session instead:
   # Edit subtask JSON: set claudeCode.resumeStrategy = "fork"
   ```

4. **Clear blocked status:**
   ```bash
   # If task was blocked due to network
   bun run tasks:update --id oa-<taskId> --status in_progress --reason ""
   ```

5. **Restart orchestrator:**
   ```bash
   bun run src/cli/mechacoder.ts --dir . --once
   ```

#### Session Persistence for Network Resilience

To maximize recoverability during network issues, the orchestrator persists:

| Artifact | Location | Purpose |
|----------|----------|---------|
| Claude Code session ID | `.openagents/subtasks/<taskId>.json` | Resume interrupted sessions |
| Local commits | Git reflog | Recover uncommitted work |
| Progress summary | `.openagents/progress.md` | Orient next session |
| Subtask state | `.openagents/subtasks/<taskId>.json` | Track completion status |
| Push pending flag | subtask `status: "pending_push"` | Retry push on network restore |

**Session Resume Options:**

```json
// In subtask claudeCode config:
{
  "claudeCode": {
    "sessionId": "session-abc123",  // Previous session to resume
    "resumeStrategy": "continue",   // "continue" | "fork" | "fresh"
    "lastActivityAt": "2025-12-03T10:00:00Z",
    "sessionExpiry": "2025-12-03T22:00:00Z"  // Sessions expire after ~12h
  }
}
```

**Resume strategies:**
- `continue`: Try to resume exact session state (may fail if expired)
- `fork`: Create new session with context from previous (recommended after failures)
- `fresh`: Start completely new session (loses all session context)

#### Logging Service Unavailability

When external services fail, log clearly for debugging:

```markdown
<!-- In progress.md -->
### Service Unavailability Log

| Time | Service | Error | Action Taken |
|------|---------|-------|--------------|
| 10:30 | Claude Code | 503 Service Unavailable | Fallback to minimal subagent |
| 10:45 | GitHub API | Connection timeout | Deferred push, task stays in_progress |
| 11:00 | Claude Code | 429 Rate Limited | Backoff 60s, retry succeeded |
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
