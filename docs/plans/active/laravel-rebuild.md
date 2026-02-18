# Laravel Rebuild Plan (Inertia + React + Laravel AI SDK)

Date: 2026-02-15

## Goal

Rebuild OpenAgents' current Effuse/Cloudflare/Convex web stack into **Laravel 12 + Inertia + React (TypeScript)** and use **Laravel AI SDK (`laravel/ai`)** as the primary model/tooling interface.

Near-term objective: migrate **basic tools** (starting with **L402 buying**) into the Laravel app so we can run an EP212-style L402 demo and then expand from there.

## Decisions (Locked)

- New app location: `apps/openagents.com/` (Laravel 12 + Inertia + React starter kit).
- UI stack: **Inertia + React (TypeScript)**.
- AI stack: **Laravel AI SDK (`laravel/ai`)** for agent + tool orchestration.
- Streaming protocol: **Vercel AI SDK data stream protocol** (what `laravel/ai` emits).
- MVP safety model: **no approval UX**. Enforce strict caps/allowlists in the tool contract (e.g. `maxSpendSats`).

## Current Status (Repo)

- `apps/openagents.com/` exists and already includes Inertia + React + TypeScript.
- WorkOS is installed (`laravel/workos`).
- Dev scripts exist (`composer dev`, `composer test`, `npm run dev`, `npm run build`, etc.).

## What We Learned From `laravel/ai`

We reviewed `/Users/christopherdavid/code/laravel-ai/`.

Key capabilities that matter for OpenAgents:

- Agents + tools:
  - Agents can expose tools via `HasTools`.
  - Tools are objects with name/description/JSON schema and a callable handler.
- Streaming:
  - `StreamableAgentResponse` can stream SSE using the **Vercel AI SDK data stream protocol**.
  - `StreamableAgentResponse::usingVercelDataProtocol()` produces `data:` lines and ends with `data: [DONE]`.

Important limitation for OpenAgents:

- The built-in conversation store captures final assistant content and tool calls/results, but it is not a full replay log of runtime decisions.
- If we keep our "receipts are replayable" invariant, we must implement an append-only **run event / receipt log** as a first-class table.

## OpenAgents Tool Surface (Current Codebase)

The core "basic tools" we support or expect to port over time include:

- `get_time`, `echo`
- L402 buyer: `lightning_l402_fetch`
- Hosted paywall control plane tools (later): `lightning_paywall_*`, `lightning_paywall_settlement_list`
- Bootstrap / identity / memory helpers (may become app features rather than LLM tools):
  - `bootstrap_set_user_handle`, `bootstrap_set_agent_name`, `bootstrap_set_agent_vibe`, `bootstrap_complete`
  - `identity_update`, `user_update`, `character_update`
  - `memory_append`, `tools_update_notes`, `heartbeat_set_checklist`, `blueprint_export`

## Target Architecture (Laravel)

### Runtime Components

- Laravel HTTP app (`apps/openagents.com/`)
  - Inertia + React UI
  - SSE endpoint(s) for streaming runs
  - REST endpoints for history + receipts
- Postgres (durable state)
  - threads, runs, messages
  - append-only run events (receipts)
  - lightning receipts + caches
- Redis (recommended early)
  - queues for background work
  - locks + rate limiting

### Data Model (MVP)

- `threads`
- `runs` (status: `queued|running|completed|failed|canceled`)
- `messages` (user/assistant; final text; linked to threads and runs)
- `run_events` (append-only)
  - text deltas (optional)
  - tool call started/succeeded/failed
  - tool receipts (hashes, latency, policy decision)
  - model finish + usage

### Streaming Strategy

- Server emits SSE via `laravel/ai` using Vercel data stream protocol.
- Client consumes the stream using the Vercel AI SDK React hook (`useChat`).
  - Do not hand-roll an SSE parser.
  - The hook already understands tool events like `tool-input-available` and `tool-output-available`.

Durability rules:

