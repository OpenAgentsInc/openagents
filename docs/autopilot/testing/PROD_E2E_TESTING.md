# Production E2E Testing (Rust Surfaces)

This runbook is the active production/staging verification path for Rust services.

## Required checks

1. Control service health/readiness:
```bash
OPENAGENTS_BASE_URL=https://staging.openagents.com apps/openagents.com/service/deploy/smoke-health.sh
OPENAGENTS_BASE_URL=https://staging.openagents.com apps/openagents.com/service/deploy/smoke-control.sh
```

2. Cutover matrix (staging + prod lanes):
```bash
scripts/release/validate-rust-cutover.sh
```

3. Runtime deploy gate (when runtime changes):
```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=openagents-runtime \
MIGRATE_JOB=openagents-runtime-migrate \
apps/runtime/deploy/cloudrun/run-migrate-job.sh
```

## Production policy

1. Test staging first.
2. Keep production rollback path available.
3. Record evidence in `docs/reports/`.

## Correlation IDs

Use `x-request-id` and `x-cloud-trace-context` from responses for log correlation.
