# After-Action: Codex Fleet Throughput Collapse + Recovery — 2026-06-28/29

## ⇨ HANDOFF: do this next, in priority order

The single proven fact: **dispatch (create lease) and execute (`assignment
run-no-spend`) are separate; the executor was missing.** Firing 12 concurrent
runners took codex 1→9 instantly. There is NO hardware/account ceiling at ~5 —
that earlier conclusion was wrong.

1. **Build a standing RUNNER POOL (highest leverage).** A supervised service that
   keeps M (start 12-20) concurrent `bun apps/pylon/src/index.ts assignment
   run-no-spend` workers alive (respawn on exit), using the **standing pylon's own
   token** (`grep oa_agent_ ~/.pylon-fable/bin/standing-pylon.sh`), with
   `OPENAI_API_KEY`/`CODEX_API_KEY` UNSET. It only EXECUTES existing leases — it
   does not dispatch — so it never contends (no 409). This is the durable version
   of the ad-hoc loop that worked. Make it part of the supervisor or a sibling
   `apps/pylon/scripts/codex-supervisor/runner-pool.sh`.
2. **Do not "recover" server already-claimed accepts as success.** On this follow
   up, a 20-slot runner pool reached `codex_exec=21`, but 20 of the workers were
   duplicated across only two leases because `acceptAssignment` treated server
   `409 pylon_api_conflict: Pylon assignment was already claimed` as accepted.
   That behavior is wrong unless the runner can prove it owns the claim locally.
   The local fix makes this `denial.assignment.duplicate_lease`, with a regression
   test in `apps/pylon/src/assignment.test.ts`.
3. **Token accounting must be fail-soft but never silent.** One completed Codex
   turn produced `353,121` output tokens but failed `/api/pylon/codex/turns`
   with HTTP 400 because the reporter sent the full raw event stream and a
   multi-MB final message through the turn-ingest route. That route has an 8 MiB
   request cap and 64 KiB item text bounds. The fix is: turn ingest carries refs,
   bounded item summaries, and usage only; raw events go through event chunks;
   local failure spools are compact and replayable. The missed row was replayed
   and `/api/public/khala-tokens-served` moved by exactly `+353,121`.
   Later in the same recovery, the bigger accounting gap was cached-input and
   interrupted-rollout usage. Reconcile local per-account Codex rollout JSONL
   against D1 before declaring the counter correct, and do not choose too-narrow
   a time window. The verified post-backfill state for the 2026-06-29 burst is
   `59` `pylon-codex-own-capacity` D1 rows, `305,251,817` total tokens from
   `2026-06-29T05:28:51.104Z` through `2026-06-29T06:27:58.820Z`, exactly
   matching local pylon-account rollout totals. A later burst exposed another
   bypass: Codex PR title/body generation after a successful coding turn creates
   a second small pylon-account rollout. Count those too. The forward fix reports
   that helper turn as turn index 2 on the same assignment; four pre-fix helper
   rows were manually backfilled.
