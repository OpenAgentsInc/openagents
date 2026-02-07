import { Effect, FiberRef } from "effect"

import { ProbeService } from "../effect/ProbeService.ts"
import { CurrentSpanId } from "../effect/span.ts"
import type { SpanId } from "../spec.ts"
import { TestContext } from "./TestContext.ts"

const asError = (u: unknown): { readonly name: string; readonly message: string } => {
  if (u instanceof Error) return { name: u.name, message: u.message }
  return { name: "UnknownError", message: String(u) }
}

export const step = <A, E, R>(
  name: string,
  effect: Effect.Effect<A, E, R>,
): Effect.Effect<A, E, ProbeService | TestContext | R> =>
  Effect.gen(function* () {
    const ctx = yield* TestContext
    const probe = yield* ProbeService

    const parentSpanId = yield* FiberRef.get(CurrentSpanId)
    const spanId = crypto.randomUUID() as SpanId
    const start = Date.now()

    yield* probe.emit({
      type: "span.started",
      runId: ctx.runId,
      ts: start,
      testId: ctx.testId,
      spanId,
      parentSpanId,
      name,
      kind: "step",
    })

    const run = Effect.locally(CurrentSpanId, spanId)(effect)

    return yield* run.pipe(
      Effect.tap(() =>
        probe.emit({
          type: "span.finished",
          runId: ctx.runId,
          ts: Date.now(),
          testId: ctx.testId,
          spanId,
          status: "passed",
          durationMs: Date.now() - start,
        }),
      ),
      Effect.tapError((error) =>
        probe.emit({
          type: "span.finished",
          runId: ctx.runId,
          ts: Date.now(),
          testId: ctx.testId,
          spanId,
          status: "failed",
          durationMs: Date.now() - start,
          error: asError(error),
        }),
      ),
    )
  })

export const assertTrue = (condition: boolean, message: string): Effect.Effect<void, Error> =>
  condition ? Effect.void : Effect.fail(new Error(message))

export const assertEqual = <A>(actual: A, expected: A, message: string): Effect.Effect<void, Error> =>
  actual === expected
    ? Effect.void
    : Effect.fail(new Error(`${message} (expected=${String(expected)} actual=${String(actual)})`))
