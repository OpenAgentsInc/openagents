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
import { makeDomRenderer, makeStubCodeEditorDriver } from "@effect-native/render-dom"
import { projectFleetCockpitCard, type FleetAuthority } from "../fleet-cockpit.ts"

import {
  unavailableCodexSettingsBridge,
  unavailableMcpConfigSettingsBridge,
  unavailableProviderAccountsSettingsBridge,
  decodeOpenAgentsSessionView,
  type CodexSettingsBridge,
  type McpConfigSettingsBridge,
  unavailablePluginConfigSettingsBridge,
  type PluginConfigSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
} from "./settings.ts"
import type { FableLocalMcpServerConfig } from "../fable-local-contract.ts"
import {
  unavailableFleetAccountsBridge,
  type FleetAccountsBridge,
} from "./fleet-workspace.ts"
import {
  unavailableTerminalBridge,
  type TerminalRendererBridge,
} from "./terminal-workspace.ts"
import type { TerminalEvent } from "../terminal-contract.ts"
import {
  unavailableGitGithubBridge,
  type GitGithubBridge,
} from "./git-panel.ts"
import {
  unavailableWorkspaceBrowserBridge,
  type WorkspaceBrowserBridge,
} from "./workspace-browser.ts"
import {
  decodeWorkspaceEditorRecoverySnapshot,
  unavailableWorkspaceDocumentBridge,
  workspaceEditorRecoverySnapshot,
  type WorkspaceDocumentBridge,
} from "./workspace-editor.ts"
import {
  desktopShellIntents,
  desktopShellView,
  initialDesktopShellState,
  makeDesktopShellHandlers,
} from "./shell.ts"
import { makeCommandNoticeController } from "./command-notice.ts"
import { historyRestoreFetchPlan, restorableHistoryThreadRef } from "./history-restore.ts"
import {
  migrateDesktopPreferences,
  type DesktopPreferences,
} from "../desktop-preferences-contract.ts"
import { preferencesRootAttributes, themeForPreferences } from "../desktop-preferences-effects.ts"
import { selectDesktopChatHostSelection } from "./runtime-conversation.ts"
import { answerDesktopRuntimeInteraction, makeDesktopRuntimeInteractionHost } from "./runtime-interactions.ts"
import {
  makeLocalHarnessChatHost,
  type FableLocalRendererBridge,
} from "./local-harness.ts"
import { withHarnessLanes, type DesktopWorkspaceName, type HarnessLanes } from "./shell.ts"
import {
  decodeFableLocalAvailability,
  type FableLocalAvailability,
  type FableLocalEventEnvelope,
  type FableLocalImageAttachment,
} from "../fable-local-contract.ts"
import { readImageFile, composerImageRejectionMessage } from "./composer-images.ts"
import {
  codexHarnessLaneFromAvailability,
  decodeCodexLocalAvailability,
  type CodexLocalAvailability,
} from "../codex-local-contract.ts"
import { type DesktopThread } from "../chat-contract.ts"
import type { DesktopWorkspaceChange } from "../workspace-contract.ts"
import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import type { CodexHistoryCatalog, CodexHistoryPage, CodexHistorySearchResponse } from "../codex-history-contract.ts"
import { historyAgentTraversalTarget, historyCatalogPageSize, historyItemPageSize, historyPrependScrollTop, historyShouldFetchNewer, historyShouldFetchOlder, isHistoryAgentTraversalShortcut } from "./history-workspace.ts"
import {
  decodeDesktopCodingCatalogProjection,
  desktopWorkspaceForCodingFocus,
  emptyDesktopCodingCatalogProjection,
  type DesktopCodingCatalogProjection,
} from "../coding-catalog-contract.ts"
import {
  decodeDesktopCommandBindingProjectionOrNull,
  desktopCanonicalCommandRegistry,
  type DesktopCommandBindingProjection,
  type DesktopCommandId,
  type DesktopDeferredCommand,
} from "../desktop-command-contract.ts"
import { resolveDesktopDeferredCommandIntent } from "./command-registry.ts"
import {
  handleComposerShiftTab,
  isShellComposerInputTarget,
} from "./composer-shortcuts.ts"

/** Effect Schema at the preload boundary (issue #8574: Schema, not Zod). */
const DesktopBridgeSchema = Schema.Struct({
  host: Schema.String,
  platform: Schema.String,
})

type DesktopBridge = Readonly<{
  host: string
  platform: string
  runtimeRequest?: (value: unknown) => Promise<DesktopRuntimeGatewayResponse>
  runtimeSubscribe?: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  stageFleet?: (value: unknown) => Promise<unknown>
  listThreads?: () => Promise<unknown>
  newThread?: () => Promise<unknown>
  openThread?: (value: unknown) => Promise<unknown>
  hydrateThread?: (value: unknown) => Promise<unknown>
  sendMessage?: (value: unknown) => Promise<unknown>
  chooseWorkspace?: () => Promise<unknown>
  workspaceTree?: (value: unknown) => Promise<unknown>
  workspaceSearch?: (value: unknown) => Promise<unknown>
  cancelWorkspaceSearch?: (value: unknown) => Promise<unknown>
  createWorkspaceEntry?: (value: unknown) => Promise<unknown>
  renameWorkspaceEntry?: (value: unknown) => Promise<unknown>
  deleteWorkspaceEntry?: (value: unknown) => Promise<unknown>
  revealWorkspaceEntry?: (value: unknown) => Promise<unknown>
  openWorkspaceDocument?: (value: unknown) => Promise<unknown>
  saveWorkspaceDocument?: (value: unknown) => Promise<unknown>
  saveWorkspaceDocumentAs?: (value: unknown) => Promise<unknown>
  refreshWorkspace?: () => Promise<unknown>
  workspaceSubscribe?: (listener: (change: DesktopWorkspaceChange) => void) => () => void
  codexAccounts?: () => Promise<unknown>
  codexConnectStart?: () => Promise<unknown>
  codexReconnectStart?: (ref: string) => Promise<unknown>
  codexConnectStatus?: () => Promise<unknown>
  codexConnectOpenVerification?: () => Promise<unknown>
  providerAccounts?: Readonly<{
    list?: () => Promise<unknown>
    usage?: (ref: string) => Promise<unknown>
  }>
  fableLocal?: Readonly<{
    availability?: () => Promise<unknown>
    start?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
    /** FROZEN question-answer bridge (EP250) — ships with the runtime lane. */
    answerQuestion?: (value: unknown) => Promise<unknown>
    /** EP250 wave-2 runtime-capability channels (G4 child steer, A3 queue). */
    steerChild?: (value: unknown) => Promise<unknown>
    queueFollowup?: (value: unknown) => Promise<unknown>
    /** Image file picker (capability I1) — main-mediated, returns attachments. */
    pickImages?: () => Promise<ReadonlyArray<FableLocalImageAttachment>>
  }>
  /** Codex local lane (EP250 codex-first-class): same bridge shape. */
  codexLocal?: Readonly<{
    availability?: () => Promise<unknown>
    start?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
    steerChild?: (value: unknown) => Promise<unknown>
    queueFollowup?: (value: unknown) => Promise<unknown>
  }>
  usageLedger?: Readonly<{
    snapshot?: () => Promise<unknown>
    onEvent?: (listener: (snapshot: unknown) => void) => () => void
  }>
  codingCatalog?: Readonly<{
    snapshot?: () => Promise<unknown>
    choose?: () => Promise<unknown>
    open?: (value: unknown) => Promise<unknown>
    archive?: (value: unknown) => Promise<unknown>
    recover?: (value: unknown) => Promise<unknown>
  }>
  commands?: Readonly<{
    onCommand?: (listener: (command: DesktopDeferredCommand) => void) => () => void
    ready?: () => Promise<unknown>
    bindings?: () => Promise<unknown>
    saveBinding?: (value: unknown) => Promise<unknown>
    resetBindings?: () => Promise<unknown>
  }>
  /** Typed Git/GitHub surface (EP250 E2–E5): one namespaced invoke. */
  gitGithub?: Readonly<{ run?: (value: unknown) => Promise<unknown> }>
  /** Workspace-bounded PTY terminals (CUT-20, #8700). */
  terminal?: Readonly<{
    create?: (value: unknown) => Promise<unknown>
    input?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    restart?: (value: unknown) => Promise<unknown>
    close?: (value: unknown) => Promise<unknown>
    snapshot?: () => Promise<unknown>
    openPreview?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (event: TerminalEvent) => void) => () => void
  }>
  mcpConfig?: Readonly<{
    list?: () => Promise<unknown>
    add?: (value: unknown) => Promise<unknown>
    remove?: (value: unknown) => Promise<unknown>
    toggle?: (value: unknown) => Promise<unknown>
  }>
  pluginConfig?: Readonly<{
    list?: () => Promise<unknown>
    choose?: () => Promise<unknown>
    toggle?: (value: unknown) => Promise<unknown>
    remove?: (value: unknown) => Promise<unknown>
  }>
  /** Typed durable preferences (CUT-24 #8704). */
  preferences?: Readonly<{
    get?: () => Promise<unknown>
    update?: (value: unknown) => Promise<unknown>
    reset?: () => Promise<unknown>
  }>
  /** Diagnostics / watchdog (CUT-24 #8704). */
  diagnostics?: Readonly<{
    gather?: () => Promise<unknown>
    exportRedacted?: () => Promise<unknown>
    runAction?: (value: unknown) => Promise<unknown>
  }>
}>

