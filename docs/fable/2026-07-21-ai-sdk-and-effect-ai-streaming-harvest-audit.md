# AI SDK And Effect AI — Streaming Harvest Audit For The Live-To-UI Path

**Date:** 2026-07-21
**Lane:** Fable strategy analysis
**Status:** Analysis and evidence survey only. This document flips no promise
state, changes no runtime authority, mints no issue, and dispatches no work.
Factual status authorities remain current code, `docs/sol/MASTER_ROADMAP.md`
(revision 126), live issue state, contracts, and receipts. Proposal packets
named below require Sol admission and owner acceptance before any dispatch.
**Directory choice:** This file sits in `docs/fable/` beside its companion
harness-harvest analysis, because it is ranked strategy and recommendation
work, not a fresh point-in-time teardown. The upstream teardown of record
stays `docs/teardowns/2026-07-17-ai-sdk-v7-harnesses-teardown.md`.
**Companions:**
[`2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`](./2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md)
(the harness-layer harvest that produced `packages/agent-harness-contract`) and
[`../desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md`](../desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md)
(the full map of the runtime these recommendations land in).

**Sources (read as real code, ideas re-derived, no code vendored):**
- Vercel AI SDK `main` at commit `6b6a8bbe92`, read-only reference clone
  `~/work/projects/repos/ai` (paths below are `packages/ai/src/...` and
  `packages/provider/src/...` under that clone).
- Effect `4.0.0-beta.94`, source under
  `node_modules/.pnpm/effect@4.0.0-beta.94/.../effect/src/unstable/ai/` and
  the core `effect/src/ExecutionPlan.ts`.
