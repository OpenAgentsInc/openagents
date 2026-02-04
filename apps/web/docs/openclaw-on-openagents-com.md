# Definitive plan: OpenClaw on openagents.com (Cloudflare-first)

## Goal

Make **OpenClaw** usable “like the real thing” from **openagents.com**: users can chat, see sessions, connect/pair devices, and (eventually) connect channels — while we run a bunch of Cloudflare-native infrastructure alongside it (Durable Objects via Agents SDK, Containers/Sandbox, R2 persistence, backups, progress streaming, human approvals).

This doc is the **canonical roadmap** for Cloudflare + OpenClaw on openagents.com.
`apps/web/docs/cloudflare-agents-sdk-openagents-com.md` is supporting/background context; if they conflict, this doc wins.

## What OpenClaw actually is (from `/Users/christopherdavid/code/openclaw`)

OpenClaw is **gateway-centric**:

- One long-lived **Gateway** owns messaging surfaces + sessions.
- Control-plane clients connect over **WebSocket** (default `ws://127.0.0.1:18789`) with a typed protocol and auth.
- Nodes (iOS/Android/mac/headless) also connect via WS with `role: node` and are approved via **device pairing**.
- There are two key HTTP surfaces on the Gateway:
  - `POST /tools/invoke` (always enabled; auth + tool policy enforced)
  - `POST /v1/responses` OpenResponses-compatible API (disabled by default; can stream SSE)
- Pairing is first-class:
  - DM pairing for inbound chats on channels
  - device pairing for nodes / operators

Docs we’re aligning to:
- `docs/concepts/architecture.md` (Gateway architecture + protocol + pairing)
- `docs/gateway/protocol.md` (WS connect + roles/scopes/device tokens)
- `docs/gateway/openresponses-http-api.md` (optional streaming HTTP `/v1/responses`)
- `docs/gateway/tools-invoke-http-api.md` (always-on HTTP tool invoke)
- `docs/web/webchat.md` (WebChat behavior and the methods used)
- `docs/start/pairing.md` (DM pairing + device pairing)

## What we have today in OpenAgents (relevant code)

We already have “Managed OpenClaw” primitives (provisioning + status):

1. **Website Worker**: `apps/web`
   - `/chat` is a server route (`apps/web/src/routes/chat.ts`) using AI SDK streaming and tools that call `/api/openclaw/*`.
2. **API Worker**: `apps/api` (Rust at `openagents.com/api/*`)
   - `/api/openclaw/*` endpoints exist for:
     - instance provision/get
     - runtime status/devices
     - device approval
     - backup/restart
     - billing summary
3. **OpenClaw Runtime Worker**: `apps/openclaw-runtime`
   - Cloudflare Containers + Sandbox DO running OpenClaw Gateway inside a container image.
   - Currently exposes `/v1/status`, `/v1/devices`, `/v1/devices/:id/approve`, `/v1/storage/backup`, `/v1/gateway/restart`.

Today’s gap: we do **not** yet provide a first-class “OpenClaw WebChat” that is actually backed by the OpenClaw Gateway session model — we only have a website chat that can *manage* OpenClaw.

## Current implementation status (Hatchery ↔ OpenClaw get/create)

As of the last update, the following is implemented and deployed:

- **Hatchery:** “Create your OpenClaw” panel (when access is allowed). Calls Convex actions `openclawApi.getInstance` and `openclawApi.createInstance`; shows instance status, Provision button, and when status is `ready` a “Provisioning complete” blurb with a link to the main Chat. No TanStack server function for instance (avoids “Only HTML” issues).
- **Convex:** Actions in `openclawApi.ts` that `fetch(PUBLIC_API_URL/openclaw/instance)` with internal key and user id; HTTP routes in `http.ts` for `/control/openclaw/instance`, `/control/openclaw/instance/status`, `/control/openclaw/instance/secret`, `/control/openclaw/billing/summary`; handlers in `openclaw_control_http.ts` that verify `x-oa-control-key` and call internal openclaw/billing functions. Env: `OA_INTERNAL_KEY`, `OA_CONTROL_KEY`, `PUBLIC_API_URL`, `OPENCLAW_ENCRYPTION_KEY`.
- **API worker (Rust):** GET/POST `/openclaw/instance` with `X-OA-Internal-Key` and `X-OA-User-Id`; all Convex calls go to `CONVEX_SITE_URL` with `CONVEX_CONTROL_KEY` (must match the same Convex deployment as the web app). Explicit error handling with prefixed messages (e.g. `openclaw getInstance: ...`). Env: `OA_INTERNAL_KEY`, `CONVEX_SITE_URL`, `CONVEX_CONTROL_KEY`, `OPENCLAW_RUNTIME_URL`.
- **What “ready” means:** Instance row in Convex with `status: ready` and `runtime_url` from API env. No per-user container is started; provision only records metadata. OpenClaw Chat (streaming) and device pairing are planned for later milestones.
- **Docs:** Full architecture, env vars, flows, and debugging: `apps/web/docs/openclaw-hatchery-architecture.md`.

