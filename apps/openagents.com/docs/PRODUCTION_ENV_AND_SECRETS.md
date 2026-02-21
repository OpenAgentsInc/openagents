# Production Env and Secrets (Rust Active Path)

Status: active

Use Rust control-service deployment/configuration flow:

1. Build/deploy with `apps/openagents.com/service/deploy/deploy-production.sh`.
2. Manage staged rollout with `apps/openagents.com/service/deploy/canary-rollout.sh`.
3. Build and push image with `apps/openagents.com/service/deploy/cloudbuild.yaml`.
4. Follow operational checks in `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`.
5. Run deploy smoke checks:
   - `apps/openagents.com/service/deploy/smoke-health.sh`
   - `apps/openagents.com/service/deploy/smoke-control.sh`

Legacy Laravel-specific secret wiring and `openagents-web` service guidance has been archived to:

- `apps/openagents.com/docs/archived/legacy-laravel-deploy/PRODUCTION_ENV_AND_SECRETS.md`

Do not use the archived Laravel env/secrets flow for current production deployments.
