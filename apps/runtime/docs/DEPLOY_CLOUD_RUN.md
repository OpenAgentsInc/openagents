# OpenAgents Runtime Cloud Run Deploy Runbook

This is the canonical production deploy flow for the Cloud Run runtime stack:

- Service: `runtime`
- Migration job: `runtime-migrate`
- Chained deploy wrapper: `apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh`
- Migration drift check: `apps/runtime/deploy/cloudrun/check-migration-drift.sh`
- DB role isolation tooling: `apps/runtime/deploy/cloudrun/apply-db-role-isolation.sh`, `apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh`
- Script index: `apps/runtime/deploy/cloudrun/README.md`
- Zero-downtime schema evolution policy: `docs/core/SCHEMA_EVOLUTION_PLAYBOOK.md`
- Spacetime replay/resume parity harness: `docs/sync/SPACETIME_PARITY_HARNESS.md`
- Shared staging/prod release matrix: `docs/core/RUST_STAGING_PROD_VALIDATION.md`

## Artifact Registry + legacy lane note (non-negotiable)

- Artifact Registry repo: `us-central1-docker.pkg.dev/openagentsgemini/openagents-runtime/*`
- Legacy runtime lane (Elixir) uses image `openagents-runtime/runtime:*` and the tag `openagents-runtime/runtime:latest`.
- Rust runtime images publish to `openagents-runtime/runtime-rust:*` (including `:latest-rust`).

Do not overwrite the legacy `openagents-runtime/runtime:latest` tag with Rust images.

## Why this exists

We previously hit a production incident where the migration job stayed pinned to an older image, so newer runtime sync tables were never created (`runtime.sync_stream_events`), causing 500s.

## Non-negotiable invariant

Before every migration run:

1. `runtime-migrate` image must equal the currently deployed runtime service image.
2. Migration execution must run the Rust binary command `runtime-migrate`.

Never run the migrate job directly without first syncing the job image.
`run-migrate-job.sh` enforces this image-lock + execution path.

## Deploy sequence

0. Confirm this rollout follows the expand/migrate/contract policy and mixed-version gates in `docs/core/SCHEMA_EVOLUTION_PLAYBOOK.md`.
0.1. For staged cutovers, run the Spacetime replay/resume parity harness and archive the parity report artifact before promotion.

1. Build and push runtime image.

```bash
gcloud builds submit \
  --config apps/runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  .
```

2. Deploy runtime and run mandatory migration validation using the chained script.

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-runtime/runtime-rust:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

This is the canonical command path. `deploy-runtime-and-migrate.sh` enforces:
- runtime service deploy,
- mandatory migration execution,
- migration job image alignment with runtime service image,
- latest migration execution success + image consistency via `check-migration-drift.sh`.

If `DB_URL` is set, role-isolation verification runs automatically (`VERIFY_DB_ROLE_ISOLATION=1` by default).

3. Apply role policy (idempotent) and verify drift status.

```bash
DB_URL='postgres://...' \
apps/runtime/deploy/cloudrun/apply-db-role-isolation.sh

DB_URL='postgres://...' \
apps/runtime/deploy/cloudrun/verify-db-role-isolation.sh
```

4. Confirm no new runtime/web 500s in logs.

```bash
gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-control-service" AND httpRequest.status=500' \
  --project openagentsgemini --freshness=10m --limit=20

gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="runtime" AND severity>=ERROR' \
  --project openagentsgemini --freshness=10m --limit=50
```

## Drift check (manual)

Run this before or after deploy if anything looks off:

```bash
gcloud run services describe runtime \
  --project openagentsgemini --region us-central1 \
  --format='value(spec.template.spec.containers[0].image)'

gcloud run jobs describe runtime-migrate \
  --project openagentsgemini --region us-central1 \
  --format='value(spec.template.spec.template.spec.containers[0].image)'
```

The two values must match.

## Manual drift audit (alert gate)

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
apps/runtime/deploy/cloudrun/check-migration-drift.sh
```

This command should be wired to operator checks/alerts; non-zero exit means migration drift or stale execution state.

## CEP MVP-1 Post-Deploy Validation

Validate the runtime-served OpenAPI and CEP health/exposure lanes after rollout:

```bash
RUNTIME_BASE_URL="https://runtime-<env>.a.run.app"
TOKEN="<runtime-internal-bearer>"

curl -sf "${RUNTIME_BASE_URL}/internal/v1/openapi.json" \
  -H "authorization: Bearer ${TOKEN}" \
  | jq '.paths["/credit/intent"], .paths["/credit/settle"], .components.schemas.CreditSettleResponseV1.properties.settlement_id'

curl -sf "${RUNTIME_BASE_URL}/internal/v1/credit/health" \
  -H "authorization: Bearer ${TOKEN}" \
  | jq '.schema, .breakers, .policy.max_sats_per_envelope'
```
