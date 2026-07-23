# @openagentsinc/agent-harness-contract

> **Layers L2–L5 — durable log, sandbox, harness, UI stream** · part of the [OpenAgents AI SDK](../../docs/README.md)

The Effect-native **agent harness contract**. One versioned adapter contract
drives a third-party coding-agent runtime (Codex, Claude Code, an ACP peer, a
managed sandbox) behind a uniform surface with durable, cursor-exact turn
suspend and continue. It carries the SDK core: the L2 durable event log, the L3
sandbox contract, the L4 `AgentHarness` adapter contract, and the L5 UI message
stream. The harness stream event IS the neutral `KhalaRuntimeEvent` — there is
no new event union.

It is the OpenAgents port of the Vercel AI SDK `HarnessV1` shape. The ideas are
re-derived. No upstream code is vendored, and there is no runtime dependency on
`@ai-sdk/harness`. This package is HARN-01 of the HARN epic (#9115).

## Install

```sh
npm install @openagentsinc/agent-harness-contract@rc
# or via the umbrella (subpaths ./harness, ./event-log, ./ui-stream, ./sandbox):
npm install @openagentsinc/ai@rc
```

## Primary API

- **L4 harness:** `AgentHarness`, `makeReferenceAdapter`, the session verbs
  (`promptTurn` / `continueTurn` / `suspendTurn` / `compact` / `detach` /
  `stop` / `destroy`), `projectHarnessReadiness`, and the ACP and opencode
  adapters.
- **L2 durable log:** `makeHarnessEventLog`, the `HarnessEventLogStore` port,
  replay, live attach, and rerun boundaries over a seq-cursor log.
- **L3 sandbox:** the sandbox-provider contract, `makeLocalSandboxProvider`, and
  `makeLocalProcessSandboxProvider`.
- **L5 UI stream:** `khalaEventToUiChunks`, `initialUiMessage`, `applyUiChunk`,
  `reduceUiMessageStream`, smooth streaming, partial-object streams, and the
  chat transports.

```ts
import { Effect, Stream } from "effect";
import { makeReferenceAdapter } from "@openagentsinc/agent-harness-contract";

// Drive one turn; suspendTurn returns the exact cursor, continueTurn attaches
// at cursor + 1 with no gap and no duplicate.
const program = Effect.gen(function* () {
  const harness = makeReferenceAdapter({ scriptWords: ["plan ", "code ", "verify"] });
  const session = yield* harness.start({
    sessionId: "session-1",
    source: { lane: "test_fixture" },
  });
  const control = yield* session.promptTurn({ turnId: "turn-1", prompt: "Say the plan." });
  return yield* Stream.runCollect(control.events);
});

const events = await Effect.runPromise(program);
console.log(events.map((event) => event.sequence)); // [0, 1, 2, 3, 4]
```

See [Getting started](../../docs/getting-started.md) for the full
suspend/continue and event → UI chunk flows.

## What it contains

- **`adapter.ts`** — `AgentHarness`. Fields are `harnessId`, `harnessKind`
  (`AgentDefinitionHarnessKind`), `adapterKind`, `builtinTools`, the two
  built-in tool flags, an optional `lifecycleStateSchema`, an optional
  `getBootstrap`, and one entry method `start`. There is no static capability
  object. Optional behavior is signalled by method presence. A request the
  adapter cannot satisfy fails with `HarnessCapabilityUnsupported` (the
  Box-facade 501 posture, one layer lower).
- **`session.ts`** — `HarnessSession` verbs `promptTurn`, `continueTurn`,
  `suspendTurn`, `compact`, `detach`, `stop`, and `destroy`, plus the
  `HarnessPromptControl` handle (event `Stream`, `done`, tool-result and
  approval and user-message submission, interrupt). Lifecycle is caller-owned
  and explicit, so a session can outlive its process (the durable journal
  persists it). This is a deliberate divergence from an auto-`Scope`-destroyed
  resource.
- **`stream.ts`** — the harness stream event IS the neutral
  `KhalaRuntimeEvent` (`openagents.khala_runtime_event.v1`). There is no new
  event union. The `sequence` field is the durable replay cursor.
- **`lifecycle-state.ts`** — `HarnessResumeState` and
  `HarnessContinuationState`. The continuation state pins the exact suspend
  cursor and records `lossy`.
- **`capability.ts`**, **`permission.ts`**, **`skill.ts`**, **`host-tool.ts`**,
  **`common-tool.ts`** (the `read`/`write`/`edit`/`bash`/`glob`/`grep`/`webSearch`
  vocabulary with `nativeName`/`commonName`/`providerExecuted` normalization),
  and **`bootstrap.ts`**. RLM-03 registers the `history_recall` host-tool wire
  form (`historyRecallHostToolSpec`, `REGISTERED_HARNESS_HOST_TOOLS`) here.
  The Effect AI Tool authoring form and HistoryRecall handlers live in
  `@openagentsinc/history-corpus`.
- **`reference-adapter.ts`** and **`event-builder.ts`** — an in-memory
  reference `AgentHarness` and event builders for the conformance suite.
- **`sandbox.ts`** and **`local-sandbox-provider.ts`** (HARN-07 core) — the
  harness sandbox-provider contract. `HarnessSandboxProvider` is the stable
  factory for a `HarnessSandboxSession` workspace with file I/O and command
  execution. Optional methods signal capability. A provider that cannot expose
  ports omits `getPortUrl`. A provider that cannot rehydrate a session omits
  `resumeSession`. The provider owns the sandbox lifecycle and the adapter never
  calls `stop`. `makeLocalSandboxProvider` is an in-memory test double for
  hermetic conformance. The managed-sandbox substrate implements this port in the
  desktop cutover. Phase-1 managed sandbox omits `getPortUrl`.
- **`event-log-store.ts`** and **`event-log.ts`** (HARN-02) — the durable
  seq-cursor event log. `HarnessEventLogStore` is the persistence port (the
  in-memory reference ships here, the desktop local-turn journal and the
  managed-sandbox event store implement it later). `HarnessEventLog` is the
  runtime: `appendEvent`, finite `replay` from a cursor (crash recovery), live
  `attach` (replay the persisted tail then follow new events, single-flight per
  `(turn, consumer class)`), `lastCursor`, and `markRerunBoundary` /
  `rerunBoundaries` so a recomputed tail is distinguishable from a lossless
  attach.

- **`slice-runner.ts`** (HARN-06) — the intra-turn slice runner. `runHarnessSlice`
  time-boxes one slice of a turn by event budget and suspends at the exact cursor
  when the budget is spent. `runTurnInSlices` drives a whole turn as a chain of
  slices, re-entering the session from `continueFrom` after each suspension, so a
  long turn survives short-lived process invocations. Every event can be
  persisted to the durable log in order.
- **`readiness.ts`** (HARN-05) — the unified readiness projection.
  `projectHarnessReadiness` turns one set of adapter readiness inputs into the
  router candidate set, the admitted (ready) subset, normalized snapshots aligned
  with `@openagentsinc/harness-conformance`, and Pylon-style counted capacity
  refs. One source feeds the kernel descriptor, FAV routing, the Apple FM
  candidate set, and the heartbeat.

- **`acp-adapter.ts`** (HARN-04) — a generic ACP harness adapter factory.
  `makeAcpHarnessAdapter` turns any admitted Agent Client Protocol peer (Grok,
  Cursor) into an `AgentHarness`. `acpEventToKhalaEvents` projects the ACP
  bridge vocabulary onto the neutral stream, and
  `acpPermissionToRuntimeInteractionPayload` routes an approval through the
  canonical `RuntimeInteraction` model instead of a bespoke path.
- **`opencode-adapter.ts`** (HARN-08) — the opencode harness adapter.
  `makeOpencodeAdapter` presents an opencode session as an `AgentHarness`, and
  `opencodeEventToKhalaEvents` projects opencode's real session stream (text and
  reasoning deltas, tool call and result, the step-ended usage boundary) onto
  the neutral stream. opencode has no explicit turn boundary, so the adapter
  synthesizes `turn.started` and derives `turn.finished` plus usage from the
  step boundary.

