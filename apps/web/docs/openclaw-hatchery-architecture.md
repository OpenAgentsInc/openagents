# OpenClaw + Hatchery: Architecture and Implementation

This doc explains how OpenClaw instance get/create works from the Hatchery UI through Convex and the API worker, and documents everything that was implemented to make it work and debuggable.

**Canonical product/roadmap:** `openclaw-on-openagents-com.md`. This doc is the **technical architecture and implementation detail** for the Hatchery ↔ OpenClaw path.

---

## Current status (as of last update)

- **Implemented and working:** Hatchery “Create your OpenClaw” panel; Convex actions `openclawApi.getInstance` / `openclawApi.createInstance`; Convex HTTP routes `/control/openclaw/*`; API worker GET/POST `/openclaw/instance` with Convex bridge; access gating (`access.getStatus`); admin panel at `/admin`; env vars and secrets (Convex + API); error logging and prefixed API error messages; TS fixes (openclaw.instance null check, Hatchery `/kb/$slug` links).
- **What “ready” means today:** Instance row is stored in Convex with `status: ready` and `runtime_url` (from API’s `OPENCLAW_RUNTIME_URL`). No per-user container is started; provision only writes metadata. OpenClaw Chat (streaming) is now live at `/openclaw/chat`; device pairing is still pending. Hatchery shows a “Provisioning complete” blurb and links to OpenClaw Chat.
- **Critical:** API worker `CONVEX_SITE_URL` must point at the **same** Convex deployment that serves the web app (e.g. `https://effervescent-anteater-82.convex.site`). If it pointed at a different deployment, getInstance returned 500; that was fixed and the API was redeployed.
- **Verify end-to-end / TS:** Done (Convex + API deployed; TS passes). Remaining work is in “What needs to be done next” below.

---

## 1. Architecture overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  Browser (Hatchery)                                                         │
│  - Calls Convex actions: openclawApi.getInstance / openclawApi.createInstance│
└─────────────────────────────────────┬─────────────────────────────────────┘
                                       │
                                       ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│  Convex (apps/web)                                                           │
