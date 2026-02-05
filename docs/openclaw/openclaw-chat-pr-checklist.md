# PR checklist — Chat-centric OpenClaw onboarding (apps/web)

Use this as the short, concrete checklist for the implementation PR(s).

## PR 1 — Fix routing + make chat tool-capable (no UI changes required)

### A) Fix `/api/*` conflict
- [ ] Change chat endpoint off `/api/*` (Rust worker owns `/api/*` in prod)
  - File: `apps/web/src/routes/api/chat.ts`
  - Route should be `/chat` (or `/_internal/chat`) — we standardize on `/chat`.
- [ ] Update assistant transport
  - File: `apps/web/src/components/assistant-ui/Assistant.tsx`
  - Change `api: '/api/chat'` → `api: '/chat'`

### B) Add OpenClaw tools to chat handler
- [ ] File: `apps/web/src/routes/api/chat.ts`
  - Add tool definitions:
    - `openclaw.getInstance`
    - `openclaw.provision`
    - `openclaw.getStatus`
    - `openclaw.listDevices`
    - `openclaw.approveDevice`
    - `openclaw.backupNow`
    - `openclaw.restart`
    - `openclaw.getBillingSummary`
  - Tools must call Rust worker endpoints under `/api/openclaw/*` server-to-server.

### C) Auth wiring for Rust `/api/openclaw/*` (beta)
- [ ] Add secrets to `apps/web` deployment:
  - `OA_INTERNAL_KEY` (server secret)
- [ ] In tools, set headers:
  - `X-OA-Internal-Key: <OA_INTERNAL_KEY>`
  - `X-OA-User-Id: <workos user.id>`

### D) Quick smoke tests
- [ ] `npm -C apps/web run lint`
- [ ] `npm -C apps/web run build`
- [ ] Manual: open app, click “Help me set up OpenClaw” suggestion, confirm tool calls happen.

## PR 2 — Port OpenClaw state + billing schema into apps/web/convex

- [ ] Port schema tables from `apps/website-old2/convex/schema.ts`:
  - `openclaw_instances`
  - `credit_ledger`
  → into `apps/web/convex/schema.ts`
- [ ] Port functions:
  - `apps/website-old2/convex/openclaw.ts` → `apps/web/convex/openclaw.ts`
  - `apps/website-old2/convex/billing.ts` → `apps/web/convex/billing.ts`
- [ ] Add minimal control endpoints or queries needed by chat/tools.

## PR 3 — Chat UX polish (optional)
- [ ] Update welcome suggestion prompt to a stronger scripted prompt.
  - File: `apps/web/src/components/assistant-ui/thread.tsx` (SUGGESTIONS)
- [ ] Add “OpenClaw status” to a sidebar widget (optional).

## PR 4 — Identity bundle import/export (depends on slim runtime)
- [ ] Once `apps/openclaw-runtime` implements `/v1/files/*`, add tools:
  - `openclaw.exportIdentityZip`
  - `openclaw.importIdentityZip`
