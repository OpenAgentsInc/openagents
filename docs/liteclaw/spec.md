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
  - `apps/web/src/components/hatchery/` (new simple spawn UI lives here)
- Hatchery content (legacy / archive; remove from golden path):
  - `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx` (OpenClaw-era demo)

### LiteClaw runtime (Agents SDK worker)

This is the “real product”: **Workers + Durable Object + Agents SDK**. It owns chat state and streaming.

- App folder (current seed; will be renamed once we commit to it):
  - `apps/cloudflare-agent-sdk-demo/` (Cloudflare Worker + DO; currently includes a demo UI we will not ship)
- Cloudflare config:
  - `apps/cloudflare-agent-sdk-demo/wrangler.jsonc`
  - `apps/cloudflare-agent-sdk-demo/.dev.vars.example` (never commit real secrets)
- Worker/DO implementation (minimum set we will own):
  - `apps/cloudflare-agent-sdk-demo/src/server.ts` (Worker fetch + `routeAgentRequest()` + DO class)
  - `apps/cloudflare-agent-sdk-demo/src/shared.ts` (shared types/constants)
  - `apps/cloudflare-agent-sdk-demo/src/utils.ts` (helpers; trim down for EA)

The demo UI in the starter template is for reference only; the shipped UI is `apps/web`:

- Demo UI (do not ship in EA):
  - `apps/cloudflare-agent-sdk-demo/src/app.tsx`
  - `apps/cloudflare-agent-sdk-demo/src/client.tsx`
  - `apps/cloudflare-agent-sdk-demo/src/styles.css`

### Agents SDK source (reference only)

We keep a local checkout of the Agents SDK **only as a reference** (to read how things are built), not as a build dependency for LiteClaw.

- Local repo path on this machine:
  - `/Users/christopherdavid/code/agents`

LiteClaw should depend on the published npm package and pin it:

- `apps/cloudflare-agent-sdk-demo/package.json` → `agents` (npm package) pinned to an explicit version.

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

- extend `apps/web/convex/schema.ts` thread `kind` union to include `liteclaw`
- add a tiny helper mutation (new file is fine):
  - `apps/web/convex/liteclaw.ts` → `getOrCreateLiteClawThread(): threadId`

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

- `apps/cloudflare-agent-sdk-demo/src/server.ts` (AIChatAgent + `routeAgentRequest`)

EA changes required in the worker:

- remove tool calling + confirmations:
  - `apps/cloudflare-agent-sdk-demo/src/tools.ts`
  - `apps/cloudflare-agent-sdk-demo/src/utils.ts`
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
- Add: `apps/web/src/components/hatchery/LiteClawHatchery.tsx`
- Modify: `apps/web/src/routes/_app/chat.$chatId.tsx` (EA: remove “new chat” creation; always funnel to LiteClaw thread)
- Modify: `apps/web/src/routes/_app/assistant.tsx` (redirect to LiteClaw thread, not `/chat/new`)
- Modify: `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` (hide “New chat” button for EA)
- Modify: `apps/web/src/components/assistant-ui/thread.tsx` (LiteClaw welcome copy)
- Modify: `apps/web/src/components/assistant-ui/openagents-chat-runtime.tsx` (use `useAgent` + `useAgentChat`)

Web (Convex metadata):

- Modify: `apps/web/convex/schema.ts` (add `liteclaw` to thread kind union)
- Add: `apps/web/convex/liteclaw.ts` (`getOrCreateLiteClawThread`)

LiteClaw worker (Agents SDK runtime):

- Modify: `apps/cloudflare-agent-sdk-demo/src/server.ts` (strip tools/scheduling; keep chat + memory)
- Modify: `apps/cloudflare-agent-sdk-demo/wrangler.jsonc` (ensure DO binding + observability)

Cloudflare routes:

- Configure: `openagents.com/agents*` → worker `liteclaw`

---

## Explicit Include / Exclude (To Prevent Scope Creep)

This is the “what do we delete/disable” list (useful because `apps/cloudflare-agent-sdk-demo/` starts from `agents-starter`).

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

## Starter Template Cleanup (apps/cloudflare-agent-sdk-demo)

`apps/cloudflare-agent-sdk-demo/` currently starts from `agents-starter`, which is intentionally *more capable* than LiteClaw EA. Before we ship EA, we should delete/disable anything that contradicts the “persistent chat only” promise.

Remove/disable for EA (concrete filepaths):

- Tool calling + confirmations:
  - `apps/cloudflare-agent-sdk-demo/src/tools.ts`
  - `apps/cloudflare-agent-sdk-demo/src/components/tool-invocation-card/ToolInvocationCard.tsx`
  - `apps/cloudflare-agent-sdk-demo/src/utils.ts` (tool-call processing helpers)
- Scheduling:
  - `apps/cloudflare-agent-sdk-demo/src/server.ts` (remove `agents/schedule` and the schedule tool prompt)

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
- Config location: `apps/cloudflare-agent-sdk-demo/wrangler.jsonc`

### Secrets / env vars (LiteClaw worker)

We will choose one model provider path:

- Cloudflare Workers AI binding (preferred for “no keys”):
  - `AI` binding in `apps/cloudflare-agent-sdk-demo/wrangler.jsonc`
- OR server-owned provider key:
  - `OPENAI_API_KEY` or `OPENROUTER_API_KEY` set as a Wrangler secret for `apps/cloudflare-agent-sdk-demo`

Waitlist gating:

- `LITECLAW_ADMIN_SECRET` (single shared secret for a tiny approval endpoint), or
- `LITECLAW_ALLOWED_EMAILS` (comma-separated allowlist) for the absolute simplest version.

---

## Local Dev / Test / Deploy (LiteClaw worker)

Commands live with the app:

- `cd apps/cloudflare-agent-sdk-demo && npm run dev`
- `cd apps/cloudflare-agent-sdk-demo && npm run test`
- `cd apps/cloudflare-agent-sdk-demo && npm run deploy`
