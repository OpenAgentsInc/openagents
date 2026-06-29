# Khala Codex Fleet Manager Runbook

**Date:** 2026-06-29
**Scope:** How the local manager agent has been dispatching OpenAgents PR and
issue work to Khala-backed local Codex workers, what failed, and how to move
the process into OpenAgents Desktop.
**Audience:** Future manager agents, desktop implementers, and operators trying
to keep Codex workers fanned out without losing accounting or confusing stale
leases for real work.

This is a companion to
[`2026-06-27-khala-codex-own-capacity-burn-runbook.md`](./2026-06-27-khala-codex-own-capacity-burn-runbook.md)
and the Codex fleet after-action in
[`../afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md`](../afteraction/2026-06-29-codex-fleet-throughput-collapse-after-action.md).

## Handoff Snapshot

At the stopping point for this report, the shell refill controller was stopped.
Do not assume the fleet is idle: existing `khala request` wrappers and Codex
child processes may still be completing. The last observed controller state:

- The generic PR refill loop was replaced with a targeted rate-limit/Pylon/Codex
  PR queue, then stopped.
- A final tick launched PR #7486 and PR #6831 before the controller received
  SIGTERM.
- The live token-report failure spool was empty.
- The repo-wide open PR count had fallen from the user's observed 418 to 267.
- Since `2026-06-29T13:45:00Z`, the GitHub query showed 19 merged PRs and 97
  closed PRs.

Before resuming, run the status commands in this document. Do not infer current
state from this snapshot.

## What The Manual Manager Did

The manual manager ran a local dispatch loop outside the product:

1. Query GitHub for open PRs or issues.
2. Choose a candidate set.
3. Claim a PR with a filesystem lock under `~/.codex-supervisor/pr-review`.
4. Fire a Khala coding assignment through local Pylon:

```sh
bun apps/pylon/src/index.ts khala request \
  --account codex-3 \
  --prompt "Resolve OpenAgents public PR #NNNN as part of the PR queue burndown..." \
  --workflow codex_agent_task \
  --repo OpenAgentsInc/openagents \
  --branch main \
  --commit "<origin/main sha>" \
  --verify "bun scripts/check-conflict-markers.mjs" \
  --json
```

5. Let Pylon auto-run the local no-spend assignment through Codex.
6. Monitor local markers, logs, GitHub state, and token-ingest failures.
7. Refill newly freed capacity.

The important distinction: `khala request` dispatch and Codex execution are not
the same thing. The manager must verify both the lease/assignment and the live
Codex child process.

## Local State Files And What They Mean

These paths were the real working surface during the session:

- `~/.pylon-fable/active-assignment-runs/*.json`: local active assignment
  markers. Useful but not sufficient; a marker can outlive useful execution.
- `~/.pylon-fable/pr-resolver-logs/pr-review-*.log`: JSON logs from PR
  resolution assignments.
- `~/.codex-supervisor/pr-review/locks/pr.<number>`: local PR claim locks.
- `~/.codex-supervisor/pr-review/done/pr.<number>`: local "do not retry"
  markers.
- `~/.pylon-fable/codex-turn-report-failures.jsonl`: exact token usage reports
  that failed to post and must be replayed.
- `~/.pylon-fable/replayed-failures/`: archived token-report replay files.
- `~/.pylon-fable/accounts/codex/<account>/`: isolated Codex homes. Never run
  login against default `~/.codex`.

The temporary controller was `/tmp/openagents_burst_pr_refill.sh`. Treat it as
an emergency sketch, not product code. It should be replaced by the desktop
harness described below.

## Dispatch Queue Selection

The generic burndown loop initially selected from all open PRs. After the user
reoriented the work toward Pylon and Codex rate limits, the candidate set was
changed to explicit high-value PRs:

```txt
7557 Expose Codex fleet quota cooldown state
7579 Expose Codex quota reset policy status
7560 fix(pylon): update account reset status
7523 fix(pylon): parse provider quota reset hints
7558 fix(pylon): classify codex account execution refusals
7246 fix(pylon): surface Codex execution refusal reasons
7221 fix(pylon): surface codex execution refusal reasons
7510 fix(khala-desktop): add Codex account readiness controls
7279 fix(codex-supervisor): GC orphan claims and fast-retry gate refusals
7104 fix(codex-supervisor): GC stale claims and fast-retry gate flakes
7073 feat(operator): surface live Pylon runtime progress
7283 fix(operator): register fleet state observability route
7230 feat(operator): surface fleet assignment progress
7336 feat(operator): surface fleet assignment progress
7589 Align Pylon coordinator with runner registry
7571 Harden Pylon agent runner resolution
7486 Harden Pylon agent runner registry contract
```

