# Khala Code Desktop Codex Fleet Runbook

**Date:** 2026-06-29
**Audience:** OpenAgents operators, manager agents, and desktop implementers
**Canonical goal:** Run the Khala/Codex fanout loop through OpenAgents Desktop
with programmatic controls, while preserving exact token accounting and enough
resume state for the next operator after compaction, restart, or reboot.

**Current lane as of 2026-06-30:** the fastest way to get a local Codex worker
moving from the desktop app is **Khala Code Desktop** in
`clients/khala-code-desktop`, using its owner-local tools:

- `pylon_ensure`
- `codex_fleet_status`
- `codex_spawn`

The older `clients/openagents-desktop` controls below remain useful source
material for the broader fanout manager, but the working MVP path today is
Khala Code Desktop → local Pylon → hosted Khala assignment → local Codex runner
→ no-spend closeout.

This replaces the shell-loop era runbook. The shell commands below are still
included because they are the bridge the current Desktop process uses under the
hood, and because they are the emergency fallback when a manager must get work
moving before a UI control is finished.

## Current Quick Start: Get Khala Code Desktop Moving

From a clean, current `main` checkout:

```sh
cd /Users/christopherdavid/work/openagents
git fetch origin +refs/heads/main:refs/remotes/origin/main
git status --short --branch
bun install --frozen-lockfile
```

Use the normal local Pylon home unless intentionally testing another one:

```sh
export OPENAGENTS_REPO_ROOT=/Users/christopherdavid/work/openagents
export OPENAGENTS_PYLON_APP_PATH="$OPENAGENTS_REPO_ROOT/apps/pylon"
export PYLON_HOME="${PYLON_HOME:-$HOME/.openagents/pylon}"
```

For hosted Khala chat, the desktop process also needs an owner-linked
`OPENAGENTS_AGENT_TOKEN` in the environment. Source it from the local ignored
secret file; never print the token, paste it into logs, or commit it. Do **not**
use `OPENROUTER_API_KEY` as a local bypass for Khala.

Important: **do not globally export `PYLON_OPENAGENTS_BASE_URL` just to inspect
local capacity from Khala Code Desktop.** The current desktop wrapper passes
`--base-url https://openagents.com` on network commands that need it. Local
provider/status probes should stay local; forcing the hosted base URL into
every Pylon command can make capacity look like stale hosted pressure (`0/4`)
even when local Codex capacity is free.

Verify local Pylon and Codex fleet state:

```sh
bun apps/pylon/src/index.ts provider go-online --json \
  | jq '{pylonRef, codingCapacity, ownCapacityDispatch}'

bun apps/pylon/src/index.ts codex accounts list --json \
  | jq '.accounts[] | {accountRef, provider, readiness, accountRefHash}'
```

Expected:

- `ownCapacityDispatch.availableCodexAssignments` is greater than 0 when no
  worker is running.
- At least one named Codex account, usually `codex-2` or `status`, has
  `readiness.state == "ready"`.
- `codex` may be `credentials_revoked`; that is not fatal if another named
  account is ready.
- `(default)` may be ready, but Khala Code Desktop now prefers named ready
  accounts before `(default)` for automatic `codex_spawn`.
- `codex_spawn` plans against advertised per-account slots, then launches the
  planned slots concurrently. The Desktop MVP caps requested fanout at five.
  For a deliberate five-worker smoke, advertise five account slots with
  `OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5`; do not set that globally
  unless the machine should really take five local Codex runs at once.

Start the app:

```sh
bun run --cwd clients/khala-code-desktop dev
```

In the chat, the useful operator prompts are deliberately plain:

```txt
pylon_ensure
codex_fleet_status
Check pylon fleet status and delegate a demo read-only task to one of the connected codexes, then summarize its result.
```

The only fleet tools in this MVP are `pylon_ensure`, `codex_fleet_status`, and
`codex_spawn`. Do not ask for or invent `codex_terminate`; it does not exist.

## Current Headless Smoke

Use this when the UI is suspect and you need to prove the same desktop tool path
outside the window:

```sh
cd /Users/christopherdavid/work/openagents

bun --eval '
  import { spawnCodexInstances } from "./clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts";
  const result = await spawnCodexInstances({
    prompt: "Read the bounded public fixture and summarize it in one sentence.",
    fixture: true,
    timeoutMs: 300000
  });
  console.log(JSON.stringify(result, null, 2));
'
```

