/**
 * Renderer entrypoint (#8574): boots the OpenAgents Desktop shell as one
 * Effect Native program — SubscriptionRef state, `makeViewProgramFromState`,
 * a typed intent registry, and the DOM renderer from the shared vendored
 * catalog. It follows the standard Effect Native consumer pattern.
 *
 * Boundary: this file runs sandboxed (contextIsolation on, nodeIntegration
 * off). The only host input is the frozen `openagentsDesktop` bridge object
 * from the preload, decoded with Effect Schema — never trusted raw.
 */
import {
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  type IntentReporter,
} from "@effect-native/core"
import { Effect, Exit, Schema, Scope, SubscriptionRef } from "@effect-native/core/effect"
import { makeDomRenderer } from "@effect-native/render-dom"

import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
} from "./shell.ts"
import { openagentsDesktopTheme } from "./theme.ts"

/** Effect Schema at the preload boundary (issue #8574: Schema, not Zod). */
const DesktopBridgeSchema = Schema.Struct({
  host: Schema.String,
  platform: Schema.String,
})

type DesktopBridge = Readonly<{
  host: string
  platform: string
  stageFleet?: (value: unknown) => Promise<unknown>
}>

export const decodeBridgeHost = (bridge: unknown): string => {
  const decoded = Schema.decodeUnknownExit(DesktopBridgeSchema)(bridge)
  return Exit.isSuccess(decoded)
    ? `${decoded.value.host}/${decoded.value.platform}`
    : "unknown-host"
}

const mountDesktopShell = (root: HTMLElement, host: string) =>
  Effect.gen(function* () {
    const state = yield* SubscriptionRef.make(initialDesktopShellState(host))
    const program = makeViewProgramFromState(state, desktopShellView)
    const registry = yield* makeIntentRegistry(
      desktopShellIntents,
      makeDesktopShellHandlers(state, undefined, async (input) => {
        const bridge = (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop
        if (typeof bridge?.stageFleet !== "function") {
          return {
            state: "unavailable",
            message: "Local Pylon control is unavailable. No fleet work was dispatched.",
            intentStatus: null,
          }
        }
        const raw = await bridge.stageFleet(input)
        if (
          typeof raw === "object" && raw !== null &&
          (raw as { state?: unknown }).state !== undefined &&
          typeof (raw as { message?: unknown }).message === "string"
        ) {
          const value = raw as { state?: unknown; message: string; intentStatus?: unknown }
          if (value.state === "accepted" || value.state === "rejected" || value.state === "unavailable") {
            return {
              state: value.state,
              message: value.message,
              intentStatus: typeof value.intentStatus === "string" ? value.intentStatus : null,
            }
          }
        }
        return {
          state: "unavailable",
          message: "Local Pylon returned an invalid response. No fleet work was dispatched.",
          intentStatus: null,
        }
      }),
    )
    const report: IntentReporter = (ref, runtimeValue) =>
      registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null))
    const renderer = makeDomRenderer({ theme: openagentsDesktopTheme })
    yield* renderer.mount(root, program.viewStream, report)
  })

const boot = (): void => {
  const root = document.getElementById("openagents-desktop-root")
  if (root === null) return
  const host = decodeBridgeHost(
    (globalThis as { openagentsDesktop?: unknown }).openagentsDesktop,
  )
  const scope = Effect.runSync(Scope.make())
  window.addEventListener(
    "pagehide",
    () => {
      void Effect.runPromise(Scope.close(scope, Exit.void))
    },
    { once: true },
  )
  void Effect.runPromise(Scope.provide(scope)(mountDesktopShell(root, host))).catch(
    (error) => {
      console.error("[openagents-desktop] shell mount failed", error)
    },
  )
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", boot)
} else {
  boot()
}
