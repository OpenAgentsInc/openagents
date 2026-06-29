# OpenAgents Desktop Codex Fanout Runbook

**Date:** 2026-06-29
**Audience:** OpenAgents operators, manager agents, and desktop implementers
**Canonical goal:** Run the Khala/Codex fanout loop through OpenAgents Desktop
with programmatic controls, while preserving exact token accounting and enough
resume state for the next operator after compaction, restart, or reboot.

This replaces the shell-loop era runbook. The shell commands below are still
included because they are the bridge the current Desktop process uses under the
hood, and because they are the emergency fallback when a manager must get work
moving before a UI control is finished.

## Hard Truth Of The Current Build

OpenAgents Desktop already owns the visible control surface under
`clients/openagents-desktop`, package `@openagentsinc/desktop`.

The Desktop webview calls its Bun side through Electrobun RPC. The important
methods are:

- `codingStatus()`
- `createPylon()`
- `khalaDispatchPlan(input)`
- `khalaFleetSnapshot()`
- `pylonStatus()`
- `replayTokenFailures()`
- `tokenAccountingStatus()`
- `verifyAssignmentTokenUsage(assignmentRef)`

Those are real in-process Desktop controls. They are not yet exposed as a
stable external HTTP or JSON-RPC endpoint. Until that endpoint exists, external
manager agents should drive the same underlying Pylon and Codex commands that
Desktop uses, keep Desktop running for visibility/reconciliation, and treat the
Desktop event store as the durable resume surface.

Codex itself has two programmatic modes that matter:

- `codex exec --json`: immediate non-interactive work from the current shell or
  an isolated account home. It emits JSONL events and final token usage. This
  can start useful work from the existing logged-in Codex session.
- `codex app-server`: JSON-RPC over stdio, WebSocket, or Unix socket. This is
  the richer long-term bridge Desktop should launch and own. The current
  OpenAgents Desktop fanout path has not fully switched to app-server yet.

## Non-Negotiable Rules

1. Use exactly one fanout controller at a time.
2. Never run `codex login` or `codex login --device-auth` against default
   `~/.codex` for fleet accounts. Use isolated account homes created by Pylon
   or `khala fleet connect`.
3. Do not kill in-flight `codex exec` children just to make the process table
   look clean. A killed turn can leave a stale lease and can lose accounting.
4. Do not count a job as complete until both execution and token proof have
   been reconciled.
5. Run a non-mutating dispatch plan before launching a burst.
6. Keep the Desktop app open during fanout so the Coding page, Pylon page,
   token replay panel, and local fleet event store stay current.
7. If the default checkout is dirty with other agents' work, create a clean
   worktree from `origin/main` for scoped fixes. Do not stash or reset other
   agents' work.

## Required Local Pieces

From the repo root:

```sh
cd /Users/christopherdavid/work/openagents
bun install --frozen-lockfile
```

Required CLIs:

```sh
npm install -g @openai/codex @openagentsinc/khala
```

Required environment for the current Desktop/Pylon bridge:

```sh
export OPENAGENTS_REPO_ROOT=/Users/christopherdavid/work/openagents
export OPENAGENTS_PYLON_APP_PATH="$OPENAGENTS_REPO_ROOT/apps/pylon"
export PYLON_OPENAGENTS_BASE_URL=https://openagents.com
export OPENAGENTS_DESKTOP_KHALA_FLEET_DB="$HOME/.openagents/desktop/khala-fleet.sqlite"

# Use the existing local Pylon home unless intentionally testing another one.
export PYLON_HOME="${PYLON_HOME:-$HOME/.openagents/pylon}"
```

Authentication is normally read from the stored local Pylon/OpenAgents token.
If an operator injects `OPENAGENTS_AGENT_TOKEN`, never print it and never write
it to tracked files.

## Connect Codex Accounts

The only part that should need a human is the ChatGPT device authorization flow
for each account. The preferred flow is:

```sh
khala fleet connect
khala fleet status
```

This creates isolated account homes under Pylon-managed account directories.
If you need to drive Pylon directly, list accounts first:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" codex accounts list --json
```

For a specific account that needs login, use the Pylon account command from the
repo's current `AGENTS.md` guidance, not default `~/.codex`:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" auth codex \
  --account codex-N \
  --force-device-login
```

The device code flow can also be used by Codex directly for a one-off isolated
home:

```sh
CODEX_HOME="$HOME/.pylon-fable/accounts/codex/codex-N" codex login --device-auth
```

Use direct `CODEX_HOME=... codex login --device-auth` only when Pylon account
commands are unavailable. The safer path is `khala fleet connect`.

## Start Desktop

Start the app from the repo root:

```sh
OPENAGENTS_REPO_ROOT="$OPENAGENTS_REPO_ROOT" \
PYLON_OPENAGENTS_BASE_URL="$PYLON_OPENAGENTS_BASE_URL" \
bun run --cwd clients/openagents-desktop dev
```

