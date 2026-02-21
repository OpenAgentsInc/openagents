# Runtime Cloud Run Deploy Scripts

Canonical deploy path (required):

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/runtime/runtime:<TAG> \
apps/runtime/deploy/cloudrun/deploy-runtime-and-migrate.sh
```

This command chains:

1. `gcloud run deploy` for runtime service.
2. `run-migrate-job.sh` migration execution (`runtime-migrate` command with image lock).
3. `check-migration-drift.sh` validation (service/job image alignment + latest execution success).

Optional role isolation verification runs during migration when `DB_URL`/`DATABASE_URL` is provided.

Manual guard checks:

```bash
GCP_PROJECT=openagentsgemini \
GCP_REGION=us-central1 \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
apps/runtime/deploy/cloudrun/check-migration-drift.sh
```