Expected green shape:

```json
{
  "acceptedCount": 1,
  "requestedCount": 1,
  "results": [
    {
      "accountRef": "status",
      "autoRunOk": true,
      "status": "accepted"
    }
  ]
}
```

`accountRef` may be any ready named account with advertised free capacity
(`status`, `codex-2`, or another linked account). If `codex-2` is busy and
`status` has a free slot, Desktop should pick `status`.

For a bounded five-slot smoke of the same Desktop tool path:

```sh
OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5 bun --eval '
  import { spawnCodexInstances } from "./clients/khala-code-desktop/src/bun/khala-codex-fleet-tools.ts";
  const result = await spawnCodexInstances({
    count: 5,
    prompt: "Read the bounded public fixture and summarize it in one sentence.",
    fixture: true,
    timeoutMs: 600000
  });
  console.log(JSON.stringify(result, null, 2));
'
```

Expected: `acceptedCount` is `5`, `requestedCount` is `5`, every slot is
`accepted`, and the selected account refs correspond to accounts that advertised
free slots. If it refuses before launch with `Only X/5 advertised ... free`,
capacity was not actually advertised; rerun the heartbeat/status commands and
check `ownCapacityDispatch.codexAccounts`.

Verified live on 2026-06-30 after deploying `ba9a5424e5`:

- command above launched five concurrent `khala request` wrappers
- all five ran through `codex-2` with public account hash
  `account.pylon.codex.651c03fed68925d7acb2c02f`
- all five returned `auto-run: completed`, `assignment run: completed`,
  `closeout: accepted, no-spend, not_applicable`, and `blocker refs: none`
- assignment refs:
  `assignment.public.khala_coding.chatcmpl_47bea9cdeb0b40e994beb8b4fbfb4727`,
  `assignment.public.khala_coding.chatcmpl_89d38b66e198475fb7ef64373db368ad`,
  `assignment.public.khala_coding.chatcmpl_3b1a9195bb594acd82918baca6dcc473`,
  `assignment.public.khala_coding.chatcmpl_2c6f011dba654af3898bee3320f65d84`,
  `assignment.public.khala_coding.chatcmpl_24d158a8b1b14b5c893a70ba7fdf706e`
- post-run: no active markers remained and
  `ownCapacityDispatch.codexAccounts` returned both named accounts to `5/5`
  available; the old pooled `availableCodexAssignments/maxCodexAssignments`
  still showed `4/4`, so use `codexAccounts` for five-slot Desktop smoke
  validation.

## Current A2A Transaction Step: Provider-Bond Contract

Checked again on 2026-06-30 before the first forfeitable-bond implementation
pass:

- local Pylon online: `pylon.33afd48282a649047e3a`
- active assignment markers: `0`
- ready Codex refs: `(default)`, `codex-2`, `status`
- `codex` remained `credentials_revoked`; the historical backup/supervisor refs
  remained `credentials_missing`
- old pooled capacity still reported `4/4`, so keep using the per-account
  projection for five-slot validation

The next concrete step toward agents transacting naturally with each other
(Nostr negotiation, Lightning/MDK/Ark later, forfeitable funds) is tracked in
`docs/labor/2026-06-30-forfeitable-bond-next-step.md`. FB-1 is now the package
contract step:

- `packages/nip90/src/lbr-bond.ts` defines ref-only `provider_bond`,
  `bond_release`, and `bond_forfeit` kind-7000 feedback variants.
- `packages/nip90/src/lbr-closeout.ts` can bind a terminal bond outcome into
  the content-addressed LBR closeout digest.
- Verification:

```sh
bun run --cwd packages/nip90 typecheck
bun run --cwd packages/nip90 test
```

This is still protocol-only. It moves no sats, creates no hold invoice, imports
no Lightning/Ark rail, changes no escrow ledger state, and does not upgrade any
public promise. The next real implementation phase is FB-2: add the validator-
only credit-ledger `forfeit` terminal state and update
`apps/openagents.com/INVARIANTS.md` with regression coverage.

The summary should include:

- `auto-run: completed`
- `assignment run: completed`
- `closeout: accepted, no-spend, not_applicable`
- `blocker refs: none`

