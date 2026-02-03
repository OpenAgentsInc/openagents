# Definitive plan: OpenClaw on openagents.com (Cloudflare-first)

## Goal

Make **OpenClaw** usable “like the real thing” from **openagents.com**: users can chat, see sessions, connect/pair devices, and (eventually) connect channels — while we run a bunch of Cloudflare-native infrastructure alongside it (Durable Objects via Agents SDK, Containers/Sandbox, R2 persistence, backups, progress streaming, human approvals).

This is the OpenClaw-specific complement to:
- `apps/web/docs/cloudflare-agents-sdk-openagents-com.md`

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

## Product surface: what users should experience on openagents.com

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

## Roadmap tied to OpenClaw (what we implement first)

### Milestone 0: align naming + UX in the site

- Left sidebar: add “OpenClaw” section (instance + sessions)
- Center: OpenClaw chat view (even if initial is Mode A)
- Right sidebar: keep community/collaboration

### Milestone 1: Mode A end-to-end (OpenClaw tools behind a durable agent)

- Build `openagents-agent-worker` and wire `apps/web /chat` to it (per the general plan).
- Add runtime tool proxy endpoints (`/v1/tools/invoke`, sessions list/history/send).
- In the agent, expose “OpenClaw tools” as function tools:
  - status, provision, devices, approve, restart, backup
  - sessions list/history/send
  - (optional) browser/canvas/nodes tools once we proxy them safely

Result: users can “use OpenClaw” from the website and the agent can operate OpenClaw capabilities, with durable background work + progress streaming + approvals.

### Milestone 2: True OpenClaw WebChat (Mode B)

- Enable gateway OpenResponses `/v1/responses`.
- Proxy streaming through runtime → API → web.
- UI switches OpenClaw chat to use this backend (OpenClaw is the conversational source of truth).

### Milestone 3: Pairing + channels

- DM pairing endpoints + UI.
- Start with one or two channels that have clear hosted onboarding (Telegram/Discord) before tackling WhatsApp.

### Milestone 4: Cloudflare extras that make it “feel unfair”

- AI Gateway integration for provider routing, cost tracking, fallbacks
- Browser Rendering integration for web automation at scale (reduce load on Sandbox)
- Workflows/Queues for long tasks + resumability + audit logs

## Notes on security and trust

- Never expose OpenClaw gateway tokens to the browser.
- Keep all `/internal/*` or runtime control endpoints server-to-server only.
- Treat any inbound channel message as untrusted input; keep pairing/allowlists as first-class UI.
- Use strict per-tenant sandbox ids + R2 prefixes to prevent cross-user data access.

