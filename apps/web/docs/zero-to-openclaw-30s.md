# Zero to OpenClaw in 30 Seconds — Current State (as of 2026-02-04)

This doc captures the **exact current status** of the “Zero to OpenClaw in 30 seconds” flow:

- What happens today when a user visits `/hatchery`
- What they see, and which clicks actually provision OpenClaw
- What is **implemented now**
- What is **designed but not implemented**
- The **gaps** that block a truly frictionless 30‑second flow

Sources referenced: actual code paths in `apps/web`, `apps/api`, `apps/openclaw-runtime` (not future docs).

---

## TL;DR (Current Reality)

**Yes, a user can provision OpenClaw and chat today.**  
But:

- Provisioning is **record-only** (no per-user container is created; runtime URL is taken from env).
- Access gating still applies (WorkOS auth + Convex `access.getStatus`).
- Chat **auto‑provisions** without an approval dialog (Hatchery does require approval).
- Delete removes the Convex record + secrets, but **does not tear down runtime**.
- Session list / transcripts / channel onboarding are not shipped in the UI.

---

## Exact Current Flow: `/hatchery`

### 1) User visits `/hatchery`
Route: `apps/web/src/routes/_app/hatchery.tsx` → `HatcheryFlowDemo`.

What the user sees:
- A flow‑graph “workspace” canvas + right panel UI.
- If not logged in or not approved, an **overlay gate** is shown (waitlist + sign‑in prompts).
- The **“Create your OpenClaw”** card is always present, but actionability depends on auth/access.

Auth/access gating:
- `access.getStatus` (Convex) checks WorkOS identity + waitlist/user access.
- If `access.allowed !== true`, the OpenClaw action flow is gated visually and by action errors.

### 2) “Create your OpenClaw” card (panel)
UI copy:  
“Provision a managed OpenClaw instance on openagents.com. One gateway per user; tools, sessions, and pairing in one place.”

Current UI states (from `HatcheryFlowDemo`):
- **No instance** → shows “No instance yet” + “Provision OpenClaw” button.
- **Provisioning** → shows “Provisioning…” while action runs.
- **Ready** → shows status badges + “Provisioning complete” callout + link to `/openclaw/chat`.

### 3) Clicking “Provision OpenClaw”
Click action:
- Opens a **confirmation dialog** (approval gate).
- On confirm → calls Convex action `openclawApi.createInstance`.

What **actually happens** on the backend:
1. **Convex action** `openclawApi.createInstance`:
   - Requires WorkOS user identity.
   - Requires `access.getStatus.allowed === true`.
   - Calls the API worker: `POST /api/openclaw/instance`
   - Uses `X-OA-Internal-Key` + `X-OA-User-Id`.

2. **API worker** `POST /openclaw/instance`:
   - Validates internal auth (or API token).
   - Calls Convex control routes to set instance status.
   - **Provisioning is env-driven**:
     - `cf::provision_instance` only reads env variables (e.g. `OPENCLAW_RUNTIME_URL`).
     - **No per-user container is created** here.
   - Generates/stores a **service token** in Convex secrets.
   - Marks the instance as `ready`.

3. **UI updates**:
   - Instance status becomes `ready`.
   - Runtime name may appear (if configured in env).
   - “Provisioning complete” message appears.

### 4) OpenClaw Controls (visible when ready)
Shown only when instance status is `ready`.

Controls available:
- **Refresh**: pulls runtime status + devices + pairing list.
- **Backup now**: calls runtime backup.
- **Restart gateway**: approval‑gated.
- **Device approvals**: list + approve pending devices.
- **DM pairing approvals**: per‑channel list + approve.

These call Convex actions → API worker → runtime worker:
- `/openclaw/runtime/status`
- `/openclaw/runtime/devices`
- `/openclaw/runtime/pairing/:channel`
- `/openclaw/runtime/backup`
- `/openclaw/runtime/restart`

### 5) Delete OpenClaw (Hatchery)
Button: “Delete OpenClaw” (approval‑gated).

What happens:
- Convex action `openclawApi.deleteInstance`.
- API worker `DELETE /openclaw/instance`.
- Convex internal mutation deletes the `openclaw_instances` record and secrets.

**Important:** This does **not** shut down any runtime container or revoke any existing runtime worker.

---

## Exact Current Flow: `/openclaw/chat`

