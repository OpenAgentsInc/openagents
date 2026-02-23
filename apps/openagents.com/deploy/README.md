# Deploy Entry Points (Rust-Only Active Lane)

Active web deploy lane is Rust-only:

- Deploy helper: `apps/openagents.com/service/deploy/deploy-production.sh`
- Staging deploy runbook: `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`
- Canary/rollback helper: `apps/openagents.com/service/deploy/canary-rollout.sh`
- Maintenance mode helper: `apps/openagents.com/service/deploy/maintenance-mode.sh`
- Health smoke check: `apps/openagents.com/service/deploy/smoke-health.sh`
- Control/API + static-host smoke check: `apps/openagents.com/service/deploy/smoke-control.sh`
- Cloud Build config: `apps/openagents.com/service/deploy/cloudbuild.yaml`
- Runtime image recipe: `apps/openagents.com/Dockerfile` (Rust-only)

Compatibility wrapper:

- `apps/openagents.com/deploy/deploy-production.sh` forwards to the Rust deploy helper.

Optional staging deploy helper (no-traffic revision):

- `apps/openagents.com/service/deploy/deploy-staging.sh` (runs local verification gates and creates a no-traffic revision; use the staging runbook to shift traffic).

Legacy Laravel deploy assets were archived for audit-only reference under:

- `apps/openagents.com/deploy/archived-laravel/`
- including legacy Dockerfile: `apps/openagents.com/deploy/archived-laravel/Dockerfile`
- freeze policy: `apps/openagents.com/deploy/archived-laravel/README.md`

Do not use archived assets for production deploys.
