# Autopilot Anonymous Chat Execution Plane (DO vs Convex)

This is a brainstorming doc for one decision:

> Should unauthenticated (“anon”) users who open `/autopilot` get their own Cloudflare Durable Object (DO), or should we run anon chat on Convex first and only allocate user-scoped DO/DO SQLite once the user authenticates (and maybe once they have credits)?

This matters because Autopilot is intentionally **WebSocket-first** (streaming `@effect/ai/Response` parts), and today our “happy path” assumes the chat session lives in an Agent DO.

## Decision (Resolved: MVP)

We are going **Convex-first for the MVP**:

- **No per-user Cloudflare infra** for MVP: no DO / DO-SQLite / per-user DO classes for chat.
- Cloudflare Worker remains the **single host** (SSR/static + APIs) and the place we run inference (Workers AI) and server-side tool execution when needed.
- Convex becomes the **canonical store** for:
  - threads
  - messages
  - **chunked streaming deltas** (written every ~250–500ms, never per-token)
  - receipts/budgets/tool calls (bounded; large payloads are BlobRefs)
- **Realtime UX** comes from Convex WebSocket subscriptions (clients watch Convex state update).
- **Anon -> authed continuity is REQUIRED**: when the user authenticates, we MUST preserve the anon transcript and attach it to the owned thread (see “Move To Owned Thread”).
- Post-MVP, we MAY reintroduce DO/DO-SQLite as an execution-plane optimization (cheaper streaming, stronger per-user consistency), but Convex remains the product DB and multiplayer surface.

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

## Decision Criteria

Use these to judge options:

- **Streaming UX**: TTFT and smooth incremental updates.
- **Reliability**: reconnection behavior, resumability, minimal “stalled” cases.
- **Abuse control**: enforce budgets, per-ip/session throttles, and visibility of tool/AI errors.
- **Persistence semantics**: what is retained, for how long, and where.
- **Security**: prevent cross-user access to threads.
- **Migration**: can we carry an anon session into an authed “owned” session cleanly?
- **Implementation complexity**: do we stay within Agents SDK or invent a second stack?
- **Observability/receipts**: budget/receipt invariants still hold.

## Options

### Option A: Keep per-anon DOs, but harden + stop persisting

Keep the current `chatId = userId ?? anon-*` mapping and Agents SDK transport, but change semantics for anon ids.

Required work:

- Worker guard before `routeAgentRequest`:
  - If `chatId` is not `anon-*`, require an authenticated session and `chatId === session.userId`.
  - If `chatId` is `anon-*`, allow but apply strict limits (see below).
- In the Chat DO:
  - For `this.name.startsWith("anon-")`, do not write to SQLite (no transcript/blueprint persistence).
  - Optionally disable blueprint/tool endpoints for anon.
  - Optionally use a cheaper model + smaller context for anon.
- Add anon rate limiting:
  - simplest: per-IP limits at Worker edge (KV counter or DO counter)
  - or Convex-based counters (since the browser already connects via WS).

Pros:

- Minimal surface area change (no new protocol).
- Keeps WebSocket-first streaming with current client code.

Cons:

- Still creates a DO instance per anon session (but without durable writes).
- Requires careful “anon mode” branching in the Chat DO to avoid accidental persistence/features.

### Option B: Split DOs: `AnonChat` vs `Chat` (same protocol, different backing)

Create a separate DO class for unauth that is intentionally minimal and not persistent.

Shape:

- `Chat` DO (agent name `chat`):
  - authed-only
  - DO SQLite persistence (transcripts, blueprint, receipts)
  - full tool surface
- `AnonChat` DO (agent name `anon-chat`):
  - unauthed allowed
  - **no DO SQLite writes** (ephemeral)
  - limited tool surface (likely “no tools”)
  - strict budgets + rate limits

Client change:

- If `session.userId` exists: connect to `agent: "chat", name: userId`
- Else: connect to `agent: "anon-chat", name: anonChatId`

Pros:

- Makes it much harder to “accidentally” persist anon data.
- Lets us tune model/tooling/budgets for the try-before-auth experience.

Cons:

- Still per-anon DO unless we also shard (Option C).
- Requires another DO binding + more code paths.

### Option C: Sharded anon hub DO(s) (bounded DO count)

