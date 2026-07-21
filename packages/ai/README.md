# @openagentsinc/ai — the OpenAgents AI SDK

The OpenAgents AI SDK is an Effect-native toolkit for durable agent
applications. It gives you durable, redaction-aware, cursor-exact agent
streams with coding-agent harnesses and recall — on Effect.

This package is the SDK front door. It holds no logic. It re-exports the
entry points of the SDK layer packages, so one install gives the full
surface. You can also install each layer package directly.

## The layers

Each layer speaks the neutral `KhalaRuntimeEvent` vocabulary upward. There
is one event union and one durable cursor.

```
L6  RECALL        @openagentsinc/history-corpus
                  corpus export, cursor-addressed entries, HistoryRecall
                  contract, deterministic Tier D recall
------------------------------------------------------------------
L5  UI STREAM     agent-harness-contract/ui-message-chunk + ui-message-reducer
                  + smooth-stream + partial-object-stream
------------------------------------------------------------------
L4  HARNESS       agent-harness-contract — AgentHarness adapter, session
                  verbs (promptTurn / suspendTurn / continueTurn / compact /
                  detach / stop / destroy), capability-by-method-presence,
                  slice runner, readiness projection, skills, host tools,
                  toolkit bridge, ACP + opencode adapters
------------------------------------------------------------------
L3  SANDBOX       harness sandbox-provider contract + local-process provider
------------------------------------------------------------------
L2  DURABLE LOG   agent-harness-contract/event-log + event-log-store —
                  seq-cursor append, replay, live attach, rerun boundaries
------------------------------------------------------------------
L1  VOCABULARY    @openagentsinc/agent-runtime-schema — KhalaRuntimeEvent
                  (the single neutral event union, sequence = durable
                  cursor, visibility + redactionClass + causalityRefs)
------------------------------------------------------------------
L0  MODEL CALL    effect/unstable/ai (upstream, consumed, never forked)
                  + @openagentsinc/khala-ai-sdk-core — the LanguageModel
                  Layer over the existing transport, bidirectional
                  StreamPart maps
```

## The roster

| Package                                 | Layer          | What it gives you                                                                                  |
| --------------------------------------- | -------------- | -------------------------------------------------------------------------------------------------- |
| `@openagentsinc/khala-ai-sdk-core`      | L0 model call  | The Effect AI `LanguageModel` Layer, `Response.StreamPart` maps, `ExecutionPlan` provider fallback |
| `@openagentsinc/agent-runtime-schema`   | L1 vocabulary  | `KhalaRuntimeEvent`, `RuntimeInteraction`, route schemas, AI SDK ingestion parts                   |
| `@openagentsinc/agent-harness-contract` | L2 durable log | Seq-cursor event log with append, replay, live attach, rerun boundaries                            |
| `@openagentsinc/agent-harness-contract` | L3 sandbox     | Sandbox-provider contract, local and local-process providers                                       |
| `@openagentsinc/agent-harness-contract` | L4 harness     | `AgentHarness` adapters, session verbs, readiness projection, skills, host tools                   |
| `@openagentsinc/agent-harness-contract` | L5 UI stream   | UI message chunks, progressive reducer, smooth stream, partial-object stream                       |
| `@openagentsinc/history-corpus`         | L6 recall      | Corpus export, `HistoryRecall` contract, deterministic Tier D recall                               |

## The subpaths

The umbrella exports curated per-layer subpaths that mirror the diagram:

- `@openagentsinc/ai` — the full surface
- `@openagentsinc/ai/model` — L0
- `@openagentsinc/ai/schema` — L1
- `@openagentsinc/ai/event-log` — L2
- `@openagentsinc/ai/sandbox` — L3
- `@openagentsinc/ai/harness` — L4
- `@openagentsinc/ai/ui-stream` — L5
- `@openagentsinc/ai/recall` — L6

## Usage sketch

Build a corpus from the durable log, run deterministic Tier D recall over
it, and project a `KhalaRuntimeEvent` stream to renderable `UiMessage`
chunks.

```ts
import { Effect } from "effect";
import {
  applyUiChunk,
  buildHistoryCorpus,
  initialUiMessage,
  khalaEventToUiChunks,
  recallTierD,
} from "@openagentsinc/ai";

const program = Effect.gen(function* () {
  // 1. Build a deterministic corpus from the durable event log (L2 -> L6).
  const { manifest, entries } = yield* buildHistoryCorpus({
    scope: { _tag: "Thread", threadId: "thread-1" },
    turnIds: ["turn-1"],
    eventLog: store, // any HarnessEventLogStore-shaped reader
    builtAt: "2026-07-21T00:00:00.000Z",
    policy: {
      includeVisibilities: ["public"],
      includeRedactionClasses: ["public_ref"],
    },
  });

  // 2. Run Tier D recall — pure traversal, zero model calls, cited spans.
  const recalled = yield* recallTierD({
    entries,
    coverageNote: manifest.coverage.note,
    question: { _tag: "Grep", pattern: "deploy" },
  });

  // 3. Project runtime events to UI chunks and fold them into a UiMessage.
  let message = initialUiMessage();
  for (const event of events) {
    for (const chunk of khalaEventToUiChunks(event)) {
      message = applyUiChunk(message, chunk);
    }
  }

  return { recalled, message };
});
```

## More

- The SDK docs index: [`docs/ai-sdk/README.md`](../../docs/ai-sdk/README.md)
- The analysis and placement decision:
  [`docs/fable/2026-07-21-effect-native-openagents-ai-sdk-analysis.md`](../../docs/fable/2026-07-21-effect-native-openagents-ai-sdk-analysis.md)
