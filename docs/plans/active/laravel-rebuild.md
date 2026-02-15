# Laravel Rebuild Plan (Inertia + React + Laravel AI SDK)

Date: 2026-02-15

## Goal

Rebuild OpenAgents' current Effuse/Cloudflare/Convex web stack into **Laravel 12 + Inertia + React (TypeScript)** and use **Laravel AI SDK (`laravel/ai`)** as the primary model/tooling interface.

Near-term objective: migrate **basic tools** (starting with **L402 buying**) into the new Laravel app so we can run the EP212-style L402 demo and then expand from there.

## Decisions (Locked)

- Frontend: **Inertia + React (TypeScript)**.
- Laravel AI integration: use **`laravel/ai` streaming + tools**.
- MVP safety model: **no explicit approval UX**. Instead, enforce strict caps/allowlists in the tool contract (e.g. `maxSpendSats`).

## What We Learned From `laravel/ai`

We reviewed `/Users/christopherdavid/code/laravel-ai/`.

Key capabilities that matter for OpenAgents:

- Unified provider abstraction (OpenAI, Anthropic, Gemini, OpenRouter, etc.) with failover.
- Agents with tools (`HasTools`) and structured output (`HasStructuredOutput`).
- Streaming: `StreamableAgentResponse` can stream SSE using the **Vercel AI SDK data stream protocol**.
  - `StreamableAgentResponse::usingVercelDataProtocol()`
  - Implementation: `CanStreamUsingVercelProtocol` (SSE `data:` lines; ends with `data: [DONE]`).

Important limitation for OpenAgents:

- The built-in conversation store captures final assistant content and tool calls/results, but it is not a complete replay log of runtime decisions. If we keep our “receipts are replayable” invariant, we must implement an append-only **run event / receipt log**.

## OpenAgents Tool Surface (Current)

From this repo’s current Autopilot tool registry (`apps/autopilot-worker/src/tools.ts`), the core “basic tools” we should expect to replicate over time include:

- `get_time`, `echo`
- L402 buyer: `lightning_l402_fetch` (and `lightning_l402_approve`, but MVP will not use approval)
- Hosted paywall control plane tools (later): `lightning_paywall_*`, `lightning_paywall_settlement_list`
- Bootstrap / identity / memory helpers (may become app features rather than LLM tools):
  - `bootstrap_set_user_handle`, `bootstrap_set_agent_name`, `bootstrap_set_agent_vibe`, `bootstrap_complete`
  - `identity_update`, `user_update`, `character_update`
  - `memory_append`, `tools_update_notes`, `heartbeat_set_checklist`, `blueprint_export`

## Architecture Sketch (Laravel)

### Runtime Components

- Laravel app (HTTP)
  - Inertia + React UI
  - SSE endpoint(s) for active runs
  - REST endpoints for history + receipts (and for RN/Electron later)

- Postgres (durable state)
  - threads, runs, messages
  - run events (append-only)
  - lightning receipts + caches

- Redis (optional but recommended early)
  - queues for background work
  - locks
  - rate limiting

### Data Model (MVP)

- `threads`
- `runs` (status: `queued|running|completed|failed|canceled`)
- `messages` (user/assistant; final text)
- `run_events` (append-only)
  - includes tool call/result, finish usage, and receipt hashes

### Streaming Strategy

- Use `laravel/ai` streaming with `usingVercelDataProtocol()`.
- React consumes SSE and renders:
  - text deltas
  - tool call/result cards
  - final finish

Durability rules:

- The stream is “best effort for UX”.
- The database is the source of truth.
- After stream completion, the UI should be able to refresh and reconstruct the same state from DB.

## Roadmap (Detailed, Ordered)

This is the order we should implement, with explicit scope boundaries so we can ship incremental value.

### Phase 0: Repo/App Bootstrap

**Objective:** create the new Laravel app scaffold and CI that can run in isolation.

1. Create a new Laravel 12 app (in this monorepo under `apps/laravel-web/` or in a new repo; pick one and standardize).
2. Add Inertia + React + TypeScript.
3. Add local dev environment:
   - Postgres (Docker compose)
   - Redis (Docker compose)
4. Add baseline CI + scripts:
   - `composer test` (phpunit)
   - `composer lint` (pint)
   - `npm test` / `npm run lint` for frontend

**Verification:**
- `composer test`
- `composer lint`
- `npm run build`

### Phase 1: Streaming Chat MVP (No Tools Yet)

**Objective:** prove end-to-end streaming chat works with Laravel AI SDK and React UI.

1. Install/configure `laravel/ai`.
2. Implement a minimal agent class (e.g. `AutopilotAgent`) with static instructions.
3. Create a route like `POST /api/chat/stream` that returns SSE via `->stream(...)->usingVercelDataProtocol()`.
4. React UI:
   - a chat page that opens the SSE stream and renders text deltas.
   - a durable “send message” UX with retries.

**Data:**
- Persist thread + messages (final text only) so refresh works.

**Verification:**
- Unit test: stream endpoint returns valid SSE (`data:` lines + `[DONE]`).
- Manual: send a message and watch deltas render.

