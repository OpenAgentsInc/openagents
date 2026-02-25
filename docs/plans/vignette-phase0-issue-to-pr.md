# Phase 0 Vignette (Execution Authority): Issue -> Verified PR (Gate L)

Status: active  
Last updated: 2026-02-23

This document is the **Phase 0 execution authority** for Gate L.  
Gate L is **green** iff the Phase 0 harness passes:

```bash
./scripts/vignette-phase0-issue-to-pr.sh
```

Hard constraints (must hold in implementation and harness):

- Authority mutations are authenticated HTTP only (`INV-02`).
- Live delivery/sync lanes are WebSocket only (`INV-03`).
- Idempotency + replay semantics are enforced for all authority mutations (`INV-07`).
- Nostr is **interop only** in Phase 0: the Bridge mirrors **provider ads + receipt pointers only** (no job chatter/streaming over Nostr).

## A) Preconditions

- User has an Autopilot identity (account) and a connected repo (fixture is acceptable for local harness).
- A known issue exists for the repo fixture (deterministic bug; deterministic fix).
- At least one **account-attached device/provider** (Zone 0.5) is enrolled with **explicit caps** and can be disabled instantly.
- A platform-managed budget exists and supports: reserve -> release/withhold on verify (idempotent).

## B) Actors

- Autopilot Desktop (executor)
- Nexus (registry + live lanes)
- OpenAgents Compute Provider (daemon)
- Verifier (objective)
- Treasury (budget reservation + pay-after-verify release)
- Bridge (Nostr mirror for ads + receipt pointers)

## C) Steps (Each Step Requires Artifacts + Receipts)

All receipts are **deterministic**, **idempotent**, and appear either:

- in the session bundle `RECEIPT.json` (preferred), and/or
- as runtime authority events mirrored into `REPLAY.jsonl`.

### 1) Enroll device (client + provider) + set caps

Required receipts:

- `DeviceEnrolled` (device_id, roles, pubkey/fingerprint if present)
- `CapsSet` (cpu/ram/gpu/network/time caps; default-off provider mode)

Required assertions:

- device is visible as connected/online in the Nexus registry view (API is sufficient in Phase 0 harness)
- presence stream exists via Spacetime delivery on `fleet:user:<user_id>:workers` (WS/poll), emitting worker snapshots as devices enroll/heartbeat/stop
- provider enable/disable is instant and blocks new work

### 2) Create run from issue

Required receipts:

- `IssueClaimed` (issue_id, claim_identity)
- `RunCreated` (run_id/session_id, repo_ref, issue_ref)

### 3) Dispatch `oa.sandbox_run.v1` to provider (idempotent)

Required receipts:

- `JobDispatched` (job_type=`oa.sandbox_run.v1`, job_hash, provider_id, idempotency_key)

Required assertions:

- retrying dispatch with the same idempotency_key does not duplicate side effects or settlement

### 4) Provider executes + returns artifacts

Required artifacts (per command):

- stdout/stderr hashes (sha256)
- exit code + duration
- environment manifest (provider id/version, OS/runtime, sandbox config)

Required receipts:

- `JobCompleted` or `JobFailed` (job_hash, artifact hashes, failure reason if any)

### 5) Verifier checks deterministically (objective)

Required receipts:

- `VerificationPassed` or `VerificationFailed` (job_hash, verifier version, reason)

### 6) Settlement: reserve -> release on pass (withhold on fail)

Required receipts:

- `BudgetReserved` (scope, amount/cap, reservation_id, idempotency_key)
- `PaymentReleased` on pass, or `PaymentWithheld` on fail (job_hash, amount, proof pointer)

Required assertions:

- verification failure implies no release
- retries do not double-spend / double-release
- stuck reservations are reconciled: if a job remains `Reserved` beyond the configured TTL, treasury reconciliation withholds and releases the reservation (no budget gets permanently locked)

### 7) Autopilot updates PR + posts status

Required artifacts:

- Verified Patch Bundle in a stable session dir:
  - `PR_SUMMARY.md`
  - `RECEIPT.json`
  - `REPLAY.jsonl`

Required receipts:

- `ForgeUpdated` (branch/commit/pr_ref) or `ForgeUpdateSkipped` (if fixture mode)

### 8) Bridge emits Nostr events (Phase 0 minimal)

Required events (and only these classes in Phase 0):

- Provider ad mirror (capabilities + pricing bands; stable identity pointer)
- Receipt pointer mirror (verification + settlement receipt hashes/pointers)

Required assertions:

- no job chatter, progress streaming, or internal coordination is mirrored to Nostr in Phase 0

## D) Failure Cases (Must Be Explicitly Tested In Harness)

- Dispatch retry (idempotent): duplicate `JobDispatched` does not double-run or double-release
- Provider offline mid-run: job fails cleanly, provider is quarantined/penalized, no payment release
- Verification fails: no payment release; quarantine/penalty applied
- Emergency disable: disabling provider blocks new jobs immediately; in-flight behavior is recorded and deterministic

## E) Metrics Asserted (Harness Must Emit + Check)

- fill rate, median latency, effective cost
- provider breadth (count of eligible providers, even if only 1 in local harness)
- verification pass rate (overall + by provider)
- rework rate (accepted then reverted/fails downstream) (may be 0 in Phase 0 fixture)
- caps enforced proof: provider reports caps + observed usage and emits an enforcement receipt/event
