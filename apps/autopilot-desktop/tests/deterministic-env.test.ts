// Deterministic Effect environment: TestClock + seeded RNG + stub transport.
//
// Demonstrates the reusable layer set and proves the property that matters: with
// injected clock/random/transport, the SAME seed + the SAME clock produce
// byte-identical output every run, and the transport calls are observable. No
// `Date.now`, no `Math.random`, no real network. Runs under bun test via
// Effect.runPromise (this repo uses bun test, not @effect/vitest).

import { describe, expect, test } from "bun:test"

import { Effect } from "effect"
import { TestClock } from "effect/testing"

import {
  Transport,
  TestEnvironmentLayer,
  withSeed,
  currentMillis,
} from "../src/testing/deterministic-env"
import {
  SyntheticEventService,
  SyntheticEventServiceLayer,
} from "../src/testing/synthetic-event-service"

describe("deterministic environment (TestClock + seeded RNG + stub transport)", () => {
  test("TestClock starts at 0 and only moves on explicit adjust", async () => {
    const program = Effect.gen(function* () {
      const before = yield* currentMillis
      yield* TestClock.adjust("1 minute")
      const after = yield* currentMillis
      return { before, after }
    })
    const { before, after } = await Effect.runPromise(
      program.pipe(Effect.provide(TestEnvironmentLayer())),
    )
    expect(before).toBe(0)
    expect(after).toBe(60_000)
  })

  test("the synthetic-event service is fully deterministic across runs", async () => {
    const responses = { "world.emitSyntheticEvent": { ok: true } }

    const run = () => {
      const program = Effect.gen(function* () {
        const service = yield* SyntheticEventService
        const transport = yield* Transport
        // Advance the test clock to a fixed time, then emit two events.
        yield* TestClock.adjust("1500 millis")
        const a = yield* service.emit("crackling_energy")
        const b = yield* service.emit("gateway_portal")
        const calls = yield* transport.calls
        return { a, b, calls }
      })
      return Effect.runPromise(
        // Seed the RNG so "random" ids are reproducible.
        withSeed("verse-spawned-scene", program).pipe(
          Effect.provide(SyntheticEventServiceLayer),
          Effect.provide(TestEnvironmentLayer(responses)),
        ),
      )
    }

    const first = await run()
    const second = await run()

    // Same seed + same TestClock ⇒ identical ids and timestamps.
    expect(first.a).toEqual(second.a)
    expect(first.b).toEqual(second.b)
    // The timestamp came from the (test) clock, not wall-clock.
    expect(first.a.generatedAtMillis).toBe(1500)
    expect(first.b.generatedAtMillis).toBe(1500)
    // The two ids differ (RNG advanced), but are reproducible.
    expect(first.a.id).not.toBe(first.b.id)
    // The transport saw exactly the two scripted calls, in order — no network.
    expect(first.calls.map((call) => call.method)).toEqual([
      "world.emitSyntheticEvent",
      "world.emitSyntheticEvent",
    ])
  })

  test("an unscripted transport method fails deterministically (never hangs)", async () => {
    const program = Effect.gen(function* () {
      const transport = yield* Transport
      return yield* transport.call({ method: "not.scripted", payload: null })
    })
    const exit = await Effect.runPromiseExit(
      program.pipe(Effect.provide(TestEnvironmentLayer({}))),
    )
    expect(exit._tag).toBe("Failure")
  })
})
