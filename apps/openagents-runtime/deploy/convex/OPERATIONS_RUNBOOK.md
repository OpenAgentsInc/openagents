# Convex Non-Prod Operations Runbook

Date: 2026-02-19  
Scope: Gate G1 non-prod Convex operations hardening for OpenAgents GCP

## Deployment Baseline

- Project: `openagentsgemini`
- Region: `us-central1`
- Backend service: `oa-convex-backend-nonprod`
- Dashboard service: `oa-convex-dashboard-nonprod`
- Cloud SQL instance: `oa-convex-nonprod-pg`
- Database: `convex_nonprod`
- Database user: `convex`

Backend topology:

- `convex-backend` Cloud Run container
- Cloud SQL Auth Proxy sidecar container
- `POSTGRES_URL` points to `localhost:5432` (no database path in URL)

## Required Environment and Hardening

Backend (`convex-backend` container) must set:

- `CONVEX_CLOUD_ORIGIN`
- `CONVEX_SITE_ORIGIN`
- `POSTGRES_URL`
- `INSTANCE_NAME`
- `REDACT_LOGS_TO_CLIENT=true`
- `DISABLE_BEACON=true`
- `DO_NOT_REQUIRE_SSL=1`

Dashboard must set:

- `NEXT_PUBLIC_DEPLOYMENT_URL`
- `NEXT_PUBLIC_LOAD_MONACO_INTERNALLY=1`

Current automation source of truth:

- `apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh`
- `apps/openagents-runtime/deploy/convex/check-nonprod-health.sh`

## Admin Key Handling Policy

Policy:

- Convex admin keys are operator-only secrets.
- Admin keys must never be committed to git or exposed in client-side env.
- Canonical storage is Google Secret Manager:
  `oa-convex-nonprod-admin-key`.
- Instance secret (`oa-convex-nonprod-instance-secret`) is a separate secret
  and remains backend-only.

Generate and rotate admin key:

```bash
INSTANCE_SECRET="$(gcloud secrets versions access latest \
  --secret=oa-convex-nonprod-instance-secret \
  --project openagentsgemini)"

ADMIN_KEY="$(cargo run -q -p keybroker --bin generate_key -- \
  convex-nonprod "$INSTANCE_SECRET")"

printf '%s' "$ADMIN_KEY" | gcloud secrets versions add \
  oa-convex-nonprod-admin-key \
  --data-file=- \
  --project openagentsgemini
```

Notes:

- Run from `~/code/convex/convex-backend` for `cargo run`.
- Grant access to `oa-convex-nonprod-admin-key` only to operator identities.
- Do not inject admin key into Cloud Run runtime env.

## Export / Import Validation (Non-Prod)

Build an env file for CLI commands:

```bash
BACKEND_URL="$(gcloud run services describe oa-convex-backend-nonprod \
  --project openagentsgemini \
  --region us-central1 \
  --format='value(status.url)')"

ADMIN_KEY="$(gcloud secrets versions access latest \
  --secret=oa-convex-nonprod-admin-key \
  --project openagentsgemini)"

cat > /tmp/convex-nonprod-self-hosted.env <<EOF
CONVEX_SELF_HOSTED_URL="$BACKEND_URL"
CONVEX_SELF_HOSTED_ADMIN_KEY="$ADMIN_KEY"
EOF
```

Run verification from a Convex app workspace:

```bash
npx convex dev --once --env-file /tmp/convex-nonprod-self-hosted.env

EXPORT_PATH="/tmp/convex-nonprod-export-$(date +%Y%m%d-%H%M%S).zip"
npx convex export --env-file /tmp/convex-nonprod-self-hosted.env --path "$EXPORT_PATH"

npx convex import --env-file /tmp/convex-nonprod-self-hosted.env --append "$EXPORT_PATH"
```

2026-02-19 evidence:

- `npx convex dev --once`: `Convex functions ready`.
- `npx convex export`: snapshot created and downloaded to
  `/tmp/convex-nonprod-export-20260219-131722.zip`.
- `npx convex import --append`: completed with `Added 0 documents`.

## Upgrade Runbook

1. Mirror new Convex backend/dashboard images into Artifact Registry.
2. Update pinned image references in:
   - `apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh`
   - `apps/openagents-runtime/deploy/convex/README.md`
3. Validate dry-run:
   - `apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh`
4. Apply:
   - `OA_CONVEX_APPLY=1 apps/openagents-runtime/deploy/convex/provision-nonprod-gcp.sh`
5. Verify:
   - `apps/openagents-runtime/deploy/convex/check-nonprod-health.sh`
   - `npx convex dev --once --env-file /tmp/convex-nonprod-self-hosted.env`
6. Export a post-upgrade snapshot.

## Rollback Runbook

Image/config rollback:

1. Identify previous ready revisions:
   - `gcloud run revisions list --project openagentsgemini --region us-central1 --service oa-convex-backend-nonprod`
   - `gcloud run revisions list --project openagentsgemini --region us-central1 --service oa-convex-dashboard-nonprod`
