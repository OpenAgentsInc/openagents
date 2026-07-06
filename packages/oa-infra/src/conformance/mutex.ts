/**
 * Mutex conformance suite (CFG-2, issue #8517).
 *
 * EVERY Mutex backend must pass this suite unmodified (audit §5 hot-swap
 * guarantee). Lock names are namespaced per test with a fresh UUID so
 * shared backends (one Postgres server) can host the suite.
 */
import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import type { Layer } from "effect"
import { Mutex, type MutexShape } from "../mutex.ts"

export interface MutexConformanceOptions {
  readonly label: string
  /** Called at TEST time (after any beforeAll infra setup). */
  readonly makeLayer: () => Layer.Layer<Mutex>
  readonly skip?: boolean
}

const sleep = (ms: number) => Effect.promise(() => new Promise((r) => setTimeout(r, ms)))

export const runMutexConformance = (options: MutexConformanceOptions): void => {
  const suite = options.skip === true ? describe.skip : describe
  const lockName = () => `mutex-conf-${crypto.randomUUID()}`

  const withMutex = async <A>(body: (mutex: MutexShape) => Promise<A>): Promise<A> => {
    const layer = options.makeLayer()
    return Effect.runPromise(
      Effect.provide(
        Effect.gen(function* () {
          const mutex = yield* Mutex
          return yield* Effect.promise(() => body(mutex))
        }),
        layer,
      ),
    )
  }

  suite(`Mutex conformance [${options.label}]`, () => {
    test("withLock returns the critical section's value", async () => {
      const name = lockName()
      const value = await withMutex((mutex) =>
        Effect.runPromise(mutex.withLock(name, Effect.succeed(42))),
      )
      expect(value).toBe(42)
    })

    test("sections with the same name never overlap", async () => {
      const name = lockName()
      await withMutex(async (mutex) => {
        let active = 0
        let maxActive = 0
        const events: Array<string> = []
        const section = (label: string) =>
          mutex.withLock(
            name,
            Effect.gen(function* () {
              active += 1
              maxActive = Math.max(maxActive, active)
              events.push(`${label}:enter`)
              yield* sleep(120)
              events.push(`${label}:exit`)
              active -= 1
            }),
          )
        await Promise.all([
          Effect.runPromise(section("a")),
          Effect.runPromise(section("b")),
          Effect.runPromise(section("c")),
        ])
        expect(maxActive).toBe(1)
        // Every enter is immediately followed by the SAME section's exit.
        for (let index = 0; index < events.length; index += 2) {
          const enter = events[index]
          const exit = events[index + 1]
          expect(enter?.endsWith(":enter")).toBe(true)
          expect(exit).toBe(`${enter?.split(":")[0]}:exit`)
        }
      })
    })

    test("different names do not contend", async () => {
      await withMutex(async (mutex) => {
        const nameA = lockName()
        const nameB = lockName()
        let bFinishedWhileAHeld = false
        let aHolding = false
        const holdA = mutex.withLock(
          nameA,
          Effect.gen(function* () {
            aHolding = true
            yield* sleep(400)
            aHolding = false
          }),
        )
        const quickB = Effect.gen(function* () {
          yield* sleep(50) // let A acquire first
          yield* mutex.withLock(
            nameB,
            Effect.sync(() => {
              bFinishedWhileAHeld = aHolding
            }),
          )
        })
        await Promise.all([Effect.runPromise(holdA), Effect.runPromise(quickB)])
        expect(bFinishedWhileAHeld).toBe(true)
      })
    })

    test("the lock is released when the section fails", async () => {
      const name = lockName()
      await withMutex(async (mutex) => {
        const failing = await Effect.runPromiseExit(
          mutex.withLock(name, Effect.fail({ _tag: "SectionBlewUp" } as const)),
        )
        expect(failing._tag).toBe("Failure")
        // If the failure leaked the lock this would hang; give it a hard cap.
        const reacquired = await Promise.race([
          Effect.runPromise(mutex.withLock(name, Effect.succeed("ok"))),
          new Promise<"timeout">((resolve) => setTimeout(() => resolve("timeout"), 2_000)),
        ])
        expect(reacquired).toBe("ok")
      })
    })
  })
}
