# Pylon v0.3 Live Worker Loop Smoke

Date: 2026-06-10

Issue: `OpenAgentsInc/openagents#4642`

Registry version during run: `2026-06-10.4`

## Command

```bash
cd apps/pylon
PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT=false bun run smoke:live-worker-loop
```

## Result

Status: `partial`

Pylon ref:

```text
pylon.codex.live_smoke.20260610075941
```

Passed steps:

- `smoke.pylon.register`
- `smoke.pylon.heartbeat`
- `smoke.pylon.wallet_readiness`
- `smoke.pylon.assignments_read`

Skipped steps:

- `skip.pylon.assignment_create.admin_token_missing`

Blockers:

- `blocker.pylon.live_worker_loop.no_assignment_available`

## Live Public Stats Check

Immediately after the smoke, a cache-busted public stats read returned:

```json
{
  "pylonsOnlineNow": 1,
  "sellablePylonsOnlineNow": 1,
  "pylonsWalletReadyNow": 1,
  "pylonsAssignmentReadyNow": 1
}
```

The recent Pylon row for `pylon.codex.live_smoke.20260610075941` reported:

- client version `openagents.pylon@0.3.0-rc1`
- runtime state `online`
- products:
  - `capability.public.background_loop`
  - `capability.public.probe_runtime`
  - `capability.public.pylon_cli`

## Remaining Lane B Work

This run did not create or settle a GEPA assignment. The live paid portion of
#4642 still requires explicit operator approval for spend, an operator-created
assignment, a settled paid GEPA assignment receipt, transition receipts before
registry edits, and a deployed registry version bump.

No spend was approved, no sats moved, no transition receipt was recorded, and
no registry edit was made in this partial smoke.

## Recheck: 2026-06-10 10:25 UTC

Command:

```bash
cd apps/pylon
PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT=false bun run smoke:live-worker-loop
```

Result:

- status: `partial`
- pylon ref: `pylon.codex.live_smoke.20260610102527`
- passed steps: `smoke.pylon.register`, `smoke.pylon.heartbeat`,
  `smoke.pylon.wallet_readiness`, `smoke.pylon.assignments_read`
- skipped: `skip.pylon.assignment_create.admin_token_missing`
- blocker: `blocker.pylon.live_worker_loop.no_assignment_available`

The smoke exited nonzero because the no-admin-token path is partial by design:
it can prove live registration/readiness and assignment read, but cannot create
or settle a GEPA assignment.

Immediately after the smoke, a cache-busted public stats read returned:

```json
{
  "pylonsOnlineNow": 1,
  "sellablePylonsOnlineNow": 1,
  "pylonsWalletReadyNow": 1,
  "pylonsAssignmentReadyNow": 1,
  "nip90MarketSettlementStats.compute.jobsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.satsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.receiptRefs": []
}
```

No operator spend was approved, no admin assignment dispatch token was present,
no paid GEPA assignment was created, no settlement receipt exists, no transition
receipt was recorded, and no registry edit was made.

## Recheck: 2026-06-11 01:10 UTC

Issue: `OpenAgentsInc/openagents#4642`

Registry version during live read: `2026-06-10.27`

Command:

```bash
cd apps/pylon
PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT=true bun run smoke:live-worker-loop
```

Result:

- status: `passed`
- pylon ref: `pylon.codex.live_smoke.20260611011036`
- assignment ref: `assignment.public.pylon_runtime_gate.20260611011036`
- passed steps: `smoke.pylon.register`, `smoke.pylon.heartbeat`,
  `smoke.pylon.wallet_readiness`, `smoke.pylon.assignment_create`,
  `smoke.pylon.assignments_read`, `smoke.pylon.assignment_accept`,
  `smoke.pylon.assignment_progress`, `smoke.pylon.artifacts`,
  `smoke.pylon.operator_closeout`
- skipped: none
- blockers: none

Immediately after the smoke, a cache-busted public stats read returned:

```json
{
  "pylonsOnlineNow": 3,
  "sellablePylonsOnlineNow": 2,
  "pylonsWalletReadyNow": 2,
  "pylonsAssignmentReadyNow": 2,
  "earningLaunchGate.state": "ready",
  "nip90MarketSettlementStats.compute.jobsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.satsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.receiptRefs": []
}
```

Transition receipt recorded before the registry edit:

```text
promise_transition_d0f7edc5-1688-4039-bcdf-8971b79512ef
```

The transition receipt is a same-state exception because
`pylon.gepa_worker_loop_v03.v1` remains yellow: this run clears the live
no-spend endpoint smoke blocker but does not clear paid settlement. No spend
was approved, no sats moved, no paid GEPA assignment was created, and no
settlement receipt exists.

## Recheck: 2026-06-10 12:09 UTC

Command:

```bash
cd apps/pylon
PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT=false bun run smoke:live-worker-loop
```

Result:

- status: `partial`
- pylon ref: `pylon.codex.live_smoke.20260610120934`
- passed steps: `smoke.pylon.register`, `smoke.pylon.heartbeat`,
  `smoke.pylon.wallet_readiness`, `smoke.pylon.assignments_read`
- skipped: `skip.pylon.assignment_create.admin_token_missing`
- blocker: `blocker.pylon.live_worker_loop.no_assignment_available`

The smoke exited with code 2 because the no-admin-token path is partial by
design.

Immediately after the smoke, a cache-busted public stats read returned:

```json
{
  "pylonsOnlineNow": 1,
  "sellablePylonsOnlineNow": 1,
  "pylonsWalletReadyNow": 1,
  "pylonsAssignmentReadyNow": 1,
  "nip90MarketSettlementStats.compute.jobsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.satsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.receiptRefs": []
}
```

No operator spend was approved, no admin assignment dispatch token was present,
no paid GEPA assignment was created, no settlement receipt exists, no transition
receipt was recorded, and no registry edit was made.

## Recheck: 2026-06-10 22:53 UTC

Issue: `OpenAgentsInc/openagents#4642`

Registry version during live read: `2026-06-10.25`

Command:

```bash
cd apps/pylon
PYLON_LIVE_SMOKE_CREATE_ASSIGNMENT=false bun run smoke:live-worker-loop
```

Result:

- status: `partial`
- pylon ref: `pylon.codex.live_smoke.20260610225333`
- passed steps: `smoke.pylon.register`, `smoke.pylon.heartbeat`,
  `smoke.pylon.wallet_readiness`, `smoke.pylon.assignments_read`
- skipped: `skip.pylon.assignment_create.admin_token_missing`
- blocker: `blocker.pylon.live_worker_loop.no_assignment_available`

Immediately after the smoke, a cache-busted public stats read returned:

```json
{
  "pylonsOnlineNow": 1,
  "sellablePylonsOnlineNow": 1,
  "pylonsWalletReadyNow": 1,
  "pylonsAssignmentReadyNow": 1,
  "nip90MarketSettlementStats.compute.jobsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.satsSettledTotal": 0,
  "nip90MarketSettlementStats.compute.receiptRefs": []
}
```

The endpoint smoke still proves only registration, heartbeat, wallet-readiness
projection, and assignment-list reachability. It does not clear
`blocker.product_promises.live_openagents_gepa_endpoint_smoke_missing` by
itself because no operator-created assignment was available to accept,
progress, artifact-submit, and close out.

No operator spend was approved, no admin assignment dispatch token was present,
no paid GEPA assignment was created, no settlement receipt exists, no transition
receipt was recorded, and no registry edit was made.
