# LiteClaw Early Access — One-Pager MVP Spec

**Product name:** LiteClaw
**Platform:** Cloudflare Workers + Durable Objects + Cloudflare Agents SDK
**Explicit constraint:** No containers, no per-user infra billing, no multi-runtime orchestration.

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

> A user can go from invite → chatting with their LiteClaw in under **3 minutes**, refresh the page, and keep going.

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

1. User opens `/liteclaw`
2. If gated → join waitlist
3. If approved → LiteClaw agent is created automatically
4. User types a message
5. Response streams immediately
6. User refreshes → conversation and memory persist
7. User returns tomorrow → same LiteClaw, same context

That’s it.

No branching paths. No modes. No configuration.

---

## Screens (max 2)

### 1. LiteClaw Chat

* Single chat interface
* Streaming responses
* Minimal agent status indicator: `ready | thinking | error`
* Clear reset button (“Reset LiteClaw memory”)

### 2. Waitlist / Access Gate

* Email or identity capture
* Simple “You’re in / You’re waiting” state

No dashboards. Keep the existing **left sidebar** only; **no right/community sidebar**. No community feed in the golden path.

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

* **Cloudflare Worker**

  * Routes `/liteclaw`
  * Auth / waitlist gating
* **Durable Object**

  * One DO per user (`liteclaw:<userId>`)
  * Owns:

    * conversation history
    * lightweight memory
    * agent state
* **Cloudflare Agents SDK**

  * Agent lifecycle
  * Streaming responses
  * Model invocation
  * Memory primitives (basic)

No Convex. No external orchestrator. No runtime indirection.

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

* **Identity (DO key):** issue a stable `liteclaw_user` HttpOnly cookie on first visit and use `liteclaw:<cookie>` as the DO id.
* **Concurrency:** one in-flight message per user; sending a new message while streaming cancels the previous stream.
* **Routes (minimum):**
  * `GET /liteclaw` (UI)
  * `POST /liteclaw/chat` (SSE streaming)
  * `POST /liteclaw/reset` (clears DO memory/history)
  * `POST /liteclaw/waitlist` (join; creates “waiting” record)
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
- App chrome:
  - `apps/web/src/components/assistant-ui/AppLayout.tsx` (right sidebar hidden on `/hatchery`)
  - `apps/web/src/components/assistant-ui/threadlist-sidebar.tsx` (left sidebar)
- Hatchery content:
  - `apps/web/src/components/hatchery/HatcheryFlowDemo.tsx`

### LiteClaw runtime (new Cloudflare Worker app)

This is the “real product”: **Workers + Durable Object + Agents SDK**. It owns chat state and streaming.

- App folder:
  - `apps/liteclaw/` (Cloudflare Worker + DO + UI assets)
- Cloudflare config:
  - `apps/liteclaw/wrangler.jsonc`
  - `apps/liteclaw/.dev.vars.example` (never commit real secrets)
- Worker/DO implementation (minimum set we will own):
  - `apps/liteclaw/src/server.ts` (Worker fetch + DO class)
  - `apps/liteclaw/src/shared.ts` (shared types/constants)
  - `apps/liteclaw/src/utils.ts` (small helpers; no business logic sprawl)
- UI (LiteClaw Chat screen; can be minimal):
  - `apps/liteclaw/src/app.tsx`
  - `apps/liteclaw/src/client.tsx`
  - `apps/liteclaw/src/styles.css`

### Agents SDK source (local dev dependency)

For development, we want to build against the local Agents SDK checkout.

- Local repo path on this machine:
  - `/Users/christopherdavid/code/agents`
- Package we depend on:
  - `/Users/christopherdavid/code/agents/packages/agents` (npm package name: `agents`)

Implementation detail (pick one):

- Option A: pin to npm `agents@^0.3.x` in `apps/liteclaw/package.json`.
- Option B (preferred for now): set `apps/liteclaw/package.json` → `"agents": "file:../../../agents/packages/agents"`.

---

## Explicit Include / Exclude (To Prevent Scope Creep)

This is the “what do we delete/disable” list (useful because `apps/liteclaw/` starts from `agents-starter`).

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

## Required Cloudflare Wiring (Paths + Config)

### Domain routing

We want `openagents.com/liteclaw` to hit the LiteClaw worker (not `apps/web`).

- Add a Workers route for LiteClaw:
  - `openagents.com/liteclaw*` → worker `liteclaw`
- Keep the main site route:
  - `openagents.com/*` → worker `openagents-web-app`

### Durable Object bindings

LiteClaw uses **one DO namespace**:

- Binding: `Chat` (or rename to `LiteClawAgent`)
- Class: `Chat` (or rename to `LiteClawAgent`)
- Config location: `apps/liteclaw/wrangler.jsonc`

### Secrets / env vars (LiteClaw worker)

We will choose one model provider path:

- Cloudflare Workers AI binding (preferred for “no keys”):
  - `AI` binding in `apps/liteclaw/wrangler.jsonc`
- OR server-owned provider key:
  - `OPENAI_API_KEY` or `OPENROUTER_API_KEY` set as a Wrangler secret for `apps/liteclaw`

Waitlist gating:

- `LITECLAW_ADMIN_SECRET` (single shared secret for a tiny approval endpoint), or
- `LITECLAW_ALLOWED_EMAILS` (comma-separated allowlist) for the absolute simplest version.

---

## Local Dev / Test / Deploy (LiteClaw worker)

Commands live with the app:

- `cd apps/liteclaw && npm run dev`
- `cd apps/liteclaw && npm run test`
- `cd apps/liteclaw && npm run deploy`