- **`local-process-sandbox-provider.ts`** (HARN-07) — a REAL local sandbox
  provider backed by the host filesystem and `child_process` (distinct from the
  in-memory reference double). It composes `<base>/<sessionId>`, materializes
  bootstrap files, runs bootstrap and `run` commands as real host processes,
  resolves paths beneath the session workspace, and omits `getPortUrl` (no port
  infrastructure). This is the owner-local cheap-isolation rung.

- **`ui-message-chunk.ts`** and **`ui-message-reducer.ts`** (STREAM-02 #9130)
  — the core live-to-UI layer. `khalaEventToUiChunks` projects the neutral
  stream onto a 17-type Schema-encodable chunk vocabulary with send-flag and
  visibility gating. Chunks carry only refs and safe text, never raw payloads.
  `applyUiChunk` is the pure progressive fold with the tool-call state machine.
  `reduceUiMessageStream` holds snapshots in a `SubscriptionRef` so a renderer
  reads the current value and the change stream from one source.
- **`smooth-stream.ts`** (STREAM-04 #9132) — a generic pacing operator for
  delta streams. Text deltas re-chunk at word, line, or regex boundaries and
  emit paced. Non-text elements flush the buffer and pass through unpaced.
  Concatenated text stays byte-identical.
- **`partial-object-stream.ts`** (STREAM-06 #9134) — partial-JSON repair and
  progressive `PartialView<T>` streaming for structured output. A partial can
  never be used where a validated value is required. The only validated path
  is the full Schema decode in `finalizePartialObject`.
- **`toolkit-bridge.ts`** (STREAM-07 #9135) — one tool substrate across the
  model-call lanes and the harness. An Effect AI `Tool` (`effect/unstable/ai`,
  schema-typed, handlers as a Layer) is the authoring form.
  `harnessHostToolSpecFromTool` projects it onto the `HarnessHostToolSpec`
  JSON Schema wire form (`Tool.getJsonSchema`), and `resolveHostToolCall`
  resolves a harness host-tool call through the Toolkit handler Layer with
  fail-closed `isError` results. `needsApproval` composes with the ONE
  canonical `RuntimeInteraction` approval model
  (`hostToolApprovalInteractionPayload`, `applyHostToolApprovalDecision` for
  `allow-once`/`allow-session`/`deny`). Preliminary handler results
  (`HandlerContext.preliminary`) stream as `tool-output-preliminary` chunks.
  The final result is `tool-output-available`.

## Conformance

`reference-adapter.test.ts` proves the contract semantics. It covers full-turn
streaming, **suspend then continue cursor exactness (attach at `cursor + 1`,
no gap, no duplicate)**, lossy-continuation honesty, fail-closed capability
refusal, and re-importable lifecycle export. `schemas.test.ts` covers the data
schemas. `event-log.test.ts` (HARN-02) proves durable replay after simulated
process death, dup-free rejection of non-increasing sequences, rerun-boundary
visibility, live-attach replay-then-follow, and single-flight supersession.

```sh
pnpm --dir packages/agent-harness-contract test
pnpm --dir packages/agent-harness-contract typecheck
```

## Consumers (later HARN packets)

HARN-02 backs the cursor with a durable event log. HARN-03 and HARN-04 re-home
the desktop provider lanes as adapters. HARN-05 merges readiness under the
router. HARN-06 implements suspend and continue on every adapter. HARN-07 adds
the managed sandbox as a harness sandbox provider.

## More

- [Layer index](../../docs/README.md) · [Packages](../../docs/packages.md) ·
  [Getting started](../../docs/getting-started.md)
