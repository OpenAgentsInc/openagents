# Runtime Deploy Jobs

This directory provides deploy-time database migration and post-deploy smoke-test gates.

## Files

- `migration-job.yaml`
  - Runs Rust SQL migrations with `runtime-migrate`.
- `smoke-job.yaml`
  - Runs Rust HTTP smoke checks with `runtime-smoke`.
- `run-postdeploy-gate.sh`
  - Orchestrates migration job then smoke job and fails fast on error.

## Usage

From repo root:

```bash
NAMESPACE=staging \
IMAGE=us-central1-docker.pkg.dev/<project>/runtime/runtime:<tag> \
apps/runtime/deploy/jobs/run-postdeploy-gate.sh
```

## Smoke coverage

The smoke job validates:

1. `GET /healthz` returns healthy response.
2. `GET /readyz` returns ready response.
3. Runtime authority path creates a run, appends an event, and fetches run state.

## Required Secrets

- `DATABASE_URL`

Migration job reads `DATABASE_URL` from `runtime-secrets` in the target namespace.
