# Managed OpenClaw implementation log (2026-02-02)

## Scope
End-to-end MVP wiring for managed OpenClaw across Convex (data + secrets + billing), Rust API worker (`/api/openclaw/*`), runtime template (service-token-gated instance API), and OpenAgents website UI + tests. Includes follow-up rename of runtime env vars to remove all `CLAWDBOT` references.

---

## Phase A — Convex schema + functions (apps/website/convex)

### Schema updates
File: `apps/website/convex/schema.ts`
- Added table `openclaw_instances` with fields:
  - `user_id`, `status`, `runtime_url`, `runtime_name`
  - Cloudflare identifiers: `cf_account_id`, `cf_worker_name`, `cf_worker_id`, `cf_container_app_id`, `cf_container_app_name`
  - R2 bucket: `r2_bucket_name`
  - Encrypted secret storage: `service_token_encrypted`, `service_token_iv`, `service_token_alg`, `provider_keys_encrypted`, `provider_keys_iv`, `provider_keys_alg`
  - Timestamps: `created_at`, `updated_at`, `last_ready_at`
  - Indexes: `by_user_id`, `by_status`
- Added table `credit_ledger` with fields:
  - `user_id`, `kind`, `amount_usd`, `meta`, `created_at`
  - Indexes: `by_user_id`, `by_user_id_created_at`, `by_user_id_kind`

### OpenClaw Convex module
File: `apps/website/convex/openclaw.ts`
- Added AES‑GCM encryption helper for secrets:
  - `OPENCLAW_ENCRYPTION_KEY` required (32‑byte key, hex or base64)
  - `storeEncryptedSecret` → encrypts and stores in `openclaw_instances`
  - `getDecryptedSecret` → decrypts on server only
- Added instance management functions:
  - `getInstanceForUser`
  - `upsertInstance`
  - `setInstanceStatus`
- Status validation and `last_ready_at` updates on transition to `ready`.

### Billing Convex module
File: `apps/website/convex/billing.ts`
- Added ledger math helpers (`roundUsd`, positive validation).
- Added operations:
  - `getCreditBalance`
  - `grantMonthlyCredits`
  - `burnCredits`
- Balance computed from `credit_ledger` sum.

### Convex control endpoints
Files:
- `apps/website/convex/control_http.ts`
- `apps/website/convex/http.ts`

Added OpenClaw control endpoints (all gated by `OA_CONTROL_KEY`):
- `GET /control/openclaw/instance`
- `POST /control/openclaw/instance`
- `POST /control/openclaw/instance/status`
- `POST /control/openclaw/instance/secret`
- `GET /control/openclaw/instance/secret`
- `GET /control/openclaw/billing/summary`

Also added helpers:
- `extractUserId` (reads `x-oa-user-id`, query params, or body)
- `sanitizeOpenclawInstance` (removes encrypted fields from responses)

---

## Phase B — Rust API Worker `/api/openclaw/*` (apps/api)

### New OpenClaw module
Directory: `apps/api/src/openclaw/`
- `mod.rs`: module exports + shared header constants
- `http.rs`: handlers for instance, runtime, and billing routes; beta auth
- `convex.rs`: Convex control bridge helpers (instance + secret + billing)
- `runtime_client.rs`: service-token-gated runtime HTTP client
- `cf.rs`: Cloudflare provisioning stub (env‑driven runtime URL)
- `billing.rs`: billing wrapper

### Routing + CORS
File: `apps/api/src/lib.rs`
- Added routes:
  - `GET /openclaw`
  - `GET /openclaw/instance`
  - `POST /openclaw/instance`
  - `GET /openclaw/runtime/status`
  - `GET /openclaw/runtime/devices`
  - `POST /openclaw/runtime/devices/:requestId/approve`
  - `POST /openclaw/runtime/backup`
  - `POST /openclaw/runtime/restart`
  - `GET /openclaw/billing/summary`
- Extended CORS allow headers for:
  - `x-oa-internal-key`, `x-oa-user-id`, `x-openagents-service-token`

### Beta auth
- `X-OA-Internal-Key` validated against `OA_INTERNAL_KEY`
- `X-OA-User-Id` required and used for Convex access

### Provisioning behavior
- Minimal stub in `openclaw/cf.rs`:
  - Reads runtime URL from env: `OPENCLAW_RUNTIME_URL` or `OPENCLAW_RUNTIME_URL_TEMPLATE`
  - Optional runtime name + R2 bucket name via env
- `POST /openclaw/instance`:
  - Creates provisioning record
  - Generates service token (or uses `OPENCLAW_SERVICE_TOKEN`/`OPENAGENTS_SERVICE_TOKEN` if set)
  - Stores encrypted token in Convex
  - Marks instance `ready`

---

## Phase C — Runtime template (moltworker)

### Internal-only runtime API
File: `/home/christopherdavid/code/moltworker/src/index.ts`
- Replaced UI + proxy with service-token-gated internal API only.
- New mount: `/v1/*` for OpenAgents control plane.

### Service token auth
File: `/home/christopherdavid/code/moltworker/src/auth/serviceToken.ts`
- Requires `X-OpenAgents-Service-Token`
- Returns OpenClaw envelope `{ ok, data | error }` on failure

### Runtime endpoints
File: `/home/christopherdavid/code/moltworker/src/routes/runtime.ts`
- `GET /v1/status` (gateway state, last backup, instance type, version)
- `POST /v1/gateway/restart`
- `POST /v1/storage/backup`
- `GET /v1/devices`
- `POST /v1/devices/:requestId/approve`

