# LiteClaw Early Access — One-Pager MVP Spec

- **Product name:** LiteClaw
- **Platform:** Cloudflare Workers + Durable Objects + Cloudflare Agents SDK
- **Explicit constraint:** No containers, no per-user infra billing, no multi-runtime orchestration.

---

## Objective

Prove that users want a **persistent, personal AI agent** that:

* remembers context,
* responds reliably,
* and feels “always there” without setup friction.

This is **not** an autonomy platform, not a marketplace, not a tooling hub.

It is a **persistent chat agent that works every time**.

---

## North Star

> A user can go from invite → chatting with their LiteClaw in under **3 seconds**, refresh the page, and keep going.

---

## Success Metrics (instrument from day one)

* **TTFT (time to first token):**

  * p50 < 5s
  * p95 < 10s
* **Activation:** ≥80% of approved users send ≥5 messages
* **Retention:** ≥30% return next day
* **Reliability:** ≥99.5% of messages return a response
* **Zero infra cost per user** (Durable Objects only)

If these aren’t green, nothing else ships.

---

## The Golden Path (the only path)

1. User opens `/hatchery`
2. If gated → join waitlist
3. If approved → click **Spawn your LiteClaw**
4. Spawn redirects user to `/chat/{id}`
5. In `/chat/{id}`, the existing chat UI streams responses from the LiteClaw agent
6. User refreshes → conversation and memory persist
7. User returns tomorrow → same LiteClaw, same context

That’s it.

No branching paths. No modes. No configuration.

---

## Screens (max 2)

### 1. Hatchery (LiteClaw Spawn)

* Basic “what is LiteClaw?” copy
* Status: `not spawned | spawning | ready | error`
* Primary CTA: **Spawn your LiteClaw**
* If already spawned: **Go to chat** (links to `/chat/{id}`)
* Secondary action: “Reset LiteClaw memory” (clear DO state)

### 2. Chat (Existing UI)

* Route: `/chat/{id}`
* Uses the existing OpenAgents chat UI
* The backend is **LiteClaw (Agents SDK + Durable Object)**, not OpenClaw
* Streaming responses (resumable)
* Stop button works

Waitlist gating is an overlay/state on Hatchery (not a separate dashboard).

No dashboards. Keep the existing **left sidebar**. The right/community sidebar is not part of the LiteClaw golden path (and is hidden on `/hatchery`).

**Chrome decision (immediate):** `/hatchery` uses the existing left sidebar layout but hides the right sidebar.

---

## What LiteClaw *Is*

* A **single Cloudflare Agent instance per user**
* Backed by a **Durable Object**
* Powered by the **Cloudflare Agents SDK**
* Stateful across requests
* Stateless when idle (hibernates automatically)

LiteClaw is **one agent**, not a fleet.

---

## What LiteClaw Is *Not* (Hard Cuts)

Explicitly out of scope for Early Access:

* “OpenClaw” branding or concepts
* Containers / Sandboxes
* Multi-agent orchestration
* Tool calling frameworks
* Approval UX
* Sessions UI
* Device or DM pairing
* Agent parity (API keys, non-human login)
* Community / Moltbook
* Billing, credits, payments
* Marketplace or plugins
* Multi-instance per user

If it’s not required to chat + remember, it doesn’t exist.

---

## Architecture (minimum viable)

### Core components

* **OpenAgents Web (existing worker)**
  * Owns the UI routes: `/hatchery`, `/chat/{id}`
  * Owns auth + gating (WorkOS + existing waitlist/access checks)
  * Can keep using Convex for:
    * waitlist/access state
    * thread index (so the left sidebar + `/chat/{id}` keep working)
* **LiteClaw runtime (Cloudflare Worker + Agents SDK)**
  * Owns the agent websocket endpoints (Agents SDK / PartySocket)
  * Owns chat streaming + persistence (via Durable Object SQLite)
* **Durable Object (LiteClaw agent)**
  * One DO per user (EA: exactly one chat id per user)
  * DO id is derived from the chat id we redirect to: `/chat/{id}`
  * Owns:
    * conversation history (canonical)
    * rolling summary memory
    * resumable stream state

Convex is allowed for *UI metadata*, but **the agent’s state and transcript live in the Durable Object**.

---

## Agent Behavior (EA-level)

* Single rolling conversation
* Memory = summarized context + recent messages
* No autonomous background tasks
* No external side effects
* No tool execution beyond text generation

Think “ChatGPT with persistence,” not “autonomous worker.”

---

## Reliability Requirements (non-negotiable)

* Agent must:

  * respond to every message or return a clear error
  * never silently fail
* Durable Object must:

  * initialize deterministically
  * recover cleanly after eviction
* Streaming must:

  * always emit *something* within 10s

If streaming fails, fallback to non-streamed response.

---

## Safety & Cost Guardrails

* Per-user message rate limit
* Max context window enforced server-side
* Automatic summarization to cap memory size
* No user-supplied API keys
* No long-running execution

