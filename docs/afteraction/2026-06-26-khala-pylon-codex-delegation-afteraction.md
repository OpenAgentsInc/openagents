# 2026-06-26 Khala -> Pylon -> Codex Delegation After-Action

## Status

Implementation, live Pylon proof, production deploy, and live smokes are
complete in this change.

Focused Pylon regression coverage is passing locally for:

- dead local no-spend owner recovery;
- expired local accepted lease pruning;
- hung Codex SDK runner bounding;
- no-spend assignment acceptance/progress/artifact/closeout flow.

Live production-API proof from the local patched Pylon completed at
`2026-06-26T23:24Z`:

- the abandoned lease
  `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632`
  was submitted as stale with closeout
  `assignment.closeout.cfd5d6dd9b2a6140f361a836`;
- two new assignments on the same linked Pylon were then run and accepted in
  parallel:
  `assignment.public.khala_coding.chatcmpl_6d190807e87c4a558dac39a098a9d268`
  and
  `assignment.public.khala_coding.chatcmpl_a2bd2121c00d4a2f8e63eb26f48f9148`;
- both produced exact owner-capacity token rows, owner-only ATIF traces, and
  private raw Codex event archives;
- after both closeouts, the same Pylon advertised `available=2`, `ready=2`,
  `busy=0`, and `queued=0` again.

The Worker-side #6358 counter-health changes were deployed through
`deploy:safe` as Worker version `95d3fcee-f740-477d-b3c4-368f198e8255`.
Production smokes passed after deploy:

- `https://openagents.com/` returned HTTP 200;
- `https://openagents.com/assets/index-MEx5hXlp.js` returned HTTP 200;
- `GET /api/public/khala-tokens-served` returned `tokensServed=301,802,319`;
- `GET /api/public/khala-tokens-served/model-mix?window=30d` returned
  `totalTokens=301,802,319`, `pylon_codex=224,016,760`, and `123` Pylon-Codex
  usage events;
- `scripts/khala-canary.sh` returned `state="up"`,
  `publicCounterCheck="skipped_internal"`, and `demandKind="internal"`;
- one bounded `scripts/khala-heartbeat.sh` wave returned `state="ok"`,
  `ok=10`, `fail=0`, `summedTokens=21,049`,
  `publicCounterCheck="skipped_internal"`, and `demandKind="internal"`.

Follow-up correction: the `skipped_internal` interpretation was wrong for the
headline counter. Internal dogfood is not external market demand, but it is
still real Khala-served token volume, so it must remain included in the public
total-only counter.

## System Under Audit

The Khala -> Pylon -> Codex path is an owner-capacity coding delegation lane:

1. A Khala coding request is typed as `codex_agent_task`.
2. The Worker dispatches a no-spend Pylon assignment only to a Pylon linked to
   the same caller/owner scope.
3. The local Pylon accepts the lease.
4. Pylon runs Codex locally with owner-local full-access settings so Codex can
   work in a real repository/worktree.
5. Pylon posts progress, artifact/proof refs, and final closeout to the Worker.
6. Pylon/Codex exact usage is ingested into `token_usage_events` as
   `provider='pylon-codex-own-capacity'`,
   `model='openagents/pylon-codex'`,
   `usage_truth='exact'`,
   `demand_kind='own_capacity'`, and
   `demand_source='khala_coding_delegation'`.
7. Owner-only redacted ATIF traces and private raw Codex SDK event archives are
   retained for audit; public closeouts and counters remain aggregate/public-safe.

The lane is intentionally not a generic marketplace worker. It is a
caller-owned, no-spend path for the user's linked Pylon and local Codex account.

## Symptoms Observed

During the #6358 counter-health work, delegation became unreliable while trying
to use one local Pylon for multiple Pylon/Codex assignments.

Observed symptoms:

- A delegated #6358 Codex assignment was accepted and then effectively hung for
  many minutes with only runtime heartbeat/progress noise.
- The local assignment state still contained accepted leases after the
  supervising process was gone or no longer producing a closeout.
- A fresh Pylon heartbeat advertised `capacity.coding.codex.available=2` and
  `capacity.coding.codex.ready=2`.
- A new fixture assignment was created successfully:
  `assignment.public.khala_coding.chatcmpl_6d190807e87c4a558dac39a098a9d268`.
- A second fresh fixture request was refused by the controlled dispatch gate
  even though the operator expected one additional slot.
- Authenticated polling showed that an older accepted assignment,
  `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632`,
  was still within its server lease window and therefore still counted against
  the advertised capacity.
