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
  unavailableCodexSettingsBridge,
  type CodexSettingsBridge,
} from "./settings.ts"
import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
} from "./shell.ts"
import { openagentsDesktopTheme } from "./theme.ts"
import { type DesktopThread } from "../chat-contract.ts"
import { type DesktopWorkspaceFile, type DesktopWorkspaceSnapshot } from "../workspace-contract.ts"

/** Effect Schema at the preload boundary (issue #8574: Schema, not Zod). */
const DesktopBridgeSchema = Schema.Struct({
  host: Schema.String,
  platform: Schema.String,
})

type DesktopBridge = Readonly<{
  host: string
  platform: string
  stageFleet?: (value: unknown) => Promise<unknown>
  listThreads?: () => Promise<unknown>
  newThread?: () => Promise<unknown>
  openThread?: (value: unknown) => Promise<unknown>
  sendMessage?: (value: unknown) => Promise<unknown>
  workspaceSummary?: () => Promise<unknown>
  chooseWorkspace?: () => Promise<unknown>
  readWorkspaceFile?: (value: unknown) => Promise<unknown>
  codexAccounts?: () => Promise<unknown>
  codexConnectStart?: () => Promise<unknown>
  codexConnectStatus?: () => Promise<unknown>
  codexConnectOpenVerification?: () => Promise<unknown>
}>

const readBridge = (): DesktopBridge | undefined =>
  (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop

/**
 * Codex settings bridge over the preload surface. Each call degrades to the
 * honest unavailable projection when the bridge is absent; the settings
 * handlers schema-decode every response before it touches state.
 */
const codexSettingsBridge: CodexSettingsBridge = {
  listAccounts: () => {
    const bridge = readBridge()
    return typeof bridge?.codexAccounts === "function"
      ? bridge.codexAccounts()
      : unavailableCodexSettingsBridge.listAccounts()
  },
  connectStart: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectStart === "function"
      ? bridge.codexConnectStart()
      : unavailableCodexSettingsBridge.connectStart()
  },
  connectStatus: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectStatus === "function"
      ? bridge.codexConnectStatus()
      : unavailableCodexSettingsBridge.connectStatus()
  },
  openVerification: () => {
    const bridge = readBridge()
    return typeof bridge?.codexConnectOpenVerification === "function"
      ? bridge.codexConnectOpenVerification()
      : unavailableCodexSettingsBridge.openVerification()
  },
}

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
      }, {
        listThreads: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.listThreads?.()
          return Array.isArray(raw) ? raw as DesktopThread[] : []
        },
        newThread: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.newThread?.()
          return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
        },
        openThread: async (id) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.openThread?.({ id })
          return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
        },
        sendMessage: async (input) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.sendMessage?.(input)
          if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") return raw as { ok: boolean; thread?: DesktopThread | null; error?: string }
          return { ok: false, error: "Desktop chat returned an invalid response." }
        },
      }, {
        summary: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceSummary?.()
          return typeof raw === "object" && raw !== null && typeof (raw as { root?: unknown }).root === "string" ? raw as DesktopWorkspaceSnapshot : null
        },
        choose: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.chooseWorkspace?.()
          return typeof raw === "object" && raw !== null && typeof (raw as { root?: unknown }).root === "string" ? raw as DesktopWorkspaceSnapshot : null
        },
        readFile: async (path) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.readWorkspaceFile?.({ path })
          return typeof raw === "object" && raw !== null && typeof (raw as { content?: unknown }).content === "string" ? raw as DesktopWorkspaceFile : null
        },
      }, codexSettingsBridge),
    )
    const bridge = (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop
    const existing = typeof bridge?.listThreads === "function" ? yield* Effect.promise(bridge.listThreads) : []
    const threads = Array.isArray(existing) ? existing.filter((item): item is DesktopThread => typeof item === "object" && item !== null && typeof (item as { id?: unknown }).id === "string") : []
    if (threads.length > 0) yield* SubscriptionRef.update(state, (current) => ({ ...current, threads, activeThreadId: threads[0]!.id, notes: threads[0]!.notes }))
    const focusComposer = (): void => {
      // Let the controlled TextField render its cleared/enabled state first.
      window.setTimeout(() => {
        root.querySelector<HTMLInputElement>('[data-en-key="shell-input"] input')?.focus()
      }, 0)
    }
    const report: IntentReporter = (ref, runtimeValue) => {
      const shouldFocus = ref.name === "DesktopNewChat" || ref.name === "DesktopNoteSubmitted"
      return registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null)).pipe(
        Effect.ensuring(Effect.sync(() => {
          if (shouldFocus) focusComposer()
        })),
      )
    }
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
