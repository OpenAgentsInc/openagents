# Autopilot Context + Tool Replay Audit (2026-02-15)

This audit documents, precisely and with code references, what context we pass into the model for `openagents.com` Autopilot chat, how tool calls/results are stored, and how tool calls/results are replayed into subsequent model calls.

It also compares our flow to the Vercel AI SDK (`/Users/christopherdavid/code/ai`) `useChat` architecture, since that is the “normal chat app” baseline.

## Executive Summary

- Autopilot chat state is stored durably in Convex as:
  - `messages` (durable user/assistant rows) and
  - `messageParts` (append-only streaming parts, tool parts, DSE parts).
  - See: `apps/web/convex/autopilot/messages.ts`.

- The model prompt is still built from `messages.text` history (plus a system prompt and bootstrap additions):
  - `apps/web/src/effuse-host/autopilot.ts:1170-1210` (`concatTextFromPromptMessages`)
  - `apps/web/src/effuse-host/autopilot.ts` (prompt assembly in `runAutopilotStream`)

- **Tool calls/results are now replayed into model context** via a bounded, redacted “tool replay” block:
  - Worker queries `getRunPartsHead` for recent runIds in-context.
  - Worker extracts + redacts recent `dse.tool` parts, then injects them into the system prompt as `extraSystem`.
  - Code:
    - Convex query: `apps/web/convex/autopilot/messages.ts` (`getRunPartsHead`)
    - Worker injection: `apps/web/src/effuse-host/autopilot.ts` (tool replay block in `runAutopilotStream`)
    - Redaction + rendering: `apps/web/src/effuse-host/toolReplay.ts`

- This is a pragmatic approximation of the Vercel AI SDK approach (where tool call/results are first-class message parts passed each turn), while keeping:
  - context bounded,
  - secrets scrubbed,
  - and Convex as the source of truth.

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
  - Contains: `threadId`, `runId`, `messageId`, `seq`, `part`.

Code reference:
- Snapshot schema + queries: `apps/web/convex/autopilot/messages.ts:17-96`
- Run creation: `apps/web/convex/autopilot/messages.ts:98-165`
- Parts append: `apps/web/convex/autopilot/messages.ts:221-297`
- Run finalization: `apps/web/convex/autopilot/messages.ts:299-371`

### “Message parts” are where tool calls/results live

A tool execution is not stored as a separate “tool message”. Instead, it is emitted as a `messageParts.part` payload.

Example part shapes (not exhaustive):

- `AiResponse.StreamPartEncoded` (streaming UI): `text-start`, `text-delta`, `text-end`, `error`, `finish`.
- DSE instrumentation parts: `dse.tool`, `dse.signature`, `dse.compile`, etc.

## End-to-End Flow (Current)

### 1) Client sends a message

Client-side `ChatService.send` performs an HTTP POST to `/api/autopilot/send` with the text and thread id:

- `apps/web/src/effect/chat.ts:488-545`

Key point: the browser does **not** call the model directly. The Worker runs the model and streams state through Convex.

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

Implication: streaming is best-effort (Cloudflare isolates can be evicted). Convex has a stale-run guard that can finalize stuck runs.

### 4) Worker loads prompt context

Inside `runAutopilotStream`, the Worker loads **messages only** using `getThreadSnapshot(maxParts: 0)`:

- `apps/web/src/effuse-host/autopilot.ts` (snapshot load in `runAutopilotStream`)

This yields `messagesRaw` with `(role, text, runId, status)`.

### 5) Tool replay injection (new behavior)

To behave more like a normal tool-calling chat app, we replay recent tool calls/results into model context.

Mechanics:

1. Worker extracts the in-context runIds from the last `MAX_CONTEXT_MESSAGES` assistant messages.
2. Worker queries Convex `getRunPartsHead` (by runId, ordered by seq) to fetch only the **head** of each run’s parts.
   - Why head: tool parts are emitted early in the run, and we do not want to pull the full token stream history.
3. Worker filters to `dse.tool` parts, redacts sensitive keys (invoice/macaroon/preimage/auth/cookies/seeds/etc.), clamps previews, and renders a bounded summary string.
4. Worker injects that summary into the system prompt via `extraSystem` so the model sees tool outcomes.

Code:
- Convex query: `apps/web/convex/autopilot/messages.ts` (`getRunPartsHead`)
- Worker injection: `apps/web/src/effuse-host/autopilot.ts` (tool replay block)
- Renderer/redactor: `apps/web/src/effuse-host/toolReplay.ts` (`renderToolReplaySystemContext`)