- The stale assignment had owner-only traces, but no exact
  `token_usage_events` closeout row, so it could not be treated as completed
  work or as public counter proof.

The practical result was bad: one abandoned local assignment could poison a
Pylon's dispatch capacity until the server lease expired, and manual parallel
delegation became easy to misread.

## Incident Timeline

- `2026-06-26T20:49Z`: roadmap refresh showed Pylon/Codex as the usable
  delegation lane, with the public counter and proof endpoints already relying
  on exact closeout rows as the source of truth.
- During #6358 counter-health work, a local no-spend Pylon/Codex run accepted
  `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632`
  and then stopped making meaningful progress. The local runner process was no
  longer a reliable active owner, but the server still saw an accepted,
  non-expired lease.
- A later `provider go-online` advertised two available Codex slots, but a
  second fixture request was refused because the abandoned accepted lease still
  consumed one server-side active slot.
- Local inspection found owner-only trace material for the abandoned run, but
  no exact token closeout row. That meant the row could not honestly increment
  public usage and could not be used as completion proof.
- The fix added local owner-process evidence, heartbeat evidence, and stale
  no-spend closeout retry.
- With the patched local Pylon, the abandoned row closed stale, capacity was
  freed, and two fresh assignments ran concurrently to accepted closeout.

## Root Causes

### 1. The Codex SDK runner was trusted to settle

`executeCodexAgentAssignment` delegated to the SDK/injected runner and assumed
the runner promise would resolve or reject. If the runner wedged without
settling, `assignment run-no-spend --json` could keep emitting progress while
never reaching progress/artifact/closeout submission.

That is not an acceptable control plane contract. A local executor can be slow,
but it must be bounded by the assignment timeout and close with a typed public
blocker when it cannot complete.

### 2. Local assignment state did not record enough ownership evidence

The local Pylon assignment state tracked accepted/running/closed-ish status, but
older rows did not carry:

- server lease expiry;
- payment mode;
- owning local process id;
- local owner heartbeat timestamp;
- local owner heartbeat sequence.

Without those fields, a sibling Pylon process could not distinguish:

- "another local process is actively running this assignment";
- "the owner process died";
- "this is a legacy accepted row from an interrupted run";
- "the server lease expired";
- "this was a no-spend local run that can safely be marked stale".

The safe default was therefore too conservative: leave the accepted row alone
until the server's long lease window expired.

### 3. Server dispatch capacity only had server-side assignment truth

The Worker dispatch gate correctly ignores expired active leases, but it cannot
know that a local Mac process died unless the local Pylon reports that fact.

For the bad row, the server saw "accepted and not expired", so the row continued
to consume one of the active Codex slots. This made the refusal correct from the
server's point of view even though the local operator knew the assignment had
been abandoned.

### 4. Manual parallel delegation had weak closeout ergonomics

The runbook documented how to:

- advertise multiple Codex slots;
- request a typed coding assignment;
- run a no-spend assignment;
- inspect exact token rows.

But the operational path still required manual shells and manual assignment
selection. If one shell died or the active lease state became stale, the next
operator had to infer too much from broad assignment polls, local JSON, and
public counter movement.

## Fixes Implemented

### Codex runner hard deadline

`apps/pylon/src/codex-agent-executor.ts` now wraps the Codex runner with an outer
deadline derived from the assignment timeout.

If the runner does not settle in time, Pylon returns a typed
`budget_exceeded` result and closes the assignment with
`blocker.assignment.codex_agent_budget_exceeded` instead of silently spinning.

Regression:

- `apps/pylon/tests/codex-agent-executor.test.ts`
  `bounds a hung Codex runner with a typed budget-exceeded closeout`

### Local lease expiry and terminal pruning

`apps/pylon/src/assignment.ts` now records server lease expiry and payment mode
for local accepted leases and preserves them on closeout.

Before polling/accepting new work, Pylon prunes locally expired active leases to
`stale` so an old active row does not block fresh local work forever.

Regression:

- `apps/pylon/tests/assignment.test.ts`
  `stales expired local accepted leases so interrupted runs do not block fresh work`

### Owner process and local heartbeat tracking

New accepted leases carry:

- `ownerProcessId`;
- `ownerStartedAt`;
- `ownerHeartbeatAt`;
- `ownerHeartbeatSequence`;
- `leaseExpiresAt`;
- `paymentMode`.

