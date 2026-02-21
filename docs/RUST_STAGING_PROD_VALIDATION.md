# Rust Staging/Prod Validation Matrix

Status: active  
Owner: `owner:infra`, `owner:openagents.com`, `owner:runtime`  
Issue: OA-RUST-110 (`#1935`)

## Purpose

Define and execute one deterministic validation matrix that gates Rust cutover readiness for staging and production.

## Canonical entrypoint

Run from repo root:

```bash
scripts/release/validate-rust-cutover.sh
```

Artifacts are written to:

- `docs/reports/rust-cutover-validation/<timestamp>/results.tsv`
- `docs/reports/rust-cutover-validation/<timestamp>/SUMMARY.md`

## Matrix lanes

1. Control service (staging + prod)
- health/readiness: `/healthz`, `/readyz`
- static host policy checks: `manifest.json`, `sw.js`, immutable assets
- auth/session/sync token checks via `smoke-control.sh` when access token is provided

2. Runtime service
- runtime health + authority API smoke (`runtime-smoke`)
- migration drift check (`check-migration-drift.sh`) validates service image == migrate-job image and latest successful execution alignment

3. Khala contract lane
- `cargo test --manifest-path apps/runtime/Cargo.toml server::tests::khala_topic_messages -- --nocapture`

4. Surface parity lane
- optional cross-surface harness (`scripts/run-cross-surface-contract-harness.sh`)

5. Observability + rollback readiness
- Cloud Logging error probes for control + runtime
- control canary status probe (`canary-rollout.sh status`)

## Required environment

Defaults:

- `STAGING_CONTROL_BASE_URL=https://staging.openagents.com`
- `PROD_CONTROL_BASE_URL=https://openagents.com`
- `GCP_PROJECT=openagentsgemini`
- `GCP_REGION=us-central1`
- `CONTROL_SERVICE=openagents-control-service`
- `RUNTIME_SERVICE=runtime`
- `MIGRATE_JOB=runtime-migrate`

Provide runtime URLs when available:

- `STAGING_RUNTIME_BASE_URL`
- `PROD_RUNTIME_BASE_URL`

Optional auth tokens for authenticated control-smoke routes:

- `STAGING_CONTROL_ACCESS_TOKEN`
- `PROD_CONTROL_ACCESS_TOKEN`

If `staging.openagents.com` domain mapping is still cert-pending or not mapped to Rust staging service, set `STAGING_CONTROL_BASE_URL` to the staging service URL (for example `https://openagents-control-service-staging-<hash>-uc.a.run.app`) and record domain-mapping status in the validation evidence.

## Current infrastructure compatibility mode

The script supports explicit service/base-url overrides so validation can run during transition periods where Cloud Run services still use legacy names.

Example transition invocation:

```bash
CONTROL_SERVICE=openagents-web \
RUNTIME_SERVICE=openagents-runtime \
MIGRATE_JOB=runtime-migrate \
STAGING_CONTROL_BASE_URL=https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app \
PROD_CONTROL_BASE_URL=https://openagents-web-ezxz4mgdsq-uc.a.run.app \
PROD_RUNTIME_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app \
FAIL_ON_REQUIRED_FAILURE=0 \
scripts/release/validate-rust-cutover.sh
```

`FAIL_ON_REQUIRED_FAILURE=0` is allowed for pre-cutover baseline capture only. Production gate decisions must run with `FAIL_ON_REQUIRED_FAILURE=1`.

## Go/No-Go policy

1. All required checks must pass.
2. Required checks cannot be skipped in release gating mode.
3. Any required failure forces no-go until fixed and rerun.
4. Each run must produce a committed report reference in `docs/reports/`.
5. Legacy deletion phases require fresh Laravel DB backup artifacts (`scripts/release/backup-laravel-db.sh`) in addition to this validation matrix.

## Rollback references

- Control canary/rollback: `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Runtime deploy/migrate rollback discipline: `apps/runtime/docs/DEPLOY_CLOUD_RUN.md`
- Schema evolution and expand/migrate/contract: `docs/SCHEMA_EVOLUTION_PLAYBOOK.md`
