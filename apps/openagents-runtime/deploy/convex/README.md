# Convex Non-Prod Deployment (GCP)

This directory provisions and validates a non-prod self-hosted Convex setup for
OpenAgents:

- Convex backend (`ghcr.io/get-convex/convex-backend`)
- Convex dashboard (`ghcr.io/get-convex/convex-dashboard`)
- Cloud SQL Postgres in the same region

## Version pinning policy

Images are pinned by digest in `provision-nonprod-gcp.sh` and must be bumped via
PR with explicit digest updates.

Current pinned digests (2026-02-19):

- Backend: `ghcr.io/get-convex/convex-backend@sha256:fde1830745d1c2c69dd731ff1a245591e3aba380df990df2d390f2338b574d73`
- Dashboard: `ghcr.io/get-convex/convex-dashboard@sha256:f809827d55bc53f617199f7ec0962b6c261f774188fbc10c62737869ed3c631b`

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
OA_CONVEX_APPLY=1 \
apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh
```

## Health verification

```bash
apps/openagents-runtime/deploy/convex/check-nonprod-health.sh
```

This validates:

1. backend `/version` endpoint returns JSON
2. dashboard HTTP response is healthy (`200` or `302`)
3. Cloud SQL region matches deployment region
4. Cloud Run service readiness conditions are `True`
