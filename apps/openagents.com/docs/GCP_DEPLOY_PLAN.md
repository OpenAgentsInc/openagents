# GCP Deploy Plan (Rust Active Path)

Status: active

The active production web deploy path is Rust-only:

- Service: `openagents-control-service`
- Deploy helper: `apps/openagents.com/service/deploy/deploy-production.sh`
- Canary/rollback runbook: `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
- Cloud Build config: `apps/openagents.com/service/deploy/cloudbuild.yaml`
- Runtime image recipe: `apps/openagents.com/Dockerfile` (Rust-only)

Build/push control-service image (Rust-only, no PHP/Composer):

```bash
gcloud builds submit \
  --config apps/openagents.com/service/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  .
```

Legacy Laravel Cloud Build / PHP-FPM deployment guidance has been archived to:

- `apps/openagents.com/docs/archived/legacy-laravel-deploy/GCP_DEPLOY_PLAN.md`

Do not use the archived Laravel deploy plan for current production deployments.
