# Khala Non-Prod Operations Runbook

Date: 2026-02-19  
Scope: Gate G1 non-prod Khala operations hardening for OpenAgents GCP

## Deployment Baseline

- Project: `openagentsgemini`
- Region: `us-central1`
- Backend service: `oa-khala-backend-nonprod`
- Dashboard service: `oa-khala-dashboard-nonprod`
- Cloud SQL instance: `oa-khala-nonprod-pg`
- Database: `khala_nonprod`
- Database user: `khala`

Backend topology:

- `khala-backend` Cloud Run container
- Cloud SQL Auth Proxy sidecar container
- `POSTGRES_URL` points to `localhost:5432` (no database path in URL)

## Required Environment and Hardening

Backend (`khala-backend` container) must set:

- `KHALA_CLOUD_ORIGIN`
- `KHALA_SITE_ORIGIN`
- `POSTGRES_URL`
- `INSTANCE_NAME`
- `REDACT_LOGS_TO_CLIENT=true`
- `DISABLE_BEACON=true`
- `DO_NOT_REQUIRE_SSL=1`

Dashboard must set:

- `NEXT_PUBLIC_DEPLOYMENT_URL`
- `NEXT_PUBLIC_LOAD_MONACO_INTERNALLY=1`

Current automation source of truth:

- `apps/openagents-runtime/deploy/khala/provision-nonprod-gcp.sh`
- `apps/openagents-runtime/deploy/khala/check-nonprod-health.sh`

## Admin Key Handling Policy

Policy:

- Khala admin keys are operator-only secrets.
- Admin keys must never be committed to git or exposed in client-side env.
- Canonical storage is Google Secret Manager:
  `oa-khala-nonprod-admin-key`.
- Instance secret (`oa-khala-nonprod-instance-secret`) is a separate secret
  and remains backend-only.

Generate and rotate admin key:

```bash
INSTANCE_SECRET="$(gcloud secrets versions access latest \
  --secret=oa-khala-nonprod-instance-secret \
  --project openagentsgemini)"

ADMIN_KEY="$(cargo run -q -p keybroker --bin generate_key -- \
  khala-nonprod "$INSTANCE_SECRET")"

printf '%s' "$ADMIN_KEY" | gcloud secrets versions add \
  oa-khala-nonprod-admin-key \
  --data-file=- \
  --project openagentsgemini
```

Notes:

- Run from `~/code/khala/khala-backend` for `cargo run`.
- Grant access to `oa-khala-nonprod-admin-key` only to operator identities.
- Do not inject admin key into Cloud Run runtime env.

## Export / Import Validation (Non-Prod)

Build an env file for CLI commands:

```bash
BACKEND_URL="$(gcloud run services describe oa-khala-backend-nonprod \
  --project openagentsgemini \
  --region us-central1 \
  --format='value(status.url)')"

ADMIN_KEY="$(gcloud secrets versions access latest \
  --secret=oa-khala-nonprod-admin-key \
  --project openagentsgemini)"

cat > /tmp/khala-nonprod-self-hosted.env <<EOF
KHALA_SELF_HOSTED_URL="$BACKEND_URL"
KHALA_SELF_HOSTED_ADMIN_KEY="$ADMIN_KEY"
EOF
```

Run verification from a Khala app workspace:

```bash
npx khala dev --once --env-file /tmp/khala-nonprod-self-hosted.env

EXPORT_PATH="/tmp/khala-nonprod-export-$(date +%Y%m%d-%H%M%S).zip"
npx khala export --env-file /tmp/khala-nonprod-self-hosted.env --path "$EXPORT_PATH"

npx khala import --env-file /tmp/khala-nonprod-self-hosted.env --append "$EXPORT_PATH"
```

2026-02-19 evidence:

- `npx khala dev --once`: `Khala functions ready`.
- `npx khala export`: snapshot created and downloaded to
  `/tmp/khala-nonprod-export-20260219-131722.zip`.
- `npx khala import --append`: completed with `Added 0 documents`.

## Upgrade Runbook

1. Mirror new Khala backend/dashboard images into Artifact Registry.
2. Update pinned image references in:
   - `apps/openagents-runtime/deploy/khala/provision-nonprod-gcp.sh`
   - `apps/openagents-runtime/deploy/khala/README.md`
3. Validate dry-run:
   - `apps/openagents-runtime/deploy/khala/provision-nonprod-gcp.sh`
4. Apply:
   - `OA_KHALA_APPLY=1 apps/openagents-runtime/deploy/khala/provision-nonprod-gcp.sh`
5. Verify:
   - `apps/openagents-runtime/deploy/khala/check-nonprod-health.sh`
   - `npx khala dev --once --env-file /tmp/khala-nonprod-self-hosted.env`
6. Export a post-upgrade snapshot.

## Rollback Runbook

Image/config rollback:

1. Identify previous ready revisions:
   - `gcloud run revisions list --project openagentsgemini --region us-central1 --service oa-khala-backend-nonprod`
   - `gcloud run revisions list --project openagentsgemini --region us-central1 --service oa-khala-dashboard-nonprod`
2. Route traffic to known-good revisions:
   - `gcloud run services update-traffic oa-khala-backend-nonprod --project openagentsgemini --region us-central1 --to-revisions <REVISION>=100`
   - `gcloud run services update-traffic oa-khala-dashboard-nonprod --project openagentsgemini --region us-central1 --to-revisions <REVISION>=100`
3. Re-pin provisioning script to prior known-good images if needed and re-apply.

Data rollback (if required):

