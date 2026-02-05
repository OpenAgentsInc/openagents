**Zero to OpenClaw**
As of February 4, 2026, this document describes the exact, current state of the “Zero to OpenClaw in 30 seconds” flow in `apps/web`, including what users see, what happens in code, what is implemented, what is only planned, and the gaps to close next.

**Goal**
A first‑time user should be able to: land on `/hatchery`, create an OpenClaw instance, send a message, receive a response without configuring any API keys, and delete the instance.

**Current Entry Points**
1. `/hatchery` is the primary “create + control” surface.
2. `/openclaw/chat` is the OpenClaw WebChat surface backed by the OpenClaw Gateway.
3. `/assistant` is the “OpenAgents assistant” surface that can call OpenClaw tools (tool calling) and show tool lists like the screenshot.

**/hatchery: What The User Sees And What Actually Happens**
1. The page loads a flow canvas (“Workspace graph”), sidebar links, and a right‑side inspector.
2. A waitlist overlay is shown when access is not allowed. The form uses `api.waitlist.joinWaitlist` and blocks provisioning until access is granted.
3. If access is allowed, the page calls `api.openclawApi.getInstance` (Convex action), which calls the Rust API worker at `/api/openclaw/instance` using internal headers.
4. If no instance exists, the user sees “Provision OpenClaw” and a status chip saying “No instance yet.”
5. Clicking “Provision OpenClaw” opens an approval dialog. On confirm it calls `api.openclawApi.createInstance`.
6. What “Provision” does today: it writes a row in Convex `openclaw_instances` and sets `status: ready`. The runtime URL is pulled from `OPENCLAW_RUNTIME_URL` or `OPENCLAW_RUNTIME_URL_TEMPLATE`. A service token is stored in Convex (generated if not provided via `OPENCLAW_SERVICE_TOKEN`). No Cloudflare resources are created. This is a record‑only provision step.
7. When `status === ready`, a “Provisioning complete” card appears with a link to `/openclaw/chat`.
8. The “OpenClaw controls” card appears and can show runtime status, last backup, instance type, refresh status/devices, trigger backup, trigger restart (approval required), list pending and paired devices, approve device requests (approval required), list DM pairing requests, and approve DM pairing requests (approval required).
9. “Delete OpenClaw” opens an approval dialog and calls `api.openclawApi.deleteInstance`.
10. What “Delete” does today: deletes the Convex record only. It does not shut down any runtime or Cloudflare resources.

**/openclaw/chat: What The User Sees And What Actually Happens**
1. The header shows instance status and runtime name when available.
2. The page includes an optional “session key” input and a message textarea.
3. On first send, it calls `api.openclawApi.createInstance` if the instance is missing. This auto‑provisions without a manual approval dialog.
4. The browser posts to `/openclaw/chat` (TanStack server handler in `apps/web/src/routes/_app/openclaw.chat.tsx`).
5. The server handler confirms WorkOS auth, resolves `OA_INTERNAL_KEY` and `OPENCLAW_API_BASE` or `PUBLIC_API_URL`, then sends `POST ${apiBase}/openclaw/chat` with `X-OA-Internal-Key`, `X-OA-User-Id`, and optional `x-openclaw-session-key` and `x-openclaw-agent-id`.
6. The API worker proxies to the OpenClaw runtime `/v1/responses` endpoint, which streams SSE from the OpenClaw Gateway.
7. The browser parses the SSE stream with `consumeOpenClawStream` and appends `delta` text to the assistant bubble.
8. The “Stop” button aborts the streaming fetch.

**/assistant: Tool‑Calling Chat**
1. `/chat` redirects to `/assistant` for UI.
2. If `AGENT_WORKER_URL` is set, `/chat` proxies to the agent worker.
3. If not set (or agent worker returns 401), it falls back to a local OpenAI Responses model (`gpt-4o-mini`).
4. Both the agent worker and local fallback can call OpenClaw tools via the Rust API worker. Sensitive actions require explicit approval.

