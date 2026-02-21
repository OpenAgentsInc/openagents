# Rust Control Service Staging Deploy Runbook

Status: active  
Owner: `owner:openagents.com`, `owner:infra`  
Scope: `staging.openagents.com` for `openagents-control-service-staging`

## Purpose

Define the canonical staging deployment flow for the Rust control service + Rust web shell before production rollout.

## Preconditions

1. `gcloud` auth is valid for project `openagentsgemini`.
2. Artifact image exists for target tag.
3. Staging service exists (default `openagents-control-service-staging`).
4. Domain mapping points `staging.openagents.com` to the staging service.
5. Required staging secrets/env are present.

## Required staging env and secrets

Minimum required configuration for staging:

1. WorkOS/auth:
   - `WORKOS_CLIENT_ID`
   - `WORKOS_API_KEY`
   - `OA_AUTH_PROVIDER_MODE=workos`
2. Sync token:
   - `OA_SYNC_TOKEN_ENABLED=true`
   - `OA_SYNC_TOKEN_SIGNING_KEY` (or `SYNC_TOKEN_SIGNING_KEY`)
   - `OA_SYNC_TOKEN_ISSUER=https://staging.openagents.com` (recommended for staging)
   - `OA_SYNC_TOKEN_AUDIENCE=openagents-sync`
3. Route split + static host:
   - `OA_ROUTE_SPLIT_ENABLED=true`
   - `OA_ROUTE_SPLIT_MODE=rust`
   - `OA_CONTROL_STATIC_DIR=/app/apps/openagents.com/web-shell/dist`
4. Compatibility (optional gating in staging):
   - `OA_COMPAT_CONTROL_ENFORCED`
   - `OA_COMPAT_CONTROL_PROTOCOL_VERSION`
   - `OA_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID`

## Deploy command (canonical)

From repo root:

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
IMAGE=us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:<TAG> \
apps/openagents.com/service/deploy/deploy-staging.sh
```

`deploy-staging.sh` reuses the same verification gates as production deploy:

1. `cargo test --manifest-path apps/openagents.com/service/Cargo.toml`
2. `cargo check -p openagents-web-shell --target wasm32-unknown-unknown`
3. `apps/openagents.com/web-shell/scripts/sw-policy-verify.sh`
4. `apps/openagents.com/web-shell/scripts/perf-budget-gate.sh`

## Domain mapping checks

```bash
gcloud beta run domain-mappings describe \
  --project openagentsgemini \
  --region us-central1 \
  --domain staging.openagents.com
```

## Smoke checks

```bash
OPENAGENTS_BASE_URL=https://staging.openagents.com \
apps/openagents.com/service/deploy/smoke-health.sh

OPENAGENTS_BASE_URL=https://staging.openagents.com \
apps/openagents.com/service/deploy/smoke-control.sh
```

Optional authenticated smoke checks:

```bash
OPENAGENTS_BASE_URL=https://staging.openagents.com \
OPENAGENTS_CONTROL_ACCESS_TOKEN=<token> \
apps/openagents.com/service/deploy/smoke-control.sh
```

## Rollback

Use standard canary rollback against staging service:

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
apps/openagents.com/service/deploy/canary-rollout.sh rollback <stable-revision>
```

## Evidence artifact

Record each staged rollout in:

- `apps/openagents.com/service/docs/reports/<timestamp>-staging-deploy-report.md`

Required fields:

1. image tag and revision IDs
2. traffic split before/after
3. smoke check outputs
4. rollback command and stable revision
