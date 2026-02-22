# Rust Ownership Backfill and Rollback Runbook (OA-WEBPARITY-011)

This runbook defines the idempotent migration/backfill path for Rust-owned store snapshots and the rollback procedure.

## Scope

Rust-owned store files:
- Auth store (`OA_AUTH_STORE_PATH`)
- Codex thread store (`OA_CODEX_THREAD_STORE_PATH`)
- Cross-domain store (`OA_DOMAIN_STORE_PATH`)

Migration utility:
- `apps/openagents.com/service/src/bin/rust_store_migrate.rs`

Wrapper scripts:
- `apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh`
- `apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh`
- `apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh`

## Goals

- Ensure store schema keys are present for the current Rust service release.
- Normalize next-id counters from existing stored rows.
- Create deterministic checksums for each migrated store.
- Produce a manifest + backups that support one-command rollback.

## Migration Model (Expand / Migrate / Contract)

1. Expand:
- Deploy Rust code that can read both pre-migration and post-migration store shapes.
- Do not remove backward-compatible reads in the same deploy.

2. Migrate:
- Run `run-rust-ownership-backfill.sh` once per environment.
- This step is idempotent and can be rerun safely.

3. Verify:
- Run `verify-rust-ownership-backfill.sh <manifest-path>`.
- Confirm checksum and count evidence before route-group cutover.

4. Contract:
- After sustained healthy canary, remove backward-compat fallback logic in a later release.

## Commands

From repo root:

```bash
apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh
```

Optional explicit paths:

```bash
AUTH_STORE_PATH=/var/lib/openagents/auth-store.json \
CODEX_THREAD_STORE_PATH=/var/lib/openagents/codex-thread-store.json \
DOMAIN_STORE_PATH=/var/lib/openagents/domain-store.json \
apps/openagents.com/service/scripts/run-rust-ownership-backfill.sh
```

Verify:

```bash
apps/openagents.com/service/scripts/verify-rust-ownership-backfill.sh \
  apps/openagents.com/storage/app/rust-store-migrate/manifests/<timestamp>.json
```

Rollback:

```bash
apps/openagents.com/service/scripts/rollback-rust-ownership-backfill.sh \
  apps/openagents.com/storage/app/rust-store-migrate/manifests/<timestamp>.json
```

## Mixed-Version Safety Rules

- Never run contract cleanup in the same deployment as backfill.
- Keep at least one prior backup manifest available for immediate rollback.
- During mixed-version windows, only additive schema transitions are allowed.
- Rollback is file-restore only; no destructive truncation is performed.

## Invariant Checks

Required before marking migration complete:
- Manifest exists and includes all targeted stores.
- `after_sha256` in manifest matches on-disk file hashes.
- Store row counts are non-negative and reasonable for environment baseline.

## Failure Handling

Use rollback immediately if:
- checksum verification fails,
- the service fails to deserialize a migrated store,
- unexpected auth/thread/domain data drops are detected.

After rollback, re-run migration in a staging-like environment and compare manifests before retrying production.