- The stream is "best effort UX".
- The database is the source of truth.
- After stream completion, the UI must be able to refresh and reconstruct the run from DB.

## Roadmap (Detailed, Ordered)

This is the implementation order. Each phase is scoped so it can be shipped and tested independently.

### Phase 0: Foundation (Dev, CI, Deploy Skeleton)

**Objective:** make `apps/openagents.com/` a stable app we can iterate on quickly.

1. Add a canonical "how to run" doc (or update an existing one) for the Laravel app:
   - `composer setup`
   - `composer dev`
   - `composer test`
2. Standardize environments:
   - Local: keep SQLite for fast iteration.
   - Add Docker Compose for Postgres + Redis so we can validate prod parity early.
3. CI baseline (Laravel app only):
   - Backend: `composer lint` and `composer test`.
   - Frontend: `npm run lint`, `npm run types`, `npm run build`.
4. Deploy skeleton (no AI yet):
   - Choose a staging domain (e.g. `next.openagents.com`).
   - Deploy "hello world" + a health endpoint.

**Verification:**
- `cd apps/openagents.com && composer test`
- `cd apps/openagents.com && npm run lint && npm run types && npm run build`

### Phase 1: Streaming Chat MVP (No Tools)

**Objective:** prove end-to-end streaming chat works (server streams, client renders, DB persists).

1. Add/configure `laravel/ai`.
2. Implement an agent class (e.g. `App\AI\Agents\AutopilotAgent`) with:
   - stable system prompt
   - a deterministic "no tools" mode
3. Create a streaming endpoint compatible with Vercel AI SDK:
   - Prefer `POST /api/chat` because `useChat` assumes it.
   - Return SSE using `StreamableAgentResponse::usingVercelDataProtocol()`.
4. UI: build a minimal chat page in Inertia React that uses `useChat`:
   - render incremental text deltas
   - render final assistant message
   - show a clear error state when streaming fails
5. Persistence:
   - store thread + messages (final text only) so refresh works

**Verification:**
- Feature test: stream endpoint returns valid SSE frames and ends with `[DONE]`.
- Manual: send message, observe deltas render.

### Phase 2: Runs + Append-Only Run Events (Receipts)

**Objective:** reintroduce auditability and determinism before adding tools.

1. Add migrations:
   - `threads`, `runs`, `messages`, `run_events`
2. Implement a Run Orchestrator service that:
   - creates a `run`
   - streams the agent response
   - writes the final assistant message
   - records finish metadata (provider/model/usage)
3. Add `run_events` writing:
   - `run_started`
   - `model_stream_started`
   - `model_finished`
   - `run_completed` / `run_failed`
4. UI: add a "Run details" drawer that loads from DB and shows the event timeline.

**Verification:**
- Feature test: refresh reconstructs a run from DB even if the client missed part of the stream.

### Phase 3: Tool Framework + First Tools (`get_time`, `echo`)

**Objective:** establish the tool contract and tool UI rendering pattern in Laravel.

1. Implement a tool registry with stable tool names and JSON schemas.
2. Implement deterministic hashing:
   - `params_hash = sha256(json_encode(params_canonical))`
   - `output_hash = sha256(json_encode(output_canonical))`
3. Wire tools into the agent via `HasTools`.
4. Persist tool events to `run_events`:
   - `tool_call_started` (includes `tool_name`, `tool_call_id`, `params_hash`)
   - `tool_call_succeeded|failed` (includes `output_hash`, latency, errors)
5. UI:
   - show compact tool summaries inline
   - keep raw JSON behind a collapsible

**Verification:**
- Feature test: tool execution produces ordered run events and correct hashes.

### Phase 4: L402 Buyer Tool (`lightning_l402_fetch`) MVP

**Objective:** implement the real EP212 demo path (pay for an L402 endpoint and return the paid payload).

**Tool contract (target):**