- OpenAgents shipped baseline: `packages/agent-harness-contract` (HARN-01…
  HARN-08, epic #9115), `packages/agent-runtime-schema`
  (`KhalaRuntimeEvent`, `KhalaRuntimeAiSdkTextStreamPart`),
  `packages/khala-ai-sdk-core`, `packages/harness-conformance`,
  `apps/openagents-desktop/src/claude-local-contract.ts`
  (`ClaudeLocalEvent`).
- Companion strategy:
  [`2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`](./2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md),
  [`../sol/2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md`](../sol/2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md),
  [`../sol/2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md`](../sol/2026-07-18-electron-ai-sdk-codex-claude-full-auto-rewrite-roadmap.md).

---

## 0. Purpose and the one-sentence finding

The owner pain that drives this audit is direct — nothing streams to the UI.
The prior harvest took the AI SDK harness layer (session verbs, suspend and
continue, sandbox seam) and OpenAgents shipped it as
`packages/agent-harness-contract`. That work gave the repo a durable,
cursor-exact runtime stream. It did not give the repo a first-class projection
from that stream to a live renderer.

The one-sentence finding: **the missing layer is not the model call and not
the runtime — Effect already owns the model call
(`effect/unstable/ai`), and OpenAgents already owns the runtime and the
durable event log. The missing layer is the UI stream contract that turns a
neutral `KhalaRuntimeEvent` stream into progressively-reconstructed messages
on a renderer, and the AI SDK is the one source studied here that has a
complete, shipped version of exactly that layer.**

Effect AI stops at an Effect `Stream` of typed response parts. The AI SDK
continues past that point through a wire chunk protocol, a client reducer, a
transport abstraction, and a resume path. That continuation is the harvest.

---

## 1. What OpenAgents already shipped (the baseline)

`packages/agent-harness-contract` is the Effect port of the AI SDK `HarnessV1`
shape. It owns one versioned adapter contract (`AgentHarness`), the session
verbs `promptTurn / continueTurn / suspendTurn / compact / detach / stop /
destroy`, capability-by-method-presence with a typed
`HarnessCapabilityUnsupported` refusal, a durable seq-cursor event log with
`appendEvent / replay / attach / markRerunBoundary` (HARN-02), an intra-turn
slice runner (HARN-06), a unified readiness projection (HARN-05), a sandbox
provider contract with an in-memory double and a real local-process provider
(HARN-07), and ACP and opencode adapters (HARN-04, HARN-08). Crucially, its
stream event **is** `KhalaRuntimeEvent` — there is no second event union, and
the `sequence` field is the durable replay cursor. This audit does not
re-recommend any of that. It recommends the projection layer that sits above
it and the generic model substrate that sits below it.

---

## 2. The AI SDK streaming stack — the four-vocabulary pipeline

The AI SDK does not have one stream type. It has an ordered pipeline of four
vocabularies, and each boundary is a real, testable transform. This structure
is the most important single idea in the AI SDK for the live-to-UI problem,
because OpenAgents today owns the first vocabulary and none of the other
three.

| Stage | Vocabulary | AI SDK path | Job |
| --- | --- | --- | --- |
| 1. Provider | `LanguageModelV4StreamPart` | `packages/provider/src/language-model/v4/language-model-v4-stream-part.ts` | Raw provider deltas — `text-*`, `reasoning-*`, `tool-input-*`, `stream-start`, `response-metadata`, `finish`, `raw`, `error`. No step or lifecycle framing. |
| 2. SDK | `TextStreamPart` (`fullStream`) | `packages/ai/src/generate-text/stream-text-result.ts` | Adds `start`, `abort`, `start-step`, `finish-step` (usage, performance), `tool-call`, `tool-result`, `tool-error`, `tool-output-denied`, approval parts. This is the executor-side truth. |
| 3. Wire | `UIMessageChunk` | `packages/ai/src/ui-message-stream/ui-message-chunks.ts` | The renderer protocol. SSE frames `data: <json>\n\n` ending with `data: [DONE]\n\n`, identified by header `x-vercel-ai-ui-message-stream: v1`. |
| 4. Client | `UIMessagePart` (a `UIMessage`) | `packages/ai/src/ui/ui-messages.ts` and `process-ui-message-stream.ts` | The reconstructed message. A reducer folds chunks into a progressively-more-complete message snapshot. |

The transforms between stages are named and small:

- **Stage 2 to stage 3** is `toUIMessageChunk` and `toUIMessageStream`
  (`packages/ai/src/ui-message-stream/to-ui-message-chunk.ts`,
  `to-ui-message-stream.ts`). Send flags gate what crosses —
  `sendReasoning` (default true), `sendSources` (default false), `sendStart`,
  `sendFinish`. File and reasoning-file parts become `data:` URLs. A
  `tool-call` becomes `tool-input-available`, a `tool-result` becomes
  `tool-output-available`.
- **Stage 3 to stage 4** is `processUIMessageStream`
  (`packages/ai/src/ui/process-ui-message-stream.ts`) and its consumer wrapper
  `readUIMessageStream` (`packages/ai/src/ui-message-stream/read-ui-message-stream.ts`),
  which emits a fresh `structuredClone` of the message on every update — every
  element of the output stream is one more-complete snapshot of the same
  message.

The stage-3 chunk vocabulary carries the whole live surface an agent UI needs.
It has streaming text (`text-start / text-delta / text-end`), streaming
reasoning, a full **tool-call state machine**
(`tool-input-start → tool-input-delta → tool-input-available →
tool-approval-request → tool-output-available | tool-output-error |
tool-output-denied`), sources, files, custom parts, `data-<name>` parts with a
`transient` flag, step boundaries, and an `error` chunk whose text is masked
by an `onError` hook so a server error never leaks to the client.

---

## 3. Harvest catalog — ranked by leverage for the live-to-UI path

Each row states what the abstraction is (with an AI SDK path), why it helps
OpenAgents, the Effect port shape, and whether the repo already has an
equivalent. The ranking is by how directly the item closes the "nothing
streams to the UI" gap.

### S1 — The `processUIMessageStream` reducer and the tool-call state machine (highest leverage)

- **What it is:** a `TransformStream<UIMessageChunk, UIMessageChunk>` that
  holds `StreamingUIMessageState` and, per chunk, mutates a message under a
  serialized job — active text parts by id, active reasoning parts by id,
  partial tool calls re-parsed live with `parsePartialJson`, and the
  input-streaming to output-available tool-call transitions
  (`packages/ai/src/ui/process-ui-message-stream.ts`,
  `packages/ai/src/ui/ui-messages.ts` `UIToolInvocation`).
- **Why it matters:** this is the exact code that turns a raw delta stream into
  a message a renderer can bind to without hand-writing per-lane merge logic.
  OpenAgents hand-writes this today, once per lane, inside the desktop
  `ClaudeLocalEvent` projection and its renderer. The tool-call state machine
  in particular is what makes a tool call render as "running → input ready →
  awaiting approval → result" rather than as opaque events.
- **Effect port shape:** an Effect reducer `Stream<KhalaRuntimeEvent> →
  Stream<UiMessageSnapshot>` built on `Stream.mapAccum` (stateful fold) with
  the live message held in a `SubscriptionRef` so the renderer reads the
  current value and a change stream from one source. Tool-call state is an
  Effect Schema discriminated union on `state`, decoded fail-closed at the
  boundary.
- **Do we have it?** No. `packages/khala-ai-sdk-core` reduces the AI SDK
  `fullStream` **into** `KhalaRuntimeEvent` (ingestion), and
  `reduceKhalaRuntimeTranscript` folds events into a coarse transcript
  projection, but there is no progressive message reducer with the tool-call
  state machine on the emission side.

### S2 — The `UIMessageChunk` wire protocol and SSE encoding

- **What it is:** the stage-3 chunk union plus `JsonToSseTransformStream`
  (`packages/ai/src/ui-message-stream/json-to-sse-transform-stream.ts`) and
  `createUIMessageStreamResponse`
  (`.../create-ui-message-stream-response.ts`). The wire format is plain SSE
  with a `[DONE]` sentinel and a version header.
- **Why it matters:** the web transcript surface has no streaming path at all
  today. A neutral chunk protocol with a trivial SSE encoding is what lets the
  same stream serve the desktop renderer (over IPC) and a future web
  transcript (over SSE from Cloud Run) without a second vocabulary.
- **Effect port shape:** an Effect Schema `UiMessageChunk` union that is a
  bounded, redaction-aware projection of `KhalaRuntimeEvent`, plus a
  `Stream<UiMessageChunk> → Stream<Uint8Array>` SSE encoder built with Effect
  `Stream`. The encoder is the only new wire artifact — the durable transport
  underneath stays the shipped harness event log.
- **Do we have it?** Partly. `KhalaRuntimeAiSdkTextStreamPart`
  (`packages/agent-runtime-schema/src/index.ts:1376`) already models the AI SDK
  stage-2 vocabulary for ingestion. There is no stage-3 emission chunk and no
  SSE encoder. The desktop uses a bespoke `ClaudeLocalEvent` IPC envelope
  instead (`apps/openagents-desktop/src/claude-local-contract.ts`).

### S3 — The `ChatTransport` abstraction and reconnect/resume

- **What it is:** `ChatTransport` with `sendMessages(...) →
  Promise<ReadableStream<UIMessageChunk>>` and `reconnectToStream(...) →
  Promise<ReadableStream<UIMessageChunk> | null>`
  (`packages/ai/src/ui/chat-transport.ts`). `DirectChatTransport` runs an
  in-process agent with no HTTP and always returns `null` for reconnect
  (`.../direct-chat-transport.ts`). `HttpChatTransport` and
  `DefaultChatTransport` POST to an endpoint and GET a resume stream, with an
  HTTP `204` mapped to "no active stream to resume"
  (`.../http-chat-transport.ts`, `.../default-chat-transport.ts`).
- **Why it matters:** the desktop is exactly the `DirectChatTransport` case
  (in-process, IPC), and the web is exactly the `HttpChatTransport` case. One
  transport interface with a `reconnectToStream` verb is the clean seam for a
  renderer that must survive a reload or an app restart. The `null`-versus-
  stream return is precisely the shipped harness log distinction — no active
  turn versus attach at cursor.
- **Effect port shape:** a `ChatTransport` service tag with `sendMessages:
  (...) => Stream<UiMessageChunk, TransportError>` and `reconnectToStream:
  (...) => Effect<Option<Stream<UiMessageChunk>>>`, where reconnect is the
  harness event log `attach` at the last renderer cursor. Two Layers implement
  it — an in-process desktop Layer over IPC, an HTTP Layer over Cloud Run SSE.
- **Do we have it?** No transport abstraction. The desktop wires IPC channels
  directly (`ClaudeLocalEventChannel`), and the durable resume primitive
  (event log `attach`) exists but is not exposed as a renderer transport.

### S4 — `smoothStream` (readable-pace streaming)

- **What it is:** a `TransformStream<TextStreamPart>` factory that buffers only
  text and reasoning deltas, re-chunks them by word, line, regex, or an
  `Intl.Segmenter`, and emits each chunk with a delay
  (`packages/ai/src/generate-text/smooth-stream.ts`). It preserves provider
  metadata such as Anthropic thinking signatures.
- **Why it matters:** raw provider deltas arrive in ragged bursts. A smoothing
  operator is the difference between text that jitters and text that reads.
  This is cheap and directly improves the perceived quality of the live
  stream.
- **Effect port shape:** an Effect `Stream` operator composed from
  `Stream.throttle` and `Stream.debounce` (both present in Effect v4, see the
  repository `STREAMS.md` reference) plus a word or line re-chunker over the
  text-delta events. It is a pure stream transform with no state authority.
- **Do we have it?** No. Delta pacing is not modeled anywhere.

### S5 — `createUIMessageStream` merge and transient-versus-persisted parts

- **What it is:** `createUIMessageStream({ execute, onError, onEnd })` hands
  `execute` a `UIMessageStreamWriter` with `write` and `merge`, keeps the
  output open until every merged sub-stream drains, and masks errors through
  `onError` (`packages/ai/src/ui-message-stream/create-ui-message-stream.ts`,
  `.../ui-message-stream-writer.ts`). Data parts marked `transient: true` fire
  an `onData` callback and never enter the persisted message
  (`process-ui-message-stream.ts`).
- **Why it matters:** a live agent surface must merge several concurrent
  sources into one message stream — the main turn, a delegate child, a
  host-tool result, a meter tick. The transient flag is the clean way to send
  ephemeral UI signals (progress, live meter, lane notice) that must not
  pollute the durable transcript.
- **Effect port shape:** `Stream.merge` of several
  `Stream<UiMessageChunk>` sources into one renderer stream, and a
  `transient` marker on the chunk that maps to the existing
  `visibility`/`redactionClass` fields on `KhalaRuntimeEvent` — a transient
  chunk is a renderer-only, non-persisted event.
- **Do we have it?** The desktop merges child and meter events into the
  `ClaudeLocalEvent` stream by hand today (`child_activity`, `meter_updated`,
  `lane_notice`). The transient-versus-persisted split is not modeled as a
  first-class stream concern.

### S6 — `streamObject` and `partialObjectStream` (structured live output)

- **What it is:** `streamObject` exposes `partialObjectStream` (progressive,
  explicitly not validated), `elementStream` (array mode), and a `fullStream`
  of `ObjectStreamPart` (`packages/ai/src/generate-object/stream-object.ts`,
  `.../stream-object-result.ts`). Partial JSON is parsed with
  `parsePartialJson`.
- **Why it matters:** Apple FM guided generation and the ProductSpec workroom
  both produce structured output. A partial-object stream lets a structured
  result render as it forms, instead of appearing whole at the end.
- **Effect port shape:** Effect AI already has the terminal half —
  `LanguageModel.generateObject` returns a decoded, schema-validated object.
  The port is the **partial** half — a `Stream` of partial decodes over the
  text-delta stream, guarded so a partial is never treated as a validated
  value.
- **Do we have it?** No partial-object streaming. Apple FM guided generation
  decodes a whole object fail-closed after the fact
  (`packages/apple-fm-runtime/src/recommendation.ts`).

### S7 — Agent loop control — `stopWhen` and `prepareStep`

- **What it is:** `ToolLoopAgent` loops until a finish reason is not
  `tool-calls`, a tool lacks an executor, a tool needs approval, or a
  `stopWhen` condition fires (`isStepCount`, `hasToolCall`) — with
  `prepareStep` able to override model, tools, tool choice, instructions, and
  messages per step (`packages/ai/src/agent/tool-loop-agent.ts`,
  `packages/ai/src/generate-text/stop-condition.ts`, `.../prepare-step.ts`).
- **Why it matters:** this is the single-call multi-step loop, which is a
  smaller and different thing from Full Auto turn orchestration. It is the
  right shape for a bounded model-call lane (Apple FM, a generic model turn),
  not for the coding-agent runtimes.
- **Effect port shape:** none needed as a new port — this maps onto Effect AI
  tool resolution plus the harness slice runner. Recorded here so the two loops
  are not conflated. Full Auto keeps its durable turn authority.
- **Do we have it?** Full Auto owns turn-granular orchestration. The
  intra-call step loop is Effect AI's job below the harness contract.

### S8 — Provider spec, provider-defined tools, provider metadata

- **What it is:** `LanguageModelV4` `doStream`, provider-defined tools
  (`packages/provider/src/language-model/v4/language-model-v4-provider-tool.ts`),
  and `providerMetadata` threaded through every part.
- **Why it matters:** this is where the AI SDK and Effect AI overlap most, and
  it is exactly the layer OpenAgents should **not** re-derive from the AI SDK —
  Effect owns it (section 4).
- **Effect port shape:** adopt Effect AI, do not port.
- **Do we have it?** Effect provides it. See section 4.

---

## 4. Effect AI alignment — adopt, port, or keep

Effect v4 ships a real AI package at `effect/unstable/ai`. The prior conversion
audit already reached this conclusion for the generic layer. This section makes
the boundary precise against the shipped harness contract, and it records the
one place the AI SDK is genuinely additive over Effect.

### 4.1 What Effect AI already models (adopt directly)

- **`LanguageModel`** — a `Context.Service` with `generateText`,
  `generateObject`, and `streamText`. `streamText` returns
  `Stream.Stream<Response.StreamPart<Tools>, AiError, LanguageModel | R>`. A
  program depends only on the `LanguageModel` tag, not on a concrete provider.
- **`Response`** — three Schema unions with `*Encoded` twins. `Part` is the
  coalesced final set. `StreamPart` is the incremental set with `text-start /
  text-delta / text-end`, reasoning deltas, `tool-params-*` deltas, `finish`,
  and an `error` part. This is Effect's equivalent of the AI SDK stage-2
  `TextStreamPart`, and it is fully Schema-encodable, so it can cross any
  transport the application builds.
- **`Model`** — a `Model` **is a `Layer`**. Provider choice is
  `Effect.provide(Model.make("anthropic", "claude-...", anthropicLayer))`.
  Swapping providers is swapping a Layer.
- **`Tool` and `Toolkit`** — schema-typed tools with handlers supplied as a
  Layer, automatic tool-call resolution (or `disableToolCallResolution`),
  `needsApproval`, and `HandlerContext.preliminary` for streamed partial tool
  results.
- **`AiError`** — one umbrella error with eighteen typed reasons, including
  `RateLimitError`, `QuotaExhaustedError`, and `AuthenticationError`. These map
  directly onto the harness-conformance mandatory failure classes
  `account_rate_limited`, `account_exhausted`, and the auth-health class
  (`packages/harness-conformance/src/contract.ts`).
- **`ExecutionPlan`** (core `effect/ExecutionPlan.ts`, applied by
  `Effect.withExecutionPlan` and `Stream.withExecutionPlan`) — an ordered list
  of steps, each supplying a provider Layer, each with its own retry
  `attempts`, `schedule`, and a `while` predicate on the error for conditional
  fallback across providers.
- **`Chat`** — stateful history in a `Ref`, `export` / `exportJson` /
  `fromJson`, and a `Persistence` service.
- **`ResponseIdTracker`** — `previousResponseId` and `incrementalPrompt`, so a
  provider is sent only the messages it has not yet seen.

### 4.2 Where Effect AI sits relative to the harness contract

The two do not compete. They stack.

- **Below the harness contract — the model-call lanes.** A single model call
  (Apple FM local inference, a Khala inference turn, any generic
  provider turn) is a `LanguageModel.streamText`, not a stateful coding-agent
  runtime. These lanes should sit on Effect AI directly. `khala-ai-sdk-core`,
  which today calls the Vercel `streamText`, is the concrete reconciliation
  target — its provider transport can become `LanguageModel.streamText`, while
  it keeps mapping parts into `KhalaRuntimeEvent`.
- **Above Effect AI — the coding-agent runtimes.** Codex, Claude Code, and the
  ACP peers are durable multi-turn runtimes with native history, suspend and
  continue, and their own tool execution. Effect AI `LanguageModel` is a single
  request or stream and cannot express those. The shipped `AgentHarness`
  contract stays the abstraction for them. It may internally use Effect AI for
  a sub-call, but it is not replaced by it.
- **Beside both — `ExecutionPlan` as advisory fallback.** `ExecutionPlan`
  models ordered provider fallback well. Full Auto lane rotation is a candidate
  consumer, but Full Auto keeps its durable authority — leases, the eight-run
  cap, journals, receipts. `ExecutionPlan` may back the retry and fallback
  mechanics inside one lane. It does not choose the next lane and does not hold
  run state.

### 4.3 The one thing Effect AI does not have

Effect AI **stops at `Stream<Response.StreamPart>`**. A whole-directory search
found no `UIMessage`, no data-stream, no SSE encoding, no `text/event-stream`,
and no transport or `useChat` equivalent. The only `Protocol` in the package is
the MCP server built on Effect RPC, which is unrelated to LLM UI streaming.

This is the precise, defensible boundary of the harvest. Effect gives a typed,
encodable `Stream` of response parts. It deliberately leaves the browser-UI
protocol, the client reducer, the transport, and the resume path to the
application. The AI SDK is the one source studied here that has shipped all
four. Therefore: **adopt Effect AI for the model call and the response-part
stream, and port the AI SDK's stage-3 and stage-4 layers (chunk protocol,
reducer, transport, resume) to Effect, fed by the `KhalaRuntimeEvent` stream
the harness contract already emits.**