The goal is **predictable cost = $0 marginal infra**.

---

## Explicit Non-Goals (EA)

* No promises of “autonomy”
* No claims of “agents that act”
* No enterprise positioning
* No platform abstractions

LiteClaw is intentionally humble.

---

## What “Done” Means

Early Access is done when:

* Users intuitively understand what LiteClaw is in <10 seconds
* Chat works every time
* Memory persistence feels real
* Users come back unprompted

Architecture elegance, extensibility, and parity **do not count** as success criteria.

---

## Concrete Defaults (So We Can Ship Without Debates)

If we don’t explicitly choose these, the “simple” plan will still stall.

* **Identity (LiteClaw id):** on first Spawn, create or reuse exactly one Convex thread for the user (kind: `liteclaw`); the Convex `threadId` becomes the LiteClaw id and is the `{id}` in `/chat/{id}`.
* **DO key:** the LiteClaw agent DO uses the same id/room name as the chat id (e.g. `chat:{threadId}`), so `/chat/{id}` deterministically maps to one DO instance.
* **Concurrency:** one in-flight message per LiteClaw id; sending a new message while streaming cancels the previous stream.
* **Routes (minimum):**
  * `GET /hatchery` (UI: spawn + status + waitlist)
  * `GET /chat/{id}` (UI: existing Thread)
  * `WS /agents/chat/{id}` (LiteClaw agent websocket; chat requests + streaming)
  * `GET /agents/chat/{id}/get-messages` (rehydrate transcript; AIChatAgent built-in)
  * Reset is a websocket command (`CF_AGENT_CHAT_CLEAR`), surfaced as a UI button.
* **Approval mechanism:** keep it dumb—either a server-side allowlist (env var) or a tiny admin endpoint protected by a single secret header.
* **DO state shape (versioned):**
  * `schema_version`
  * `messages[]` (recent turns only; capped)
  * `summary` (rolling memory; capped)
  * `state` (`ready | thinking | error`)
  * `created_at`, `updated_at`
* **Memory policy:** keep last N turns (e.g. 25) + a single summary string; when history grows, summarize and drop oldest turns.
* **Model pin:** one fast/cheap default model, pinned server-side (no user config); enforce `max_output_tokens` and a max context window.
* **TTFT measurement:** log `ttft_ms` as the time to first model delta (not “status” events), plus `duration_ms` and `ok/error` for every message.

These defaults can be revised later, but we need *some* choice to build against.

---

## Why This Scope Is Correct

This spec:

* Tests **desire**, not ambition
* Eliminates infra risk
* Avoids premature abstraction
* Lets you observe real usage before naming futures

If users don’t care about a persistent LiteClaw, no amount of agent infrastructure will matter.

---

## Repo Map (Where The Code Lives)

LiteClaw EA is intentionally small. This section is here so we don’t scatter logic.

### Spec (source of truth)

- `docs/liteclaw/spec.md` (this doc)

### OpenAgents web shell (existing)

We keep using the existing OpenAgents UI chrome for navigation and gating, but **remove the right/community sidebar from Hatchery**.

- Route entrypoints:
  - `apps/web/src/routes/_app/hatchery.tsx`
  - `apps/web/src/routes/_app/chat.$chatId.tsx`
- App chrome:
  - `apps/web/src/components/assistant-ui/AppLayout.tsx` (right sidebar hidden on `/hatchery`)
  - `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` (left sidebar)
- Chat UI:
  - `apps/web/src/components/assistant-ui/thread.tsx` (message UI)
  - `apps/web/src/components/assistant-ui/openagents-chat-runtime.tsx` (chat runtime hook; will be updated for LiteClaw)
- Hatchery content (LiteClaw):
  - `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx` (currently implements the LiteClaw spawn/waitlist UI; rename to `LiteClawHatchery.tsx` when we touch it next)

### Legacy durable chat worker (removed)

The OpenClaw-era “durable /chat proxy” worker (`apps/agent-worker/`, custom DO + `/internal/chat`) has been **removed**. Chat is cut over to the Agents SDK worker under `/agents/*`; `/chat` and approvals no longer use `AGENT_WORKER_URL`.

### LiteClaw runtime (Agents SDK worker)

This is the “real product”: **Workers + Durable Object + Agents SDK**. It owns chat state and streaming.

- Shipped worker folder (LiteClaw runtime):
  - `apps/liteclaw-worker/` (Cloudflare Worker + DO; **no UI assets**)
- Reference folder (do not ship):
  - `apps/cloudflare-agent-sdk-demo/` (unmodified `agents-starter` template; used only to copy patterns)

### LiteClaw local agent (tunnel executor)

Local executor for Phase 4 tunnel-backed tools.

- `apps/liteclaw-local-agent/` (Node HTTP server for `workspace.read/write/edit`)

What moves from `apps/cloudflare-agent-sdk-demo/` → `apps/liteclaw-worker/`:

- Copy/keep (worker runtime only):
  - `package.json` (but **trim dependencies** to worker-only: `agents`, `@cloudflare/ai-chat`, `ai`, + one model provider)
  - `tsconfig.json`
  - `wrangler.jsonc` (DO binding + AI binding + observability; remove `assets` config)
  - `.dev.vars.example` (never commit real secrets)
  - `env.d.ts` (generated via `wrangler types`; should match bindings we actually use)
  - `src/server.ts` pattern:
    - `Chat extends AIChatAgent` (or rename to `LiteClawAgent`)
    - `export default { fetch(...) { return routeAgentRequest(request, env) || 404 } }`
  - `vitest.config.ts` + `tests/index.test.ts` (basic smoke tests are fine)
- Do not copy (demo UI + build tooling):
  - `index.html`, `public/`
  - `vite.config.ts`
  - `src/app.tsx`, `src/client.tsx`, `src/styles.css`
  - `src/components/**`, `src/hooks/**`, `src/providers/**`
  - any Tailwind/React/demo-only deps
  - `patches/` + `patch-package` (unless we *actually* use MCP and need the patch)

### Agents SDK source (reference only)

We keep a local checkout of the Agents SDK **only as a reference** (to read how things are built), not as a build dependency for LiteClaw.

- Local repo path on this machine:
  - `/Users/christopherdavid/code/agents`

LiteClaw should depend on the published npm package and pin it:

- `apps/liteclaw-worker/package.json` → `agents` (npm package) pinned to an explicit version.

Do **not** use a `file:` dependency pointing at the local repo.

---

## Hatchery → Chat (Concrete Wiring)

This is the “keep it dirt simple” plan: take the proven Agents SDK + AIChatAgent pattern from the starter, and render it through our existing `/chat/{id}` UI.

### Hatchery (LiteClaw spawn UI)

- `/hatchery` becomes a simple LiteClaw page (not a graph/canvas).
- `apps/web/src/routes/_app/hatchery.tsx` should render a new minimal component (e.g. `apps/web/src/components/hatchery/LiteClawHatchery.tsx`).
- It should reuse the existing gating primitives:
  - `apps/web/convex/access.ts` (`api.access.getStatus`)
  - `apps/web/convex/waitlist.ts` (`api.waitlist.joinWaitlist`)
- Clicking **Spawn your LiteClaw** should:
  - create or reuse exactly one Convex thread for this user (kind: `liteclaw`)
  - return that `threadId`
  - navigate to `/chat/{threadId}`
- EA constraint: do not expose multiple chats. `/assistant` and “New chat” should funnel into the single LiteClaw thread.
  - `apps/web/src/routes/_app/assistant.tsx` (stop redirecting to `/chat/new`)
  - `apps/web/src/routes/_app/chat.$chatId.tsx` (don’t create arbitrary new threads for EA)
  - `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` (hide/remove “New chat” button)

Recommended Convex shape (minimal):

- We already have the basics:
  - `apps/web/convex/schema.ts` includes thread `kind: 'liteclaw'`
  - `apps/web/convex/threads.ts` includes `getLiteclawThread` + `create({ kind: 'liteclaw' })`
- Optional (nice-to-have): add `getOrCreateLiteclawThread` mutation to avoid races and keep Hatchery logic dumb.

### Chat (existing `/chat/{id}` UI)

We keep the UI. We swap the backend.

- Route stays:
  - `apps/web/src/routes/_app/chat.$chatId.tsx`
- UI stays:
  - `apps/web/src/components/assistant-ui/thread.tsx`
  - `apps/web/src/components/assistant-ui/AppLayout.tsx` (chrome)
  - Update the empty-thread welcome state to be LiteClaw-first (today it is OpenClaw-first):
    - `apps/web/src/components/assistant-ui/thread.tsx` (`ThreadWelcome`, suggestions, and setup cards)

LiteClaw chat backing:

- The chat runtime for `/chat/{id}` should connect to the LiteClaw agent DO via Agents SDK:
  - `useAgent({ agent: "chat", name: threadId })` (WebSocket: `/agents/chat/{threadId}`)
  - `useAgentChat({ agent })` (streaming, persistence, resume)
- Transcript rehydration comes from AIChatAgent’s built-in endpoint:
  - `GET /agents/chat/{threadId}/get-messages`

Where the wiring lives (web):

- `apps/web/src/components/assistant-ui/openagents-chat-runtime.tsx` should be updated to back the assistant runtime with `useAgentChat` (Agents SDK) instead of the legacy HTTP `/chat` transport.
- `apps/web/src/components/assistant-ui/AppLayout.tsx` should keep the chrome (sidebar + header) but should not be the place we “do chat logic” anymore. The runtime hook should own the transport.

Implementation note:

- Today, `/chat` (the HTTP endpoint) is a legacy OpenClaw-era surface. LiteClaw should not depend on it; LiteClaw uses the Agent websocket transport.

### LiteClaw worker (Agents SDK runtime)

We adapt code from:

- Reference: `apps/cloudflare-agent-sdk-demo/src/server.ts` (AIChatAgent + `routeAgentRequest`)
- Shipped code: `apps/liteclaw-worker/src/server.ts`

