/**
 * Transient command-notice controller + toast rendering (#8712 follow-up).
 *
 * Owner report (verbatim): "what is that yellow command request shit at top, is
 * that supposed to be there, fix if not". These tests prove the notice is now a
 * TRANSIENT, dismissible toast — it auto-clears on a bounded Effect-scheduled
 * timer (driven here by Effect's TestClock, never wall-clock), a new notice
 * cancels the prior pending clear, and the typed dismiss intent clears it
 * immediately — rather than persisting as a permanent top banner.
 */
import { describe, expect, test } from "vite-plus/test"
import { resolveIntentRef } from "@effect-native/core"
import { Effect, SubscriptionRef } from "@effect-native/core/effect"
import { TestClock } from "@effect-native/core/testing"

import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  type DesktopShellState,
} from "./shell.ts"
import { commandNoticeAutoDismissMillis, makeCommandNoticeController } from "./command-notice.ts"

const { makeIntentRegistry } = await import("@effect-native/core")

const baseState = (): DesktopShellState => initialDesktopShellState("electron/darwin", "18:00")

type AnyNode = Readonly<Record<string, unknown>> & { key?: string; _tag?: string }
const collect = (node: unknown, out: Array<AnyNode> = []): Array<AnyNode> => {
  if (Array.isArray(node)) {
    for (const item of node) collect(item, out)
    return out
  }
  if (typeof node !== "object" || node === null) return out
  const record = node as AnyNode
  if (typeof record._tag === "string") out.push(record)
  for (const value of Object.values(record)) collect(value, out)
  return out
}
const byKey = (root: unknown, key: string): AnyNode | undefined =>
  collect(root).find((node) => node.key === key)

describe("command notice is a transient, dismissible toast (#8712)", () => {
  test("controller auto-clears on the bounded Effect timer (TestClock, not wall-clock)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState())
        const controller = makeCommandNoticeController(state)

        yield* controller.setTransientNotice("That command request was already handled. The duplicate was ignored.")
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBe(
          "That command request was already handled. The duplicate was ignored.",
        )

        // Just under the delay: still present.
        yield* TestClock.adjust(commandNoticeAutoDismissMillis - 1)
        expect((yield* SubscriptionRef.get(state)).commandNotice).not.toBeNull()

        // Crossing the delay boundary: auto-cleared.
        yield* TestClock.adjust(1)
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBeNull()
      }).pipe(Effect.provide(TestClock.layer())),
    )
  })

  test("a new notice cancels the prior pending clear (no early or double dismiss)", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState())
        const controller = makeCommandNoticeController(state)

        yield* controller.setTransientNotice("first")
        // Nearly to the first timer's deadline, then replace it.
        yield* TestClock.adjust(commandNoticeAutoDismissMillis - 100)
        yield* controller.setTransientNotice("second")

        // Past when the FIRST timer would have fired — the replacement notice
        // must still be showing because the prior timer was cancelled.
        yield* TestClock.adjust(200)
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBe("second")

        // The second timer clears on its own full delay.
        yield* TestClock.adjust(commandNoticeAutoDismissMillis)
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBeNull()
      }).pipe(Effect.provide(TestClock.layer())),
    )
  })

  test("dismiss clears immediately and cancels the pending timer", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make(baseState())
        const controller = makeCommandNoticeController(state)

        yield* controller.setTransientNotice("Keybindings are unavailable.")
        yield* controller.dismissNotice
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBeNull()

        // A stale timer must never resurrect or re-clear later.
        yield* TestClock.adjust(commandNoticeAutoDismissMillis * 2)
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBeNull()
      }).pipe(Effect.provide(TestClock.layer())),
    )
  })

  test("the view renders the notice as a warn Toast with the typed dismiss intent, and nothing when clear", () => {
    const withNotice: DesktopShellState = { ...baseState(), commandNotice: "Keybindings are unavailable." }
    const toast = byKey(desktopShellView(withNotice), "desktop-command-notice") as {
      _tag?: string
      notification?: { tone?: string; title?: string; id?: string }
      onDismiss?: { name?: string }
    }
    expect(toast?._tag).toBe("Toast")
    expect(toast?.notification).toMatchObject({ tone: "warn", title: "Keybindings are unavailable." })
    expect(toast?.onDismiss?.name).toBe("DesktopCommandNoticeDismissed")

    expect(byKey(desktopShellView({ ...baseState(), commandNotice: null }), "desktop-command-notice")).toBeUndefined()
  })

  test("the DesktopCommandNoticeDismissed intent clears the notice through the real registry", async () => {
    await Effect.runPromise(
      Effect.gen(function* () {
        const state = yield* SubscriptionRef.make<DesktopShellState>({
          ...baseState(),
          commandNotice: "That command request was already handled. The duplicate was ignored.",
        })
        const registry = yield* makeIntentRegistry(desktopShellIntents, makeDesktopShellHandlers(state))

        const toast = byKey(desktopShellView(yield* SubscriptionRef.get(state)), "desktop-command-notice") as {
          onDismiss?: Parameters<typeof resolveIntentRef>[0]
        }
        yield* registry.dispatch(resolveIntentRef(toast.onDismiss!, null))
        expect((yield* SubscriptionRef.get(state)).commandNotice).toBeNull()
      }),
    )
  })
})
