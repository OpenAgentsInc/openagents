import { beforeEach, describe, expect, test } from "bun:test"
import { Effect } from "effect"
import { logMessage, makePylonNodeRuntime, setWalletStatus } from "../src/node/runtime"
import { attachRuntimeToView } from "../src/tui/bridge"
import { feedItems, resetViewState, walletState } from "../src/tui/store"

describe("tui bridge", () => {
  beforeEach(() => {
    resetViewState()
  })

  test("replays existing feed and follows live events into the view store", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          // Logged before attach: must be replayed.
          yield* logMessage(runtime, "info", "before attach")
          yield* attachRuntimeToView(runtime, { verbose: false, batchWindowMs: 1 })
          expect(feedItems.length).toBe(1)
          expect(feedItems[0]?.markdown).toContain("before attach")
          // Logged after attach: must arrive via the batched event tail.
          yield* logMessage(runtime, "info", "after attach")
          yield* logMessage(runtime, "verbose", "hidden chatter")
          yield* Effect.sleep("80 millis")
          expect(feedItems.length).toBe(2)
          expect(feedItems[1]?.markdown).toContain("after attach")
        }),
      ),
    )
  })

  test("wallet ref changes propagate to the wallet signal", async () => {
    await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const runtime = yield* makePylonNodeRuntime
          yield* attachRuntimeToView(runtime, { verbose: false, batchWindowMs: 1 })
          expect(walletState().daemonOnline).toBe(false)
          yield* setWalletStatus(runtime, { daemonOnline: true, balanceSats: 41, readiness: "receive-ready" })
          yield* Effect.sleep("50 millis")
          expect(walletState().daemonOnline).toBe(true)
          expect(walletState().balanceSats).toBe(41)
        }),
      ),
    )
  })

  test("detaches cleanly when the scope closes", async () => {
    const runtime = await Effect.runPromise(
      Effect.scoped(
        Effect.gen(function* () {
          const created = yield* makePylonNodeRuntime
          yield* attachRuntimeToView(created, { verbose: false, batchWindowMs: 1 })
          return created
        }),
      ),
    )
    const before = feedItems.length
    // Scope closed: further runtime logs must not reach the view store.
    await Effect.runPromise(logMessage(runtime, "info", "post-detach"))
    await new Promise((resolve) => setTimeout(resolve, 60))
    expect(feedItems.length).toBe(before)
  })
})
