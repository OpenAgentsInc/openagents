# Rust Control Service Maintenance Mode Runbook

Status: active  
Owner: `owner:openagents.com`, `owner:infra`  
Issue: OA-RUST-112 (`#1937`)

## Purpose

Provide a safe cutover workflow where public users receive a maintenance page (`503`) while operators can still validate production behavior through a secure bypass token.

## Maintenance behavior

When `OA_MAINTENANCE_MODE_ENABLED=true`:

1. Non-allowlisted requests return the branded maintenance HTML page with `503 Service Unavailable`.
2. Allowlisted paths (default: `/healthz`, `/readyz`) remain reachable.
3. Operators can bypass maintenance with:
   - `/?maintenance_bypass=<token>` to bootstrap a secure HttpOnly cookie
   - subsequent requests carrying that cookie.

Security properties:

1. Bypass token is loaded from `OA_MAINTENANCE_BYPASS_TOKEN` (secret-backed in Cloud Run).
2. Bypass cookie is signed (HMAC), TTL-bound, and includes `Secure; HttpOnly; SameSite=Lax`.
3. Raw bypass token is never emitted in responses.

## Canonical helper

Use:

- `apps/openagents.com/service/deploy/maintenance-mode.sh`

### Enable maintenance mode

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
MAINTENANCE_BYPASS_TOKEN='<rotate-this-token>' \
apps/openagents.com/service/deploy/maintenance-mode.sh enable
```

Notes:

1. The helper creates/updates Secret Manager secret `openagents-control-maintenance-bypass-token` (or `MAINTENANCE_BYPASS_SECRET_NAME`) and binds it to `OA_MAINTENANCE_BYPASS_TOKEN`.
2. Use `MAINTENANCE_ALLOWED_PATHS` to expand allowlisted health/control endpoints if needed.

### Disable maintenance mode

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
apps/openagents.com/service/deploy/maintenance-mode.sh disable
```

### Inspect current status

```bash
PROJECT=openagentsgemini \
REGION=us-central1 \
SERVICE=openagents-control-service-staging \
apps/openagents.com/service/deploy/maintenance-mode.sh status
```

## Staging rehearsal checklist

Resolve and validate the staging base URL first:

```bash
gcloud beta run domain-mappings describe \
  --project openagentsgemini \
  --region us-central1 \
  --domain staging.openagents.com
```

If `staging.openagents.com` is not mapped to the Rust staging service or `Ready` is not `True`, rehearse against the Cloud Run staging service URL and record the domain-mapping status in the report artifact.

1. Enable maintenance mode on `openagents-control-service-staging`.
2. Validate public behavior:
   - `curl -i <staging-base-url>/` returns `503`.
3. Validate bypass bootstrap:
   - `curl -i -c /tmp/oa-maint.cookie "<staging-base-url>/?maintenance_bypass=<token>"`
   - response returns redirect with `Set-Cookie`.
4. Validate bypass access:
   - `curl -i -b /tmp/oa-maint.cookie <staging-base-url>/manifest.json` returns `200`.
5. Run smoke suite through bypass session:
   - `OPENAGENTS_BASE_URL=<staging-base-url> OPENAGENTS_MAINTENANCE_BYPASS_TOKEN=<token> apps/openagents.com/service/deploy/smoke-control.sh`
6. Disable maintenance mode and verify normal access is restored.

Staging rehearsal evidence:

- `apps/openagents.com/service/docs/reports/2026-02-21-maintenance-mode-staging-rehearsal.md`

## Production cutover workflow

1. Re-run Rust cutover validation gate:
   - `scripts/release/validate-rust-cutover.sh`
2. Enable maintenance mode on `openagents-control-service`.
3. Confirm public `openagents.com` returns maintenance page.
4. Validate production via bypass token + smoke checks.
5. Execute OA-RUST-111 Phase C legacy deletion actions.
6. Disable maintenance mode when cutover validation is complete.

## Rollback

If production validation fails while in maintenance mode:

1. Keep maintenance mode enabled.
2. Execute control-service rollback (revision traffic rollback) per:
   - `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`
3. Re-run smoke checks through bypass.
4. Disable maintenance mode only after stable recovery.