`assignment run-no-spend` starts a local heartbeat for the claimed lease and
updates the local state while the run is active. Sibling Pylon processes can now
detect active-vs-dead local ownership with evidence instead of waiting for a
server timeout.

### Dead-owner no-spend recovery closeout

Before polling for new assignments, `assignment run-no-spend` scans local active
leases. If a no-spend local lease has a dead owner process, stale heartbeat, or
expired server lease, Pylon submits a public-safe stale closeout:

- `status='stale'`;
- `paymentMode='no-spend'`;
- `settlementState='not_applicable'`;
- `payoutClaimAllowed=false`;
- `blocker.assignment.local_run_interrupted`.

This tells the Worker the old accepted assignment no longer represents active
work and frees the dispatch capacity for new assignments.

Regression:

- `apps/pylon/tests/assignment.test.ts`
  `closeouts dead local no-spend owners so parallel runners can claim fresh work`

### Stale closeout retry

The first live recovery attempt exposed a second edge case: a local row could be
marked `stale` before the server accepted or recorded the stale closeout. That
local state is not enough. If `serverCloseoutSubmittedAt` is missing,
`assignment run-no-spend` now retries the server closeout before polling for new
work.

Regression:

- `apps/pylon/tests/assignment.test.ts`
  `retries local stale no-spend closeout until the server closeout is recorded`

### Server invariant coverage

`apps/openagents.com/workers/api/src/pylon-api-routes.test.ts` now covers the
dispatch-side invariant that expired active leases do not consume future
capacity. That was already the intended behavior, but it is now locked as a
regression test.

### Counter-health correction

The same patch also fixes the #6358 counter-health failure mode that triggered
the investigation:

- public token counter projections include all real served-token rows:
  `internal`, `internal_stress`, `own_capacity`, external, and unlabeled;
- `demand_kind` / `demand_source` remain in the private ledger for segmentation
  but are not subtracted from the headline public aggregate;
- sync deltas publish internal rows too, with only event refs, timestamps, and
  counts, so the homepage stream reconciles with the scalar;
- `scripts/khala-heartbeat.sh` and `scripts/khala-canary.sh` now validate 200
  status, non-empty provider usage, and readable/monotonic public counters,
  requiring counter movement by default even for internal probes.

The important policy distinction is that "internal dogfood" is not external
market demand, but it is still real Khala-served token volume. Public market
claims must use segmented/private analytics; the headline token counter is a
total served counter and must not move backward when segmentation improves.

## Live Multi-Assignment Proof

The patched local Pylon published two Codex slots:

- `pylonRef`: `pylon.33afd48282a649047e3a`;
- `capacity.coding.codex.available`: `2`;
- `capacity.coding.codex.ready`: `2`;
- `capacity.coding.codex.busy`: `0`;
- `capacity.coding.codex.queued`: `0`;
- heartbeat sequence: `231`;
- heartbeat time: `2026-06-26T23:24:27.833Z`.

The abandoned assignment was recovered first:

- assignment:
  `assignment.public.khala_coding.chatcmpl_fd33103f7b4349218f9b0760e8ca5632`;
- local terminal status: `stale`;
- closeout:
  `assignment.closeout.cfd5d6dd9b2a6140f361a836`;
- closeout submitted at: `2026-06-26T23:22:01.074Z`;
- blocker: `blocker.assignment.local_run_interrupted`;
- settlement: `not_applicable`;
- payout claim allowed: `false`.

Two fresh assignments then ran concurrently and both closed accepted:

| Assignment | Closeout | Total tokens | Prompt | Completion | Reasoning | Cache read | Trace count | Raw events |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `assignment.public.khala_coding.chatcmpl_6d190807e87c4a558dac39a098a9d268` | `assignment.closeout.5d3bbae95ed256342870f3fe` | 161,832 | 160,879 | 953 | 74 | 131,968 | 15 | 22 |
| `assignment.public.khala_coding.chatcmpl_a2bd2121c00d4a2f8e63eb26f48f9148` | `assignment.closeout.711d0b1571137c02661d6827` | 128,873 | 128,138 | 735 | 61 | 99,328 | 12 | 17 |

Both exact usage rows carried:

- `provider='pylon-codex-own-capacity'`;
- `model='openagents/pylon-codex'`;
- `usage_truth='exact'`;
- `demand_kind='own_capacity'`;
- `demand_source='khala_coding_delegation'`.

After the two accepted closeouts, `provider go-online` again showed
`available=2`, `ready=2`, `busy=0`, and `queued=0`, proving the stale row no
longer poisoned dispatch capacity.

