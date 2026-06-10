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
