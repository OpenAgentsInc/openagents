import { describe, expect, test } from "bun:test"
import { Deferred, Effect, Fiber } from "effect"

import {
  PylonRuntimeRetrySchedules,
  scopedTimeout,
} from "./effect-runtime-patterns.ts"

describe("PY-1 effect-runtime-patterns (#8578)", () => {
  test("scoped deadline timers are released when the owning fiber is interrupted", async () => {
    const events: string[] = []

    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const acquired = yield* Deferred.make<void>()
          const fiber = yield* Effect.forkScoped(
            Effect.scoped(
              Effect.gen(function* () {
                yield* scopedTimeout({
                  delayMs: 10_000,
                  onTimeout: () => events.push("timeout-fired"),
                  setTimeout: ((() => {
                    events.push("timeout-scheduled")
                    return 42 as unknown as ReturnType<typeof setTimeout>
                  }) as unknown) as typeof setTimeout,
                  clearTimeout: (((timer: ReturnType<typeof setTimeout>) => {
                    events.push(`timeout-cleared:${String(timer)}`)
                  }) as unknown) as typeof clearTimeout,
                })
                yield* Deferred.succeed(acquired, undefined)
                return yield* Effect.never
              }),
            ),
          )

          yield* Deferred.await(acquired)
          yield* Fiber.interrupt(fiber)
        }),
      ),
    )

    expect(events).toEqual(["timeout-scheduled", "timeout-cleared:42"])
  })

  test("Pylon retry schedules expose named Effect schedules for shared runtime paths", () => {
    expect(Object.keys(PylonRuntimeRetrySchedules).sort()).toEqual([
      "d1TransientFailure",
      "durableObjectCall",
      "externalHttpProviderCall",
      "gitGithubOperation",
      "publicProjectionSync",
      "walletAdjacentCall",
    ])
  })

  test("scopedTimeout clamps negative delay to zero", async () => {
    let scheduledDelay: number | undefined

    await Effect.runPromise(
      Effect.scoped(
        scopedTimeout({
          delayMs: -50,
          onTimeout: () => {},
          setTimeout: (((
            _fn: (...args: unknown[]) => void,
            delay?: number,
          ) => {
            scheduledDelay = delay
            return 1 as unknown as ReturnType<typeof setTimeout>
          }) as unknown) as typeof setTimeout,
          clearTimeout: ((() => {}) as unknown) as typeof clearTimeout,
        }),
      ),
    )

    expect(scheduledDelay).toBe(0)
  })
})
