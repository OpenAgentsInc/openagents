# Getting started

This guide installs the OpenAgents AI SDK and shows three small programs.
Each program runs against the published packages. Every symbol in this guide
exists in the `rc` train on npm.

The SDK source lives at
[OpenAgentsInc/ai](https://github.com/OpenAgentsInc/ai) under Apache-2.0.

## Install

Install the umbrella package from the `rc` dist-tag:

```sh
pnpm add @openagentsinc/ai@rc
```

For a production consumer, pin the exact train instead of a floating tag:

```sh
pnpm add @openagentsinc/ai@0.2.0-rc.1
```

Two facts about the packages:

- The packages publish TypeScript source directly. Use a TypeScript-aware
  loader such as `tsx`, Vite, or Vite Plus. Plain `node` does not strip types
  inside `node_modules`.
- The packages depend on `effect` `4.0.0-beta.94`. Keep one `effect` version
  in your application.

## The umbrella subpaths

The root entry `@openagentsinc/ai` re-exports every layer. Each layer also
has its own subpath:

| Subpath | Layer | Contents |
| --- | --- | --- |
| `@openagentsinc/ai/model` | L0 | LanguageModel Layer, StreamPart maps, ExecutionPlan fallback |
| `@openagentsinc/ai/schema` | L1 | `KhalaRuntimeEvent` and the runtime vocabulary |
| `@openagentsinc/ai/event-log` | L2 | Durable event log and event-log store |
| `@openagentsinc/ai/sandbox` | L3 | Sandbox contract and the local-process provider |
| `@openagentsinc/ai/harness` | L4 | `AgentHarness` adapters, readiness, host tools |
| `@openagentsinc/ai/ui-stream` | L5 | UI chunk projection, reducer, chat transports |
| `@openagentsinc/ai/recall` | L6 | History corpus and Tier D recall |
| `@openagentsinc/ai/rlm` | RLM | Recursive recall engine over a corpus source |

## Run a harness turn, suspend it, and continue it

The reference adapter is the in-memory harness that exercises the contract.
`suspendTurn` returns the exact cursor of the last event you pulled.
`continueTurn` attaches at `cursor + 1` with no gap and no duplicate.

```ts
import { Effect, Stream } from 'effect'
import { makeReferenceAdapter } from '@openagentsinc/ai/harness'

const program = Effect.gen(function* () {
  const harness = makeReferenceAdapter({ scriptWords: ['plan ', 'code ', 'verify'] })

  const session = yield* harness.start({
    sessionId: 'session-1',
    source: { lane: 'test_fixture' },
  })
  const control = yield* session.promptTurn({ turnId: 'turn-1', prompt: 'Say the plan.' })

  const firstSlice = yield* Stream.runCollect(control.events.pipe(Stream.take(2)))
  const continuation = yield* session.suspendTurn()

  const resumed = yield* harness.start({
    sessionId: 'session-1',
    source: { lane: 'test_fixture' },
    continueFrom: continuation,
  })
  const control2 = yield* resumed.continueTurn({})
  const secondSlice = yield* Stream.runCollect(control2.events)

  return { continuation, firstSlice, secondSlice }
})

const out = await Effect.runPromise(program)
console.log(out.continuation.cursor) // 1
console.log(out.firstSlice.map(event => event.sequence)) // [0, 1]
console.log(out.secondSlice.map(event => event.sequence)) // [2, 3, 4]
```

The second `start` call can run in a different process. The continuation
carries the cursor and the remaining state, so the two slices concatenate
into one contiguous stream.

## Project runtime events to UI chunks

`khalaEventToUiChunks` is the pure projection from one `KhalaRuntimeEvent` to
renderable chunks. `applyUiChunk` folds chunks into one `UiMessage`. The
projection is redaction-aware, so a public surface admits only public events.

```ts
import { Effect, Stream } from 'effect'
import { makeReferenceAdapter } from '@openagentsinc/ai/harness'
import {
  applyUiChunk,
  initialUiMessage,
  khalaEventToUiChunks,
} from '@openagentsinc/ai/ui-stream'

const program = Effect.gen(function* () {
  const harness = makeReferenceAdapter({ scriptWords: ['Hello ', 'world'] })
  const session = yield* harness.start({
    sessionId: 'session-1',
    source: { lane: 'test_fixture' },
  })
  const control = yield* session.promptTurn({ turnId: 'turn-1', prompt: 'Greet.' })
  const events = yield* Stream.runCollect(control.events)

  const chunks = events.flatMap(event => khalaEventToUiChunks(event))
  return chunks.reduce(applyUiChunk, initialUiMessage())
})

const message = await Effect.runPromise(program)
console.log(message.status) // 'complete'
console.log(message.parts) // one text part: 'Hello world'
```

## Build a corpus and run Tier D recall

`buildHistoryCorpus` exports history into cursor-addressed entries with a
counted inclusion policy. `recallTierD` answers a typed question over those
entries. Tier D is pure and deterministic and makes zero model calls.

```ts
import { Effect } from 'effect'
import { buildHistoryCorpus, recallTierD } from '@openagentsinc/ai/recall'

const program = Effect.gen(function* () {
  const corpus = yield* buildHistoryCorpus({
    scope: { _tag: 'Thread', threadId: 'thread-1' },
    threads: [
      {
        id: 'thread-1',
        title: 'Deploy review',
        updatedAt: '2026-07-21T00:00:00Z',
        notes: [
          {
            key: 'note-1',
            role: 'user',
            text: 'Deploy the docs site after the tests pass.',
            timestamp: '2026-07-21T00:00:00Z',
          },
          {
            key: 'note-2',
            role: 'assistant',
            text: 'The tests passed. The deploy is complete.',
            timestamp: '2026-07-21T00:05:00Z',
          },
        ],
      },
    ],
    policy: {
      includeVisibilities: ['private'],
      includeRedactionClasses: ['private_ref'],
    },
    builtAt: '2026-07-21T00:10:00Z',
  })

  return yield* recallTierD({
    entries: corpus.entries,
    coverageNote: corpus.manifest.coverage.note,
    question: { _tag: 'Grep', pattern: 'deploy' },
  })
})

const response = await Effect.runPromise(program)
console.log(response.honesty.tier) // 'deterministic'
console.log(response.cost.modelCalls) // 0
for (const span of response.answers) console.log(span.excerpt)
```

The response carries an honesty record: entries scanned, entries total, caps
hit, and the corpus coverage note. Nothing is dropped silently.

## Next steps

- Read [Packages](/aisdk/docs/packages) for the per-package export map.
- Read the upstream docs index at
  [OpenAgentsInc/ai docs](https://github.com/OpenAgentsInc/ai/blob/main/docs/README.md).