### 4.4 Keep OpenAgents-native

- `KhalaRuntimeEvent` as the single neutral runtime event union and durable
  cursor. It is a superset of both the AI SDK stage-2 vocabulary and the Effect
  `StreamPart` vocabulary, and it carries `visibility`, `redactionClass`, and
  `causalityRefs` that neither upstream has.
- The shipped harness durable event log (attach, replay, rerun boundaries).
  This is stronger than the AI SDK best-effort per-turn bridge log and stronger
  than anything Effect AI provides.
- The Apple FM guided-generation router and the advisory-recommendation to
  authoritative-decision split.
- Full Auto durable authority, account custody, and the exact usage ledger.

---

## 5. Streaming deep-dive — a clean Effect streaming contract for the live UI

This section states, end to end, how the live stream should flow, tied to the
shipped harness contract and the desktop `ClaudeLocalEvent` envelope.

### 5.1 The current reality

- The **source** already exists. Every lane emits `KhalaRuntimeEvent` with a
  monotonic `sequence`. The harness event log persists it and can `attach` a
  consumer at a cursor with no gap and no duplicate (HARN-02, shipped).
- The **ingestion** side already exists. `KhalaRuntimeAiSdkTextStreamPart` and
  `khala-ai-sdk-core` map AI SDK `fullStream` parts into `KhalaRuntimeEvent`.
