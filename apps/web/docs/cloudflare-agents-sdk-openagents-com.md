# Background: Cloudflare Agents SDK + agent worker on openagents.com

## Context / why now

We want openagents.com to make it easy for people to interact with **remote agents** via chat, with:

- **Durable execution** (agents can run for minutes/days)
- **Automatic persistence** (state/memory survives restarts)
- **Real-time progress streaming**
- **Human approval** gates for sensitive actions

Cloudflare’s framing (paraphrase): “durable background agents that run for minutes or days, persist state automatically, stream progress in real time, and pause for human approval when needed.” This doc captures background/context and implementation notes.

This aligns with how we’re building OpenClaw (formerly “Moltbot”; editorial note from Jan 30, 2026) and with the “Moltworker → OpenClaw runtime on Cloudflare” direction.

Related internal doc (background/product framing):
- `docs/local/openclaw-openagents-website-integration.md` proposes (a) an optional “link your own OpenClaw” bridge flow and (b) a hosted/multi-tenant OpenClaw track. The Agents SDK approach below maps cleanly to the hosted track, and can also serve as the “bridge” surface (with strict scoping + explicit user consent).

OpenClaw-specific “definitive plan” doc:
- `apps/web/docs/openclaw-on-openagents-com.md`

Separately, the planned UI split is already reflected in `apps/web`:
- Left sidebar: chats / projects / OpenClaw
- Right sidebar: community posts & collaboration

## Canonical roadmap (Cloudflare + OpenClaw)

The single, unified roadmap for implementing durable website chat + OpenClaw Cloud on openagents.com is:
- `apps/web/docs/openclaw-on-openagents-com.md` (see “Unified roadmap (Cloudflare + OpenClaw)”)

This doc is supporting/background context: what we have today, recommended architecture for an Agents SDK “agent worker”, and Cloudflare-specific implementation notes.

## What we have today (actual code + Cloudflare usage)

### Worker topology (today)

- `apps/web` is a **Cloudflare Worker** (TanStack Start SSR) deployed via Wrangler.
  - Config: `apps/web/wrangler.jsonc` (name `openagents-web-app`, `nodejs_compat`, observability enabled)
  - Build/deploy: `apps/web/package.json` (`npm run deploy` → `vite build` + `wrangler deploy`)
- `apps/api` is a **separate Cloudflare Worker** routed to `openagents.com/api/*`.
  - Config: `apps/api/wrangler.toml`
  - It owns `/api/*` on the apex domain in production, so `apps/web` should not mount routes under `/api/*`.
- `apps/openclaw-runtime` is a **Cloudflare Containers + Durable Object** app (`Sandbox` DO) with R2 and cron.
  - Config: `apps/openclaw-runtime/wrangler.jsonc`

### Chat in `apps/web` (today)

- Server route: `apps/web/src/routes/chat.ts`
  - `POST /chat` uses the AI SDK (`streamText`) with `openai.responses('gpt-4o-mini')`.
  - It exposes server-side tools that call the Rust API worker under `/api/openclaw/*`.
  - In “beta internal auth”, it calls `/api/openclaw/*` using:
    - `X-OA-Internal-Key` (from `OA_INTERNAL_KEY` configured as a Wrangler secret on `apps/web`)
    - `X-OA-User-Id` (WorkOS user id, from `getAuth()`)
  - Helper library: `apps/web/src/lib/openclawApi.ts`
- UI: `apps/web/src/components/assistant-ui/*`
  - The main chat thread UI is `apps/web/src/components/assistant-ui/thread.tsx`.
  - Layout matches the “left chat / right community” plan: `apps/web/src/components/assistant-ui/AppLayout.tsx` + `apps/web/src/components/assistant-ui/right-sidebar.tsx`.

### OpenClaw instance flow (Hatchery, current)

