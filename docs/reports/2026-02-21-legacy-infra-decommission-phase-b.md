# 2026-02-21 Legacy Infra Decommission Phase B

Issue: OA-RUST-111 (`#1936`)  
Scope: freeze legacy Laravel deploy lanes + capture rollback metadata  
Timestamp (UTC): 2026-02-21T19:52:58Z

## Inventory and rollback artifacts

Directory:

- `docs/reports/legacy-infra/20260221T195258Z-phase-b/`

Files:

- `services.snapshot.json`
- `jobs.snapshot.json`
- `domain-mappings.snapshot.json`
- `artifact-repos.snapshot.json`
- `secrets.snapshot.json`
- `cloudbuild-triggers.snapshot.json`
- `openagents-web.service.yaml`
- `openagents-web-staging.service.yaml`
- `openagents-migrate.job.yaml`
- `openagents-maintenance-down.job.yaml`
- `openagents-runtime-migrate.job.yaml`
- `openagents-web.rollback-pointer.json`
- `openagents-web-staging.rollback-pointer.json`
- `openagents-migrate.rollback-pointer.json`
- `openagents-maintenance-down.rollback-pointer.json`
- `openagents-web-images.snapshot.json`
- `smoke.after-freeze.json`

## Phase B actions executed

1. Reopened OA-RUST-111 because only Phase A had been executed.
2. Added hard freeze guards to legacy deploy entrypoints:
   - `apps/openagents.com/deploy/archived-laravel/phase-b-freeze-guard.sh`
   - `apps/openagents.com/deploy/archived-laravel/apply-production-env.sh`
   - `apps/openagents.com/deploy/archived-laravel/sync-openapi-to-docs.sh`
   - `apps/openagents.com/deploy/archived-laravel/cloudbuild.yaml`
3. Added archive policy file:
   - `apps/openagents.com/deploy/archived-laravel/README.md`

## Freeze behavior

By default, legacy Laravel deploy scripts now fail with exit code `78`.

Explicit override is required:

- `OA_LEGACY_LARAVEL_UNFREEZE=1`
- `OA_LEGACY_LARAVEL_CHANGE_TICKET=<approved-ticket-id>`

Legacy Cloud Build lane also requires:

- `_OA_LEGACY_LARAVEL_UNFREEZE=1`
- `_OA_LEGACY_LARAVEL_CHANGE_TICKET=<approved-ticket-id>`

## Rollback pointers captured

- `openagents-web` revision: `openagents-web-00097-jr6`  
  image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:436a8deba`
- `openagents-web-staging` revision: `openagents-web-staging-00037-kdw`  
  image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:dd035e27f`
- `openagents-migrate` command: `php artisan migrate --force`  
  image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:latest`
- `openagents-maintenance-down` command: `php artisan down ...`  
  image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-web/laravel:72cae86ae`

## No-regression checks after freeze

See `docs/reports/legacy-infra/20260221T195258Z-phase-b/smoke.after-freeze.json`.

- `https://openagents.com` -> `200`
- `https://staging.openagents.com` -> `200`
- `https://openagents-web-ezxz4mgdsq-uc.a.run.app` -> `200`
- `https://openagents-web-staging-ezxz4mgdsq-uc.a.run.app` -> `200`

## Phase C gate status

Not executed in Phase B:

- production Laravel resource deletion
- domain cutover from `openagents-web`
- maintenance-mode public cutover flow (tracked by OA-RUST-112)