const readBridge = (): DesktopBridge | undefined =>
  (globalThis as { openagentsDesktop?: DesktopBridge }).openagentsDesktop

/**
 * Load the durable preferences via the bridge, defensively migrated so a
 * missing preload or a malformed payload resolves to valid defaults (identity
 * theme, OS-deferred motion). CUT-24 #8704.
 */
const loadDesktopPreferences = async (): Promise<DesktopPreferences> => {
  const raw = await (readBridge()?.preferences?.get?.() ?? Promise.resolve(undefined)).catch(() => undefined)
  return migrateDesktopPreferences(raw).preferences
}

const workspaceBrowserBridge: WorkspaceBrowserBridge = {
  workspaceTree: (value) => readBridge()?.workspaceTree?.(value) ?? unavailableWorkspaceBrowserBridge.workspaceTree(value),
  workspaceSearch: (value) => readBridge()?.workspaceSearch?.(value) ?? unavailableWorkspaceBrowserBridge.workspaceSearch(value),
  cancelWorkspaceSearch: (value) => readBridge()?.cancelWorkspaceSearch?.(value) ?? unavailableWorkspaceBrowserBridge.cancelWorkspaceSearch(value),
  createWorkspaceEntry: (value) => readBridge()?.createWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.createWorkspaceEntry(value),
  renameWorkspaceEntry: (value) => readBridge()?.renameWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.renameWorkspaceEntry(value),
  deleteWorkspaceEntry: (value) => readBridge()?.deleteWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.deleteWorkspaceEntry(value),
  revealWorkspaceEntry: (value) => readBridge()?.revealWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.revealWorkspaceEntry(value),
  refreshWorkspace: () => readBridge()?.refreshWorkspace?.() ?? unavailableWorkspaceBrowserBridge.refreshWorkspace(),
}

const workspaceDocumentBridge: WorkspaceDocumentBridge = {
  openWorkspaceDocument: (value) => readBridge()?.openWorkspaceDocument?.(value) ?? unavailableWorkspaceDocumentBridge.openWorkspaceDocument(value),
  saveWorkspaceDocument: (value) => readBridge()?.saveWorkspaceDocument?.(value) ?? unavailableWorkspaceDocumentBridge.saveWorkspaceDocument(value),
  saveWorkspaceDocumentAs: (value) => readBridge()?.saveWorkspaceDocumentAs?.(value) ?? unavailableWorkspaceDocumentBridge.saveWorkspaceDocumentAs(value),
}

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
  reconnectStart: (ref: string) => {
    const bridge = readBridge()
    return typeof bridge?.codexReconnectStart === "function"
      ? bridge.codexReconnectStart(ref)
      : unavailableCodexSettingsBridge.reconnectStart!(ref)
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

/**
 * Fleet accounts bridge over the preload surface. Each call degrades to the
 * honest unavailable projection when the bridge is absent; the fleet handlers
 * schema-decode every response before it touches state.
 */
const fleetAccountsBridge: FleetAccountsBridge = {
  list: () => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.list === "function"
      ? bridge.providerAccounts.list()
      : unavailableFleetAccountsBridge.list()
  },
  usage: (ref) => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.usage === "function"
      ? bridge.providerAccounts.usage(ref)
      : unavailableFleetAccountsBridge.usage(ref)
  },
  // Session usage ledger snapshot (#8712 Lane C): absent-bridge hosts simply
  // render no Session usage section (the fleet decode drops a null).
  ledger: () => {
    const bridge = readBridge()
    return typeof bridge?.usageLedger?.snapshot === "function"
      ? bridge.usageLedger.snapshot()
      : Promise.resolve(null)
  },
  cockpit: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return { authority: "unknown", cards: [] }
    const catalog = await bridge.runtimeRequest({ kind: "query", requestId: `fleet-catalog-${Date.now()}`, query: { id: "conversation.catalog" } }).catch(() => null)
    if (catalog === null || catalog.kind !== "conversation_catalog") return { authority: "unknown", cards: [] }
    const authority: FleetAuthority = catalog.status.phase === "live" ? "live"
      : catalog.status.phase === "denied" ? "revoked"
        : catalog.status.phase === "idle" ? "unknown" : "stale"
    const cards = (await Promise.all(catalog.threads.slice(0, 50).map(async (thread: { threadRef: string; title: string }) => {
      const [timeline, interactions] = await Promise.all([
        bridge.runtimeRequest!({ kind: "query", requestId: `fleet-timeline-${thread.threadRef}`, query: { id: "conversation.timeline", threadRef: thread.threadRef } }).catch(() => null),
        bridge.runtimeRequest!({ kind: "query", requestId: `fleet-interactions-${thread.threadRef}`, query: { id: "runtime.interactions", threadRef: thread.threadRef } }).catch(() => null),
      ])
      if (timeline === null || timeline.kind !== "conversation_timeline" || timeline.run === null) return null
      const agentRefs = timeline.events.flatMap((event: { item?: { kind?: string; agentRef?: string } | null }) => event.item?.kind === "agent" && typeof event.item.agentRef === "string" ? [event.item.agentRef] : [])
      const receiptRefs = timeline.events.flatMap((event: { artifactRefs?: ReadonlyArray<string> }) => event.artifactRefs ?? [])
      return projectFleetCockpitCard({
        threadRef: thread.threadRef,
        title: thread.title,
        authority,
        run: timeline.run,
        interactions: interactions?.kind === "runtime_interactions" ? interactions.interactions : [],
        agentRefs,
        receiptRefs,
        ...(timeline.run.workContextRef === undefined ? {} : { repositoryRef: timeline.run.workContextRef }),
      })
    }))).filter((card): card is NonNullable<typeof card> => card !== null)
    return { authority, cards }
  },
  control: async (command) => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    const id = command.action === "resume" ? "conversation.continue"
      : command.action === "retry" ? "conversation.retry"
        : command.action === "close" ? "conversation.close"
          : "conversation.interrupt"
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `fleet-${command.action}-${command.runRef}-${command.expectedVersion}`,
      command: {
        id,
        commandRef: `fleet.${command.action}.${command.runRef}.${command.expectedVersion}`,
        threadRef: command.threadRef,
        runRef: command.runRef,
        expectedVersion: command.expectedVersion,
      },
    })
  },
  decideAttention: async (command) => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    const suffix = globalThis.crypto.randomUUID().replace(/[^A-Za-z0-9._:]/g, "")
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `fleet-attention-${suffix}`,
      command: {
        id: "runtime.decideInteraction",
        interactionRef: command.interactionRef,
        threadRef: command.threadRef,
        turnRef: command.runRef,
        envelope: {
          decisionRef: `decision.desktop.${suffix}`,
          idempotencyKey: `idem.desktop.${suffix}`,
          decidedAt: new Date().toISOString(),
          surface: "desktop",
          decision: { kind: "tool_approval", outcome: command.action },
        },
      },
    })
  },
}

