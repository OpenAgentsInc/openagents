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
  IntentRef,
  makeIntentRegistry,
  makeViewProgramFromState,
  resolveIntentRef,
  StaticPayload,
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
import {
  type DesktopWorkspaceFile,
  type DesktopWorkspaceGitDiff,
  type DesktopWorkspaceGitStatus,
  type DesktopWorkspaceSaveResult,
  type DesktopWorkspaceSnapshot,
} from "../workspace-contract.ts"

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
  hydrateThread?: (value: unknown) => Promise<unknown>
  sendMessage?: (value: unknown) => Promise<unknown>
  workspaceSummary?: () => Promise<unknown>
  chooseWorkspace?: () => Promise<unknown>
  readWorkspaceFile?: (value: unknown) => Promise<unknown>
  saveWorkspaceFile?: (value: unknown) => Promise<unknown>
  workspaceGitStatus?: () => Promise<unknown>
  workspaceGitDiff?: (value: unknown) => Promise<unknown>
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
        hydrateThread: async (id) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.hydrateThread?.({ id })
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
        saveFile: async (input) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.saveWorkspaceFile?.(input)
          if (typeof raw !== "object" || raw === null) {
            return { state: "unavailable", message: "Workspace save returned an invalid response." }
          }
          const value = raw as { state?: unknown; file?: unknown; message?: unknown }
          if (
            (value.state === "saved" || value.state === "conflict") &&
            typeof value.file === "object" && value.file !== null &&
            typeof (value.file as { content?: unknown }).content === "string" &&
            typeof (value.file as { revision?: unknown }).revision === "string"
          ) return value as DesktopWorkspaceSaveResult
          return {
            state: "unavailable",
            message: typeof value.message === "string" ? value.message : "Workspace save is unavailable.",
          }
        },
        gitStatus: async () => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceGitStatus?.()
          if (typeof raw !== "object" || raw === null) return { state: "unavailable" }
          const value = raw as { state?: unknown; changes?: unknown; truncated?: unknown }
          return value.state === "available" && Array.isArray(value.changes) && typeof value.truncated === "boolean"
            ? value as DesktopWorkspaceGitStatus
            : { state: "unavailable" }
        },
        gitDiff: async (path) => {
          const raw = await (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop?.workspaceGitDiff?.({ path })
          if (typeof raw !== "object" || raw === null) return { state: "unavailable", message: "Git review is unavailable." }
          const value = raw as { state?: unknown; path?: unknown; content?: unknown; truncated?: unknown; message?: unknown }
          if (value.state === "available" && typeof value.path === "string" && typeof value.content === "string" && typeof value.truncated === "boolean") {
            return value as DesktopWorkspaceGitDiff
          }
          return { state: "unavailable", message: typeof value.message === "string" ? value.message : "Git review is unavailable." }
        },
      }, codexSettingsBridge),
    )
    const bridge = (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop
    const existing = typeof bridge?.listThreads === "function" ? yield* Effect.promise(bridge.listThreads) : []
    const threads = Array.isArray(existing) ? existing.filter((item): item is DesktopThread => typeof item === "object" && item !== null && typeof (item as { id?: unknown }).id === "string") : []
    if (threads.length > 0) {
      const first = threads[0]!
      yield* SubscriptionRef.update(state, (current) => ({
        ...current,
        threads,
        activeThreadId: first.id,
        notes: [],
      }))
    }
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
    const onCommandPaletteShortcut = (event: KeyboardEvent): void => {
      const target = event.target
      const editable = target instanceof HTMLElement &&
        target.closest("input, textarea, [contenteditable='true']") !== null
      if (
        event.defaultPrevented ||
        editable ||
        event.key.toLowerCase() !== "k" ||
        (!event.metaKey && !event.ctrlKey)
      ) return
      event.preventDefault()
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopCommandPaletteToggled", StaticPayload(null)))),
      )
    }
    window.addEventListener("keydown", onCommandPaletteShortcut)
    window.addEventListener("pagehide", () => {
      window.removeEventListener("keydown", onCommandPaletteShortcut)
    }, { once: true })
    const renderer = makeDomRenderer({ theme: openagentsDesktopTheme })
    yield* renderer.mount(root, program.viewStream, report)
    // First paint must never wait on local rollout parsing. The sidebar gets
    // metadata immediately; the selected thread receives five recent messages
    // and then its bounded expanded tail after the DOM is already visible.
    if (threads.length > 0 && typeof bridge?.openThread === "function") {
      const id = threads[0]!.id
      window.setTimeout(() => {
        void (async () => {
          const detail = await bridge.openThread!({ id })
          if (typeof detail === "object" && detail !== null && typeof (detail as { id?: unknown }).id === "string") {
            const selected = detail as DesktopThread
            await Effect.runPromise(SubscriptionRef.update(state, current => current.activeThreadId === id
              ? { ...current, threads: [selected, ...current.threads.filter(thread => thread.id !== id)], notes: selected.notes }
              : current))
          }
          if (typeof bridge.hydrateThread !== "function") return
          const hydrated = await bridge.hydrateThread({ id })
          if (typeof hydrated === "object" && hydrated !== null && typeof (hydrated as { id?: unknown }).id === "string") {
            const expanded = hydrated as DesktopThread
            await Effect.runPromise(SubscriptionRef.update(state, current => current.activeThreadId === id
              ? { ...current, threads: [expanded, ...current.threads.filter(thread => thread.id !== id)], notes: expanded.notes }
              : current))
          }
        })()
      }, 0)
    }
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