## What This Fix Does Not Claim

This fix does not claim:

- broad marketplace delegation;
- routing to someone else's Pylon;
- payout eligibility;
- public exposure of raw Codex events;
- public counter proof from a running assignment before exact closeout usage is
  ingested;
- server-side knowledge of local process death without Pylon reporting it.

It only fixes the owner-local no-spend Pylon/Codex path so interrupted local
runs can close as stale and multiple advertised Codex slots can be used without
being poisoned by dead local state.

## Verification Checklist

Local:

- `bun test apps/pylon/tests/assignment.test.ts apps/pylon/tests/codex-agent-executor.test.ts`
  passed before this after-action update.
- `bun run --cwd apps/openagents.com/workers/api test -- src/pylon-api-routes.test.ts`
  passed before this after-action update.
- #6358 focused counter tests:
  - `src/public-khala-tokens-served-routes.test.ts`
  - `src/public-khala-tokens-served-history-routes.test.ts`
  - `src/public-khala-tokens-served-model-mix-routes.test.ts`
  - `src/inference/served-tokens-recorder.test.ts`
  - `src/inference/khala-tokens-served-sync.test.ts`
  - `src/token-usage-ledger.test.ts`
- `bun run --cwd apps/openagents.com/workers/api typecheck`
- `bun run --cwd apps/openagents.com check:architecture`
- `git diff --check`

Live proof:

1. Publish `OPENAGENTS_PYLON_CODEX_CONCURRENCY=2` with
   `provider go-online` and `presence heartbeat`. Complete.
2. Create two fixture assignments targeted to the same linked Pylon. Complete.
3. Run both no-spend assignments concurrently with explicit assignment refs.
   Complete.
4. Confirm both reach closeout. Complete.
5. Confirm the stale older assignment is no longer dispatch-blocking after the
   local stale closeout. Complete.
6. Confirm exact `token_usage_events` rows for real Codex assignments before
   treating public counter movement as proof. Complete.
7. Confirm the public counter remains monotonic and excludes only
   `demand_kind='internal'`, while preserving `own_capacity` Pylon/Codex usage.
   Complete after Worker deploy `95d3fcee-f740-477d-b3c4-368f198e8255`:
   public counter/model-mix read live, internal canary/heartbeat stayed green
   without requiring public counter movement, and Pylon-Codex exact rows remain
   included in the model mix.

## Remaining Risks

- Manual multi-assignment supervision is still too easy to mess up. The next
  improvement should be an explicit Pylon command that accepts N assignment refs,
  runs up to the advertised local capacity, streams per-assignment lifecycle
  state, and exits nonzero if any assignment fails to close.
- Local process liveness is best-effort on the same machine. It is adequate for
  local Pylon recovery, but it is not a distributed lease system.
- Legacy accepted rows without owner heartbeat are reclaimed by a short local
  fallback TTL. That is necessary to recover abandoned rows, but old Pylon
  versions cannot prove they are still alive. Operators should update Pylon
  before running large parallel batches.
- Server dispatch still depends on local Pylon closeout to learn that a
  non-expired active lease was interrupted. That is correct, but it means
  network failures during stale closeout can still leave server capacity blocked
  until retry or server expiry.
- Public counters are closeout/accounting projections. They are not live
  streaming progress indicators for in-flight Codex work.

## Operator Guidance

When delegation looks wedged:

1. Check advertised capacity:
   `pylon provider go-online --json` and `pylon presence heartbeat --json`.
2. Poll assignments with the same token/Pylon owner scope.
3. Inspect local assignment state only for local recovery evidence; do not paste
   raw state into public issues.
4. Prefer explicit assignment refs when running parallel no-spend tasks.
5. Treat any accepted non-expired lease without progress/closeout as a recovery
   target, not as proof of active work.
6. Verify exact token rows and owner-only traces after closeout.

## Follow-Up Work

- Add a first-class `pylon assignment run-many-no-spend --assignment-ref ...`
  command that manages parallelism internally.
- Add an admin/operator assignment health endpoint that summarizes active,
  stale, expired, closeout-submitted, and token-proof state without requiring
  raw D1 queries.
- Add an assignment-ref to exact-token-proof CLI command so agents do not use
  public counter movement as a proxy for closeout proof.
- Add a compact public-safe "why dispatch refused" explanation that includes
  active capacity, active assignment refs, expiry windows, and caller/Pylon
  ownership status.