The app window should show:

- top-right `CODING: N`
- top-right `PYLONS: N`
- a Coding page with active/recent sessions and transcript detail
- a Pylons page with connected user pylons
- token accounting status and replay controls

If the app is already running, leave it open. Do not restart it in the middle
of an active fanout unless the UI bridge is wedged and the Pylon/Codex children
have been verified independently.

## Start Or Refresh Local Pylon

From Desktop, the `Create Pylon` button calls `createPylon()`, which runs:

```sh
bun run --cwd "$OPENAGENTS_PYLON_APP_PATH" start
```

The headless equivalent is:

```sh
bun run --cwd "$OPENAGENTS_PYLON_APP_PATH" start
```

Bring the node online and publish a fresh heartbeat when the CLI supports it:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" provider go-online
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" presence heartbeat
```

If those commands fail because this checkout has a newer or older CLI shape,
use Desktop's Pylons page and `pylonStatus()` as source of truth, then inspect:

```sh
find "$PYLON_HOME" "$HOME/.pylon-fable" -maxdepth 3 -type f \
  \( -name '*heartbeat*' -o -name '*capacity*' -o -name '*assignment*' \) \
  2>/dev/null | head -50
```

## Start From The Existing Codex Session If Needed

If the fleet accounts are not connected yet, an operator can still get one
bootstrap task moving through the current Codex login:

```sh
cd "$OPENAGENTS_REPO_ROOT"
codex exec --json \
  "Inspect issues 7590-7598 and report the safest merge order. Do not edit files."
```

This is useful for immediate analysis. It is not the preferred Khala counted
path unless wrapped by Pylon assignment execution. If the goal is Khala token
usage and public accounting, route work through Pylon assignments below.

## Programmatic Control Map

| Desired action | Desktop method | Current headless bridge |
| --- | --- | --- |
| Show Codex sessions | `codingStatus()` | process table, Codex JSONL rollout files, Pylon active markers |
| Show user pylons | `pylonStatus()` | Pylon provider/presence commands and local Pylon home |
| Start local pylon | `createPylon()` | `bun run --cwd apps/pylon start` |
| Plan fanout slots | `khalaDispatchPlan(input)` | `pylon khala dispatch --json` |
| Show durable queue state | `khalaFleetSnapshot()` | SQLite store at `OPENAGENTS_DESKTOP_KHALA_FLEET_DB` |
| Check token failures | `tokenAccountingStatus()` | `~/.pylon-fable/codex-turn-report-failures.jsonl` |
| Replay token failures | `replayTokenFailures()` | Desktop RPC today; verify with proof CLI after replay |
| Verify one assignment | `verifyAssignmentTokenUsage(ref)` | `pylon khala proof <assignmentRef> --json` |

The headless bridge is intentionally listed because a manager agent can run it
today, while the Desktop renders and reconciles the same state.

## Preflight Checklist Before Fanout

Run these in order:

```sh
cd "$OPENAGENTS_REPO_ROOT"
git fetch origin +refs/heads/main:refs/remotes/origin/main
COMMIT="$(git rev-parse origin/main)"
VERIFY="bun scripts/check-conflict-markers.mjs"
```

Check account readiness without printing sensitive auth:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" codex accounts list --json \
  | node -e '
      const fs = require("fs");
      const json = JSON.parse(fs.readFileSync(0, "utf8"));
      const accounts = Array.isArray(json.accounts) ? json.accounts : [];
      console.log(JSON.stringify({
        ok: json.ok !== false,
        count: accounts.length,
        ready: accounts
          .map((a) => ({ accountRef: a.accountRef || a.ref || null, readiness: a.readiness || a.status || null }))
          .filter((a) => a.accountRef)
      }, null, 2));
    '
```

Inspect local active work:

```sh
python3 - <<'PY'
import glob, json, os, subprocess
markers = glob.glob(os.path.expanduser('~/.pylon-fable/active-assignment-runs/*.json'))
ps = subprocess.check_output(['ps', '-axo', 'pid,ppid,etime,command'], text=True)
print('active_assignment_markers', len(markers))
print('codex_exec_processes', sum(1 for line in ps.splitlines() if 'codex exec' in line))
print('khala_request_wrappers', sum(1 for line in ps.splitlines() if 'khala request' in line))
spool = os.path.expanduser('~/.pylon-fable/codex-turn-report-failures.jsonl')
print('token_failure_spool_bytes', os.path.getsize(spool) if os.path.exists(spool) else 0)
PY
```

If the token failure spool is non-empty, do not launch a massive burst until
you understand whether accounting is degraded.

## Plan A Burst

`khala dispatch` is a non-mutating planner. It proves that the account list,
candidate list, concurrency, base URL, repo, commit, and verifier can form real
assignment requests.