**Tool Status: Are Those Tools Implemented?**
The tools shown in the screenshot come from the agent worker tool set. All tools listed below are wired to API worker endpoints, but only some have UI coverage.

| Tool | Where It Exists | Backing Endpoint | Requires Approval | UI Coverage |
| --- | --- | --- | --- | --- |
| `openclaw_get_instance` | Agent worker + local fallback | `GET /api/openclaw/instance` | No | Hatchery + assistant |
| `openclaw_provision` | Agent worker + local fallback | `POST /api/openclaw/instance` | Yes | Hatchery + assistant |
| `openclaw_get_status` | Agent worker + local fallback | `GET /api/openclaw/runtime/status` | No | Hatchery + assistant |
| `openclaw_list_devices` | Agent worker + local fallback | `GET /api/openclaw/runtime/devices` | No | Hatchery + assistant |
| `openclaw_approve_device` | Agent worker + local fallback | `POST /api/openclaw/runtime/devices/:id/approve` | Yes | Hatchery + assistant |
| `openclaw_list_pairing_requests` | Agent worker + local fallback | `GET /api/openclaw/runtime/pairing/:channel` | No | Hatchery + assistant |
| `openclaw_approve_pairing` | Agent worker + local fallback | `POST /api/openclaw/runtime/pairing/:channel/approve` | Yes | Hatchery + assistant |
| `openclaw_backup_now` | Agent worker + local fallback | `POST /api/openclaw/runtime/backup` | No | Hatchery + assistant |
| `openclaw_restart` | Agent worker + local fallback | `POST /api/openclaw/runtime/restart` | Yes | Hatchery + assistant |
| `openclaw_get_billing_summary` | Agent worker + local fallback | `GET /api/openclaw/billing/summary` | No | Assistant only |
| `openclaw_list_sessions` | Agent worker only | `GET /api/openclaw/sessions` | No | Assistant only |
| `openclaw_get_session_history` | Agent worker only | `GET /api/openclaw/sessions/:key/history` | No | Assistant only |
| `openclaw_send_session_message` | Agent worker only | `POST /api/openclaw/sessions/:key/send` | No | Assistant only |

**Device Pairing: What It Means And How It Works Today**
Device pairing is for OpenClaw “node” clients that want to connect to your gateway (mobile/desktop/headless).

1. A device connects to the OpenClaw Gateway and creates a pending pairing request.
2. Hatchery calls `openclaw devices list` via the runtime and surfaces the pending request ID.
3. You click “Approve,” which runs `openclaw devices approve <requestId>` in the runtime sandbox.
4. After approval, the device is considered paired and can connect to the gateway.

Current gap: there is no UI flow to generate device pairing requests or guide a user through pairing. The UI only approves requests that already exist.

**DM Pairing: What It Means And How It Works Today**
DM pairing allows a messaging channel (Telegram/Slack/etc.) to deliver messages into your OpenClaw gateway.

1. A channel integration creates a pairing request with a code.
2. Hatchery lets you enter a channel name and load pending requests.
3. You click “Approve,” which runs `openclaw pairing approve <channel> <code>` in the runtime sandbox.

Current gap: there is no OAuth or bot setup flow in the web app to create pairing requests. Only approvals exist.

**Data And Control Plane (End‑to‑End)**
1. Web app (`apps/web`) uses WorkOS AuthKit for identity, calls Convex for access gating and instance status, and uses server routes to proxy chat and instance actions.
2. Convex stores `openclaw_instances` and encrypted secrets, and provides actions that call the Rust API worker with `X-OA-Internal-Key` plus `X-OA-User-Id`.
3. Rust API worker (`apps/api`) authorizes internal requests with `OA_INTERNAL_KEY`, looks up instance and service token in Convex, and proxies to the runtime worker with `X-OpenAgents-Service-Token`.
4. OpenClaw runtime (`apps/openclaw-runtime`) runs the OpenClaw Gateway in a container and proxies `/v1/responses` (SSE), `/v1/tools/invoke`, device/pairing commands, and sessions tools.

