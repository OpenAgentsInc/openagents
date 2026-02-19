# Runtime Deploy Jobs

This directory provides deploy-time database migration and post-deploy smoke-test gates.

## Files

- `migration-job.yaml`
  - Runs release migrations with `OpenAgentsRuntime.Release.migrate/0`.
- `smoke-job.yaml`
  - Runs HTTP + runtime-path smoke checks with `OpenAgentsRuntime.Deploy.Smoke.run!/1`.
- `run-postdeploy-gate.sh`
  - Orchestrates migration job then smoke job and fails fast on error.

## Usage

From repo root:

```bash
NAMESPACE=staging \
IMAGE=us-central1-docker.pkg.dev/<project>/openagents-runtime/runtime:<tag> \
apps/openagents-runtime/deploy/jobs/run-postdeploy-gate.sh
```

## Smoke coverage

The smoke job validates:

1. `GET /internal/v1/health` returns healthy response.
2. Stream path emits delta + `[DONE]` for a seeded run.
3. Tool execution path persists task lifecycle and `tool.call` / `tool.result` events.

## Required Secrets

- `DATABASE_URL`
- `SECRET_KEY_BASE`
- `RUNTIME_SIGNATURE_SECRET`

All jobs read these from `openagents-runtime-secrets` in the target namespace.
