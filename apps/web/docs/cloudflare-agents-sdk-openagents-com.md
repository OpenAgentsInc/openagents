# Proposal: Use Cloudflare Agents SDK for durable remote-agent chat on openagents.com

## Context / why now

We want openagents.com to make it easy for people to interact with **remote agents** via chat, with:

- **Durable execution** (agents can run for minutes/days)
- **Automatic persistence** (state/memory survives restarts)
- **Real-time progress streaming**
- **Human approval** gates for sensitive actions

Cloudflare’s framing (paraphrase): “durable background agents that run for minutes or days, persist state automatically, stream progress in real time, and pause for human approval when needed.” This proposal is the concrete implementation plan for that inside openagents.com.

This aligns with how we’re building OpenClaw (formerly “Moltbot”; editorial note from Jan 30, 2026) and with the “Moltworker → OpenClaw runtime on Cloudflare” direction.

Related internal doc (background/product framing):
- `docs/local/openclaw-openagents-website-integration.md` proposes (a) an optional “link your own OpenClaw” bridge flow and (b) a hosted/multi-tenant OpenClaw track. The Agents SDK approach below maps cleanly to the hosted track, and can also serve as the “bridge” surface (with strict scoping + explicit user consent).

OpenClaw-specific “definitive plan” doc:
- `apps/web/docs/openclaw-on-openagents-com.md`

Separately, the planned UI split is already reflected in `apps/web`:
- Left sidebar: chats / projects / OpenClaw
- Right sidebar: community posts & collaboration

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

## Detailed roadmap (ship the important 80% first)

This section is the “do it now” path: get durable remote-agent chat working end-to-end in the website with minimal UI churn, while staying compatible with our current constraints (not owning `/api/*` on the apex domain, and `apps/web` currently being the TanStack Start SSR worker).

### What “80%” means (MVP definition)

Deliverables for the first iteration:

1. **Durable chat state per thread** (or per user if we want an even faster MVP): messages + minimal agent state stored in a DO.
2. **Streaming chat responses** that work with the current UI (keep the AI SDK “UI message stream” response shape so `@assistant-ui/react-ai-sdk` keeps working).
3. **A small OpenClaw tool surface** inside the durable agent (status, provision, device list/approve, backup/restart, billing summary).
4. **Human approval for risky steps** (at minimum: provisioning, device approval, restarts) with an explicit UI confirmation flow.
5. **A real thread list on the left sidebar** backed by Convex (title + last activity), so chats/projects/OpenClaw can actually be “on the left” across sessions/devices.

Defer (nice-to-have after MVP):
- full “state sync” to multiple clients (Agents SDK React hook style)
- complex job orchestration (Queues/Workflows) beyond a basic “background task” primitive
- deep community automation/bots (right sidebar is already fine)

### Milestone 1 (1 PR): Introduce the Agent Worker (durable chat core)

**Goal:** Stand up a new Worker that uses Cloudflare Agents SDK + Durable Objects, and can run a single durable chat instance.

Proposed changes:
- Add new app: `apps/agent-worker/`
  - `wrangler.jsonc` (or `wrangler.toml`) for `openagents-agent-worker`
  - `compatibility_flags`: `nodejs_compat`
  - DO binding(s): start with **one** DO class:
    - `ThreadAgent` (recommended), keyed by `threadId`
    - (optional faster MVP) `UserAgent`, keyed by WorkOS `userId`
  - Add `OPENAI_API_KEY` (or a Cloudflare AI Gateway URL) as secrets/vars as needed.
- Implement:
  - `ThreadAgent` durable state:
    - stored messages (or a rolling window + summary field)
    - an “approval queue” (pending approvals by id)
    - a small “event log” (for progress + auditing)
  - HTTP entrypoints for server-to-server use (keep browser out initially):
    - `POST /internal/chat` → streams AI SDK UI messages
    - `POST /internal/approval/respond` → approve/reject a pending approval
    - `GET /internal/thread/:threadId/summary` → for sidebar/title (optional in MVP if Convex holds it)

Auth contract (server-to-server):
- Require `X-OA-Internal-Key` (same concept as `apps/web` → `apps/api`) on every `/internal/*` request.
- Require `X-OA-User-Id` (WorkOS user id) so the agent can:
  - enforce ownership/tenant isolation (thread belongs to user)
  - call `apps/api` OpenClaw endpoints using the same internal auth scheme (or bearer token later)

