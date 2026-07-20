/**
 * Renderer entrypoint (#8574): boots the OpenAgents Desktop shell as one
 * Effect Native program — SubscriptionRef state, `makeViewProgramFromState`,
 * a typed intent registry, and the React DOM renderer from the shared vendored
 * catalog. React owns the host tree; Effect Native owns the application.
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
import { Effect, Exit, Schema, Scope, Stream, SubscriptionRef } from "@effect-native/core/effect"
import { makeStubCodeEditorDriver } from "@effect-native/render-dom"
import { makeReactDomRenderer } from "@effect-native/render-dom/react"
import { projectFleetCockpitCard, type FleetAuthority } from "../fleet-cockpit.ts"
import "./app.css"
import { mountReactWorkbench } from "./react-primitive-adapters.tsx"

import {
  unavailableCodexSettingsBridge,
  unavailableMcpConfigSettingsBridge,
  unavailableProviderAccountsSettingsBridge,
  decodeOpenAgentsSessionView,
  type CodexSettingsBridge,
  type McpConfigSettingsBridge,
  unavailablePluginConfigSettingsBridge,
  type PluginConfigSettingsBridge,
  type HarnessMaintenanceSettingsBridge,
  type OpenAgentsSessionSettingsBridge,
  type ProviderAccountsSettingsBridge,
} from "./settings.ts"
import {
  unavailableAcpProviderSettingsBridge,
  type AcpProviderSettingsBridge,
} from "../acp-provider-contract.ts"
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
import type { IdeRunEvent } from "../ide/run-contract.ts"
import {
  unavailableGitGithubBridge,
  type GitGithubBridge,
} from "./git-panel.ts"
import {
  decodeGitGithubRequest,
  gitGithubError,
  type GitFileStatus,
  type GitGithubOp,
} from "../git-github-contract.ts"
import {
  IdeSourceControlOperationRefSchema,
  decodeIdeSourceControlCommandResult,
  decodeIdeSourceControlSnapshot,
  type IdeSourceControlSnapshot,
} from "../source-control-renderer-contract.ts"
import {
  unavailableWorkspaceBrowserBridge,
  type WorkspaceBrowserBridge,
} from "./workspace-browser.ts"
import {
  decodeWorkspaceEditorRecoverySnapshot,
  unavailableWorkspaceDocumentBridge,
  unavailableWorkspaceLanguageBridge,
  workspaceEditorRecoverySnapshot,
  type WorkspaceDocumentBridge,
  type WorkspaceLanguageBridge,
} from "./workspace-editor.ts"
import {
  desktopShellIntents,
  desktopShellView,
  desktopConversationShortcutTargets,
  formatShellTimestamp,
  initialDesktopShellState,
  makeDesktopShellHandlers,
  withFullAutoLiveState,
  withInput,
  withLiveAgentGraph,
  withThreadCatalog,
  withThreads,
  withProviderLaneCapabilities,
} from "./shell.ts"
import { makeCommandNoticeController } from "./command-notice.ts"
import { makeComposerFocuser, makeComposerFocusSettler } from "./composer-focus.ts"
import { withVoiceHostState } from "./voice-mode.ts"
import { executeVoiceAction, makeVoiceFinalLedger } from "./voice-actions.ts"
import {
  migrateDesktopPreferences,
  type DesktopPreferences,
} from "../desktop-preferences-contract.ts"
import { preferencesRootAttributes, themeForPreferences } from "../desktop-preferences-effects.ts"
import { makeConvergingDesktopChatHost } from "./runtime-conversation.ts"
import {
  decodeDesktopRuntimeControlOutcomeLookupResult,
  decodeDesktopRuntimeControlOutcomeRecordResult,
  type DesktopRuntimeControlOutcomeLookup,
  type DesktopRuntimeControlOutcomeRecord,
} from "../runtime-control-outcome-contract.ts"
import {
  makeLocalHarnessChatHost,
  type FableLocalRendererBridge,
} from "./local-harness.ts"
import { withHarnessLanes, type AppleFmBootState, type DesktopAppleFmChatHost, type DesktopWorkspaceName, type HarnessLanes } from "./shell.ts"
import type { AppleFmStartTurnRequest, AppleFmStatus, AppleFmStopResult, AppleFmTurnResult } from "../apple-fm-contract.ts"
import { unavailableFullAutoRunOutcome, type FullAutoRunRendererHost } from "../full-auto-run-ipc-contract.ts"
import { decodeProviderLaneComposerProjections } from "../provider-lane-capabilities.ts"
import {
  decodeFableLocalAvailability,
  type FableLocalAvailability,
  type FableLocalEventEnvelope,
  type FableLocalImageAttachment,
} from "../fable-local-contract.ts"
import { installComposerImageAcquisition } from "./composer-image-acquisition.ts"
import {
  codexHarnessLaneFromAvailability,
  decodeCodexLocalAvailability,
  type CodexLocalAvailability,
} from "../codex-local-contract.ts"
import { type DesktopThread } from "../chat-contract.ts"
import type { LiveAgentGraphHostSnapshot, LiveAgentGraphUpdate } from "../live-agent-graph-contract.ts"
import { projectLiveAgentGraphPresentation } from "../agent-graph-presentation.ts"
import type { DesktopWorkspaceChange } from "../workspace-contract.ts"
import type {
  DesktopRuntimeGatewayEvent,
  DesktopRuntimeGatewayResponse,
} from "../runtime-gateway-contract.ts"
import type { CodexHistoryCatalog, CodexHistoryPage, CodexHistorySearchResponse } from "../codex-history-contract.ts"
import { historyAgentTraversalTarget, historyCatalogPageSize, historyConversationShortcutAction, historyItemPageSize, historyShouldFetchNewer, historyShouldFetchOlder, isHistoryAgentTraversalShortcut } from "./history-workspace.ts"
import {
  decodeDesktopCodingCatalogProjection,
  desktopWorkspaceForCodingFocus,
  emptyDesktopCodingCatalogProjection,
  type DesktopCodingCatalogProjection,
} from "../coding-catalog-contract.ts"
import {
  decodeDesktopUpdateProjection,
  emptyDesktopUpdateProjection,
} from "../update-staging-contract.ts"
import {
  decodeDesktopCommandBindingProjectionOrNull,
  desktopCanonicalCommandRegistry,
  type DesktopCommandBindingProjection,
  type DesktopCommandId,
  type DesktopDeferredCommand,
} from "../desktop-command-contract.ts"
import { resolveDesktopDeferredCommandIntent } from "./command-registry.ts"
import { desktopCommandShortcutMatches } from "./command-shortcuts.ts"
import { decodeDesktopPreviewChangeEvent } from "../dev-preview-contract.ts"
import { desktopPreviewReloadRisk } from "./dev-preview.ts"
import { decodeDesktopLaunchContext } from "../desktop-launch-context.ts"
import {
  loadAgentCodeRendererSnapshot,
  unavailableIdeAgentCodeRendererHost,
  type IdeAgentCodeRendererHost,
} from "./ide/agent-code.ts"
import {
  cancelInvalidatedIdeCursor,
  invalidateIdeCursorRendererState,
  loadIdeCursorRendererSnapshot,
  unavailableIdeCursorRendererHost,
  type IdeCursorRendererHost,
} from "./ide/cursor.ts"
import {
  loadIdeManagedSandboxRendererSnapshot,
  unavailableIdeManagedSandboxRendererHost,
  type IdeManagedSandboxRendererHost,
} from "./ide/managed-sandbox.ts"

/** Effect Schema at the preload boundary (issue #8574: Schema, not Zod). */
const DesktopBridgeSchema = Schema.Struct({
  host: Schema.String,
  platform: Schema.String,
})

