# Autopilot Chat Execution Plane (Convex-First MVP)

This doc records one resolved decision:

> For the MVP, should Autopilot chat run on a per-user Cloudflare Durable Object (DO/DO SQLite) execution plane, or should we run *all chat state* in Convex (with chunked streaming) and defer per-user Cloudflare persistence to post-MVP?

This matters because Autopilot is intentionally **realtime-first**, and we want multiplayer/observers to be trivial (Convex subscriptions).

## Decision (Resolved: MVP)

We are going **Convex-first for the MVP** (anon + authed):

- **No per-user Cloudflare infra for MVP**: no DO / DO-SQLite / per-user DO classes for chat or “user-space”.
- Cloudflare Worker remains the **single host** (SSR/static + APIs) and the place we run:
  - inference (current provider, via `@effect/ai/LanguageModel`)
  - server-side tool execution (when needed)
  - budgets/receipts enforcement
- Convex becomes the **canonical store for MVP**:
  - threads
  - messages
  - **chunked streaming deltas** (written every ~250–500ms or every N chars, never per-token)
  - receipts/budgets/tool calls (bounded; large payloads are BlobRefs)
  - (optional) presence/participants for multiplayer
- **Realtime UX** comes from Convex WebSocket subscriptions (clients watch Convex state update).
- **Anon -> authed continuity is REQUIRED**: when the user authenticates, we MUST preserve the anon transcript and attach it to an owned thread (see “Anon -> Owned Thread Migration”).
- Post-MVP, we MAY reintroduce DO/DO-SQLite as an execution-plane optimization (cheaper streaming, stronger per-user consistency), but Convex remains the product DB and multiplayer surface.

## MVP Reference Architecture (Convex-Only Chat, Chunked Streaming)

### Transport

- **Browser ↔ Convex**: WebSocket (Convex client) for realtime subscriptions. This is the primary “streaming” surface.
- **Browser ↔ Worker**: HTTP endpoints for SSR and secret-bearing operations (e.g. initiating inference runs). Do not proxy Convex subscriptions through the Worker.

### Minimal Convex Schema (v1)

Required tables (names illustrative):

- `threads`
  - `ownerId?: string` (WorkOS user id when authed; absent/null when anon)
  - `anonKey?: string` (stable secret for anon access; stored client-side)
  - `createdAt`, `updatedAt`, `title?`, `visibility?`
- `messages`
  - `threadId`
  - `role: "user" | "assistant" | "system"`
  - `status: "draft" | "streaming" | "final" | "error" | "canceled"`
  - `createdAt`, `updatedAt`
  - `runId?: string` (stable id for an assistant generation run)
  - `seq: number` (monotonic for streaming updates; see parts)
- `messageParts` (recommended, to bound updates + support idempotency)
  - `messageId`
  - `runId` (duplicate for indexing/idempotency)
  - `seq: number` (monotonic per `runId` or per `messageId`)
  - `kind: "text-delta" | "tool-call" | "tool-result" | "error" | "finish"`
  - `data` (bounded; large payloads are `BlobRef`s)
  - `createdAt`
- `receipts` (optional but strongly recommended)
  - `runId`
  - `type: "model" | "tool"`
  - `toolCallId?: string`
  - `data` (bounded + `BlobRef`s)
  - `createdAt`

Notes:

- For MVP, “Blueprint/bootstrap state” can also live in Convex (schemas + receipts still apply). If/when we introduce a DO execution plane, it becomes an optimization.

### Chunked Streaming Algorithm (Worker -> Convex)

Hard rules:

- **No per-token writes** to Convex.
- Writes MUST be **chunked** (time-based ~250–500ms and/or size-based N chars).
- Writes MUST be **idempotent** and safe to retry.

Reference algorithm:

1. Client submits a user message:
  - Convex mutation creates the user message.
  - Worker endpoint starts an inference run and returns `{ runId, assistantMessageId }`.
2. Worker creates/marks the assistant message:
  - `messages.status="streaming"`, `messages.runId=runId`, `messages.seq=0`.
3. Worker streams provider output, buffers locally, and flushes periodically:
  - every `T=250–500ms` OR when buffer length exceeds N chars:
    - append `messageParts` row `{ runId, messageId, seq++, kind:"text-delta", data:{ text } }`
4. Tool calls/results are written as parts:
  - `{kind:"tool-call"}` and `{kind:"tool-result"}` with stable `toolCallId`.
  - Large tool I/O MUST be stored in blob storage and referenced by `BlobRef` in `data`.
5. On completion:
  - append `{kind:"finish", data:{ usage, ... } }`
  - set `messages.status="final"`.
6. On error/cancel/budget stop:
  - append `{kind:"error", data:{ code, message } }` (or a `finish` with a stop reason)
  - set `messages.status="error" | "canceled"`.

Idempotency:

- Writes MUST be keyed by `(runId, seq)` (or equivalent unique constraint).
- Retrying a flush MUST NOT duplicate parts.

Backpressure/cancellation:

- If the client disconnects/cancels or budgets are exceeded, the Worker MUST stop writing parts and must finalize the run state in Convex (`status="canceled"` + terminal part).

### Anon -> Owned Thread Migration (Required)

We care about carrying “try before auth” transcripts into the owned Autopilot thread.

Minimum acceptable behavior:

- After authentication, the prior anon thread’s transcript MUST still be visible to the user in their owned Autopilot experience (at least `{role, text}` deltas assembled).

Recommended MVP mechanism (Convex-only makes this simple):