Instead of per-anon DO, create a fixed set of hub DOs:

- `AnonHub-0..63` (for example)
- Client picks a hub by hashing `anonSessionId`.
- Hub maintains per-session state in memory and multiplexes WebSockets.

Pros:

- Bounded DO count (no DO explosion).
- Still WebSocket-first and Cloudflare-native.

Cons:

- More custom protocol/state management (less “use Agents SDK as intended”).
- Hub hot-spot risk; would need good sharding and bounded per-session memory.
- Harder to do “resume stream” semantics cleanly.

### Option D: Convex-first for anon; DO after auth/credits (the proposed direction)

Anon chat lives in Convex. The browser streams by subscribing to Convex queries (WS), not by connecting to `/agents/*`.

Possible architecture:

1. Client creates or resumes an `anonThreadId` in Convex (stored in sessionStorage).
2. On user message:
   - Convex mutation writes the user message.
   - Triggers inference job (details below).
3. Inference runner (Worker/DO/Queue) produces `@effect/ai/Response` parts and writes them back to Convex in batches.
4. UI is a Convex subscription on `(threadId -> messages + parts)`.
5. After auth (and optionally after credits):
   - Create owned `threadId = userId` (Chat DO + DO SQLite).
   - Import the anon transcript into the owned DO (MUST for v1; at least `{ role, text }`).

Where inference runs:

- still likely Cloudflare Worker/DO because we standardized on Workers AI and receipts.
- Convex is the **control plane** and the realtime “fanout bus” (WS).

Pros:

- Avoids creating per-anon DOs and per-anon DO SQLite state.
- Convex gives a natural place for analytics and quota counters.
- Aligns with “Convex is the product DB + realtime backbone.”

Cons / risks:

- Streaming via Convex implies frequent writes (token-level writes are too expensive).
  - We’d need batching (e.g., write every 250-500ms or per sentence) to keep costs sane.
  - This increases perceived latency vs direct socket streaming.
- We’d be running **two** streaming stacks (Agents WS for authed DO, Convex WS for anon) unless we also migrate authed to Convex.
- More moving parts (Convex schema, mutations/actions, inference runner, subscription modeling).

### Option F: Convex-only MVP (no per-user DO; everything through Convex)

This is the “nuclear simplification” idea: keep hosting on Cloudflare (SSR/static), but run **all Autopilot state + backend actions** in Convex for the MVP, including authenticated users.

Shape:

- **Transport**
  - No `/agents/*` for MVP.
  - Browser uses Convex WS subscriptions as the realtime transport for chat state (messages + parts).
- **State**
  - Transcript, blueprint, tool receipts, budgets, and thread metadata live in Convex tables.
  - “One thread per user” becomes: `threadId = userId` in Convex (not DO name).
- **Inference + tools**
  - A Convex action (or action + scheduler) runs model inference and tool execution.
  - It writes incremental `@effect/ai/Response` parts into Convex so subscriptions update the UI.
  - Cancellation is modeled explicitly (e.g. `runId` + `cancel` mutation) since there is no DO-local abort controller to target.

Why it might be simpler:

- Single backend system for MVP: Convex schema + actions + auth.
- No per-user DO lifecycle, migrations, or DO auth gating.
- The “anon -> owned” migration becomes trivial (it’s all in Convex): on auth, re-bind the same thread (or copy it) to the user id.

Why it might be harder in practice (given today’s constraints):

- **Streaming becomes DB-write-driven.**
  - Convex subscriptions react to DB changes; there is no native token socket.
  - We must batch (e.g. every 250-500ms or per sentence) to keep write volume/cost sane.
  - This introduces more perceived latency and increases implementation complexity for “stop” and “resume”.
- **Provider constraint mismatch.**
  - Today we rely on Cloudflare Workers AI (`env.AI` binding) for “no keys” inference.
  - Convex actions cannot access `env.AI` directly; to keep the same provider we’d need an HTTP bridge (Cloudflare API token + REST) or accept a provider change for MVP.
- **We’d be rewriting working code.**
  - The current DO already ships: transcript persistence, blueprint, tools, receipts, WebSocket protocol.
  - A Convex-only MVP likely means replacing most of that with a different execution model.