4. **Fix the supervisor backoff (#6987):** treat transient `503`/`500`/`409`
   (D1 read flakes) as fast-retry (≤2s), NOT 15-300s backoff. The backoff on
   transient flakes is what collapses concurrency. Also: stale-lease closeout on
   startup, claim GC, never claim epics/standing-tasks.
5. **Retry transient assignment accepts:** the local runner now retries transient
   accept failures such as `pylon_api_storage_error`, `D1 DB is overloaded`, and
   HTTP `500`/`503`/`504` a few times with a short delay. This is the local patch
   that made the manual stagger reusable instead of losing workers to one D1 blip.
6. **Server-side gate D1-read resilience:** the gate's "linked owner registration
   read" + "linked Pylon capacity read" 503/500 intermittently — add retry/cache
   so a D1 blip returns valid capacity instead of failing dispatch.
7. **Token discipline:** the supervisor/runners MUST use the pylon's own token
   (the one that publishes presence). A mismatch → "heartbeat stale".
8. **Deploy the Artanis fix** (`2d46d808`, fail-soft operator chat) — needs a prod
   Worker deploy to take effect.
9. **Bound the Vertex/Khala burn** so it can never overload D1 and starve codex
   dispatch (codex ≈90% of tokens, burn ≈1.6%). My 16-burn "max burn" took codex
   down — do not repeat.
10. **Offload to more machines** (`archlinux` 100.108.56.85, `imac-pro-bertha`
   100.97.233.57 — both online on the Tailnet; reach via Tailscale SSH, run codex
   with `bash -ic`). One Mac + 5 accounts is not the path to tens; runner pools on
   multiple machines is.

**Do NOT:** reflex-restart the supervisor (stale 5-min leases poison the gate);
fire large one-off dispatch batches (herd fills the gate with unrun leases);
run unbounded Vertex burns (D1 overload). Watch `clients/openagents-desktop`
(the live fleet dashboard) for ground truth.

---

## ⇨ ADDENDUM: 2026-06-29 local follow-up findings (Codex)

This addendum records the follow-up session after the first collapse audit. It is
written for future agents resuming after compaction, especially while the owner
is asking for maximum local fanout on this Mac.

### Token accounting reconciliation: do not miss aborted rollout files

At `2026-06-29T06:50Z`, the local pylon-account Codex rollouts were reconciled
against production D1 after the owner noticed the public counter was not moving
enough for the 17-session burst.

The important distinction:

- The normal completed-turn route had reported only the human CLI footer number
  for some runs. That number was effectively "uncached input + output", so the
  large `cached_input_tokens` component was missing.
- Several per-account Codex rollout files had cumulative `token_count` events
  but no final `task_complete` event. Those were interrupted/aborted worker
  sessions. They still burned tokens and must count.

Verified local/D1 reconciliation:

```text
local pylon-account rollout files from 2026-06-29T05:28:51Z:
  sessions: 42
  total_tokens: 305,251,817

first backfill:
  17 cached-input correction rows
  +143,127,808 tokens
  original+correction total: 147,588,905

second backfill:
  21 orphan/interrupted rollout rows
  +157,599,111 tokens

third backfill:
  4 short rollout rows that were missed by the original 05:30Z cutoff
  +63,801 tokens

final production D1 aggregate:
  rows: 59
  input_tokens: 300,392,192
  output_tokens: 4,859,625
  reasoning_tokens: 51,614
  cache_read_tokens: 295,080,832
  total_tokens: 305,251,817
  first_observed: 2026-06-29T05:28:51.104Z
  latest_observed: 2026-06-29T06:27:58.820Z
```

Public counter after the backfill:

```text
/api/public/khala-tokens-served -> 4,296,902,514
generatedAt: 2026-06-29T07:00:20.292Z
```

### Second reconciliation: PR title/body helper turns count too

At `2026-06-29T07:52Z`, after the next local burst drained, every token-bearing
Codex rollout under `~/.pylon-fable/accounts/codex/*/sessions/2026/06/29/` from
the `07:25Z` window was reconciled against production D1.

The new failure mode:

- The main coding turn reported through `/api/pylon/codex/turns` correctly.
- After a verified diff, `createOwnCapacityTitleBodyGenerator` launched a second
  read-only `runCodexComposerStream` turn to write the GitHub PR title/body.
- That helper turn used a pylon Codex account and produced a normal rollout
  JSONL token_count, but it bypassed the Codex turn reporter before the local
  fix.
- The missed rows were small (~22k tokens each), but the rule is exact: every
  pylon-account Codex rollout with token_count belongs in `token_usage_events`.

Forward fix:

- `apps/pylon/src/codex-agent-executor.ts` now passes the existing
  `CodexTurnReporter` into the PR title/body generator.
- The helper turn reports as `turnIndex: 2` on the same assignment/run/workspace
  with a distinct `session.pylon.codex_composer.*` ref.
- Focused tests passed:
  `bun test apps/pylon/tests/codex-agent-executor.test.ts apps/pylon/tests/codex-composer.test.ts`.

Manual reconciliation performed for pre-fix helper sessions:

```text
session.pylon.codex_composer.45c7481ac5720d564f41c05e  #6829  22,308
session.pylon.codex_composer.41b1217b701e5c93f6c00588  #6892  22,098
session.pylon.codex_composer.532726edd384c15288cfdbfd  #6902  22,655
session.pylon.codex_composer.69c11f69f518761fc66dd74e  #6796  22,562
```

Verified exact match for this later burst:

```text
local pylon-account rollout rows from 2026-06-29T07:25:00Z:
  rows: 14
  total_tokens: 72,034,415

production D1 pylon-codex-own-capacity rows in the same window:
  rows: 14
  total_tokens: 72,034,415
  latest_observed: 2026-06-29T07:50:45.744Z

/api/public/khala-tokens-served:
  tokensServed: 4,485,066,347
  generatedAt: 2026-06-29T07:52:24.010Z
```

The same `07:25Z` burst later had three more local sessions finish. Do not call
the window reconciled while Codex children are still alive: in-flight rollout
`token_count` entries are partial and the normal reporter writes the final row
only after process exit. A foreground repair loop watched the last live session,
waited for the active marker to disappear and `task_complete` to appear, then
queried D1 by deterministic `session.pylon.codex_composer.*` before doing any
manual repair. No repair was needed; the normal closeout reporter inserted both
remaining rows.

Final verified state for that expanded window:

```text
local pylon-account rollout rows from 2026-06-29T07:25:00Z:
  rows: 18
  total_tokens: 98,031,746

production D1 pylon-codex-own-capacity rows in the same window:
  rows: 18
  total_tokens: 98,031,746
  latest_observed: 2026-06-29T08:11:57.791Z

/api/public/khala-tokens-served:
  tokensServed: 4,511,124,936
  generatedAt: 2026-06-29T08:13:03.200Z
```

If a future active turn appears "missing" from D1, first prove it is no longer
running. Then prefer a repair POST to `/api/pylon/codex/turns` with the same
`pylon.codex.turn.<pylonRef>.<assignmentRef>.<sessionRef>.1` idempotency key the
normal reporter would use. Never backfill a still-running turn from a partial
rollout snapshot; the final closeout may contain more tokens and would then be
idempotently suppressed.

When reconciling future bursts, do not filter only for long coding sessions.
Scan all pylon-account Codex rollout JSONL files in the window, including short
PR metadata helpers and aborted/refused sessions with token_count events.

How to check this without guessing:

```sh
bunx wrangler d1 execute openagents-autopilot --remote --json --command \
  "SELECT COUNT(*) AS rows,
          COALESCE(SUM(input_tokens),0) AS input_tokens,
          COALESCE(SUM(output_tokens),0) AS output_tokens,
          COALESCE(SUM(reasoning_tokens),0) AS reasoning_tokens,
          COALESCE(SUM(cache_read_tokens),0) AS cache_read_tokens,
          COALESCE(SUM(total_tokens),0) AS total_tokens,
          MAX(ingested_at) AS latest_ingested,
          MAX(observed_at) AS latest_observed
     FROM token_usage_events
    WHERE provider='pylon-codex-own-capacity'
      AND observed_at >= '2026-06-29T05:20:00.000Z';"
```

The original reconciliation window started at `05:30Z` and missed four short
worker sessions at `05:28:51Z`, `05:28:56Z`, `05:29:00Z`, and `05:29:04Z`.
Future agents should first list the local rollout files and use the earliest
local `token_count` timestamp as the D1 query lower bound.

Schema footgun: the D1 column is `reasoning_tokens`, not
`reasoning_output_tokens`.

Backfill rule:

- Prefer the normal `/api/pylon/codex/turns` route when there is a valid owned
  assignment; it validates assignment ownership and writes trace/private refs.
- If the rollout is orphaned or interrupted and the normal route cannot pass
  `requireOwnedAssignment`, insert one idempotent `token_usage_events` row per
  local pylon-account rollout using `usageBasis:
  codex_rollout_token_count_backfill`.
- Do not include local file paths, raw prompts, stdout, provider payloads, or
  auth material in `safe_metadata_json`. The backfill rows used only pylon ref,
  Codex account alias, rollout session id, public-safe workspace ref, usage
  split, and reconciliation metadata.
- Exclude the operator's default `~/.codex` session unless it was actually run
  by Pylon. The trusted worker source for this reconciliation is
  `~/.pylon-fable/accounts/codex/*/sessions/YYYY/MM/DD/*.jsonl`.

Forward fix made locally:

- `apps/pylon/src/codex-composer.ts` now reads cumulative Codex rollout usage
  from JSONL and splits output vs reasoning.
- `apps/pylon/src/codex-agent-executor.ts` now reports the normalized CLI usage.
- The CLI failure/timeout path now remembers the raw Codex thread id, re-reads
  the rollout file, and attempts a failed-turn usage report before returning a
  budget/refusal outcome. If the turn ingest rejects, the compact local failure
  spool remains the fallback.

Focused verification passed:

```sh
bun test apps/pylon/tests/codex-composer.test.ts \
  apps/pylon/tests/codex-agent-executor.test.ts \
  apps/pylon/tests/codex-turn-reporter.test.ts
```

Result: `46 pass`, `0 fail`.

### Current live snapshot at addendum time

At about `2026-06-29T05:06Z`, after several direct-fire experiments:

- filtered `codex_exec=0`;
- filtered `assignment run-no-spend=0`;
- `codex-supervisor` was not running;
- generic chat-completion stress burn was stopped (`khala-burn-sustained` and
  `oa-sustained-burn/1.0` process count zero), though one unrelated
  `/api/v1/chat/completions` curl may still appear in process counts;
- the public token counter had advanced to about `3,991,318,510`, so some token
  reporting was still flowing, but not enough local Codex was alive.

Use this as a diagnosis baseline, not as a success state.

### Latest local fanout snapshot: high count, bad spread

At about `2026-06-29T05:45Z`, after switching the local runner pool to
`RUNNER_COUNT=20`, `OPENAGENTS_PYLON_CODEX_CONCURRENCY=25`,
`OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5`, and
`OPENAGENTS_PYLON_CODEX_AGENT_RUNNER=cli`, the sanitized process count was:

- filtered `assignment run-no-spend=22`;
- filtered `codex_exec=21`;
- public token counter around `3,991,474,410`.

That looked like success until the active-run marker spread was checked:

```text
active_marker_files 21 unique_assignments 3
11 assignment.public.khala_coding.chatcmpl_d6b44df5f5c44a9ca116e8948a6ce350
 9 assignment.public.khala_coding.chatcmpl_601cdbabdaba452f94d418f05c2d58bd
 1 assignment.public.khala_coding.chatcmpl_01bdcd279d8e438da3ed911a75a473bb
```

Root cause: `apps/pylon/src/assignment.ts` treated server
`409 pylon_api_conflict` with reason `Pylon assignment was already claimed` as a
successful accept by synthesizing
`assignment.accepted.already_claimed.<lease>`. With a 20-slot pool, that caused
many independent workers to run the same already-claimed lease and materialize
the same workspaces. It is a token burn, but it is not useful fleet fanout.

Fix made in the local checkout: server already-claimed conflicts now return
`denial.assignment.duplicate_lease` by default, preserving the server error for
diagnosis. The focused regression is
`apps/pylon/src/assignment.test.ts`, and this passed:

```sh
bun test apps/pylon/src/assignment.test.ts \
  apps/pylon/src/khala-burndown.test.ts \
  apps/pylon/src/khala-spawn.test.ts \
  apps/pylon/src/presence-codex-account-capacity.test.ts
```

Do **not** kill the already-running duplicate Codex children just to clean the
process table; let them close out. The fix applies when slots respawn and try to
claim new work. If future idempotent recovery is needed, it must first prove the
local store already has the same lease in `accepted`/`running` state under this
process/node, not merely see the server's "already claimed" text.

### Successful local fanout snapshot: non-targeted request plus staggered runners

At about `2026-06-29T07:10Z`, this Mac reached the owner's requested local
upper-bound probe:

```text
filtered assignment run-no-spend=36
filtered codex_exec=20
active_marker_files=20
unique active assignments=20
```

The path that worked:

1. Keep the standing runner pool alive, but do not rely on it to claim every new
   lease promptly.
2. Dispatch new work with `khala request --workflow codex_agent_task ... --no-run`
   and **do not pass `--pylon-ref`** while targeted gating is flaky. The
   non-targeted request path delegated to the linked pylon and returned
   `evidence.khala_coding.own_capacity_linked_pylon`.
3. Launch explicit runners for any unclaimed returned assignment refs:

   ```sh
   PYLON_HOME="$HOME/.pylon-fable" \
   PYLON_DISABLE_DAEMON_ROUTING=1 \
   PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
   OPENAGENTS_AGENT_TOKEN="$(tr -d '\n' < "$HOME/.pylon-fable/auth/openagents-agent-token")" \
   OPENAGENTS_PYLON_CODEX_AGENT_RUNNER=cli \
   OPENAGENTS_PYLON_CODEX_CONCURRENCY=25 \
   OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5 \
   PYLON_MIN_ASSIGNMENT_LEASE_TIME_REMAINING_MS=5000 \
   bun apps/pylon/src/index.ts assignment run-no-spend \
     --assignment-ref "$ASSIGNMENT_REF" \
     --json
   ```

4. Stagger accepts by about 2 seconds when D1 is hot. A simultaneous accept burst
   produced transient `pylon_api_storage_error` / `D1 DB is overloaded. Requests
   queued for too long.` denials; staggered retries succeeded seconds later.

Fresh issue leases started in this successful tranche included #7030, #7029,
#7028, #7026, #7025, #7024, #7022, #7021, #7019, #7017, #7016, #7015,
#7012, #7011, #7010, #7009, #6987, #6978, and #6973, plus one more from the
active queue. Issue #7013 had one request failure and can be retried later.

Accounting check while the tranche drained: completed rollout files since
`2026-06-29T07:00:00Z` were reconciled to production D1. After normal reporter
rows plus idempotent local-rollout backfills for tiny completed rows that missed
normal closeout, the final check showed `codex_exec=0`, no pending report
failure spool, and D1 matched local completed usage exactly:

```text
local_completed: 35
d1_rows: 35
total_tokens: 115,960,734
missing: 0
public /api/public/khala-tokens-served: 4,412,966,410
generatedAt: 2026-06-29T07:26:29.709Z
```

Local fix made after observing the D1 overload: `acceptAssignment` now retries
transient accept failures (`pylon_api_storage_error`, D1 overload, HTTP
`500`/`503`/`504`) a bounded number of times with a short delay, while preserving
`Pylon assignment was already claimed` as `denial.assignment.duplicate_lease`.
Focused verification:

```sh
bun test apps/pylon/src/assignment.test.ts
```

Result: `2 pass`, `0 fail`.

### Latest upper-bound probe: 20 live Codex children held, 25 did not

At `2026-06-29T08:15Z`, another local burn wave was launched after confirming the
previous wave's tokens matched D1 exactly. The dispatch pattern was:

- first tranche: request 20 public issue assignments with `khala request
  --workflow codex_agent_task --repo OpenAgentsInc/openagents --commit
  02506a04e2ed212c6af021a5a5f075e120e65f8e --verify true --no-run`;
- exact runner launched immediately for every returned assignment ref;
- six initial request attempts returned capacity `503`, then all 11 retry/top-up
  requests were accepted after another heartbeat;
- a final five-issue top-up accepted three more refs but those three refused
  immediately.

The stable live state after the retry wave:

```text
codex_exec=20
active_marker_files=20
```

The 20 live children covered 20 unique assignment refs. Dispatch/evidence
directories for this probe:

```text
~/.codex-supervisor/dispatch-20260629T0815
~/.codex-supervisor/exact-run-20260629T0815
~/.codex-supervisor/dispatch-20260629T0817
~/.codex-supervisor/exact-run-20260629T0817
~/.codex-supervisor/dispatch-20260629T0819
~/.codex-supervisor/exact-run-20260629T0819
```

Important finding: the extra accepted top-up assignments for #6822, #6823,
#6824, #6846, #6637, and #6707 closed out almost immediately with
`blocker.assignment.codex_agent_execution_refused`. They are useful as evidence
that the machine can accept more leases, but not useful as sustained token burn.
When probing above 20, use non-epic, non-standing issues with clear bounded
implementation scope; broad epic/supervisor/standing tasks are more likely to
refuse before burning meaningful tokens.

Also, capacity projection still lies in two ways:

- `provider go-online` may show top-level `codingCapacity.available=3` /
  `busy=22` while `ownCapacityDispatch.totalAvailableCodexAssignments` reports
  much higher availability from per-account load. The request path did still
  accept enough work to reach 20, so do not trust the top-level scalar alone.
- Local active marker spread exceeded the intended account cap on one account
  (`account.pylon.codex.d4d6...` had 10 active markers despite
  `OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5`). This is either stale-marker
  contamination or an account-selection/load-enforcement bug. Future durable
  runner work must enforce per-account caps from live process/marker ownership,
  not from stale aggregate presence alone.

Ground rule for the next operator: while 20 children are alive, do not kill or
restart them for cleanliness. Let them close out, then run the rollout-vs-D1
reconciliation before declaring the counter correct.

### The D1 dashboard was this project

The Cloudflare dashboard the owner showed was for D1 database
`openagents-autopilot`, id `9644ea09-f682-4971-98de-e0c791cb67fb`. That exact id
is the production `OPENAGENTS_DB` binding in
`apps/openagents.com/workers/api/wrangler.jsonc`. So the dashboard spike was not
some unrelated Cloudflare project: it was the same D1 that backs Khala token
usage, public stats, Pylon presence, assignment dispatch, assignment progress,
and closeout paths.

### Generic Khala burn was a real D1 load source

A workspace-level stress script was running for hours:

```sh
/Users/christopherdavid/work/scripts/khala-burn-sustained.sh 16 2000
```

It loops `curl` calls to `/api/v1/chat/completions` with user-agent
`oa-sustained-burn/1.0`, spreading over keys from
`/Users/christopherdavid/work/.secrets/khala-heartbeat.env`. This is not Codex
assignment fanout; it is generic Khala completion burn. Each successful
completion records a `token_usage_events` row, and the public stats/counter
routes read that same ledger. At 16 workers with 2000 max output tokens it can
saturate D1 and starve assignment accept/progress/closeout.

It was stopped with:

```sh
pkill -f 'scripts/khala-burn-sustained.sh' || true
pkill -f 'oa-sustained-burn/1.0' || true
```

Do not restart unbounded generic burn while Codex is underfilled. If a stress
burn is needed, cap it and back it off when assignment accept/progress sees D1
overload.

### Public stats need a small cache, honestly declared

The owner explicitly allowed a 2 second cache. The previous state was worse than
it looked: `/api/public/khala-tokens-served/history` already had a 4 second
in-isolate cache while still declaring `maxStalenessSeconds:0`. The correct
pattern is to cache the hot public token-stat reads briefly and tell the truth in
the staleness contract.

Patch in progress in the local checkout:

- `public-khala-tokens-served-routes.ts`: add a 2s in-isolate scalar cache.
- `public-khala-tokens-served-history-routes.ts`: reduce existing 4s cache to 2s.
- `public-khala-tokens-served-model-mix-routes.ts`: add a 2s cache.
- `public-khala-tokens-served-demand-mix-routes.ts`: add a 2s cache.
- `public-projection-staleness.ts`: add `cachedLiveAtReadStaleness(...)`.
- product-promise copy/tests updated from `maxStalenessSeconds:0` to `2` for the
  model-mix/public-stats claim.

This still needs proper Worker test execution and deployment before it affects
production. A plain root `bun test ...public-khala...` command failed because
the test harness hit the known `cloudflare:workers` module import issue via
`effect-cf`; product-promises and the staleness unit tests passed. Prefer the
Worker package test command, for example:

```sh
bun run --cwd apps/openagents.com/workers/api test -- \
  src/public-khala-tokens-served-routes.test.ts \
  src/public-khala-tokens-served-history-routes.test.ts \
  src/public-khala-tokens-served-model-mix-routes.test.ts \
  src/public-khala-tokens-served-demand-mix-routes.test.ts \
  src/product-promises.test.ts
```

Do not run the full prepush/deploy gates unless the owner explicitly allows it.

### The direct-fire SIGHUP trap

A major follow-up footgun: launching many `pylon khala request ... &` jobs from a
short-lived `bash -lc` shell is not a durable runner pool. The shell exits, the
background jobs can receive SIGHUP, and logs may stop immediately after:

```text
assignment_run.accepted
assignment_run.runtime_started
```

This creates the most confusing failure mode:

- server leases exist;
- local workspaces appear under `~/.pylon-fable/cache/codex-agent-tasks/`;
- local active-run marker files appear under
  `~/.pylon-fable/active-assignment-runs/`;
- `codex_exec` is zero;
- per-account capacity is falsely consumed because the stale local active-run
  files project `busy=N`;
- logs may be empty or end before a Codex child is sampled.

Do not interpret `runtime_started` as proof that a durable Codex turn survived.
Always confirm a matching live `codex exec` child or a later progress/closeout
event.

Durable launch patterns:

```sh
nohup bash -lc 'set -euo pipefail; cd /Users/christopherdavid/work/openagents; ...; bun apps/pylon/src/index.ts assignment run-no-spend --assignment-ref <ref> --account-ref <account> --json' \
  > "$HOME/.codex-supervisor/nohup-exact/<name>.log" 2>&1 &
disown || true
```

or run a supervised foreground loop that remains alive. Do not use plain `&`
under a short Codex `exec_command` shell for long Codex turns.

### Stale active-run markers are local, but dangerous

The active local marker files are at:

```text
~/.pylon-fable/active-assignment-runs/*.json
```

They are local load projection, not proof of a live Codex process. If a direct
runner is SIGHUP'd after `runtime_started`, these files can remain and advertise
false `busy` capacity to the server.

Only move/delete them after proving there are no live `codex exec` children for
the corresponding refs. Safe check:

```sh
ps -axo command | awk 'BEGIN{IGNORECASE=1} /codex\/vendor.*bin\/codex exec|(^| )codex exec( |$)/ && $0 !~ /awk|rg -i|ps -axo|codex exec\|/ {c++} END{print "codex_exec=" c+0}'
```

In the follow-up session, `codex_exec=0`, so four stale markers were moved aside
to a timestamped directory:

```text
~/.pylon-fable/stale-active-assignment-runs-<timestamp>/
```

Do not do this when a real Codex child is alive; it will make server-visible load
lie in the opposite direction.

### Per-account capacity is a separate knob

Publishing `OPENAGENTS_PYLON_CODEX_CONCURRENCY=20` is not enough if per-account
capacity still resolves to one slot. The relevant Pylon code has
`OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY` as the explicit per-account override.
When testing upper bounds on this Mac, export both before heartbeat and before
launching the supervisor:

```sh
export OPENAGENTS_PYLON_CODEX_CONCURRENCY=20
export OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=4
```

Then verify the heartbeat response, not just the local env. Look for
`ownCapacityDispatch.codex.codexAccounts[*].available` and
`capacity.coding.codex.account.<hash>.available=N`. If these still show `1` per
account, the gate will not admit 20 no matter how many local runner processes
exist.

### Codex-6 remains suspect

`codex-6` repeatedly produced execution-refusal/no-child behavior while other
accounts could at least materialize workspaces. Avoid `codex-6` during emergency
fanout until issue #6902 or a direct account health check proves it is clean.
Known-good targets from the follow-up were `codex-3`, `codex-4`, `codex-5`, and
`codex-7`, but even those need durable runners rather than SIGHUP-prone shells.

### Process matching must be sanitized

Do not print full process command lines that may include `Authorization: Bearer`
or `oa_agent_...` tokens. Count processes with filtered `awk` commands and redact
if tailing logs. The useful counts are:

```sh
ps -axo command | awk 'BEGIN{IGNORECASE=1} /codex\/vendor.*bin\/codex exec|(^| )codex exec( |$)/ && $0 !~ /awk|rg -i|ps -axo|codex exec\|/ {c++} END{print "codex_exec=" c+0}'
ps -axo command | awk 'BEGIN{IGNORECASE=1} /assignment run-no-spend/ && $0 !~ /awk|rg -i|ps -axo|live burn session|executor_slot|assignment run-no-spend\|/ {c++} END{print "run_no_spend=" c+0}'
ps -axo command | awk 'BEGIN{IGNORECASE=1} /curl .*api\/v1\/chat\/completions|oa-sustained-burn|khala-burn-sustained/ && $0 !~ /awk|ps -axo/ {c++} END{print "chat_completion_curls=" c+0}'
```

### Updated immediate next moves

1. Finish and test the 2 second public token-stats cache, then deploy only via
   the sanctioned Worker deploy path when the owner allows deploy gates.
2. Build the durable runner pool as a real script/service. It must use
   `nohup`/`caffeinate`/launchd or stay foreground-supervised; it must not rely
   on short-lived shell background jobs.
3. Launch a small durable exact-runner set first (4-6), confirm live `codex exec`
   children, then raise toward 12-20. Do not fire a 20-job batch until the first
   set survives past `runtime_started`.
4. Keep generic completion burn off until Codex is saturated and D1 is healthy.
5. Use the desktop Coding view as the scoreboard, but cross-check with sanitized
   local counts because stale active-run marker files can make UI capacity lie.

---


Author: Claude-main (overseer). Window: ~evening of 2026-06-28 CT. Repo:
`OpenAgentsInc/openagents`. Honest, no-theater account of a throughput collapse
that was **mostly self-inflicted by the overseer (me)**, the real root causes,
the fixes landed, and how to unfuck it durably.

## TL;DR

- Goal was max token burn. The token engine is **Pylon-Codex (~90% of network
  tokens historically; 1.9B/day peak)**. It collapsed to ~0-6 concurrent for
  hours.
- **The collapse was largely my own doing**: ~15 supervisor restarts, over-firing
  hundreds of one-off dispatches, and running 16 parallel Vertex burns that
  **overloaded the shared D1/Khala backend** — which is the SAME backend the codex
  dispatch gate reads. That cascaded into 503/500/401 dispatch failures.
- **The dispatch refusals were NOT a hard capacity ceiling** (the owner correctly
  insisted — tens of instances ran the day before). They were: (a) stale leases
  from my restarts, (b) a supervisor↔pylon **token mismatch**, (c) **intermittent
  server-side D1 read failures** on the gate, amplified by (d) an aggressive
  15-300s backoff that idles a slot on every transient flake.
- Real fixes landed; codex recovered 0→~6. Getting back to tens needs the
  **backoff/retry fix (#6987)** + D1-read resilience + not overloading D1.

## What actually broke (root causes, in order of discovery)

1. **Stale leases → `dispatch_gate_blocked`.** The gate counts active leases per
   account within a **5-minute TTL** (`pylon-api-routes.ts` `PYLON_ASSIGNMENT_ACTIVE_LEASE_TTL_MS`).
   Every supervisor restart killed in-flight assignments, leaving lease rows that
   the gate counted for 5 min → refused new dispatches. My ~15 restarts kept the
   gate full of ghosts. **Fixed:** TTL cut 5min→90s, deployed (`b06227b021`).
2. **Supervisor↔pylon token mismatch → "heartbeat stale or missing".** The
   standing pylon publishes presence under token `…iob7JuM`; the supervisor was
   dispatching with the artanis token `…F4dlJPs`. The gate scopes the heartbeat to
   the owning token, so dispatch saw "no fresh heartbeat." **Fixed:** run the
   supervisor with the pylon's own token.
3. **Vertex burns overloaded D1/Khala → cascading 503/500/401.** I scaled to 16
   parallel Khala-routed Vertex burns ("max burn"). They went idle (0% CPU per the
   dashboard) but kept hammering Khala with failing requests, overloading the
   shared D1. That broke the codex dispatch gate's reads
   (`503 "could not read linked owner registration"`, `503 "linked Pylon capacity"`,
   `500 internal_server_error`, and even `401 unauthorized` on presence). **This
   was the biggest self-inflicted failure.** **Fixed (mitigated):** killed all
   burns → presence + dispatch immediately recovered.
4. **Per-worktree `bun install` capped concurrency.** Tasks DO use git worktrees
   off a shared bare store (efficient), but worktrees don't share `node_modules`,
   so each ran a fresh monorepo `bun install` → N concurrent installs thrash disk
   → serialize. **Fixed:** lockfile-keyed shared `node_modules` cache, symlinked
   into each worktree (`#6992`, `codex-agent-executor.ts`).
5. **Intermittent D1 read flakiness + aggressive backoff = low concurrency.** Even
   after 1-4, direct dispatches intermittently return `503 "could not read linked
   owner registration"` / `500`. These are transient D1 read failures. The
   supervisor treats any rc≠0 dispatch as `NO-DISPATCH` and backs the slot off
   **15→300s**, so a brief D1 flake idles a slot for minutes → effective
   concurrency collapses. **NOT fixed yet — this is the key remaining bug (#6987).**

## My fuckups (own them)

- **Thrashing.** ~15 supervisor restarts. Each created stale leases that poisoned
  the gate for 5 min. Restarting was my reflex and it actively made things worse.
- **Over-firing.** Fired 30+ one-off dispatches repeatedly (300+ khala-request
  procs at peak) — a thundering herd that filled the gate with accepted-but-not-
  executing leases and blocked the supervisor.
- **"Max burn" that sabotaged the main engine.** 16 Vertex burns overloaded the
  shared D1 and took down codex dispatch entirely. The burns are ~1.6% of tokens;
  codex is ~90%. I starved the 90% engine to feed the 1.6% one.
- **Repeated misdiagnosis stated as fact.** I called it a "global serving ceiling
  ~30M/hr," then a "hard ~5/account gate cap," then "full git clones," then a
  "machine execution ceiling ~5." All wrong or partial. The owner repeatedly had
  to correct me ("we ran tens yesterday"). I should have read the exact error
  every time before concluding.
- **"Wait 5 minutes."** Told the owner to wait for a TTL to drain instead of
  editing code. Not acceptable — the fix was a code/config change.
- **Token/PYLON_HOME confusion.** Gave re-auth commands without `PYLON_HOME` (went
  to the wrong home) and didn't reconcile the supervisor/pylon tokens for far too
  long.

## What I learned

- **The dashboard (`clients/openagents-desktop`, #6932 + #6958) is ground truth.**
  It surfaced "presence 401", "No dispatch", "Claims 25 > Desired 12", idle burns
  — exactly the diagnosis. Build/trust observability before theorizing.
- **Codex dispatch and the Khala chat/Vertex burns share one D1/Khala backend.**
  Hammering one starves the other. Burn budget must be bounded so it never
  degrades codex dispatch.
- **Restarts are not free.** Each leaves 5-min stale leases. Prefer in-place
  recovery (stale-lease closeout) over restart.
- **Read the exact error, never infer a "ceiling."** Direct dispatch succeeded
  while the supervisor reported NO-DISPATCH — proof the gate was fine and the
  failure was elsewhere.

## Fixes landed

| Fix | Where | Status |
|---|---|---|
| Active-lease TTL 5min→90s | `pylon-api-routes.ts` (`b06227b021`) | deployed to prod |
| Shared node_modules across worktrees (skip per-task install) | `codex-agent-executor.ts` (#6992, `f19825e209`) | merged; effective on supervisor restart |
| Artanis operator chat fail-soft + real error surfaced | `artanis-operator-chat-routes.ts` (`2d46d808`) | merged; **needs deploy** |
| Supervisor on the pylon's own token | ops | applied |
| Burns OFF (D1 relief) | ops | applied — recovered presence/dispatch |

## How to unfuck it durably (action items)

1. **[P0] #6987 — supervisor refusal handling.** Treat transient `503`/`500`/`409`
   (D1 read flakes) as **fast-retry (≤2s), NOT 15-300s backoff**. A transient flake
   must not idle a slot for minutes. Also: submit stale-lease closeout on startup;
   claim GC; never claim epics/standing-tasks. **This is the single highest-leverage
   fix for getting back to tens of concurrent.**
2. **[P0] Server-side gate D1-read resilience.** The gate's "linked owner
   registration read" and "linked Pylon capacity read" should retry/cache so a D1
   blip returns valid capacity instead of 503-ing the dispatch.
3. **[P1] Bound the Vertex/Khala burn** so it can never degrade codex dispatch
   (cap concurrent burn requests; back off when D1 latency rises). Codex (90%) >
   burn (1.6%).
4. **[P1] Deploy the Artanis fix** (`2d46d808`) so the operator chat works.
5. **[P2] Offload to more machines.** `archlinux` (and `imac-pro-bertha`) are online
   on the Tailnet. Real horizontal scale = run codex workers there, each with its
   own accounts — one Mac + 5 accounts is not the path to tens.
6. **Operating discipline:** don't restart the supervisor reflexively; don't fire
   large one-off batches; keep burns bounded; read the exact error before concluding.

## THE decisive finding — assignments need RUNNERS (this is the real unfuck)

After all the gate/token/lease fixes, codex was still stuck at ~1-2 while the
dashboard showed many Khala-request procs **IDLE (0% CPU)** and `Claims 38`,
`Issues 42`, but `CODEX EXEC 1`. The dispatches were **creating assignments**
(leases) but **nothing was executing them**, because:

- The **standing pylon's `assignment run-no-spend` loop was the executor**, and I
  **disabled it** earlier to stop the 409 contention with the supervisor.
- The supervisor's per-slot autoRun was LOCKOUT-ing on stale claims, so it wasn't
  running them either.
- So assignments piled up **created-but-unrun** → 1 codex exec.

**Proof + fix:** firing **12 concurrent `assignment run-no-spend` runners**
(`bun apps/pylon/src/index.ts assignment run-no-spend` with the pylon's token)
took **codex exec 1 → 9** in ~80s. Each runner picks the next pending lease and
runs its codex turn.

**So the model is:** dispatch (create lease) and execute (run-no-spend) are
SEPARATE. You need a **continuous pool of N concurrent runners** to keep N codex
turns executing — the supervisor's one-autoRun-per-slot is fragile (backoff on any
transient flake idles the slot). The robust pattern is a **standing runner pool**:
M concurrent `assignment run-no-spend` workers in a respawn loop, independent of
the dispatch/claim path. That is how you get to tens of concurrent codex.

### Corrected action items (supersede earlier "ceiling" conclusions)

1. **[P0] Standing runner pool.** Run M (e.g. 12-20) concurrent
   `assignment run-no-spend` workers in a supervised respawn loop, decoupled from
   dispatch. This is what was missing — NOT a hardware/account ceiling. (My
   earlier "~5 concurrent ceiling" conclusion was WRONG; with a runner pool codex
   went to 9 immediately and can go higher.)
2. **[P0] Re-add an executor loop to the standing pylon** (or a dedicated runner
   service) that doesn't contend with the supervisor (the 409 was supervisor↔
   standing-pylon both dispatching; a pure RUNNER that only executes existing
   leases doesn't dispatch, so no contention).
3. #6987 backoff fix, D1-read resilience, bounded burns, offload — still valid.

## State at write time

- **Codex recovered 0 → 9 concurrent** once I ran a concurrent runner pool — the
  real bottleneck was a MISSING EXECUTOR, not a ceiling. Burns OFF. node_modules-fix
  + TTL-fix live; Artanis fix merged (awaiting deploy). Next: make the runner pool
  continuous + standing, fix the supervisor backoff (#6987), add D1-read
  resilience, then offload to archlinux/bertha for true horizontal scale.

## Addendum: 2026-06-29 open-queue triage + immediate burn plan

Snapshot taken from live GitHub state after `main` was clean at
`b6ddbab062755737eacd8adbcc56c18bb1504b18`: 42 open issues and 7 open PRs. The
goal for the next two hours is not "perfect architecture first"; it is MAX Khala
token usage from Codex workers, while landing the architecture in parallel.

### Live action already started

At `2026-06-29T03:13Z`, start one live burn session from current `main` using the
standing Pylon token and `PYLON_HOME=$HOME/.pylon-fable`:

- 12 pure executor loops:
  `bun apps/pylon/src/index.ts assignment run-no-spend --json`
- 1 dispatcher:
  `bash apps/pylon/scripts/codex-supervisor/codex-supervisor.sh`
- unset `OPENAI_API_KEY` and `CODEX_API_KEY`
- overrides: `SUP_MAX_SLOTS=20`, `SUP_PER_ACCOUNT=5`,
  `SUP_BACKOFF_MIN=2`, `SUP_BACKOFF_MAX=30`,
  `SUP_FAILURE_BACKOFF_ESCALATE_THRESHOLD=99`

The first naive monitor printed `codex_exec=15`, but that was a bad process
match: the monitor command line itself contained the `codex exec` regex. A
filtered process check showed the real state:

- `actual_codex_exec=0`
- `actual_khala_request=0`
- `actual_run_no_spend=13`

The executor pool was alive, but the dispatcher was not creating work. The
immediate blocker was `codex-supervisor.sh: line 340: filtered[@]: unbound
variable` under `set -u` when the live-open-issue intersection was empty. Fix
that before trusting any saturation number.

Keep that session alive unless it is demonstrably hurting the fleet. Do not
reflex-restart it. Watch counts in the OpenAgents desktop Coding view and in
`$HOME/.codex-supervisor/live-burn.log`.

### Immediate operating loop

1. Keep a single dispatcher and many executors. Dispatch creates leases;
   `assignment run-no-spend` executes them. More executors are useful only when
   there are leases or stale leases to close out.
2. If `codex_exec` falls below 10 while open issues remain, first inspect exact
   recent failures in `live-burn.log`, `supervisor.log`, and the desktop Coding
   view. Do not infer a ceiling.
3. If runner loops are alive but `khala_request` is 0, fix/restart only the
   dispatcher path. Earlier direct execution proved the executor pool works.
4. If `khala_request` is high but `codex_exec` is low, the gate is accepting work
   that is not executing; increase pure `run-no-spend` loops or clear stale lease
   closeout, not one-off dispatch batches.
5. Keep Vertex/GLM/MirrorCode burn bounded until Codex stays saturated. Codex is
   still the main token engine.

### PR triage for max fanout

P0, review and merge first:

- [#6995](https://github.com/OpenAgentsInc/openagents/pull/6995)
  `feat(operator): add owner-scoped fleet state`. This is the "make it visible"
  PR for [#6958](https://github.com/OpenAgentsInc/openagents/issues/6958). It
  adds `/api/operator/fleet/state`, active assignment progress, supervisor slot
  state, and Codex account health/rate-limit details. After it lands, wire
  `clients/openagents-desktop` to this endpoint so the desktop shows both local
  process truth and server fleet truth.
- [#6994](https://github.com/OpenAgentsInc/openagents/pull/6994)
  `fix(operator): accept accountRefHash for account resets`. This unblocks part
  of [#6637](https://github.com/OpenAgentsInc/openagents/issues/6637), which is
  needed to know which accounts are limited and when resets are possible.

P0, build/land immediately if no PR exists:

- [#6987](https://github.com/OpenAgentsInc/openagents/issues/6987)
  supervisor stale-claim GC + fast retry. This is still the main fanout bug.
  Treat transient `503`/`500`/D1 read errors and contention-like `409`s as fast
  retry with jitter, not 15-300 second slot idles. Never claim epics or
  `standing-task` issues. On startup, GC claims that have no live assignment or
  active local run.
- Durable runner pool. There is still no open PR dedicated to the thing that
  actually moved `codex_exec` up: a supervised `assignment run-no-spend` pool.
  Make it a sibling to the supervisor, for example
  `apps/pylon/scripts/codex-supervisor/runner-pool.sh`, with launchd/systemd
  snippets and desktop-visible health.

P1, merge after the P0 path is stable:

- [#6993](https://github.com/OpenAgentsInc/openagents/pull/6993) for VMQ
  PR fast-forward planning. This reduces PR closeout drag once workers produce
  branches.
- [#6990](https://github.com/OpenAgentsInc/openagents/pull/6990) and
  [#6991](https://github.com/OpenAgentsInc/openagents/pull/6991) only after
  Codex remains saturated and D1 is healthy. They can add useful $0 burn, but
  unbounded burn already starved the Codex path once.

### Desktop visibility requirements

The desktop should be the scoreboard during this burn. Add or keep these as
top-screen counters:

- active `codex exec`
- active `assignment run-no-spend`
- active `khala request`
- supervisor desired slots
- open issue count
- runner pool configured/running/crashed
- stale claims and recent closeouts
- per-account ready/limited/revoked/rate-limited status
- last exact dispatch/execution failure reason

When #6995 lands, desktop should blend:

- local process truth from `clients/openagents-desktop/src/shared/coding-status.ts`
- server fleet truth from `/api/operator/fleet/state`
- Codex session messages from local rollout JSONL, clickable by active session

### Do not do during this two-hour burn

- Do not wait on docs before keeping workers busy.
- Do not run broad pre-push gates before delegating obvious open issues.
- Do not fire hundreds of one-off `khala request` processes.
- Do not run unbounded Vertex/GLM/MirrorCode burn while Codex is underfilled.
- Do not restart live Codex turns just to tidy the process table.
- Do not treat a stale local process list as the whole truth once the fleet
  state endpoint exists.

### Acceptance for the next handoff

- Sustained filtered `codex_exec >= 12` for at least 30 minutes, with no
  increasing stale claim count. The process matcher must exclude monitor shell
  command lines.
- Desktop shows active workers and lets the owner click each active Codex
  session to read messages.
- #6995 and #6994 are either merged or rejected with exact failing test output.
- #6987 has a PR, or the next agent has a precise failing command/log proving
  why it could not be started.
- Durable runner pool work is either landed or captured as the next P0 issue/PR
  with exact launch commands that reproduced the live `codex_exec` jump.

## 2026-06-29 08:45Z continuation: accounting and refill traps

The 08:15Z fanout wave did reach the high-water mark the owner wanted: 20 live
Codex sessions were observed briefly, then the fleet drained rapidly as many
issues finished or refused. Treat `active-assignment-runs/*.json` as the best
short-term local truth for live Codex runs; a simple `ps` matcher can undercount
while sessions are materializing or in closeout, and it can overcount if the
matcher sees its own command line.

### Token accounting check

For the 08:13Z+ burst, the correct reconciliation loop was:

```sh
cd apps/openagents.com/workers/api
bunx wrangler d1 execute openagents-autopilot --remote --json --command \
  "SELECT COUNT(*) AS rows, COALESCE(SUM(total_tokens),0) AS total_tokens, MAX(observed_at) AS latest_observed
     FROM token_usage_events
    WHERE provider='pylon-codex-own-capacity'
      AND observed_at >= '2026-06-29T08:13:00.000Z';"
```

and local rollouts under:

```text
~/.pylon-fable/accounts/codex/*/sessions/2026/06/29/*.jsonl
```

Use latest cumulative `payload.type == "token_count"` per rollout, not
per-message deltas. D1 rows are one row per reported Codex turn/composer turn,
with `provider='pylon-codex-own-capacity'`, `session_ref`, `run_ref`,
`task_ref`, and `total_tokens`.

Important: do not backfill while an active marker still exists. D1 can lead or
trail a naive local "task_complete" detector because the reporter may insert
the Codex turn row while the assignment process is still doing PR/closeout work.
During this continuation, D1 matched all closed-out work exactly at multiple
checkpoints:

- `08:22Z`: 11 rows / 19,479,682 tokens in local completed rollouts and D1.
- `08:29Z`: 28 rows / 74,749,124 tokens in local completed rollouts and D1.
- `08:35Z`: 30 rows / 91,242,784 tokens in local completed rollouts and D1.

Final checkpoint after the owner observed the burst had drained: local active
markers were zero, local rollout JSONL from the `08:13Z` wave totaled 38 rows /
141,878,037 tokens, and D1 initially showed 37 rows / 141,858,023 tokens. The
only missing row was public issue `#6820`
(`assignment.public.khala_coding.chatcmpl_49aeee886ea4481880265f78c4516f16`):
the runner log stopped after `assignment_run.runtime_started`, but the Codex
rollout file existed and contained one cumulative `token_count` event:

```text
observed_at: 2026-06-29T08:34:07.961Z
input_tokens: 19,764
cached_input_tokens: 9,600
output_tokens: 250
reasoning_output_tokens: 0
total_tokens: 20,014
```

That row was repaired through the normal `/api/pylon/codex/turns` reporter path
using the current pylon ref `pylon.a1469b9cdf6965a57530`, not by editing an
aggregate or hand-writing D1. After replay, production D1 matched local exactly:

```text
provider: pylon-codex-own-capacity
window: observed_at >= 2026-06-29T08:13:00.000Z
d1_rows: 38
d1_total_tokens: 141,878,037
latest_observed: 2026-06-29T08:48:13.652Z
public /api/public/khala-tokens-served: 4,653,140,117
generatedAt: 2026-06-29T08:53:30.485Z
```

Later checkpoints intentionally showed D1 behind local *total* because one live
session was still burning. That is not a backfill condition. Backfill only when:

- the active marker is gone,
- the reporter failure file is non-empty or D1 still lacks the row after a short
  grace window,
- the idempotency key/session/run identity is known, and
- the insertion uses the same normal token-reporting endpoint/shape, not a
  hand-edited aggregate.

Check for reporter failures with:

```sh
test -s "$HOME/.pylon-fable/codex-turn-report-failures.jsonl" \
  && tail -50 "$HOME/.pylon-fable/codex-turn-report-failures.jsonl"
```

### Refill findings

There were three distinct refill bugs/traps:

1. Pylon ref drift. `provider go-online` returned
   `pylon.a1469b9cdf6965a57530`, while some older commands still targeted
   `pylon.33afd48282a649047e3a`. Requests pinned to the stale pylon ref got
   false `409 target_pylon_ref.unavailable` denials.
2. Top-level capacity can lie. `ownCapacityDispatch.codex.available` sometimes
   showed `0` while per-account `totalAvailableCodexAssignments` was non-zero.
   Always inspect the per-account `capacity.coding.codex.account.*.available`
   refs before concluding the machine is full.
3. Server `already claimed` denials poisoned generic runners. Before the local
   patch in this continuation, `acceptAssignment` returned
   `denial.assignment.duplicate_lease` for server `pylon_api_conflict` but did
   not tombstone the lease in the local assignment store. Generic
   `assignment run-no-spend --json` loops then re-polled the same duplicate and
   never fell through.

The local patch made server already-claimed denials write a terminal rejected
record to `assignment-state.json`, and the focused regression is
`bun test apps/pylon/src/assignment.test.ts`.

There is still another local-only stale case: if a runner accepts a lease and
dies before real runtime progress, the local accepted record can block generic
workers until the heartbeat/prune path marks it stale. Future work should make
accepted records with dead `ownerProcessId` prune immediately, then submit a
public-safe stale closeout when appropriate.

### What worked for immediate fanout

The reliable manual pattern was:

1. Refresh heartbeat with the high-capacity env:

   ```sh
   PYLON_HOME="$HOME/.pylon-fable" \
   PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
   OPENAGENTS_AGENT_TOKEN="$(tr -d '\n' < "$HOME/.pylon-fable/auth/openagents-agent-token")" \
   OPENAGENTS_PYLON_CODEX_CONCURRENCY=25 \
   OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5 \
   PYLON_DISABLE_DAEMON_ROUTING=1 \
   bun apps/pylon/src/index.ts provider go-online --json
   ```

2. Request work with `--no-run`, target the *current* pylon ref from
   `go-online`, and use a fresh `origin/main` SHA from `git ls-remote`.
3. Immediately run that exact assignment with:

   ```sh
   bun apps/pylon/src/index.ts assignment run-no-spend \
     --account <codex-account-ref> \
     --assignment-ref <assignment.public.khala_coding...> \
     --json
   ```

This got concrete issue work live for #6794, #7014, #6820, #6376, and #6795.
Several remaining candidates were exhausted, already PR'd, or bad fuel:

- `#6654` repeatedly returned `500 internal_server_error` during request.
- `#6695` was stolen/claimed before exact execution and became a duplicate.
- `#6822`, `#6823`, `#6824`, `#6846`, `#6637`, `#6707`, `#6709` had already
  proven quick-refusal/bad-fuel in this run.

If the queue is empty, do not pretend a low live count is a Codex ceiling. It is
often just no admissible issue fuel plus stale/duplicate leases.

### 09:05Z refill footgun: exact runners can poison leases with the wrong account ref

After the 08:13Z wave was fully accounted, another refill attempted to request
19 issue assignments with `khala request --no-run`, then start exact runners via
`assignment run-no-spend --account <account> --assignment-ref <ref>`. The account
refs were taken from `pylon accounts list` (`account.pylon.codex.651c...`,
`6be...`, etc.). Those refs were not accepted by the local runner for the
server-returned Codex provider, which expects the heartbeat capacity account refs
(`account.pylon.codex.d4d6...`, `dc8b...`, `e915...`, `ed2d...`, `f88a...`).

Bad result:

- the first exact runner accepted/claimed the server lease,
- then it failed locally with `Pylon account ref is not registered for this
  provider`,
- subsequent correct/unpinned retries got server `409 Pylon assignment was
  already claimed`,
- those leases counted as busy capacity but did not spawn durable Codex work.

Do not repeat that path during emergency fanout. If exact-running a returned
assignment, either use the `codingAssignment.codex.accountRefHash` from the lease
itself or omit `--account` and let the runner resolve the server-selected account.
Even safer for refill: use `khala request ... --json` without `--no-run` and
without `--account`/`--pylon-ref`; the request process delegates and immediately
auto-runs on the matched local account.

Evidence from this refill:

```text
bad no-run dispatch dir: ~/.codex-supervisor/refill-20260629T085711Z
bad exact-run symptom: Pylon account ref is not registered for this provider
retry symptom: OpenAgents assignment request failed (409) ... Pylon assignment was already claimed

working auto-run dir: ~/.codex-supervisor/autorun-20260629T090307Z
working top-up dir: ~/.codex-supervisor/autorun-topup-20260629T090504Z
observed active markers after top-up: 7
public /api/public/khala-tokens-served at 2026-06-29T09:05:43.049Z:
  4,658,120,667
```

The active set after the successful auto-run refill included assignments for
`#7029`, `#7028`, `#7026`, `#7011`, `#6892`, `#6796`, and `#6829`. Several other
auto-run requests were refused immediately with
`blocker.assignment.codex_agent_execution_refused`; those rows still must be
reconciled for token usage, but they are not useful as sustained burn.

Final accounting for the 09:00Z refill window:

```text
local pylon-account rollout JSONL rows since 2026-06-29T09:00:00Z:
  rows: 5
  total_tokens: 5,261,456

production D1 after replay:
  provider: pylon-codex-own-capacity
  rows: 5
  total_tokens: 5,261,456
  latest_observed: 2026-06-29T09:04:25.557Z
  reporter failure spool: 0 bytes
public /api/public/khala-tokens-served at 2026-06-29T09:16:24.941Z:
  4,658,493,521
```

Four small rows (#7028, #7011, #7029, #7026) had local rollout `token_count`
events but no D1 row after all active markers drained. They were replayed through
`/api/pylon/codex/turns`. Replay got one subtle detail wrong on the first try:
the ingest route stores `output_tokens = outputTokens + reasoningOutputTokens`.
For local Codex `token_count`, `total_tokens = input_tokens + output_tokens`;
therefore a manual replay must pass non-reasoning output as `outputTokens` if
`reasoningOutputTokens` is also set. The first replay overcounted the four rows
by exactly 296 reasoning tokens; D1 was corrected by subtracting `reasoning_tokens`
from `output_tokens` and `total_tokens` for those four task refs. Future replay
scripts should do this correctly up front.

One more accepted run landed after the replay: generic `assignment run-no-spend`
picked up public issue `#6893` and completed successfully, reusing/opening PR
`#7032`. After natural reporter closeout, the 09:00Z window still matched local
rollouts exactly:

```text
local pylon-account rollout JSONL rows since 2026-06-29T09:00:00Z:
  rows: 6
  total_tokens: 6,728,167

production D1:
  rows: 6
  total_tokens: 6,728,167
  latest_observed: 2026-06-29T09:23:27.789Z
  #6893 row: 1,466,711 tokens
  public /api/public/khala-tokens-served at 2026-06-29T09:23:52.139Z:
    4,659,963,688
```

After repairing 11 duplicate-claimed leases with explicit stale closeouts, the
server-side top-level scalar still reported `codex.available=0` / `busy=25` while
every per-account entry reported `available=5` and `totalAvailable=25`. New
non-targeted auto-run requests made during that state produced empty detached
logs and no active markers. Treat this as a separate gate/projection bug: the
per-account capacity truth can be fully open while the top-level admission scalar
keeps request fanout effectively wedged. The immediate safe response is to stop
hammering requests, keep accounting clean, and patch the gate/projection logic so
closed/stale/rejected duplicate claims stop contributing to top-level busy.

### 10:00Z operating note: 15 live workers, accounting lag, and heartbeat replica lag

The later 10:00Z fanout successfully held the current Mac at the practical
healthy ceiling: 15 live Codex workers, exactly 5 each on the three sustained
accounts:

```text
account.pylon.codex.dc8b3d28d00e76c92c157821  codex-3  active=5
account.pylon.codex.f88a4773edd26cae162ceb2f  codex-4  active=5
account.pylon.codex.d4d6aab4448922e4a9c4d1e1  codex-5  active=5
```

Do not count the advertised `e915...` and `ed2d...` slots as real capacity until
they pass an execution smoke. They repeatedly accepted readiness in heartbeat
data but refused immediately in local Codex execution. Until fixed, the local
upper bound is 3 good accounts x 5 concurrent runs = 15 workers.

Accounting behavior during this burst:

- Local Codex rollout JSONL emits many live `event_msg` / `token_count` rows.
  Those rows use `payload.info.last_token_usage` for the per-update delta and
  `payload.info.total_token_usage` for the session cumulative total.
- The production counter does not ingest every live `token_count` update. The
  CLI runner reports one `/api/pylon/codex/turns` row when the Codex process
  completes or fails and recovers rollout usage.
- Therefore D1 can legitimately lag local live token totals while active marker
  files still exist. This is not a backfill condition.
- Backfill only after the active marker is gone and either
  `~/.pylon-fable/codex-turn-report-failures.jsonl` is non-empty or D1 still
  lacks the completed session after a short grace window.
- For manual replay, use the normal `/api/pylon/codex/turns` shape. Pass local
  `output_tokens - reasoning_output_tokens` as `outputTokens` and pass
  `reasoning_output_tokens` separately, because the ingest route stores
  `output_tokens` and `reasoning_tokens` as separate D1 columns.

Point-in-time checks from the 10:00Z wave:

```text
public /api/public/khala-tokens-served at 2026-06-29T10:27:47.403Z:
  4,876,159,276

D1 token_usage_events, provider=pylon-codex-own-capacity,
observed_at >= 2026-06-29T10:00:00.000Z, checked at ~10:27Z:
  rows: 51
  total_tokens: 193,142,128
  latest_observed: 2026-06-29T10:26:50.443Z

reporter failure spool:
  ~/.pylon-fable/codex-turn-report-failures.jsonl absent / empty
```

Follow-up accounting check while the 15-worker pool was draining/refilling:

```text
2026-06-29T10:32:34Z public /api/public/khala-tokens-served:
  4,918,338,733

2026-06-29T10:33Z D1 since 10:00Z:
  rows: 62
  total_tokens: 235,321,585
  latest_observed: 2026-06-29T10:32:17.281Z

2026-06-29T10:39Z D1 since 10:00Z:
  rows: 71
  total_tokens: 268,519,330
  latest_observed: 2026-06-29T10:38:55.551Z

2026-06-29T10:41Z D1 since 10:00Z:
  rows: 76
  total_tokens: 286,255,298
  latest_observed: 2026-06-29T10:40:43.883Z

2026-06-29T10:41Z public /api/public/khala-tokens-served:
  4,969,330,958
```

The useful lesson: do not treat "local finished total is ahead of D1" as an
instant backfill trigger. One local rollout finished at `10:38:42Z` with exactly
`2,282,424` tokens. For a few seconds local finished-rollout sum exceeded the
D1 10:00Z slice by exactly `2,282,424`; a repeat D1 query then showed the row:

```text
task_ref: assignment.public.khala_coding.chatcmpl_d36b2244bbf54d3db8523f5c0ec9a8c8
total_tokens: 2,282,424
observed_at: 2026-06-29T10:38:42.563Z
```

So the correct closeout audit order is:

1. Confirm the assignment no longer has a file in
   `~/.pylon-fable/active-assignment-runs/`.
2. Check `~/.pylon-fable/codex-turn-report-failures.jsonl`.
3. Give D1 a short closeout grace and query by `task_ref` or deterministic
   `session_ref`.
4. Replay only if the failure spool exists or D1 still lacks the completed row.

During this check, the active-rollout local token total was already tens of
millions of tokens, but those active totals were intentionally not in D1 yet.
That is expected for the current CLI runner because it posts the final turn row
on closeout, not every live token update.

Heartbeat behavior during this burst:

- `provider go-online --json` is not enough for the public request gate; use
  explicit `presence heartbeat --base-url https://openagents.com --json`.
- The request path can still read stale heartbeat state for a few seconds after
  a successful heartbeat write. We observed immediate 409
  `stale_or_missing_heartbeat` right after a fresh heartbeat, then D1 showed the
  row as fresh and the same targeted requests accepted on retry.
- In emergency fanout, refresh heartbeat, wait a few seconds if the next request
  says stale, verify `pylon_api_registrations.latest_heartbeat_at`, then retry.
  Do not respond to the first stale 409 by killing running workers.

Reliable top-up pattern from this wave:

```sh
OPENAGENTS_AGENT_TOKEN="$(tr -d '\n' < "$HOME/.pylon-fable/auth/openagents-agent-token")" \
PYLON_HOME="$HOME/.pylon-fable" \
PYLON_OPENAGENTS_BASE_URL="https://openagents.com" \
OPENAGENTS_PYLON_CODEX_AGENT_RUNNER=cli \
OPENAGENTS_PYLON_CODEX_CONCURRENCY=25 \
OPENAGENTS_PYLON_CODEX_ACCOUNT_CONCURRENCY=5 \
PYLON_MIN_ASSIGNMENT_LEASE_TIME_REMAINING_MS=5000 \
PYLON_DISABLE_DAEMON_ROUTING=1 \
bun apps/pylon/src/index.ts khala request \
  --account codex-3 \
  --prompt 'Audit public issue #NNNN in OpenAgentsInc/openagents ...' \
  --workflow codex_agent_task \
  --repo OpenAgentsInc/openagents \
  --commit 02506a04e2ed212c6af021a5a5f075e120e65f8e \
  --verify true \
  --pylon-ref pylon.a1469b9cdf6965a57530 \
  --json
```

Use only `codex-3`, `codex-4`, and `codex-5` if no fresh smoke has been run for
`codex-7`. Pinning `--account` is the difference between sustained burn and the
server selecting a locally broken account. The account pin also makes the
desktop fleet view easier to reason about: if active drops below the target,
look at the per-account marker counts and top up only the account with open
healthy slots.

### 11:00Z operating note: codex-7 usable, 20 live sampled, and exact accounting guardrails

The earlier "15 worker" ceiling was a temporary safe ceiling, not the final
machine ceiling. `codex-6` remained unusable (`accounts usage --account codex-6
--refresh --json` showed the local session missing and no provider/platform
truth), but `codex-7` was later smoked successfully:

```text
codex-7 account hash:
  account.pylon.codex.e91557ce7066af1069b76661

proof:
  accounts usage --account codex-7 --refresh --json returned localSession usage
  one real canary assignment accepted and emitted runtime_progress for >20s
  four additional codex-7 assignments accepted and progressed
```

That made the practical local target `4 usable accounts x 5 concurrent runs =
20`. A manual top-up loop eventually sampled the full ceiling:

```text
2026-06-29T11:10:43Z active_total=20
  codex-3 5
  codex-4 5
  codex-5 5
  codex-7 5

2026-06-29T11:11Z active marker sample:
  active_marker_files: 20
  codex-3/hash dc8b3d28d00e76c92c157821: 5
  codex-4/hash f88a4773edd26cae162ceb2f: 5
  codex-5/hash d4d6aab4448922e4a9c4d1e1: 5
  codex-7/hash e91557ce7066af1069b76661: 5
  codex exec process count: 20
```

Important operational lesson: one-shot manual refill is too slow when many
short issue runs finish at once. The winning emergency pattern was:

1. Refresh `presence heartbeat` every loop pass, not every few minutes.
2. Count only `~/.pylon-fable/active-assignment-runs/*.json`, grouped by
   `accountRefHash`.
3. Treat target capacity as `5` per known-good account.
4. Spawn new `khala request --account <ref>` processes only for the missing
   slots.
5. Sleep briefly, then repeat.

The first emergency loop used a rough issue list and accidentally included a
few standing/epic refs (`#6376`, `#6637`, `#6654`, `#6707`, `#6708`, `#6709`).
Those refs should not be part of steady-state issue assignment. Stop only the
top-up loop process, not the spawned `khala request` workers, then restart with
a filtered open-issue pool. A replacement loop at `2026-06-29T11:16Z` excluded
those refs, saw `codex-7` drop to 4, spawned issue `#7030`, and returned the
pool to 20/20.

`codex-6` was removed from the local Pylon account config at
`2026-06-29T11:20Z` after repeat usage refresh showed
`blocker.codex_agent.credentials_missing` plus missing provider/platform truth.
The backup is local-only at
`~/.pylon-fable/config.before-disable-codex-6-20260629T1119Z.json`. The standing
heartbeat script `~/.pylon-fable/bin/standing-pylon.sh` was also patched from
pooled `OPENAGENTS_PYLON_CODEX_CONCURRENCY=30` to `20`. If future raw
`capacity.coding.codex.ready` still disagrees with the per-account refs, trust
the per-account refs and local active markers; the raw pooled scalar can lag or
reflect another heartbeat pass.

Keep the heartbeat hot. During this wave, a refill batch failed with
`stale_or_missing_heartbeat` despite a heartbeat roughly 90 seconds earlier.
A fresh heartbeat at `2026-06-29T11:07:15.130Z` made the same style of targeted
requests accept immediately. For high fanout, assume the useful heartbeat window
is closer to tens of seconds than minutes.

Do not count active rollout `token_count` events as missing D1 rows. The live
counter and `/stats` are backed by `token_usage_events`, and the current CLI path
writes that table at closeout. The correct watch is:

```sh
python3 - <<'PY'
import glob,json,os
live=glob.glob(os.path.expanduser('~/.pylon-fable/codex-turn-report-failures.jsonl'))
print('live_failure_spools', len(live))
PY

cd apps/openagents.com/workers/api && bunx wrangler d1 execute openagents-autopilot --remote --json --command \
"SELECT COUNT(*) AS rows, COALESCE(SUM(total_tokens),0) AS total_tokens, MAX(observed_at) AS latest_observed FROM token_usage_events WHERE provider='pylon-codex-own-capacity' AND observed_at >= '2026-06-29T10:00:00.000Z';"

curl -fsS https://openagents.com/api/public/khala-tokens-served
```

Accounting checkpoints from the 20-worker wave:

```text
2026-06-29T10:54Z public counter:
  5,048,300,631
2026-06-29T10:57Z public counter:
  5,091,161,445
2026-06-29T11:01Z D1 since 10:00Z:
  rows: 117
  total_tokens: 425,748,922
  latest_observed: 2026-06-29T11:00:16.564Z
2026-06-29T11:04Z D1 since 10:00Z:
  rows: 121
  total_tokens: 436,403,850
  latest_observed: 2026-06-29T11:03:38.547Z
2026-06-29T11:06Z D1 since 10:00Z:
  rows: 126
  total_tokens: 460,273,049
  latest_observed: 2026-06-29T11:05:59.199Z
2026-06-29T11:09Z D1 since 10:00Z:
  rows: 133
  total_tokens: 490,592,789
  latest_observed: 2026-06-29T11:08:39.155Z
2026-06-29T11:11Z D1 since 10:00Z:
  rows: 134
  total_tokens: 497,849,668
  latest_observed: 2026-06-29T11:09:55.892Z
2026-06-29T11:11Z public /api/public/khala-tokens-served:
  5,180,963,926
2026-06-29T11:13Z loop health:
  active_total: 20
  codex-3: 5
  codex-4: 5
  codex-5: 5
  codex-7: 5
  latest loop heartbeat: 2026-06-29T11:13:52.155Z
2026-06-29T11:13Z D1 since 10:00Z:
  rows: 140
  total_tokens: 523,975,254
  latest_observed: 2026-06-29T11:13:14.224Z
2026-06-29T11:13Z public /api/public/khala-tokens-served:
  5,207,114,287
2026-06-29T11:24Z D1 since 10:00Z:
  rows: 166
  total_tokens: 627,355,704
  latest_observed: 2026-06-29T11:24:12.425Z
2026-06-29T11:24Z public /api/public/khala-tokens-served:
  5,310,498,203
2026-06-29T11:26Z D1 since 10:00Z:
  rows: 170
  total_tokens: 645,607,037
  latest_observed: 2026-06-29T11:26:45.070Z
2026-06-29T11:26Z public /api/public/khala-tokens-served:
  5,317,574,159
2026-06-29T11:29Z loop health:
  active_total: 20
  codex-3: 5
  codex-4: 5
  codex-5: 5
  codex-7: 5
  latest loop heartbeat: 2026-06-29T11:29:44.790Z
2026-06-29T11:29Z D1 since 10:00Z:
  rows: 175
  total_tokens: 679,013,102
  latest_observed: 2026-06-29T11:28:50.852Z
2026-06-29T11:29Z public /api/public/khala-tokens-served:
  5,362,195,151
2026-06-29T11:31Z D1 since 10:00Z:
  rows: 184
  total_tokens: 712,050,313
  latest_observed: 2026-06-29T11:31:30.813Z
2026-06-29T11:31Z public /api/public/khala-tokens-served:
  5,395,250,550
2026-06-29T11:35Z loop health:
  active_total: 20
  codex-3: 5
  codex-4: 5
  codex-5: 5
  codex-7: 5
  latest loop heartbeat: 2026-06-29T11:35:40.214Z
2026-06-29T11:35Z D1 since 10:00Z:
  rows: 190
  total_tokens: 736,080,565
  latest_observed: 2026-06-29T11:35:14.743Z
```

Exact recent-ref accounting audit at `2026-06-29T11:28Z`:

```text
recent accepted assignment refs scanned from local top-up logs: 48
currently active assignment refs: 19
finished candidates (accepted and no active marker): 29
D1 task_ref matches for finished candidates: 29
missing finished candidates: 0
active refs without D1 rows yet: 19
```

The same wave was reconciled against local Codex rollout JSONL at
`2026-06-29T11:34Z`, using `assignment_run.accepted.statusRef` to map
`workspace.pylon.codex_agent_task.*` back to assignment refs and then hashing
the rollout thread id into `session.pylon.codex_composer.*`:

```text
local pylon-account rollout files with token_count since 2026-06-29T11:00Z: 91
rollouts mapped to known assignment refs: 61
finished known assignment rollouts: 43
finished known local total_tokens: 163,214,477
finished known D1 total_tokens by session_ref: 163,214,477
missing finished known rollouts: 0
active known rollouts pending closeout: 17
```

There were 30 rollout files whose workspace could not be mapped from the
top-up logs (they were older/manual/supervisor-originated relative to that log
sample), but the D1 session-ref query returned 74 ledger matches across 91 local
rollouts. The difference was exactly the active pending set at that instant.
That is the useful invariant for this style of audit: every non-active local
pylon-account rollout since the cut had a D1 row; only active rollouts lacked
rows.

At `2026-06-29T11:36Z`, after additional closeouts, the same rollout-level audit
still matched:

```text
local pylon-account rollout files with token_count since 2026-06-29T11:00Z: 93
rollouts mapped to known assignment refs: 63
active known rollouts pending closeout: 18
finished known assignment rollouts: 45
finished known local total_tokens: 173,788,392
finished known D1 total_tokens by session_ref: 173,788,392
missing finished known rollouts: 0
```

At `2026-06-29T11:38Z`, the public surfaces backed by `/stats` reflected the
same ledger, not a stale side counter:

```text
/api/public/khala-tokens-served:
  tokensServed: 5,448,807,847
/api/public/khala-tokens-served/model-mix:
  Pylon-Codex: 4,999,853,765 tokens across 1,309 requests
/api/public/khala-tokens-served/demand-mix:
  own_capacity / khala_coding_delegation:
    4,999,856,652 tokens across 1,310 requests
```

At `2026-06-29T11:40Z`, `gh issue list` and `gh pr list` showed that the local
fanout had created a different bottleneck: roughly 60 open issues, but about
120 open PRs, and almost every non-standing open issue had at least one obvious
open PR already. Continuing to feed the old issue-only loop started duplicating
issue implementations. The safer max-burn target became PR review/fix tasks:
fill open slots with prompts of the form "review open PR #NNNN, check out the PR
branch, run narrow verification, fix clear blockers on that branch, and do not
open a duplicate PR."

The old issue top-up shell was stopped by killing only the controller process;
active assignment workers survived. The replacement controller uses the same
four good accounts and waits for active slots before spawning PR-review workers.
The first PR-review wave accepted and progressed on PRs `#7293` through `#7272`.
One heartbeat during the controller swap returned a D1 idempotency-key unique
constraint from `/api/pylons/.../heartbeat`; the next heartbeat succeeded and
dispatch continued. Treat this as another transient heartbeat-write wrinkle
unless it repeats often enough to block accepts.

First PR-review accounting checkpoint:

```text
2026-06-29T11:48Z PR-wave assignment diff:
  recent PR-review assignment refs: 21
  active assignment refs: 18
  finished PR-review candidates: 3
  D1 token_usage_events matches: 3
  missing finished candidates: 0

2026-06-29T11:48Z D1 since 10:00Z:
  rows: 210
  total_tokens: 803,616,830
  latest_observed: 2026-06-29T11:48:42.004Z
```

At `2026-06-29T11:50Z`, the first PR-review controller was replaced with a v2
controller because the v1 loop only spawned one replacement per account per
pass. During a fast-draining PR wave, that left accounts with multi-slot gaps
for another 16-second sleep. The v2 loop fills all missing per-account slots in
one pass, still capped at `20` total / `5` per account, and skips PR numbers it
has already launched by scanning `~/.pylon-fable/pr-topup-logs/`.

```text
2026-06-29T11:51Z v2 loop sample:
  active_total before fill: 16
  codex-3: 5
  codex-4: 2
  codex-5: 4
  codex-7: 5
  spawned: codex-4 #7306, codex-4 #7305, codex-4 #7304, codex-5 #7303

2026-06-29T11:52Z active marker sample:
  active_total: 20
  codex-3: 5
  codex-4: 5
  codex-5: 5
  codex-7: 5

2026-06-29T11:52Z D1 since 10:00Z:
  rows: 233
  total_tokens: 820,173,911
  latest_observed: 2026-06-29T11:50:57.396Z

2026-06-29T11:52Z public /api/public/khala-tokens-served:
  5,503,432,788
```

The important operational distinction: an accepted ref that still has an active
marker is not missing from the counter yet; its final row is written on closeout.
Only an accepted ref with no active marker and no `token_usage_events.task_ref`
row is a repair candidate. The set-diff recipe that avoided guessing was:

```sh
python3 - <<'PY'
import glob, json, os, re, subprocess, time

api = "apps/openagents.com/workers/api"
logs = sorted(
    glob.glob(os.path.expanduser("~/.pylon-fable/topup-logs/khala-request-*.log")),
    key=os.path.getmtime,
    reverse=True,
)[:180]

recent = []
seen = set()
for path in logs:
    text = open(path, errors="replace").read()
    refs = re.findall(r"assignment\.public\.khala_coding\.chatcmpl_[A-Za-z0-9_\-]+", text)
    if not refs or refs[-1] in seen:
        continue
    seen.add(refs[-1])
    recent.append(refs[-1])

active = set()
for path in glob.glob(os.path.expanduser("~/.pylon-fable/active-assignment-runs/*.json")):
    try:
        assignment_ref = json.load(open(path)).get("assignmentRef")
    except Exception:
        continue
    if assignment_ref:
        active.add(assignment_ref)

quoted = ",".join("'" + ref.replace("'", "''") + "'" for ref in recent)
sql = (
    "SELECT task_ref, observed_at, total_tokens FROM token_usage_events "
    "WHERE provider='pylon-codex-own-capacity' "
    f"AND task_ref IN ({quoted});"
)
data = json.loads(subprocess.check_output(
    ["bunx", "wrangler", "d1", "execute", "openagents-autopilot", "--remote", "--json", "--command", sql],
    cwd=api,
    text=True,
))
ledger = {row["task_ref"] for row in data[0].get("results") or []}
finished = [ref for ref in recent if ref not in active]
missing = [ref for ref in finished if ref not in ledger]
print("recent_refs", len(recent))
print("active_refs", len(active))
print("finished_candidates", len(finished))
print("missing_finished_candidates", len(missing))
for ref in missing:
    print("MISSING", ref)
PY
```

One D1 read during this audit returned Cloudflare API `Authentication error
[code: 10000]`, while `wrangler whoami` immediately confirmed a valid OAuth
token and the retry succeeded. Treat a single D1 read failure during high churn
as a transient read failure until auth and retry say otherwise; do not infer
that accounting failed from one failed operator query.

Reporter failure spool handling from this wave:

- At ~10:54Z, `~/.pylon-fable/codex-turn-report-failures.jsonl` contained three
  real failed closeout reports: two `401 unauthorized` and one `500
  internal_server_error`.
- They were replayed through `createPylonCodexTurnReporter` with the current
  pylon agent token and then verified in D1.
- The replayed failure file was archived as
  `~/.pylon-fable/codex-turn-report-failures.replayed-20260629T105559Z.jsonl`.
- At 11:04Z, 11:06Z, 11:09Z, 11:11Z, and 11:13Z there was no live
  `codex-turn-report-failures.jsonl`.

Rule for future agents: if a live failure spool reappears, replay it before
continuing to claim the public counter is correct. If no live failure spool
exists and active marker files still exist, do not backfill active sessions.
Wait for closeout, then query by `task_ref`/`session_ref` if the row still seems
missing after a short grace period.

Second accounting wrinkle discovered at `2026-06-29T12:00Z`: not every missing
closeout creates `codex-turn-report-failures.jsonl`. A fast PR-review wave left
14 accepted assignments with:

- no active marker under `~/.pylon-fable/active-assignment-runs/`;
- no live `codex exec`/wrapper process for the assignment workspace;
- no `token_usage_events.task_ref` row;
- no live report-failure spool;
- but a real local Codex `token_count` event in the accepting account's rollout
  JSONL.

The reliable correlation key was NOT `rg assignmentRef` over session files:
agents can read one another's logs, so that produces false positives. The safe
mapping is:

1. Read the accepted lifecycle event from the local top-up log.
2. Extract `statusRef: assignment.accepted.<hash>`.
3. Restrict to the accepting account from the log name/account hash.
4. Match only rollout JSONL files whose first `session_meta.payload.cwd` contains
   `workspace.pylon.codex_agent_task.<hash>`.
5. Use the latest `event_msg` / `token_count` `total_token_usage`.
6. Convert Codex's `output_tokens` to reporter shape by subtracting
   `reasoning_output_tokens` before setting `usage.outputTokens`; otherwise the
   replay double-counts reasoning tokens.
7. Derive `sessionRef` as
   `session.pylon.codex_composer.<sha256(session_id).slice(0,24)>`, matching the
   pylon CLI runner's normal reporter path.

Those 14 terminal-no-closeout refs were replayed through
`createPylonCodexTurnReporter` to `POST /api/pylon/codex/turns` with the current
pylon agent token, not by editing D1 directly. Replayed total: `9,291,393`
tokens.

Verification after replay:

```text
2026-06-29T12:01Z PR-review assignment diff:
  recent PR-review assignment refs: 77
  active assignment refs: 19
  finished candidates: 58
  D1 token_usage_events matches for recent refs: 58
  missing finished candidates: 0
  recent ledger tokens: 55,962,746
  latest observed: 2026-06-29T12:01:27.300Z

2026-06-29T12:01Z D1 since 10:00Z:
  rows: 299
  total_tokens: 857,744,116
  latest_observed: 2026-06-29T12:01:27.300Z

2026-06-29T12:01Z public /api/public/khala-tokens-served:
  5,541,003,856

2026-06-29T12:02Z v2 refill loop:
  active_total: 20
  codex-3: 5
  codex-4: 5
  codex-5: 5
  codex-7: 5
```

Immediate lesson: the failure spool is necessary but not sufficient. During
high fanout, periodically run the finished-ref diff and, if it reports missing
rows, inspect the exact-account `session_meta.cwd` mapping before replaying.
Never backfill sessions that still have a live process or active marker; with
the normal idempotency key, early replay would lock in an undercount if the
session later produced more tokens.

Probable cause for the 14 terminal-no-closeout rows: they all stopped around
the PR-controller swap at ~11:50Z and their Codex rollout JSONL never emitted
`task_complete`. Even when you believe you are killing only the controller PID,
shell-owned background children can still receive `SIGHUP`/termination depending
on how the controller was launched. Future refill controllers should either:

- never be replaced while they still own live child workers; or
- spawn workers detached enough to survive controller replacement (`nohup`,
  `setsid`, or an equivalent supervisor-owned launch path), with stdout/stderr
  already redirected to per-worker logs.

Post-replay follow-up at `2026-06-29T12:04Z` stayed clean:

```text
recent PR-review entries: 86
active refs: 18
finished candidates: 68
D1 matches in recent set: 70
missing finished candidates: 0
live failure spools: 0
```

Follow-up at `2026-06-29T12:20Z` exposed both accounting paths in the same
burst:

- A fresh `~/.pylon-fable/codex-turn-report-failures.jsonl` appeared with four
  complete turn reports: three `401 unauthorized` ingest failures and one `503
  pylon_codex_storage_error` from `token_usage_ingest`.
- The stored pylon agent token in
  `~/.pylon-fable/auth/openagents-agent-token` was still valid
  (`/api/agents/me` returned 200), so the 401s were stale-token failures from
  the worker process environment rather than bad local report data.
- Replaying the four spooled reports through `createPylonCodexTurnReporter`
  succeeded on the first attempt for all four.
- Exact D1 verification showed the four assignment refs present afterward. Two
  of the refs had multiple turn rows, which is expected when the same assignment
  produces both main and helper turns.
- The two PR-review refs that were missing before replay were then present in
  the finished-ref diff.
- The replayed spool was moved to
  `~/.pylon-fable/replayed-failures/codex-turn-report-failures-20260629T121911Z.jsonl`
  so any new live failure creates a fresh obvious file.

Verification after that replay:

```text
2026-06-29T12:20Z PR-review assignment diff:
  recent PR-review entries: 146
  active refs: 16
  finished candidates: 130
  D1 matches in recent set: 130
  missing finished candidates: 0
  recent ledger tokens: 167,698,119
  latest observed: 2026-06-29T12:19:34.948Z

2026-06-29T12:19Z D1 since 10:00Z:
  rows: 426
  total_tokens: 969,479,489
  public_tokens: 969,479,489
  latest_observed: 2026-06-29T12:19:34.948Z

2026-06-29T12:19Z public /api/public/khala-tokens-served:
  tokensServed: 5,652,802,481

live failure spools after archive:
  0
```

## 13:10Z note: ad hoc UI task dispatch should not force `--pylon-ref`

After the fleet was reoriented toward PR resolution plus a small ORCA/UI lane,
three explicit UI/operator tasks were requested:

- fix the OpenAgents desktop Coding page false empty-state;
- move the `/stats` Khala token counter into the top header area, widen the
  daily chart, include June 24 forward, and deploy;
- implement the Codex weekly-exhaustion reset/recovery policy while leaving
  5-hour cooldown accounts to wait for reset.

The first attempts were launched with detached logs but exited silently before
acceptance. Re-running one request in the foreground showed the real blocker:

```text
pylon khala request failed (409): ... blocker.public.pylon_dispatch.duplicate_active_assignment
```

That happened only on the forced `--pylon-ref pylon.a1469b9cdf6965a57530` path.
The same prompts accepted immediately when dispatched like the working PR
resolver, pinned to a Codex account but without an explicit `--pylon-ref`.

Accepted refs:

```text
desktop empty-state fix:
  account: codex-7
  assignment: assignment.public.khala_coding.chatcmpl_50eef2af5b7c47c1bfa70d6b740f3c01

/stats counter/chart/deploy:
  account: codex-3
  assignment: assignment.public.khala_coding.chatcmpl_c6aa92db7533443bada2392e2937e085

weekly rate-limit reset policy:
  account: codex-4
  assignment: assignment.public.khala_coding.chatcmpl_1d2ce7605c874f85a11a3f632a1f3173
```

Operational rule: during high fanout, use `--account <codex-ref>` and let the
public request gate choose the linked pylon unless a specific pylon-target smoke
is the thing being tested. A forced `--pylon-ref` can collide with an existing
active assignment for that pylon and produce a duplicate-active refusal even
when the target Codex account has free slots.

Follow-up at `2026-06-29T12:37Z` and `2026-06-29T12:45Z`: the "some finished,
some still running" state is expected and should be audited by splitting active
markers from finished candidates.

At `12:37Z`, the PR-review accepted-ref diff showed:

```text
recent PR-review entries: 223
active refs: 17
finished candidates: 206
D1 token_usage_events matches: 206
missing finished candidates: 0
recent ledger tokens: 257,209,279
latest observed: 2026-06-29T12:37:08.119Z
```

At `12:45Z`, after another fast wave of reviews:

```text
live active marker files: 18
live failure spools: 0

D1 since 10:00Z:
  rows: 603
  total_tokens: 1,103,776,318
  public_tokens: 1,103,776,318
  latest_observed: 2026-06-29T12:45:45.475Z

public /api/public/khala-tokens-served:
  tokensServed: 5,786,123,542

PR-review accepted-ref diff:
  recent PR-review entries: 263
  active refs: 19
  finished candidates: 244
  D1 token_usage_events matches: 244
  missing finished candidates: 0
  recent ledger tokens: 301,994,948
  latest observed: 2026-06-29T12:45:45.475Z
```

Interpretation: finished PR-review assignments were counted; active sessions
were correctly not counted yet. Future agents should not retroactively add rows
for active sessions. If a session has an active marker, wait for closeout or a
real failure spool. If a finished candidate is missing and no failure spool
exists, only then do the exact-account rollout JSONL reconstruction described
above.

Code follow-up added in commit scope after this audit: a sourceable PR-review
refill helper at
`apps/pylon/scripts/codex-supervisor/pr-review-refill.sh` plus
`pr-review-refill.test.sh`.

What the helper does:

- takes an atomic local lock keyed by PR number before launching `khala request`;
- keeps accepted PR locks while the matching assignment ref is active under
  `~/.pylon-fable/active-assignment-runs`;
- converts stale accepted/inactive locks into short-lived done markers so the
  refiller keeps walking the PR queue instead of rereviewing one hot PR;
- releases failed-before-accept locks immediately so transient launch failures
  do not park useful work;
- parses recent `pr-review-*.log` accepted lifecycle events back into lock
  state after a controller restart;
- provides `refill-once --account <codex-ref>` as the small replacement unit for
  the ad hoc v2 refill loop.

Focused verification was intentionally limited, per operator instruction not to
run all prepush gates yet:

```text
bash apps/pylon/scripts/codex-supervisor/pr-review-refill.test.sh
  12 passed, 0 failed

bash apps/pylon/scripts/codex-supervisor/claim-dispatch.test.sh
  13 passed, 0 failed

bash -n apps/pylon/scripts/codex-supervisor/pr-review-refill.sh \
  apps/pylon/scripts/codex-supervisor/pr-review-refill.test.sh
  ok
```

Operational guidance: do not kill the currently saturated ad hoc controller
while it owns live children. Start using the lock-aware helper only as a
replacement/refill unit once the current controller can be stopped without
orphaning active accepted assignments, or wrap it in a parent loop that does
not terminate its launched worker children on controller replacement.

The public stats read side also flapped during this same check:
`/api/public/khala-tokens-served/history` and `/model-mix` returned transient
500s, then returned 200 on retry. Direct D1 reads for the exact history and
model-mix SQL succeeded in about 300 ms, so this was D1/load/read-path pressure,
not bad token rows. Do not confuse a transient public stats 500 with missing
accounting; check the ledger diff and the failure spool first.

Follow-up at `2026-06-29T12:30Z` changed the work-queue diagnosis:

```text
open GitHub issues: 60
open GitHub PRs: 385
mergeable PRs: 225
unknown-mergeability PRs: 160
issues with a detected PR by branch/title: 59 / 60
```

The one issue not detected by the simple `issue-<number>` / `#<number>` parser
was `#7011`, but there is an open PR branch named
`pylon-account-registry-effect-service-7011`, so the real bottleneck is no
longer issue-start fanout. The bottleneck is PR review, dedupe, and merge
readiness. Keep the local fleet pointed at PR review/fix work until that queue
is drained or the merge path is automated.

The v2 PR-review refill loop kept the machine near saturation, but it sometimes
launched duplicate reviews for the same very new PR. Example: recent logs showed
duplicate launches for `#7410`, `#7409`, `#7408`, `#7407`, `#7406`, `#7405`,
and `#7404`; `#7404` briefly had two active accepted assignments at once. Some
duplicates were harmless failed-before-accept attempts, but at least one was
real active duplication.

Next controller fix: take an atomic per-PR local lock before calling
`khala request`, keep that lock while an accepted assignment is active, and write
a separate terminal marker for failed-before-accept attempts. The controller
must distinguish:

- never accepted: retrying the same PR is fine;
- accepted and active: do not launch another reviewer for that PR;
- accepted and finished with D1 row: skip unless a human intentionally asks for
  another review pass.

This is not worth killing the live controller for while it is saturated. Let the
current workers close out; build the lock-aware controller as the next durable
replacement.

Another fresh failure spool appeared at `2026-06-29T12:24Z` with two reports:
one `401 unauthorized` main turn and one `500 internal_server_error` helper
turn. Both replayed successfully through `createPylonCodexTurnReporter` with the
stored pylon token, exact D1 rows were verified, and the spool was archived as:

```text
~/.pylon-fable/replayed-failures/codex-turn-report-failures-20260629T122702Z.jsonl
```

Verification immediately after:

```text
2026-06-29T12:26Z D1 since 10:00Z:
  rows: 478
  total_tokens: 1,009,645,916
  public_tokens: 1,009,645,916
  latest_observed: 2026-06-29T12:26:38.283Z

2026-06-29T12:26Z public /api/public/khala-tokens-served:
  tokensServed: 5,693,025,375

live failure spools after archive:
  0
```
