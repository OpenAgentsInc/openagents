import { expect, it } from "@effect/vitest"
import { Effect, Random } from "effect"

import {
  currentMillis,
  KhalaQaTransport,
  TestClock,
  TestEnvironmentLayer,
  withSeed,
} from "../deterministic-env.js"

it.effect("TestEnvironmentLayer provides TestClock and scripted transport", () =>
  Effect.gen(function* () {
    const before = yield* currentMillis
    yield* TestClock.adjust("2 seconds")
    const after = yield* currentMillis
    const transport = yield* KhalaQaTransport
    const response = yield* transport.call({
      method: "desktop.snapshot",
      payload: { surface: "fleet" },
    })
    const calls = yield* transport.calls

    expect(before).toBe(0)
    expect(after).toBe(2_000)
    expect(response).toEqual({ ok: true })
    expect(calls).toEqual([
      { method: "desktop.snapshot", payload: { surface: "fleet" } },
    ])
  }).pipe(Effect.provide(TestEnvironmentLayer({ "desktop.snapshot": { ok: true } }))),
)

it.effect("withSeed makes Effect random values reproducible", () => {
  const sample = withSeed(
    "khala-qa-seed",
    Effect.gen(function* () {
      const first = yield* Random.nextIntBetween(0, 999_999)
      const second = yield* Random.nextIntBetween(0, 999_999)
      return [first, second] as const
    }),
  )

  return Effect.gen(function* () {
    const firstRun = yield* sample
    const secondRun = yield* sample

    expect(firstRun).toEqual(secondRun)
    expect(firstRun[0]).not.toBe(firstRun[1])
  })
})
