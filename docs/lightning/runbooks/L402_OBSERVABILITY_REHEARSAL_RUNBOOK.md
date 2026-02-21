# L402 Observability + Rehearsal Runbook

## Purpose

This runbook defines a single, queryable correlation model for L402 execution across:

- hosted-node path (control plane, gateway, settlement, UI projection)
- local-node path (desktop executor, local wallet/runtime state, UI projection)

It is the operational guide for issue `#1594`.

## Canonical Observability Contract

All L402 telemetry records must conform to `L402ObservabilityRecord` in:

- `packages/lightning-effect/src/contracts/observability.ts`

Required fields (always present, nullable when unknown):

- `requestId`
- `userId`
- `paywallId`
- `taskId`
- `endpoint`
- `quotedCostMsats`
- `capAppliedMsats`
- `paidAmountMsats`
- `paymentProofRef`
- `cacheHit`
- `denyReason`
- `executor`
- `plane`
- `executionPath`
- `desktopSessionId`
- `desktopRuntimeStatus`
- `walletState`
- `nodeSyncStatus`
- `observedAtMs`

Domain enums:

- `executor`: `desktop | gateway | system`
- `plane`: `control | gateway | settlement | ui`
- `executionPath`: `local-node | hosted-node`
- `walletState`: `locked | unlocked | initializing | recovering`
- `nodeSyncStatus`: `syncing | synced | degraded`

## Emission Surfaces

Hosted dry run:

- `apps/lightning-ops/src/main.rs` (`smoke:observability`)
- CLI: `cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:observability --json --mode mock`
- Full-flow CLI (issue `#1604`): `cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:full-flow --json --mode mock --allow-missing-local-artifact`
- Full-flow artifacts: `output/lightning-ops/full-flow/<requestId>/events.jsonl` and `summary.json`

Local dry run:

- Legacy Electron local-node flow (removed with OA-RUST-053).
- Historical artifact path (for prior runs): `output/l402-local-node-smoke-artifact.json`

## Correlation Workflow

1. Worker request correlation:
   - read `x-oa-request-id` from response headers
   - tail Worker logs by `oa_req=<id>` (see `docs/autopilot/testing/PROD_E2E_TESTING.md`)
2. Khala correlation:
   - correlate by `requestId`, `taskId`, and `paywallId` in task/settlement records
3. Settlement proof correlation:
   - join on `paymentProofRef`
4. Desktop local correlation:
   - correlate by `desktopSessionId`, `taskId`, and `requestId` from local artifact
5. UI projection correlation:
   - check `plane=ui` records for the same `requestId/taskId/paymentProofRef`

## Rehearsal Checklist

### 1) Hosted Success (Full-flow)

Run:

```bash
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- \
  smoke:full-flow --json --mode mock --allow-missing-local-artifact \
  > output/l402-hosted-full-flow-summary.json
```

Verify:

- summary `ok=true`
- gateway probes succeeded (`challengeOk`, `proxyOk`, `healthOk`)
- success and deny scenarios are both present:
  - `paidRequest.status=paid`
  - `policyDeniedRequest.status=denied`
- hosted observability includes no missing required keys
- hosted artifact parity keys include:
  - `executionPath`
  - `requestId`
  - `taskId`
  - `paymentProofRef`
- artifacts were written:
  - `events.jsonl`
  - `summary.json`

### 2) Local-Node Success (Legacy Path)

The prior Electron local-node rehearsal path was removed with `OA-RUST-053` (`apps/desktop` deletion).

Current operational stance:

1. Use hosted full-flow rehearsal as the active production gate.
2. Treat historical local-node artifacts as reference-only evidence until Rust-native local-node parity lanes are reintroduced under active issues.

### 3) Cache Reuse

Hosted dry-run settlement records include idempotent replay via `cacheHit=true` when an existing settlement is observed.

Verify in hosted output:

- at least one record where `cacheHit=true`

### 4) Policy Block

Hosted dry run includes security-gate denial modeling.

Verify:

- at least one record has non-null `denyReason`
- denial record is in `plane=control` with `executor=system`

### 5) Settlement Failure (Triage Drill)

Procedure:

1. capture a failing run request id
2. correlate Worker `oa_req=<id>`
3. locate settlement-plane record by `requestId`
4. confirm whether `paymentProofRef` is absent or mismatch across planes

Pass condition:

- operator can identify failing domain (`gateway` vs `settlement`) without code changes

### 6) Desktop Executor Offline (Triage Drill)

Procedure:

1. stop desktop executor / desktop app
2. run hosted or local request and capture `requestId/taskId`
3. verify local artifact shows stale or missing progress with local-node compatibility state

Pass condition:

- operator can classify failure as local executor availability issue using record fields only

## Incident Triage Decision Tree

1. `denyReason` present in `plane=control`:
   - policy/security gate denied before payment
2. no `paymentProofRef` and `plane=gateway` unhealthy:
   - challenge/proxy/gateway path issue
3. `paymentProofRef` present in settlement but missing in `plane=ui`:
   - projection/UI read path issue
4. `executionPath=local-node` and `nodeSyncStatus=degraded`:
   - local runtime/wallet/node health issue

## Programmatic Verification

Run these before sign-off:

```bash
cargo test --manifest-path apps/lightning-ops/Cargo.toml
cargo run --manifest-path apps/lightning-ops/Cargo.toml -- smoke:full-flow --json --mode mock --allow-missing-local-artifact
# apps/autopilot-worker and apps/web removed; run L402 e2e from apps/openagents.com or packages/effuse-test as applicable
cargo check -p autopilot-desktop
```

Store artifacts under `output/` for rehearsal evidence and incident replay.