### Lifecycle / Failure Rendering Contract

As of 2026-06-30, the Pylon/Desktop handshake carries assignment lifecycle
evidence in both places operators need it:

- `pylon khala request --json` still streams public-safe lifecycle JSONL to
  stderr while the worker is active.
- The final JSON stdout now also includes `assignmentLifecycleEvents`, so a
  completed tool card can summarize the same path without scraping stderr.
- Khala Code Desktop parses both the final array and the stderr JSONL fallback.
  Timeout summaries should show `command timed out` plus the last lifecycle
  state, for example `assignment_run.runtime_started (phase=runtime_active)`.
- If a hosted assignment ref was created but the local auto-run returns
  `autoRun.ok: false`, `codex_spawn` is a failed slot. It must render red and
  include the closeout/blocker refs instead of showing a green `OK` card.

This is intentionally not a new execution path. The local worker still runs
through Pylon `runNoSpendAssignment`; the change is that Desktop no longer
collapses a live worker into a blank "running" card or raw JSON failure blob.

After the smoke, confirm Pylon is back to idle:

```sh
find "$PYLON_HOME/active-assignment-runs" -maxdepth 1 -type f -name '*.json' -print

bun apps/pylon/src/index.ts provider go-online --json \
  | jq '.ownCapacityDispatch | {availableCodexAssignments, maxCodexAssignments, codex, loadRefs}'
```

Expected: no active marker files and capacity back at `N/N available` (on the
owner machine this was `4/4` after the 2026-06-30 smoke).

## Assign One Issue To A Codex, Non-Blocking (verified 2026-06-30)

This is the smallest "route one GitHub issue to a local Codex worker" recipe,
the headless equivalent of typing a task into Khala Code Desktop. It splits
*assign* (fast, returns a ref) from *run* (long, backgrounded) so an operator or
manager agent does not block while Codex works.

```sh
cd /Users/christopherdavid/work/openagents
export OPENAGENTS_REPO_ROOT=/Users/christopherdavid/work/openagents
export OPENAGENTS_PYLON_APP_PATH="$OPENAGENTS_REPO_ROOT/apps/pylon"
export PYLON_HOME="${PYLON_HOME:-$HOME/.openagents/pylon}"
COMMIT="$(git rev-parse origin/main)"
PYLON="bun $OPENAGENTS_PYLON_APP_PATH/src/index.ts"

# 0) Preflight: a ready Codex account + free capacity.
$PYLON codex accounts list --json | jq -c '[.accounts[]?|{ref:(.accountRef//.ref), readiness:(.readiness.state//.readiness//.status)}]'
$PYLON provider go-online --json | jq '{pylonRef, cap:(.ownCapacityDispatch|{availableCodexAssignments,maxCodexAssignments})}'

# 1) REQUIRED before khala request: publish a FRESH server heartbeat, or the
#    request 409s with a stale-heartbeat error (see gotchas). provider go-online
#    alone does NOT refresh the server's view.
$PYLON presence heartbeat --base-url https://openagents.com --json >/dev/null 2>&1 || true

# 2) Plan (non-mutating) and read the target pylon ref from the slot.
$PYLON khala dispatch --base-url https://openagents.com \
  --candidates issue:7652 --accounts codex-2 --concurrency 1 \
  --priority-lane khala-code --repo OpenAgentsInc/openagents --branch main \
  --commit "$COMMIT" --verify "bun run test:khala-tools" --json \
  | jq -r '.slots[0].requestInput.targetPylonRef'   # e.g. pylon.33afd48282a649047e3a

# 3) Create the assignment WITHOUT running it; capture the ref immediately.
ASSIGN="$($PYLON khala request --base-url https://openagents.com \
  --account codex-2 --pylon-ref pylon.33afd48282a649047e3a \
  --prompt "You are working in OpenAgentsInc/openagents. Implement issue #7652 ... Run 'bun run test:khala-tools' before finishing. Open or update a PR that closes #7652." \
  --workflow codex_agent_task --repo OpenAgentsInc/openagents --branch main \
  --commit "$COMMIT" --verify "bun run test:khala-tools" \
  --no-run --json | jq -r '.assignmentRef')"
echo "$ASSIGN"

# 4) Run it (long; background it). Codex executes in a materialized workspace.
nohup $PYLON assignment run-no-spend --base-url https://openagents.com \
  --assignment-ref "$ASSIGN" --json > /tmp/run-$ASSIGN.json 2>/tmp/run-$ASSIGN.err &

# 5) Confirm it was accepted and a codex exec child is live.
grep -o '"event":"[^"]*"' /tmp/run-$ASSIGN.err | tail -3   # want assignment_run.accepted then runtime_progress
ps -axo pid,etime,command | grep 'codex exec' | grep -v grep
```

