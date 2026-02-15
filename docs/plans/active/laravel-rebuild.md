# Laravel Rebuild Evaluation (Laravel AI SDK)

Date: 2026-02-15

## Goal

Evaluate a from-scratch rebuild of OpenAgents' current Effuse/Cloudflare/Convex web stack into **Laravel 12 + Laravel AI SDK (`laravel/ai`)**, and recommend one of:

1. **Laravel + Inertia + React (TypeScript)**
2. **Laravel + Livewire**

This doc focuses on the *product surface* (openagents.com Autopilot chat + Lightning/L402 UX + admin surfaces), and how the Laravel AI SDK changes the calculus.

## What We Learned From `laravel/ai`

We reviewed `/Users/christopherdavid/code/laravel-ai/`.

Key capabilities that matter for OpenAgents:

- **Unified provider abstraction** (OpenAI, Anthropic, Gemini, OpenRouter, etc.) with failover.
- **Agents with tools**: agents implement interfaces like `HasTools`, `Conversational`, `HasStructuredOutput`.
- **Structured output** via Laravel JSON schema builder (`illuminate/json-schema`).
- **Streaming**: `StreamableAgentResponse` can stream SSE, including a mode that speaks the **Vercel AI SDK data stream protocol**.
  - See `Laravel\\Ai\\Responses\\StreamableAgentResponse::usingVercelDataProtocol()`.
  - Implementation: `Laravel\\Ai\\Responses\\Concerns\\CanStreamUsingVercelProtocol`.
  - It emits SSE `data:` lines with `type: start|text-delta|tool-input-available|tool-output-available|finish` and terminates with `data: [DONE]`.
- **Broadcasting streaming events** to channels (queue-backed or immediate). This is a natural fit for WebSockets (Laravel Reverb / Pusher style).
- **Conversation persistence**: a built-in DB conversation store exists (`DatabaseConversationStore`) and migrations create:
  - `agent_conversations`
  - `agent_conversation_messages` (stores: role/content/tool_calls/tool_results/usage/meta)

Important limitation for OpenAgents:

- The built-in conversation store captures the **final** assistant content and tool calls/results, but it is *not* a complete “replay log” of stream deltas and runtime decisions. If we keep our “replayable receipts / deterministic audit” invariant, we must implement our own receipt/event log.

## OpenAgents Requirements That Drive The Frontend Choice

Non-negotiable UX/engineering needs (based on current product direction):

- Autopilot chat must support **streaming text**, **tool cards**, **approval gates** (e.g. L402 pay intent), and **retries**.
- We need a UI that can present “rich runs”: tool calls/results, receipts, and human-friendly summaries.
- We will maintain multiple clients:
  - Web (openagents.com)
  - React Native mobile
  - Electron desktop (planned)
- We should retain strong type-safety on the client (or we will regress reliability significantly).

## Option A: Laravel + Inertia + React (TypeScript)

### What It Looks Like

- Laravel serves pages via Inertia.
- React/TS renders the Autopilot chat UI, Lightning panes, admin UI, etc.
- API endpoints (Laravel routes/controllers) provide:
  - chat history (threads/messages)
  - SSE streaming endpoint(s) for runs
  - tool execution endpoints (or internal job dispatch)
  - Lightning wallet/executor endpoints

### Why This Fits `laravel/ai`

Laravel AI SDK explicitly supports the **Vercel AI SDK stream protocol**.

That means we can either:

- Use Vercel AI SDK on the client (React) for parsing the stream, *or*
- Implement our own SSE parser in TS (still easy), but keep protocol compatibility.

Either way, React is the “native” client environment for that protocol.

### How To Map Our Current Autopilot UX

- Replace “Convex subscription → client rebuilds message parts” with:
  - **SSE for the current run** (low-latency deltas)
  - **DB persistence** for messages + a replay log (for refresh/backfill)
  - optional **WebSockets broadcast** for multi-tab observers

- Tool UX:
  - Tools are modeled as `laravel/ai` Tools.
  - We keep a strict schema boundary: input schema validation before execution.
  - Tools emit receipts (see “Receipts and Replay” below).

- Approval UX (e.g. L402):
  - First tool invocation emits a `payment_intent` tool output and transitions the run to `approval_required`.
  - React renders an approve button.
  - Approve triggers a follow-up request that continues the run.

### Pros

- Best fit for streaming chat + rich UI.
- Shared React component strategy across **web + Electron**, and mental model close to React Native.
- TypeScript where we need it (UI correctness).
- Directly aligns with `laravel/ai`’s Vercel protocol mode.