**Milestone 3 (sidebar + OpenClaw) — partially done:** Left sidebar has an “OpenClaw Cloud” section (status from Convex, link to Hatchery, link to Chat when ready) and a “Chats” section backed by Convex `threads` (list, “New chat” creates a thread and navigates to /assistant). Threads table: `user_id`, `title`, `kind` (chat/project/openclaw), `archived`, `created_at`, `updated_at`. Remaining: Hatchery “Your workspace graph” from Convex threads; /assistant loading a thread by `threadId` search param.

## Product surface: what users should experience on openagents.com

### Primary UI: Hatchery (Flow canvas)

The center of the app should be a **Flow-first canvas** (SVG graph) that users can pan/zoom and click into:
- nodes represent *things* (Chats, Projects, OpenClaw Cloud, Sessions, Approvals, Community)
- edges represent *relationships and execution* (agent runs, tool calls, approvals, streaming progress)
- the right sidebar becomes an **inspector** (details + actions for the selected node)

Implementation details and parity notes live in:
- `apps/web/docs/flow-conversion-plan.md`

### Core “OpenClaw Cloud” UX (ship-first)

1. **OpenClaw tab / sidebar entry**
   - Status: running/starting/stopped/error, version, last backup, instance type
   - Billing summary (credits)
   - Buttons: Provision, Restart, Backup

2. **OpenClaw WebChat**
   - Chat with your OpenClaw (streaming responses)
   - “Deliver to channel” (later): allow a reply to be sent to Slack/Telegram/etc.
   - Attachments (later): files/images → OpenClaw OpenResponses input items

3. **Pairing**
   - Device pairing UI (nodes)
   - DM pairing UI (channels) once we expose it

4. **Sessions list (OpenClaw-native)**
   - A “Sessions” view that mirrors OpenClaw’s session model:
     - `main`
     - `channel:account:peer`
     - group sessions, etc.
   - Click a session to view transcript and continue chatting in that session.

### Advanced (after ship-first)

- Channels onboarding flows (WhatsApp QR, Slack OAuth, Telegram bot token, etc.)
- Canvas/A2UI viewer embedded in site
- Node capabilities UI (camera, screen recording, location)
- Skills browser + install (curated)
- “Bring your own OpenClaw” linking flow (self-hosted Gateway + approved scopes)

## Definitive architecture on Cloudflare (how it all connects)

### Workers/components

1. `apps/web` (UI/SSR worker)
   - WorkOS auth
   - Browser-facing routes (no `/api/*` ownership in prod)
   - Renders the full OpenClaw experience

2. `apps/api` (Rust API worker at `openagents.com/api/*`)
   - Canonical API surface for the website + external clients
   - Auth/billing
   - Owns all `/api/openclaw/*` endpoints

3. `apps/openclaw-runtime` (Containers/Sandbox worker)
   - Runs the OpenClaw Gateway per tenant (multi-tenant plan below)
   - Owns container lifecycle, backups, and “inside-gateway” connectivity

4. `openagents-agent-worker` (new, Agents SDK durable orchestrator)
   - Owns durable web-native chat threads/projects/progress streaming
   - Coordinates between:
     - OpenClaw runtime (gateway + tools + channels + nodes)
     - OpenAgents services (payments, community, etc.)

### Key decision: two modes, one UI

We should support *one UI*, but two execution modes:

**Mode A — Website Agent + OpenClaw tools (fastest path, gets us 80% quickly)**
- LLM runs in `openagents-agent-worker` (Agents SDK) for streaming/progress.
- Tool execution is delegated to OpenClaw via a **gateway tool proxy**:
  - the agent worker calls `apps/api` → calls `apps/openclaw-runtime` → invokes OpenClaw Gateway tools (policy enforced).
- This gives users “OpenClaw powers” (nodes/canvas/channels/tools) via the website quickly, even before we fully proxy OpenClaw’s own chat/session surfaces.

