# MechaCoder Operations Guide (OpenAgents Desktop)

A guide for humans and future agents on **running, supervising, and debugging MechaCoder** in the `openagents` repo.

MechaCoder is an autonomous coding agent that picks up tasks, implements code, runs tests, and commits — learning patterns and conventions over time. This guide describes how to operate the **current local incarnation** (Bun + Effect, launchd + CLI). The host (launchd vs desktop UI) may change, but these fundamentals stay useful.

---

## 1. Where Logs Are

### 1.1. Per-run agent logs (most important)

Per-run logs live in:

```bash
~/code/openagents/docs/logs/YYYYMMDD/*.md
````

Common patterns:

* `HHMMSS-bead-run.md` – single MechaCoder bead/loop run
* `*-golden-loop-e2e-log.md` – Golden Loop e2e test runs
* `*-testing-infra-log.md`, `*-bead-audit-log.md` – audits, infra runs

Example:

```bash
docs/logs/20251202/093350-bead-run.md
docs/logs/20251202/1516-testing-infra-log.md
docs/logs/20251202/2001-golden-loop-live-log.md
```

Each run log typically contains:

* Start time
* Working directory (`--dir` repo path)
* Bead/task ID (if applicable)
* All tool calls and results
* Tests run and their output
* Final status message

**View latest log for today:**

```bash
cd ~/code/openagents
cat $(ls -t docs/logs/$(date +%Y%m%d)/*.md | head -1)
```

**Watch latest log in real time:**

```bash
cd ~/code/openagents
tail -f $(ls -t docs/logs/$(date +%Y%m%d)/*.md | head -1)
```

> Tip: use this while MechaCoder is running to see what it’s actually doing.

---

### 1.2. System logs (stdout/stderr)

MechaCoder’s raw stdout/stderr logs (for the `openagents` repo) live in:

```bash
~/code/openagents/logs/mechacoder-stdout.log
~/code/openagents/logs/mechacoder-stderr.log
```

These files are **gitignored** and can grow over time.

To inspect recent errors:

```bash
cd ~/code/openagents
tail -50 logs/mechacoder-stderr.log
```

To watch stdout live:

```bash
cd ~/code/openagents
tail -f logs/mechacoder-stdout.log
```

---

## 2. How to Run / Control MechaCoder

Right now there are two ways to run MechaCoder:

1. As a **launchd job** (cron-like, runs every N minutes against a target repo).
2. As a **manual CLI run** (once, against any repo with `.openagents/`).

### 2.1. Launchd job (current deployment)

The launchd plist lives at:

```bash
# Active plist (used by launchd):
~/Library/LaunchAgents/com.openagents.mechacoder.plist

# Source (edit this one and re-load):
~/code/openagents/scripts/com.openagents.mechacoder.plist
```

Key fields in the plist:

* `StartInterval`: e.g. `300` (run every 5 minutes)
* `WorkingDirectory`: repo MechaCoder will act in (e.g. `/Users/christopherdavid/code/nostr-effect` or another repo)
* `ProgramArguments`: usually calls `bun` with `src/agent/do-one-bead.ts --dir <repo>`
* `PATH`: must include `$HOME/.bun/bin` and `$HOME/.local/bin` (for `bd`, if still used)

#### Check if launchd agent is running

```bash
launchctl list | grep mechacoder
# Shows PID if running, "-" if not present
```

#### Stop the launchd agent

```bash
launchctl unload ~/Library/LaunchAgents/com.openagents.mechacoder.plist
```

#### Start (or restart) the launchd agent

```bash
cd ~/code/openagents
./scripts/start-mechacoder.sh   # copies plist if needed + load
```

This script should:

* Ensure plist is in `~/Library/LaunchAgents/`
* Load it via `launchctl load`

---

### 2.2. Manual CLI run (one loop, any repo)

You can run MechaCoder **once** against any repo that has `.openagents/` configured.

From the `openagents` repo:

```bash
cd ~/code/openagents

# Run against the openagents repo itself:
bun src/agent/do-one-bead.ts --dir .

# Run against nostr-effect repo:
bun src/agent/do-one-bead.ts --dir ~/code/nostr-effect
```

What this does:

* Reads `.openagents/project.json` under `--dir`.
* Uses `.openagents/tasks.jsonl` as the task source (with beads as legacy input if configured).
* Picks one ready task, edits code, runs tests, commits/pushes (if allowed), updates the task, and logs the run.

> Use this when debugging MechaCoder behavior on a single task, or during integration work.

---

## 3. Project & Task Configuration

MechaCoder now uses an **OpenAgents-native project/task system** under `.openagents/`, with Beads (`bd`) as optional interop.

### 3.1. Project metadata

Each repo MechaCoder can work on should have:

```bash
repo/
└── .openagents/
    ├── project.json
    ├── tasks.jsonl
    ├── models.json        # optional
    └── agents.json        # optional
```

Example `project.json` (simplified):

```jsonc
{
  "version": 1,
  "projectId": "openagents",
  "defaultBranch": "main",
  "defaultModel": "x-ai/grok-4.1-fast",
  "rootDir": ".",
  "testCommands": ["bun test"],
  "e2eCommands": [],              // e.g. ["E2E_STUB=1 pnpm run test:golden-loop-e2e:local-stub"]
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

The **TaskService** and **ProjectService** implementations in `src/tasks/` are the source of truth for these formats.

### 3.2. Tasks (`tasks.jsonl`)

Tasks are stored one per line in `.openagents/tasks.jsonl`:

```jsonc
{
  "id": "oa-1a2b3c",
  "title": "Add Golden Loop desktop harness",
  "description": "Implement Bun/Effect harness for Golden Loop e2e.",
  "type": "task",                 // bug | feature | task | epic | chore
  "priority": 1,                  // 0..4
  "status": "open",               // open | in_progress | blocked | closed
  "labels": ["testing", "golden-loop"],
  "createdAt": "2025-12-02T15:46:00Z",
  "updatedAt": "2025-12-02T16:00:00Z",
  "closedAt": null,
  "assignee": "mechacoder",
  "deps": [
    {
      "id": "oa-abc123",
      "type": "blocks"            // blocks | related | parent-child | discovered-from
    }
  ],
  "commits": [
    "ed3b5a9..."
  ],
  "source": {
    "repo": "openagents",
    "discoveredFrom": "oa-abc123",
    "externalRef": null
  }
}
```

**Humans** can inspect tasks via:

```bash
cd /path/to/repo
cat .openagents/tasks.jsonl | jq '.'
```

> Most of the time, though, you’ll interact via MechaCoder, not by editing this file by hand.

### 3.3. Legacy Beads (`bd`)

Some repos (e.g. `nostr-effect`) still use Beads (`bd`) as a **planning layer**. In that case:

* `.beads/issues.jsonl` is still present and used.
* The `.openagents` system may mirror/import beads (see `src/tasks/beads.ts`).

For **nostr-effect** specifically, you can still:

```bash
cd ~/code/nostr-effect
$HOME/.local/bin/bd ready --json
$HOME/.local/bin/bd list --json | jq '.[] | "\(.id) | \(.status) | \(.title)"'
$HOME/.local/bin/bd update <id> --status open
$HOME/.local/bin/bd close <id> --reason "Manual close"
```

Over time, `.openagents/tasks.jsonl` is intended to become the canonical task store across projects.

---

## 4. Known Operational Issues & Recovery

### 4.1. Uncommitted or broken changes

Historically, the agent could leave uncommitted/broken changes if:

* It ran out of turns,
* Tests failed and it didn’t recover, or
* Pre-push hooks failed.

**If you suspect this:**

```bash
cd /path/to/affected/repo   # e.g. ~/code/nostr-effect or ~/code/openagents

# Check for uncommitted changes:
git status

# If work looks good:
bun test                      # or configured tests
git add -A
git commit -m "Manual fix after MechaCoder run"
git push origin main

# If work is broken and you want to discard:
git checkout -- .
git clean -fd

# For bead-based repos, reset stuck beads:
$HOME/.local/bin/bd update <id> --status open
```

### 4.2. Agent not running (launchd)

```bash
# Check launchd status
launchctl list | grep mechacoder

# If not present or crashed:
cd ~/code/openagents
./scripts/start-mechacoder.sh
```

If it still fails:

```bash
cd ~/code/openagents
tail -50 logs/mechacoder-stderr.log
```

Look for:

* Missing env (OPENROUTER_API_KEY),
* Type errors,
* File path issues.

### 4.3. Agent running but making no progress

1. Check the **latest run log** in `docs/logs/YYYYMMDD/*.md`.
2. Check for repeated failures on the same task (tests failing, type errors).
3. For bead-based repos:

   * Look for tasks stuck in `in_progress`.
   * Reset them: `bd update <id> --status open`.
4. For `.openagents`-based repos:

   * Look in `.openagents/tasks.jsonl` for tasks stuck in `in_progress` too long.
   * Either update them to `blocked` with a reason, or `open` to retry.

### 4.4. API / model issues

* Check `OPENROUTER_API_KEY` is configured for the environment where the agent runs (launchd vs CLI).

* Model is currently pinned to:

  ```text
  x-ai/grok-4.1-fast
  ```

* The LLM client in `src/llm/openrouter.ts` uses **raw fetch** (not SDK) to avoid response validation issues.

If you see repeated model/API failures in logs, you may need to:

* Rotate or reconfigure API keys,
* Add retry/backoff logic, or
* Temporarily disable runs until stable.

---

## 5. Model Configuration (Current Constraints)

**Current rule:** MechaCoder uses `x-ai/grok-4.1-fast` via OpenRouter.

* Do **not** change the default model ID unless you also:

  * Update prompts/system messages,
  * Re-run tests for the agent loop,
  * Intentionally capture any new quirks (toolcall formats, etc.).

If you need different models per project in the future, use `.openagents/models.json` to express that, but keep the **default** aligned with the spec and prompts in `src/agent/prompts.ts`.

---

## 6. Log Format Example

A typical per-run log (`HHMMSS-bead-run.md`) looks like:

```markdown
# Bead Run Log

Started: 2025-12-02T15:04:07.329Z

[timestamp] DO ONE BEAD - Starting
[timestamp] Work directory: /Users/christopherdavid/code/openagents
[timestamp] Changed to: /Users/christopherdavid/code/openagents

## Agent Turns

### Tool Call: bash
{"command":"bun test"}

### Tool Result: bash ✅ SUCCESS
[... output ...]

### Tool Call: read
{"path":"src/tasks/service.ts","offset":0,"limit":4000}

### Tool Result: read ✅ SUCCESS
[... snippet ...]

### Assistant
I'll update TaskService to handle labelsAny filters and then add tests...

[... more turns ...]

## Final Message
BEAD_COMPLETED: openagents-5bb.2

[timestamp] SUCCESS - Bead completed!
```

---

## 7. For Future Agents

If you're an AI agent reading this:

1. Do **not** modify this file unless explicitly asked.
2. Always check **logs** before assuming current agent state.
3. Use `.openagents/tasks.jsonl` as the **primary task source** in this repo; treat `.beads` as legacy/interop.
4. For bead-based repos (nostr-effect, etc.), still use `bd` commands instead of markdown TODOs.
5. Always run `bun test` (and any configured e2e commands) before committing.
6. Never silently change the default model; update specs and prompts when you do.
7. Prefer **small, focused runs** that:

   * pick one task,
   * implement,
   * test,
   * commit/push,
   * close,
   * log.

---

## 8. Quick Reference

| Task                        | Command                                                                              |
| --------------------------- | ------------------------------------------------------------------------------------ |
| Check launchd running       | `launchctl list \| grep mechacoder`                                                  |
| Stop launchd agent          | `launchctl unload ~/Library/LaunchAgents/com.openagents.mechacoder.plist`            |
| Start launchd agent         | `cd ~/code/openagents && ./scripts/start-mechacoder.sh`                              |
| Run MechaCoder once (repo)  | `cd ~/code/openagents && bun src/agent/do-one-bead.ts --dir /path/to/repo`           |
| View latest log             | `cd ~/code/openagents && cat $(ls -t docs/logs/$(date +%Y%m%d)/*.md \| head -1)`     |
| Watch latest log            | `cd ~/code/openagents && tail -f $(ls -t docs/logs/$(date +%Y%m%d)/*.md \| head -1)` |
| Inspect tasks (.openagents) | `cd /path/to/repo && cat .openagents/tasks.jsonl \| jq '.'`                          |
| Check beads (legacy)        | `cd /path/to/repo && $HOME/.local/bin/bd list --json`                                |
| Reset bead (legacy)         | `$HOME/.local/bin/bd update <id> --status open`                                      |
| Check last commits          | `cd /path/to/repo && git log --oneline -5`                                           |
| Run tests                   | `cd /path/to/repo && bun test`                                                       |

For conceptual behavior and design details, see:

* `docs/mechacoder/README.md`
* `docs/mechacoder/GOLDEN-LOOP-v2.md`
* `docs/mechacoder/spec.md`