- Treat “anon” as a real Convex thread with `ownerId = null` and an `anonKey` secret held by the browser.
- On auth, run a Convex mutation that:
  - verifies the `anonKey`
  - sets `threads.ownerId = <userId>`
  - clears `anonKey` (or marks it rotated/expired)
- Result: the same `threadId` becomes owned without copying data.

If we later enforce “one thread per user id”, we can:

- create a new owned thread and copy parts/messages, OR
- keep a stable thread id and enforce “one default thread pointer” per user.

## Current State (What The Code Does Today)

### `/autopilot` chooses a chat id

- The Autopilot controller picks:
  - `chatId = session.userId ?? anonChatId`
  - `anonChatId` is stored in `sessionStorage` under `autopilot-anon-chat-id` and looks like `anon-<random>` (per-tab-session).  
    Source: `apps/web/src/effuse-app/controllers/autopilotController.ts`

### The browser connects to Agents WebSockets using that chat id

- The chat client uses Cloudflare Agents SDK:
  - WebSocket target: `WS /agents/chat/:chatId`
  - REST: `GET /agents/chat/:chatId/get-messages`, plus blueprint + contracts endpoints.
  - Implementation uses `AgentClient({ agent: "chat", name: chatId, ... })`.  
    Source: `apps/web/src/effect/chat.ts`, `apps/web/src/effect/agentApi.ts`

### The Worker routes `/agents/*` directly to Agents SDK routing

- `apps/web` Worker does:
  - `if (url.pathname.startsWith("/agents")) return routeAgentRequest(request, env as any)`
  - No guardrails before dispatch.
  - This is currently required because `/autopilot` MUST work for unauthed users.  
    Source: `apps/web/src/effuse-host/worker.ts`

### The Chat DO persists state in DO SQLite for *any* chat id (including `anon-*`)

- The Agent DO (`Chat extends Agent<Env>`) creates/persists:
  - `cf_ai_chat_agent_messages` transcript table
  - `autopilot_blueprint_state` table
  - DSE + AI/tool receipt tables
  - It loads messages from DB on construct and persists updates back into SQLite.
  - There are **no auth checks** in the DO for which client may access which chat id.
    Source: `apps/autopilot-worker/src/server.ts` (re-exported from `apps/web/src/effuse-host/do/chat.ts`)

Result: opening `/autopilot` while unauthed allocates a new DO instance keyed by `anon-...` and (today) writes persistent state.

## Why Re-Assess

### 1) Cost + abuse surface

- Anonymous visitors can create unlimited `anon-*` chat ids (new tab/session) and drive:
  - Workers AI usage (real cost)
  - DO CPU time
  - Potentially unbounded DO SQLite storage writes (transcripts, blueprint, receipts).

We will need a free-tier budget anyway; the question is where to enforce it and how expensive the anon execution plane should be.

### 2) Storage bloat and unclear retention

- `sessionStorage` ids are ephemeral, but DO SQLite persistence is durable.
- This is a mismatch: we persist “forever” for ids that are typically abandoned in minutes.

### 3) Security / chat id hijack risk

- Today, **anyone can connect to any `chatId`** because neither the Worker nor the DO checks identity.
- For authed users, `chatId = WorkOS userId` is *not* intended to be public.
  - Even if WorkOS ids are unguessable, accidental leakage is plausible (logs, screenshots, copied URLs, etc.).
- For anon ids, the id itself is acting like an auth token, but it is only ~12 chars of base36 and stored client-side.

We need an explicit access-control story regardless of which plane we choose.

### 4) Product posture: “Try it” vs “Own it”

The user journey we likely want:

1. **Try Autopilot** (no account): short-lived, limited, low-risk, minimal persistence.
2. **Authenticate**: unlock identity, ownership, and continuity.
3. **Credits/billing** (later): unlock durable workspace + heavier tools + longer context.

The current implementation gives anon users nearly the same backend surface as authed users.

## MVP Implications (Abuse Control, Security, Credits)

Even with Convex as the canonical store, the Worker remains the enforcement point for:

- budgets (token/time/tool caps)
- hard stops with visible `error`/terminal parts + receipts
- rate limiting and abuse controls (IP/session-based)

Convex is the natural place to store:

- user profile + entitlements (credits, flags)
- thread ownership and sharing
- presence/participants (if we add multiplayer affordances)

Security requirements:

- Authed thread access MUST be enforced by Convex auth (row-level ownership/membership checks).
- Anon thread access MUST be protected by an explicit secret (for example `threads.anonKey`) and must not rely on “thread id as bearer token”.
- Worker endpoints that initiate inference MUST validate access to the target thread (owner or valid anonKey), then write parts into Convex.

## Post-MVP (Optional): Reintroduce a Cloudflare Execution Plane

If/when we want cheaper “true streaming” and stronger per-user consistency:

- Introduce a per-user (or per-thread) execution plane (DO/DO-SQLite).
- Keep Convex as the product DB and multiplayer surface.
- Use event-sourced projection (idempotent `eventId`, monotonic `seq`) to mirror execution history into Convex.

This is a performance/consistency optimization, not an MVP requirement.

## Remaining Open Questions (Small)

1. Default flush cadence and chunk sizing: start at `T=350ms` and `N=1–2k chars`, or go tighter?
2. Blob storage for `BlobRef`s in MVP: Cloudflare R2 vs Convex file storage (or both)?
3. “One thread per user” enforcement: stable thread id vs “default thread pointer” mapping after auth?