Example for one smoke slot:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala dispatch \
  --candidates issue:7590 \
  --accounts codex-2 \
  --concurrency 1 \
  --priority-lane khala-code-smoke \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$COMMIT" \
  --verify "$VERIFY" \
  --json
```

Example for a wider issue/PR burst:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala dispatch \
  --candidates issue:7590,issue:7591,issue:7592,issue:7593,issue:7594,issue:7595,issue:7596,issue:7597,issue:7598 \
  --accounts codex-2,codex-3,codex-4,codex-5,codex-6,codex-7 \
  --concurrency 12 \
  --priority-lane khala-code \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$COMMIT" \
  --verify "$VERIFY" \
  --json
```

A healthy plan has:

- Desktop RPC wrapper: `ok: true`
- raw Pylon CLI output: `schema: "openagents.pylon.khala_dispatch_plan.v0.1"`
- at least one `slots[]` entry
- `blockerRefs: []`
- each slot has `workflow: "codex_agent_task"`
- each slot has a concrete `accountRefHash`
- each slot carries the exact `repository`, `commit`, and `verifier`

Common blockers:

- `blocker.khala_dispatch.no_account_targets`: the account refs are not known
  or are not Codex-capable.
- `blocker.khala_dispatch.no_dispatch_slots`: account/candidate/concurrency
  combination produced no work.
- missing `PYLON_OPENAGENTS_BASE_URL`: pass `--base-url` or export the env var.

## Execute Planned Work

The current Desktop bridge plans slots and records/visualizes the fleet. The
actual execution bridge is still Pylon's assignment path.

For each planned candidate/account pair:

```sh
PYLON_REF="<target pylon ref from the dispatch slot>"

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala request \
  --account codex-2 \
  --pylon-ref "$PYLON_REF" \
  --prompt "You are working in OpenAgentsInc/openagents. Complete issue #7590. Run the required verifier before finishing. Open or update the PR that closes the issue." \
  --workflow codex_agent_task \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$COMMIT" \
  --verify "$VERIFY" \
  --json
```

If the request creates an assignment but does not run it automatically, run the
assignment through the no-spend local executor:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" assignment run-no-spend \
  --assignment-ref "$ASSIGNMENT_REF" \
  --json
```

Do not fan out only by spawning raw `codex exec` processes unless the user has
explicitly accepted uncounted work. Pylon assignment execution is what binds the
work to Khala assignment refs and token proof.

## Monitor Fanout In Desktop

Open the Desktop Coding page. It must show:

- `CODEX EXEC`: live `codex exec` children
- `BURNING`: active assignment runs consuming Codex
- `KHALA REQ`: request wrappers
- `READY`: known ready account capacity where available
- active or recent sessions at the top
- selected transcript messages, tool calls, and tool outputs
- recent dispatch/refusal events

If the top says no live sessions but the left list contains active processes,
the UI filter is wrong. The backing state is still usable; inspect with:

```sh
ps -axo pid,ppid,etime,command | rg 'codex exec|khala request|assignment run-no-spend' || true
find "$HOME/.pylon-fable/active-assignment-runs" -type f -maxdepth 1 2>/dev/null | wc -l
```

Do not treat `assignment_run.accepted` as success. Watch for a final
`assignment_run.completed` status and then verify token proof.

## Token Accounting And Replay

Every Codex turn that executes for Khala must land in token accounting. The
Desktop bridge reads the local failure spool:

```sh
test -s "$HOME/.pylon-fable/codex-turn-report-failures.jsonl" && {
  wc -l "$HOME/.pylon-fable/codex-turn-report-failures.jsonl"
  jq -r '{assignmentRef:.report.assignmentRef, usage:.report.usage, error:(.error|tostring|.[0:160])}' \
    "$HOME/.pylon-fable/codex-turn-report-failures.jsonl"
} || echo 'no token report failures'
```

If failures exist, use Desktop's replay control, which calls
`replayTokenFailures()`. Then verify each assignment:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala proof "$ASSIGNMENT_REF" --json
```

Expected result:

- the assignment ref exists
- the proof has exact input/output/total token counts
- the public `/stats` and `/khala` counters eventually include the usage

If replay still fails, stop launching new work, preserve the failure spool, and
document the assignment refs in the Desktop event store. Do not hand-wave
missing accounting.

## Rate Limits And Resets

OpenAgents Desktop must surface account readiness and cooldowns. Until the
Desktop controls are fully switched to Codex app-server/account APIs, use the
Pylon account commands:

```sh
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" accounts status --json
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" accounts usage --account codex-2 --refresh --json
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" accounts status --account codex-2 --reset --json
```

The policy is:

- If an account only exhausted the short 5h window but has weekly budget left,
  wait for the cooldown.