2. Route traffic to known-good revisions:
   - `gcloud run services update-traffic oa-convex-backend-nonprod --project openagentsgemini --region us-central1 --to-revisions <REVISION>=100`
   - `gcloud run services update-traffic oa-convex-dashboard-nonprod --project openagentsgemini --region us-central1 --to-revisions <REVISION>=100`
3. Re-pin provisioning script to prior known-good images if needed and re-apply.

Data rollback (if required):

1. Use a known-good snapshot ZIP.
2. Import with replace semantics:
   - `npx convex import --env-file /tmp/convex-nonprod-self-hosted.env --replace-all -y <SNAPSHOT_ZIP>`
3. Re-run health checks and CLI smoke tests.

## Runtime Projection Rebuild (Drop + Replay)

When Convex summary projections drift from runtime truth, rebuild from runtime
durable history.

Run from `apps/openagents-runtime/`:

```bash
mix runtime.convex.reproject --run-id <run_id>
mix runtime.convex.reproject --worker-id <worker_id>
mix runtime.convex.reproject --all
```

Behavior:

- rebuild drops runtime checkpoint state for the selected entity/scope
- projector replays deterministic summary from runtime Postgres state/events
- replay emits telemetry for observability:
  - `openagents_runtime.convex.projection.replay.count`
  - `openagents_runtime.convex.projection.replay.duration_ms`

Projection health metrics to watch during/after rebuild:

- `openagents_runtime.convex.projection.lag_events`
- `openagents_runtime.convex.projection.write.count` (filter `result=error`)
- `openagents_runtime.convex.projection.write_failure.count`
- `openagents_runtime.convex.projection.drift.count`

## Gate G7 Drill Automation

Use the helper scripts for repeatable drills:

- backup/restore drill:
  - `apps/openagents-runtime/deploy/convex/run-backup-restore-drill.sh`
- rollback drill (dry-run by default):
  - `apps/openagents-runtime/deploy/convex/run-rollback-drill.sh`
- rollback drill (apply):
  - `OA_CONVEX_ROLLBACK_DRILL_APPLY=1 apps/openagents-runtime/deploy/convex/run-rollback-drill.sh`
- runtime replay drill:
  - `apps/openagents-runtime/deploy/convex/run-runtime-replay-drill.sh`

Evidence reports:

- `apps/openagents-runtime/docs/reports/2026-02-19-convex-runtime-projector-load-chaos-report.md`
- `apps/openagents-runtime/docs/reports/2026-02-19-convex-g7-backup-restore-replay-rollback-drill.md`

## Staged Rollout Plan (Internal -> Limited -> Full)

Phase A: Internal users only

1. Route 100% of OpenAgents staff/admin users to Convex-backed sync surfaces.
2. Hold period: 24h.
3. Promotion requirements:
   - no critical alerts in `apps/openagents-runtime/deploy/monitoring/prometheus/openagents-runtime-alert-rules.yaml`,
   - rollback drill RTO remains within target.

Phase B: Limited cohort

1. Expand to 5-10% of external users (canary cohort).
2. Hold period: 48h.
3. Promotion requirements:
   - alert budgets hold for lag/drift/write-failure/replay,
   - no unresolved Sev1/Sev2 incidents in cohort.

Phase C: Full exposure

1. Ramp to 25%, then 50%, then 100% with health checks between each step.
2. Keep rollback command path pre-staged and validated before each increment.
3. Stop ramp immediately if any critical Convex projection alert fires for >5m.

## Rollback Drill Objective and Evidence

RTO target:

- rollback to known-good Convex revisions + health restoration in <= 5 minutes.

2026-02-19 non-prod measured drill:

- backend rollback revision: `oa-convex-backend-nonprod-00010-kpf`
- dashboard rollback revision: `oa-convex-dashboard-nonprod-00001-8rs`
- restored target revisions:
  - backend: `oa-convex-backend-nonprod-00011-v4r`
  - dashboard: `oa-convex-dashboard-nonprod-00002-hwm`
- measured rollback RTO: 20 seconds
- measured restore duration: 18 seconds
- total drill duration: 38 seconds

Result: RTO target met in non-prod production-like environment.

## On-Call Ownership and Escalation

Primary ownership:

1. Runtime on-call:
   - owns runtime projector behavior, replay operations, and runtime API health.
2. Infra/SRE on-call:
   - owns Cloud Run revision traffic management, Cloud SQL health, secret access.
3. Web platform on-call:
   - owns Laravel Convex token minting path and client auth bridge.

Escalation order:

1. Runtime on-call triages first and classifies incident lane:
   - projector lag/drift/error/replay
   - Convex infra/service availability
   - auth token bridge
2. If infra lane: page Infra/SRE on-call immediately.
3. If auth lane: page Web platform on-call immediately.
4. If blast radius exceeds canary cohort or Sev1 impact:
   - execute rollback drill path,
   - notify engineering incident commander and product owner,
   - freeze rollout progression until post-incident review.
