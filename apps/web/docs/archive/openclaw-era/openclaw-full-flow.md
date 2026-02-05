# OpenClaw Full System Flow (Hatchery + Runtime)

This doc explains the end‑to‑end OpenClaw flow so future agents can debug the entire system quickly. It intentionally omits any secret values.

## Components

### 1) Web app (Hatchery UI)
- Location: `apps/web/`
- Uses Convex for data access and for server‑side OpenClaw calls.
- UI pages that trigger OpenClaw calls typically request:
  - Runtime status (`/openclaw/runtime/status`)
  - Runtime devices (`/openclaw/runtime/devices`)
  - Instance state (`/openclaw/instance`)

### 2) Convex (actions + storage)
- Location: `apps/web/convex/`
- Actions in `openclawApi.ts` call the API worker over HTTPS.
- Convex stores:
  - Thread lists and messages
  - OpenClaw instance records and secrets
- **Important**: Convex actions run on Convex infra and **do not read `.env.local`**. Production env vars must be set in the Convex deployment.

### 3) API Worker (Rust)
- Location: `apps/api/`
- Route: `https://openagents.com/api/*`
- Handles OpenClaw routes (instance, runtime status/devices, etc).
- Auths Convex requests using an internal key (headers or query param).
- Proxies runtime calls to the OpenClaw runtime worker.

### 4) OpenClaw Runtime Worker
- Location: `apps/openclaw-runtime/`
- Route: (runtime URL, configured via `OPENCLAW_RUNTIME_URL`)
- Handles `/v1/*` endpoints (status, devices, tools, etc).
- Auths using a **service token** (`OPENAGENTS_SERVICE_TOKEN`).

## End‑to‑End Request Flow

### Runtime status / devices
1. **UI** calls Convex action in `openclawApi.ts`.
2. **Convex Action** builds the API URL and attaches the internal key:
   - `X-OA-Internal-Key` header
   - `Authorization: Bearer <internal-key>` header
   - `oa_internal_key` query param (cache‑busting param also added)
3. **API Worker** validates the internal key.
4. **API Worker** fetches runtime status/devices:
   - Uses `OPENCLAW_RUNTIME_URL` to call the runtime worker
   - Uses `OPENAGENTS_SERVICE_TOKEN` to authenticate to the runtime
5. **Runtime Worker** returns result to API worker.
6. **API Worker** returns JSON back to Convex.
7. **Convex Action** returns data to the UI.

### Instance lookup
1. **UI** calls Convex action `openclawApi.getInstance`.
2. **Convex Action** calls API worker `/openclaw/instance` with the internal key.
3. **API Worker** queries Convex control endpoints for the user instance.
4. **API Worker** returns instance summary or `null`.

## Required Environment Variables

### Convex (production)
- `OA_INTERNAL_KEY` (internal key sent to API worker)
- `PUBLIC_API_URL` (base URL; should be `https://openagents.com/api`)

### API Worker (apps/api)
- `OA_INTERNAL_KEY` (must match Convex)
- `OPENCLAW_RUNTIME_URL` (runtime worker URL)
- `OPENAGENTS_SERVICE_TOKEN` (service token for runtime auth)
- `CONVEX_SITE_URL` (Convex control endpoint URL)

### Runtime Worker (apps/openclaw-runtime)
- `OPENAGENTS_SERVICE_TOKEN` (must match API worker)

## Authentication Summary

### Convex → API Worker
- Internal key is accepted from:
  1. `X-OA-Internal-Key`
  2. `Authorization: Bearer <internal-key>`
  3. `oa_internal_key` query param

### API Worker → Runtime
- Service token required:
  - Header: `x-openagents-service-token`
  - Or `Authorization: Bearer <service-token>`

## Common Failure Modes

### 1) 401 from API worker
Likely causes:
- `OA_INTERNAL_KEY` mismatch between Convex and API worker
- Convex not using prod env vars
- Internal key stripped by upstream proxy (now mitigated via Bearer + query param)

### 2) 401 from runtime
Likely causes:
- `OPENAGENTS_SERVICE_TOKEN` mismatch between API worker and runtime
- Runtime not redeployed after token update

### 3) 404 “instance not found”
Normal when no OpenClaw instance exists for the user yet.

### 4) Cache‑served errors
Mitigated by:
- cache‑busting query params on Convex actions
- strict no‑cache headers on API responses

## Deployment Notes

### Convex
- Use `cd apps/web && npm run deploy` (do **not** run raw `npx convex deploy`).

### API Worker
- `cd apps/api && npx wrangler deploy`

### Runtime Worker
- `cd apps/openclaw-runtime && npx wrangler deploy`
- Requires Docker running (runtime uses containers).