- If weekly usage is exhausted and the account supports reset, use the reset
  control for that account.
- Record reset attempts and outcomes in the Desktop fleet event store.

Do not repeatedly dispatch into an account that is returning execution refusal
or quota exhaustion. That creates noisy rejected assignments and hides the real
upper bound.

## Safe Stop

Stop only the launcher/controller first:

```sh
pgrep -fl 'khala.*refill|codex.*refill|openagents_burst|rate_limit_pr_refill|pr-review-refill' || true
```

Then kill only the controller PIDs, not child Codex turns:

```sh
kill <controller-pid>
```

After that, let active `codex exec` children close out. Verify:

```sh
ps -axo pid,ppid,etime,command | rg 'codex exec|assignment run-no-spend|khala request' || true
```

If a child is genuinely wedged, capture its assignment ref, last transcript
event, and token status before terminating it.

## Resume After Compaction Or Reboot

Run:

```sh
cd "$OPENAGENTS_REPO_ROOT"
git fetch origin +refs/heads/main:refs/remotes/origin/main

python3 - <<'PY'
import glob, os, subprocess
markers = glob.glob(os.path.expanduser('~/.pylon-fable/active-assignment-runs/*.json'))
print('active_assignment_markers', len(markers))
print(subprocess.check_output(['ps', '-axo', 'pid,ppid,etime,command'], text=True))
PY
```

Then open Desktop and inspect:

- Coding page live/recent sessions
- Pylons page connected pylons
- token failure panel
- local fleet snapshot
- GitHub issue/PR state

If there are no child processes but active markers remain, classify them as
stale only after checking recent logs and assignment proofs.

## What To Move Fully Into Desktop Next

The current runbook is intentionally executable today, but the product should
absorb these shell bridges:

1. Expose an external local Desktop control endpoint for manager agents. Use
   loopback plus a local token, matching Pylon's control-server shape.
2. Make `khalaDispatchPlan` optionally execute planned slots, not only plan
   them.
3. Launch Codex through `codex app-server` or `@openai/codex-sdk` so Desktop
   owns sessions, steering, transcripts, approvals, and usage events.
4. Store every planned slot, launched assignment, process PID, account state,
   closeout, proof, and replay attempt in the Desktop SQLite event store.
5. Add deterministic resume checks that compare:
   - GitHub issue/PR state
   - active assignment markers
   - live process table
   - Codex JSONL transcripts
   - token proof rows
6. Add one-click account cooldown and reset controls, with policy safeguards.
7. Retire temp shell refill loops once Desktop can execute and refill slots
   directly.

## Tested On 2026-06-29

The runbook was validated from a clean detached worktree at:

```txt
/tmp/openagents-desktop-fanout-runbook
```

The following checks were performed:

1. Installed dependencies with `bun install --frozen-lockfile`.
2. Confirmed `@openagentsinc/desktop` exposes the Desktop RPC methods listed
   above.
3. Confirmed missing `PYLON_OPENAGENTS_BASE_URL` fails fast for
   `khala dispatch`.
4. Confirmed account listing works without printing sensitive auth.
5. Confirmed a non-mutating dispatch plan with ready account `codex-2`,
   candidate `issue:7590`, `concurrency 1`, `repo OpenAgentsInc/openagents`,
   current `origin/main`, and verifier `bun scripts/check-conflict-markers.mjs`
   returned one slot and zero blockers.

Run these final verification commands after editing this runbook:

```sh
bun run --cwd clients/openagents-desktop verify

COMMIT="$(git rev-parse origin/main)"
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala dispatch \
  --candidates issue:7590 \
  --accounts codex-2 \
  --concurrency 1 \
  --priority-lane khala-code-smoke \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$COMMIT" \
  --verify "bun scripts/check-conflict-markers.mjs" \
  --json
```

The second command is non-mutating. It is safe to run as a smoke test, but it
depends on `codex-2` still being connected and ready on the local machine. If
it fails with `no_account_targets`, run the account connection flow first or
replace `codex-2` with a ready account from `codex accounts list --json`.

## Quick Operator Loop

For an urgent but controlled fanout:

```sh
cd "$OPENAGENTS_REPO_ROOT"
export PYLON_OPENAGENTS_BASE_URL=https://openagents.com
COMMIT="$(git rev-parse origin/main)"
VERIFY="bun scripts/check-conflict-markers.mjs"

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" codex accounts list --json

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala dispatch \
  --candidates issue:7590,issue:7591,issue:7592 \
  --accounts codex-2,codex-3,codex-4 \
  --concurrency 3 \
  --priority-lane khala-code \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "$COMMIT" \
  --verify "$VERIFY" \
  --json

# If the plan is healthy, launch one request per planned slot and watch Desktop.
```

Do not increase concurrency until the Desktop Coding page shows real live Codex
processes and the token failure spool remains empty.