Route: `apps/web/src/routes/_app/openclaw.chat.tsx`

What the user sees:
- “OpenClaw Chat” header with instance status.
- **Create OpenClaw** button if no instance.
- **Delete OpenClaw** button if instance exists.
- Chat form with optional session key.

### Sending a message
When user clicks “Send”:
1. If instance is missing/not ready → **auto‑provisions** via `createOpenclawInstance`.
   - **No approval dialog** here (unlike Hatchery).
2. Sends request to `/openclaw/chat` (server route).
3. Server route verifies WorkOS auth, resolves internal key + API base.
4. Calls API worker `/api/openclaw/chat` (SSE stream).
5. UI consumes SSE via `consumeOpenClawStream`.

Session behavior:
- Optional `sessionKey` is forwarded as `x-openclaw-session-key`.
- No UI for listing sessions yet.

### “Delete OpenClaw” in Chat
Same delete flow as Hatchery (Convex + API delete); UI clears messages and session key.

---

## Implemented Today (Confirmed in Code)

### Core
- `/hatchery` route with OpenClaw provisioning UI + controls.
- `/openclaw/chat` streaming UI.
- OpenClaw instance **create** (Convex → API worker → runtime URL + service token).
- OpenClaw instance **delete** (Convex record + secret removal).
- Runtime status, devices, pairing list/approve, backup, restart.

### Infrastructure
- API worker routes: `/openclaw/instance`, `/openclaw/chat`, `/openclaw/runtime/*`, `/openclaw/tools`, `/openclaw/sessions`.
- Runtime worker routes: `/v1/status`, `/v1/devices`, `/v1/pairing/*`, `/v1/storage/backup`, `/v1/gateway/restart`, `/v1/responses`.
- Gateway responses endpoint enabled in `start-openclaw.sh`.

---

## Designed but Not Implemented (Documented Intent)

From `openclaw-on-openagents-com.md` + Hatchery notes:
- **Sessions list UI** (OpenClaw-native sessions, transcript view, and navigation).
- **Channel onboarding flows** (Slack/Telegram/WhatsApp).
- **Canvas/A2UI viewer** embedded in site.
- **BYO OpenClaw** linking flow (self-hosted gateway + scopes).
- **Mode A vs Mode B UX unification** (single UI for web‑agent vs OpenClaw‑native).
- **OpenClaw tools surface in UI** (beyond runtime controls).

---

## Missing Gaps for “Zero to OpenClaw in 30 Seconds”

### 1) Access/Approval Friction
- Hatchery requires explicit approval for provisioning.
- Chat auto‑provisions without approval.
- No single “instant start” CTA or one‑click flow.

### 2) Provisioning Is Record‑Only
- `createInstance` does **not** create a per‑user container.
- Runtime URL is fixed from env; service token stored in Convex.
- If a real per‑user runtime is required, this is missing.

### 3) Default AI Key Dependency
- Chat only responds if **server‑owned provider keys** are set in runtime env:
  - `OPENAI_API_KEY`, `OPENROUTER_API_KEY`, or `ANTHROPIC_API_KEY`.
- If not set, OpenClaw chat will not respond (error from gateway).

### 4) Deletion Is Incomplete
- Deleting an instance removes **Convex record + secrets only**.
- No runtime teardown or deprovision step.

### 5) Sessions UX Is Missing
- No session list, transcript browsing, or persistent session navigation.

---

## What “Zero to OpenClaw in 30 Seconds” Would Require

1. **Instant entry point** (CTA + direct `/openclaw/chat` entry).
2. **Auto‑provision** from first visit (no approval gate).
3. **Guaranteed runtime key** (server‑owned LLM key set).
4. **Per‑user runtime provisioning** (if required) or clear multi‑tenant story.
5. **Clear success confirmation** + “Start chatting now” moment.
6. **Deletion that actually deprovisions** (optional, but consistent).

---

## Appendix: Key Paths

- UI:
  - `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx`
  - `apps/web/src/routes/_app/openclaw.chat.tsx`

- API worker:
  - `apps/api/src/openclaw/http.rs`
  - `apps/api/src/openclaw/convex.rs`

- Runtime worker:
  - `apps/openclaw-runtime/src/routes/v1.ts`
  - `apps/openclaw-runtime/start-openclaw.sh`