### Env + doc updates (rename to OPENCLAW_*)
- No `CLAWDBOT` strings remain.
- Replaced env names:
  - `OPENCLAW_GATEWAY_TOKEN`
  - `OPENCLAW_DEV_MODE`
  - `OPENCLAW_BIND_MODE`
  - `OPENCLAW_VERSION`
- Updated mappings in:
  - `/home/christopherdavid/code/moltworker/start-moltbot.sh`
  - `/home/christopherdavid/code/moltworker/src/gateway/env.ts`
  - `/home/christopherdavid/code/moltworker/src/gateway/env.test.ts`
  - `/home/christopherdavid/code/moltworker/src/routes/runtime.ts`
  - `/home/christopherdavid/code/moltworker/src/routes/debug.ts`
  - `/home/christopherdavid/code/moltworker/src/types.ts`
  - `/home/christopherdavid/code/moltworker/AGENTS.md`

---

## Phase D — Website UI + API wrapper (apps/website/src)

### OpenClaw API wrapper
File: `apps/website/src/lib/openclawApi.ts`
- Server functions for all `/api/openclaw/*` endpoints
- Internal header helpers:
  - `buildInternalHeaders` (`X-OA-Internal-Key`, `X-OA-User-Id`)
  - `buildServiceTokenHeader`
- Default env lookup:
  - `OA_INTERNAL_KEY`
  - `OA_INTERNAL_USER_ID`
- Shared `roundUsd` helper

### UI components
Directory: `apps/website/src/components/openclaw/`
- `InstanceStatusCard`
- `ProvisioningStepper`
- `DeviceList`
- `CreditsWidget`

### Routes
New routes in `apps/website/src/routes/_app/`:
- `openclaw.tsx` (overview)
- `openclaw.create.tsx` (provisioning)
- `openclaw.security.tsx` (pairing approvals)
- `openclaw.usage.tsx` (backup + restart)
- `openclaw.billing.tsx` (credits summary)

### Navigation
- Added OpenClaw entry to sidebar: `apps/website/src/components/AppSidebar.tsx`
- Router tree regenerated: `apps/website/src/routeTree.gen.ts`

---

## Tests

### Website tests
File: `apps/website/src/lib/openclawApi.test.ts`
- Unit tests for:
  - `roundUsd`
  - `buildInternalHeaders`
  - `buildServiceTokenHeader`

### Vitest configuration
File: `apps/website/vite.config.ts`
- Added `test` configuration with Node environment
- Avoided Cloudflare plugin in test mode to prevent module loader issues

### Runtime tests
- Updated env mapping tests in moltworker (`src/gateway/env.test.ts`)

---

## Verification runs

### In `apps/api`
- `cargo check` (success; warning about unused fields in `runtime_client.rs`)

### In `apps/website`
- `pnpm test` (success; minor `punycode` deprecation warning)

### In `/home/christopherdavid/code/moltworker`
- `npm test` (success)

---

## Git operations

### openagents repo
- Commit: **Add managed OpenClaw API, Convex, and UI**
- Pushed to `main`

### moltworker repo
- Commit: **Rename runtime envs and add OpenClaw service API**
- Push **failed** due to missing GitHub credentials (manual push required)

---

## Environment variables introduced / updated

### Convex (apps/website)
- `OPENCLAW_ENCRYPTION_KEY` (32‑byte key, hex or base64)

### Rust API (apps/api)
- `OA_INTERNAL_KEY`
- `OPENCLAW_RUNTIME_URL` or `OPENCLAW_RUNTIME_URL_TEMPLATE`
- Optional: `OPENCLAW_R2_BUCKET_PREFIX`, `OPENCLAW_RUNTIME_NAME_PREFIX`
- Optional: `OPENCLAW_SERVICE_TOKEN` or `OPENAGENTS_SERVICE_TOKEN`

### Runtime (moltworker)
- `OPENAGENTS_SERVICE_TOKEN`
- `OPENCLAW_GATEWAY_TOKEN`
- `OPENCLAW_DEV_MODE`
- `OPENCLAW_BIND_MODE`
- `OPENCLAW_VERSION`

---

## Files added/changed (high level)

### Added
- `apps/api/src/openclaw/*`
- `apps/website/convex/openclaw.ts`
- `apps/website/convex/billing.ts`
- `apps/website/src/lib/openclawApi.ts`
- `apps/website/src/lib/openclawApi.test.ts`
- `apps/website/src/components/openclaw/*`
- `apps/website/src/routes/_app/openclaw*.tsx`
- `/home/christopherdavid/code/moltworker/src/auth/serviceToken.ts`
- `/home/christopherdavid/code/moltworker/src/routes/runtime.ts`

### Modified
- `apps/website/convex/schema.ts`
- `apps/website/convex/control_http.ts`
- `apps/website/convex/http.ts`
- `apps/api/src/lib.rs`
- `apps/website/src/components/AppSidebar.tsx`
- `apps/website/src/routeTree.gen.ts`
- `apps/website/vite.config.ts`
- `/home/christopherdavid/code/moltworker/start-moltbot.sh`
- `/home/christopherdavid/code/moltworker/src/index.ts`
- `/home/christopherdavid/code/moltworker/src/gateway/env.ts`
- `/home/christopherdavid/code/moltworker/src/gateway/env.test.ts`
- `/home/christopherdavid/code/moltworker/src/routes/debug.ts`
- `/home/christopherdavid/code/moltworker/src/types.ts`
- `/home/christopherdavid/code/moltworker/AGENTS.md`
