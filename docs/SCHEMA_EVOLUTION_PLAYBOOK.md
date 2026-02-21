# Zero-Downtime Schema Evolution Playbook (Rust Era)

Status: Active  
Applies to: `apps/openagents.com` control service, `apps/runtime`, shared proto contracts  
Issue: OA-RUST-090 (`#1925`)

## Purpose

Define the required expand/migrate/contract sequence for database and proto changes while mixed service versions are live.

## Non-Negotiable Invariants

1. Proto contracts are additive-first and governed by Buf checks.
2. `control.*` and `runtime.*` authority planes evolve independently; no cross-plane SQL joins in production code.
3. Compatibility windows must cover old+new binaries during rollout.
4. Rollbacks must be possible without emergency hotfix migrations.

## Change Classes

1. DB schema changes (`control.*`, `runtime.*` tables/indexes/constraints).
2. Proto contract changes (`proto/openagents/**`).
3. Service behavior changes (read/write path toggles, feature flags, contract usage).

## Expand -> Migrate -> Contract Lifecycle

### 1) Expand (safe with old binaries still running)

1. Add nullable columns, additive tables, additive indexes.
2. Add proto fields/messages/enums only (no removals or renames in place).
3. Ship tolerant readers/writers that can handle both old and new representations.
4. Run compatibility gates:
   - `buf lint`
   - `buf breaking --against '.git#branch=main,subdir=proto'`
   - `./scripts/verify-proto-generate.sh`

### 2) Migrate (old+new coexistence window)

1. Backfill new columns/tables in idempotent batches.
2. Dual-read or shadow-read until parity is verified.
3. Flip write path to new fields only after parity checks pass.
4. Keep fallback read path for rollback window.

### 3) Contract (only after compatibility window closes)

1. Remove legacy writes.
2. Remove legacy reads.
3. Drop legacy columns/tables/feature flags in a later release.
4. Update docs/runbooks and close migration tasks.

## Deployment Sequencing Matrix (Mixed Versions)

| Step | Schema state | Control service | Runtime service | Allowed? | Notes |
| --- | --- | --- | --- | --- | --- |
| 1 | pre-expand | old | old | Yes | Baseline |
| 2 | expanded | old | old | Yes | Expand must be backward compatible |
| 3 | expanded | new | old | Yes | New binaries must tolerate old data |
| 4 | expanded | old | new | Yes | Runtime/control roll independently |
| 5 | expanded + backfilled | new | new | Yes | Dual-read/validation window |
| 6 | contracted | new | new | Yes | Only after rollback window closes |
| 7 | contracted | old | any | No | Old binaries cannot run post-contract |

## Rollback and Abort Criteria

Abort migration and hold rollout when any of the following occur:

1. Migration job fails or drift check fails.
2. Contract checks fail (Buf/verification scripts).
3. Error rate/SLO regression exceeds rollout threshold.
4. Data parity checks fail during dual-read/backfill.

Rollback rules:

1. Roll back binaries first (to prior compatible release).
2. Do not run destructive schema rollback during incident response.
3. Keep expanded schema until safe contract cleanup window.

## Concrete Examples

### Runtime example (`runtime.*`)

Goal: add new delivery metadata field for Khala diagnostics.

1. Expand: add nullable DB column + additive proto field.
2. Deploy runtime reading old/new shapes; writing old shape.
3. Enable dual-write to new column; verify parity.
4. Switch readers to new column.
5. Contract: remove legacy column/read path in later release.

### Control example (`control.*`)

Goal: add device-session provenance field.

1. Expand: add nullable session column + additive API/proto field.
2. Deploy control service tolerant readers.
3. Backfill existing sessions from audit data.
4. Switch write path to require provenance on new sessions.
5. Contract: remove legacy fallback behavior after support window.

## Staging Drill Checklist Template

Use this before production rollout:

1. Confirm target issue/ADR links and migration owner.
2. Run contract gates (Buf + proto verification).
3. Apply expand migration in staging.
4. Deploy mixed versions (`new control + old runtime`, then `old control + new runtime`).
5. Run smoke tests on control + runtime APIs.
6. Run parity/backfill verification.
7. Exercise rollback to previous binary version (without schema rollback).
8. Record results and signoff from control/runtime maintainers.

Suggested report path:

- `apps/runtime/docs/reports/<date>-schema-evolution-drill.md`

## Required Runbook References

1. Runtime Cloud Run deploy: `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
2. Control service canary/rollback: `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
3. Compatibility policy: `docs/protocol/COMPATIBILITY_NEGOTIATION_POLICY.md`