Net: Convex-only MVP is conceptually clean and likely reduces infra surface area, but may be a larger rewrite than hardening the existing DO approach.

### Option E: Worker-only ephemeral WebSocket for anon (no DO, no Convex)

Implement a dedicated `GET /ws/autopilot-try` endpoint in the Worker fetch handler.

- No persistence.
- Simple budgets + rate limits.
- After auth, switch to DO chat.

Pros:

- No DO explosion.
- No Convex schema work.

Cons:

- More bespoke code; doesn’t leverage Agents SDK primitives (resume, message_updated, etc.).
- WebSocket reliability/hibernation story is weaker than DO-based websockets.

## “Move To Owned DO” Mechanics (If We Split Planes)

If we want to preserve the anon conversation after signup, we need a defined migration:

- Minimal migration: import only `{role, text}` messages (drop tool parts, drop blueprint).
- Better migration: import messages + selected metadata; keep receipts separate.

Implementation sketch (client-driven):

1. While unauthed, keep `anonChatId` and transcript in memory (or Convex).
2. After auth:
   - Fetch anon messages (`GET /agents/anon-chat/:anonId/get-messages` or Convex query).
   - Connect to owned `Chat` DO (`chatId = session.userId`).
   - Push messages into owned DO:
     - either via a dedicated endpoint `POST /agents/chat/:userId/import`
     - or via the existing “set messages” WS message (`CF_AGENT_CHAT_MESSAGES`) once connected.

Decision: **anon -> owned transcript migration is a MUST for v1**.

- Minimum acceptable behavior: after authentication, the user’s owned Autopilot thread must contain the prior anon transcript (at least `{ role, text }`).
- Preferred behavior: preserve tool parts when possible and keep a stable receipt trail (even if receipts remain stored separately).

## Credits/Billing Gating (Future-Proofing This Decision)

Even before “real billing,” we likely want:

- Free tier:
  - anon: low caps, minimal persistence, cheaper model, no tools
  - authed but no credits: limited caps, maybe still no heavy tools
- Paid/credits:
  - enable persistent DO SQLite state
  - enable tool execution with receipts
  - enable larger context windows and long-running tasks

Convex is a good place to store:

- user profile
- balance/credits
- feature flags / entitlements

Worker/DO is a good place to enforce at runtime:

- budgets (token/time/tool)
- hard stops with visible tool/AI error parts + receipts

## Security Requirements (Needed No Matter What)

Regardless of which plane we choose:

- **Authed chat access control**:
  - Requests to `/agents/chat/:userId` MUST require a WorkOS session for that `userId`.
  - A Worker pre-check is a good first line of defense (cheap), but the DO should also enforce.
- **Anon chat access control**:
  - Either accept that `anonChatId` is the bearer token (not great),
  - or introduce a `chatSecret` (random) stored in sessionStorage and required on WS connect and REST fetches.
- **Rate limiting**:
  - Must exist for anon. (IP-based + session-based).

## Recommendation (Provisional)

If we want the smallest delta with the biggest risk reduction:

1. Implement **Worker gating** for `/agents/chat/:id`:
   - `id === session.userId` (authed) OR `id` is `anon-*` (unauthed).
2. Stop persisting anon sessions:
   - either conditional “anon mode” inside `Chat`, or better, **Option B** (`AnonChat` DO with no SQLite writes).
3. Implement the required **anon -> owned transcript migration** on auth (minimum: `{ role, text }`; ideally preserve tool parts).

If we want the cleanest product separation (“try” on Convex, “own” on DO) and are OK with added complexity:

- pursue **Option D** (Convex-first anon chat) with explicit batching semantics for “streaming via subscription”.

## Open Questions

1. Do we want anon `/autopilot` to expose the full blueprint/tools UI, or a simplified try flow?
2. What is the free-tier cap (messages/day, tokens/day) and how do we enforce it (KV vs Convex vs DO)?
3. Is “Convex subscription streaming” acceptable latency-wise if we batch parts (vs direct WebSocket streaming)?
4. Should the long-term goal be a single chat transport (Agents WS everywhere), or is split transport acceptable?
5. If we considered a Convex-only MVP (Option F), would we accept a model/provider change (or an HTTP bridge) so inference can run inside Convex actions?