│  - openclawApi.ts: actions that HTTP-call the API with OA internal auth     │
│  - openclaw_control_http.ts: HTTP handlers for /control/openclaw/*           │
│  - openclaw.ts: internal mutations/queries (openclaw_instances, secrets)     │
│  - billing.ts: internal query (credit_ledger for billing summary)           │
└─────────────────────────────────────┬─────────────────────────────────────┘
                                       │
         ┌─────────────────────────────┼─────────────────────────────┐
         │                             │                             │
         ▼                             ▼                             ▼
┌─────────────────────┐   ┌─────────────────────────────┐   ┌─────────────────────┐
│  From Hatchery:     │   │  From API worker (Rust):    │   │  Convex DB          │
│  Convex actions     │   │  HTTP to CONVEX_SITE_URL     │   │  openclaw_instances  │
│  fetch(API_URL +    │   │  /control/openclaw/instance  │   │  credit_ledger       │
│  /openclaw/instance) │   │  with x-oa-control-key      │   │  (via internal       │
│  with X-OA-Internal-│   │  (CONVEX_CONTROL_KEY)        │   │   openclaw/billing)  │
│  Key, X-OA-User-Id  │   │                              │   │                     │
└──────────┬──────────┘   └──────────────┬──────────────┘   └─────────────────────┘
           │                              │
           ▼                              │
┌─────────────────────────────────────────────────────────────────────────────┐
│  API worker (Rust, apps/api) — openagents.com/api/*                          │
│  - GET/POST /openclaw/instance: require X-OA-Internal-Key + X-OA-User-Id      │
│  - For POST (create): get_instance → upsert(provisioning) → provision_instance│
│    → store_secret → upsert(ready). All Convex calls go to CONVEX_SITE_URL     │
│    with CONVEX_CONTROL_KEY (x-oa-control-key).                                │
│  - provision_instance reads OPENCLAW_RUNTIME_URL (or _TEMPLATE) and builds  │
│    ProvisionedInstance (no actual container spin-up in current impl).       │
└─────────────────────────────────────────────────────────────────────────────┘
```

**Why two paths into Convex?**

- **Path A (Hatchery → Convex action → API worker):** The Hatchery runs in the browser and calls Convex actions. Those actions need to call the **API** at `openagents.com/api` so that the API can enforce auth (internal key + user id) and orchestrate Convex + provisioning. The Hatchery does **not** call the API directly from the client (to avoid exposing the internal key and to avoid CORS/SSR issues).
- **Path B (API worker → Convex HTTP):** The API worker (Rust) does not use the Convex JS client; it calls Convex’s **HTTP** surface. That surface is the Convex app’s HTTP router (`convex/http.ts`) with routes under `/control/openclaw/*`. Those routes verify `x-oa-control-key` and forward to internal Convex functions (openclaw mutations/queries, billing query).

So: **Hatchery → Convex actions → API (Rust) → Convex HTTP control → Convex DB.**

---

## 2. Components and files

### 2.1 Web app (apps/web)

| Location | Purpose |
|----------|---------|
| `convex/openclawApi.ts` | Convex **actions** `getInstance` and `createInstance`. Called by Hatchery. They `fetch(PUBLIC_API_URL/openclaw/instance)` with `X-OA-Internal-Key` and `X-OA-User-Id`. Return instance summary or null (get) / throw with real API error (create). |
| `convex/http.ts` | Convex HTTP router. Registers `/control/openclaw/instance`, `/control/openclaw/instance/status`, `/control/openclaw/instance/secret`, `/control/openclaw/billing/summary` (GET/POST as needed). |
| `convex/openclaw_control_http.ts` | HTTP **handlers** for those routes. Each checks `x-oa-control-key` against `OA_CONTROL_KEY`, then calls `internal.openclaw.*` or `internal.billing.getCreditBalance`. Return JSON `{ ok, data?, error? }`. |
| `convex/openclaw.ts` | **Internal** Convex functions: `getInstanceForUser`, `upsertInstance`, `setInstanceStatus`, `storeEncryptedSecret`, `getDecryptedSecret`. Persist to `openclaw_instances` (and encrypted secret fields). Require `OPENCLAW_ENCRYPTION_KEY` for secrets. |
| `convex/billing.ts` | **Internal** `getCreditBalance` for billing summary. |
| `convex/schema.ts` | Defines `openclaw_instances` and `credit_ledger` tables. |
| `convex/control_auth.ts` | Shared `requireControlKey(request)` and handlers for `/control/auth/*` (resolve-token, agent/register). Same `OA_CONTROL_KEY` / `x-oa-control-key` contract. |
| Hatchery UI | Calls `api.openclawApi.getInstance` / `api.openclawApi.createInstance` (Convex client). Shows status or “Provision” and handles errors. |

### 2.2 API worker (apps/api, Rust)

| Location | Purpose |
|----------|---------|
| `src/lib.rs` | Registers `GET/POST /openclaw/instance`, etc. Implements `forward_convex_control`: HTTP request to `CONVEX_SITE_URL` + path with header `x-oa-control-key: CONVEX_CONTROL_KEY`. |
| `src/openclaw/http.rs` | `handle_instance_get`, `handle_instance_post`. Auth via `require_openclaw_user` (X-OA-Internal-Key + X-OA-User-Id). GET: convex get_instance → return summary. POST: get_instance → upsert(provisioning) → provision_instance → store_secret → upsert(ready). All Convex calls go through `openclaw/convex.rs`. |
| `src/openclaw/convex.rs` | `get_instance`, `upsert_instance`, `set_status`, `store_secret`, `get_secret`, `get_billing_summary`. All call `forward_convex_control` with paths like `control/openclaw/instance`, `control/openclaw/instance/status`, `control/openclaw/instance/secret`, `control/openclaw/billing/summary`. |
| `src/openclaw/cf.rs` | `provision_instance`: reads `OPENCLAW_RUNTIME_URL` (or `OPENCLAW_RUNTIME_URL_TEMPLATE` with `{user_id}`), optional `OPENCLAW_RUNTIME_NAME*`, `OPENCLAW_R2_BUCKET*`, etc. Returns `ProvisionedInstance` (no container creation in current impl). |

### 2.3 OpenClaw runtime (apps/openclaw-runtime)

Separate worker (Containers + Durable Objects). Referenced by `OPENCLAW_RUNTIME_URL`. The API’s “provision” step currently only configures URL/metadata and stores them in Convex; it does not create the runtime worker itself.

---

## 3. Request flows

### 3.1 Get instance (Hatchery loads)

1. Hatchery calls Convex action `openclawApi.getInstance`.
2. Action: auth (Convex identity + `access.getStatus`), then `fetch(API_BASE/openclaw/instance)` with `X-OA-Internal-Key`, `X-OA-User-Id`.
3. API worker: `require_openclaw_user` (validates internal key and user id), then `convex::get_instance(env, user_id)`.
4. API worker: `forward_convex_control(GET, control/openclaw/instance?user_id=...)` to Convex.
5. Convex HTTP: `handleInstanceGet` in `openclaw_control_http.ts` checks `x-oa-control-key`, then `ctx.runQuery(internal.openclaw.getInstanceForUser, { user_id })`.
6. Convex returns `{ ok: true, data: instance }` (or null). API returns that to the Convex action. Action returns instance summary or `null` to Hatchery (or throws on 4xx with API error message).
7. **Resilience:** If API returns 5xx or non-JSON, action logs with `[openclawApi getInstance]` and returns `null` so UI can show “unavailable” / retry.

### 3.2 Create instance (user clicks Provision)

1. Hatchery calls Convex action `openclawApi.createInstance`.
2. Action: same auth, then `fetch(API_BASE/openclaw/instance, { method: 'POST', ... })`.
3. API worker: `handle_instance_post`:
   - `require_openclaw_user`.
   - `convex::get_instance` → if existing and not error/deleted, return it.
   - `convex::upsert_instance` with `status: "provisioning"`.
   - `cf::provision_instance` (reads `OPENCLAW_RUNTIME_URL`; on failure sets status to error and returns 500 with clear message).
   - `convex::store_secret` (service_token).
   - `convex::upsert_instance` with `status: "ready"` and provisioned metadata.
4. Each Convex call from the API is `forward_convex_control` to the same Convex HTTP routes; handlers in `openclaw_control_http.ts` run the corresponding internal openclaw/billing functions.
5. **Errors:** Every failure in the API is returned as JSON `{ ok: false, error: "openclaw createInstance <step>: <detail>" }`. The Convex action throws that `error` string (and logs it), so the user sees the real cause (e.g. OPENCLAW_RUNTIME_URL not configured, convex error 404, OPENCLAW_ENCRYPTION_KEY not configured).

---

## 4. Environment variables and secrets

### 4.1 Convex (apps/web, Convex dashboard or `npx convex env set`)

| Variable | Required | Purpose |
|----------|----------|---------|
| `OA_INTERNAL_KEY` | Yes (for Hatchery path) | Sent by Convex actions as `X-OA-Internal-Key` when calling the API. Must match API worker’s `OA_INTERNAL_KEY`. |
| `OA_CONTROL_KEY` | Yes (for API → Convex) | Checked by `/control/openclaw/*` (and `/control/auth/*`) HTTP handlers. Must match API worker’s `CONVEX_CONTROL_KEY`. |
| `PUBLIC_API_URL` | No (has default) | Base URL for the API. Convex actions use this (or `OPENAGENTS_API_URL`) to build `fetch(…/openclaw/instance)`. Default in code: `https://openagents.com/api`. |
| `OPENAGENTS_API_URL` | No | Same as above, alternative name. |
| `OPENCLAW_ENCRYPTION_KEY` | Yes (for store_secret) | 32-byte hex (or base64) used to encrypt instance secrets (e.g. service_token) in `openclaw_instances`. |

### 4.2 API worker (apps/api, Wrangler vars + secrets)

| Variable | Required | Purpose |
|----------|----------|---------|
| `OA_INTERNAL_KEY` | Yes | Secret. Must match Convex `OA_INTERNAL_KEY`. Validates `X-OA-Internal-Key` on `/openclaw/*`. |
| `CONVEX_SITE_URL` | Yes | Base URL of Convex HTTP app (e.g. `https://<deployment>.convex.site`). All `forward_convex_control` requests go here. |
| `CONVEX_CONTROL_KEY` | Yes | Secret. Must match Convex `OA_CONTROL_KEY`. Sent as `x-oa-control-key` on every Convex control request. |
| `OPENCLAW_RUNTIME_URL` | Yes (for create) | URL of the OpenClaw runtime (e.g. `https://openclaw-runtime.<subdomain>.workers.dev`). Used by `provision_instance`. |
| `OPENCLAW_RUNTIME_URL_TEMPLATE` | Alternative | If set, can contain `{user_id}`; replaced per user. |
| Optional | No | `OPENCLAW_RUNTIME_NAME`, `OPENCLAW_RUNTIME_NAME_PREFIX`, `OPENCLAW_R2_BUCKET`, `OPENCLAW_R2_BUCKET_PREFIX`, `CF_ACCOUNT_ID`, `OPENCLAW_CF_WORKER_NAME`, `OPENCLAW_CF_CONTAINER_APP_NAME` for metadata. |

### 4.3 Web worker (apps/web, Wrangler)

For server-side or other callers that hit the API with the internal key: `OA_INTERNAL_KEY` (secret) if needed. Hatchery uses Convex actions, so the key lives in Convex, not necessarily in the web worker.

For `/chat` tool calls in `apps/web`, the web worker must also have the OpenClaw API base configured (e.g. `PUBLIC_API_URL=https://openagents.com/api` or `OPENCLAW_API_BASE=...`) so server-side tool execution does not fall back to same-origin `/api`.

---

## 5. What was implemented / fixed

### 5.1 Convex actions for Hatchery (openclawApi.ts)

- **Problem:** Hatchery used TanStack server functions to call the API; in some setups the request hit the document router and returned “Only HTML requests are supported,” so instance get/create failed.
- **Change:** Added Convex actions `openclawApi.getInstance` and `openclawApi.createInstance` that perform `fetch(API_BASE/openclaw/instance)` with internal key and user id. Hatchery calls these actions instead of server functions.
- **Details:**
  - API base: `PUBLIC_API_URL` or `OPENAGENTS_API_URL` or default `https://openagents.com/api`.
  - Internal key: `OA_INTERNAL_KEY` (or `OPENAGENTS_INTERNAL_KEY`); throws if missing.
  - getInstance: on 5xx or non-JSON, log with `[openclawApi getInstance]` and return `null`; on 4xx/!ok throw with API `error` message.
  - createInstance: on 5xx/non-JSON/parse error, log and throw with **actual** status and body (e.g. `OpenClaw API error (500): ...` or API `error` field).

### 5.2 Convex HTTP routes for API worker (openclaw_control_http.ts + http.ts)

- **Problem:** The API worker called `CONVEX_SITE_URL` with paths like `control/openclaw/instance`, but only `/control/auth/*` and `/nostr/ingest` existed. Convex returned 404/500 and the API returned “INTERNAL SERVER ERROR” to the client.
- **Change:** Implemented HTTP handlers for:
  - `GET /control/openclaw/instance?user_id=`
  - `POST /control/openclaw/instance` (body: user_id, status, runtime_*, cf_*, r2_bucket_name)
  - `POST /control/openclaw/instance/status` (body: user_id, status)
  - `POST /control/openclaw/instance/secret` (body: user_id, key, value)
  - `GET /control/openclaw/instance/secret?user_id=&key=`
  - `GET /control/openclaw/billing/summary?user_id=`
- Each handler: require `x-oa-control-key` === `OA_CONTROL_KEY`, then call the corresponding internal openclaw/billing function and return `{ ok, data?, error? }`.
- Registered all routes in `convex/http.ts`.

### 5.3 Environment and secrets

- Set in Convex (dev + prod): `OA_INTERNAL_KEY`, `OA_CONTROL_KEY`, `PUBLIC_API_URL`, `OPENCLAW_ENCRYPTION_KEY`.
- Set on API worker: Wrangler secret `OA_INTERNAL_KEY`, `CONVEX_CONTROL_KEY`; vars in `wrangler.toml`: `CONVEX_SITE_URL`, `OPENCLAW_RUNTIME_URL` (default `https://openclaw-runtime.openagents.workers.dev`; override if your runtime URL differs).
- Set on web worker: Wrangler secret `OA_INTERNAL_KEY` if anything server-side calls the API with it.

### 5.4 Error handling and debugging

- **Convex openclawApi:** All failure paths log with a `[openclawApi getInstance]` or `[openclawApi createInstance]` prefix and status/body. createInstance throws the **actual** API error text (or body slice) so the user and logs show the real cause.
- **Convex openclaw_control_http:** Every catch logs `console.error('[openclaw control ...]', e)` and returns `error` in JSON. Control key missing or wrong logs and returns a clear message (OA_CONTROL_KEY missing / invalid x-oa-control-key).
- **API worker (Rust):** Replaced `?` with explicit match in `handle_instance_get` and `handle_instance_post`. Every error returns 500 with a **prefixed** message, e.g.:
  - `openclaw getInstance: convex error 404: ...`
  - `openclaw createInstance get_instance: ...`
  - `openclaw createInstance upsert (provisioning): ...`
  - `openclaw createInstance provision_instance: OPENCLAW_RUNTIME_URL not configured`
  - `openclaw createInstance store_secret: ...`
  - `openclaw createInstance upsert (ready): ...`
- So Convex and the client receive a single, unambiguous string for each failure.

---

## 6. Debugging checklist

- **Hatchery shows “OpenClaw service temporarily unavailable” or null:**  
  Check Convex logs for `[openclawApi getInstance]` or `[openclawApi createInstance]` — they log API status and body. Then check API worker logs (e.g. `wrangler tail`) for the corresponding request.

- **createInstance fails with a specific message:**  
  The thrown error is the API’s `error` field. Search for the prefix:
  - `openclaw createInstance get_instance` → Convex HTTP (control) or Convex DB; check Convex logs for `[openclaw control GET /instance]` or POST.
  - `openclaw createInstance provision_instance` → Usually `OPENCLAW_RUNTIME_URL not configured`; set it in API worker (Wrangler vars).
  - `openclaw createInstance store_secret` → Often `OPENCLAW_ENCRYPTION_KEY not configured` or decryption error; set in Convex env.

- **API returns 401 on /openclaw/instance:**  
  Check `X-OA-Internal-Key` and `X-OA-User-Id`. Convex actions get user id from `ctx.auth.getUserIdentity().subject` and key from `OA_INTERNAL_KEY`. API must have the same `OA_INTERNAL_KEY` (Wrangler secret).

- **API gets 401 or 500 from Convex control:**  
  Convex checks `x-oa-control-key` against `OA_CONTROL_KEY`. API sends `CONVEX_CONTROL_KEY`. They must be identical (Convex env and API Wrangler secret).

- **Convex control returns 500 with “Control key not configured”:**  
  Set `OA_CONTROL_KEY` in Convex (dashboard or `npx convex env set OA_CONTROL_KEY <value>`).

- **Convex control returns 500 with “OPENCLAW_ENCRYPTION_KEY not configured”:**  
  Set `OPENCLAW_ENCRYPTION_KEY` in Convex (32-byte hex or base64).

---

## 7. Data shape (summary)

- **Instance (API / Convex):** `user_id`, `status` (provisioning | ready | error | deleted), `runtime_url`, `runtime_name`, `created_at`, `updated_at`, `last_ready_at`, optional cf_* and r2_bucket_name. Secrets (e.g. service_token) stored encrypted in `openclaw_instances` using `OPENCLAW_ENCRYPTION_KEY`.
- **API response:** `{ ok: boolean, data?: InstanceSummary | null, error?: string }`. InstanceSummary is the subset the UI needs (status, runtime_name, created_at, updated_at, last_ready_at).
- **Convex control response:** Same `{ ok, data?, error? }` for all openclaw/billing endpoints; Convex HTTP handlers return this and the API forwards it.

---

## 8. What needs to be done next

Ordered by dependency; see `openclaw-on-openagents-com.md` for full roadmap.

1. **Verify end-to-end** — **Done.** Convex and API worker deployed; `CONVEX_SITE_URL` in API set to the same Convex deployment as the web app; TS errors fixed (Hatchery `/kb/$slug` params, openclaw.instance null check). Provision flow works; “ready” = metadata stored, no per-user container.

2. **Milestone 2.5 (onboarding + access)**  
   - Confirm Hatchery gates on `access.getStatus.allowed`: show waitlist overlay when not allowed, “Create your OpenClaw” / Provision when allowed.  
   - Confirm `/admin` can toggle `access_enabled` and approve/revoke waitlist; admin list shows users and waitlist state.

3. **Milestone 3 (sidebar + OpenClaw section)** — **Partially done.**  
   - **Done:** “OpenClaw Cloud” section in the left sidebar (`threadlist-sidebar.tsx`): shows status via `openclaw.getInstanceForCurrentUser`, link to Hatchery, link to Chat when instance is ready. Convex-backed `threads` table and `threads.list` / `threads.create` / `threads.updateTitle` / `threads.archive`; “Chats” section in sidebar with “New chat” (creates thread, navigates to /assistant) and list of Convex threads (links to /assistant?threadId=…). Public query `openclaw.getInstanceForCurrentUser` for sidebar.  
   - **Remaining:** Flow canvas / Hatchery graph “Your workspace graph” from Convex thread index; optional threadId handling on /assistant to load a Convex thread.

4. **Milestone 1 + 2 (durable chat)**  
   - Stand up `openagents-agent-worker` (DO + internal chat endpoint).  
   - Wire `apps/web` chat route to agent worker when `AGENT_WORKER_URL` is set; keep in-process fallback.

5. **Milestone 4 (Mode A — OpenClaw tools)** — **Done.**  
   - Runtime worker proxies Gateway HTTP (tools/invoke, sessions list/history/send).  
   - API exposes stable endpoints for tools + sessions.  
   - Agent worker includes OpenClaw tools (instance/status/devices/approve/backup/restart/billing + sessions).

6. **Milestone 5 (Mode B — true WebChat)** — **Done.**  
   - Runtime: OpenResponses enabled + `POST /v1/responses` proxy.  
   - API: `POST /api/openclaw/chat` streaming endpoint.  
   - Web UI: `/openclaw/chat` route wired to `/api/openclaw/chat`.

7. **Milestone 6 (human approvals)**  
   - Approval flow for provision, device approve, restart (and later DM pairing / exec approvals).

8. **Multi-tenant runtime (from openclaw-on-openagents-com)**  
   - `openclaw-runtime`: accept tenant key (e.g. WorkOS user id), use it as sandbox id; per-user R2 prefix; no single shared sandbox for all users.

---

## 9. References

- Product/roadmap: `openclaw-on-openagents-com.md`
- API deployment (Convex control bridge, secrets): `apps/api/docs/deployment.md`
- Convex schema: `apps/web/convex/schema.ts`
- OpenClaw runtime worker: `apps/openclaw-runtime/`