**Mode B — True OpenClaw WebChat (the “same shit” mode)**
- Chat/session model is OpenClaw-native:
  - sessions list = OpenClaw sessions
  - transcript = OpenClaw transcript
  - sending = OpenClaw agent run (OpenResponses or WS `agent`), streaming
- The agent worker still exists for background orchestration and “cloud extras,” but OpenClaw is the conversational source of truth for this mode.

We implement Mode A first, then Mode B.

## Concrete API plan (what to build)

### A) Expand the runtime worker to support tool proxying (Mode A)

Add to `apps/openclaw-runtime`:

1. `POST /v1/tools/invoke`
   - Input: `{ tool, action?, args?, sessionKey?, headers? }`
   - Behavior: call the **Gateway** `POST /tools/invoke` with bearer auth (gateway token), return JSON.
   - Reason: lets our agent worker use OpenClaw’s tool policy + implementations without implementing the WS protocol.

2. `GET /v1/sessions`
   - Use gateway tools (via `/tools/invoke`) to run `sessions_list`.

3. `GET /v1/sessions/:sessionKey/history`
   - Run `sessions_history` via `/tools/invoke`.

4. `POST /v1/sessions/:sessionKey/send`
   - Run `sessions_send` via `/tools/invoke` (non-streaming first; streaming later).

Notes:
- OpenClaw tools list includes `sessions_list`, `sessions_history`, `sessions_send`, and many “cool” tools (`browser`, `canvas`, `nodes`, `cron`, etc.).
- `/tools/invoke` is **always enabled** in OpenClaw Gateway, so we don’t need to flip config for this MVP.

### B) Add a True WebChat streaming path (Mode B)

Preferred approach: OpenClaw OpenResponses API.

1. Enable Gateway `POST /v1/responses` inside the runtime container:
   - Set `gateway.http.endpoints.responses.enabled=true` in the OpenClaw config written by `apps/openclaw-runtime/start-openclaw.sh`.
2. Add runtime endpoint:
   - `POST /v1/responses`
   - Proxy to the gateway `POST /v1/responses` and stream SSE through.

This gives us:
- streaming output
- item-based inputs (files/images)
- standard “client tool calling” continuation flows if we choose to expose them

Fallback approach (if SSE proxying from Sandbox → Worker is hard):
- Implement a WS client in the runtime worker that speaks OpenClaw Gateway protocol (`agent` / `chat.send`) and forwards events to the website.

### C) Promote stable APIs in `apps/api` (website and external clients)

Add to `apps/api` (Rust) as the stable “public-ish” surface:

- `POST /api/openclaw/chat` (streaming)
  - Auth: OpenAgents bearer token (preferred) or internal headers (server-to-server)
  - Implementation: call runtime `/v1/responses` and stream back
- `POST /api/openclaw/tools/invoke`
  - Implementation: call runtime `/v1/tools/invoke`
- `GET /api/openclaw/sessions` + `GET /api/openclaw/sessions/:key/history`
  - Implementation: call runtime endpoints

This keeps `apps/web` simple and avoids browser access to privileged runtime endpoints.

## Multi-tenancy (must-have for “OpenClaw Cloud”)

OpenClaw is designed for “one gateway per host.” In Cloudflare, that becomes:

- One **sandbox container** per user (tenant isolation boundary)
- One **service token** per user (already stored via Convex in `apps/api`)
- One **gateway auth token** per user (should be stored as a secret; never sent to the browser)
- One R2 “home” per user (prefix or bucket-per-user; pick one and standardize)

Implementation direction:

- `apps/openclaw-runtime` should accept a tenant key (WorkOS `userId`) and use it as the sandbox id:
  - `getSandbox(env.Sandbox, normalize(userId))`
  - Stop using a single fixed sandbox id (`openclaw-runtime`) for all tenants.
- Backups:
  - store under `r2://openclaw-data/<userId>/...` or a dedicated bucket name per user.
- Rate limits + guardrails:
  - cap concurrent container starts per account
  - cap background tasks and browser rendering usage

## Human approvals (OpenClaw-native + website-native)

We need two approval layers:

1. **OpenClaw approvals** (gateway level)
   - Device pairing approvals (already started: `/v1/devices` + approve)
   - DM pairing approvals (add endpoints mirroring `openclaw pairing list/approve`)
   - Exec approvals (OpenClaw has an `exec.approval.*` event flow in WS; for Mode A, expose it via tool proxying or dedicated endpoints)