- **Hatchery** (when the user has access) shows a “Create your OpenClaw” panel. It calls **Convex actions** `openclawApi.getInstance` and `openclawApi.createInstance` (not the API directly from the client), which in turn `fetch(openagents.com/api/openclaw/instance)` with `X-OA-Internal-Key` and `X-OA-User-Id`.
- The **API worker** handles GET/POST `/openclaw/instance` and calls **Convex HTTP** at `CONVEX_SITE_URL` (e.g. `https://<deployment>.convex.site`) with paths like `control/openclaw/instance` and header `x-oa-control-key: CONVEX_CONTROL_KEY`. **`CONVEX_SITE_URL` must point at the same Convex deployment that serves the web app**, or getInstance/createInstance return 500.
- Convex stores the instance in `openclaw_instances`; “provision” today only writes metadata and runtime URL (no per-user container). Full architecture, env vars, and debugging: `apps/web/docs/openclaw-hatchery-architecture.md`. Product status: `apps/web/docs/openclaw-on-openagents-com.md` (“Current implementation status”).

### Current limitations

- Server chat is still effectively **stateless per request** (the client sends message history; the server doesn’t own durable thread state).
- “Long-running” actions can be initiated via tools, but we don’t yet have a first-class:
  - background job model
  - progress event stream that persists/replays
  - human approval workflow for sensitive tool calls
- Our domain routing constraints (Rust API owns `/api/*`) mean we must be careful where we mount any new endpoints.

## Goal: make remote agents feel native in the website

Desired end state:

- Each user (and/or each thread/project) corresponds to a **durable remote agent**.
- The website can:
  - create/provision the agent (e.g., “Provision my OpenClaw”)
  - chat with it (streaming)
  - watch it work (progress updates)
  - approve/reject actions (human-in-the-loop)
  - come back later and resume (state + logs persisted)

## Proposal: Add a Cloudflare Agents SDK “agent worker”

Cloudflare Agents SDK (the `agents` package) is designed around **Durable Objects** and provides:

- persistent, stateful “Agent” instances
- WebSocket-based real-time communication + state sync
- scheduling/alarms patterns (plus optional Queues/Workflows integration)
- a natural place to implement “pause for approval” flows

### Key design decision: don’t embed DO classes into `apps/web` (recommended)

Today `apps/web` is built/deployed with:

```jsonc
// apps/web/wrangler.jsonc
"main": "@tanstack/react-start/server-entry"
```

That makes it awkward to also export Durable Object classes from the same Worker without changing the entrypoint strategy.

Recommendation:

- Create a **separate Worker** dedicated to the Agents SDK (e.g. `openagents-agent-worker`).
- Route it to a non-`/api` path on the apex domain (e.g. `openagents.com/agents/*`) or keep it private and call it via service binding.
- Keep `apps/web` as the UI/SSR worker, and make it the “auth + routing shim” for the browser.

## Suggested architecture

### Components

1. **Web UI (apps/web)** (Worker + SSR)
   - Auth (WorkOS)
   - Renders chat UI, left sidebar, right sidebar
   - Owns browser-facing “safe” endpoints like `/chat` (or `/agents/chat`)

2. **Agent Worker (new)**
   - Uses Cloudflare Agents SDK
   - Exposes agent instances as Durable Objects:
     - `UserAgent` (one per WorkOS user) and/or
     - `ThreadAgent` (one per chat thread) and/or
     - `ProjectAgent` (one per project)

3. **OpenAgents API (apps/api)** (Rust Worker at `/api/*`)
   - Auth/billing + managed OpenClaw endpoints under `/api/openclaw/*`
   - Continues to own `/api/*` on openagents.com

4. **OpenClaw Runtime (apps/openclaw-runtime)** (Containers + DO + R2)
   - Executes the actual OpenClaw gateway/runtime work

### Request flow (chat)

Recommended near-term flow (min UI change):

1. Browser → `apps/web` `POST /chat`
2. `apps/web` validates WorkOS session and resolves:
   - `userId` (WorkOS)
   - `threadId` (from assistant-ui runtime / URL / generated id)
3. `apps/web` forwards the request to the Agent Worker (service binding or internal HTTP), including:
   - `userId` as an authenticated principal
   - `threadId` as the agent instance key
4. Agent Worker routes to `ThreadAgent(threadId)`:
   - loads persisted state
   - runs the model + tools
   - streams incremental output/events
5. Response is streamed back to the browser (same format as today so the UI keeps working).

## How the Agents SDK maps to our product UI

### Left sidebar: chats / projects / OpenClaw