EA changes required in the worker:

- no tools / confirmations / MCP:
  - do not ship `src/tools.ts`, `src/utils.ts`, or any UI tool-confirmation components
- remove scheduling (`agents/schedule`)
- pin a single model provider path (prefer Workers AI binding `AI`)
- keep observability on (Wrangler `observability.enabled = true`)

### Cloudflare routing (production)

- `/hatchery` and `/chat/*` stay on `openagents-web-app`.
- `/agents/*` must route to the LiteClaw worker so the browser can open the Agent websocket:
  - `openagents.com/agents*` → worker `liteclaw`

### Implementation Checklist (Paths)

Web (UI):

- Modify: `apps/web/src/routes/_app/hatchery.tsx` (render LiteClaw spawn UI)
- Optional: rename `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx` → `apps/web/src/components/hatchery/LiteClawHatchery.tsx`
- Modify: `apps/web/src/routes/_app/chat.$chatId.tsx` (EA: remove “new chat” creation; always funnel to LiteClaw thread)
- Modify: `apps/web/src/routes/_app/assistant.tsx` (redirect to LiteClaw thread, not `/chat/new`)
- Modify: `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` (hide “New chat” button for EA)
- Modify: `apps/web/src/components/assistant-ui/thread.tsx` (LiteClaw welcome copy)
- Modify: `apps/web/src/components/assistant-ui/openagents-chat-runtime.tsx` (use `useAgent` + `useAgentChat`)

Web (Convex metadata):

- Already: `apps/web/convex/schema.ts` (thread kind union includes `liteclaw`)
- Already: `apps/web/convex/threads.ts` (`getLiteclawThread`, `create`)
- Optional: add `apps/web/convex/liteclaw.ts` (`getOrCreateLiteclawThread`)

LiteClaw worker (Agents SDK runtime):

- Implement: `apps/liteclaw-worker/src/server.ts` (chat + memory only)
- Configure: `apps/liteclaw-worker/wrangler.jsonc` (DO binding + observability)

Cloudflare routes:

- Configure: `openagents.com/agents*` → worker `liteclaw`

---

## Explicit Include / Exclude (To Prevent Scope Creep)

This is the “what do we delete/disable” list (useful because LiteClaw starts from an `agents-starter` pattern).

### Include (EA)

- One DO per user, one rolling conversation, one memory summary
- Streaming responses with a 10s “first bytes” guarantee (fallback to non-streamed)
- Reset button (clears memory/history)
- Waitlist gating (simple allowlist or “approved users” table; no billing)
- Metrics logging: `ttft_ms`, `duration_ms`, `ok/error`, message counts

### Exclude (EA)

- Tool calling (no tools registry, no confirmations, no executions map)
- Scheduling/cron tasks
- Multiple chats/threads per user
- Multi-agent/fleet features
- Any “community” surfaces in the golden path

---

## Starter Template Cleanup (apps/liteclaw-worker)

When creating `apps/liteclaw-worker/`, start from the `agents-starter` pattern but **do not ship** anything that contradicts the “persistent chat only” promise.

Remove/disable for EA (concrete filepaths):

- Tool calling + confirmations:
  - do not include `apps/liteclaw-worker/src/tools.ts`
  - do not include `apps/liteclaw-worker/src/utils.ts` (tool-call processing helpers)
  - do not include any tool-confirmation UI components
- Scheduling:
  - `apps/liteclaw-worker/src/server.ts` (remove `agents/schedule` and any schedule prompt)

We can keep the files around during prototyping, but **don’t ship them in the EA golden path** (and don’t accidentally mention them in UX copy).

---

## Required Cloudflare Wiring (Paths + Config)

### Domain routing

We want `/hatchery` + `/chat/*` to stay on the main web worker, but the **Agents SDK websocket endpoints** to hit the LiteClaw worker.

- Add a Workers route for Agents SDK:
  - `openagents.com/agents*` → worker `liteclaw`
- Keep the main site route:
  - `openagents.com/*` → worker `openagents-web-app`

### Durable Object bindings

LiteClaw uses **one DO namespace**:

- Binding: `Chat` (or rename to `LiteClawAgent`)
- Class: `Chat` (or rename to `LiteClawAgent`)
- Config location: `apps/liteclaw-worker/wrangler.jsonc`

### Secrets / env vars (LiteClaw worker)

We will choose one model provider path:

- Cloudflare Workers AI binding (preferred for “no keys”):
  - `AI` binding in `apps/liteclaw-worker/wrangler.jsonc`
- OR server-owned provider key:
  - `OPENAI_API_KEY` or `OPENROUTER_API_KEY` set as a Wrangler secret for `apps/liteclaw-worker`

Waitlist gating:

- `LITECLAW_ADMIN_SECRET` (single shared secret for a tiny approval endpoint), or
- `LITECLAW_ALLOWED_EMAILS` (comma-separated allowlist) for the absolute simplest version.

---