Acceptance criteria:
- `POST /internal/chat` can be called from `curl` (or a tiny node script) and streams a response.
- State persists across requests (send a message, refresh, continue).

### Milestone 2 (1 PR): Connect `apps/web /chat` to the Agent Worker (feature flag)

**Goal:** Keep the UI unchanged, but move execution into the durable agent behind a flag.

Proposed changes:
- `apps/web/src/routes/chat.ts`
  - Add a “forward mode”:
    - if `process.env.AGENT_WORKER_URL` is set, forward the incoming chat request to `POST ${AGENT_WORKER_URL}/internal/chat`
    - include `X-OA-Internal-Key` and `X-OA-User-Id`
    - include `threadId` (see next bullet)
  - else, fall back to the current in-process `streamText()` implementation.
- Ensure the forwarded response is returned verbatim (streaming).

Thread identity (minimal change path):
- Update the client transport to send `threadId` on every request.
  - Recommended: move the chat UI to a thread route, e.g. `/_app/t/$threadId`, so the server can read `threadId` from the URL (no custom transport hacks).
  - Quick alternative (if `@assistant-ui/react-ai-sdk` supports it): include `threadId` as a header or in the JSON body alongside `messages`.

Acceptance criteria:
- With `AGENT_WORKER_URL` unset, the app behaves as today.
- With `AGENT_WORKER_URL` set, chat goes through the agent worker and remains streaming.

### Milestone 3 (1–2 PRs): Persist threads for the left sidebar (Convex-backed)

**Goal:** Make the left sidebar real: chats persist across sessions/devices and line up with DO identities.

Proposed changes:
- Add a Convex table (or extend existing schema) for `threads`:
  - `thread_id`, `user_id`, `title`, `created_at`, `updated_at`, `archived`
  - optional: `kind` (`chat` | `project` | `openclaw`) so the sidebar can mix “Chats / Projects / OpenClaw”.
- UI changes:
  - Replace `ThreadListPrimitive`-driven thread list (currently runtime-local) with a Convex query-backed list.
  - Selecting a thread navigates to `/_app/t/$threadId`.
  - “New Thread” creates a new `thread_id` (Convex mutation) and navigates to it.
- Agent worker changes:
  - When a thread receives its first assistant message, suggest/compute a title and write it back to Convex.

Acceptance criteria:
- Threads appear in the left sidebar after refresh and on another device.
- Each thread maps 1:1 to the DO instance key used by the agent worker.

### Milestone 4 (1 PR): Human approvals in the website (minimum viable)

**Goal:** “Pause for approval” works in the product UI.

Proposed changes:
- Agent worker:
  - Mark certain tools as approval-gated:
    - `openclaw.provision`
    - `openclaw.approveDevice`
    - `openclaw.restart`
    - (optional) `openclaw.backupNow`
  - When gated, emit an `approval.requested` event and stop until resolved.
- `apps/web` UI:
  - Add an approvals modal that subscribes to the current thread’s pending approvals.
  - Send `approval.respond` to the agent worker (`POST /internal/approval/respond`) via the `apps/web` server (not directly from the browser).

Acceptance criteria:
- The assistant can say “I need approval to restart OpenClaw” and the UI shows Approve/Reject.
- Approving continues the run without losing the stream.

### Milestone 5 (optional, 1–2 PRs): Background work + progress streaming

**Goal:** Long tasks keep running and users can watch progress.

Proposed changes:
- Agent worker:
  - A “job” abstraction stored in the DO:
    - `job_id`, `status`, `progress`, `logs`, `started_at`, `updated_at`
  - A streaming endpoint:
    - `GET /internal/thread/:threadId/events` (SSE or WebSocket) for real-time progress events
  - DO alarm/scheduling for resuming work.
- UI:
  - Show progress in-thread (tool events / status chips)
  - Show a “Running…” indicator in the thread list with last event timestamp

Acceptance criteria:
- Start a task, refresh the page, and see it still running / continuing.

## How to connect it (wiring + deploy plan)

This is the practical “how do we hook up the pieces?” section.

### 1) Create and deploy the Agent Worker