- **Chats:** `ThreadAgent` per thread. Durable state holds:
  - message history (or a rolling window + summaries)
  - tool execution receipts
  - “current task” progress
- **Projects:** `ProjectAgent` per project with:
  - project context, files/links, long-running tasks
  - “background agent” workflow (runs after the user disconnects)
- **OpenClaw:** surface “managed OpenClaw” status and actions:
  - provision instance
  - show runtime status, pairing requests
  - restart/backup

### Right sidebar: community posts & collaboration

No change required. Keep it in the existing data stack (Convex/Nostr/indexer) and render alongside the chat.

## Tooling model: where tool calls should live

Move “agent decisions” into the durable agent:

- The **Agent Worker** owns:
  - model prompt + policy
  - tool routing
  - durable state transitions
  - job/progress events
  - approval gating

Keep “UI and auth” in `apps/web`:

- browser never sees internal secrets
- `apps/web` validates user identity and passes only a scoped identity to the agent worker

### OpenClaw tools (concrete)

Today, `/chat` tools call `/api/openclaw/*` via internal headers from `apps/web/src/lib/openclawApi.ts`.

In the Agents SDK design:

- implement the OpenClaw toolset inside the **agent worker**
  - preferred auth: service-to-service token (or internal key) configured as worker secret
  - avoid delegating privileged headers to the browser
- keep the Rust API as the stable surface:
  - `/api/openclaw/instance`
  - `/api/openclaw/runtime/status`
  - `/api/openclaw/runtime/devices`
  - `/api/openclaw/runtime/devices/:requestId/approve`
  - `/api/openclaw/runtime/backup`
  - `/api/openclaw/runtime/restart`
  - `/api/openclaw/billing/summary`

## Human-in-the-loop approvals

We want explicit approval for actions like:

- provisioning a managed OpenClaw instance
- approving pairing/device requests
- restarting the gateway
- operations that cost money or touch user data (email/social)

Pattern:

1. Agent decides it needs to call a sensitive tool.
2. Agent emits an **approval request event** (`approval.requested`) with:
   - tool name + params
   - human-readable summary
   - risk/cost hints
3. UI shows a modal with Approve/Reject.
4. UI sends `approval.respond` back to the agent.
5. Agent resumes or aborts, and records an auditable receipt.

Implementation note: this is much easier if the agent owns a durable event log and the UI connects via WebSocket.

## Background work (minutes to days)

For long-running tasks (research, migrations, batch processing), we should:

- persist job state in the agent DO (so it can resume after restarts)
- stream progress events to connected clients
- keep working when no client is connected

Mechanisms (pick incrementally):

- Durable Object alarms/scheduling for lightweight timers
- Cloudflare Queues for asynchronous steps
- Cloudflare Workflows when we need durable, multi-step orchestration with retries/visibility

## Storage and indexing (threads/projects list)

Durable Objects are great for per-entity state, but not for “list all threads” queries.

Recommendation (incremental, minimal disruption):

- Keep an **index** in Convex (since we already use it in `apps/web`), storing:
  - `threadId`, `userId`, title/summary, timestamps, pinned/archived
  - optional “agent type” (`thread`, `project`, `openclaw`)
- Store the “heavy state” in the DO:
  - message history, tool receipts, job states, approval queue
- When the DO updates title/last activity, it writes a small update to Convex (server-to-server).

Later, we can move indexing to D1 (Cloudflare-native) if we want to reduce external dependencies.

## Routing constraints on openagents.com

- `/api/*` is owned by `apps/api` (Rust worker). Don’t mount agent endpoints under `/api/*`.
- Prefer one of:
  - `openagents.com/chat` (UI chat endpoint) → forwards internally
  - `openagents.com/agents/*` (agent runtime endpoints)

## Appendix: code pointers

- `apps/web/wrangler.jsonc` — UI worker config (Cloudflare Workers)
- `apps/web/src/routes/chat.ts` — current chat endpoint (AI SDK + OpenClaw tools)
- `apps/web/src/lib/openclawApi.ts` — current OpenClaw API client (internal headers)
- `apps/api/wrangler.toml` — Rust API worker routing (`openagents.com/api/*`)
- `apps/openclaw-runtime/wrangler.jsonc` — OpenClaw runtime (Containers + DO + R2)