- Name: `lightning_l402_fetch`
- Params:
  - `url`, `method`, `headers`, `body`
  - `maxSpendSats` (hard cap)
  - `scope` (cache namespace; e.g. `ep212.sats4ai`)
  - optional: `allowHosts` / `denyHosts` (but keep a server-side allowlist as the real gate)
- Output (bounded):
  - `status` (`completed|cached|blocked|failed`)
  - `amountMsats`, `quotedAmountMsats`
  - `paymentBackend`
  - `proofReference` (never dump full secrets into the transcript)
  - response metadata:
    - `responseStatusCode`, `responseContentType`, `responseBytes`
    - `responseBodyTextPreview` (truncated)
    - `responseBodySha256`
  - `cacheHit`

**Implementation steps:**

1. L402 HTTP client:
   - initial request
   - on 402: parse `WWW-Authenticate: L402 macaroon=..., invoice=...`
   - apply policy before payment (hard cap + allowlist)
   - pay invoice, get preimage
   - retry request with `Authorization: L402 <macaroon>:<preimage>`
   - capture response preview + sha256 (bounded)
2. Payment backend abstraction:
   - define an internal `InvoicePayer` interface
   - ship at least:
     - `FakeInvoicePayer` (tests)
     - `LndRestInvoicePayer` (pragmatic default if we have an LND we control)
   - treat Spark as a follow-on unless we have a clean PHP client
3. Credential cache:
   - persist `(host, scope) -> macaroon + preimage + ttl`
   - store encrypted at rest (Laravel encryption) and enforce TTL
4. Receipts:
   - always write a run event with:
     - params hash
     - whether we paid
     - amount quoted/paid
     - response hash
     - cache hit/miss
5. UI:
   - show 1-line summary: `host + sats + cache hit/miss + proofReference prefix`
   - collapse full response preview and receipt JSON

**Verification (must be deterministic):**

- Local fake L402 server in tests:
  - request 1 returns 402 + invoice + macaroon
  - request 2 (with auth) returns 200 + premium payload
- Fake payer returns deterministic preimage.
- Tests assert:
  - first call pays and stores receipt + response preview/hash
  - second call is cache hit and does not pay
  - over-cap blocks pre-payment

### Phase 5: Demo Presets + Programmatic Demo Runner

**Objective:** make the demo executable by humans and by agents (CLI), not just in the UI.

1. Create endpoint presets (env/config) for EP212:
   - external L402 endpoint (e.g. sats4ai)
   - our internal endpoint later
2. Add an Artisan command that runs the demo end-to-end without a browser:
   - `php artisan demo:l402 --preset=sats4ai --max-spend-sats=100`
   - should print the payment summary + response sha256
3. Add a smoke test suite that runs against the local fake L402 server.

**Verification:**
- `php artisan demo:l402` succeeds locally against the fake server.

### Phase 6: Productionization + Cutover

**Objective:** run Laravel alongside the existing app until it is ready for traffic.

1. Staging domain + deployment:
   - Postgres (Cloud SQL)
   - Redis (Memorystore)
   - secrets via Secret Manager
2. Streaming reliability validation (SSE):
   - verify no buffering timeouts
   - verify proxy settings
3. Cutover plan:
   - keep current Effuse app as primary
   - route internal users to Laravel staging
   - later switch DNS / edge routing

## Risks / Open Questions

- SSE streaming reliability depends heavily on infra buffering (nginx/php-fpm/Cloud Run defaults). We must validate streaming early in staging.
- Spark wallet integration from PHP may require either:
  - an internal wallet microservice (Node/TS), or
  - LND as the payer in MVP, with Spark later.
- `run_events` must exist early or we will recreate the same "we can't debug what happened" problem later.

## Decision Log

- 2026-02-15: Commit to **Laravel + Inertia + React (TS)** and adopt `laravel/ai` streaming protocol for chat.
- 2026-02-15: MVP will **skip approvals**; safety enforced via strict tool caps/allowlists.
