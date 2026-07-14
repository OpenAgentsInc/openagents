# Pipeline Signals

Deterministic completion signals for async package pipelines (openagents
issue #8782, ported from the T3 Code `DrainableWorker` / runtime-signal-bus
pattern).

Two primitives:

- `makeDrainableWorker(process)` — a queue-backed worker whose `drain` effect
  settles when the queue is empty AND all in-flight work has completed.
  Failures in `process` never kill the worker loop or hang `drain`; wire
  `onFailure` to publish a failure milestone.
- `makePipelineSignalBus<S>()` — a typed milestone bus on Effect PubSub.
  Pipelines publish schema-tagged milestone signals; tests and orchestration
  subscribe *before* triggering work, then `awaitPipelineSignal(subscription,
  predicate)` for the exact milestone instead of sleeping or polling.

```ts
const worker = yield* makeDrainableWorker(item => handle(item))
yield* worker.enqueue(job)
yield* worker.drain // settles when queue empty + in-flight work done

const bus = yield* makePipelineSignalBus<MyMilestone>()
const subscription = yield* bus.subscribe // subscribe FIRST
yield* startPipeline({ signalBus: bus })
const settled = yield* awaitPipelineSignal(
  subscription,
  (signal): signal is Settled => signal.kind === "my.pipeline.settled",
)
```

## Naming contract — not evidence receipts

Pipeline signals are deterministic **test/orchestration synchronization
events**. They are explicitly NOT the user-facing evidence "receipts"
vocabulary used by Blueprint/Cloud (`resource_usage_receipt.v1`, action
receipts, payout receipts). Never surface a pipeline signal as user-facing
evidence, and never name a pipeline signal type `*Receipt`.

## Adopters

- `@openagentsinc/khala-tools` — dispatcher milestones (turn call budgets,
  bounded-output finalization, dispatch settlement) and the queued
  drainable dispatcher.
- `@openagentsinc/world-client` — browser transport milestones (socket
  creation, pending commands) awaited by transport tests.
