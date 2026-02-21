# Rust Service Deployment Matrix

Status: active  
Last updated: 2026-02-21

This is the canonical deploy/process map for active Rust service lanes.

Cross-environment release validation gate:

- `docs/RUST_STAGING_PROD_VALIDATION.md`

## Canonical deploy entrypoints

1. `openagents-control-service` (production)
   - Build image:
     - `gcloud builds submit --config apps/openagents.com/service/deploy/cloudbuild.yaml --substitutions _TAG=\"$(git rev-parse --short HEAD)\" .`
   - Deploy:
     - `PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:<TAG> apps/openagents.com/service/deploy/deploy-production.sh`
   - Rollout/rollback:
     - `apps/openagents.com/service/deploy/canary-rollout.sh`
2. `openagents-control-service-staging`
   - Deploy:
     - `PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service-staging IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:<TAG> apps/openagents.com/service/deploy/deploy-staging.sh`
   - Runbook:
     - `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`
3. `runtime`
   - Build image:
     - `gcloud builds submit --config apps/runtime/deploy/cloudbuild.yaml --substitutions _TAG=\"$(git rev-parse --short HEAD)\" .`
   - Deploy + mandatory migration:
     - `GCP_PROJECT=openagentsgemini GCP_REGION=us-central1 RUNTIME_SERVICE=runtime MIGRATE_JOB=runtime-migrate IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh`
   - Mandatory migrate rerun helper:
     - `apps/runtime/deploy/cloudrun/run-migrate-job.sh`
4. `lightning-ops`
   - Active lane is Rust binary execution (`cargo run --manifest-path apps/lightning-ops/Cargo.toml -- <command>`).
   - No legacy TypeScript deploy process is canonical.
5. `lightning-wallet-executor`
   - Active lane is Rust binary execution (`cargo run --manifest-path apps/lightning-wallet-executor/Cargo.toml -- serve`).
   - No legacy TypeScript deploy process is canonical.

## Non-canonical lanes

1. `openagents-web` and `openagents-migrate` are legacy Laravel resources and are not canonical deploy targets for new rollouts.
2. Legacy Laravel/PHP (`php artisan`), legacy Elixir (`mix test`), and legacy Node deploy commands are not canonical for active Rust service deployment.
3. Legacy deploy assets remain under archived paths only:
   - `apps/openagents.com/deploy/archived-laravel/`
   - `apps/openagents.com/docs/archived/legacy-laravel-deploy/`