## Local Dev / Test / Deploy (LiteClaw worker)

Commands live with the app:

- `cd apps/liteclaw-worker && npm run dev`
- `cd apps/liteclaw-worker && npm run test`
- `cd apps/liteclaw-worker && npm run deploy`

---

## Coding Agent Roadmap (Do This In Order)

This section is intentionally procedural. If you’re the coding agent implementing LiteClaw, follow this order and don’t “improve” scope.

### 1) Create `apps/liteclaw-worker/` (Shipped Runtime)

Goal: a Cloudflare Worker that serves the Agents SDK websocket endpoints under `/agents/*` and persists chat in a Durable Object (SQLite).

Target tree (minimum):

- `apps/liteclaw-worker/package.json`
- `apps/liteclaw-worker/wrangler.jsonc`
- `apps/liteclaw-worker/tsconfig.json`
- `apps/liteclaw-worker/env.d.ts` (generated by `wrangler types`)
- `apps/liteclaw-worker/src/server.ts`
- `apps/liteclaw-worker/tests/index.test.ts`
- `apps/liteclaw-worker/vitest.config.ts`
- `apps/liteclaw-worker/.dev.vars.example` (never commit secrets)

What to copy from `apps/cloudflare-agent-sdk-demo/` (and how to simplify it):

- Copy `wrangler.jsonc` → `apps/liteclaw-worker/wrangler.jsonc`
  - Keep: `compatibility_date`, `nodejs_compat`, `durable_objects.bindings`, `migrations` (sqlite), `observability.enabled`
  - Keep (preferred): `"ai": { "binding": "AI", "remote": true }`
  - Remove: `"assets": { ... }` (no UI in this worker)
  - Set `"name": "liteclaw"` (this is the worker that receives `openagents.com/agents*`)
- Copy `src/server.ts` patterns (don’t copy verbatim):
  - Keep: `routeAgentRequest(request, env)` + `AIChatAgent` subclass
  - Remove: scheduling (`agents/schedule`, `getSchedulePrompt`, `executeTask`)
  - Remove: tools (`tools.ts`, `utils.ts`, `processToolCalls`, confirmations)
  - Remove: `/check-open-ai-key` endpoint and any `process.env.*` checks from the demo
  - Replace model with a single pinned provider path:
    - Preferred: Workers AI binding (`env.AI`) in the worker
    - Allowed: server-owned `OPENAI_API_KEY` / `OPENROUTER_API_KEY` secret, pinned model
- Copy `tsconfig.json`, `vitest.config.ts`, `tests/index.test.ts` and trim as needed.
- Do not copy any Vite/React/UI files:
  - `index.html`, `public/`, `vite.config.ts`, `src/app.tsx`, `src/client.tsx`, `src/styles.css`, `src/components/**`, `src/providers/**`, `src/hooks/**`
  - Remove demo-only deps from `package.json` (React, Tailwind, Radix, etc)

Minimum `package.json` intent (don’t overthink it):

- Scripts:
  - `dev`: `wrangler dev`
  - `deploy`: `wrangler deploy`
  - `types`: `wrangler types --include-runtime false`
  - `test`: `vitest`
- Dependencies (pin exact versions):
  - `agents`
  - `@cloudflare/ai-chat`
  - `ai`
  - exactly one model provider (Workers AI or OpenAI/OpenRouter)

Acceptance checks:

- `npm run dev` starts without bundling UI assets
- `GET /agents/*` routes through `routeAgentRequest` (no 500s)
- Durable Object migrations are configured (sqlite class is declared in `migrations`)

### 2) Add Production Route: `openagents.com/agents*` → `liteclaw`

Goal: ensure the browser websocket connects to the LiteClaw worker.

- Cloudflare Dashboard → Routes:
  - `openagents.com/agents*` → worker `liteclaw`
  - `openagents.com/*` → worker `openagents-web-app`

Acceptance checks:

- In production, opening `/hatchery` then navigating to `/chat/{id}` results in a websocket connection to `/agents/chat/{id}` that upgrades successfully (101).

### 3) Web: Make `/hatchery` The LiteClaw Spawn UI (Left Sidebar Only)

Goal: `/hatchery` is a simple page that funnels into one DO-backed chat.

- Confirm `/hatchery` is under the `_app` layout:
  - Route: `apps/web/src/routes/_app/hatchery.tsx`
  - Layout: `apps/web/src/components/assistant-ui/AppLayout.tsx`
- Ensure the right/community sidebar is hidden on `/hatchery`:
  - `AppLayout.tsx` should not render `RightSidebar` when `pathname.startsWith('/hatchery')`
- Implement Hatchery UI (keep it boring):
  - Current implementation lives in `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx`
  - Optional cleanup: rename to `LiteClawHatchery.tsx`
- Spawn behavior:
  - Use `api.threads.getLiteclawThread` + `api.threads.create({ kind: 'liteclaw' })`
  - Redirect to `/chat/{threadId}`

Acceptance checks:

- `/hatchery` shows left sidebar and no right sidebar
- Clicking “Spawn your LiteClaw” takes you to `/chat/{id}`

### 4) Web: Switch `/chat/{id}` Transport To Agents SDK (Keep The UI)

Goal: keep the existing chat UI, but replace the backend transport with Agents SDK + DO state.

- Update `apps/web/src/components/assistant-ui/openagents-chat-runtime.tsx`:
  - Replace the legacy HTTP transport (`/chat`) with Agents SDK websocket transport:
    - `useAgent({ agent: 'chat', name: threadId })` (from `agents/react`)
    - `useAgentChat({ agent })` (from `@cloudflare/ai-chat/react`)
  - Rehydrate transcript via:
    - `GET /agents/chat/{threadId}/get-messages`
- Keep the route:
  - `apps/web/src/routes/_app/chat.$chatId.tsx`
- Keep the UI component:
  - `apps/web/src/components/assistant-ui/thread.tsx`

Acceptance checks:

- In production, POSTing a message from `/chat/{id}` results in streamed tokens via the websocket
- Refreshing `/chat/{id}` restores the transcript (DO-backed)

### 5) Enforce EA Constraint: One LiteClaw Thread Per User

Goal: remove all “new chat” affordances for Early Access.

- `apps/web/src/routes/_app/assistant.tsx` should always redirect to the LiteClaw thread (create if missing)
- `apps/web/src/routes/_app/chat.$chatId.tsx` should reject `chatId === 'new'` for EA
- `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` should hide/remove “New chat”

Acceptance checks:

- There is exactly one `kind: 'liteclaw'` thread per user
- Navigating to `/chat/new` doesn’t create extra threads

### 6) Verification + Deploy

Goal: ship with confidence.

- LiteClaw worker:
  - `cd apps/liteclaw-worker && npm run test`
  - `cd apps/liteclaw-worker && npm run deploy`
- Web:
  - `cd apps/web && npm run test`
  - `cd apps/web && npm run deploy`

Production smoke (minimum):

- `/hatchery` loads (left sidebar only)
- Spawn creates/uses a LiteClaw thread and redirects to `/chat/{id}`
- `/chat/{id}` connects websocket to `/agents/chat/{id}` and streams
- Refresh restores transcript

---

## Log
- 2026-02-05: Implemented LiteClaw web wiring (Hatchery spawn now uses get-or-create, /assistant and /chat/new funnel to the single LiteClaw thread, chat transport switched to Agents SDK), updated welcome copy, and hid new-chat affordances.
- 2026-02-05: Ran LiteClaw worker + web tests, deployed liteclaw worker and openagents-web-app, and pushed Convex functions for the LiteClaw web flow.
- 2026-02-05: Added Hatchery "Reset LiteClaw memory" action that sends CF_AGENT_CHAT_CLEAR via Agents SDK and reran web tests.
- 2026-02-05: Added LiteClaw worker metrics logging (ttft_ms, duration_ms, ok/error, message_count) and reran worker tests.
- 2026-02-05: Added LiteClaw worker guardrails (per-user rate limit, rolling summary + trimming, cancel in-flight streams, fallback to non-streamed response on stream failure), reran worker tests, and deployed liteclaw worker.
- 2026-02-05: Implemented Sky-mode scaffolding in the LiteClaw worker (sky tables + memory storage, run/event/receipt logging behind `LITECLAW_SKY_MODE`, `/agents/chat/{id}/export` JSONL output), reran worker tests, and deployed liteclaw worker.
- 2026-02-05: Added Phase 1 Sky contracts (TypeBox + AJV schemas for messages/events/receipts, tool args streaming contract, R2 ref normalization, compatibility doc), reran worker tests, and deployed liteclaw worker.
- 2026-02-05: Implemented Phase 2 tool registry (http.fetch + summarize + extract), added tool policy/budgets, tool event + receipt logging, updated Sky receipt schema to include tool receipts, reran worker tests, and deployed liteclaw worker.
- 2026-02-05: Implemented Phase 3 workspace tools (read/write/edit) with executor gating and diff receipts (patch_hash), updated Sky contracts + compatibility doc, reran LiteClaw worker tests, and deployed liteclaw worker (version `1f9c5eb4-7bb6-4d52-bf07-ada1cfea2665`).
- 2026-02-05: Implemented Phase 4 tunnel executor scaffolding (LiteClaw local agent for read/write/edit, tunnel dispatch + signed local receipts), updated Sky contracts + compatibility doc, reran LiteClaw worker tests, and deployed liteclaw worker (version `1be3086a-c2af-4645-a4e0-135a83cc1db5`).
- 2026-02-05: Implemented Phase 5 extension scaffolding (manifest allowlists + per-thread policy endpoint, extension hooks + metrics, and a sample `sky.echo` tool extension), reran LiteClaw worker tests, and deployed liteclaw worker (version `ef9a671d-e0b6-4352-a440-365f37cdcc9a`).
- 2026-02-05: Added extension catalog admin endpoint (`/extensions/catalog`) for managing manifests, reran LiteClaw worker tests, and deployed liteclaw worker (version `d4f0cdf3-bb51-4763-9400-70d2392760b5`).
- 2026-02-05: Added per-extension tool-call metrics logging for extension-owned tools, reran LiteClaw worker tests, and deployed liteclaw worker (version `699014ce-d654-4241-8467-30f5f6ffdec2`).
- 2026-02-05: Enforced extension tool declarations (tools must be listed in `manifest.tools`), reran LiteClaw worker tests, and deployed liteclaw worker (version `40ccc90a-82d3-4bed-84a7-8ee7c9be20dd`).
- 2026-02-05: Enforced extension policy updates against allowlist + catalog, reran LiteClaw worker tests, and deployed liteclaw worker (version `d09184b1-f77d-42e4-aa75-cc08f9b5119a`).
- 2026-02-05: Added KV/R2-backed extension catalog loading with configurable key, reran LiteClaw worker tests, and deployed liteclaw worker (version `a9e3b627-3359-4ed8-904a-b0e9d4c914fe`).
- 2026-02-05: Enforced pinned extension versions in policy updates, reran LiteClaw worker tests, and deployed liteclaw worker (version `bd986663-714a-4a5d-811c-848e3d616e30`).
- 2026-02-05: Added manifest-only extension support (system prompt only, no tools), reran LiteClaw worker tests, and deployed liteclaw worker (version `3ee02e40-97b6-4205-baab-3c0ab7ed1e58`).

