# Staging Rust Deploy Report (OA-RUST-108)

Date: 2026-02-21  
Target domain: `https://staging.openagents.com`  
Service target: `openagents-control-service-staging`

## Pre-deploy baseline checks

Observed from local probe:

1. `GET /healthz` returned non-service Google `404`.
2. `GET /readyz` returned Laravel-style `404 Not Found`.

This indicates `staging.openagents.com` was not fully serving the Rust control-service health routes at the time of check.

## Deploy execution status

Staging deploy wrapper and runbook were added:

1. `apps/openagents.com/service/deploy/deploy-staging.sh`
2. `apps/openagents.com/service/docs/STAGING_DEPLOY_RUNBOOK.md`

Live Cloud Run deploy and revision capture were blocked by local gcloud auth refresh in non-interactive execution:

```text
ERROR: (gcloud.run.services.list) There was a problem refreshing your current auth tokens:
Reauthentication failed. cannot prompt during non-interactive execution.
```

Local dry-run validation succeeded:

```bash
PROJECT=openagentsgemini REGION=us-central1 SERVICE=openagents-control-service-staging \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:test \
SKIP_VERIFY=1 DRY_RUN=1 apps/openagents.com/service/deploy/deploy-staging.sh
```

Observed output included:

1. web-shell dist build completed.
2. planned no-traffic deploy command printed for `openagents-control-service-staging`.

## Next command once gcloud auth is refreshed

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:<TAG> \
apps/openagents.com/service/deploy/deploy-staging.sh
```

Post-deploy smoke:

```bash
OPENAGENTS_BASE_URL=https://staging.openagents.com \
apps/openagents.com/service/deploy/smoke-health.sh

OPENAGENTS_BASE_URL=https://staging.openagents.com \
apps/openagents.com/service/deploy/smoke-control.sh
```

Current smoke result before live deploy:

```text
[smoke] health
curl: (56) The requested URL returned error: 404
```