Manager agents should prefer this cluster until Codex account status, reset
timers, refusal reasons, fleet progress, and desktop visibility are integrated.
Do not let random PR burndown starve the rate-limit work.

## Safe Stop And Resume

To stop launching new work, kill only the controller loop and refill script:

```sh
pgrep -fl 'rate_limit_pr_refill|openagents_burst_pr_refill|pr-review-refill|burst_refill'
kill <controller-pid> <refill-script-pid>
```

Do not kill `codex exec` children or active `khala request` wrappers unless you
have proven they have no child process, no active marker, and no useful output
in flight. Killing an in-flight turn creates stale leases and hides token usage.

After stopping the launcher, verify:

```sh
pgrep -fl 'rate_limit_pr_refill|openagents_burst_pr_refill|pr-review-refill|burst_refill' || true
```

Then inspect active work:

```sh
python3 - <<'PY'
import glob, json, os, collections, subprocess
paths = glob.glob(os.path.expanduser('~/.pylon-fable/active-assignment-runs/*.json'))
by = collections.Counter()
for path in paths:
    try:
        data = json.load(open(path))
    except Exception:
        continue
    by[data.get('accountRefHash') or '-'] += 1
out = subprocess.check_output(['ps', '-axo', 'pid,ppid,etime,command'], text=True)
print('active_marker_files', len(paths))
print('by_account_hash', dict(by))
print('codex_exec', sum(1 for line in out.splitlines() if 'codex exec' in line))
print('khala_request_wrappers', sum(1 for line in out.splitlines() if 'bun apps/pylon/src/index.ts khala request' in line))
spool = os.path.expanduser('~/.pylon-fable/codex-turn-report-failures.jsonl')
print('failure_spool_bytes', os.path.getsize(spool) if os.path.exists(spool) else 0)
PY
```

## Deterministic Checks For Manager Agents

Always run these before deciding the fleet is healthy or stuck.

### 1. Active markers versus actual Codex

Compare marker count to `codex exec` process count. The marker count is not the
throughput number. During the session the system reached 23 active markers, but
actual `codex exec` processes lagged around 16-17. Report both numbers.

### 2. Recent PR assignment logs

Classify recent logs:

```sh
python3 - <<'PY'
import glob, os, collections
logs = glob.glob(os.path.expanduser('~/.pylon-fable/pr-resolver-logs/pr-review-*.log'))
logs.sort(key=os.path.getmtime, reverse=True)
c = collections.Counter()
for path in logs[:160]:
    text = open(path, errors='replace').read()
    if '"event":"assignment_run.completed"' in text and '"status":"accepted"' in text:
        c['completed_accepted'] += 1
    elif '"event":"assignment_run.completed"' in text and '"status":"rejected"' in text:
        c['completed_rejected'] += 1
    elif '"event":"assignment_run.accepted"' in text:
        c['accepted_running_or_unknown'] += 1
    elif '"ok": false' in text or '"error":' in text:
        c['failed_before_accept'] += 1
    elif text.strip():
        c['pending_output'] += 1
    else:
        c['empty'] += 1
print(dict(c))
PY
```

Do not treat an `assignment_run.accepted` event as success. A run can accept a
lease and then close out rejected with
`blocker.assignment.codex_agent_execution_refused` or
`blocker.assignment.codex_agent_test_failed`.

### 3. GitHub movement

Use GitHub state, not local optimism:

```sh
gh pr list --repo OpenAgentsInc/openagents --state open --json number --limit 500 --jq 'length'
gh pr list --repo OpenAgentsInc/openagents --state merged --json number,mergedAt --limit 150 \
  --jq '[.[] | select(.mergedAt >= "2026-06-29T13:45:00Z")] | length'
gh pr list --repo OpenAgentsInc/openagents --state closed --json number,closedAt --limit 150 \
  --jq '[.[] | select(.closedAt >= "2026-06-29T13:45:00Z")] | length'
```

### 4. Token usage failure spool

Every completed Codex turn must post exact usage. If this file is non-empty,
replay it before claiming accounting is complete:

```sh
test -s "$HOME/.pylon-fable/codex-turn-report-failures.jsonl" && {
  wc -l "$HOME/.pylon-fable/codex-turn-report-failures.jsonl"
  jq -r '{observedAt,error:(.error|tostring|.[0:120]), assignmentRef:.report.assignmentRef, usage:.report.usage}' \
    "$HOME/.pylon-fable/codex-turn-report-failures.jsonl"
} || echo 'no live failure spool'
```