1. Use a known-good snapshot ZIP.
2. Import with replace semantics:
   - `npx khala import --env-file /tmp/khala-nonprod-self-hosted.env --replace-all -y <SNAPSHOT_ZIP>`
3. Re-run health checks and CLI smoke tests.

## Runtime Projection Rebuild (Drop + Replay)

When Khala summary projections drift from runtime truth, rebuild from runtime
durable history.

Run from `apps/openagents-runtime/`:

```bash
mix runtime.khala.reproject --run-id <run_id>
mix runtime.khala.reproject --worker-id <worker_id>
mix runtime.khala.reproject --all
```

Behavior:

- rebuild drops runtime checkpoint state for the selected entity/scope
- projector replays deterministic summary from runtime Postgres state/events
- replay emits telemetry for observability:
  - `openagents_runtime.khala.projection.replay.count`
  - `openagents_runtime.khala.projection.replay.duration_ms`

Projection health metrics to watch during/after rebuild:

- `openagents_runtime.khala.projection.lag_events`
- `openagents_runtime.khala.projection.write.count` (filter `result=error`)
- `openagents_runtime.khala.projection.write_failure.count`
- `openagents_runtime.khala.projection.drift.count`

## Gate G7 Drill Automation

Use the helper scripts for repeatable drills:

- backup/restore drill:
  - `apps/openagents-runtime/deploy/khala/run-backup-restore-drill.sh`
- rollback drill (dry-run by default):
  - `apps/openagents-runtime/deploy/khala/run-rollback-drill.sh`
- rollback drill (apply):
  - `OA_KHALA_ROLLBACK_DRILL_APPLY=1 apps/openagents-runtime/deploy/khala/run-rollback-drill.sh`
- runtime replay drill:
  - `apps/openagents-runtime/deploy/khala/run-runtime-replay-drill.sh`

Evidence reports:

- `apps/openagents-runtime/docs/reports/2026-02-19-khala-runtime-projector-load-chaos-report.md`
- `apps/openagents-runtime/docs/reports/2026-02-19-khala-g7-backup-restore-replay-rollback-drill.md`

## Staged Rollout Plan (Internal -> Limited -> Full)

Phase A: Internal users only

1. Route 100% of OpenAgents staff/admin users to Khala-backed sync surfaces.
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
3. Stop ramp immediately if any critical Khala projection alert fires for >5m.

## Rollback Drill Objective and Evidence

RTO target:

- rollback to known-good Khala revisions + health restoration in <= 5 minutes.

2026-02-19 non-prod measured drill:

- backend rollback revision: `oa-khala-backend-nonprod-00010-kpf`
- dashboard rollback revision: `oa-khala-dashboard-nonprod-00001-8rs`
- restored target revisions:
  - backend: `oa-khala-backend-nonprod-00011-v4r`
  - dashboard: `oa-khala-dashboard-nonprod-00002-hwm`
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
   - owns Laravel Khala token minting path and client auth bridge.

Escalation order:

1. Runtime on-call triages first and classifies incident lane:
   - projector lag/drift/error/replay
   - Khala infra/service availability
   - auth token bridge
2. If infra lane: page Infra/SRE on-call immediately.
3. If auth lane: page Web platform on-call immediately.
4. If blast radius exceeds canary cohort or Sev1 impact:
   - execute rollback drill path,
   - notify engineering incident commander and product owner,
   - freeze rollout progression until post-incident review.

## Security Hardening Controls

Admin key handling:

1. Khala admin key remains in Secret Manager only (`oa-khala-nonprod-admin-key`).
2. Cloud Run service envs must not contain `KHALA_SELF_HOSTED_ADMIN_KEY`, `KHALA_ADMIN_KEY`, or `ADMIN_KEY`.
3. Runtime projection sink credentials are operator-managed and never issued to end-user clients.

Least privilege and network boundaries:

1. Backend and dashboard run under dedicated service accounts (not default compute SA).
2. Backend DB connectivity is constrained through the `cloud-sql-proxy` sidecar.
3. Runtime ingress network policy remains restricted to trusted control-plane clients:
   - `apps/openagents-runtime/deploy/k8s/base/networkpolicy-ingress.yaml`

## MCP Production Access Control

Default posture:

- production MCP access is denied by default.

Gate command:

```bash
apps/openagents-runtime/deploy/khala/mcp-production-access-gate.sh
```

Temporary enablement requirements:

1. `OA_KHALA_MCP_PROD_ACCESS_ENABLED=1`
2. `OA_CHANGE_TICKET=<ticket>`
3. `OA_MCP_PROD_ACCESS_REASON=<reason>`
4. `OA_MCP_PROD_ACCESS_TTL_MINUTES=<1..60>`
5. `OA_MCP_PROD_ACKNOWLEDGE_RISK=YES`

Without these fields the gate exits non-zero and access remains blocked.

## Secret-Handling Audit Path

Runtime sanitization guards secret/PII surfaces across:

1. run events and codex worker payloads,
2. tool task inputs/outputs,
3. telemetry metadata,
4. trace capture payloads.

Validation tests:

- `apps/openagents-runtime/test/openagents_runtime/security/sanitizer_test.exs`
- `apps/openagents-runtime/test/openagents_runtime/security/sanitization_integration_test.exs`

## Security Review Checklist Command

Run this checklist before production rollout:

```bash
apps/openagents-runtime/deploy/khala/run-security-review-checklist.sh
```

Current evidence artifact:

- `apps/openagents-runtime/docs/reports/2026-02-19-khala-security-review-checklist.md`