- The **emission** side is the gap. The desktop projects events to the renderer
  over the IPC channel `openagents:claude-local:event` with a bespoke,
  bounded, path-redacted `ClaudeLocalEvent` union
  (`text_delta`, `tool_use`, `tool_progress`, `reasoning`, `child_activity`,
  `meter_updated`, and more). That union is hand-mapped per lane, and the web
  transcript has no streaming path at all.

### 5.2 The proposed shape

Four Effect layers, all fed by the one `KhalaRuntimeEvent` stream:

1. **Projection** — `KhalaRuntimeEvent → UiMessageChunk`. A pure, redaction-
   aware Effect Schema projection. It is the emission twin of the existing
   `KhalaRuntimeAiSdkTextStreamPart` ingestion type. Send-flag gating
   (reasoning, sources, start, finish) is a decode-time option. The transient
   flag maps to `visibility`.
2. **Pace** — an optional `smoothStream`-equivalent Effect `Stream` operator
   (`Stream.throttle` plus word or line re-chunking) applied to text and
   reasoning deltas only.
3. **Reduce** — `Stream<UiMessageChunk> → SubscriptionRef<UiMessage>`. A
   `Stream.mapAccum` fold that reconstructs progressive message snapshots and
   runs the tool-call state machine, held in a `SubscriptionRef` so the
   renderer reads the current value and the change stream from one source.