Security posture:
- We **do not** feed raw tool outputs into the model.
- We feed only a **redacted and bounded** summary.

### 6) Worker builds the model prompt

The Worker maps message rows to `(role, text)` and builds the prompt:

- System prompt + bootstrap additions: `apps/web/src/effuse-host/autopilot.ts:1170-1210`

`concatTextFromPromptMessages(...)` creates an `AiPrompt.RawInput` with:

- One `system` message: hardcoded “You are Autopilot …” + bootstrap additions + optional `extraSystem` (including tool replay).
- Then each prior message as `user`/`assistant`, with `content: [{ type: "text", text: ... }]`.

### 7) Worker writes streaming parts to Convex

During model streaming (and also during deterministic tool routing), the Worker appends `messageParts` via:

- `apps/web/src/effuse-host/autopilot.ts` → `appendParts` mutation

### 8) Worker finalizes the run (durable `messages.text`)

At end-of-run, the Worker finalizes the run and writes `messages.text`:

- Worker: `apps/web/src/effuse-host/autopilot.ts` (finalize in `runAutopilotStream`)
- Convex: `apps/web/convex/autopilot/messages.ts:299-371`

Even with tool replay, `messages.text` remains the canonical durable transcript.

### 9) UI subscribes to both messages and messageParts

The UI is not streamed directly from the model; it is rebuilt deterministically from Convex.

- Snapshot subscription: `apps/web/src/effect/chat.ts:253` (subscribeQuery)
- Deterministic rebuild: `apps/web/src/effect/chat.ts` (apply wire parts to reconstruct message state)

## Comparison: Vercel AI SDK (`/Users/christopherdavid/code/ai`) “Normal Chat App” Behavior

This section documents what the Vercel AI SDK does so we can explicitly compare.

### Client (`useChat`) maintains + sends full message history

Files:

- Hook: `/Users/christopherdavid/code/ai/packages/react/src/use-chat.ts`
- Core chat state: `/Users/christopherdavid/code/ai/packages/ai/src/ui/chat.ts`
- HTTP transport: `/Users/christopherdavid/code/ai/packages/ai/src/ui/http-chat-transport.ts`
- UI message schema: `/Users/christopherdavid/code/ai/packages/ai/src/ui/ui-messages.ts`

Key behavior:

- Client maintains `messages: UIMessage[]` where each message has `parts`.
- Tool calls/results and approvals are first-class parts inside the message.
- Each request POSTs something like:
  - `{ id, messages, ... }`
- Server replies as a JSON event stream of `UIMessageChunk`s.

### What “tool replay” means in AI SDK

In AI SDK:

- The server receives the full message list including:
  - prior tool calls
  - prior tool outputs
  - approvals

So the model can reliably answer:

- “what just happened?”
- “did we already pay?”
- “what was the tool output?”

without relying on the assistant having re-stated the tool result in plain text.

### How OpenAgents differs (and what we mirrored)

- We do not use Vercel AI SDK transport today in `apps/web`; we use `@effect/ai` for inference and Convex for state.
- Our client POST body is `{ threadId, text }`:
  - `apps/web/src/effect/chat.ts:502-511`
- Historically, our Worker prompt context was built from `messages.text` only.

What we mirrored:

- We now replay tool calls/results into model context each turn.
- Instead of sending full `UIMessage[]` from the browser, we:
  - treat Convex as the canonical history store
  - query a bounded subset of run head parts
  - inject a redacted summary into the system prompt

This gives the model the “memory” it needs, without pulling full token delta history or risking secret exfiltration.

## Practical Consequences / Why This Matters

1. The model can now reliably answer questions about prior tool outcomes even when:
   - tool output wasn’t written verbatim into assistant durable text
   - the UI showed a tool card but the assistant text stayed terse

2. Multi-step flows (approval → pay → fetch → summarize) become less brittle because the model has the prior tool states.

3. We can iteratively make tool replay richer (more tool-specific summaries) without changing the storage model.

## Follow-Ups / Next Improvements

- Convert tool replay from a system-block string into a more structured “tool message” representation (closer to AI SDK), while keeping redaction.
- Add per-tool summary renderers for additional tools beyond Lightning (today, Lightning is the main specialized renderer).
- If we adopt the Vercel AI SDK client transport later, we can still keep Convex as canonical store; but it would be a larger architecture shift.