### Cons

- Still a “two-language” system (PHP backend + TS frontend).
- You’ll build/maintain an API surface anyway for RN/Electron.

## Option B: Laravel + Livewire

### What It Looks Like

- Laravel renders Livewire components for most UI.
- Livewire sends incremental updates over its own transport.
- For streaming AI, we would likely bridge in one of two ways:
  1. Use `laravel/ai` streaming + **broadcast events** and subscribe from Livewire via Echo/Reverb.
  2. Poll a “run state” row and append updates (worse UX).

### Pros

- Very fast to build CRUD/admin surfaces.
- Great for forms + simple interactive pages.
- Keeps most logic in PHP.

### Cons (for OpenAgents)

- Streaming chat with tool cards + approval + long-running flows becomes awkward.
- Harder to share code/UX with Electron/RN.
- Client-side type safety is weaker (you will re-learn these bugs).
- We will likely end up building a separate API for RN/Electron anyway, which reduces the value of “all-in server UI”.

## Recommendation

If we do a Laravel rebuild, we should choose:

**Laravel + Inertia + React (TypeScript)**

Reasoning:

- `laravel/ai` is intentionally compatible with the **Vercel AI SDK streaming protocol**, which is best consumed from **React**.
- OpenAgents is trending toward a UI that is not “CRUD-ish”: it’s streaming, tool-heavy, and multi-client.
- React gives us the highest leverage across web + Electron and keeps us closer to the RN mental model.

Livewire remains attractive for an **internal admin** app, but it is the wrong primary UI technology for a streaming agent product.

## Proposed Laravel Architecture (If We Rebuild)

### Runtime Components

- **Laravel App** (primary)
  - Auth (WorkOS or Laravel native)
  - Autopilot threads/runs storage in Postgres
  - Streaming endpoints (SSE)
  - Tool execution orchestration
  - Lightning wallet + executor integration

- **Queue + workers**
  - Redis-backed queue
  - Separate worker pool for:
    - tool execution
    - L402 payments / wallet IO
    - long tasks

- **Realtime**
  - Optional: Laravel Reverb (WebSockets) for observers
  - Not strictly required if we rely on SSE per active run + DB backfill

### Data Model Sketch

- `threads`
- `runs` (status: queued, streaming, awaiting_approval, completed, failed, canceled)
- `messages` (user/assistant)
- `run_events` (the replay log / receipt log)
  - append-only
  - includes: text deltas (optional), tool call + tool result, approvals, errors
  - references hashes and bounded payload previews

### Receipts and Replay (Carry Over Our Invariant)

`laravel/ai` already gives:

- tool call/result content
- a final assistant message

But we need:

- deterministic receipts
- bounded payload hashing
- ability to reproduce runs

Implementation approach:

- Subscribe to Laravel AI SDK streaming events (or wrap the stream generator) and persist `run_events`:
  - `text-delta` events (optional; can store only final text + tool events if storage is too costly)
  - `tool-input-available` / `tool-output-available`
  - `finish` usage
- Store:
  - `params_hash`, `output_hash`, `latency_ms`, `side_effects`
  - “counterfactuals” when migrating from legacy paths

## Rebuild Plan (High Level)

1. **Spike**: new Laravel app that can stream a trivial agent response using `usingVercelDataProtocol()`.
2. **Chat persistence**: threads/messages in Postgres; load history UI.
3. **Tool system**: implement 2-3 tools with schemas and tool cards.
4. **Approval gating**: implement an “approval required” tool loop.
5. **Lightning / L402**: port or call existing L402 buyer capability behind a tool contract.
6. **Receipts**: build `run_events` append-only log and a replay/export command.
7. **Parity**: migrate remaining product surfaces (admin, ops).
8. **Cutover**: ship on a new domain/staging, then switch traffic.

## Risks

- Large rewrite will stall product work unless we timebox a spike and prove it increases velocity.
- We may lose key properties of the current system if we do not re-introduce:
  - strict tool schemas
  - deterministic receipts
  - replay bundles
- Laravel AI SDK is new and targets Laravel 12 / PHP 8.4; the ecosystem may shift quickly.

## Decision Log

- 2026-02-15: Based on `laravel/ai`’s explicit support for the Vercel AI SDK data stream protocol and our multi-client, tool-heavy UI requirements, prefer **Laravel + Inertia + React (TS)** over Livewire for a rebuild.
