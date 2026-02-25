# Rust Control Service Staging Deploy Runbook

Status: active  
Owner: `owner:openagents.com`, `owner:infra`  
Scope: `staging.openagents.com` for `openagents-control-service-staging`

## Purpose

Define the canonical staging deployment flow for the Rust control service in landing-only web mode before production rollout.

## Preconditions

1. `gcloud` auth is valid for project `openagentsgemini`.
2. Artifact image exists for target tag.
3. Staging service exists (default `openagents-control-service-staging`).
4. Domain mapping for `staging.openagents.com` is verified (or an explicit Cloud Run service URL override is used while cert/domain mapping is pending).
5. Required staging secrets/env are present.
6. (Optional) `curl` is available if you plan to run the `smoke-*.sh` scripts as-is.

## Required staging env and secrets

Minimum required configuration for staging:

1. Static host:
   - `OA_CONTROL_STATIC_DIR=/app/service/static`
2. Desktop download redirect:
   - `OA_DESKTOP_DOWNLOAD_URL=https://github.com/openagents/openagents/releases/latest` (or release bucket URL)
3. Route split:
   - `OA_ROUTE_SPLIT_MODE=rust` (typical for staging)
4. Auth provider mode:
   - Staging can run with `OA_AUTH_PROVIDER_MODE=mock` for smoke/testing.
   - Production-like staging uses `OA_AUTH_PROVIDER_MODE=workos` and requires `WORKOS_CLIENT_ID` + `WORKOS_API_KEY`.
5. Sync token (optional in staging):
   - `OA_SYNC_TOKEN_ENABLED=true|false`
   - When enabled: `OA_SYNC_TOKEN_SIGNING_KEY`, `OA_SYNC_TOKEN_ISSUER`, `OA_SYNC_TOKEN_AUDIENCE`
6. Compatibility gates (optional in staging):
   - `OA_COMPAT_CONTROL_ENFORCED`
   - `OA_COMPAT_CONTROL_PROTOCOL_VERSION`
   - `OA_COMPAT_CONTROL_MIN_CLIENT_BUILD_ID`

## Deploy sequence (canonical)

From repo root:

1. (Recommended) run the local verification gates that mirror production:

```bash
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
```

2. Build and push the control-service image (repo root context required):

```bash
TAG="$(git rev-parse --short HEAD)"
gcloud builds submit \
  --config apps/openagents.com/service/deploy/cloudbuild.yaml \
  --substitutions _TAG="${TAG}" \
  .
```

3. Deploy to staging with 100% traffic:

```bash
TAG="$(git rev-parse --short HEAD)"
gcloud run deploy openagents-control-service-staging \
  --project openagentsgemini \
  --region us-central1 \
  --image "us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:${TAG}" \
  --quiet
```

Optional (no-traffic canary on staging):

- `apps/openagents.com/service/deploy/deploy-staging.sh` creates a no-traffic revision.
- If you use it, you must explicitly shift traffic (see `apps/openagents.com/service/deploy/canary-rollout.sh`).

## Domain mapping checks

```bash
gcloud beta run domain-mappings describe \
  --project openagentsgemini \
  --region us-central1 \
  --domain staging.openagents.com
```

Expected for domain-based staging smoke:

1. `spec.routeName: openagents-control-service-staging`
2. `Ready=True`
3. `CertificateProvisioned=True`

If certificate/domain mapping is pending, use the staging Cloud Run URL as `OPENAGENTS_BASE_URL` for validation and record the domain-mapping status in the deploy report.

## Smoke checks

Important note: `GET /readyz` is the canonical probe. Some staging environments may return `404` for `GET /healthz` at the domain/edge layer.

```bash
OPENAGENTS_BASE_URL=${OPENAGENTS_BASE_URL:-https://staging.openagents.com} \
apps/openagents.com/service/deploy/smoke-control.sh
```

Optional authenticated smoke checks:

```bash
OPENAGENTS_BASE_URL=${OPENAGENTS_BASE_URL:-https://staging.openagents.com} \
OPENAGENTS_CONTROL_ACCESS_TOKEN=<token> \
apps/openagents.com/service/deploy/smoke-control.sh
```

Maintenance-window smoke checks:

```bash
OPENAGENTS_BASE_URL=${OPENAGENTS_BASE_URL:-https://staging.openagents.com} \
OPENAGENTS_MAINTENANCE_BYPASS_TOKEN=<token> \
apps/openagents.com/service/deploy/smoke-control.sh
```

Maintenance mode enable/disable helper:

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
apps/openagents.com/service/deploy/maintenance-mode.sh status
```

Canonical maintenance cutover runbook:

- `apps/openagents.com/service/docs/MAINTENANCE_MODE_CUTOVER_RUNBOOK.md`

## Staging/Prod validation matrix gate

Before promoting staging changes toward production, run the shared Rust matrix:

```bash
CONTROL_SERVICE=openagents-control-service-staging \
RUNTIME_SERVICE=runtime \
MIGRATE_JOB=runtime-migrate \
STAGING_CONTROL_BASE_URL=https://staging.openagents.com \
PROD_CONTROL_BASE_URL=https://openagents.com \
scripts/release/validate-rust-cutover.sh
```

Canonical matrix spec: `docs/core/RUST_STAGING_PROD_VALIDATION.md`.

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