Proposed worker name: `openagents-agent-worker`

Suggested routing options (pick one):

1. **Internal-only (recommended):** deploy to Workers.dev and only call it from `apps/web` server-side using an internal URL.
   - Pros: no browser access, simpler auth story.
   - Cons: needs an `AGENT_WORKER_URL` configured.
2. **Apex path route:** attach a route like `openagents.com/agents/*` to the agent worker.
   - Pros: same-zone routing; easier to reason about later for WebSockets.
   - Cons: must be careful not to expose privileged endpoints; still recommend keeping `/internal/*` locked behind `X-OA-Internal-Key`.

Secrets/vars:
- Set on the agent worker:
  - `OA_INTERNAL_KEY` (for verifying calls from `apps/web`, and for calling `apps/api` in “beta internal auth” mode)
  - model credentials (`OPENAI_API_KEY` or AI Gateway config)

### 2) Wire `apps/web` to the agent worker

In `apps/web`:
- Configure:
  - `AGENT_WORKER_URL=https://<worker-subdomain>.workers.dev` (or the apex route origin)
  - `OA_INTERNAL_KEY` as an `apps/web` secret (already required today for `/chat` OpenClaw tools)
- Update `apps/web/src/routes/chat.ts` to:
  - compute `userId` (WorkOS) as it already does
  - forward to `${AGENT_WORKER_URL}/internal/chat`
  - include:
    - `X-OA-Internal-Key: ${OA_INTERNAL_KEY}`
    - `X-OA-User-Id: ${userId}`
    - `threadId` (url param or body)

### 3) Wire the agent worker to the Rust API (`apps/api`)

The agent worker should call OpenClaw endpoints via the stable surface:

- `https://openagents.com/api/openclaw/*`

Auth options:
- **Short-term (matches today):** internal headers:
  - `X-OA-Internal-Key` + `X-OA-User-Id`
- **Medium-term (better):** bearer tokens (agent principal) using `/api/auth/agent/register` and storing per-user/per-agent tokens in the DO/Convex.

### 4) Verify routing constraints on the apex domain

- Keep `apps/api` route as-is: `openagents.com/api/*`.
- Do **not** mount the agent worker under `/api/*`.
- If routing the agent worker on the apex domain, prefer `openagents.com/agents/*` (or `openagents.com/_agents/*`).
- Keep the main website worker (`apps/web` or `apps/website-*`) owning the remaining paths.

### 5) UI connection (thread routing)

Recommended UX wiring:
- `/_app/t/$threadId` route becomes the canonical chat URL.
- Left sidebar uses Convex to list `threads` for the user.
- Selecting a thread sets the active `threadId` used by `/chat` forwarding → DO instance key.

## Rollout plan (high-level)

### Phase 0 (today)

- Keep `apps/web/src/routes/chat.ts` as-is (AI SDK + OpenClaw tools).

### Phase 1: introduce the Agent Worker behind a flag

- Add a new worker that uses Cloudflare Agents SDK.
- Implement `ThreadAgent` with:
  - persistent conversation state
  - a minimal toolset (`openclaw.getInstance`, `openclaw.getStatus`, etc.)
- Add a feature flag so `POST /chat` can either:
  - run locally (current behavior), or
  - forward to `ThreadAgent` and stream back

### Phase 2: persist threads/projects for real

- Store thread/project metadata in Convex for sidebar rendering.
- Add “resume” behavior (reload thread state when user returns).

### Phase 3: human approvals + background jobs

- Add approval event protocol and UI modal.
- Add job model + progress stream.
- Add alarms/queues/workflows for long tasks.

### Phase 4: expand remote agents

- Projects: per-project agents with their own memory/workdir references
- Community bots (read-only at first) that can draft posts or summarize discussions

## Appendix: code pointers

- `apps/web/wrangler.jsonc` — UI worker config (Cloudflare Workers)
- `apps/web/src/routes/chat.ts` — current chat endpoint (AI SDK + OpenClaw tools)
- `apps/web/src/lib/openclawApi.ts` — current OpenClaw API client (internal headers)
- `apps/api/wrangler.toml` — Rust API worker routing (`openagents.com/api/*`)
- `apps/openclaw-runtime/wrangler.jsonc` — OpenClaw runtime (Containers + DO + R2)