4. **Transport** — a `ChatTransport` service with two Layers. The desktop Layer
   carries `UiMessageChunk` over the existing IPC channel, so
   `ClaudeLocalEvent` becomes **one projection of** this stream rather than a
   second source of truth. The web Layer SSE-encodes the same chunk stream from
   Cloud Run. Both back `reconnectToStream` with the harness event log
   `attach` at the last renderer cursor — the `null`-versus-stream return of
   the AI SDK maps exactly onto "no active turn" versus "attach at cursor N".

The result is one contract from the neutral runtime event to the rendered
message, with the durable resume already solved by the shipped event log, and
with the desktop and web renderers as two Layers over one vocabulary. This is
the concrete answer to "nothing streams to the UI" — the stream exists and is
durable, and this contract is the projection it lacks.

### 5.3 A note on a superseded mechanism

`docs/research/2026-06-23-effect-durable-streams-on-do-audit.md` proposed
offset-addressed resumable streams on Cloudflare Durable Objects. Cloudflare
Workers and Durable Objects are retired for OpenAgents (Google Cloud is the
sole production infrastructure authority). The resumable-cursor idea it
describes is already realized by the shipped harness event log. The transport
Layer above encodes that same cursor over IPC or Cloud Run SSE, not over a
retired Durable Object.

