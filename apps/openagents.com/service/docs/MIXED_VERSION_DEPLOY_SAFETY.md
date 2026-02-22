# Mixed-Version Deploy Safety, Rollback, and Backfill Invariants (OA-WEBPARITY-067)

Date: 2026-02-22  
Status: active  
Owner: openagents.com platform

This runbook defines required rules and verification gates while Laravel and Rust lanes overlap during cutover windows.

## Scope

- Rust store migration and rollback wrappers:
  - `apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh`
  - `apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh`
  - `apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh`
- Route-group rollback matrix:
  - `apps/openagents.com/service/docs/ROUTE_SPLIT_ROLLBACK_MATRIX.md`
- Rust ownership migration internals:
  - `apps/openagents.com/service/src/bin/rust_store_migrate.rs`

## Mixed-Version Rules (Non-negotiable)

1. Expand first: deploy backward-compatible Rust readers before any migration or contract cleanup.
2. Migrate second: run backfill with manifest + backups, then verify checksums and row-count invariants.
3. Contract last: remove fallback behavior only after sustained healthy canary and rollback evidence.
4. Rollback must stay available for the entire mixed-version window:
   - route-level rollback to legacy target,
   - store rollback from the same migration manifest.
5. No destructive schema/data operations are allowed during mixed-version overlap.
6. Cutover for each route group requires signed evidence for checksum and count invariants.

## Expand / Migrate / Contract Procedure

### 1) Expand

- Deploy Rust release that can read both pre-migration and migrated store shapes.
- Confirm route-split rollback target exists for each cutover domain.

### 2) Migrate

```bash
apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh
```

Expected output:
- manifest path under `apps/openagents.com/storage/app/rust-store-migrate/manifests/`
- backup directory under `apps/openagents.com/storage/app/rust-store-migrate/backups/`

### 3) Verify

```bash
apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh \
  apps/openagents.com/storage/app/rust-store-migrate/manifests/<timestamp>.json
```

Required invariant checks:
- every migrated store has a deterministic `after_sha256`,
- expected count keys are present per store,
- counts are consistent with baseline expectations for that environment.

Example count inspection:

```bash
jq '.stores[] | {store, counts}' \
  apps/openagents.com/storage/app/rust-store-migrate/manifests/<timestamp>.json
```

### 4) Cutover / Rollback Safety

- Apply route-split canary changes only after verification passes.
- If any gate fails, rollback immediately:

```bash
apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh \
  apps/openagents.com/storage/app/rust-store-migrate/manifests/<timestamp>.json
```

Then apply route rollback mapping from `ROUTE_SPLIT_ROLLBACK_MATRIX.md`.

### 5) Contract

- Remove backward-compat reads in a later release only after canary window is stable and rollback drill evidence exists.

## Required Evidence Per Cutover

1. Migration manifest JSON path and artifact retention confirmation.
2. Verification output proving checksum parity for all target stores.
3. Count snapshot (`jq` output) signed off by platform owner.
4. Rollback drill output (store restore + route rollback) from same release train.
5. Issue-linked report artifact from `run-mixed-version-deploy-safety-harness.sh`.

## CI/Local Harness

Run:

```bash
./apps/openagents.com/scripts/run-mixed-version-deploy-safety-harness.sh
```

Artifact path:
- `apps/openagents.com/storage/app/mixed-version-deploy-safety/<timestamp>/summary.json`

The harness verifies:
- migration script integrity,
- migration manifest structure/checksums/counts,
- rollback hash restoration,
- runbook presence for operational use.