**What Is Implemented Today (Factually True In Code)**
1. Hatchery waitlist gating and access checks.
2. Instance get/create/delete via Convex → API worker.
3. Runtime status, devices list, backup, restart.
4. Device approval and DM pairing approvals.
5. OpenClaw WebChat via `/openclaw/chat` streaming SSE.
6. Agent worker tool set for OpenClaw (including sessions APIs).

**What Is Designed But Not Implemented**
1. Real per‑user Cloudflare provisioning. The current provision step is record‑only.
2. Runtime teardown on delete.
3. Sessions UI (list/history/send) in the web app.
4. DM and device pairing onboarding flows (OAuth, QR codes, device tokens).
5. A single CTA “Create and chat now” that unifies Hatchery + OpenClaw Chat flow.
6. Persisted approvals in storage. Approval state is in memory only for assistant tool calls.

**Gaps Blocking “Zero to OpenClaw in 30 Seconds”**
1. Auto‑provision consistency. Hatchery requires manual approval, but `/openclaw/chat` auto‑provisions. The flow feels inconsistent and slow.
2. Provider keys. The runtime must have a server‑owned model key. Without it, chat fails.
3. Record‑only provisioning. There is no real runtime allocation per user.
4. Delete does not teardown. Only the Convex row is removed.
5. No pairing onboarding. You can approve requests, but cannot create them.

**What We Should Do Next**
1. Make the first‑time flow one click: a single CTA that provisions (or reuses) and sends the first chat message.
2. Ensure OpenClaw runtime has a server‑owned model key in prod so chat always answers.
3. Decide multi‑tenant vs per‑user runtime. If per‑user, wire real Cloudflare provisioning and teardown.
4. Add Sessions UI (list, history, send) using the existing API endpoints.
5. Build pairing onboarding for devices and DM channels so users can create pairing requests from the UI.

**Why “unauthorized” (401) from Hatchery / Convex actions**
The 401 is returned by the **API worker** when Convex actions call it. Convex actions run inside the Convex deployment and **do not read `apps/web/.env.local`**. So even if you set `OA_INTERNAL_KEY` locally, the Convex deployment still uses whatever is set in **Convex** (or nothing). If your dev Convex deployment hits the prod API (`https://openagents.com/api`), the key Convex sends must match the API worker’s `OA_INTERNAL_KEY`.

**Fix (same key + URL everywhere):**
1. **Convex (dev deployment)** – set env in Convex, not in .env:
   ```bash
   cd apps/web
   npx convex env set OA_INTERNAL_KEY "<same key as API worker>" --deployment-name dev:effervescent-anteater-82
   npx convex env set PUBLIC_API_URL "https://openagents.com/api" --deployment-name dev:effervescent-anteater-82
   ```
2. **API worker** – ensure it has the same internal key:
   ```bash
   cd apps/api
   npx wrangler secret put OA_INTERNAL_KEY
   ```
3. **Web worker** (if server routes call the API with the key):
   ```bash
   cd apps/web
   npx wrangler secret put OA_INTERNAL_KEY
   npx wrangler secret put PUBLIC_API_URL
   ```
For prod Convex, set the same variables in the prod Convex deployment (Dashboard or `npx convex env set ...` without `--deployment-name` or with the prod deployment name).

**Appendix: Key Files**
- Hatchery UI: `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx`
- OpenClaw Chat: `apps/web/src/routes/_app/openclaw.chat.tsx`
- Assistant tools: `apps/web/src/routes/chat.ts`
- Agent worker tool set: `apps/agent-worker/src/threadAgent.ts`
- Convex actions: `apps/web/convex/openclawApi.ts`
- API worker: `apps/api/src/openclaw/http.rs`
- Runtime worker: `apps/openclaw-runtime/src/routes/v1.ts`
- Runtime startup: `apps/openclaw-runtime/start-openclaw.sh`