2. **Website approvals** (product level)
   - Anything that spends credits, connects accounts, or changes privacy/security posture should require explicit UI confirmation.

## Unified roadmap (Cloudflare + OpenClaw)

This is the “single roadmap” for:
- OpenClaw Cloud (hosted/managed OpenClaw)
- the `openagents-agent-worker` (Agents SDK durable orchestration)
- the website UI (`apps/web`)
- the stable API surface (`apps/api`)
- the OpenClaw runtime substrate (`apps/openclaw-runtime`)

### MVP definition (“important 80%”)

Ship these first:

1. **Durable chat threads** on the website (persisted across reloads/devices).
2. **Onboarding + access gating**:
   - waitlist with manual approvals
   - `/admin` admin panel to flip access on/off per user or waitlist email
   - Hatchery shows a “create OpenClaw” onboarding flow only when access is enabled
3. **OpenClaw Cloud controls** in the UI (provision/status/devices/approve/backup/restart/billing).
4. **One “OpenClaw chat” experience** in the site that is durable + streaming:
   - Mode A first (website agent + OpenClaw tool proxy), then
   - Mode B as the follow-up (true OpenClaw WebChat via OpenResponses).
5. **Human approvals** for the risky actions (provision, device approve, restart; later DM pairing / exec approvals).

Defer (post-MVP):
- full multi-client state sync (Agent SDK client hooks)
- channel onboarding UX beyond 1–2 easiest (Telegram/Discord first)
- heavy long-running orchestration (Workflows/Queues) beyond basic DO alarms

### Milestone 1: Stand up `openagents-agent-worker` (durable orchestration core)

**Goal:** A dedicated Cloudflare Worker using the Cloudflare Agents SDK that can own durable “threads” and stream responses.

Changes:
- Add `apps/agent-worker/` (new Worker app)
  - DO class: `ThreadAgent` (keyed by `threadId`)
  - Internal endpoints (server-to-server only):
    - `POST /internal/chat` (streams AI SDK UI messages)
    - `POST /internal/approval/respond`
- Auth (server-to-server):
  - `X-OA-Internal-Key`
  - `X-OA-User-Id` (WorkOS user id)

Acceptance criteria:
- `POST /internal/chat` streams and persists state across calls.
- `threadId` deterministically maps to a DO instance.

### Milestone 2: Wire the website (`apps/web`) to the agent worker (no UI rewrite)

**Goal:** Keep the current chat UI, but run it through the durable agent when enabled.

Changes:
- Update `apps/web/src/routes/chat.ts`:
  - if `process.env.AGENT_WORKER_URL` is set:
    - forward to `${AGENT_WORKER_URL}/internal/chat`
    - include `X-OA-Internal-Key` and `X-OA-User-Id`
  - else fall back to the current in-process `streamText()`
- Make thread identity explicit:
  - MVP: use the assistant UI transport `id` field as `threadId` (prefix with `userId` for uniqueness).
  - Later: add a thread route like `/_app/t/$threadId` so the URL is the canonical source of the active thread.

Acceptance criteria:
- With `AGENT_WORKER_URL` unset, behavior is unchanged.
- With `AGENT_WORKER_URL` set, chat is durable + streaming and still works with `@assistant-ui/*`.

### Milestone 2.5: Onboarding + access control (waitlist + admin panel)

**Goal:** Move from “waitlist-only” to controlled onboarding with explicit approvals.

Changes:
- Add access flags and waitlist approvals in Convex:
  - `users.access_enabled` (+ audit fields)
  - `waitlist.approved` (+ audit fields)
- Admin panel at `/admin`:
  - lists all users and waitlist entries
  - toggle access for users
  - approve/revoke waitlist entries (manual, one-by-one)
- Hatchery gating:
  - if access is **off**, show the waitlist overlay
  - if access is **on**, show the real Hatchery UI starting at “Create your OpenClaw”
- Server proxy for onboarding:
  - `GET /openclaw/instance` (status)
  - `POST /openclaw/instance` (provision)
  - server-side auth + access check before calling `/api/openclaw/*`

Acceptance criteria:
- Admin can approve/revoke waitlist entries and enable access from `/admin`.
- Users with access enabled can provision an OpenClaw instance from Hatchery.
- Users without access only see the waitlist overlay.

### Milestone 3: Make the left sidebar real (thread index + “OpenClaw” section)

