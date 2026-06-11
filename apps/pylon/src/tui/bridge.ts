// Effect <-> Solid bridge (issue #4737).
//
// This is the ONLY module that imports both Effect and Solid. It subscribes
// to the Phase 0 runtime seam (SubscriptionRefs + PylonEvent PubSub, issue
// #4736) and writes Solid signals/stores, applying event bursts inside
// `batch()` on a ~16ms window (the pattern opencode uses for SSE bursts).
// Components never see a fiber; services never see a signal.

import { Effect, PubSub, Stream, SubscriptionRef, type Scope } from "effect"
import { batch } from "solid-js"
import type { PylonNodeRuntime } from "../node/runtime"
import {
  appendRuntimeLogEntry,
  recordBalancePoint,
  setOperatorText,
  setTelemetryState,
  setVerboseMode,
  setWalletState,
} from "./store"

export interface AttachRuntimeOptions {
  verbose: boolean
  // Max events drained per flush window. Bounded so a pathological burst
  // cannot starve the render loop inside a single batch().
  maxBatch?: number
  batchWindowMs?: number
}

// Subscribes the Solid view state to the node runtime. Consumers are forked
// into the caller's Scope, so closing it (Ctrl+C, smoke exit) detaches the
// view cleanly. Subscribe-before-replay ordering matches the Phase 0
// subscriber: no event can fall between feed replay and the live tail, and
// nothing else writes during this window because services fork afterwards.
export const attachRuntimeToView = (
  runtime: PylonNodeRuntime,
  options: AttachRuntimeOptions,
): Effect.Effect<void, never, Scope.Scope> =>
  Effect.gen(function* () {
    const maxBatch = options.maxBatch ?? 256
    const batchWindowMs = options.batchWindowMs ?? 16

    setVerboseMode(options.verbose)

    const eventSubscription = yield* PubSub.subscribe(runtime.events)

    const wallet = yield* SubscriptionRef.get(runtime.wallet)
    const telemetry = yield* SubscriptionRef.get(runtime.telemetry)
    const operator = yield* SubscriptionRef.get(runtime.operator)
    const feed = yield* SubscriptionRef.get(runtime.logFeed)
    batch(() => {
      setWalletState(wallet)
      setTelemetryState(telemetry)
      setOperatorText(operator.text)
      for (const entry of feed) {
        appendRuntimeLogEntry(entry)
      }
    })

    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.wallet), (state) =>
        Effect.sync(() => {
          setWalletState(state)
          recordBalancePoint(new Date().toISOString(), state.balanceSats)
        }),
      ),
    )
    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.telemetry), (state) =>
        Effect.sync(() => setTelemetryState(state)),
      ),
    )
    yield* Effect.forkScoped(
      Stream.runForEach(SubscriptionRef.changes(runtime.operator), (state) =>
        Effect.sync(() => setOperatorText(state.text)),
      ),
    )

    // Event tail: wait for at least one event, drain up to maxBatch, apply
    // them in one batch(), then idle for the window before the next drain.
    yield* Effect.forkScoped(
      Effect.gen(function* () {
        while (true) {
          const events = yield* PubSub.takeBetween(eventSubscription, 1, maxBatch)
          batch(() => {
            for (const event of events) {
              if (event.type === "log") {
                appendRuntimeLogEntry(event)
              }
            }
          })
          yield* Effect.sleep(`${batchWindowMs} millis`)
        }
      }),
    )
  })