The replay path is documented in the after-action. After replaying, query D1
for the assignment refs and verify `token_usage_events.total_tokens`.

## Lessons From This Session

### One controller only

Two refill loops with different state directories caused duplicate launches,
stale locks, and confusing output. Use exactly one controller and one lock
namespace. For the temporary controller that namespace was:

```sh
SUP_PR_REVIEW_STATE_DIR="$HOME/.codex-supervisor/pr-review"
SUP_PR_REVIEW_LOG_DIR="$HOME/.pylon-fable/pr-resolver-logs"
```

### A high lease count is not the same as useful fanout

The manager can briefly drive active assignment markers into the 20s while
actual `codex exec` is lower. The desktop must show both:

- `assigned`: accepted active leases
- `executing`: live Codex child processes
- `fresh`: last event/progress age
- `closed_accepted` and `closed_rejected`

### Capacity misses are normal under pressure

This failure is expected when a specific account is saturated:

```txt
pylon khala request failed (503): No linked, heartbeat-fresh,
Codex-capable Pylon capacity is available for this account.
```

Do not panic or mark the whole fleet broken. Back off that account or let the
refill loop try another account.

### Execution refused means the account or runner needs status, not guesswork

Several accepted PR assignments immediately closed rejected with:

```txt
blocker.assignment.codex_agent_execution_refused
```

This is why the rate-limit/status PR cluster matters. The manager needs typed
account state: ready, five-hour cooldown, weekly exhausted, auth refresh failed,
missing credentials, model unavailable, or execution refused for another typed
reason.

### `--verify true` is not a safe portable verifier

Some earlier workers failed closeout with `ENOENT: posix_spawn 'true'`. Use a
real repo command for lightweight PR review, for example:

```sh
--verify "bun scripts/check-conflict-markers.mjs"
```

For implementation work, use the narrow test named by the issue or PR.

### Log parsing must be exact

The temporary script initially parsed log names with:

```regex
(?:codex|codex-\d+)-([0-9]+)
```

For a log named `pr-review-...-codex-3-7557-...log`, this captured PR `3`
instead of `7557`. The correct order is:

```regex
(?:codex-\d+|codex)-([0-9]+)
```

This bug created bogus `pr.3` and `pr.7` bookkeeping and hid real PR locks. The
desktop harness must not parse critical state out of filenames; it should store
structured rows.

### Accepted logs can still be rejected work

The first lock-release logic skipped any log containing
`assignment_run.accepted`. That kept rejected runs locked forever. Correct
classification must prefer final lifecycle state:

1. `assignment_run.completed` with `status:"accepted"` means done.
2. `assignment_run.completed` with `status:"rejected"` means release or retry.
3. `assignment_run.accepted` without completed state means still running or
   unknown.
4. `ok:false` before acceptance means release or retry after backoff.

### Never kill in-flight Codex turns for cleanup

If a worker has a live `codex exec` child, let it finish. Killing the process
mid-turn creates stale leases and can delay dispatch for minutes. Only clean up
wrappers with no child process and no matching active assignment marker.

### Account inventory matters

During this session:

- `codex-2` had local state but no `auth.json`, so it was not a usable lane.
- `codex-6` had auth but had been removed from config earlier. Treat it as
  disabled until a rate-limit/status check says it can be safely resumed.
- `codex-3`, `codex-4`, `codex-5`, and `codex-7` were the active lanes.

Do not blindly add a configured account to the dispatcher. Confirm isolated
home auth, provider status, and reset/cooldown state first.

## Moving This Into OpenAgents Desktop

The current shell process should become a desktop-owned fleet harness. Desktop
already has the operator surface; it should also own the local deterministic
control plane.

### Target architecture

Add a local desktop service, conceptually `OpenAgentsDesktopFleetManager`, with
these parts:

- `CodexAccountRegistry`: reads isolated Codex homes, auth readiness, model
  cache status, last refresh time, and provider usage/cooldown state.
- `PylonCapacityService`: publishes heartbeat/go-online, reads current Pylon
  ref, and reconciles advertised capacity against active assignments.
- `GitHubQueueService`: builds PR and issue candidate queues using structured
  GitHub data, labels, search terms, merge state, and superseding relationships.
- `DispatchPlanner`: decides which account gets which task using typed account
  health, desired concurrency, per-account slots, backoff, and priority lanes.
- `AssignmentRunner`: calls the Pylon assignment APIs or a structured local Bun
  worker. Shelling out is acceptable as a bridge, but the product API should be
  structured IPC with JSON events, not filename parsing.
