# Chat-centric Managed OpenClaw onboarding (apps/web) — migration + coding instructions

Date: 2026-02-02

You rebuilt the main OpenAgents web app as `apps/web/` (Convex + TanStack Start + WorkOS) and want Managed OpenClaw onboarding to be **chat-first**.

This doc updates the implementation plan to reflect that reality and explains what to adapt from the prior implementation in `apps/website-old2/`.

---

## 0) Reality check: route conflicts (IMPORTANT)

Your legacy Rust API Worker is served at `https://openagents.com/api/*` (path prefix `/api`).

In the new `apps/web/` app, the chat endpoint is currently defined at:
- `apps/web/src/routes/api/chat.ts` → route `/api/chat`

If production routing stays the same (Rust worker owns `/api/*`), then **`/api/chat` will be handled by the Rust worker, not the web app**.

**Action:** move the web chat endpoint away from the `/api/*` prefix.

Recommendation:
- Rename route file to something like:
  - `apps/web/src/routes/_chat.ts`  → `/chat`
  - or `apps/web/src/routes/_internal/chat.ts` → `/_internal/chat`
- Update `apps/web/src/components/assistant-ui/Assistant.tsx` transport:
  - from `api: '/api/chat'`
  - to `api: '/chat'` (or your chosen path)

---

## 1) New architecture: everything is one “superpowered chat”

### User experience goal
- The center chat is the orchestrator.
- Buttons like **“Help me set up OpenClaw”** should kick off a chat flow.
- Sidebars can show state (instance status, credits, pairing) but the *primary* interaction is the chat.

### Backend flow (unchanged, but chat-driven)
- `apps/web` (WorkOS-authenticated UI + chat) calls into OpenAgents infra.
- OpenClaw provisioning/control stays behind:
  - Rust API: `apps/api` at `/api/openclaw/*`
  - Runtime: per-user runtime (`apps/openclaw-runtime/` template) behind service token
  - Convex: source of truth for instance + billing ledger

---

## 2) What we already built previously (apps/website-old2) and what to reuse

### A) Reuse (copy/adapt) — OpenClaw API wrapper logic
From:
- `apps/website-old2/src/lib/openclawApi.ts`

What it does well:
- Provides a server-side wrapper around `GET/POST /api/openclaw/*`.
- Uses internal headers for beta (`X-OA-Internal-Key`, `X-OA-User-Id`).

How to adapt into `apps/web`:
- Create: `apps/web/src/lib/openclawApi.ts`
- Port these functions (as plain server utilities, not necessarily TanStack `createServerFn`):
  - `getOpenclawInstance`
  - `createOpenclawInstance`
  - `getRuntimeStatus`
  - `getRuntimeDevices`
  - `approveRuntimeDevice`
  - `backupRuntime`
  - `restartRuntime`
  - `getBillingSummary`

**Note:** `apps/web` currently uses TanStack Start server handlers (see `apps/web/src/routes/api/chat.ts`).
You can keep OpenClaw calls entirely server-side inside that handler.

### B) Reuse (copy/adapt) — Convex schema + OpenClaw state machine
From:
- `apps/website-old2/convex/schema.ts` (tables)
- `apps/website-old2/convex/openclaw.ts`
- `apps/website-old2/convex/billing.ts`

How to adapt into `apps/web/convex/`:
- Update `apps/web/convex/schema.ts` to include:
  - `openclaw_instances`
  - `credit_ledger`
- Copy `openclaw.ts` + `billing.ts` modules over (then update imports/paths as needed).

### C) Do NOT reuse as-is — the old “OpenClaw dashboard pages”
Old pages/components are now secondary because the new product is chat-first.

These are still useful as reference UI or sidebar widgets, but not as the primary flow:
- `apps/website-old2/src/components/openclaw/*`
- `apps/website-old2/src/components/openclaw/screens/*`
- `apps/website-old2/src/routes/_app/openclaw*.tsx`

Instead:
- Reuse the smaller pieces as sidebar widgets or status cards where useful.

---

## 3) Implement OpenClaw setup as a chat tool (apps/web)