---

## Post-EA Roadmap: Cloudflare-Native Sky (LiteClaw-First)

Goal: build our own Pi-style runtime, branded as **Sky**, on Workers + Durable Objects, while keeping the LiteClaw product surface stable.

Non-goal: replacing the LiteClaw UI or transport. The UI (`/chat/{id}`) and transport (`/agents/chat/*`) stay constant; only the runtime beneath evolves.

### Why Sky-style (what changes vs AIChatAgent)

AIChatAgent is great for “chat that persists.” Sky adds:

- A stable run/event contract (streamed deltas + structured tool lifecycle)
- Tool semantics (schemas, streaming args, receipts, replay)
- Determinism and observability (run IDs, step IDs, timings, typed errors)
- Portability (export/import into OpenClaw, later reuse skills/extensions)

Principle: LiteClaw remains the UX; Sky becomes the engine.

---

### Core Contract (freeze early)

These primitives are standardized first so later phases do not thrash.

#### 1) IDs (always present)

- `thread_id` (LiteClaw id)
- `run_id` (one per user send)
- `step_id` (model step or tool step)
- `event_id` (monotonic per run)

#### 2) Message model (Sky-ish)

- Roles: `system | user | assistant | tool`
- `content[]` parts: `{ type: "text" | "image" | "json" | "ref", ... }`
- `metadata` bag: model id, tokens, timings, policy flags

#### 3) Event stream (truth the UI renders)

Even if tools are not enabled, the stream shape stays unified:

- `run.started`
- `model.delta`
- `model.completed`
- `tool.call.started`
- `tool.call.delta`
- `tool.call.completed`
- `tool.result`
- `run.error`
- `run.completed`

#### 4) Receipts (replay unit)

Receipts are append-only and emitted for every run and tool:

- `receipt.run`: input hash, model config id, output hash, timings
- `receipt.tool`: tool name, args hash, output hash, duration, status

Receipts are the base layer for replay, billing later, debugging, and trust.

#### 5) Versioning policy

- `liteclaw_session_version` (int) on every transcript export
- `cf_sky_version` (semver) on every run receipt
- Backward compatibility: read old forever, write new only

---

### Phase 0 - Proof of Concept: LiteClaw-Sky Core in the Worker

Intent: build the core engine and adapters while keeping endpoints identical.

#### What we build

`cf-sky` module (internal)
- `ModelProvider`: `stream(messages, options) -> AsyncIterable<ModelEvent>`
- `Compactor`: `compact(messages) -> { summary, keep[] }`
- `EventMux`: converts model/tool events into the unified event stream

Message adapters
- `AIChatAgent` <-> `SkyMessage` conversion
- normalize `content` parts
- normalize roles
- preserve metadata

Persistence layout (DO SQLite)
- Keep AIChatAgent tables, add Sky tables alongside
- `sky_runs(run_id, thread_id, started_at, completed_at, status, model_config_id, error_code?)`
- `sky_events(run_id, event_id, type, payload_json, created_at)`
- `sky_receipts(run_id, receipt_json, created_at)`
- `sky_memory(thread_id, summary, updated_at, schema_version)`

Feature flag
- `LITECLAW_SKY_MODE=1` routes `onChatMessage` through `cf-sky`
- Same websocket endpoints, same UI rendering path

Export
- `GET /agents/chat/{id}/export` emits Sky-compatible JSONL
- includes messages, events, receipts (versioned)

#### Definition of done (tight)

