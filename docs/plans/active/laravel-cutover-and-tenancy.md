# Laravel Cutover + Multi-Tenant Deployment Strategy

Date: 2026-02-16
Scope: `apps/openagents.com` (Laravel 12 + Inertia + React + laravel/ai)

This document covers:
- How we run Laravel alongside the existing Effuse/Cloudflare app
- What a safe cutover looks like
- How we support user sites on subdomains with their own Laravel installs (initially)

## Current Reality

- Primary product remains the existing Effuse/Cloudflare stack.
- Laravel app is deployed to GCP Cloud Run as `openagents-web`.
- We will validate streaming (SSE) and tool correctness (L402 buyer MVP) in staging before routing real traffic.

## Cutover Plan (Safe, Reversible)

### Phase A: Staging and Internal Validation

1. Keep Cloud Run URL as staging entrypoint:
   - Example: `https://openagents-web-...run.app`
2. Add a staging domain mapping when ready:
   - `next.openagents.com` -> Cloud Run `openagents-web`
3. Verify:
   - `/up` health
   - `/api/smoke/stream` SSE smoke (gated by header secret)
   - `php artisan demo:l402 --preset=fake ...` (deterministic)
   - Real L402 buying (once real InvoicePayer is configured)

### Phase B: Limited Routing

1. Route only internal users to the Laravel app.
   - Option: a separate subdomain (`next.openagents.com`) and internal comms
   - Option: edge routing rules for a user allowlist
2. Keep Effuse app as the default.
3. Ensure we can roll back instantly by removing the allowlist routing.

### Phase C: Primary Cutover

1. When Laravel reaches feature parity for the critical workflows:
   - Move `openagents.com` / `app.openagents.com` to the Laravel service.
2. Keep Effuse stack live for rollback (at least one full release cycle).
3. Preserve request correlation:
   - Keep emitting a request/run id header (e.g. `x-oa-run-id`).

### Rollback

Rollback is intentionally DNS/edge-only:
- Switch the domain mapping back to the Effuse app.
- Do not attempt DB rollback; keep migrations forward-only.

## Multi-Tenant: User Sites on Subdomains

### Initial Recommendation: Service-Per-Tenant

For MVP, prefer isolation:
- Each tenant gets its own Cloud Run service deployment
- Each tenant gets its own Postgres database (Cloud SQL)
- Each tenant gets its own Secret Manager entries

This is operationally boring and debuggable, which matters more than per-tenant cost initially.

#### Why Service-Per-Tenant First

- Clear blast radius boundaries
- Easy per-tenant secrets and key rotation
- Easy per-tenant rate limits and spend caps
- No complex tenant-routing middleware required on day 1

### Naming Conventions

Cloud Run services:
- `tenant-<slug>-web`
- `tenant-<slug>-migrate` (job)

Cloud SQL:
- database: `tenant_<slug>`
- user: `tenant_<slug>`

Secret Manager:
- `tenant-<slug>-app-key`
- `tenant-<slug>-db-password`
- `tenant-<slug>-openrouter-api-key`
- `tenant-<slug>-workos-client-id` (optional)
- `tenant-<slug>-workos-api-key` (optional)

Domains:
- `<slug>.openagents.com` -> `tenant-<slug>-web`

### Later (Optional): Shared Runtime with Tenant Routing

Only consider a shared runtime if we are forced by cost/scale.

Tradeoffs:
- Harder to isolate incidents
- More complex migrations and deploys
- Multi-tenant security mistakes become catastrophic

If we go here, we will need:
- strict tenant routing (host -> tenant id)
- per-tenant encryption keys (not just per-row)
- strong request auditing and abuse controls

## Operational Commands (Single-Tenant)

Build and push image:

```bash
gcloud builds submit \
  --config apps/openagents.com/deploy/cloudbuild.yaml \
  --substitutions _TAG="$(git rev-parse --short HEAD)" \
  apps/openagents.com
```

Run migrations:

```bash
gcloud run jobs execute openagents-migrate \
  --project openagentsgemini \
  --region us-central1
```

Smoke:

```bash
OPENAGENTS_BASE_URL="https://openagents-web-...run.app" \
  OA_SMOKE_SECRET="..." \
  apps/openagents.com/deploy/smoke/health.sh

OPENAGENTS_BASE_URL="https://openagents-web-...run.app" \
  OA_SMOKE_SECRET="..." \
  apps/openagents.com/deploy/smoke/stream.sh
```
