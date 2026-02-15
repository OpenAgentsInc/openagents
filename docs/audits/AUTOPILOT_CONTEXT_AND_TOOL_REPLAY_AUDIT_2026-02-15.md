# Autopilot Context + Tool Replay Audit (2026-02-15)

This audit documents, precisely and with code references, what context we pass into the model for `openagents.com` Autopilot chat, and how tool calls/results are stored and (not) replayed into subsequent model calls.

## Executive Summary

- The **model prompt context is built from `messages.text` only** (Convex `messages` table). We intentionally **do not load `messageParts`** (tool calls/results, streaming deltas, DSE parts) into the model prompt.
- Tool calls/results are recorded as **`messageParts` rows** and rendered in the UI, but **they are not replayed back into the model** in a structured way.
- The only way a tool outcome affects subsequent model behavior is if the Worker **writes a human-readable summary into the assistant’s durable `messages.text`** when finalizing the run.
- This differs from “typical tool-calling chat apps”, where the model sees prior tool calls and tool outputs as first-class context messages.

## Scope

- In scope: `openagents.com` web chat runtime in `apps/web` (Cloudflare Worker + Convex).
- Out of scope: `apps/autopilot-worker` DO runtime (separate tool host). This audit focuses on what’s running for the public web app chat.

## Terminology / Data Model

### Convex tables involved

All of this lives under `apps/web/convex/autopilot/messages.ts`:

- `messages` (durable): one row per user/assistant message.
  - Contains: `messageId`, `role`, `status`, and (critically) `text`.
- `runs` (durable): one row per assistant generation.
  - Contains: `runId`, `assistantMessageId`, `status`, `cancelRequested`.
- `messageParts` (durable append-only): streaming parts, tool parts, DSE parts.
  - Contains: `runId`, `messageId`, `seq`, and opaque `part` objects.

Code reference:
- Snapshot schema + queries: `apps/web/convex/autopilot/messages.ts:17-96`
- Run creation: `apps/web/convex/autopilot/messages.ts:98-165`
- Parts append: `apps/web/convex/autopilot/messages.ts:221-297`
- Run finalization: `apps/web/convex/autopilot/messages.ts:299-371`

### “Message parts” are where tool calls/results live

A tool execution is not stored as a separate “tool message”. Instead, it is emitted as a `messageParts.part` payload.

Example part shapes (not exhaustive):

- `AiResponse.StreamPartEncoded` (streaming UI): `text-start`, `text-delta`, `text-end`, `error`, `finish`.
- DSE instrumentation parts (UI + receipts): `dse.tool`, `dse.signature`, `dse.compile`, etc.

These parts are visible in the UI and/or in admin traces, but are not fed back to the model prompt today.

## End-to-End Flow

### 1) Client sends a message

Client-side `ChatService.send` performs an HTTP POST to `/api/autopilot/send` with the text and thread id:

- `apps/web/src/effect/chat.ts:488-512`

Key point: this is not a direct model call from the browser. The Worker runs the model and streams state through Convex.

### 2) Worker creates the run + placeholder assistant message

Worker handler for `/api/autopilot/send`:

- `apps/web/src/effuse-host/autopilot.ts:3969-4042`

It calls Convex `createRun`, which inserts:

- a final user `messages` row (with `text`)
- a streaming assistant `messages` row (empty `text`, but `runId` set)
- a streaming `runs` row

See:
- `apps/web/convex/autopilot/messages.ts:98-150`

### 3) Worker starts the streaming job

The Worker uses `ctx.waitUntil(runAutopilotStream(...))`:

- `apps/web/src/effuse-host/autopilot.ts:4026-4036`

The important implication: **streaming is best-effort** (Cloudflare isolates can be evicted). This is why we have a stale-run finalizer on the Convex side.

### 4) Worker loads prompt context (messages only)

Inside `runAutopilotStream`, the Worker queries Convex for the thread snapshot with `maxParts: 0`:

- `apps/web/src/effuse-host/autopilot.ts:2621-2634`

This is the core decision:

- We load **only `messages` rows**.
- We load **zero `messageParts`**.

This means **tool calls/results from prior turns are not present** when building the prompt.

### 5) Worker builds the model prompt from text-only history

The Worker maps Convex message rows to `(role, text)` and builds a prompt:

- Messages extracted: `apps/web/src/effuse-host/autopilot.ts:2664-2671`
- System prompt + bootstrap system additions: `apps/web/src/effuse-host/autopilot.ts:1170-1210`

`concatTextFromPromptMessages(...)` creates an `AiPrompt.RawInput` with:

- One `system` message: hardcoded “You are Autopilot …” + bootstrap additions
- Then each prior message as `user`/`assistant`, with `content: [{ type: "text", text: ... }]`

Critically:

- No `messageParts` are ever considered.
- No tool-call JSON, no tool-result payloads, no structured receipts.