/**
 * Typed Git/GitHub bridge (#8712 E2–E5) over the preload gitGithub surface.
 * A single namespaced invoke carries the closed operation set; the git-panel
 * handlers schema-decode every response before it touches state. An absent
 * bridge degrades to the typed no_workspace error.
 */
const gitGithubBridge: GitGithubBridge = {
  run: (value: unknown) => {
    const bridge = readBridge()
    return typeof bridge?.gitGithub?.run === "function"
      ? bridge.gitGithub.run(value)
      : unavailableGitGithubBridge.run(value)
  },
}

const terminalRendererBridge: TerminalRendererBridge = {
  create: (value) => readBridge()?.terminal?.create?.(value) ?? unavailableTerminalBridge.create(value),
  input: (value) => readBridge()?.terminal?.input?.(value) ?? unavailableTerminalBridge.input(value),
  interrupt: (value) => readBridge()?.terminal?.interrupt?.(value) ?? unavailableTerminalBridge.interrupt(value),
  restart: (value) => readBridge()?.terminal?.restart?.(value) ?? unavailableTerminalBridge.restart(value),
  close: (value) => readBridge()?.terminal?.close?.(value) ?? unavailableTerminalBridge.close(value),
  snapshot: () => readBridge()?.terminal?.snapshot?.() ?? unavailableTerminalBridge.snapshot(),
  openPreview: (value) => readBridge()?.terminal?.openPreview?.(value) ?? unavailableTerminalBridge.openPreview(value),
}

/**
 * Settings Claude-accounts bridge over the same preload providerAccounts
 * surface. Degrades to the explicit unavailable projection when the bridge
 * is absent; the settings handlers schema-decode every response.
 */
const providerAccountsSettingsBridge: ProviderAccountsSettingsBridge = {
  list: () => {
    const bridge = readBridge()
    return typeof bridge?.providerAccounts?.list === "function"
      ? bridge.providerAccounts.list()
      : unavailableProviderAccountsSettingsBridge.list()
  },
}

/**
 * User-configured MCP servers bridge (I2, EP250 wave-2) over the preload
 * surface. Add sends the typed config to main (secret values cross once);
 * every response is schema-decoded in the settings handlers before it touches
 * state, and the projection carries no secret values back.
 */
const mcpConfigSettingsBridge: McpConfigSettingsBridge = {
  list: () => {
    const bridge = readBridge()
    return typeof bridge?.mcpConfig?.list === "function"
      ? bridge.mcpConfig.list()
      : unavailableMcpConfigSettingsBridge.list()
  },
  add: (config: FableLocalMcpServerConfig) => {
    const bridge = readBridge()
    return typeof bridge?.mcpConfig?.add === "function"
      ? bridge.mcpConfig.add(config)
      : unavailableMcpConfigSettingsBridge.add(config)
  },
  remove: (name: string) => {
    const bridge = readBridge()
    return typeof bridge?.mcpConfig?.remove === "function"
      ? bridge.mcpConfig.remove({ name })
      : unavailableMcpConfigSettingsBridge.remove(name)
  },
  toggle: (name: string, enabled: boolean) => {
    const bridge = readBridge()
    return typeof bridge?.mcpConfig?.toggle === "function"
      ? bridge.mcpConfig.toggle({ name, enabled })
      : unavailableMcpConfigSettingsBridge.toggle(name, enabled)
  },
}
const pluginConfigSettingsBridge: PluginConfigSettingsBridge = {
  list: () => readBridge()?.pluginConfig?.list?.() ?? unavailablePluginConfigSettingsBridge.list(),
  choose: () => readBridge()?.pluginConfig?.choose?.() ?? unavailablePluginConfigSettingsBridge.choose(),
  toggle: (ref, enabled) => readBridge()?.pluginConfig?.toggle?.({ ref, enabled }) ?? unavailablePluginConfigSettingsBridge.toggle(ref, enabled),
  remove: ref => readBridge()?.pluginConfig?.remove?.({ ref }) ?? unavailablePluginConfigSettingsBridge.remove(ref),
}

