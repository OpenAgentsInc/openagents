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