The materialized workspace for inspection is
`$PYLON_HOME/cache/codex-agent-tasks/workspace.pylon.codex_agent_task.<statusHash>`
(the `<statusHash>` is the tail of the `statusRef` in the `assignment_run.accepted`
event). When the runner exits, verify:

```sh
$PYLON khala closeout "$ASSIGN" --base-url https://openagents.com --json | jq '.closeoutChecklist.ok'
$PYLON khala proof "$ASSIGN" --base-url https://openagents.com --json
```

Expected: `closeoutChecklist.ok: true`, `paymentMode: no-spend`,
`settlementState: not_applicable`, `payoutClaimAllowed: false`, and exact token
rows. This is own-capacity work: it consumes the linked ChatGPT account's rate
budget, not OpenAgents settlement.

### Field Notes And Gotchas (2026-06-30)

- **`khala request` 409 "stale or missing heartbeat" even when local capacity is
  `4/4`.** `provider go-online` (no base URL) refreshes the *local* capacity view
  only; the server still sees the Pylon as stale. Always run
  `presence heartbeat --base-url https://openagents.com` immediately before the
  request. Error/evidence refs:
  `evidence.khala_coding.target_pylon_ref.unavailable.stale_or_missing_heartbeat`.
- **`presence heartbeat --json` can print nothing to stdout yet still exit 0 and
  succeed** — the server write is the side effect. Do not treat empty output as
  failure; confirm by retrying the request (the 409 clears) or checking the
  public projection. It exited cleanly here, so the "needs an outer timeout"
  caveat did not bite on this build.
- **macOS has no `timeout`/`gtimeout`.** Wrapping a Pylon command in `timeout`
  fails with `command not found`, and inside a pipe that silently produces empty
  output (looks like the Pylon command "hung" or "returned nothing"). Run the
  Pylon command directly, or install coreutils and use `gtimeout`.
- **Dispatch slot shape (`...khala_dispatch_plan.v0.1`).** The target pylon ref is
  `slots[0].requestInput.targetPylonRef`, not a top-level field; the account hash
  is `slots[0].account.accountRefHash`; the workspace/verifier are under
  `slots[0].requestInput.workspace`. A bare `--candidates issue:NNNN` auto-derives
  a generic objective ("Implement OpenAgents issue #NNNN. ... Run verifier: ..."),
  so pass the real, bounded prompt on `khala request --prompt`.
- **`codex accounts list --json` returns `ok: null`** (not `true`) in this build;
  read readiness from `.accounts[].readiness` (string: `ready` /
  `credentials_revoked` / `credentials_missing`). Prefer a named `ready` account
  (e.g. `codex-2`); `codex` (default) was `credentials_revoked` here.
- **The `codex exec` child confirms the owner-local executor invariant:**
  `codex exec --experimental-json --sandbox danger-full-access --skip-git-repo-check
  --config sandbox_workspace_write.network_access=true --config approval_policy=never`
  in the materialized cache workspace. That full access is owner-local only; never
  a public wire field.
- **A concurrent fleet burst is normal on the owner machine.** `poll_complete`
  showed `leaseCount: 25` while a separate controller was bursting; the new
  assignment was still admitted because Codex capacity was free. Capacity, not the
  raw process count, is the gate — check `availableCodexAssignments`, not `ps`.

## Current Troubleshooting Cheatsheet

### `codex_fleet_status` Says `0/N Available`

First distinguish real busy capacity from poisoned local status:

```sh
find "$PYLON_HOME/active-assignment-runs" -maxdepth 1 -type f -name '*.json' -print -exec sed -n '1,120p' {} \;

ps -axo pid,ppid,etime,command \
  | rg 'khala request|codex exec|assignment run-no-spend' \
  | rg -v 'rg|ps -axo'

bun apps/pylon/src/index.ts provider go-online --json \
  | jq '.ownCapacityDispatch | {availableCodexAssignments, maxCodexAssignments, codex, loadRefs}'
```

If there is an active marker and a live `codex exec`, the worker is actually
running. If there are no markers and `provider go-online` says capacity is free,
Desktop should also show free capacity on current code. If Desktop still says
`0/N`, verify you are on or after commit
`a0e3a20df1214dd0084ac7b636462151d2ebb309` and that the app was restarted from
that checkout.

Do not “fix” this by exporting `PYLON_OPENAGENTS_BASE_URL` globally. Use explicit
`--base-url https://openagents.com` only on commands that talk to
`openagents.com`, such as `presence heartbeat`, `assignment run-no-spend`, and
`khala request`.

### `codex_spawn` Card Sits On `RUNNING`

Check the same process/marker commands above.

- Marker refreshing + `codex exec` visible: local Codex is still running.
- Marker gone + capacity free + `khala request` wrapper still alive: local Codex
  finished and the wrapper is in the post-runtime network submit leg.
- Current Pylon has a 30s default assignment HTTP timeout for poll, progress,
  artifact, and closeout calls. If a wrapper predates that fix, stop that smoke
  and retry from current `main`.

The UI should render timed-out/failed `codex_spawn` cards as failed, not green.
The tool result is failed whenever accepted count is less than requested count.

### `codex_spawn` Returns Duplicate Active Assignment

If the tool fails with
`blocker.public.pylon_dispatch.duplicate_active_assignment` while
`codex_fleet_status` shows another named account has free capacity, check the
handoff shape and deployed Worker version:

- Pylon must send the public-safe account hash both under
  `openagents.coding.targetAccountRefHash` and at root `targetAccountRefHash`.
- The Worker must accept either shape and scope the duplicate gate to that
  account's advertised slot count.
- After patching either side, deploy `apps/openagents.com` with the sanctioned
  `bun run --cwd workers/api deploy:safe` path, heartbeat again, and retry.

Do not work around this by deleting active markers from a live worker. First
confirm whether `codex exec` is still running for the assignment.

### `codex_spawn` Chooses The Wrong Account

Automatic selection now prefers named ready accounts before `(default)`. If you
need to force a specific account:

```txt
Run codex_spawn with account_ref codex-2 and fixture true for a demo read-only task.
```

If the account is present but not ready, fix that account instead of relogging
the default `~/.codex`. Use isolated Pylon account homes only.

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
export OPENAGENTS_DESKTOP_KHALA_FLEET_DB="$HOME/.openagents/desktop/khala-fleet.sqlite"

# Use the existing local Pylon home unless intentionally testing another one.
export PYLON_HOME="${PYLON_HOME:-$HOME/.openagents/pylon}"
```

For raw Pylon commands that talk to `openagents.com`, pass
`--base-url https://openagents.com` explicitly. Avoid exporting
`PYLON_OPENAGENTS_BASE_URL` in the long-lived shell that launches Khala Code
Desktop or runs local provider status probes.

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
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" presence heartbeat --base-url https://openagents.com
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
  --base-url https://openagents.com \
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
  --base-url https://openagents.com \
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
- missing base URL: pass `--base-url https://openagents.com` on the network
  command instead of exporting a global env var into local status probes.

## Execute Planned Work

The current Desktop bridge plans slots and records/visualizes the fleet. The
actual execution bridge is still Pylon's assignment path.

For each planned candidate/account pair:

```sh
PYLON_REF="<target pylon ref from the dispatch slot>"

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala request \
  --base-url https://openagents.com \
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
  --base-url https://openagents.com \
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
bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala proof "$ASSIGNMENT_REF" \
  --base-url https://openagents.com \
  --json
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
3. Confirmed missing `--base-url` fails fast for `khala dispatch`.
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
  --base-url https://openagents.com \
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
COMMIT="$(git rev-parse origin/main)"
VERIFY="bun scripts/check-conflict-markers.mjs"

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" codex accounts list --json

bun "$OPENAGENTS_PYLON_APP_PATH/src/index.ts" khala dispatch \
  --base-url https://openagents.com \
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
