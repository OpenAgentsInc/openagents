# Convex Non-Prod Deployment (GCP)

This directory provisions and validates a non-prod self-hosted Convex setup for
OpenAgents:

- Convex backend (pinned image mirrored to Artifact Registry)
- Convex dashboard (pinned image mirrored to Artifact Registry)
- Cloud SQL Auth Proxy sidecar in backend Cloud Run service
- Cloud SQL Postgres in the same region

## Version pinning policy

Images are pinned in `provision-nonprod-gcp.sh` and must be bumped via PR with
explicit version updates.

Current pinned versions (2026-02-19):

- Backend: `us-central1-docker.pkg.dev/openagentsgemini/thirdparty/convex-backend:2026-02-19-amd64`
- Dashboard: `us-central1-docker.pkg.dev/openagentsgemini/thirdparty/convex-dashboard:2026-02-19-amd64`
- Cloud SQL proxy: `gcr.io/cloud-sql-connectors/cloud-sql-proxy:2.19.0`

Source images for mirror refresh:

- `ghcr.io/get-convex/convex-backend@sha256:fde1830745d1c2c69dd731ff1a245591e3aba380df990df2d390f2338b574d73`
- `ghcr.io/get-convex/convex-dashboard@sha256:f809827d55bc53f617199f7ec0962b6c261f774188fbc10c62737869ed3c631b`

## Prerequisites

- `gcloud` authenticated with deploy permissions in target project
- `jq`, `curl`, `rg`, `openssl`
- APIs enabled in project:
  - `run.googleapis.com`
  - `sqladmin.googleapis.com`
  - `secretmanager.googleapis.com`
  - `iam.googleapis.com`

## Dry-run

```bash
apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh
```

## Apply

```bash
OA_CONVEX_APPLY=1 apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh
```

Optional overrides:

```bash
PROJECT_ID=openagentsgemini \
REGION=us-central1 \
CLOUD_SQL_INSTANCE=oa-convex-nonprod-pg \
CONVEX_BACKEND_SERVICE=oa-convex-backend-nonprod \
CONVEX_DASHBOARD_SERVICE=oa-convex-dashboard-nonprod \
CONVEX_PROXY_IMAGE=gcr.io/cloud-sql-connectors/cloud-sql-proxy:2.19.0 \
OA_CONVEX_APPLY=1 \
apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh
```

## Health verification

```bash
apps/openagents-runtime/deploy/convex/check-nonprod-health.sh
```

This validates:

1. backend `/version` endpoint returns a non-empty response
2. dashboard HTTP response is healthy (`200` or `302`)
3. Cloud SQL region matches deployment region
4. backend root health payload confirms the service is running
5. Cloud Run service readiness conditions are `True`

## Operations runbook

Day-2 operations (admin key policy, export/import backup validation, upgrade and
rollback flow) are documented in:

- `apps/openagents-runtime/deploy/convex/OPERATIONS_RUNBOOK.md`

## Gate G7 drill helpers

Backup/restore drill:

```bash
apps/openagents-runtime/deploy/convex/run-backup-restore-drill.sh
```

Rollback drill (dry-run by default):

```bash
apps/openagents-runtime/deploy/convex/run-rollback-drill.sh
```

Rollback drill (apply):

```bash
OA_CONVEX_ROLLBACK_DRILL_APPLY=1 \
apps/openagents-runtime/deploy/convex/run-rollback-drill.sh
```

Runtime replay drill:

```bash
apps/openagents-runtime/deploy/convex/run-runtime-replay-drill.sh
```

Drill evidence/reporting artifacts:

- `apps/openagents-runtime/docs/reports/2026-02-19-convex-runtime-projector-load-chaos-report.md`
- `apps/openagents-runtime/docs/reports/2026-02-19-convex-g7-backup-restore-replay-rollback-drill.md`
