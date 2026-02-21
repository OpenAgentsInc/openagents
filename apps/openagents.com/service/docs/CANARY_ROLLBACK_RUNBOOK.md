# Rust Control Service Canary + Rollback Runbook

Status: active
Owner: `owner:openagents.com`
Scope: `apps/openagents.com/service` (`openagents-control-service`)
Related issues: OA-RUST-020, OA-RUST-021, OA-RUST-022, OA-RUST-062

## Purpose

Provide deterministic canary rollout and rollback steps for the Rust control service before major traffic shifts.
This runbook remains required after default-router switch (`OA_ROUTE_SPLIT_MODE=rust`) so route-level rollback stays fast and tested.

## Preconditions

1. Rust control service image built and available in Artifact Registry.
2. Staging and production Cloud Run services exist.
3. `gcloud` CLI is authenticated with deploy permissions.
4. Control-service smoke checks are green:
   - `cargo test -p openagents-control-service`
   - `OPENAGENTS_BASE_URL=https://<target-host> apps/openagents.com/service/deploy/smoke-health.sh`

## Required Environment

```bash
export PROJECT=openagentsgemini
export REGION=us-central1
export SERVICE=openagents-control-service
export IMAGE=us-central1-docker.pkg.dev/${PROJECT}/openagents-control-service/control:<tag>
```

## Canary Stage Plan

Use these stages in order. Do not advance if any gate fails.

1. Stage A (`0%`) deploy no-traffic revision.
2. Stage B (`5%`) initial canary.
3. Stage C (`25%`) expanded canary.
4. Stage D (`50%`) pre-full cutover.
5. Stage E (`100%`) full promotion.

## Traffic Commands

Use the helper script:

`apps/openagents.com/service/deploy/canary-rollout.sh`

Examples:

```bash
# Observe current traffic/revisions
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh status

# Deploy new revision with no traffic
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh deploy-no-traffic "${IMAGE}"

# Shift to 5% canary
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh set-traffic <stable-revision> <canary-revision> 5

# Shift to 25%
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh set-traffic <stable-revision> <canary-revision> 25

# Shift to 50%
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh set-traffic <stable-revision> <canary-revision> 50

# Promote to 100%
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh set-traffic <stable-revision> <canary-revision> 100
```

## Canary Gates (Go / No-Go)

Each stage requires all checks below over a 10-minute window:

1. HTTP 5xx rate remains below 1%.
2. P95 latency does not regress by more than 20% from stable baseline.
3. Auth verify failures (`auth.verify.completed`/failure lane + auth error responses) do not spike.
4. Sync token errors (`sync_token_unavailable`, invalid scope/request anomalies) do not spike.
5. Route split decision and override audit events remain present and parseable.

Suggested log probe:

```bash
gcloud logging read \
  'resource.type="cloud_run_revision" AND resource.labels.service_name="openagents-control-service"' \
  --project "${PROJECT}" --limit 100 --format json
```

## Post-Stage Verification Checklist

1. `GET /healthz` returns `200`.
2. `GET /readyz` returns `200`.
3. Auth flow succeeds (`/api/auth/email`, `/api/auth/verify`, `/api/auth/session`).
4. Sync token mint succeeds (`/api/sync/token`).
5. Route split endpoints succeed for authenticated operators:
   - `GET /api/v1/control/route-split/status`
   - `POST /api/v1/control/route-split/evaluate`
6. Auth/onboarding route ownership checks succeed for Rust-targeted cohorts:
   - route evaluate for `/login` and `/onboarding/checklist` returns Rust target in intended cohort.
7. Billing/lightning operator route checks succeed for Rust-targeted cohorts:
   - route evaluate for `/l402/paywalls` returns Rust target in intended cohort.
   - `POST /api/policy/authorize` returns expected allow/deny semantics for operator scopes.
8. Audit event stream includes:
   - `auth.verify.completed`
   - `sync.token.issued`
   - `route.split.decision`

## Deterministic Rollback Procedure

Trigger rollback immediately if any gate fails.

1. Route-level rollback (app behavior safety):

```bash
curl -sS -X POST "https://openagents.com/api/v1/control/route-split/override" \
  -H "authorization: Bearer ${CONTROL_ACCESS_TOKEN}" \
  -H "content-type: application/json" \
  -d '{"target":"legacy"}'
```

2. Infra traffic rollback (Cloud Run safety):

```bash
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh rollback <stable-revision>
```

3. Verify rollback status:

```bash
PROJECT=${PROJECT} REGION=${REGION} SERVICE=${SERVICE} \
apps/openagents.com/service/deploy/canary-rollout.sh status
```

4. Confirm service health and auth/session/sync endpoints on stable revision.

5. Record incident notes with failed stage, symptom, and revision IDs.

## Staging Rehearsal Evidence (2026-02-21)

Dry-run command rehearsal executed:

```text
+ gcloud run services describe openagents-control-service --project openagentsgemini --region us-central1 --format yaml(status.traffic,status.latestCreatedRevisionName,status.latestReadyRevisionName)
+ gcloud run deploy openagents-control-service --project openagentsgemini --region us-central1 --image us-central1-docker.pkg.dev/openagentsgemini/openagents-control-service/control:canary-test --no-traffic
+ gcloud run services update-traffic openagents-control-service --project openagentsgemini --region us-central1 --to-revisions openagents-control-service-00010-abc=95,openagents-control-service-00011-def=5
+ gcloud run services update-traffic openagents-control-service --project openagentsgemini --region us-central1 --to-revisions openagents-control-service-00010-abc=100
```

Simulated rollback behavior verification:

```bash
cargo test -p openagents-control-service route_split_override_supports_fast_rollback_to_legacy
```

This test validates immediate route-level fallback to legacy target after override.