- Flag off: existing behavior unchanged.
- Flag on: `/chat/{id}` works end-to-end: streaming, persistence, resume.
- Export JSONL loads in OpenClaw Pi tooling without conversion errors, or with a documented minimal adapter.
- Tests cover message conversion round-trip, event ordering monotonicity, and compaction correctness.

---

### Phase 1 - Contracts + Portability

Intent: turn Phase 0 “it works” into a stable contract for tools and skills.

#### What we add

- Typed schemas for events, messages, and receipts. Use TypeBox + AJV or Zod. Pick one and pin schema versions.
- Streaming tool-args contract, even if tools are stubbed. Partial JSON deltas; final args must validate.
- Attachment normalization with `ref` parts for R2 blobs (`r2://bucket/key#sha256=...`).
- Model registry config: `model_config_id -> provider + model + options`, stored in code/config.

#### Done when

- Every event payload validates against schemas in tests.
- Export includes `liteclaw_session_version` and schema versions.
- A compatibility doc exists: what LiteClaw exports guarantee.

---

### Phase 2 - Cloud-Only Tools (Workers Native)

Intent: introduce tools without containers and keep zero per-user infra costs.

#### Tool runtime (Workers-native)

Implement a tool registry with strict boundaries:

- `http.fetch` (allowlist + timeouts + max bytes)
- `r2.put/get/list` (scoped prefixes)
- `kv.get/put` (scoped)
- `d1.query` (scoped, optional)
- `summarize` (internal compaction / memory)
- `extract` (structured extraction, internal)

#### Security model

- Default-deny tool access.
- Per-thread tool policy: `none | read-only | read-write`
- Request budgets: max tool calls per run, max outbound bytes, per-domain allowlist for HTTP

#### Receipts

- Every tool call produces a receipt (args hash, output hash, timing)

#### Done when

- Tools show up in the event stream and render in the chat UI (basic cards).
- Tool calls are replayable from receipts in a test harness (deterministic tools only).
- Abuse controls exist (rate limit + allowlists).

---

### Phase 3 - Sandboxed Coding Tools (Containers)

Intent: add coding tools via containers without changing the contract.

#### Contract-first changes

- `ExecutorKind = workers | container | tunnel`
- Tool runtime dispatches `read/edit/write/bash` to an executor
- The same tool events and receipts are emitted regardless of executor

#### Workspace model

- Ephemeral workspace per run or per thread (choose one; default per thread with TTL)
- Snapshot export/import: tarball to R2 with hash + metadata

#### Done when

- `read` works end-to-end with receipts and replay.
- Deterministic diff receipt exists for `edit/write` (`input_hash`, `patch_hash`, `output_hash`).

---

### Phase 4 - Tunnel-Backed Local Tools (Your Repo)

Intent: make LiteClaw useful on real repos without Cloud infra.

#### What we build

- Local agent runtime implementing the same executor API.
- Signed requests, per-tool allowlists, and directory scoping.
- Cloudflare Tunnel endpoint bridges `tool.invoke` to local executor.
- Tool output streams back as tool events.

#### Trust and audit

- Every local tool invocation produces a receipt signed by the local agent, or at least hashed + timestamped.
- UI clearly labels “local tool executed.”

#### Done when

- User can run `read` and `edit` against a local directory with explicit scoping.
- Revocation story exists: disconnect tunnel, rotate token.

---

### Phase 5 - Skills + Extensions (Sky-Compatible)

Intent: reuse OpenClaw skills/extensions without changing LiteClaw UX.

#### Extension model

- Manifest-driven: name, version, tools, permissions, prompts, UI hints
- Loaded from R2/KV with pinned versions and an allowlist for EA
  - Per-thread enable/disable via `/agents/chat/{id}/extensions` (admin secret required)
  - Catalog management via `/agents/chat/{id}/extensions/catalog` (admin secret required)
- Runtime rejects extension tools not declared in `manifest.tools`
- Extension policy updates reject entries not in the allowlist or missing from the catalog
- Extension policy entries must include pinned versions (rejects missing `@version`)
- Catalog can be sourced from KV/R2 (key `extensions/catalog.json` by default)
- Prompt-only extensions (system prompt only, no tools) can be loaded without a custom runtime

#### Compatibility

- Align hook points to Sky Extension API: `onRunStart`, `onMessage`, `onToolCall`, `onRunComplete`
- Skills run inside the runtime: modify tool registry, inject system prompts, add memory transforms

#### Done when

- One extension can add one tool and it shows in UI.
- Extension metrics exist (calls, errors, latency).
- Disable/enable is policy-driven per thread.

---

### Risks and Guardrails (to prevent scope creep)

#### Key risks

- Event contract churn: freeze early and version everything.
- Memory bloat: cap aggressively and require summaries.
- Tool sprawl: start with 2-3 tools, enforce schemas and budgets.
- UI drift: keep UI dumb; it renders events, not logic.

#### Guardrails

- No new UI surfaces for Sky phases.
- No new principal types until Phase 4+.
- No marketplace framing until extensions exist and are stable.