let sessionRequestSequence = 0
const openAgentsSessionSettingsBridge: OpenAgentsSessionSettingsBridge = {
  status: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "query",
      requestId: `renderer-session-status-${++sessionRequestSequence}`,
      query: { id: "runtime.bootstrap" },
    })
  },
  signIn: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `renderer-session-sign-in-${++sessionRequestSequence}`,
      command: { id: "session.sign_in" },
    })
  },
  signOut: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `renderer-session-sign-out-${++sessionRequestSequence}`,
      command: { id: "session.sign_out" },
    })
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
    const bridge = readBridge()
    const recoveryStorageKey = (workspaceSessionRef: string): string =>
      `openagents.desktop.workspace-editor.v2.${workspaceSessionRef}`
    const loadWorkspaceRecovery = (workspaceSessionRef: string) => {
      try {
        const raw = localStorage.getItem(recoveryStorageKey(workspaceSessionRef))
        return raw === null ? null : decodeWorkspaceEditorRecoverySnapshot(JSON.parse(raw))
      } catch {
        return null
      }
    }
    const saveWorkspaceRecovery = (
      workspaceSessionRef: string,
      snapshot: ReturnType<typeof workspaceEditorRecoverySnapshot>,
    ): void => {
      try {
        localStorage.setItem(recoveryStorageKey(workspaceSessionRef), JSON.stringify(snapshot))
      } catch { /* bounded recovery is best effort */ }
    }
    const localChat = {
      listThreads: async () => {
        const raw = await bridge?.listThreads?.()
        return Array.isArray(raw) ? raw as DesktopThread[] : []
      },
      newThread: async () => {
        const raw = await bridge?.newThread?.()
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      openThread: async (id: string) => {
        const raw = await bridge?.openThread?.({ id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      hydrateThread: async (id: string) => {
        const raw = await bridge?.hydrateThread?.({ id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      sendMessage: async (input: Readonly<{ id: string; message: string }>) => {
        const raw = await bridge?.sendMessage?.(input)
        if (typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean") return raw as { ok: boolean; thread?: DesktopThread | null; error?: string }
        return { ok: false, error: "Desktop chat returned an invalid response." }
      },
    }
    // Fable local lane (#8712): narrow bridge over the preload surface. The
    // local-mode chat host routes "fable" through it and refuses "codex"
    // explicitly — never the legacy cloud gateway (no silent substitution).
    const fableLocalBridge: FableLocalRendererBridge | null =
      typeof bridge?.fableLocal?.start === "function" &&
      typeof bridge.fableLocal.availability === "function" &&
      typeof bridge.fableLocal.interrupt === "function" &&
      typeof bridge.fableLocal.onEvent === "function"
        ? {
            availability: bridge.fableLocal.availability,
            start: bridge.fableLocal.start,
            interrupt: bridge.fableLocal.interrupt,
            onEvent: bridge.fableLocal.onEvent as (
              listener: (envelope: FableLocalEventEnvelope) => void,
            ) => () => void,
            // EP250 wave-2: additive child-steer (G4) + queue-followup (A3)
            // channels; absent on older preloads (the shell degrades to no-op).
            ...(typeof bridge.fableLocal.steerChild === "function"
              ? { steerChild: bridge.fableLocal.steerChild }
              : {}),
            ...(typeof bridge.fableLocal.queueFollowup === "function"
              ? { queueFollowup: bridge.fableLocal.queueFollowup }
              : {}),
          }
        : null
    // Interactive question cards (EP250): the FROZEN answer bridge is
    // fableLocal.answerQuestion({ turnRef, questionRef, answers }) with
    // answers as [{ question, labels }]. Defensive: if the bridge is absent,
    // cards render read-only pending (evidence-gated).
    const answerQuestion = bridge?.fableLocal?.answerQuestion
    const localQuestionHost = typeof answerQuestion === "function"
      ? { answer: (input: Readonly<{ turnRef: string; questionRef: string; answers: ReadonlyArray<{ readonly question: string; readonly labels: ReadonlyArray<string> }> }>) => answerQuestion({ turnRef: input.turnRef, questionRef: input.questionRef, answers: input.answers }) }
      : { answer: null }
    let fableAvailability: FableLocalAvailability | null = null
    // Codex local lane (EP250): same narrow bridge shape over its own
    // channels; the local-mode chat host routes "codex" through it over
    // PROBE-VERIFIED evidence only — never the legacy cloud gateway.
    const codexLocalBridge: FableLocalRendererBridge | null =
      typeof bridge?.codexLocal?.start === "function" &&
      typeof bridge.codexLocal.availability === "function" &&
      typeof bridge.codexLocal.interrupt === "function" &&
      typeof bridge.codexLocal.onEvent === "function"
        ? {
            availability: bridge.codexLocal.availability,
            start: bridge.codexLocal.start,
            interrupt: bridge.codexLocal.interrupt,
            onEvent: bridge.codexLocal.onEvent as (
              listener: (envelope: FableLocalEventEnvelope) => void,
            ) => () => void,
            ...(typeof bridge.codexLocal.steerChild === "function"
              ? { steerChild: bridge.codexLocal.steerChild }
              : {}),
            ...(typeof bridge.codexLocal.queueFollowup === "function"
              ? { queueFollowup: bridge.codexLocal.queueFollowup }
              : {}),
          }
        : null
    let codexAvailability: CodexLocalAvailability | null = null
    const localHarnessChat = makeLocalHarnessChatHost({
      base: localChat,
      fable: fableLocalBridge,
      fableAvailability: () => fableAvailability,
      codex: codexLocalBridge,
      codexAvailability: () => codexAvailability,
    })
    const selection = yield* Effect.promise(() => selectDesktopChatHostSelection({
      request: bridge?.runtimeRequest,
      subscribe: bridge?.runtimeSubscribe,
      local: localHarnessChat,
    }))
    const chat = selection.host
    const runtimeInteractionHost = typeof bridge?.runtimeRequest === "function"
      ? makeDesktopRuntimeInteractionHost({
          request: bridge.runtimeRequest,
          subscribe: bridge.runtimeSubscribe,
        })
      : null
    const questionHost = selection.mode === "runtime" && runtimeInteractionHost !== null
      ? {
          answer: async (input: Readonly<{
            turnRef: string
            threadRef?: string
            questionRef: string
            answers: ReadonlyArray<{ readonly question: string; readonly labels: ReadonlyArray<string> }>
          }>): Promise<boolean> => {
            return answerDesktopRuntimeInteraction(runtimeInteractionHost, input)
          },
        }
      : localQuestionHost
    const codingCatalogCall = async (
      call: (() => Promise<unknown>) | undefined,
    ): Promise<DesktopCodingCatalogProjection> => {
      if (call === undefined) return emptyDesktopCodingCatalogProjection()
      try {
        return decodeDesktopCodingCatalogProjection(await call()) ?? emptyDesktopCodingCatalogProjection()
      } catch {
        return emptyDesktopCodingCatalogProjection()
      }
    }
    const codingCatalogHost = {
      snapshot: () => codingCatalogCall(bridge?.codingCatalog?.snapshot),
      choose: () => codingCatalogCall(bridge?.codingCatalog?.choose),
      open: (sessionRef: string) => codingCatalogCall(
        bridge?.codingCatalog?.open === undefined
          ? undefined
          : () => bridge.codingCatalog!.open!({ sessionRef }),
      ),
      archive: (sessionRef: string) => codingCatalogCall(
        bridge?.codingCatalog?.archive === undefined
          ? undefined
          : () => bridge.codingCatalog!.archive!({ sessionRef }),
      ),
      recover: (sessionRef: string) => codingCatalogCall(
        bridge?.codingCatalog?.recover === undefined
          ? undefined
          : () => bridge.codingCatalog!.recover!({ sessionRef }),
      ),
    }
    const decodeCommandBindings = (value: unknown): DesktopCommandBindingProjection | null =>
      decodeDesktopCommandBindingProjectionOrNull(value)
    const commandBindingHost = {
      snapshot: async () => decodeCommandBindings(await bridge?.commands?.bindings?.()),
      save: async (input: Readonly<{ commandId: DesktopCommandId; chord: string | null }>) =>
        decodeCommandBindings(await bridge?.commands?.saveBinding?.(input)),
      reset: async () => decodeCommandBindings(await bridge?.commands?.resetBindings?.()),
    }
    // Evidence-gated composer lanes (#8712), resolved BEFORE first mount so
    // the chips never flash an unproven state.
    if (fableLocalBridge !== null && selection.mode === "local") {
      const rawAvailability = yield* Effect.promise(() =>
        fableLocalBridge.availability().catch(() => null))
      fableAvailability = decodeFableLocalAvailability(rawAvailability)
    }
    // Composer lanes (EP250 chip-verified-evidence rule): the Codex chip
    // lights only on a PROBE-VERIFIED account. The preflight probe is a real
    // (bounded) codex turn per account, so first mount is never blocked on
    // it: the chip starts as "verifying…" and the availability promise
    // updates the lane state as soon as the session probe lands.
    const localLanes = (): HarnessLanes => ({
      fable: fableAvailability?.state === "available"
        ? { available: true, reason: null }
        : { available: false, reason: "Fable — unavailable: no linked Claude account" },
      codex: codexLocalBridge === null
        ? { available: false, reason: "Codex — no verified account · Reconnect in Settings" }
        : codexHarnessLaneFromAvailability(codexAvailability),
    })
    const harnessLanes: HarnessLanes = selection.mode === "runtime"
      ? {
          fable: { available: true, reason: null },
          codex: { available: true, reason: null },
        }
      : localLanes()
    yield* SubscriptionRef.update(state, current => withHarnessLanes(current, harnessLanes))
    if (selection.mode === "local" && codexLocalBridge !== null) {
      // Non-blocking: the probe round can take tens of seconds on broken
      // accounts; the shell mounts immediately and the chip updates when
      // session-scoped probe evidence lands.
      void codexLocalBridge.availability()
        .catch(() => null)
        .then(raw => {
          // A failed decode is honest non-evidence: the chip settles on the
          // reconnect reason rather than parking on "verifying…" forever.
          codexAvailability = decodeCodexLocalAvailability(raw) ??
            { state: "unavailable", reason: "no_verified_account" }
          return Effect.runPromise(
            SubscriptionRef.update(state, current => withHarnessLanes(current, localLanes())),
          )
        })
        .catch(() => {})
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      questionAnswerHostAvailable: questionHost.answer !== null,
    }))
    let historyRequestSequence = 0
    const restoreHistory = (): { selectedThreadRef:string;offset:number;selectedItemRef:string|null;railCollapsed:boolean;expandedThreadRefs:ReadonlyArray<string> } | null => { try { const value=JSON.parse(localStorage.getItem("openagents.desktop.history.v1")??"null");return value&&typeof value.selectedThreadRef==="string"&&Number.isInteger(value.offset)&&value.offset>=0&&value.offset<=1_000_000&&typeof value.railCollapsed==="boolean"&&(value.selectedItemRef===null||typeof value.selectedItemRef==="string")&&Array.isArray(value.expandedThreadRefs)&&value.expandedThreadRefs.every((ref:unknown)=>typeof ref==="string")?value:null } catch{return null} }
    const historyHost = {
      catalog: async (): Promise<CodexHistoryCatalog | null> => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({ kind: "query", requestId: `renderer-history-catalog-${++historyRequestSequence}`, query: { id: "codex.history.catalog" } })
        return response.kind === "codex_history_catalog" ? response.catalog : null
      },
      page: async (threadRef: string, offset: number, limit: number): Promise<CodexHistoryPage | null> => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({ kind: "query", requestId: `renderer-history-page-${++historyRequestSequence}`, query: { id: "codex.history.page", threadRef, offset, limit } })
        return response.kind === "codex_history_page" ? response.page : null
      },
      search: async (query: string, limit: number): Promise<CodexHistorySearchResponse | null> => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({ kind: "query", requestId: `renderer-history-search-${++historyRequestSequence}`, query: { id: "codex.history.search", query, limit } })
        return response.kind === "codex_history_search" ? response.search : null
      },
      save: (value: any): void => { try { localStorage.setItem("openagents.desktop.history.v1",JSON.stringify({...value,expandedThreadRefs:Array.isArray(value?.expandedThreadRefs)?value.expandedThreadRefs:[]})) } catch { /* restoration is best effort and contains refs only */ } },
    }
    // One shared transient command-notice controller: threaded into the shell
    // handlers AND the deferred-command dispatch below so both paths cancel one
    // another's pending auto-clear. shutdown is a scope finalizer so a pending
    // clear fiber can never fire after the renderer unmounts.
    const noticeController = makeCommandNoticeController(state)
    yield* Effect.addFinalizer(() => noticeController.shutdown)
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
      }, chat, {
        choose: async () => (await readBridge()?.chooseWorkspace?.()) === true,
        browser: workspaceBrowserBridge,
        documents: workspaceDocumentBridge,
        recovery: {
          load: loadWorkspaceRecovery,
          save: saveWorkspaceRecovery,
        },
      }, codexSettingsBridge, undefined, openAgentsSessionSettingsBridge, historyHost, fleetAccountsBridge, providerAccountsSettingsBridge, codingCatalogHost, questionHost, commandBindingHost, {
        toggleFullScreen: async () => {
          const raw = await (globalThis as { openagentsDesktop?: { toggleFullScreen?: () => Promise<boolean> } }).openagentsDesktop?.toggleFullScreen?.()
          return raw === true
        },
      }, gitGithubBridge, mcpConfigSettingsBridge, pluginConfigSettingsBridge, {
        // Image file picker (capability I1): main-mediated native dialog. Absent
        // bridge degrades to no attachments (drop/paste still work in-renderer).
        pick: async () => {
          const pick = readBridge()?.fableLocal?.pickImages
          return typeof pick === "function" ? await pick() : []
        },
      }, terminalRendererBridge, noticeController, {
        // Diagnostics/watchdog bridge (CUT-24 #8704). Absent preload degrades to
        // an unavailable projection; the renderer schema-decodes every result.
        gather: () => readBridge()?.diagnostics?.gather?.() ?? Promise.resolve(null),
        runAction: (action) => readBridge()?.diagnostics?.runAction?.(action) ?? Promise.resolve({ ok: false, notice: "Diagnostics unavailable" }),
        exportRedacted: () => readBridge()?.diagnostics?.exportRedacted?.() ?? Promise.resolve({ ok: false, notice: "Diagnostics unavailable" }),
      }),
    )
    if (typeof bridge?.workspaceSubscribe === "function") {
      const unsubscribeWorkspace = bridge.workspaceSubscribe(change => {
        void Effect.runPromise(
          Effect.all([
            registry.dispatch(resolveIntentRef(IntentRef("WorkspaceBrowserChangeReceived", StaticPayload(change)))),
            registry.dispatch(resolveIntentRef(IntentRef("WorkspaceEditorExternalChangeReceived", StaticPayload(change)))),
          ], { concurrency: 1, discard: true }),
        )
      })
      window.addEventListener("pagehide", () => unsubscribeWorkspace(), { once: true })
    }
    // Workspace-bounded PTY terminals (CUT-20, #8700): stream every host event
    // into the terminal workspace state (schema-decoded in the handler), and
    // pull the snapshot once so a persisted tail is recovered after a restart.
    if (typeof bridge?.terminal?.onEvent === "function") {
      const unsubscribeTerminal = bridge.terminal.onEvent((event) => {
        void Effect.runPromise(
          registry.dispatch(resolveIntentRef(IntentRef("TerminalEventReceived", StaticPayload(event)))),
        )
      })
      window.addEventListener("pagehide", () => unsubscribeTerminal(), { once: true })
    }
    if (typeof bridge?.terminal?.snapshot === "function") {
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("TerminalRefreshRequested", StaticPayload(null)))),
      )
    }
    // Session usage ledger push (#8712 Lane C): every ledger change re-pulls
    // the typed snapshot through the fleet handlers (schema-decoded there).
    if (typeof bridge?.usageLedger?.onEvent === "function") {
      const unsubscribeLedger = bridge.usageLedger.onEvent(() => {
        void Effect.runPromise(
          registry.dispatch(resolveIntentRef(IntentRef("FleetLedgerUpdated", StaticPayload(null)))),
        )
      })
      window.addEventListener("pagehide", () => unsubscribeLedger(), { once: true })
    }
    const historyCatalog = yield* Effect.promise(historyHost.catalog)
    // EP250 bottom-anchored flow: a restored ITEM selection loads the window
    // AROUND that item and scrolls to it; otherwise the conversation opens at
    // its END with the newest items visible.
    let initialHistoryAnchor: Readonly<{ kind: "end" }> | Readonly<{ kind: "item"; itemRef: string }> | null = null
    if (historyCatalog !== null) {
      const restored=restoreHistory(); const selected=restorableHistoryThreadRef(historyCatalog,restored?.selectedThreadRef,historyCatalogPageSize)
      let firstPage: CodexHistoryPage | null = null
      if (selected !== null) {
        const probe = yield* Effect.promise(() => historyHost.page(selected, 0, 1))
        if (probe !== null) {
          const plan = historyRestoreFetchPlan(restored, probe.totalItems, historyItemPageSize)
          firstPage = yield* Effect.promise(() => historyHost.page(selected, plan.offset, historyItemPageSize))
          initialHistoryAnchor = plan.anchor === "item" && restored?.selectedItemRef != null && firstPage !== null && firstPage.items.some(item => item.itemRef === restored.selectedItemRef)
            ? { kind: "item", itemRef: restored.selectedItemRef }
            : { kind: "end" }
        }
      }
      yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, catalog: historyCatalog, page: firstPage, selectedItemRef: firstPage?.items.some(item=>item.itemRef===restored?.selectedItemRef)?restored!.selectedItemRef:null, railCollapsed:restored?.railCollapsed??false, expandedThreadRefs:restored?.expandedThreadRefs??firstPage?.agents.filter(agent=>agent.descendantCount>0).map(agent=>agent.threadRef)??[] } }))
    }
    let restoredWorkspace: DesktopWorkspaceName | null = null
    const codingCatalog = yield* Effect.promise(codingCatalogHost.snapshot)
    if (codingCatalog.sessions.length > 0) {
      restoredWorkspace = desktopWorkspaceForCodingFocus(codingCatalog.focus)
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        codingCatalog,
      }))
    }
    const existing = yield* Effect.promise(chat.listThreads)
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
    const sessionView = decodeOpenAgentsSessionView(
      yield* Effect.promise(openAgentsSessionSettingsBridge.status),
    )
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, openAgentsSession: sessionView },
    }))
    const dispatchDeferredCommand = (command: DesktopDeferredCommand) => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const resolution = resolveDesktopDeferredCommandIntent(command, {
        sessionReady: current.settings.openAgentsSession === "session_ready",
        verifiedOwner: current.settings.openAgentsSession === "session_ready",
        workspaceReady: current.workspaceBrowser.grantRef !== null || current.codingCatalog.sessions.length > 0,
      })
      if (resolution.state === "rejected") {
        // CUT-15: the command IS still rejected/ignored. Only the notice
        // presentation is now transient (a self-dismissing toast) instead of a
        // permanent top banner.
        yield* noticeController.setTransientNotice(
          resolution.reason === "duplicate"
            ? "That command request was already handled. The duplicate was ignored."
            : "That command is unavailable for the current session or workspace.",
        )
        return
      }
      yield* noticeController.dismissNotice
      const ref = IntentRef(resolution.intentName, StaticPayload(resolution.payload))
      yield* registry.dispatch(resolveIntentRef(ref))
    })
    if (restoredWorkspace !== null) {
      const commandId = restoredWorkspace === "chat" ? "chat.open" : `workspace.${restoredWorkspace}`
      const definition = desktopCanonicalCommandRegistry.find(value => value.id === commandId)
      if (definition !== undefined) {
        yield* dispatchDeferredCommand({
          schema: "openagents.desktop.deferred_command.v1",
          requestRef: `command.restore.${restoredWorkspace}`,
          commandId: definition.id,
          arguments: definition.defaultArguments,
          source: "restore",
          delivery: "dispatch",
        })
      }
    }
    if (typeof bridge?.commands?.onCommand === "function") {
      const unsubscribeCommands = bridge.commands.onCommand(command => {
        void Effect.runPromise(dispatchDeferredCommand(command))
      })
      window.addEventListener("pagehide", () => unsubscribeCommands(), { once: true })
      if (typeof bridge.commands.ready === "function") {
        yield* Effect.promise(bridge.commands.ready)
      }
    }
    const focusComposer = (): void => {
      // Focus must land AFTER the chat view mounts. A New-chat dispatch can
      // clear a loaded history page, which swaps the whole center view and
      // (re)mounts the composer on a LATER render commit than the intent
      // completion — and a re-parented input loses focus even when it was
      // focused earlier. So retry across commits until the input exists AND
      // holds focus (owner contract: "when i do new chat, clicking button
      // or command N, auto focus the input").
      let attempts = 0
      const tryFocus = (): void => {
        // EP250 OpenCode restyle made the composer a multiline textarea; accept
        // either the textarea or a legacy input so focus still lands.
        const input = root.querySelector<HTMLTextAreaElement | HTMLInputElement>(
          '[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input',
        )
        if (input !== null && !input.disabled) {
          input.focus()
          if (root.ownerDocument.activeElement === input) return
        }
        attempts += 1
        if (attempts < 20) window.setTimeout(tryFocus, 16)
      }
      window.setTimeout(tryFocus, 0)
    }
    const report: IntentReporter = (ref, runtimeValue) => {
      const shouldFocus = ref.name === "DesktopNewChat" || ref.name === "DesktopNoteSubmitted"
      // Selection-driven history loads land bottom-anchored (EP250: "show
      // the most recent messages, starting at bottom").
      const shouldAnchorHistoryEnd = ref.name === "HistoryConversationSelected" || ref.name === "HistoryAgentSelected"
      // Opening a search result windows on its matching item (content match) or
      // the end (title match) — reuse the restore-to-item scroll flow (H4).
      const shouldAnchorSearchOpen = ref.name === "HistorySearchResultOpened"
      return registry.dispatch(resolveIntentRef(ref, runtimeValue ?? null)).pipe(
        Effect.ensuring(Effect.gen(function* () {
          const current = yield* SubscriptionRef.get(state)
          yield* Effect.sync(() => {
            if (shouldFocus) focusComposer()
            if (shouldAnchorHistoryEnd) void anchorHistoryEnd()
            if (shouldAnchorSearchOpen) { const itemRef = current.history.selectedItemRef; void (itemRef === null ? anchorHistoryEnd() : anchorHistoryItem(itemRef)) }
          })
        })),
      )
    }
    // Cmd+N / Ctrl+N -> DesktopNewChat (owner contract: "when i do new chat,
    // clicking button or command N, auto focus the input"). The chord is the
    // canonical `chat.new` default binding from the desktop command contract;
    // dispatch rides the same IntentReporter as button presses so the
    // post-mount composer-focus hook fires on this path too. Same platform-
    // modifier + editable-guard pattern as the other global shortcuts.
    const onNewChatShortcut = (event: KeyboardEvent): void => {
      const target = event.target
      const editable = target instanceof HTMLElement &&
        target.closest("input, textarea, [contenteditable='true']") !== null
      const platformModifier = bridge?.platform === "darwin"
        ? event.metaKey && !event.ctrlKey
        : event.ctrlKey && !event.metaKey
      if (
        event.defaultPrevented ||
        editable ||
        event.key.toLowerCase() !== "n" ||
        !platformModifier ||
        event.altKey ||
        event.shiftKey
      ) return
      event.preventDefault()
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopNewChat", StaticPayload(null)))).pipe(
          Effect.ensuring(Effect.sync(() => focusComposer())),
        ),
      )
    }
    // Cmd+F / Ctrl+F -> DesktopFullscreenToggled (owner contract EP250:
    // "add a hotkey for maximizing (command+something) to fullscreen like
    // command f"). Deliberately NO editable guard: fullscreen from the
    // composer is expected; the app has no find-in-page yet (rebind review
    // when find lands).
    const onFullscreenShortcut = (event: KeyboardEvent): void => {
      const platformModifier = bridge?.platform === "darwin"
        ? event.metaKey && !event.ctrlKey
        : event.ctrlKey && !event.metaKey
      if (
        event.defaultPrevented ||
        event.key.toLowerCase() !== "f" ||
        !platformModifier ||
        event.altKey ||
        event.shiftKey
      ) return
      event.preventDefault()
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopFullscreenToggled", StaticPayload(null)))),
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
    // EP250 owner statement (verbatim): "i want shift+tab to togle between
    // modes in composer (fable / codex) in this case". Scoped to the focused
    // composer input only (normal Shift+Tab focus navigation everywhere
    // else); dispatches the SAME DesktopHarnessSelected intent the chips use.
    const onComposerShiftTab = (event: KeyboardEvent): void => {
      handleComposerShiftTab(event, {
        isComposerInput: isShellComposerInputTarget,
        selectedHarness: () => Effect.runSync(SubscriptionRef.get(state)).selectedHarness,
        selectHarness: harness => {
          void Effect.runPromise(
            registry.dispatch(resolveIntentRef(IntentRef("DesktopHarnessSelected", StaticPayload(harness)))),
          )
        },
      })
    }
    let historyShortcutSteps=0
    let historyShortcutAbsoluteIndex:number|null=null
    let historyShortcutRunning=false
    let historySelectionTimer:number|null=null
    const settleFrame=():Promise<void>=>new Promise(resolve=>requestAnimationFrame(()=>resolve()))
    const scrollHistorySelectionIntoView=async(index:number,threadRef:string):Promise<void>=>{
      for(let attempt=0;attempt<4;attempt++){
        await settleFrame()
        const list=root.querySelector<HTMLElement>('[data-en-key="sidebar-history-list"]')
        if(list===null)return
        if(list.getAttribute("data-en-virtualized")==="true"){
          list.scrollTop=Math.max(0,index*28-Math.max(0,list.clientHeight-28)/2)
          list.dispatchEvent(new Event("scroll",{bubbles:true}))
          continue
        }
        const row=[...root.querySelectorAll<HTMLElement>('[data-en-key^="sidebar-thread-"]')].find(item=>item.getAttribute("data-en-key")===`sidebar-thread-${threadRef}`)
        const item=row?.closest<HTMLElement>('[data-en-role="item"]')??row
        if(row===undefined||item==null)continue
        const rows=Array.from(list.querySelectorAll<HTMLElement>('[data-en-key^="sidebar-thread-"][data-en-tag="Button"]'))
        const rowIndex=rows.findIndex(candidate=>candidate.getAttribute("data-en-key")===`sidebar-thread-${threadRef}`)
        const measuredRowHeight=rows.map(candidate=>candidate.getBoundingClientRect().height).find(height=>height>0)??24
        const estimatedTop=rowIndex<0?item.offsetTop:rowIndex*measuredRowHeight
        const requestedScrollTop=Math.max(0,estimatedTop-Math.max(0,list.clientHeight-measuredRowHeight)/2)
        list.scrollTop=requestedScrollTop
        list.dispatchEvent(new Event("scroll",{bubbles:true}))
        await settleFrame()
        const rowRect=row.getBoundingClientRect();const listRect=list.getBoundingClientRect()
        if(rowRect.top>=listRect.top-1&&rowRect.bottom<=listRect.bottom+1)return
      }
    }
    // --- EP250 bottom-anchored history scrolling -------------------------
    const historyScrollEl=():HTMLElement|null=>root.querySelector<HTMLElement>('[data-en-key="history-timeline-page"]')
    const anchorHistoryEnd=async():Promise<void>=>{
      for(let attempt=0;attempt<8;attempt++){
        await settleFrame()
        const el=historyScrollEl()
        if(el!==null&&el.scrollHeight>0){
          el.scrollTop=el.scrollHeight
          if(el.scrollHeight-(el.scrollTop+el.clientHeight)<=2)return
        }
      }
    }
    const anchorHistoryItem=async(itemRef:string):Promise<void>=>{
      for(let attempt=0;attempt<8;attempt++){
        await settleFrame()
        const el=historyScrollEl()
        const row=el?.querySelector<HTMLElement>(`[data-en-key="history-item-${itemRef}"]`)??null
        if(el!==null&&row!==null){el.scrollTop=Math.max(0,row.offsetTop-Math.max(0,el.clientHeight-row.offsetHeight)/2);return}
      }
    }
    // Smart prefetch: fetch the older page ~1.5 viewports BEFORE the reader
    // reaches the top of loaded content; preserve the scroll anchor on
    // prepend so the viewport never jumps (owner contract: "auto load them
    // as i scroll up, smartly loading before the cursor").
    let historyEdgeFetchInFlight=false
    const onHistoryTimelineScroll=(event:Event):void=>{
      const el=event.target
      if(!(el instanceof HTMLElement)||el.getAttribute("data-en-key")!=="history-timeline-page"||historyEdgeFetchInFlight)return
      const metrics={scrollTop:el.scrollTop,clientHeight:el.clientHeight,scrollHeight:el.scrollHeight}
      historyEdgeFetchInFlight=true
      void Effect.runPromise(SubscriptionRef.get(state)).then(async current=>{
        const page=current.history.page
        if(current.workspace!=="chat"||page===null)return
        if(historyShouldFetchOlder({scrollTop:metrics.scrollTop,clientHeight:metrics.clientHeight,offset:page.offset,loadingEdge:current.history.loadingEdge})){
          const savedTop=metrics.scrollTop;const savedHeight=metrics.scrollHeight
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryOlderRequested",StaticPayload(null)))))
          await settleFrame()
          const after=historyScrollEl()
          if(after!==null)after.scrollTop=historyPrependScrollTop(savedTop,savedHeight,after.scrollHeight)
        } else if(historyShouldFetchNewer({scrollTop:metrics.scrollTop,clientHeight:metrics.clientHeight,scrollHeight:metrics.scrollHeight,windowEnd:page.offset+page.items.length,totalItems:page.totalItems,loadingEdge:current.history.loadingEdge})){
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryNewerRequested",StaticPayload(null)))))
        }
      }).finally(()=>{historyEdgeFetchInFlight=false})
    }
    const pumpHistoryConversationShortcut=async():Promise<void>=>{
      if(historyShortcutRunning)return
      historyShortcutRunning=true
      try{
        while(historyShortcutSteps!==0||historyShortcutAbsoluteIndex!==null){
          await new Promise(resolve=>window.setTimeout(resolve,35))
          const current=await Effect.runPromise(SubscriptionRef.get(state))
          if(current.workspace!=="chat"||current.history.catalog.roots.length===0){historyShortcutSteps=0;historyShortcutAbsoluteIndex=null;break}
          const roots=current.history.catalog.roots
          const activeRef=current.history.pendingThreadRef??current.history.page?.rootThreadRef
          const activeIndex=roots.findIndex(item=>item.threadRef===activeRef)
          const steps=historyShortcutSteps
          const absoluteIndex=historyShortcutAbsoluteIndex
          historyShortcutSteps=0
          historyShortcutAbsoluteIndex=null
          const baseIndex=activeIndex<0?(steps>0?-1:1):activeIndex
          const targetIndex=Math.max(0,Math.min(roots.length-1,absoluteIndex??baseIndex+steps))
          if(targetIndex===activeIndex)continue
          let visible=current.history.visibleRootCount
          while(targetIndex>=visible){
            await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryCatalogMoreRequested",StaticPayload(null)))))
            visible+=historyCatalogPageSize
          }
          const targetRef=roots[targetIndex]!.threadRef
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryConversationPreviewed",StaticPayload(targetRef)))))
          await scrollHistorySelectionIntoView(targetIndex,targetRef)
          if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
          historySelectionTimer=window.setTimeout(()=>{
            historySelectionTimer=null
            void Effect.runPromise(SubscriptionRef.get(state)).then(current=>{
              if(current.history.pendingThreadRef!==targetRef)return
              return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryConversationSelected",StaticPayload(targetRef)))))
            }).then(()=>{void anchorHistoryEnd();return scrollHistorySelectionIntoView(targetIndex,targetRef)})
          },110)
        }
      }finally{
        historyShortcutRunning=false
        if(historyShortcutSteps!==0||historyShortcutAbsoluteIndex!==null)void pumpHistoryConversationShortcut()
      }
    }
    const setHistoryShortcutHints=(visible:boolean):void=>{
      void Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryShortcutHintsChanged",StaticPayload(visible)))))
    }
    const onHistoryConversationShortcut = (event: KeyboardEvent): void => {
      const target=event.target
      const editable=target instanceof HTMLElement&&target.closest("input, textarea, [contenteditable='true']")!==null
      const platformModifier=bridge?.platform==="darwin"?event.metaKey&&!event.ctrlKey:event.ctrlKey&&!event.metaKey
      const digit=/^[1-9]$/.test(event.key)?Number(event.key)-1:null
      if(event.defaultPrevented||editable||!platformModifier||event.altKey||event.shiftKey||(digit===null&&event.key!=="ArrowUp"&&event.key!=="ArrowDown"))return
      event.preventDefault()
      if(digit!==null){historyShortcutSteps=0;historyShortcutAbsoluteIndex=digit}
      else {historyShortcutAbsoluteIndex=null;historyShortcutSteps+=event.key==="ArrowDown"?1:-1}
      void pumpHistoryConversationShortcut()
    }
    // Cmd+Shift+Up/Down (Ctrl+Shift off-macOS): agent traversal inside the
    // open conversation (owner contract: "just like command up and down
    // scrolsl thru chats, have command shift up and down go up and down the
    // agents of a convo."). Same platform-modifier + editable-guard pattern
    // as the unshifted conversation shortcut above (which explicitly ignores
    // shifted chords); dispatches the SAME HistoryAgentSelected intent the
    // right-rail agent rows dispatch — one selection path, no parallel route.
    const onHistoryAgentShortcut = (event: KeyboardEvent): void => {
      const target=event.target
      const editable=target instanceof HTMLElement&&target.closest("input, textarea, [contenteditable='true']")!==null
      if(event.defaultPrevented||editable||!isHistoryAgentTraversalShortcut(event,bridge?.platform))return
      event.preventDefault()
      const delta=event.key==="ArrowDown"?1:-1
      void Effect.runPromise(SubscriptionRef.get(state)).then(current=>{
        if(current.workspace!=="chat"||current.history.page===null)return
        const targetRef=historyAgentTraversalTarget(current.history,delta)
        if(targetRef===null)return
        return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryAgentSelected",StaticPayload(targetRef))))).then(()=>anchorHistoryEnd())
      })
    }
    const onHistoryModifierDown=(event:KeyboardEvent):void=>{
      const platformModifier=bridge?.platform==="darwin"?event.metaKey:event.ctrlKey
      if(platformModifier)setHistoryShortcutHints(true)
    }
    const onHistoryModifierUp=(event:KeyboardEvent):void=>{
      if((bridge?.platform==="darwin"&&event.key==="Meta")||(bridge?.platform!=="darwin"&&event.key==="Control"))setHistoryShortcutHints(false)
    }
    const onHistoryWindowBlur=():void=>setHistoryShortcutHints(false)
    // Capability I1: drag-drop + paste-from-clipboard image attach, scoped to
    // the composer. Both feed the SAME renderer-side decode/validation path
    // (readImageFile) and dispatch the SAME DesktopComposerImageAdded intent
    // the picker uses — File bytes already live in the renderer (a user drop or
    // clipboard item), so nothing here reads the filesystem.
    const targetInComposer = (target: EventTarget | null): boolean =>
      target instanceof HTMLElement && target.closest('[data-en-key="shell-composer"]') !== null
    const addImagesFromFiles = async (files: ReadonlyArray<File>): Promise<void> => {
      const snapshot = await Effect.runPromise(SubscriptionRef.get(state))
      if (snapshot.pending) return
      let count = snapshot.composerImages.length
      let firstRejection: string | null = null
      for (const file of files) {
        const result = await readImageFile(file, count)
        if (result.ok) {
          count += 1
          await Effect.runPromise(
            registry.dispatch(resolveIntentRef(IntentRef("DesktopComposerImageAdded", StaticPayload(result.attachment)))),
          )
        } else if (firstRejection === null) {
          firstRejection = composerImageRejectionMessage(result.reason)
        }
      }
      if (firstRejection !== null) {
        await Effect.runPromise(
          registry.dispatch(resolveIntentRef(IntentRef("DesktopComposerImagesRejected", StaticPayload(firstRejection)))),
        )
      }
    }
    const imageFilesFrom = (list: FileList | null | undefined): File[] =>
      list === null || list === undefined
        ? []
        : [...list].filter((file) => file.type.startsWith("image/"))
    const onComposerDragOver = (event: DragEvent): void => {
      if (!targetInComposer(event.target)) return
      // Signal a copy drop so the browser shows the drop affordance.
      event.preventDefault()
    }
    const onComposerDrop = (event: DragEvent): void => {
      if (!targetInComposer(event.target)) return
      const files = imageFilesFrom(event.dataTransfer?.files)
      if (files.length === 0) return
      event.preventDefault()
      void addImagesFromFiles(files)
    }
    const onComposerPaste = (event: ClipboardEvent): void => {
      if (!targetInComposer(event.target)) return
      const items = event.clipboardData?.items
      if (items === undefined) return
      const files: File[] = []
      for (const item of items) {
        if (item.kind === "file" && item.type.startsWith("image/")) {
          const file = item.getAsFile()
          if (file !== null) files.push(file)
        }
      }
      if (files.length === 0) return // plain-text paste falls through unchanged
      event.preventDefault()
      void addImagesFromFiles(files)
    }
    window.addEventListener("dragover", onComposerDragOver)
    window.addEventListener("drop", onComposerDrop)
    window.addEventListener("paste", onComposerPaste)
    window.addEventListener("keydown", onNewChatShortcut)
    window.addEventListener("keydown", onFullscreenShortcut)
    window.addEventListener("keydown", onComposerShiftTab)
    window.addEventListener("keydown", onCommandPaletteShortcut)
    window.addEventListener("keydown", onHistoryModifierDown)
    window.addEventListener("keydown", onHistoryConversationShortcut)
    window.addEventListener("keydown", onHistoryAgentShortcut)
    window.addEventListener("keyup", onHistoryModifierUp)
    window.addEventListener("blur", onHistoryWindowBlur)
    // Scroll events do not bubble; capture phase observes the history region.
    window.addEventListener("scroll", onHistoryTimelineScroll, true)
    window.addEventListener("pagehide", () => {
      window.removeEventListener("dragover", onComposerDragOver)
      window.removeEventListener("drop", onComposerDrop)
      window.removeEventListener("paste", onComposerPaste)
      window.removeEventListener("keydown", onNewChatShortcut)
      window.removeEventListener("keydown", onFullscreenShortcut)
      window.removeEventListener("keydown", onComposerShiftTab)
      window.removeEventListener("keydown", onCommandPaletteShortcut)
      window.removeEventListener("keydown", onHistoryModifierDown)
      window.removeEventListener("keydown", onHistoryConversationShortcut)
      window.removeEventListener("keydown", onHistoryAgentShortcut)
      window.removeEventListener("keyup", onHistoryModifierUp)
      window.removeEventListener("blur", onHistoryWindowBlur)
      window.removeEventListener("scroll", onHistoryTimelineScroll, true)
      if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
    }, { once: true })
    // Durable preferences → real presentation (CUT-24 #8704): density + font
    // scale the shared theme through the token pipeline; reduced-motion resolves
    // to a root data attribute the app CSS honors. Defaults are identity, so the
    // common path (and the no-preload smoke path) is unchanged.
    const preferences = yield* Effect.promise(loadDesktopPreferences)
    for (const [name, value] of Object.entries(preferencesRootAttributes(preferences))) {
      document.documentElement.setAttribute(name, value)
    }
    const renderer = makeDomRenderer({
      theme: themeForPreferences(preferences),
      hostDrivers: [makeStubCodeEditorDriver()],
    })
    yield* renderer.mount(root, program.viewStream, report)
    // Startup-timing instrumentation (measure-constantly discipline; see
    // scripts/startup-bench.ts + docs/fable/2026-07-11-desktop-startup-speed-audit.md).
    // A plain renderer-local global — NEVER a preload/IPC channel — that the
    // main-process bench mode reads back with executeJavaScript. Records the
    // wall-clock instant (ms epoch) the shell DOM first mounted: the interactive
    // frame. Harmless in production (an unread global object).
    {
      const marks = ((globalThis as { __oaStartupMarks?: Record<string, number> }).__oaStartupMarks ??= {})
      marks.shellMounted = Date.now()
    }
    // Sidebar connected-accounts box (EP250): one boot-time accounts pull so
    // the pinned bottom box has evidence without visiting the Fleet
    // workspace. This rides the EXISTING FleetRefreshRequested flow (list +
    // session ledger) after first paint — event-driven, not a polling loop;
    // an absent bridge degrades to the honest unavailable projection and the
    // box simply does not render.
    void Effect.runPromise(
      registry.dispatch(resolveIntentRef(IntentRef("FleetRefreshRequested", StaticPayload(null)))),
    )
    // One boot-time diagnostics gather so the watchdog panel has live health the
    // first time Settings is opened (CUT-24 #8704). Event-driven, not a poll.
    void Effect.runPromise(
      registry.dispatch(resolveIntentRef(IntentRef("DesktopDiagnosticsRefreshRequested", StaticPayload(null)))),
    )
    // Restored history lands where the reader left it: window AROUND the
    // saved item (scrolled to it) or bottom-anchored at the newest items.
    if (initialHistoryAnchor !== null) {
      const anchor = initialHistoryAnchor
      window.setTimeout(() => { void (anchor.kind === "end" ? anchorHistoryEnd() : anchorHistoryItem(anchor.itemRef)) }, 0)
    }
    // First paint must never wait on local rollout parsing. The sidebar gets
    // metadata immediately; the selected thread receives five recent messages
    // and then its bounded expanded tail after the DOM is already visible.
    if (threads.length > 0) {
      const id = threads[0]!.id
      window.setTimeout(() => {
        void (async () => {
          const detail = await chat.openThread(id)
          if (typeof detail === "object" && detail !== null && typeof (detail as { id?: unknown }).id === "string") {
            const selected = detail as DesktopThread
            await Effect.runPromise(SubscriptionRef.update(state, current => current.activeThreadId === id
              ? { ...current, threads: [selected, ...current.threads.filter(thread => thread.id !== id)], notes: selected.notes }
              : current))
          }
          if (chat.hydrateThread === undefined) return
          const hydrated = await chat.hydrateThread(id)
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
  // Startup-timing: earliest renderer-script instant (ms epoch). See the
  // shellMounted mark above and scripts/startup-bench.ts.
  ;((globalThis as { __oaStartupMarks?: Record<string, number> }).__oaStartupMarks ??= {}).bootStart = Date.now()
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