type DesktopBridge = Readonly<{
  host: string
  platform: string
  smokeProviderTurns?: boolean
  launchContext?: unknown
  /** Apple FM native bridge (AFM-6 #9075). Optional across dev preload rolls. */
  appleFm?: Readonly<{
    status?: () => Promise<AppleFmStatus>
    refresh?: () => Promise<AppleFmStatus>
    startTurn?: (request: AppleFmStartTurnRequest) => Promise<AppleFmTurnResult>
    stop?: () => Promise<AppleFmStopResult>
  }>
  runtimeRequest?: (value: unknown) => Promise<DesktopRuntimeGatewayResponse>
  runtimeSubscribe?: (listener: (event: DesktopRuntimeGatewayEvent) => void) => () => void
  controlOutcomes?: Readonly<{
    lookup?: (value: DesktopRuntimeControlOutcomeLookup) => Promise<unknown>
    record?: (value: DesktopRuntimeControlOutcomeRecord) => Promise<unknown>
  }>
  stageFleet?: (value: unknown) => Promise<unknown>
  listThreads?: () => Promise<unknown>
  localTurnRecovery?: Readonly<{
    onUpdate?: (listener: (thread: DesktopThread) => void) => () => void
  }>
  newThread?: (value?: unknown) => Promise<unknown>
  openThread?: (value: unknown) => Promise<unknown>
  hydrateThread?: (value: unknown) => Promise<unknown>
  historyThreads?: Readonly<{
    listLocal?: () => Promise<unknown>
    resumeLocal?: (value: unknown) => Promise<unknown>
    renameLocal?: (value: unknown) => Promise<unknown>
    fork?: (value: unknown) => Promise<unknown>
  }>
  sendMessage?: (value: unknown) => Promise<unknown>
  chooseWorkspace?: () => Promise<unknown>
  workingDirectory?: () => Promise<unknown>
  ideAgentCode?: Readonly<{
    snapshot?: () => Promise<unknown>
    command?: (value: unknown) => Promise<unknown>
  }>
  ideCursor?: Readonly<{
    snapshot?: () => Promise<unknown>
    command?: (value: unknown) => Promise<unknown>
  }>
  ideManagedSandbox?: Readonly<{
    snapshot?: () => Promise<unknown>
    command?: (value: unknown) => Promise<unknown>
  }>
  productSpec?: Readonly<{
    open?: (value: unknown) => Promise<unknown>
    create?: (value: unknown) => Promise<unknown>
    proposeEdit?: (value: unknown) => Promise<unknown>
    confirmEdit?: (value: unknown) => Promise<unknown>
    proposeEvidenceAttachment?: (value: unknown) => Promise<unknown>
    confirmEvidenceAttachment?: (value: unknown) => Promise<unknown>
    proposePlan?: (value: unknown) => Promise<unknown>
    acceptPlan?: (value: unknown) => Promise<unknown>
    admitPacket?: (value: unknown) => Promise<unknown>
    blockPacket?: (value: unknown) => Promise<unknown>
    disposePacket?: (value: unknown) => Promise<unknown>
    disposeRun?: (value: unknown) => Promise<unknown>
    recordEvidence?: (value: unknown) => Promise<unknown>
    verifyEvidence?: (value: unknown) => Promise<unknown>
    setOwnerDisposition?: (value: unknown) => Promise<unknown>
    run?: (value: unknown) => Promise<unknown>
  }>
  workspaceTree?: (value: unknown) => Promise<unknown>
  workspaceSearch?: (value: unknown) => Promise<unknown>
  cancelWorkspaceSearch?: (value: unknown) => Promise<unknown>
  createWorkspaceEntry?: (value: unknown) => Promise<unknown>
  renameWorkspaceEntry?: (value: unknown) => Promise<unknown>
  moveWorkspaceEntry?: (value: unknown) => Promise<unknown>
  copyWorkspaceEntry?: (value: unknown) => Promise<unknown>
  duplicateWorkspaceEntry?: (value: unknown) => Promise<unknown>
  deleteWorkspaceEntry?: (value: unknown) => Promise<unknown>
  revealWorkspaceEntry?: (value: unknown) => Promise<unknown>
  openWorkspaceDocument?: (value: unknown) => Promise<unknown>
  saveWorkspaceDocument?: (value: unknown) => Promise<unknown>
  saveWorkspaceDocumentAs?: (value: unknown) => Promise<unknown>
  requestWorkspaceLanguage?: (value: unknown) => Promise<unknown>
  cancelWorkspaceLanguage?: (value: unknown) => Promise<unknown>
  stopWorkspaceLanguage?: (value: unknown) => Promise<unknown>
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
  providerLanes?: Readonly<{
    capabilities?: () => Promise<unknown>
    list?: () => Promise<unknown>
    select?: (value: unknown) => Promise<unknown>
  }>
  fleetRuns?: Readonly<{ list?: () => Promise<unknown> }>
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
    pickImages?: () => Promise<import("../fable-local-contract.ts").FableLocalPickedImagesResult>
  }>
  /** Codex local lane (EP250 codex-first-class): same bridge shape. */
  codexLocal?: Readonly<{
    availability?: () => Promise<unknown>
    start?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (envelope: FableLocalEventEnvelope) => void) => () => void
    steerCurrent?: (value: unknown) => Promise<unknown>
    steerChild?: (value: unknown) => Promise<unknown>
    queueFollowup?: (value: unknown) => Promise<unknown>
    queueList?: (threadRef: unknown) => Promise<unknown>
    queueEdit?: (value: unknown) => Promise<unknown>
    queueCancel?: (value: unknown) => Promise<unknown>
    /** Full Auto (#8853): main-owned durable per-thread toggle; FA-H4
     * (#8877) adds the background-turn stop and the coarse live-state push
     * (preload schema-decodes each event before this listener sees it). */
    fullAuto?: Readonly<{
      set?: (input: unknown) => Promise<unknown>
      get?: (input: unknown) => Promise<unknown>
      interrupt?: (input: unknown) => Promise<unknown>
      onState?: (listener: (state: Readonly<{
        threadRef: string
        state: string
        turnRef: string | null
        detail?: string
      }>) => void) => () => void
    }>
  }>
  /** FA-UX-01 (#8974): the dedicated Full Auto launcher/run-view bridge. */
  fullAutoRun?: Readonly<{
    list?: () => Promise<unknown>
    start?: (value: unknown) => Promise<unknown>
    get?: (runRef: unknown) => Promise<unknown>
    pause?: (runRef: unknown) => Promise<unknown>
    resume?: (runRef: unknown) => Promise<unknown>
    stop?: (runRef: unknown) => Promise<unknown>
    retryNow?: (runRef: unknown) => Promise<unknown>
    handoff?: (value: unknown) => Promise<unknown>
    report?: (runRef: unknown) => Promise<unknown>
    receipt?: (runRef: unknown) => Promise<unknown>
  }>
  codexEcosystem?: Readonly<{
    snapshot?: () => Promise<unknown>
    mutate?: (value: unknown) => Promise<unknown>
  }>
  codexHost?: Readonly<{
    snapshot?: () => Promise<unknown>
    request?: (value: unknown) => Promise<unknown>
  }>
  codexExperimental?: Readonly<{
    snapshot?: () => Promise<unknown>
    request?: (value: unknown) => Promise<unknown>
  }>
  codexConformance?: Readonly<{
    snapshot?: () => Promise<unknown>
  }>
  codexHandoff?: Readonly<{
    open?: (value: unknown) => Promise<unknown>
  }>
  usageLedger?: Readonly<{
    snapshot?: () => Promise<unknown>
    onEvent?: (listener: (snapshot: unknown) => void) => () => void
  }>
  liveAgentGraph?: Readonly<{
    snapshot?: () => Promise<LiveAgentGraphHostSnapshot | null>
    onUpdate?: (listener: (update: LiveAgentGraphUpdate) => void) => () => void
  }>
  codingCatalog?: Readonly<{
    snapshot?: (value?: unknown) => Promise<unknown>
    choose?: () => Promise<unknown>
    open?: (value: unknown) => Promise<unknown>
    archive?: (value: unknown) => Promise<unknown>
    delete?: (value: unknown) => Promise<unknown>
    recover?: (value: unknown) => Promise<unknown>
  }>
  updates?: Readonly<{ run?: (value: unknown) => Promise<unknown> }>
  commands?: Readonly<{
    onCommand?: (listener: (command: DesktopDeferredCommand) => void) => () => void
    ready?: () => Promise<unknown>
    bindings?: () => Promise<unknown>
    saveBinding?: (value: unknown) => Promise<unknown>
    resetBindings?: () => Promise<unknown>
  }>
  /** Typed Git/GitHub surface (EP250 E2–E5): one namespaced invoke. */
  gitGithub?: Readonly<{ run?: (value: unknown) => Promise<unknown> }>
  sourceControl?: Readonly<{
    snapshot?: () => Promise<unknown>
    command?: (value: unknown) => Promise<unknown>
  }>
  /** Workspace-bounded PTY terminals (CUT-20, #8700). */
  terminal?: Readonly<{
    create?: (value: unknown) => Promise<unknown>
    input?: (value: unknown) => Promise<unknown>
    resize?: (value: unknown) => Promise<unknown>
    interrupt?: (value: unknown) => Promise<unknown>
    restart?: (value: unknown) => Promise<unknown>
    close?: (value: unknown) => Promise<unknown>
    snapshot?: () => Promise<unknown>
    openPreview?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (event: TerminalEvent) => void) => () => void
  }>
  ideRun?: Readonly<{
    snapshot?: () => Promise<unknown>
    command?: (value: unknown) => Promise<unknown>
    onEvent?: (listener: (event: IdeRunEvent) => void) => () => void
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
  acpProviders?: Readonly<{
    status?: () => Promise<unknown>
    action?: (value: unknown) => Promise<unknown>
    supportBundle?: () => Promise<unknown>
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
  moveWorkspaceEntry: (value) => readBridge()?.moveWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.moveWorkspaceEntry(value),
  copyWorkspaceEntry: (value) => readBridge()?.copyWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.copyWorkspaceEntry(value),
  duplicateWorkspaceEntry: (value) => readBridge()?.duplicateWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.duplicateWorkspaceEntry(value),
  deleteWorkspaceEntry: (value) => readBridge()?.deleteWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.deleteWorkspaceEntry(value),
  revealWorkspaceEntry: (value) => readBridge()?.revealWorkspaceEntry?.(value) ?? unavailableWorkspaceBrowserBridge.revealWorkspaceEntry(value),
  refreshWorkspace: () => readBridge()?.refreshWorkspace?.() ?? unavailableWorkspaceBrowserBridge.refreshWorkspace(),
}

const workspaceDocumentBridge: WorkspaceDocumentBridge = {
  openWorkspaceDocument: (value) => readBridge()?.openWorkspaceDocument?.(value) ?? unavailableWorkspaceDocumentBridge.openWorkspaceDocument(value),
  saveWorkspaceDocument: (value) => readBridge()?.saveWorkspaceDocument?.(value) ?? unavailableWorkspaceDocumentBridge.saveWorkspaceDocument(value),
  saveWorkspaceDocumentAs: (value) => readBridge()?.saveWorkspaceDocumentAs?.(value) ?? unavailableWorkspaceDocumentBridge.saveWorkspaceDocumentAs(value),
}

const workspaceLanguageBridge: WorkspaceLanguageBridge = {
  requestWorkspaceLanguage: value => readBridge()?.requestWorkspaceLanguage?.(value) ?? unavailableWorkspaceLanguageBridge.requestWorkspaceLanguage(value),
  cancelWorkspaceLanguage: value => readBridge()?.cancelWorkspaceLanguage?.(value) ?? unavailableWorkspaceLanguageBridge.cancelWorkspaceLanguage(value),
  stopWorkspaceLanguage: value => readBridge()?.stopWorkspaceLanguage?.(value) ?? unavailableWorkspaceLanguageBridge.stopWorkspaceLanguage(value),
}

const ideAgentCodeRendererHost: IdeAgentCodeRendererHost = {
  snapshot: () => readBridge()?.ideAgentCode?.snapshot?.() ?? unavailableIdeAgentCodeRendererHost.snapshot(),
  command: value => readBridge()?.ideAgentCode?.command?.(value) ?? unavailableIdeAgentCodeRendererHost.command(value),
}

const ideCursorRendererHost: IdeCursorRendererHost = {
  snapshot: () => readBridge()?.ideCursor?.snapshot?.() ?? unavailableIdeCursorRendererHost.snapshot(),
  command: value => readBridge()?.ideCursor?.command?.(value) ?? unavailableIdeCursorRendererHost.command(value),
}

const ideManagedSandboxRendererHost: IdeManagedSandboxRendererHost = {
  snapshot: () => readBridge()?.ideManagedSandbox?.snapshot?.() ?? unavailableIdeManagedSandboxRendererHost.snapshot(),
  command: value => readBridge()?.ideManagedSandbox?.command?.(value) ?? unavailableIdeManagedSandboxRendererHost.command(value),
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
  fleetRuns: () => {
    const bridge = readBridge()
    return typeof bridge?.fleetRuns?.list === "function"
      ? bridge.fleetRuns.list()
      : Promise.resolve({ state: "unavailable" })
  },
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
        // CUT-16: carry the confirmed provider lane so the durable lane fence
        // admits controls on Claude/hosted turns instead of rejecting the
        // hard-coded Codex default.
        ...(command.lane === undefined ? {} : { lane: command.lane }),
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
let sourceControlOperationSequence = 0
const sourceControlOperationRef = () => IdeSourceControlOperationRefSchema.make(
  `ide.scm-operation.renderer-${Date.now()}-${++sourceControlOperationSequence}`,
)
const sourceControlActor = { _tag: "Human" as const, actorRef: "desktop.owner" }

const legacyStatus = (value: IdeSourceControlSnapshot) => {
  const fileStatus = (state: string): GitFileStatus => {
    if (state === "type_changed") return "type-changed"
    if (state === "conflicted") return "unmerged"
    if (["added", "modified", "deleted", "renamed", "copied", "untracked"].includes(state)) return state as GitFileStatus
    return "modified"
  }
  return {
    ok: true as const,
    op: "status" as const,
    branch: value.branch,
    upstream: value.upstream,
    detached: value.detached,
    ahead: value.ahead,
    behind: value.behind,
    staged: value.paths.filter(entry => entry.indexState !== "unmodified").map(entry => ({ path: entry.path, status: fileStatus(entry.indexState) })),
    unstaged: value.paths.filter(entry => entry.worktreeState !== "unmodified" && entry.worktreeState !== "untracked").map(entry => ({ path: entry.path, status: fileStatus(entry.worktreeState) })),
    untracked: value.paths.filter(entry => entry.worktreeState === "untracked").map(entry => ({ path: entry.path, status: "untracked" as const })),
    truncated: value.truncated,
    repositoryRef: value.binding.repositoryRef,
    statusRef: value.version.statusRef,
    headRef: value.version.headOid,
    delivery: value.delivery.map(fact => ({
      phase: fact.phase,
      proven: fact.proven,
      freshness: fact.freshness,
      evidenceRefs: fact.evidenceRefs,
    })),
  }
}

const sourceControlError = (op: GitGithubOp, result: unknown) => {
  const decoded = decodeIdeSourceControlCommandResult(result)
  if (decoded?._tag !== "Failure") return gitGithubError(op, "operation_failed", "The source-control operation did not return a receipt.")
  const code = decoded.failure.code
  return gitGithubError(
    op,
    code === "stale_version" ? "stale_status"
      : code === "non_fast_forward" ? "non_fast_forward"
      : code === "hook_failed" ? "blocked_by_hook"
      : code === "credential_unavailable" ? "auth_failed"
      : code === "dirty_state" ? "dirty_tree"
      : "operation_failed",
    decoded.failure.message,
  )
}

const gitGithubBridge: GitGithubBridge = {
  run: async (value: unknown) => {
    const request = decodeGitGithubRequest(value)
    if (request === null) return gitGithubError("status", "invalid_request", "The Git request could not be decoded.")
    const bridge = readBridge()
    const legacyRun = bridge?.gitGithub?.run
    const snapshotRun = bridge?.sourceControl?.snapshot
    const commandRun = bridge?.sourceControl?.command
    if (request.op === "issueList" || request.op === "issueView" || request.op === "issueCreate" || request.op === "prList" || request.op === "prView" || request.op === "prCreate" || request.op === "branchList") {
      return typeof legacyRun === "function" ? legacyRun(value) : unavailableGitGithubBridge.run(value)
    }
    if (typeof snapshotRun !== "function") return gitGithubError(request.op, "no_workspace", "Choose a Git workspace first.")
    const snapshot = decodeIdeSourceControlSnapshot(await snapshotRun())
    if (snapshot === null) return gitGithubError(request.op, "not_a_repo", "The source-control snapshot is unavailable.")
    if (request.op === "status") return legacyStatus(snapshot)
    if (request.repositoryRef !== snapshot.binding.repositoryRef || request.statusRef !== snapshot.version.statusRef) {
      return gitGithubError(request.op, "stale_status", "Repository state changed. Refresh and preview the operation again.")
    }
    if (request.op === "diff") {
      if (typeof legacyRun !== "function") return unavailableGitGithubBridge.run(value)
      const observed = await legacyRun({ op: "status" }) as { repositoryRef?: unknown; statusRef?: unknown }
      if (typeof observed.repositoryRef !== "string" || typeof observed.statusRef !== "string") return gitGithubError("diff", "operation_failed", "The diff adapter is unavailable.")
      const result = await legacyRun({ ...request, repositoryRef: observed.repositoryRef, statusRef: observed.statusRef })
      return typeof result === "object" && result !== null && "ok" in result && result.ok === true
        ? { ...result, repositoryRef: snapshot.binding.repositoryRef, statusRef: snapshot.version.statusRef }
        : result
    }
    if (typeof commandRun !== "function") return gitGithubError(request.op, "operation_failed", "The source-control authority is unavailable.")
    const mutation = {
      operationRef: sourceControlOperationRef(),
      binding: snapshot.binding,
      expected: snapshot.version,
      actor: sourceControlActor,
      approvalRef: null,
    }
    const command = request.op === "stage" ? { _tag: "Stage" as const, ...mutation, selection: { _tag: "Paths" as const, paths: request.paths } }
      : request.op === "unstage" ? { _tag: "Unstage" as const, ...mutation, selection: { _tag: "Paths" as const, paths: request.paths } }
      : request.op === "discard" ? { _tag: "Discard" as const, ...mutation, selection: { _tag: "Paths" as const, paths: [request.path] }, recoveryRequired: true as const }
      : request.op === "recover" ? { _tag: "Recover" as const, ...mutation, recoveryRef: request.recoveryRef as never }
      : request.op === "commit" ? { _tag: "Commit" as const, ...mutation, message: request.message, amend: false, sign: false, runHooks: true }
      : request.op === "branchCreate" ? { _tag: "BranchCreate" as const, ...mutation, name: request.name, checkout: request.checkout }
      : request.op === "checkout" ? { _tag: "Switch" as const, ...mutation, refName: request.name, detach: false }
      : request.op === "push" && snapshot.branch !== null ? {
          _tag: "Push" as const,
          ...mutation,
          remote: snapshot.upstream?.split("/", 1)[0] ?? "origin",
          refspec: `HEAD:refs/heads/${snapshot.branch}`,
          forcePolicy: "forbid" as const,
          expectedRemoteOid: null,
        }
      : null
    if (command === null) return gitGithubError(request.op, "no_upstream", "The current branch has no push destination.")
    const rawResult = await commandRun(command)
    const result = decodeIdeSourceControlCommandResult(rawResult)
    if (result?._tag !== "Success") return sourceControlError(request.op, rawResult)
    if (request.op === "stage" || request.op === "unstage") return { ok: true, op: request.op, paths: request.paths }
    if (request.op === "discard") return { ok: true, op: "discard", repositoryRef: snapshot.binding.repositoryRef, path: request.path, statusRef: result.snapshot.version.statusRef, recoveryRef: result.receipt?.recoveryRef ?? undefined }
    if (request.op === "recover") return { ok: true, op: "recover", repositoryRef: snapshot.binding.repositoryRef, statusRef: result.snapshot.version.statusRef, recoveryRef: request.recoveryRef }
    if (request.op === "commit") {
      const sha = result.snapshot.version.headOid ?? ""
      return { ok: true, op: "commit", sha, shortSha: sha.slice(0, 7), summary: request.message.split("\n", 1)[0] ?? "Commit" }
    }
    if (request.op === "push") return { ok: true, op: "push", ref: result.snapshot.branch ?? "HEAD", remote: snapshot.upstream?.split("/", 1)[0] ?? "origin", sha: result.snapshot.version.headOid ?? "" }
    if (request.op === "branchCreate") return { ok: true, op: "branchCreate", name: request.name, checkedOut: request.checkout }
    return { ok: true, op: "checkout", name: request.name }
  },
}

const terminalRendererBridge: TerminalRendererBridge = {
  create: (value) => readBridge()?.terminal?.create?.(value) ?? unavailableTerminalBridge.create(value),
  input: (value) => readBridge()?.terminal?.input?.(value) ?? unavailableTerminalBridge.input(value),
  resize: (value) => readBridge()?.terminal?.resize?.(value) ?? unavailableTerminalBridge.resize(value),
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

// Typed per-harness maintenance (MAINT-1, #8785): the settings surface drives
// the gateway's maintenance query/command; the renderer sees only the
// public-safe typed projection.
let harnessMaintenanceRequestSequence = 0
const harnessMaintenanceSettingsBridge: HarnessMaintenanceSettingsBridge = {
  status: async () => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "query",
      requestId: `renderer-harness-maintenance-status-${++harnessMaintenanceRequestSequence}`,
      query: { id: "maintenance.harness_status", harness: "codex" },
    })
  },
  update: async harness => {
    const bridge = readBridge()
    if (typeof bridge?.runtimeRequest !== "function") return null
    return bridge.runtimeRequest({
      kind: "command",
      commandId: `renderer-harness-maintenance-update-${++harnessMaintenanceRequestSequence}`,
      command: { id: "maintenance.harness_update", harness },
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
    const bridge = readBridge()
    const launchContext = decodeDesktopLaunchContext(bridge?.launchContext)
    const documentLaunch = launchContext.documentOpenPathRef !== null
    const initialState = initialDesktopShellState(
      host,
      formatShellTimestamp(new Date()),
      documentLaunch ? "files" : "chat",
    )
    const state = yield* SubscriptionRef.make({
      ...initialState,
      ...(bridge?.smokeProviderTurns === true ? { openAgentsStandby: false } : {}),
    })
    const initialAgentCode = yield* Effect.promise(() =>
      loadAgentCodeRendererSnapshot(ideAgentCodeRendererHost).catch(() =>
        loadAgentCodeRendererSnapshot(unavailableIdeAgentCodeRendererHost)))
    const initialIdeCursor = yield* Effect.promise(() =>
      loadIdeCursorRendererSnapshot(ideCursorRendererHost).catch(() =>
        loadIdeCursorRendererSnapshot(unavailableIdeCursorRendererHost)))
    const initialManagedSandbox = yield* Effect.promise(() =>
      loadIdeManagedSandboxRendererSnapshot(ideManagedSandboxRendererHost).catch(() =>
        loadIdeManagedSandboxRendererSnapshot(unavailableIdeManagedSandboxRendererHost)))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      agentCode: initialAgentCode,
      ideCursor: { ...current.ideCursor, snapshot: initialIdeCursor },
      managedSandbox: initialManagedSandbox,
    }))
    let cursorInvalidationOrdinal = 0
    yield* Effect.forkScoped(Stream.runForEach(SubscriptionRef.changes(state), latest => {
      const invalidated = invalidateIdeCursorRendererState(latest.ideCursor, latest)
      if (invalidated === latest.ideCursor) return Effect.void
      return Effect.gen(function* () {
        yield* SubscriptionRef.update(state, current => current.ideCursor === latest.ideCursor
          ? { ...current, ideCursor: invalidated }
          : current)
        const result = yield* Effect.promise(() => cancelInvalidatedIdeCursor(
          ideCursorRendererHost,
          latest.ideCursor,
          invalidated,
          ++cursorInvalidationOrdinal,
        ))
        if (result === null) return
        yield* SubscriptionRef.update(state, current => current.ideCursor === invalidated
          ? { ...current, ideCursor: { ...invalidated, snapshot: result.snapshot } }
          : current)
      })
    }))
    const program = makeViewProgramFromState(state, desktopShellView)
    document.documentElement.dataset.desktopPlatform = bridge?.platform ?? "unknown"
    if (typeof bridge?.localTurnRecovery?.onUpdate === "function") {
      const unsubscribeRecovery = bridge.localTurnRecovery.onUpdate(thread => {
        void Effect.runPromise(SubscriptionRef.update(state, current =>
          withThreads(current, [thread, ...current.threads.filter(value => value.id !== thread.id)])))
      })
      window.addEventListener("pagehide", () => unsubscribeRecovery(), { once: true })
    }
    // FA-H4 (#8877): coarse Full Auto live-state push — same direct
    // SubscriptionRef-update style as the recovery subscription above. Each
    // event (already schema-decoded by the preload) projects one thread's
    // background-turn state so the composer's badge/Stop/fencing reflect it.
    if (typeof bridge?.codexLocal?.fullAuto?.onState === "function") {
      const unsubscribeFullAutoState = bridge.codexLocal.fullAuto.onState(event => {
        void Effect.runPromise(SubscriptionRef.update(state, current =>
          withFullAutoLiveState(current, event.threadRef, { state: event.state, turnRef: event.turnRef })))
      })
      window.addEventListener("pagehide", () => unsubscribeFullAutoState(), { once: true })
    }
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
        const raw = await bridge?.historyThreads?.listLocal?.()
        return Array.isArray(raw) ? raw as DesktopThread[] : []
      },
      newThread: async (laneRef?: string) => {
        const raw = await bridge?.newThread?.({ laneRef: laneRef ?? "codex-local" })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      selectLane: async (threadRef: string, laneRef: string) => {
        const raw = await bridge?.providerLanes?.select?.({ threadRef, laneRef })
        return typeof raw === "object" && raw !== null && typeof (raw as { ok?: unknown }).ok === "boolean"
          ? raw as { ok: boolean; reason?: string; message?: string }
          : { ok: false, reason: "unknown_lane", message: "Provider lane registry unavailable." }
      },
      laneForThread: async (threadRef: string) => {
        const raw = await bridge?.providerLanes?.list?.()
        if (typeof raw !== "object" || raw === null || !Array.isArray((raw as { selections?: unknown }).selections)) return null
        const selection = ((raw as { selections: Array<{ threadRef?: unknown; laneRef?: unknown }> }).selections)
          .find(row => row.threadRef === threadRef)
        return typeof selection?.laneRef === "string" ? selection.laneRef : "codex-local"
      },
      openThread: async (id: string) => {
        const raw = await bridge?.historyThreads?.resumeLocal?.({ threadRef: id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      hydrateThread: async (id: string) => {
        const raw = await bridge?.historyThreads?.resumeLocal?.({ threadRef: id })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      renameThread: async (input: Readonly<{ threadRef: string; title: string }>) => {
        const raw = await bridge?.historyThreads?.renameLocal?.(input)
        if (typeof raw !== "object" || raw === null || typeof (raw as { ok?: unknown }).ok !== "boolean") {
          return { ok: false as const, error: "Desktop chat returned an invalid rename response." }
        }
        return raw as { ok: boolean; thread?: DesktopThread; error?: string }
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
            ...(typeof bridge.codexLocal.steerCurrent === "function"
              ? { steerCurrent: bridge.codexLocal.steerCurrent }
              : {}),
            ...(typeof bridge.codexLocal.steerChild === "function"
              ? { steerChild: bridge.codexLocal.steerChild }
              : {}),
            ...(typeof bridge.codexLocal.queueFollowup === "function"
              ? { queueFollowup: bridge.codexLocal.queueFollowup }
              : {}),
            ...(typeof bridge.codexLocal.queueList === "function" ? { queueList: bridge.codexLocal.queueList } : {}),
            ...(typeof bridge.codexLocal.queueEdit === "function" ? { queueEdit: bridge.codexLocal.queueEdit } : {}),
            ...(typeof bridge.codexLocal.queueCancel === "function" ? { queueCancel: bridge.codexLocal.queueCancel } : {}),
          }
        : null
    let codexAvailability: CodexLocalAvailability | null = null
    const localHarnessChat = makeLocalHarnessChatHost({
      base: localChat,
      fable: fableLocalBridge,
      fableAvailability: () => fableAvailability,
      codex: codexLocalBridge,
      codexAvailability: () => codexAvailability,
      onComposerAdmission: (threadRef, admission) => {
        Effect.runFork(SubscriptionRef.update(state, current => ({
          ...current,
          composerAdmissionByThread: { ...current.composerAdmissionByThread, [threadRef]: admission },
          ...(current.activeThreadId === threadRef ? { composerAdmission: admission } : {}),
        })))
      },
      onComposerQueue: (threadRef, composerQueue) => {
        Effect.runFork(SubscriptionRef.update(state, current =>
          current.activeThreadId === threadRef
            ? { ...current, composerQueue: composerQueue.filter(entry => entry.threadRef === threadRef) }
            : current))
      },
    })
    // Owner directive 2026-07-20: never block first paint on the runtime-gateway
    // `conversation.catalog` query. Start LOCAL (the Finder "Open With" fast
    // path already proved this), and let the converging chat host below re-admit
    // each operation against the live runtime per-operation — no initial mode
    // query, no delay to first paint.
    const selection = { host: localHarnessChat, mode: "local" as const }
    // The initial selection gates first-paint lane chrome, but it is never a
    // lifetime routing decision: verified Sync may still be catching up. The
    // converging facade re-admits each operation with one authoritative query
    // and pins existing thread refs to their creating host (CUT-10).
    const convergingChat = makeConvergingDesktopChatHost({
      request: bridge?.runtimeRequest,
      subscribe: bridge?.runtimeSubscribe,
      local: localHarnessChat,
    })
    const chat = {
      ...convergingChat,
      reconcileControlOutcome: async (lookup: DesktopRuntimeControlOutcomeLookup) => {
        const result = decodeDesktopRuntimeControlOutcomeLookupResult(
          await bridge?.controlOutcomes?.lookup?.(lookup),
        )
        if (result?.status === "found") return { status: "found" as const, outcome: result.record.outcome }
        if (result?.status === "missing") return { status: "missing" as const }
        return { status: "unavailable" as const }
      },
      recordControlOutcome: async (record: DesktopRuntimeControlOutcomeRecord): Promise<boolean> => {
        const result = decodeDesktopRuntimeControlOutcomeRecordResult(
          await bridge?.controlOutcomes?.record?.(record),
        )
        return result !== null && result.status !== "rejected"
      },
    }
    // Owner directive 2026-07-20 (local-first startup): the shell always mounts in
    // local mode, so owner-card answers route to the local harness question host.
    // Runtime/cloud question routing, when that path ships, belongs in the
    // per-operation converging router (which pins each thread to its creating
    // host), never a static mount-time selection that would block first paint.
    const questionHost = localQuestionHost
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
      snapshot: () => codingCatalogCall(bridge?.codingCatalog?.snapshot === undefined
        ? undefined
        : () => bridge.codingCatalog!.snapshot!({ offset: 0 })),
      page: (offset: number) => codingCatalogCall(bridge?.codingCatalog?.snapshot === undefined
        ? undefined
        : () => bridge.codingCatalog!.snapshot!({ offset })),
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
      delete: (sessionRef: string) => codingCatalogCall(
        bridge?.codingCatalog?.delete === undefined
          ? undefined
          : () => bridge.codingCatalog!.delete!({ sessionRef }),
      ),
      recover: (sessionRef: string) => codingCatalogCall(
        bridge?.codingCatalog?.recover === undefined
          ? undefined
          : () => bridge.codingCatalog!.recover!({ sessionRef }),
      ),
    }
    const updateRendererHost = {
      run: async (action: "snapshot" | "check" | "download" | "open_installer" | "apply" | "rollback") => {
        try {
          const value = await bridge?.updates?.run?.({ action })
          return decodeDesktopUpdateProjection(value) ?? emptyDesktopUpdateProjection()
        } catch {
          return emptyDesktopUpdateProjection()
        }
      },
    }
    const decodeCommandBindings = (value: unknown): DesktopCommandBindingProjection | null =>
      decodeDesktopCommandBindingProjectionOrNull(value)
    const commandBindingHost = {
      snapshot: async () => decodeCommandBindings(await bridge?.commands?.bindings?.()),
      save: async (input: Readonly<{ commandId: DesktopCommandId; chord: string | null }>) =>
        decodeCommandBindings(await bridge?.commands?.saveBinding?.(input)),
      reset: async () => decodeCommandBindings(await bridge?.commands?.resetBindings?.()),
    }
    // Owner directive 2026-07-20: startup discovery is DEFERRED so first paint is
    // never blocked on availability + catalog IPC round-trips (the Finder "Open
    // With" path already proved this fast path). The shell mounts IMMEDIATELY
    // with an honest "verifying…" lane state; these probes then stream real
    // availability plus the installed model catalog into the mounted state as
    // evidence lands. The Codex chip still only lights on a PROBE-VERIFIED
    // account, and a failed decode settles on the reconnect reason rather than
    // parking on "verifying…" forever.
    const localLanes = (): HarnessLanes => ({
      fable: fableAvailability?.state === "available"
        ? { available: true, reason: null }
        : { available: false, reason: "Fable — unavailable: no linked Claude account" },
      codex: codexLocalBridge === null
        ? { available: false, reason: "Codex — no verified account · Reconnect in Settings" }
        : codexHarnessLaneFromAvailability(codexAvailability),
    })
    yield* SubscriptionRef.update(state, current => withHarnessLanes(current, localLanes()))
    const refreshLaneCapabilities = async (): Promise<void> => {
      const refreshed = decodeProviderLaneComposerProjections(
        await (bridge?.providerLanes?.capabilities?.().catch(() => null) ?? Promise.resolve(null)),
      )
      if (refreshed !== null) {
        await Effect.runPromise(SubscriptionRef.update(state, current =>
          withProviderLaneCapabilities(current, refreshed)))
      }
    }
    if (!documentLaunch) {
      if (fableLocalBridge !== null) {
        void fableLocalBridge.availability()
          .catch(() => null)
          .then(async raw => {
            fableAvailability = decodeFableLocalAvailability(raw)
            await Effect.runPromise(SubscriptionRef.update(state, current => withHarnessLanes(current, localLanes())))
            await refreshLaneCapabilities()
          })
          .catch(() => {})
      }
      if (codexLocalBridge !== null) {
        // The codex probe round can take tens of seconds on broken accounts.
        void codexLocalBridge.availability()
          .catch(() => null)
          .then(async raw => {
            codexAvailability = decodeCodexLocalAvailability(raw) ??
              { state: "unavailable", reason: "no_verified_account" }
            await Effect.runPromise(SubscriptionRef.update(state, current => withHarnessLanes(current, localLanes())))
            await refreshLaneCapabilities()
          })
          .catch(() => {})
      } else {
        void refreshLaneCapabilities().catch(() => {})
      }
    }
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      questionAnswerHostAvailable: questionHost.answer !== null,
    }))
    // Apple FM discovery for the Boot Sequence (owner directive 2026-07-20):
    // probe the native bridge and, when it reports ready, run one bounded test
    // inference so the scan proves Apple FM actually answers. Non-blocking — the
    // shell mounts immediately and the scan updates when evidence lands.
    void (async () => {
      const setBoot = (boot: AppleFmBootState): Promise<unknown> =>
        Effect.runPromise(SubscriptionRef.update(state, current => ({ ...current, appleFmBoot: boot })))
      const appleFm = readBridge()?.appleFm
      if (appleFm?.status === undefined) {
        await setBoot({ status: "unavailable", detail: "bridge unavailable", testInference: null })
        return
      }
      const status = await appleFm.status().catch(() => null)
      if (status === null || !status.ready) {
        await setBoot({ status: "unavailable", detail: status?.unavailableReason ?? "not detected", testInference: null })
        return
      }
      const detail = status.model ?? status.mode
      await setBoot({ status: "available", detail, testInference: null })
      const turn = await appleFm.startTurn?.({ prompt: "Reply in one short sentence to confirm you are online." })
        .catch(() => null)
      if (turn != null && turn.outcome === "completed" && turn.text !== null) {
        await setBoot({ status: "available", detail, testInference: turn.text.trim().slice(0, 200) })
      }
    })().catch(() => {})
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
      localThreads: async (): Promise<ReadonlyArray<DesktopThread>> => {
        const raw = await bridge?.historyThreads?.listLocal?.()
        return Array.isArray(raw)
          ? raw.filter((value): value is DesktopThread => typeof value === "object" && value !== null && typeof (value as { id?: unknown }).id === "string")
          : []
      },
      resumeLocalThread: async (threadRef: string): Promise<DesktopThread | null> => {
        const raw = await bridge?.historyThreads?.resumeLocal?.({ threadRef })
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      forkLocalThread: async (request: Readonly<{ sourceThreadRef: string; throughSequence: number | null }>): Promise<DesktopThread | null> => {
        const raw = await bridge?.historyThreads?.fork?.(request)
        return typeof raw === "object" && raw !== null && typeof (raw as { id?: unknown }).id === "string" ? raw as DesktopThread : null
      },
      save: (value: any): void => { try { localStorage.setItem("openagents.desktop.history.v1",JSON.stringify({...value,expandedThreadRefs:Array.isArray(value?.expandedThreadRefs)?value.expandedThreadRefs:[]})) } catch { /* restoration is best effort and contains refs only */ } },
    }
    // One shared transient command-notice controller: threaded into the shell
    // handlers AND the deferred-command dispatch below so both paths cancel one
    // another's pending auto-clear. shutdown is a scope finalizer so a pending
    // clear fiber can never fire after the renderer unmounts.
    const noticeController = makeCommandNoticeController(state)
    yield* Effect.addFinalizer(() => noticeController.shutdown)
    if (import.meta.hot !== undefined) {
      const onPreviewChange = (raw: unknown): void => {
        const change = decodeDesktopPreviewChangeEvent(raw)
        if (change === null) return
        if (change.kind === "host_restart_required" || change.kind === "dependency_sync_required") {
          const action = change.kind === "dependency_sync_required" ? "dependency sync and preview restart" : "preview restart"
          void Effect.runPromise(noticeController.setTransientNotice(`${change.pathRef} requires ${action}; it was not hot-applied.`))
          return
        }
        if (change.kind !== "renderer_reload_required") return
        void Effect.runPromise(SubscriptionRef.get(state)).then(current => {
          if (desktopPreviewReloadRisk(current) && !window.confirm("Reload the worktree preview? An unsent draft, attachment, or pending owner interaction may be discarded.")) {
            return Effect.runPromise(noticeController.setTransientNotice("Preview reload deferred to preserve unsent renderer state."))
          }
          window.location.reload()
        }).catch(() => {})
      }
      import.meta.hot.on("openagents:preview-change", onPreviewChange)
      const onViteFullReload = (): void => {
        const current = Effect.runSync(SubscriptionRef.get(state))
        if (desktopPreviewReloadRisk(current)) {
          window.confirm("Vite must reload this worktree preview. An unsent draft, attachment, or pending owner interaction may be discarded.")
        }
      }
      import.meta.hot.on("vite:beforeFullReload", onViteFullReload)
      yield* Effect.addFinalizer(() => Effect.sync(() => {
        import.meta.hot?.off("openagents:preview-change", onPreviewChange)
        import.meta.hot?.off("vite:beforeFullReload", onViteFullReload)
      }))
    }
    let voiceCommandSequence = 0
    const voiceFinalLedger = makeVoiceFinalLedger()
    let voiceMessageBusy = false
    const voiceHost = {
      command: async (command: Readonly<Record<string, unknown>>) => {
        if (typeof bridge?.runtimeRequest !== "function") return null
        const response = await bridge.runtimeRequest({
          kind: "command",
          commandId: `renderer-voice-${++voiceCommandSequence}`,
          command,
        })
        return response.kind === "voice_state" ? response.state : null
      },
    }
    // Full Auto (#8853, FA-H1 #8874): main-owned durable per-thread toggle.
    // Absent preload/bridge degrades to a no-op set and an always-off get --
    // the composer toggle still works locally, it just cannot survive a
    // restart without this bridge. `get` is genuinely called: once below at
    // mount to seed the active thread's toggle from durable truth, and again
    // by the shell's thread-selection path on every switch.
    const fullAutoHost = {
      set: async (input: Readonly<{ threadRef: string; enabled: boolean }>) =>
        (await readBridge()?.codexLocal?.fullAuto?.set?.(input)) ?? { ok: false },
      get: async (input: Readonly<{ threadRef: string }>) => {
        const raw = await readBridge()?.codexLocal?.fullAuto?.get?.(input)
        if (typeof raw !== "object" || raw === null || typeof (raw as { enabled?: unknown }).enabled !== "boolean") {
          return { enabled: false }
        }
        const value = raw as { enabled: boolean; state?: unknown; turnRef?: unknown }
        // FA-H4 (#8877): the live-state fields are additive/optional — an
        // older main that returns only { enabled } keeps hydration working.
        return {
          enabled: value.enabled,
          ...(typeof value.state === "string" ? { state: value.state } : {}),
          ...(typeof value.turnRef === "string" || value.turnRef === null ? { turnRef: value.turnRef } : {}),
        }
      },
      // FA-H4 (#8877): thread-scoped stop for the background continuation
      // turn. Absent preload degrades to { ok: false } (Stop no-ops).
      interrupt: async (input: Readonly<{ threadRef: string }>) => {
        const raw = await readBridge()?.codexLocal?.fullAuto?.interrupt?.(input)
        return typeof raw === "object" && raw !== null && (raw as { ok?: unknown }).ok === true
          ? { ok: true }
          : { ok: false }
      },
    }
    // FA-UX-01 (#8974): the dedicated Full Auto launcher/run-view bridge.
    // Every method is a thin pass-through to `bridge.fullAutoRun` (absent
    // preload degrades to the raw bridge's own `unavailableFullAutoRunOutcome`
    // shape); `full-auto-workspace.ts` is the one place that schema-decodes
    // the raw payload.
    const fullAutoRunHost: FullAutoRunRendererHost = {
      list: () => readBridge()?.fullAutoRun?.list?.() ?? Promise.resolve({ runs: [], resolvedWorkspaceRef: null }),
      start: request => readBridge()?.fullAutoRun?.start?.(request) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      get: runRef => readBridge()?.fullAutoRun?.get?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      pause: runRef => readBridge()?.fullAutoRun?.pause?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      resume: runRef => readBridge()?.fullAutoRun?.resume?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      stop: runRef => readBridge()?.fullAutoRun?.stop?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      retryNow: runRef => readBridge()?.fullAutoRun?.retryNow?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      handoff: request => readBridge()?.fullAutoRun?.handoff?.(request) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      report: runRef => readBridge()?.fullAutoRun?.report?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
      receipt: runRef => readBridge()?.fullAutoRun?.receipt?.(runRef) ?? Promise.resolve(unavailableFullAutoRunOutcome()),
    }
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
        workingDirectory: async () => {
          const value = await readBridge()?.workingDirectory?.()
          return typeof value === "string" && value.length > 0 ? value : null
        },
        browser: workspaceBrowserBridge,
        documents: workspaceDocumentBridge,
        language: workspaceLanguageBridge,
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
          return typeof pick === "function" ? await pick() : { images: [], rejection: null }
        },
      }, terminalRendererBridge, noticeController, {
        // Diagnostics/watchdog bridge (CUT-24 #8704). Absent preload degrades to
        // an unavailable projection; the renderer schema-decodes every result.
        gather: () => readBridge()?.diagnostics?.gather?.() ?? Promise.resolve(null),
        runAction: (action) => readBridge()?.diagnostics?.runAction?.(action) ?? Promise.resolve({ ok: false, notice: "Diagnostics unavailable" }),
        exportRedacted: () => readBridge()?.diagnostics?.exportRedacted?.() ?? Promise.resolve({ ok: false, notice: "Diagnostics unavailable" }),
      }, voiceHost, {
        open: async request => {
          const result = await readBridge()?.codexHandoff?.open?.(request)
          if (typeof result === "object" && result !== null &&
            ((result as { state?: unknown }).state === "opened" || (result as { state?: unknown }).state === "refused") &&
            typeof (result as { message?: unknown }).message === "string") {
            return result as import("../codex-handoff-contract.ts").CodexHandoffOpenResult
          }
          return {
            state: "refused",
            reason: "invalid_request",
            message: "The Open in Codex response was invalid.",
          }
        },
      }, updateRendererHost, harnessMaintenanceSettingsBridge, {
        setSidebarCollapsed: async (sidebarCollapsed) => {
          await readBridge()?.preferences?.update?.({ presentation: { sidebarCollapsed } })
        },
        setLocalCodexUsageSharing: async (enabled) => {
          await readBridge()?.preferences?.update?.({ privacy: { shareLocalCodexUsage: enabled } })
        },
        setEditorVimEnabled: async (enabled) => {
          await readBridge()?.preferences?.update?.({ editor: { vim: { enabled } } })
        },
      }, fullAutoHost, {
        status: () => readBridge()?.acpProviders?.status?.() ?? unavailableAcpProviderSettingsBridge.status(),
        action: (provider, action) => readBridge()?.acpProviders?.action?.({ provider, action }) ?? unavailableAcpProviderSettingsBridge.action(provider, action),
        supportExport: () => readBridge()?.acpProviders?.supportBundle?.() ?? unavailableAcpProviderSettingsBridge.supportExport(),
      } satisfies AcpProviderSettingsBridge, {
        snapshot: () => readBridge()?.codexExperimental?.snapshot?.() ?? Promise.resolve(null),
        request: value => readBridge()?.codexExperimental?.request?.(value) ?? Promise.resolve({ ok: false, reason: "unavailable" }),
      }, fullAutoRunHost, ideAgentCodeRendererHost, ideCursorRendererHost, ideManagedSandboxRendererHost, {
        // Apple FM on-device answer host (owner directive 2026-07-20): run one
        // bounded local turn and return its reply text, or null when the model
        // refuses/fails or the bridge is unavailable. Availability is gated by
        // the boot-sequence probe in shell state, so this is only called when
        // Apple FM already reported ready.
        respond: async (prompt: string) => {
          const appleFm = readBridge()?.appleFm
          if (appleFm?.startTurn === undefined) return null
          const turn = await appleFm.startTurn({ prompt }).catch(() => null)
          return turn != null && turn.outcome === "completed" && turn.text !== null ? turn.text : null
        },
      } satisfies DesktopAppleFmChatHost),
    )
    if (!documentLaunch && typeof bridge?.runtimeRequest === "function") {
      // Non-blocking: the initial voice-state query never gates first paint.
      // The live lifecycle subscription below keeps the mounted state fresh, so
      // this is only a one-shot backfill that streams in when it lands.
      void bridge.runtimeRequest({
        kind: "query",
        requestId: "renderer-voice-initial",
        query: { id: "voice.state" },
      })
        .then(response => {
          if (response.kind !== "voice_state") return
          return Effect.runPromise(SubscriptionRef.update(state, current => ({
            ...current,
            voice: withVoiceHostState(current.voice, response.state),
          })))
        })
        .catch(() => {})
    }
    if (typeof bridge?.runtimeSubscribe === "function") {
      const unsubscribeVoice = bridge.runtimeSubscribe(event => {
        if (event.kind !== "voice.lifecycle") return
        const arrivedDuringMessage = voiceMessageBusy
        void Effect.runPromise(Effect.gen(function* () {
          const before = yield* SubscriptionRef.get(state)
          yield* SubscriptionRef.update(state, current => ({
            ...current,
            voice: withVoiceHostState(current.voice, event.state),
          }))
          const transcript = event.state.transcript
          if (transcript?.final !== true || before.voice.sessionRef === null || event.state.generation !== before.voice.host.generation) return
          // The microphone is visibly paused for an admitted message turn. A
          // provider final already buffered during that pause belongs to the
          // closed turn boundary and must never replace/queue a fragment.
          if (arrivedDuringMessage) return
          const action = voiceFinalLedger.admit({
            sessionRef: before.voice.sessionRef,
            generation: event.state.generation,
            utteranceRef: transcript.utteranceRef,
            text: transcript.text,
          }, before.voice.host.generation)
          if (action === null) return
          yield* Effect.promise(() => executeVoiceAction(action, {
            submitMessage: async text => {
              voiceMessageBusy = true
              const admitted = await Effect.runPromise(SubscriptionRef.get(state))
              const admittedSession = admitted.voice.sessionRef
              const admittedGeneration = admitted.voice.host.generation
              try {
                const muted = await voiceHost.command({ protocolVersion: 1, id: "voice.mute" })
                if (muted !== null) await Effect.runPromise(SubscriptionRef.update(state, current => ({ ...current, voice: withVoiceHostState(current.voice, muted) })))
                await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopNoteSubmitted", StaticPayload(text)), null)))
              } finally {
                const latest = await Effect.runPromise(SubscriptionRef.get(state))
                if (latest.voice.sessionRef === admittedSession && latest.voice.host.generation === admittedGeneration && latest.voice.host.phase === "muted") {
                  const resumed = await voiceHost.command({ protocolVersion: 1, id: "voice.unmute" })
                  if (resumed !== null) await Effect.runPromise(SubscriptionRef.update(state, current => ({ ...current, voice: withVoiceHostState(current.voice, resumed) })))
                }
                voiceMessageBusy = false
              }
            },
            interrupt: () => Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopTurnInterrupted"), null))),
            focusRegisteredCommand: commandId => {
              const workspace = commandId === "workspace.files" ? "files"
                : commandId === "workspace.review" ? "review" : "chat"
              return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopWorkspaceSelected", StaticPayload(workspace)), null)))
            },
            editFallback: text => Effect.runPromise(SubscriptionRef.update(state, current => withInput(current, text))),
          }))
        })).catch(error => {
          voiceMessageBusy = false
          console.error("[openagents-desktop:voice] lifecycle handling failed:", error instanceof Error ? error.message : "unknown_error")
        })
      })
      yield* Effect.addFinalizer(() => Effect.sync(unsubscribeVoice))
    }
    if (typeof bridge?.workspaceSubscribe === "function") {
      const unsubscribeWorkspace = bridge.workspaceSubscribe(change => {
        void Effect.runPromise(SubscriptionRef.get(state)).then(current => Effect.runPromise(
          Effect.all([
            ...(current.workspace === "files"
              ? [registry.dispatch(resolveIntentRef(IntentRef("WorkspaceBrowserChangeReceived", StaticPayload(change))))]
              : []),
            registry.dispatch(resolveIntentRef(IntentRef("WorkspaceEditorExternalChangeReceived", StaticPayload(change)))),
          ], { concurrency: 1, discard: true }),
        ))
      })
      yield* Effect.addFinalizer(() => Effect.sync(unsubscribeWorkspace))
    }
    // Workspace-bounded PTY terminals (CUT-20, #8700): stream every host event
    // into the terminal workspace state (schema-decoded in the handler), and
    // pull the snapshot once so a persisted tail is recovered after a restart.
    if (typeof bridge?.terminal?.onEvent === "function") {
      type PendingTerminalEvent = import("../terminal-contract.ts").TerminalEvent | Readonly<{
        kind: "batched_output"
        sessionRef: string
        chunks: string[]
        length: number
      }>
      let pendingTerminalEvents: PendingTerminalEvent[] = []
      let terminalFrame: ReturnType<typeof setTimeout> | null = null
      const flushTerminalEvents = () => {
        terminalFrame = null
        if (pendingTerminalEvents.length === 0) return
        const events: import("../terminal-contract.ts").TerminalEvent[] = pendingTerminalEvents.map(event =>
          event.kind === "batched_output"
            ? { kind: "output", sessionRef: event.sessionRef, chunk: event.chunks.join("").slice(-100_000) }
            : event)
        pendingTerminalEvents = []
        void Effect.runPromise(
          registry.dispatch(resolveIntentRef(IntentRef("TerminalEventsReceived", StaticPayload(events)))),
        )
      }
      const unsubscribeTerminal = bridge.terminal.onEvent((event) => {
        const last = pendingTerminalEvents.at(-1)
        if (event.kind === "output" && last?.kind === "batched_output" && last.sessionRef === event.sessionRef) {
          last.chunks.push(event.chunk)
          const length = last.length + event.chunk.length
          pendingTerminalEvents[pendingTerminalEvents.length - 1] = length <= 200_000
            ? { ...last, length }
            : { ...last, chunks: [last.chunks.join("").slice(-100_000)], length: 100_000 }
        } else if (event.kind === "output") {
          pendingTerminalEvents.push({ kind: "batched_output", sessionRef: event.sessionRef, chunks: [event.chunk], length: event.chunk.length })
        } else pendingTerminalEvents.push(event)
        if (pendingTerminalEvents.length >= 512) flushTerminalEvents()
        else if (terminalFrame === null) terminalFrame = setTimeout(flushTerminalEvents, 16)
      })
      yield* Effect.addFinalizer(() => Effect.sync(() => {
        unsubscribeTerminal()
        if (terminalFrame !== null) clearTimeout(terminalFrame)
        terminalFrame = null
        pendingTerminalEvents = []
      }))
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
    // Desktop-local Claude/Codex turns publish the same canonical graph as
    // confirmed Runtime Gateway chats, but over the local preload bridge.
    // Subscribe before taking the snapshot, retain the newest cursor per
    // thread, and project through the shared presentation model.
    const localGraphs = new Map<string, LiveAgentGraphUpdate>()
    const applyLocalGraph = (update: LiveAgentGraphUpdate): void => {
      const previous = localGraphs.get(update.threadRef)
      if (previous !== undefined && previous.graph.cursor > update.graph.cursor) return
      localGraphs.set(update.threadRef, update)
      void Effect.runPromise(SubscriptionRef.update(state, current =>
        withLiveAgentGraph(
          current,
          update.threadRef,
          projectLiveAgentGraphPresentation(update.graph, { maxRows: 200 }),
        )))
    }
    if (typeof bridge?.liveAgentGraph?.onUpdate === "function") {
      const unsubscribeGraph = bridge.liveAgentGraph.onUpdate(applyLocalGraph)
      window.addEventListener("pagehide", () => unsubscribeGraph(), { once: true })
    }
    // 2026-07-13 startup incident
    // (`openagents_desktop.startup.window_first_no_blank_frame.v1`):
    // everything inside hydrateAfterMount previously ran BEFORE
    // `renderer.mount`, holding the first shell paint hostage to the full
    // local coding-history scan (measured 5.3–6.5 s against a real ~/.codex).
    // The shell now mounts first — composer focusable, sidebar showing an
    // honest scanning state — and this hydration streams the history catalog,
    // coding sessions, update projection, threads, session view, and deferred
    // commands into the already-mounted state afterwards.
    const dispatchDeferredCommand = (command: DesktopDeferredCommand) => Effect.gen(function* () {
      const current = yield* SubscriptionRef.get(state)
      const resolution = resolveDesktopDeferredCommandIntent(command, {
        sessionReady: current.settings.openAgentsSession === "session_ready",
        verifiedOwner: current.settings.openAgentsSession === "session_ready",
        workspaceReady: current.workingDirectory !== null || current.workspaceBrowser.grantRef !== null || current.codingCatalog.sessions.length > 0 || documentLaunch,
      })
      if (resolution.state === "rejected") {
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
    const attachDesktopCommandBridge = Effect.gen(function* () {
      if (typeof bridge?.commands?.onCommand !== "function") return
      const unsubscribeCommands = bridge.commands.onCommand(command => {
        void Effect.runPromise(dispatchDeferredCommand(command))
      })
      window.addEventListener("pagehide", () => unsubscribeCommands(), { once: true })
      if (typeof bridge.commands.ready === "function") {
        yield* Effect.promise(bridge.commands.ready)
      }
    })
    const hydrateDocumentLaunchMetadata = (): void => {
      if (!documentLaunch) return
      if (fableLocalBridge !== null) {
        void fableLocalBridge.availability().catch(() => null).then(async raw => {
          fableAvailability = decodeFableLocalAvailability(raw)
          await Effect.runPromise(SubscriptionRef.update(state, current =>
            withHarnessLanes(current, localLanes())))
        })
      }
      void (bridge?.providerLanes?.capabilities?.().catch(() => null) ?? Promise.resolve(null))
        .then(async raw => {
          const capabilities = decodeProviderLaneComposerProjections(raw)
          if (capabilities !== null) {
            await Effect.runPromise(SubscriptionRef.update(state, current =>
              withProviderLaneCapabilities(current, capabilities)))
          }
        })
      if (typeof bridge?.runtimeRequest === "function") {
        void bridge.runtimeRequest({
          kind: "query",
          requestId: "renderer-voice-document-launch",
          query: { id: "voice.state" },
        }).then(async response => {
          if (response.kind === "voice_state") {
            await Effect.runPromise(SubscriptionRef.update(state, current => ({
              ...current,
              voice: withVoiceHostState(current.voice, response.state),
            })))
          }
        }).catch(() => undefined)
      }
    }
    const hydrateAfterMount = Effect.gen(function* () {
    // The shell is live while this hydration runs: a user selection made in
    // that window OWNS the workspace. Capture the at-mount workspace so the
    // persisted-focus restore below never stomps explicit navigation.
    const workspaceAtMount = (yield* SubscriptionRef.get(state)).workspace
    const workingDirectory = yield* Effect.promise(async () => {
      const value = await bridge?.workingDirectory?.().catch(() => null)
      return typeof value === "string" && value.length > 0 ? value : null
    })
    yield* SubscriptionRef.update(state, current => ({ ...current, workingDirectory }))
    if (typeof bridge?.liveAgentGraph?.snapshot === "function") {
      const snapshot = yield* Effect.promise(() => bridge.liveAgentGraph!.snapshot!().catch(() => null))
      for (const update of snapshot?.graphs ?? []) applyLocalGraph(update)
    }
    // Start independent metadata requests together, but never join them before
    // publishing the catalog. A slow local-thread adapter must not hold the
    // recent-session rail hostage.
    const historyCatalogPromise = historyHost.catalog()
    const localHistoryThreadsPromise = historyHost.localThreads()
    const historyCatalog = yield* Effect.promise(() => historyCatalogPromise)
    if (historyCatalog !== null) {
      const restored = restoreHistory()
      // Transcript 248 / metadata-first law: publish stable top-level rows as
      // soon as the catalog arrives. Startup never restores or opens a thread:
      // history is discovery metadata until the user explicitly selects it.
      yield* SubscriptionRef.update(state, current => ({
        ...current,
        history: {
          ...current.history,
          catalog: historyCatalog,
          page: null,
          selectedItemRef: null,
          railCollapsed: restored?.railCollapsed ?? false,
          expandedThreadRefs: restored?.expandedThreadRefs ?? [],
        },
      }))
      // Two animation frames guarantee the metadata commit gets a visible
      // paint before any selected-thread page request begins.
      yield* Effect.promise(() => new Promise<void>(resolve =>
        requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
      ))
    }
    const localHistoryThreads = yield* Effect.promise(() => localHistoryThreadsPromise)
    if (localHistoryThreads.length > 0) {
      yield* SubscriptionRef.update(state, current => ({ ...current, history: { ...current.history, localThreads: localHistoryThreads } }))
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
    const update = yield* Effect.promise(() => updateRendererHost.run("snapshot"))
    yield* SubscriptionRef.update(state, current => ({ ...current, update }))
    const existing = yield* Effect.promise(chat.listThreads)
    const threads = Array.isArray(existing) ? existing.filter((item): item is DesktopThread => typeof item === "object" && item !== null && typeof (item as { id?: unknown }).id === "string") : []
    if (threads.length > 0) {
      // Catalog arrival is discovery-only. Never turn the newest row into an
      // implicit selection while the owner is composing the startup draft.
      yield* SubscriptionRef.update(state, current => withThreadCatalog(current, threads))
    }
    // Snapshot/update delivery can precede the initial local-thread catalog;
    // replay retained newest post-images once those thread rows exist.
    for (const update of localGraphs.values()) applyLocalGraph(update)
    // Full Auto mount hydration (FA-H1 #8874): now that the initial thread
    // catalog is loaded, seed the ACTIVE thread's composer toggle from main's
    // durable registry truth instead of the hard-coded off default — after a
    // restart, a thread main is still auto-continuing must show ON so one
    // click stops it. A user toggle that raced this fetch already wrote a map
    // entry (and persisted itself via fullAutoHost.set), so an existing entry
    // always wins here; threads selected later hydrate through the shell's
    // selection path.
    {
      const mounted = yield* SubscriptionRef.get(state)
      const activeThreadRef = mounted.activeThreadId
      if (activeThreadRef !== null) {
        const durable = yield* Effect.promise(() => fullAutoHost.get({ threadRef: activeThreadRef }))
        yield* SubscriptionRef.update(state, current =>
          current.fullAutoByThread[activeThreadRef] === undefined
            ? { ...current, fullAutoByThread: { ...current.fullAutoByThread, [activeThreadRef]: durable.enabled } }
            : current)
      }
    }
    const sessionView = decodeOpenAgentsSessionView(
      yield* Effect.promise(openAgentsSessionSettingsBridge.status),
    )
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      settings: { ...current.settings, openAgentsSession: sessionView },
    }))
    if (!documentLaunch && restoredWorkspace !== null &&
      (yield* SubscriptionRef.get(state)).workspace === workspaceAtMount) {
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
    // A Files selection that landed BEFORE the coding catalog hydrated could
    // not resolve its workspace session ref, so editor recovery was skipped.
    // Re-dispatch the same typed selection now that the catalog is live —
    // idempotent (recovery returns early once tabs exist) and it is the same
    // intent path a user press takes, never a parallel route.
    if ((yield* SubscriptionRef.get(state)).workspace === "files") {
      yield* registry.dispatch(resolveIntentRef(IntentRef("DesktopWorkspaceSelected", StaticPayload("files")), null))
    }
    // History hydration settled (found or honestly absent): the sidebar's
    // scanning row yields to real rows or the true empty state.
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      history: { ...current.history, hydrated: true },
    }))
    // Startup-timing: first instant the mounted shell holds hydrated
    // thread-list content (read back by the startup marks/trace driver).
    {
      const marks = ((globalThis as { __oaStartupMarks?: Record<string, number> }).__oaStartupMarks ??= {})
      marks.historyHydrated = Date.now()
    }
    })
    // Focus must land AFTER the chat view mounts. A New-chat dispatch can
    // clear a loaded history page, which swaps the whole center view and
    // (re)mounts the composer on a LATER render commit than the intent
    // completion — and a re-parented input loses focus even when it was
    // focused earlier. So retry across commits until the input exists AND
    // holds focus (owner contract: "when i do new chat, clicking button
    // or command N, auto focus the input"). #8787 moved the retry logic into
    // composer-focus.ts so open-time focus shares one tested implementation.
    const focusComposer = makeComposerFocuser({ root })
    // #8787: the guarded settle pass claims UNOWNED focus for the composer
    // (post-hydration, macOS re-activate) and never steals owned focus.
    const settleComposerFocus = makeComposerFocusSettler({ root })
    // macOS re-activate with an existing window (#8787): when the window
    // regains OS focus and nothing owns keyboard focus, the composer takes it
    // so the first keystroke lands without a click.
    window.addEventListener("focus", () => settleComposerFocus())
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
            if (shouldAnchorHistoryEnd) void anchorHistoryEnd(current.history.page?.selectedThreadRef)
            if (shouldAnchorSearchOpen) { const itemRef = current.history.selectedItemRef; void (itemRef === null ? anchorHistoryEnd(current.history.page?.selectedThreadRef) : anchorHistoryItem(itemRef)) }
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
    const onFilesModeShortcut = (event: KeyboardEvent): void => {
      const current = Effect.runSync(SubscriptionRef.get(state))
      const target = event.target
      const editable = target instanceof HTMLElement &&
        target.closest("input, textarea, [contenteditable='true']") !== null
      const matched = desktopCommandShortcutMatches(
        current.commandBindings,
        "workspace.files",
        bridge?.platform ?? "unknown",
        event,
        editable,
      )
      if (!matched) return
      const workspaceReady = current.workingDirectory !== null || current.workspaceBrowser.grantRef !== null || current.codingCatalog.sessions.length > 0
      if (!workspaceReady) return
      event.preventDefault()
      event.stopPropagation()
      void Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopFilesModeToggled", StaticPayload(null)))),
      ).catch(() => undefined)
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
    const historyScrollEl=(threadRef?:string):HTMLElement|null=>threadRef===undefined
      ? root.querySelector<HTMLElement>('[data-en-key^="history-timeline-page-"]')
      : [...root.querySelectorAll<HTMLElement>('[data-en-key^="history-timeline-page-"]')].find(element=>element.getAttribute("data-en-key")===`history-timeline-page-${threadRef}`)??null
    const anchorHistoryEnd=async(threadRef?:string):Promise<void>=>{
      for(let attempt=0;attempt<8;attempt++){
        await settleFrame()
        const el=historyScrollEl(threadRef)
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
      if(!(el instanceof HTMLElement)||!el.getAttribute("data-en-key")?.startsWith("history-timeline-page-")||historyEdgeFetchInFlight)return
      const metrics={scrollTop:el.scrollTop,clientHeight:el.clientHeight,scrollHeight:el.scrollHeight}
      historyEdgeFetchInFlight=true
      void Effect.runPromise(SubscriptionRef.get(state)).then(async current=>{
        const page=current.history.page
        if(current.workspace!=="chat"||page===null)return
        if(historyShouldFetchOlder({scrollTop:metrics.scrollTop,clientHeight:metrics.clientHeight,offset:page.offset,loadingEdge:current.history.loadingEdge})){
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryOlderRequested",StaticPayload(null)))))
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
          const targets=desktopConversationShortcutTargets(current)
          if(current.workspace!=="chat"||targets.length===0){historyShortcutSteps=0;historyShortcutAbsoluteIndex=null;break}
          const activeRef=current.history.pendingThreadRef??current.history.page?.rootThreadRef??current.activeThreadId
          const activeIndex=targets.findIndex(item=>item.threadRef===activeRef)
          const steps=historyShortcutSteps
          const absoluteIndex=historyShortcutAbsoluteIndex
          historyShortcutSteps=0
          historyShortcutAbsoluteIndex=null
          const baseIndex=activeIndex<0?(steps>0?-1:1):activeIndex
          const targetIndex=Math.max(0,Math.min(targets.length-1,absoluteIndex??baseIndex+steps))
          if(targetIndex===activeIndex)continue
          const target=targets[targetIndex]!
          const targetRef=target.threadRef
          if(target.kind==="runtime"){
            if(historySelectionTimer!==null){window.clearTimeout(historySelectionTimer);historySelectionTimer=null}
            await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopChatSelected",StaticPayload(targetRef)))))
            await scrollHistorySelectionIntoView(targetIndex,targetRef)
            continue
          }
          const catalogIndex=current.history.catalog.roots.findIndex(root=>root.threadRef===targetRef)
          let visible=current.history.visibleRootCount
          while(catalogIndex>=visible){
            await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryCatalogMoreRequested",StaticPayload(null)))))
            visible+=historyCatalogPageSize
          }
          await Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryConversationPreviewed",StaticPayload(targetRef)))))
          await scrollHistorySelectionIntoView(targetIndex,targetRef)
          if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
          historySelectionTimer=window.setTimeout(()=>{
            historySelectionTimer=null
            void Effect.runPromise(SubscriptionRef.get(state)).then(current=>{
              if(current.history.pendingThreadRef!==targetRef)return
              return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryConversationSelected",StaticPayload(targetRef)))))
            }).then(()=>{void anchorHistoryEnd(targetRef);return scrollHistorySelectionIntoView(targetIndex,targetRef)})
          },110)
        }
      }finally{
        historyShortcutRunning=false
        if(historyShortcutSteps!==0||historyShortcutAbsoluteIndex!==null)void pumpHistoryConversationShortcut()
      }
    }
    let historyShortcutHintTimer:number|null=null
    let historyShortcutHintsVisible=false
    const setHistoryShortcutHints=(visible:boolean):void=>{
      if(historyShortcutHintsVisible===visible)return
      historyShortcutHintsVisible=visible
      void Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("DesktopHistoryShortcutHintsChanged",StaticPayload(visible)))))
    }
    const onHistoryConversationShortcut = (event: KeyboardEvent): void => {
      const target=event.target
      const editable=target instanceof HTMLElement&&target.closest("input, textarea, [contenteditable='true']")!==null
      const action=historyConversationShortcutAction(event,bridge?.platform,editable)
      if(action===null)return
      event.preventDefault()
      if(action.kind==="absolute"){historyShortcutSteps=0;historyShortcutAbsoluteIndex=action.index}
      else {historyShortcutAbsoluteIndex=null;historyShortcutSteps+=action.delta}
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
        return Effect.runPromise(registry.dispatch(resolveIntentRef(IntentRef("HistoryAgentSelected",StaticPayload(targetRef))))).then(()=>anchorHistoryEnd(targetRef))
      })
    }
    const onHistoryModifierDown=(event:KeyboardEvent):void=>{
      const modifierKey=bridge?.platform==="darwin"?(event.key==="Meta"||event.key==="OS"):event.key==="Control"
      if(!modifierKey||historyShortcutHintTimer!==null||historyShortcutHintsVisible)return
      historyShortcutHintTimer=window.setTimeout(()=>{
        historyShortcutHintTimer=null
        setHistoryShortcutHints(true)
      },100)
    }
    const onHistoryModifierUp=(event:KeyboardEvent):void=>{
      if((bridge?.platform==="darwin"&&(event.key==="Meta"||event.key==="OS"))||(bridge?.platform!=="darwin"&&event.key==="Control")){
        if(historyShortcutHintTimer!==null){window.clearTimeout(historyShortcutHintTimer);historyShortcutHintTimer=null}
        setHistoryShortcutHints(false)
      }
    }
    const onHistoryWindowBlur=():void=>{
      if(historyShortcutHintTimer!==null){window.clearTimeout(historyShortcutHintTimer);historyShortcutHintTimer=null}
      setHistoryShortcutHints(false)
    }
    const removeComposerImageAcquisition = installComposerImageAcquisition(window, {
      readSnapshot: () => Effect.runPromise(SubscriptionRef.get(state)).then(snapshot => ({
        pending: snapshot.pending,
        imageCount: snapshot.composerImages.length,
      })),
      add: attachment => Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopComposerImageAdded", StaticPayload(attachment)))),
      ),
      reject: message => Effect.runPromise(
        registry.dispatch(resolveIntentRef(IntentRef("DesktopComposerImagesRejected", StaticPayload(message)))),
      ),
    })
    window.addEventListener("keydown", onNewChatShortcut)
    window.addEventListener("keydown", onFullscreenShortcut)
    window.addEventListener("keydown", onFilesModeShortcut, true)
    window.addEventListener("keydown", onCommandPaletteShortcut)
    window.addEventListener("keydown", onHistoryModifierDown, true)
    window.addEventListener("keydown", onHistoryConversationShortcut)
    window.addEventListener("keydown", onHistoryAgentShortcut)
    window.addEventListener("keyup", onHistoryModifierUp, true)
    window.addEventListener("blur", onHistoryWindowBlur)
    // Scroll events do not bubble; capture phase observes the history region.
    window.addEventListener("scroll", onHistoryTimelineScroll, true)
    window.addEventListener("pagehide", () => {
      removeComposerImageAcquisition()
      window.removeEventListener("keydown", onNewChatShortcut)
      window.removeEventListener("keydown", onFullscreenShortcut)
      window.removeEventListener("keydown", onFilesModeShortcut, true)
      window.removeEventListener("keydown", onCommandPaletteShortcut)
      window.removeEventListener("keydown", onHistoryModifierDown, true)
      window.removeEventListener("keydown", onHistoryConversationShortcut)
      window.removeEventListener("keydown", onHistoryAgentShortcut)
      window.removeEventListener("keyup", onHistoryModifierUp, true)
      window.removeEventListener("blur", onHistoryWindowBlur)
      window.removeEventListener("scroll", onHistoryTimelineScroll, true)
      if(historySelectionTimer!==null)window.clearTimeout(historySelectionTimer)
      if(historyShortcutHintTimer!==null)window.clearTimeout(historyShortcutHintTimer)
    }, { once: true })
    // Durable preferences → real presentation (CUT-24 #8704): density + font
    // scale the shared theme through the token pipeline; reduced-motion resolves
    // to a root data attribute the app CSS honors. Defaults are identity, so the
    // common path (and the no-preload smoke path) is unchanged.
    const [preferences, commandBindings] = yield* Effect.promise(() => Promise.all([
      loadDesktopPreferences(),
      commandBindingHost.snapshot().catch(() => null),
    ]))
    yield* SubscriptionRef.update(state, current => ({
      ...current,
      commandBindings,
      presentation: {
        ...current.presentation,
        // Owner directive 2026-07-20: the sidebar is HIDDEN ON OPEN, always. The
        // initial state already starts collapsed; do NOT restore a persisted
        // expanded rail at launch. The expander stays reachable and the user can
        // open it during the session.
        sidebarCollapsed: true,
        // Disclosure is intentionally launch-ephemeral. The authoritative
        // query remains in history state and is never duplicated in prefs.
        sessionSearchOpen: false,
      },
      settings: {
        ...current.settings,
        localCodexUsageControlAvailable:
          preferences.privacy.localCodexUsageControlAvailable,
        shareLocalCodexUsage: preferences.privacy.shareLocalCodexUsage,
      },
      workspaceEditor: {
        ...current.workspaceEditor,
        vimEnabled: preferences.editor.vim.enabled,
      },
    }))
    for (const [name, value] of Object.entries(preferencesRootAttributes(preferences))) {
      document.documentElement.setAttribute(name, value)
    }
    const theme = themeForPreferences(preferences)
    const compatibilityRequested = new URLSearchParams(window.location.search).get("renderer") === "compatibility"
    document.documentElement.dataset.desktopRenderer = compatibilityRequested ? "compatibility" : "react"
    if (compatibilityRequested) {
      const renderer = makeReactDomRenderer({
        backend: "compatibility",
        theme,
        hostDrivers: [makeStubCodeEditorDriver()],
      })
      yield* renderer.mount(root, program.viewStream, report)
    } else {
      // MVP-02F: ordinary Desktop launches install the React Codex workbench.
      // It still consumes the one Effect-owned snapshot stream and typed intent
      // registry; the compatibility catalog is an explicit exclusive fallback.
      yield* mountReactWorkbench(root, SubscriptionRef.changes(state), report, { theme })
    }
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
    // The static branded boot frame (index.html) has done its job the moment
    // the real shell is mounted underneath it.
    document.getElementById("openagents-boot-frame")?.remove()
    // Finder-open commands are drained before any history/catalog hydration.
    // The launch context already made Files the first shell; this completes
    // the real workspace/editor transition without ever painting chat.
    yield* attachDesktopCommandBridge
    hydrateDocumentLaunchMetadata()
    // One boot-time diagnostics gather so the watchdog panel has live health the
    // first time Settings is opened (CUT-24 #8704). Event-driven, not a poll.
    void Effect.runPromise(
      registry.dispatch(resolveIntentRef(IntentRef("DesktopDiagnosticsRefreshRequested", StaticPayload(null)))),
    )
    // T3-style provider advisory, deliberately scoped to Codex. It runs only
    // after the interactive shell mounts and never delays composer focus.
    void Effect.runPromise(
      registry.dispatch(resolveIntentRef(IntentRef("DesktopHarnessMaintenanceRefreshRequested", StaticPayload(null)))),
    )
    // #8787 (owner verbatim: "the text input should be focused immediately on
    // open. so i can start typing right away."): keyboard focus lands in the
    // composer at SHELL-INTERACTABLE — right here, before background history
    // hydration — so the first keystroke goes into the message input with
    // zero clicks.
    focusComposer()
    // 2026-07-13 startup incident: the shell above is already mounted and
    // interactable; the coding-history/catalog/threads hydration streams in
    // afterwards (see hydrateAfterMount).
    yield* hydrateAfterMount
    // #8787: late hydration can re-parent the center view and drop focus on
    // the floor. Re-claim it for the composer ONLY when focus is unowned —
    // a user selection made while hydration streamed keeps focus.
    settleComposerFocus()
  })

const boot = (): void => {
  // Startup-timing: earliest renderer-script instant (ms epoch). See the
  // shellMounted mark above and scripts/startup-bench.ts.
  ;((globalThis as { __oaStartupMarks?: Record<string, number> }).__oaStartupMarks ??= {}).bootStart = Date.now()
  const root = document.getElementById("openagents-desktop-root")
  if (root === null) return
  // QA-3 (#8908): visual-baseline probe mode — mount one frozen fixture shell
  // state instead of the live shell. Lazy import keeps the module (and its
  // Date-freezing shim) entirely off the production startup path.
  const visualBaseline = new URLSearchParams(window.location.search).get("visualBaseline")
  if (visualBaseline !== null) {
    void import("./visual-baseline.ts").then(module =>
      module.mountVisualBaseline(root, visualBaseline)
    ).catch(error => {
      document.documentElement.dataset.visualBaselineError = error instanceof Error ? error.message : "mount failed"
    })
    return
  }
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
