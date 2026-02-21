# OpenAgents Runtime Cloud Run Deploy Runbook

This is the canonical production deploy flow for the Cloud Run runtime stack:

- Service: `openagents-runtime`
- Migration job: `openagents-runtime-migrate`

## Why this exists

We previously hit a production incident where the migration job stayed pinned to an older image, so newer runtime tables were never created (`runtime.khala_projection_checkpoints`, `runtime.sync_stream_events`), causing 500s.

## Non-negotiable invariant

Before every migration run:

1. `openagents-runtime-migrate` image must equal the currently deployed runtime service image.
2. Migration execution must run `OpenAgentsRuntime.Release.migrate_and_verify!()`.

Never run the migrate job directly without first syncing the job image.

`run-migrate-job.sh` enforces this and attempts `migrate_and_verify!()` first. If the currently deployed runtime image predates that helper, the script detects that specific failure and falls back to `migrate()` for that run so deploys are not blocked during transition.

## Deploy sequence

1. Build and push runtime image.

```bash
gcloud builds submit \
  --config apps/openagents-runtime/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/openagents-runtime
```

2. Deploy runtime service with that exact image.

```bash
gcloud run deploy openagents-runtime \
  --project openagentsgemini \
  --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/openagents-runtime/runtime:<TAG>
```

3. Sync migrate job image to runtime image and execute migrations.

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=openagents-runtime \
MIGRATE_JOB=openagents-runtime-migrate \
apps/openagents-runtime/deploy/cloudrun/run-migrate-job.sh
```

4. Confirm no new runtime/web 500s in logs.

```bash
gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-web" AND httpRequest.status=500' \
  --project openagentsgemini --freshness=10m --limit=20

gcloud logging read 'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-runtime" AND severity>=ERROR' \
  --project openagentsgemini --freshness=10m --limit=50
```

## Drift check (manual)

Run this before or after deploy if anything looks off:

```bash
gcloud run services describe openagents-runtime \
  --project openagentsgemini --region us-central1 \
  --format='value(spec.template.spec.containers[0].image)'

gcloud run jobs describe openagents-runtime-migrate \
  --project openagentsgemini --region us-central1 \
  --format='value(spec.template.spec.template.spec.containers[0].image)'
```

The two values must match.