### Phase 2: Run/Event Log + Tool Card Rendering

**Objective:** reintroduce our auditability invariant and make tool output first-class in the UI.

1. Implement `runs` + `run_events` tables.
2. Add a server-side run orchestrator that:
   - creates a `run`
   - streams the model response
   - appends `run_events` as the stream progresses (bounded)
   - finalizes `run` and writes final assistant message
3. React UI:
   - render tool cards using the Vercel protocol tool events (`tool-input-available`, `tool-output-available`).
   - add a “details” drawer per run that shows:
     - tool call ids
     - receipt hashes
     - cost/usage

4. Port the easiest tools first (parity checks):
   - `get_time`
   - `echo`

**Verification:**
- Programmatic test that a run produces an ordered `run_events` log and is reconstructible after refresh.

### Phase 3: L402 Buyer Tool (MVP)

**Objective:** migrate L402 buying into Laravel and make the EP212 demo path work without explicit approvals.

Tool contract (target shape):

- Name: `lightning_l402_fetch`
- Params:
  - `url`, `method`, `headers`, `body`
  - `maxSpendSats` (hard cap)
  - `scope` (cache key namespace; e.g. `ep212.sats4ai`)
  - optional: `allowHosts` / `denyHosts` policy inputs (server-side allowlist is still recommended)
- Output:
  - `status` (`completed|cached|blocked|failed`)
  - `amountMsats`, `quotedAmountMsats`, `paymentBackend`
  - `proofReference` (e.g. preimage prefix) and receipt hashes
  - response metadata: `responseStatusCode`, `responseContentType`, `responseBytes`, `responseBodyTextPreview`, `responseBodySha256`
  - `cacheHit`

Implementation steps:

1. Implement an L402 HTTP client that:
   - makes the initial request
   - if `402`, parses `WWW-Authenticate: L402 macaroon=..., invoice=...`
   - enforces policy before paying (`maxSpendSats`, allowlist)
   - pays the BOLT11 invoice
   - retries with `Authorization: L402 <macaroon>:<preimage>`
   - captures response body (bounded) + sha256

2. Wallet backend (pick one for MVP):
   - **Spark wallet** (preferred): hold a Spark seed in the server-side DB/secret store; pay invoices; return preimage.
   - LND (optional later).

3. Credential cache:
   - persist `(host, scope) -> macaroon + preimage + created_at + ttl` (bounded)
   - on repeat request, try cached credential first (must not pay again).

4. UI:
   - show a compact “payment sent” line item (host + sats + proof ref prefix + cache hit/miss)
   - keep full receipt JSON behind a collapsible.

**Verification (must be deterministic):**

- Local fake L402 server for tests:
  - request 1: returns 402 + invoice + macaroon
  - request 2 (with auth): returns 200 + premium payload
- Fake wallet payer in tests returns deterministic preimage.
- Tests assert:
  - first call pays and stores receipt + response preview/hash
  - second call uses cache and does not pay
  - over-cap blocks pre-payment

### Phase 4: External Demo Endpoints + Internal Gateway

**Objective:** make the demo stable with 2 endpoints:

1. External seller endpoint (e.g. sats4ai)
2. Our own endpoint behind our gateway (Aperture) when we’re ready

For the Laravel rebuild, we should treat these as configurations/presets:

- `EP212_ENDPOINT_A` (external)
- `EP212_ENDPOINT_B` (internal)

**Verification:**
- A scripted “demo runner” that executes both endpoints and asserts:
  - endpoint A paid success
  - endpoint A repeat cache hit
  - endpoint B blocked or paid depending on config

### Phase 5: Parity Expansion (After MVP)

1. Port remaining core tools that should remain “tool-shaped” (not just app features).
2. Reintroduce optional approvals (only if we need them for safety; not part of MVP).
3. Build admin pages for:
   - wallet status/balance
   - payments list
   - L402 cache state
   - run receipts

### Phase 6: Cutover Strategy

1. Run Laravel app on a staging domain.
2. Mirror key flows (auth + chat + L402).
3. Gradually route traffic or switch DNS.

## Notes on Skipping Approvals (MVP)

Skipping approvals is feasible if we enforce:

- hard `maxSpendSats` caps
- allowlist/denylist on hosts
- bounded response storage
- receipts + logs so we can audit what happened

If we later reintroduce approvals, it should be treated as a UX addition on top of the same tool contract, not a rewrite.

## Risks / Open Questions

- Streaming reliability depends heavily on infra buffering (nginx/fpm/Cloud Run). We should pick a deployment mode that supports SSE well.
- Spark wallet integration in PHP may require:
  - a separate wallet service, or
  - calling an existing SDK via another runtime.
- We must not regress our “tools are auditable” invariant; `run_events` needs to exist early, not as an afterthought.

## Decision Log

- 2026-02-15: Commit to **Laravel + Inertia + React (TS)** and adopt `laravel/ai` streaming protocol for chat.
- 2026-02-15: MVP will **skip approvals**; safety enforced via strict tool caps/allowlists.