### 6) Worker writes streaming parts to Convex

During model streaming (and also for tool routing), the Worker appends `messageParts` batches via:

- `apps/web/src/effuse-host/autopilot.ts:2591-2606` → `appendParts` mutation

Those rows are rendered by the UI (see next section).

### 7) Worker finalizes the run (durable `messages.text`)

At end-of-run, the Worker finalizes the run and writes `messages.text`:

- Worker: `apps/web/src/effuse-host/autopilot.ts` (finalize happens near the end of `runAutopilotStream`, after streaming)
- Convex: `apps/web/convex/autopilot/messages.ts:299-371`

This `messages.text` is the **only** part of the assistant run that is guaranteed to be used later as model context.

### 8) UI subscribes to both messages and messageParts

The UI is not “streamed from the model”; it is “streamed from Convex”.

- `ChatService.open` subscribes to `getThreadSnapshot`:
  - `apps/web/src/effect/chat.ts:253` (subscribeQuery)
- It rebuilds a deterministic message structure:
  - parses `messages` + `parts`
  - applies `applyChatWirePart(...)` to reconstruct a per-message active stream

Key detail:

- We recently added a guard so the UI won’t show an empty assistant bubble when we have a finalized `messages.text`:
  - `apps/web/src/effect/chat.ts:290-337`

## Tool Calls and Results: Storage vs Prompt Replay

### How tool executions are represented

When the Worker runs a tool (example: L402), it emits a `dse.tool` part into `messageParts`.

Reference (tool routing path in `runAutopilotStream`):

- Tool start part: `apps/web/src/effuse-host/autopilot.ts:2673-2691`

Then, depending on success/failure/approval-needed, it emits another `dse.tool` part with `state=ok|error|approval-requested` plus output payload.

### How tool executions become visible to the user

The UI converts tool parts into “cards” via:

- `apps/web/src/effuse-app/controllers/autopilotChatParts.ts`

For Lightning L402, we intentionally hide the raw tool payload card by default and render a `payment-state` card:

- `apps/web/src/effuse-app/controllers/autopilotChatParts.ts:503-543`

The payment-state card is rendered by:

- `apps/web/src/effuse-pages/autopilot.ts` → `renderPaymentStateCard`

### What the model sees on the next turn

The model sees **none** of the above tool parts.

On the next request, we build prompt context from:

- Convex `messages` (role + text)
- plus the hardcoded system prompt

Because we query `getThreadSnapshot` with `maxParts: 0`:

- `apps/web/src/effuse-host/autopilot.ts:2621-2628`

So unless the tool outcome is reflected into the assistant’s durable `messages.text`, the model has no memory of:

- tool names
- tool inputs/outputs
- tool receipts
- task IDs
- approvals
- payment proofs

### How tool outcomes currently influence future turns

Only via the **assistant text** we choose to write.

For Lightning L402 specifically:

- The tool execution sets `outputText` to a human-readable string derived from the terminal state.
- That `outputText` is emitted as a `text-delta` part (for UI) and written as `messages.text` (durable).

So “tool replay” today is effectively:

- **Tool result summary as plain English text**

not:

- structured tool messages in the prompt.

## How This Differs From Typical Tool-Calling Chat Apps

Typical tool-calling chat architectures do something like:

- include prior assistant tool calls (`{"tool_calls": ...}`) in context
- include tool outputs as `role=tool` messages
- let the model reason over tool outputs and decide next actions

Our current architecture:

- mostly does not use model-native tool calling
- uses deterministic routing for certain commands (e.g. L402 flows)
- stores tool data in `messageParts` for UI/receipts
- but **does not replay tool data into the model prompt**

That is the concrete, code-backed answer to “are we passing tool calls/results back to the AI?”

## Practical Consequences (Why This Matters)

1. If the assistant’s durable `messages.text` is too terse (or empty due to a streaming failure), the model cannot reliably answer:
   - “what just happened?”
   - “what’s the task id?”
   - “did we already pay?”

2. Any “multi-step” flows that depend on hidden tool state will be brittle unless we explicitly re-surface that state in plain text.

3. It becomes easy for the model to appear “amnesiac” relative to the UI (the UI shows tool cards; the model does not see them).

## Recommendations (Minimal + Safe)

If we want the model to behave more like tool-calling chat apps *without* blowing up context size or leaking secrets, the safest path is to inject a **bounded, deterministic tool recap** into the system prompt.

Suggested approach:

- Query `getThreadSnapshot` with a small `maxParts` (e.g. last 200 parts) *only on the Worker side*.
- Extract just the last N tool events (especially terminal statuses and pending approvals).
- Serialize a recap as a short, stable string and append to `extraSystem` passed into `concatTextFromPromptMessages`.

This would keep:

- runtime deterministic
- context bounded
- secrets out (we choose what to include)

while enabling the model to reason over prior tool outcomes.