---

## 6. Recommendations — ranked proposal-packet candidates

These are candidates. Each requires Sol admission and owner acceptance before
any dispatch. Each is "port to Effect" — no vendoring, ideas re-derived. Names
are provisional. They compose with the existing HARN epic (#9115), and they do
not weaken any Full Auto guardrail or the fail-closed routing posture.

- **STREAM-01 — Adopt `effect/unstable/ai` as the model-call substrate.**
  Reconcile `khala-ai-sdk-core` and the Apple FM inference path toward
  Effect AI `LanguageModel.streamText` and `generateObject`, keeping the map
  into `KhalaRuntimeEvent`. Map `AiError` reasons onto the mandatory
  harness-conformance failure classes. Pure below-the-harness work. No renderer
  change. Highest confidence, lowest risk — the repo already stated this
  posture.
- **STREAM-02 — Port the `UiMessageChunk` projection and the progressive
  reducer.** The emission twin of `KhalaRuntimeAiSdkTextStreamPart`, plus the
  `SubscriptionRef`-backed reducer with the tool-call state machine (S1, S2).
  Contract and conformance tests only. This is the core of the live-to-UI
  harvest.
- **STREAM-03 — An Effect `ChatTransport` contract over the harness event
  log.** `sendMessages → Stream`, `reconnectToStream → attach at cursor or
  none` (S3). A desktop IPC Layer and a web SSE Layer. `ClaudeLocalEvent`
  becomes one projection.
- **STREAM-04 — A `smoothStream` Effect `Stream` operator.** `Stream.throttle`
  plus word or line re-chunking for text and reasoning deltas (S4). Small, pure,
  immediately improves perceived quality.
- **STREAM-05 — Evaluate `ExecutionPlan` for in-lane provider fallback.**
  Advisory only. `Effect.withExecutionPlan` for retry and fallback mechanics
  inside one lane, with Full Auto retaining lease, cap, journal, and receipt
  authority (S7, section 4.2).
- **STREAM-06 — Partial-object streaming for guided output.** A partial-decode
  `Stream` for Apple FM guided generation and the ProductSpec workroom, guarded
  so a partial is never a validated value (S6). Effect AI already owns the
  terminal decode.
- **STREAM-07 — Reconcile harness host-tools with Effect `Toolkit`.** Align the
  shipped harness host-tool schema with Effect `Tool` and `Toolkit`
  (schema-typed, handlers as a Layer, `needsApproval`, `preliminary`), so one
  tool substrate serves both the model-call lanes and the harness adapters
  (S8, section 4.1).

Sequencing note: STREAM-01, STREAM-02, and STREAM-04 are pure additions and can
proceed under normal admission. STREAM-03 touches the live desktop IPC path and
the `ClaudeLocalEvent` behavior contract, so it needs the usual oracle
coverage. STREAM-05 must not move any run-selection or settlement authority out
of Full Auto.

---

## 7. Bottom line

The prior harvest took the AI SDK harness layer and OpenAgents shipped it. This
harvest is narrower and more urgent — it is the layer between the shipped
durable runtime stream and the renderer. Effect AI already owns everything below
that layer, up to a typed, encodable `Stream` of response parts, and OpenAgents
already owns the neutral event union, the durable cursor, and the runtime. The
AI SDK is the one source studied here that ships the missing four stages — chunk
protocol, progressive reducer, transport, and cursor resume — and every one of
them ports cleanly onto contracts the repo already has. Adopt Effect AI for the
model call. Keep `KhalaRuntimeEvent`, the harness contract, the Apple FM router,
and Full Auto authority. Port the AI SDK streaming projection onto them. That is
the path from a durable stream that no one can see to a live message on the
screen.