### Where chat currently lives
- UI shell: `apps/web/src/components/assistant-ui/Assistant.tsx`
- Transport endpoint: currently `api: '/api/chat'` (must be moved per section 0)
- Server handler: `apps/web/src/routes/api/chat.ts`

### Required change: tool-capable chat handler
Right now `apps/web/src/routes/api/chat.ts` just streams text from `gpt-4o-mini` and has no tools.

We need to add a **tool layer** so the model can call actions like:
- `openclaw.getStatus`
- `openclaw.provision`
- `openclaw.listDevices`
- `openclaw.approveDevice`
- `openclaw.backupNow`
- `openclaw.restart`
- `openclaw.exportIdentityZip`
- `openclaw.importIdentityZip`

Implementation sketch (AI SDK tools):
- Use `streamText({ tools: { ... } })` and implement each tool by calling:
  - Rust `/api/openclaw/*` endpoints (server-to-server)
  - Runtime `/v1/files/*` for identity import/export once the slim runtime ships

### Chat-driven onboarding script (behavior)
When user clicks “Help me set up OpenClaw”:
1) Chat checks: do you already have an instance?
2) If not, ask for confirmation (“create now?”) then call provision.
3) Poll status and narrate progress.
4) When ready: guide pairing and show pending devices.
5) Offer “import identity bundle” (zip) and/or “export current bundle”.

---

## 4) Where to put “Help me set up OpenClaw” button wiring

Likely locations in `apps/web`:
- `apps/web/src/components/assistant-ui/thread.tsx` (where the chat input lives)
- or a toolbar component used by the chat view

Implementation approach:
- Button inserts a predefined user message into the current thread, e.g.
  - “Help me set up OpenClaw in this account. If I don’t have an instance, provision one. Then help me pair my device.”

---

## 5) Data and auth in the new app

### WorkOS user identity
`apps/web/src/routes/__root.tsx` fetches WorkOS auth and sets `token`.

We need a consistent mapping from WorkOS user → OpenAgents user_id used in Convex/OpenClaw instance records.

MVP recommendation:
- Use `user.id` from WorkOS as the canonical `user_id` in Convex tables (or store a mapping table).

### Internal header beta auth
Until Rust validates WorkOS JWT directly:
- `apps/web` server handlers call Rust `/api/openclaw/*` using:
  - `X-OA-Internal-Key` (server secret)
  - `X-OA-User-Id` (WorkOS user.id)

---

## 6) Concrete to-do list for the coding agent

1) Fix chat endpoint routing conflict:
   - move `apps/web/src/routes/api/chat.ts` off `/api/*`
   - update `Assistant.tsx` transport path

2) Port Convex OpenClaw schema + functions:
   - from `apps/website-old2/convex/*` → `apps/web/convex/*`

3) Port OpenClaw API wrapper:
   - from `apps/website-old2/src/lib/openclawApi.ts` → `apps/web/src/lib/openclawApi.ts`

4) Add tool-capable chat handler:
   - implement OpenClaw tools calling Rust `/api/openclaw/*`

5) Wire “Help me set up OpenClaw” button:
   - inject onboarding prompt into thread

6) Optional: reuse old dashboard components as sidebar widgets:
   - `InstanceStatusCard`, `DeviceList`, `CreditsWidget`

---

## 7) Reference filepaths (quick)

**New app (active):**
- `apps/web/src/components/assistant-ui/Assistant.tsx`
- `apps/web/src/routes/api/chat.ts` (move this route)
- `apps/web/convex/schema.ts` (needs OpenClaw tables)

**Old implementation (reference/adapt):**
- `apps/website-old2/src/lib/openclawApi.ts`
- `apps/website-old2/convex/schema.ts`
- `apps/website-old2/convex/openclaw.ts`
- `apps/website-old2/convex/billing.ts`
- `apps/website-old2/src/components/openclaw/*`

**Backend contracts/specs:**
- `/home/christopherdavid/.openclaw/workspace/instance-runtime-api-contract.md`
- `apps/openclaw-runtime/` (slim runtime app)
- `apps/api/` (Rust API at `/api/openclaw/*`)