**Goal:** The UI matches the product plan: chats/projects/OpenClaw on the left; community on the right.

Changes:
- Add a Convex-backed thread index:
  - `thread_id`, `user_id`, `title`, `created_at`, `updated_at`, `archived`
  - optional: `kind` (`chat` | `project` | `openclaw`)
- Update `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` to render the Convex list.
- Add a Hatchery graph query/view that uses the same index to render “Your workspace graph” in the Flow canvas.
- Add an “OpenClaw” section in the left sidebar that:
  - shows OpenClaw Cloud status + actions
  - links to OpenClaw sessions/chat views

Acceptance criteria:
- Threads persist across refresh/devices.
- “OpenClaw” is always reachable from the left sidebar.

### Milestone 4: Mode A — OpenClaw tools behind the durable agent (bridge without WS)

**Goal:** Users can “use OpenClaw” from the website even before we proxy the full Gateway WS surface.

Changes (runtime worker):
- Extend `apps/openclaw-runtime` to proxy OpenClaw Gateway HTTP surfaces:
  - `POST /v1/tools/invoke` → gateway `POST /tools/invoke`
  - `GET /v1/sessions` (via `sessions_list`)
  - `GET /v1/sessions/:sessionKey/history` (via `sessions_history`)
  - `POST /v1/sessions/:sessionKey/send` (via `sessions_send`)

Changes (API worker):
- Promote stable endpoints in `apps/api`:
  - `POST /api/openclaw/tools/invoke` → runtime `/v1/tools/invoke`
  - `GET /api/openclaw/sessions` + history → runtime endpoints

Changes (agent worker):
- Add “OpenClaw tools” as function tools:
  - instance/status/devices/approve/backup/restart/billing (already exist in `apps/api`)
  - sessions list/history/send (new)
  - (optional) selected “cool” tools once we proxy safely (browser/canvas/nodes/cron)

Acceptance criteria:
- From the website, user can browse OpenClaw sessions, open one, and send a message into it.
- No OpenClaw gateway tokens ever hit the browser.

### Milestone 5: Mode B — True OpenClaw WebChat (OpenResponses streaming)

**Goal:** The website can be a real OpenClaw WebChat client, backed by the OpenClaw Gateway session model and streaming semantics.

Changes (runtime worker):
- Enable OpenClaw Gateway OpenResponses endpoint in the container config:
  - set `gateway.http.endpoints.responses.enabled=true` in the config written by `apps/openclaw-runtime/start-openclaw.sh`
- Add runtime proxy:
  - `POST /v1/responses` → gateway `POST /v1/responses` (SSE streaming)

Changes (API worker):
- Add a stable streaming endpoint:
  - `POST /api/openclaw/chat` → runtime `/v1/responses` (stream back to callers)

Changes (web UI):
- Add an “OpenClaw Chat” route that uses `/api/openclaw/chat` as its transport.

Acceptance criteria:
- Streaming works end-to-end (site ↔ api ↔ runtime ↔ gateway).
- OpenClaw sessions are the source of truth for transcript/history in this mode.

### Milestone 6: Human approvals (end-to-end)

**Goal:** “Pause for approval” is a first-class website feature across:
- OpenClaw Cloud (device pairing, DM pairing, exec approvals)
- website-driven actions (cost/spend/account connects)

Changes:
- Agent worker emits `approval.requested` events and blocks until resolved.
- Website shows an approval modal and sends `approval.respond` (server-to-server).
- Extend OpenClaw surfaces:
  - device approve (already present)
  - DM pairing list/approve endpoints (add)
  - exec approval queue endpoints (add; may require WS integration if not tool-invokable)

Acceptance criteria:
- Provision/device approve/restart are gated by explicit UI approval.

### Milestone 7: Cloudflare “unfair advantages”

Add the Cloudflare-native pieces that make hosted OpenClaw feel better than self-hosting:

- **AI Gateway** for centralized visibility/cost control and provider failover.
- **Browser Rendering** to offload heavy browser automation from Sandbox containers.
- **Workflows/Queues** for long-running tasks with retries and auditable logs.
- Better **observability**: per-user run logs, tool receipts, latency/cost dashboards.

## Notes on security and trust

- Never expose OpenClaw gateway tokens to the browser.
- Keep all `/internal/*` or runtime control endpoints server-to-server only.
- Treat any inbound channel message as untrusted input; keep pairing/allowlists as first-class UI.
- Use strict per-tenant sandbox ids + R2 prefixes to prevent cross-user data access.