- `WorkerProcessTracker`: records child PIDs, start time, account, assignment
  ref, PR or issue ref, workspace path, current phase, last event time, and
  closeout status.
- `TokenReconciler`: watches exact Codex turn usage posts, replays local failure
  spools, and verifies D1 or public API projection before marking a run counted.
- `EventStore`: a local SQLite store inside the desktop profile. This replaces
  flat lock directories as source of truth.

### Desktop UI requirements

The desktop should make the manager state visible without requiring shell
reconstruction:

- Top-level counters: `Coding`, `Pylons`, `Executing`, `Assigned`, `Rejected`,
  `Tokens counted`, and `Token failures`.
- Per-account cards: account ref, auth state, provider readiness, current
  concurrency, active assignments, cooldown/reset time, weekly status, and last
  successful turn.
- Queue view: current priority lane, candidates, claimed-by, lock age, latest
  GitHub state, and reason for skipping.
- Worker detail view: assignment ref, PR/issue, prompt summary, account, PID,
  live event stream, tool calls, final closeout, GitHub action taken, and token
  rows.
- Reconciliation panel: unposted usage reports, replay button, D1/API proof,
  and projection delta.

### Rate limit and reset behavior

The desktop harness must distinguish at least these states:

- ready
- missing credentials
- auth refresh failed
- five-hour cooldown active, with `reset_at`
- weekly usage exhausted, with `reset_at`
- model unavailable
- execution refused with unknown provider detail

The user wants ORCA-like rate-limit visibility and reset controls. Implement
this as a typed operator action, not a blind retry loop:

1. Read provider-reported status and reset times.
2. If the account only hit a short five-hour cooldown and has weekly capacity
   left, do not spend a reset action. Schedule automatic resume at `reset_at`.
3. If weekly usage is exhausted and a legitimate provider-supported manual reset
   or operator reset workflow exists, expose a deliberate `Request reset`
   action, record the reason, and log the result.
4. If no supported reset exists, show that clearly and keep the account out of
   rotation until the provider reset time.

Do not let the manager keep probing an exhausted account every few seconds. That
creates noise, rejections, and misleading "active" UI.

### Deterministic preflight before launching a worker

Before any new assignment:

1. Confirm exactly one controller is active.
2. Confirm PR/issue is still open and not already merged/closed.
3. Confirm no live accepted worker is already assigned to that PR/issue.
4. Confirm account is ready and not cooling down.
5. Confirm advertised Pylon capacity minus active accepted assignments leaves a
   slot.
6. Confirm `origin/main` commit and verifier command are recorded.
7. Write a structured local `planned` row before calling Khala.

### Deterministic postflight after launching a worker

After dispatch:

1. Require `assignment_run.accepted` within a bounded timeout, or classify
   failed-before-accept.
2. Require a child process or event stream within a bounded timeout, or
   classify accepted-but-not-executing.
3. Poll progress freshness. Show stale age in the UI.
4. On closeout, classify accepted versus rejected from final lifecycle state.
5. Verify GitHub state changed as claimed by the worker.
6. Verify exact token usage is posted or replay the failure spool.
7. Mark the queue row done, retryable, blocked, or superseded with a typed
   reason.

## Manager Agent Protocol After Compaction

When a new manager agent picks this up:

1. Read this file and the 2026-06-29 after-action first.
2. Stop any shell refill parent if the user asked for a pause or reorientation.
   Leave in-flight Codex workers alone.
3. Run the active marker versus `codex exec` status command.
4. Check the token failure spool.
5. Query GitHub open/merged/closed counts.
6. Inspect the latest PR resolver logs by final lifecycle state.
7. Rebuild the candidate set from current GitHub state. Do not trust old
   temp-file candidates.
8. Resume with one controller only, or use the desktop harness once available.
9. Report exact state: assigned, executing, accepted, rejected, token failures,
   open PR count, and the current priority lane.

## What To Build Next

The immediate product direction is clear:

1. Merge or consolidate the PRs that expose Codex quota/cooldown/reset status,
   execution refusal reasons, account readiness, supervisor stale-claim GC, and
   live fleet progress.
2. Replace `/tmp/openagents_burst_pr_refill.sh` with a checked-in Pylon/Desktop
   harness that uses structured state and tests.
3. Move the queue, account status, dispatcher, process tracker, and token
   reconciler into OpenAgents Desktop.
4. Make the desktop UI the operator source of truth for Codex fleet status and
   work assignment, while keeping Codex itself as the local coding engine.

The goal is not just higher fanout. The goal is higher fanout with state that a
future manager agent can trust after compaction.
