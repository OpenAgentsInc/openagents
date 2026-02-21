# 2026-02-21 Rust Staging/Prod Validation Baseline

Issue: OA-RUST-110 (`#1935`)  
Timestamp (UTC): 2026-02-21T19:27:46Z  
Harness: `scripts/release/validate-rust-cutover.sh`

## Invocation

```bash
CONTROL_SERVICE=openagents-web \
RUNTIME_SERVICE=openagents-runtime \
MIGRATE_JOB=runtime-migrate \
STAGING_CONTROL_BASE_URL=https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app \
PROD_CONTROL_BASE_URL=https://openagents-web-ezxz4mgdsq-uc.a.run.app \
STAGING_RUNTIME_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app \
PROD_RUNTIME_BASE_URL=https://openagents-runtime-ezxz4mgdsq-uc.a.run.app \
FAIL_ON_REQUIRED_FAILURE=0 \
scripts/release/validate-rust-cutover.sh
```

Artifacts:

- `docs/reports/rust-cutover-validation/20260221T192746Z/SUMMARY.md`
- `docs/reports/rust-cutover-validation/20260221T192746Z/results.tsv`
- `docs/reports/rust-cutover-validation/20260221T192746Z/logs/`

## Result

Overall: `failed` (`7` required failures, `4` required passes, `1` optional skipped).

Required failures captured:

1. Control staging/prod health and full smoke checks fail with `404` on `/healthz` and `/readyz`.
2. Runtime staging/prod smoke checks fail with `404` on runtime `/healthz`.
3. Runtime migration drift check fails because Cloud Run job `runtime-migrate` is not present in current service namespace.

Required passes captured:

1. Khala runtime contract tests pass (`server::tests::khala_topic_messages`).
2. Control and runtime error log probes execute successfully.
3. Control canary status probe executes successfully (`openagents-web` revisions visible).

## Go/No-Go

Current state is `NO-GO` for Rust cutover gate because required checks are failing.

Blocking actions before re-run in strict mode (`FAIL_ON_REQUIRED_FAILURE=1`):

1. Deploy Rust control service revision that exposes `/healthz` and `/readyz` on staging + production target services/domains.
2. Deploy Rust runtime revision that exposes expected runtime smoke endpoints.
3. Create/align runtime migration job and service naming to configured `MIGRATE_JOB`.
4. Re-run matrix with canonical Rust service names after cutover alignment.
