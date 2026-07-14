/**
 * OpenAgents Desktop — Electron main process (#8574).
 *
 * Scaffolded from the pinned MIT-licensed LuanRoger/electron-shadcn template
 * (see UPSTREAM.md) and hardened per the issue's mandatory first-commit
 * boundary: contextIsolation on, nodeIntegration OFF, sandbox ON, no webview,
 * deny-by-default permissions/navigation/window-open, and a minimal
 * contextBridge preload (no ipcRenderer, no MessagePort/oRPC bridge, no
 * updater, no devtools installer).
 *
 * Plain TypeScript, bundled by `scripts/build.ts` (Bun) into `dist/`.
 */
import path from "node:path"
import { homedir } from "node:os"
import { createHash, randomUUID } from "node:crypto"
import { cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs"
import { execFile, execFileSync } from "node:child_process"
import { BrowserWindow, Menu, app, dialog, ipcMain, protocol, shell, systemPreferences, utilityProcess, type IpcMainInvokeEvent, type MenuItemConstructorOptions, type Session } from "electron"
import { Effect } from "effect"
import {
  fetchFleetRunClientProjection,
  buildCloseTurnIntent,
  buildContinueTurnIntent,
  buildInterruptTurnIntent,
  buildRetryTurnIntent,
  buildStartTurnIntent,
} from "@openagentsinc/khala-sync-client"
import { FleetRunProjectionListChannel } from "./fleet-run-projection-contract.ts"
import { openDesktopUpdateStagingHost } from "./update-staging-host.ts"
import { openMacOSUpdateApplier } from "./macos-update-applier.ts"
import {
  DesktopUpdateStagingChannel,
  decodeDesktopUpdateStagingAction,
} from "./update-staging-contract.ts"

// macOS derives Electron safeStorage's Keychain service from the application
// name. Keep production stable while isolating unsigned development and smoke
// launches so they can never contest the signed app's Keychain ACL.
const desktopApplicationName = app.isPackaged ? "OpenAgents" : "OpenAgents Dev"
app.setName(desktopApplicationName)
process.title = desktopApplicationName

protocol.registerSchemesAsPrivileged([{
  scheme: DesktopRendererScheme,
  privileges: {
    standard: true,
    secure: true,
    codeCache: true,
    supportFetchAPI: true,
  },
}])

import {
  CodexAccountsChannel,
  CodexConnectOpenChannel,
  CodexConnectStartChannel,
  CodexConnectStatusChannel,
  CodexReconnectStartChannel,
} from "./codex-connect-contract.ts"
import { makeCodexConnectService, makeFixtureSpawnPylon } from "./codex-connect.ts"
import {
  ProviderAccountsListChannel,
  ProviderAccountsUsageChannel,
  decodeProviderAccountUsageRequest,
  unavailableProviderAccountUsageResult,
} from "./provider-accounts-contract.ts"
import { makeFixtureProviderAccountsSpawn, makeProviderAccountsService } from "./provider-accounts.ts"
import { FleetStageChannel, decodeFleetStageRequest, unavailableFleetStageResult } from "./fleet-contract.ts"
import { submitFleetBrief } from "./fleet-control.ts"
import { completeChatTurn } from "./chat-service.ts"
import {
  DesktopChatTurnChannel,
  DesktopLocalTurnRecoveryUpdateChannel,
  DesktopForkHistoryThreadChannel,
  DesktopForkHistoryThreadRequestSchema,
  DesktopHydrateThreadChannel,
  DesktopLocalThreadsChannel,
  DesktopNewThreadChannel,
  DesktopOpenThreadChannel,
  DesktopResumeLocalThreadChannel,
  DesktopResumeLocalThreadRequestSchema,
  DesktopThreadsChannel,
  decode,
  DesktopThreadRequestSchema,
  DesktopTurnRequestSchema,
  type DesktopForkHistoryThreadRequest,
  type DesktopMessage,
  type DesktopResumeLocalThreadRequest,
} from "./chat-contract.ts"
import { historyForkFetchPlan, historyForkSeed } from "./history-thread-actions.ts"
import {
  isolatedAppProofChromiumSwitches,
  isolatedAppProofWorkspaceRoot,
  isolatedProofReceiptPath,
  isIsolatedAppProof,
} from "./isolated-app-proof.ts"
import {
  DesktopRendererScheme,
  desktopRendererEntryUrl,
  isTrustedDesktopRendererUrl,
} from "./desktop-renderer-location.ts"
import { desktopWorkerUrl } from "./desktop-worker-location.ts"
import { desktopRuntimeWorkspaceRoot } from "./desktop-runtime-workspace.ts"
import {
  FABLE_LOCAL_FINAL_TEXT_LIMIT,
  FableLocalAnswerQuestionChannel,
  FableLocalAvailabilityChannel,
  FableLocalEventChannel,
  FableLocalInterruptChannel,
  FABLE_LOCAL_IMAGE_BYTES_LIMIT,
  FABLE_LOCAL_IMAGE_COUNT_LIMIT,
  FABLE_LOCAL_IMAGE_MEDIA_TYPES,
  FableLocalPickImagesChannel,
  FableLocalQueueFollowupChannel,
  FableLocalStartChannel,
  FableLocalSteerChildChannel,
  decodeFableLocalAnswerQuestionRequest,
  decodeFableLocalInterruptRequest,
  decodeFableLocalQueueFollowupRequest,
  decodeFableLocalStartRequest,
  decodeFableLocalSteerChildRequest,
  fableLocalFailureMessage,
  fableLocalModelNoteText,
  fableLocalTraceNoteMeta,
  fableLocalTraceNoteText,
  isClaudeModel,
  isCodexModel,
  startRequestHasContent,
} from "./fable-local-contract.ts"
import {
  McpConfigAddChannel,
  McpConfigListChannel,
  McpConfigRemoveChannel,
  McpConfigToggleChannel,
  decodeMcpConfigAddRequest,
  decodeMcpConfigNameRequest,
  decodeMcpConfigToggleRequest,
} from "./mcp-config-contract.ts"
import { openMcpConfigStore } from "./mcp-config-host.ts"
import {
  PluginConfigChooseChannel,
  PluginConfigListChannel,
  PluginConfigRemoveChannel,
  PluginConfigToggleChannel,
  decodePluginRefRequest,
  decodePluginToggleRequest,
} from "./plugin-config-contract.ts"
import { openPluginConfigStore } from "./plugin-config-host.ts"
import {
  DiagnosticsActionChannel,
  DiagnosticsExportChannel,
  DiagnosticsGatherChannel,
  decodeDiagnosticsAction,
} from "./diagnostics-contract.ts"
import { makeDiagnosticsHost } from "./diagnostics-host.ts"
import type { DiagnosticsInputs } from "./diagnostics-report.ts"
import {
  DesktopPreferencesGetChannel,
  DesktopPreferencesResetChannel,
  DesktopPreferencesUpdateChannel,
  decodeDesktopPreferencesPatch,
} from "./desktop-preferences-contract.ts"
import { openDesktopPreferencesStore } from "./desktop-preferences-host.ts"
import {
  FABLE_LOCAL_FIXTURE_ACCOUNT,
  FABLE_LOCAL_MODEL,
  makeFableLocalRuntime,
  makeFixtureFableLocalQuery,
  makeFixtureFableMcpFactory,
} from "./fable-local-runtime.ts"
import {
  fixtureCodexRevokedStderr,
  fixtureCodexRevokedStdout,
  fixtureCodexSuccessStdout,
  discoverRegisteredCodexAccounts,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
} from "./codex-child-runtime.ts"
import { CODEX_CHILD_MODEL } from "./codex-child-contract.ts"
import {
  CodexLocalAvailabilityChannel,
  CodexLocalEventChannel,
  CodexLocalInterruptChannel,
  CodexLocalQueueFollowupChannel,
  CodexLocalStartChannel,
  CodexLocalSteerTurnChannel,
  codexLocalFailureMessage,
  codexLocalModelNoteText,
  codexLocalRequestedModelLabel,
  CODEX_LOCAL_MODEL,
  CODEX_LOCAL_RUNTIME_COMPATIBILITY_REF,
} from "./codex-local-contract.ts"
import {
  FIXTURE_CODEX_LOCAL_ACCOUNT,
  FIXTURE_CODEX_LOCAL_TEXT,
  fixtureCodexLocalTurnStdout,
  makeCodexLocalRuntime,
} from "./codex-local-runtime.ts"
import { makeCodexPreflight, type CodexProbeResult } from "./codex-preflight.ts"
import { resolveBundledCodexExecutable } from "./provider-runtime-host.ts"
import { installBuiltinProductSpecWorkSkill, verifyBuiltinProductSpecWorkSkill } from "./builtin-productspec-skill.ts"
import {
  LiveAgentGraphSnapshotChannel,
  LiveAgentGraphUpdateChannel,
  type LiveAgentGraphUpdateWire,
} from "./live-agent-graph-contract.ts"
import { makeLiveAgentGraphHost } from "./live-agent-graph-host.ts"
import {
  UsageLedgerEventChannel,
  UsageLedgerSnapshotChannel,
} from "./usage-ledger-contract.ts"
import { makeUsageLedger } from "./usage-ledger.ts"
import { makeThreadStore } from "./thread-store.ts"
import { localRuntimePersistenceOperation } from "./local-runtime-event-persistence.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import { reconcileLocalTurns } from "./local-turn-recovery.ts"
import { makeLocalTurnTextPersistence } from "./local-turn-text-persistence.ts"
import {
  DesktopCodingCatalogArchiveChannel,
  DesktopCodingCatalogChooseChannel,
  DesktopCodingCatalogDeleteChannel,
  DesktopCodingCatalogFocusChannel,
  DesktopCodingCatalogOpenChannel,
  DesktopCodingCatalogRecoverChannel,
  DesktopCodingCatalogSnapshotChannel,
  decodeDesktopCodingFocusRequest,
  decodeDesktopCodingCatalogPageRequest,
  decodeDesktopCodingSessionRequest,
  emptyDesktopCodingCatalogProjection,
  projectDesktopCodingCatalog,
} from "./coding-catalog-contract.ts"
import { makeCodexHistoryHost } from "./codex-history-host.ts"
import { makeCodexHistoryUtilityFactory } from "./codex-history-utility.ts"
import { makeDesktopHostLifecycle } from "./desktop-host-lifecycle.ts"
import { createDesktopVoiceHost, type VoiceNativeMedia } from "./voice-host.ts"
import { createPackagedVoiceNativeMedia } from "./voice-native-helper.ts"
import {
  DesktopWorkspaceChooseChannel,
  DesktopWorkspaceFilesChannel,
  DesktopWorkspaceGitDiffChannel,
  DesktopWorkspaceGitStatusChannel,
  DesktopWorkspaceReadChannel,
  DesktopWorkspaceSaveChannel,
  DesktopWorkspaceSummaryChannel,
  DesktopWorkspaceTreeChannel,
  DesktopWorkspaceSearchChannel,
  DesktopWorkspaceSearchCancelChannel,
  DesktopWorkspaceCreateChannel,
  DesktopWorkspaceRenameChannel,
  DesktopWorkspaceDeleteChannel,
  DesktopWorkspaceRevealChannel,
  DesktopWorkspaceDocumentOpenChannel,
  DesktopWorkspaceDocumentSaveChannel,
  DesktopWorkspaceDocumentSaveAsChannel,
  DesktopWorkspaceRefreshChannel,
  DesktopWorkspaceWatchChannel,
  DesktopWorkspaceChangeChannel,
  decodeWorkspaceFileRequest,
  decodeWorkspaceGitDiffRequest,
  decodeWorkspaceSaveRequest,
  decodeWorkspaceSearchBridgeRequest,
  decodeWorkspaceSearchCancelRequest,
  decodeWorkspaceCreateRequest,
  decodeWorkspaceRenameRequest,
  decodeWorkspaceDeleteRequest,
  decodeWorkspaceRevealRequest,
  decodeWorkspaceDocumentRequest,
  decodeWorkspaceDocumentSaveRequest,
  decodeWorkspaceDocumentSaveAsRequest,
  decodeWorkspaceTreeRequest,
  decodeWorkspaceWatchRequest,
} from "./workspace-contract.ts"
import { DesktopWindowFullscreenChannel } from "./window-contract.ts"
import { openWorkspaceService } from "./workspace-service.ts"
import { openAdmittedDesktopWorkspace } from "./desktop-workspace-admission.ts"
import {
  ProductSpecCreateChannel,
  ProductSpecEditConfirmChannel,
  ProductSpecEditProposeChannel,
  ProductSpecEvidenceAttachmentConfirmChannel,
  ProductSpecEvidenceAttachmentProposeChannel,
  ProductSpecEvidenceRecordChannel,
  ProductSpecEvidenceVerifyChannel,
  ProductSpecOpenChannel,
  ProductSpecOwnerDispositionChannel,
  ProductSpecPacketAdmitChannel,
  ProductSpecPacketBlockChannel,
  ProductSpecPacketDispositionChannel,
  ProductSpecPlanAcceptChannel,
  ProductSpecPlanProposeChannel,
  ProductSpecRunGetChannel,
  ProductSpecRunDispositionChannel,
  decodeProductSpecCreateRequest,
  decodeProductSpecEditConfirmRequest,
  decodeProductSpecEditProposalRequest,
  decodeProductSpecEvidenceAttachmentConfirmRequest,
  decodeProductSpecEvidenceAttachmentProposalRequest,
  decodeProductSpecEvidenceRequest,
  decodeProductSpecOpenRequest,
  decodeProductSpecOwnerDispositionRequest,
  decodeProductSpecPacketAdmitRequest,
  decodeProductSpecPacketBlockRequest,
  decodeProductSpecPacketDispositionRequest,
  decodeProductSpecPlanAcceptRequest,
  decodeProductSpecPlanProposalRequest,
  decodeProductSpecRunGetRequest,
  decodeProductSpecRunDispositionRequest,
  decodeProductSpecVerificationRequest,
  type ProductSpecOperationError,
} from "./product-spec-workroom-contract.ts"
import { makeProductSpecWorkroom } from "./product-spec-workroom.ts"
import { ProductSpecDynamicTools, handleProductSpecDynamicTool } from "./product-spec-app-server-tools.ts"
import {
  CodexHandoffOpenChannel,
  decodeCodexHandoffOpenRequest,
  openCodexHandoffLedger,
} from "./codex-handoff-contract.ts"
import { makeCodexHandoffHost, openCodexHandoffBindings } from "./codex-handoff-host.ts"
import { GitGithubChannel } from "./git-github-contract.ts"
import { openGitGithubService } from "./git-github-host.ts"
import { openTurnCheckpointService } from "./turn-checkpoint-host.ts"
import { workspaceGitEnvironment } from "./git-process-environment.ts"
import {
  TerminalCloseChannel,
  TerminalCreateChannel,
  TerminalEventChannel,
  TerminalInputChannel,
  TerminalInterruptChannel,
  TerminalPreviewOpenChannel,
  TerminalResizeChannel,
  TerminalRestartChannel,
  TerminalSnapshotChannel,
  decodeTerminalCreateRequest,
  decodeTerminalInputRequest,
  decodeTerminalPreviewOpenRequest,
  decodeTerminalResizeRequest,
  decodeTerminalSessionRequest,
  type TerminalEvent,
} from "./terminal-contract.ts"
import { makeTerminalHost } from "./terminal-host.ts"
import { makeWorkspaceSearchRegistry } from "./workspace-search-registry.ts"
import {
  DesktopRuntimeGatewayEventChannel,
  DesktopRuntimeGatewayInvokeChannel,
  DesktopRuntimeGatewayProtocolVersion,
  decodeDesktopRuntimeGatewayRequest,
  invalidDesktopRuntimeGatewayResponse,
  type DesktopRuntimeGatewayRequest,
} from "./runtime-gateway-contract.ts"
import { createDesktopRuntimeGateway } from "./runtime-gateway.ts"
import { desktopRuntimeCapabilities } from "./runtime-gateway.ts"
import {
  collectHarnessMaintenanceStatus,
  persistHarnessMaintenanceReceipt,
  runHarnessMaintenanceUpdate,
} from "@openagentsinc/pylon-core/custody/harness-maintenance"
import { resolvePylonHome } from "@openagentsinc/pylon-core/shared/bootstrap"
import { createDesktopRuntimeLiveSubscriptions } from "./runtime-live-subscriptions.ts"
import {
  desktopOperationRef,
  decodeDesktopOperationContext,
  makeDesktopCorrelationJournal,
  type DesktopOperationContext,
} from "./desktop-operation-context.ts"
import { openDesktopSyncHost } from "./desktop-sync-host.ts"
import {
  openDesktopSessionVault,
  type DesktopSessionVault,
} from "./desktop-session-vault.ts"
import { recoverVerifiedDesktopSession } from "./desktop-session-recovery.ts"
import { traceAcceptanceJourney, traceAcceptanceReload } from "./electron-trace-acceptance.ts"
import { resolveLiveProofConfig, runLiveProof } from "./live-proof.ts"
import { mvpProofEnvironmentFromArgv, resolveMvpProofConfig, runMvpProof } from "./mvp-proof.ts"
import {
  signInDesktopSession,
  signOutDesktopSession,
} from "./desktop-session-pkce.ts"
import {
  DesktopCommandEventChannel,
  DesktopCommandReadyChannel,
  DesktopCommandBindingsChannel,
  DesktopCommandBindingSaveChannel,
  DesktopCommandBindingsResetChannel,
  decodeDesktopCommandBindingUpdateOrNull,
  desktopCanonicalCommandRegistry,
  type DesktopCommandBindingProjection,
} from "./desktop-command-contract.ts"
import {
  deferredDesktopCommand,
  desktopCommandsFromArgv,
  makeDesktopCommandHost,
  parseDesktopCommandUrl,
} from "./desktop-command-host.ts"
import {
  commandBindingForNativeMenu,
  openDesktopCommandBindingStore,
  type DesktopCommandBindingStore,
} from "./desktop-command-bindings.ts"

const here = import.meta.dirname
const builtinSkillsRoot = app.isPackaged
  ? path.join(process.resourcesPath, "builtin-skills")
  : path.join(here, "builtin-skills")
const rendererRoot = app.isPackaged
  ? path.join(process.resourcesPath, "app.asar.unpacked", "dist", "renderer")
  : path.join(here, "renderer")
const rendererAssetContentTypes = new Map([
  ["index.html", "text/html; charset=utf-8"],
  ["boot.js", "text/javascript; charset=utf-8"],
  ["app.css", "text/css; charset=utf-8"],
] as const)
const installDesktopRendererProtocol = (): void => {
  protocol.handle(DesktopRendererScheme, request => {
    const url = new URL(request.url)
    const asset = url.hostname === "renderer" ? url.pathname.replace(/^\/+/, "") : ""
    const contentType = rendererAssetContentTypes.get(asset as "index.html" | "boot.js" | "app.css")
    if (contentType === undefined) return new Response("Not found", { status: 404 })
    return new Response(readFileSync(path.join(rendererRoot, asset)), {
      status: 200,
      headers: { "content-type": contentType, "cache-control": "no-store" },
    })
  })
}
const smokeFixtureSourceRoot = app.isPackaged
  ? path.join(process.resourcesPath, "app.asar.unpacked", "dist", "renderer", "smoke-fixtures")
  : path.join(here, "..", "tests", "fixtures")
// Startup-timing harness (measure-constantly discipline; scripts/startup-bench.ts).
// When set to a file path, the app runs the deterministic fixture startup path
// (implies smoke wiring — no network, no real ~/.codex scan), records the
// milestone marks, writes them as JSON to that path, and exits. It NEVER drives
// the smoke composer flow. `desktopStartupT0` is the wall-clock (ms epoch) of
// this main process's performance origin (≈ process start); every reported mark
// is `Date.now()` minus this origin.
const argvMvpProofEnvironment = mvpProofEnvironmentFromArgv(process.argv)
if (argvMvpProofEnvironment !== null) Object.assign(process.env, argvMvpProofEnvironment)
const startupMarksFile = process.env.OPENAGENTS_DESKTOP_STARTUP_MARKS ?? null
const startupMarksMode = startupMarksFile !== null
// Real-wiring startup trace (2026-07-13 startup incident): the SAME milestone
// driver as startup-marks, but WITHOUT fixture substitution — real userData,
// real ~/.codex, real session recovery, then exit. Receipts carry timings
// only (never paths, tokens, or thread content). Ignored when the fixture
// marks mode is active; the two must not mix wiring.
const startupTraceFile = startupMarksMode ? null : (process.env.OPENAGENTS_DESKTOP_STARTUP_TRACE ?? null)
const startupTraceMode = startupTraceFile !== null
const desktopStartupT0 = performance.timeOrigin
const desktopMainMarks: Record<string, number> = {}
const recordMainMark = (name: string): void => {
  if (desktopMainMarks[name] === undefined) desktopMainMarks[name] = Date.now()
}
recordMainMark("mainModuleEvaluated")
// Startup-marks mode reuses the deterministic smoke fixtures so the benchmark
// isolates main-process init ordering rather than live filesystem/network state.
const localTurnRestartProbe = process.env.OPENAGENTS_DESKTOP_LOCAL_TURN_RESTART_PROBE === "seed" ||
    process.env.OPENAGENTS_DESKTOP_LOCAL_TURN_RESTART_PROBE === "recover"
  ? process.env.OPENAGENTS_DESKTOP_LOCAL_TURN_RESTART_PROBE
  : null
const smokeMode = process.env.OPENAGENTS_DESKTOP_SMOKE === "1" || startupMarksMode || localTurnRestartProbe !== null
const liveProofDriverMode = process.env.OPENAGENTS_DESKTOP_LIVE_PROOF === "1"
const mvpProofDriverMode = process.env.OPENAGENTS_DESKTOP_MVP_PROOF === "1"
// Capture before any host lifecycle can change process state. This is the
// default top-level coding workspace today; the runtime-facing getter is the
// seam a future persisted directory setting/picker will replace.
const desktopLaunchWorkingDirectory = path.resolve(app.getPath("home"))
const productionUserDataPath = path.join(app.getPath("appData"), "OpenAgents")
const legacyDevelopmentUserDataPath = path.join(app.getPath("appData"), "OpenAgentsDesktopDev")
if (!smokeMode && !liveProofDriverMode && !mvpProofDriverMode && process.env.OPENAGENTS_DESKTOP_USER_DATA === undefined &&
    !existsSync(productionUserDataPath) && existsSync(legacyDevelopmentUserDataPath)) {
  try {
    // Same-parent rename preserves the complete durable profile atomically.
    // Failure is non-destructive: production still starts at its canonical
    // path and the legacy directory remains untouched for manual recovery.
    renameSync(legacyDevelopmentUserDataPath, productionUserDataPath)
  } catch { /* retain the legacy profile without deleting or partially copying it */ }
}
const desktopUserDataPath = process.env.OPENAGENTS_DESKTOP_USER_DATA ?? (
  smokeMode || liveProofDriverMode || mvpProofDriverMode
    ? path.join(
        app.getPath("temp"),
        `openagents-desktop-${startupMarksMode ? "startup-marks" : smokeMode ? "smoke" : liveProofDriverMode ? "live-proof" : "mvp-proof"}-${process.pid}`,
      )
    : productionUserDataPath
)
app.setPath("userData", desktopUserDataPath)
const isolatedAppProofMode = isIsolatedAppProof({
  env: process.env,
  userDataPath: desktopUserDataPath,
  temporaryDirectory: app.getPath("temp"),
})
const desktopRendererEntry = desktopRendererEntryUrl
for (const chromiumSwitch of isolatedAppProofChromiumSwitches(isolatedAppProofMode)) {
  // Chromium normally initializes macOS cookie encryption when its default
  // session is created. The signed, signed-out acceptance candidate uses an
  // OS-temp profile and must neither prompt for nor read the owner's Keychain.
  app.commandLine.appendSwitch(chromiumSwitch)
}
const smokeFixtureRoot = smokeMode && app.isPackaged
  ? path.join(desktopUserDataPath, "smoke-fixtures")
  : smokeFixtureSourceRoot
if (smokeMode && app.isPackaged) {
  rmSync(smokeFixtureRoot, { recursive: true, force: true })
  cpSync(smokeFixtureSourceRoot, smokeFixtureRoot, { recursive: true })
}
const primaryDesktopInstance = app.requestSingleInstanceLock()
if (!primaryDesktopInstance) app.quit()
const desktopCommandHost = makeDesktopCommandHost()
let desktopCommandWindow: BrowserWindow | null = null
// BrowserWindow is a native resource whose JavaScript wrapper must remain
// strongly reachable even before the renderer completes its IPC handshake.
// Production normally attaches quickly; isolated proof deliberately exercises
// a fresh browser profile and must not let GC close the only window mid-bootstrap.
let primaryDesktopWindow: BrowserWindow | null = null
let desktopCommandBindings: DesktopCommandBindingStore | null = null

const focusDesktopCommandWindow = (): void => {
  const window = desktopCommandWindow ?? BrowserWindow.getAllWindows()[0] ?? null
  if (window === null || window.isDestroyed()) return
  if (window.isMinimized()) window.restore()
  window.show()
  window.focus()
}

for (const command of desktopCommandsFromArgv(process.argv, "deep_link")) {
  desktopCommandHost.enqueue(command)
}
app.on("second-instance", (_event, argv) => {
  focusDesktopCommandWindow()
  for (const command of desktopCommandsFromArgv(argv, "second_instance")) {
    desktopCommandHost.enqueue(command)
  }
})
app.on("open-url", (event, url) => {
  event.preventDefault()
  focusDesktopCommandWindow()
  const command = parseDesktopCommandUrl(url)
  if (command !== null) desktopCommandHost.enqueue(command)
})
// Generated by the desktop build from the exact checked-in OpenAgents mobile
// icon. Keep it inside the packaged runtime directory: no renderer or
// user-controlled path has filesystem authority through this asset.
const desktopIconPath = path.join(here, "assets", "openagents-icon.png")

// Smoke runs headless and can never complete a real browser device-auth, so
// it uses a scripted fixture child (clearly logged; never in normal runs).
if (smokeMode) {
  console.log("[openagents-desktop] codex-connect running in SMOKE FIXTURE mode (no real pylon spawn)")
}
const codexConnect = makeCodexConnectService(here, {
  ...(smokeMode ? { spawnPylon: makeFixtureSpawnPylon() } : {}),
  openExternal: (url) => shell.openExternal(url),
})
if (smokeMode) {
  console.log("[openagents-desktop] provider-accounts running in SMOKE FIXTURE mode (no real pylon spawn)")
}
const providerAccountsDiagnostics: Array<Readonly<Record<string, string | number | boolean | null>>> = []
const providerAccounts = makeProviderAccountsService(
  here,
  smokeMode
    ? { spawnPylon: makeFixtureProviderAccountsSpawn() }
    : { packaged: app.isPackaged, diagnostic: event => providerAccountsDiagnostics.push(event) },
)
let desktopSessionVault: DesktopSessionVault | null = null
let desktopSessionState: "signed_out" | "credential_present_unverified" | "session_ready" | "denied" | "unavailable" = "unavailable"
const desktopOperationSessionRef = `session.desktop.${randomUUID()}`
let desktopCorrelationSequence = 0
const desktopCorrelationJournal = makeDesktopCorrelationJournal()
const operationContextFor = (request: DesktopRuntimeGatewayRequest): DesktopOperationContext | null => {
  const operationRef = desktopOperationRef(request)
  const runRef = request.kind === "command" && "runRef" in request.command
    ? request.command.runRef
    : undefined
  if (request.context !== undefined) {
    return request.context.operationRef === operationRef &&
      request.context.sessionRef === desktopOperationSessionRef &&
      request.context.runRef === runRef
      ? request.context
      : null
  }
  return decodeDesktopOperationContext({
    operationRef,
    sessionRef: desktopOperationSessionRef,
    correlationRef: `correlation.desktop.${++desktopCorrelationSequence}`,
    ...(runRef === undefined ? {} : { runRef }),
  })
}
const connectVerifiedDesktopSync = (): boolean => {
  const syncHost = hostLifecycle.sync()
  if (syncHost === null || desktopSessionVault === null) return false
  try {
    const credential = desktopSessionVault.load()
    if (credential === null) {
      syncHost.disconnectAuthenticated()
      return false
    }
    syncHost.connectAuthenticated({
      verification:"server_verified",
      baseUrl: process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com",
      ownerUserId: credential.ownerUserId,
      authToken: () => desktopSessionVault?.load()?.accessToken ?? "",
    })
    return true
  } catch {
    syncHost.disconnectAuthenticated()
    return false
  }
}
const runtimeLiveSubscriptions = createDesktopRuntimeLiveSubscriptions({
  conversation: () => hostLifecycle.sync()?.conversation() ?? null,
  timeline: () => hostLifecycle.sync()?.timeline() ?? null,
  agentGraph: () => hostLifecycle.sync()?.agentGraph() ?? null,
})
const runtimeGateway = createDesktopRuntimeGateway(() => desktopRuntimeCapabilities({
  sessionLocalState: desktopSessionState,
  syncLocalState: hostLifecycle.sync()?.status().state === "local_ready" ? "ready" : "unavailable",
  syncNetworkPhase: hostLifecycle.sync()?.status().syncPhase ?? "closed",
}), {
  signIn: async signal => {
    if (desktopSessionVault === null) return { state: "unavailable" }
    const previous = desktopSessionState
    const result = await signInDesktopSession({
      vault: desktopSessionVault,
      openExternal: url => shell.openExternal(url),
      signal,
    })
    if (result.state === "verified") {
      await runtimeLiveSubscriptions.reset()
      desktopSessionState = connectVerifiedDesktopSync() ? "session_ready" : "unavailable"
    } else {
      desktopSessionState = result.state === "cancelled" ? previous : "unavailable"
    }
    return result
  },
  signOut: async signal => {
    if (desktopSessionVault === null) return { state: "unavailable" }
    // Close and purge the account-linked Sync session before the renderer can
    // race another command against remote token revocation.
    await runtimeLiveSubscriptions.reset()
    await hostLifecycle.voice()?.command({ protocolVersion: 1, id: "voice.revoke" })
    try { hostLifecycle.sync()?.unlinkAccount() } catch { /* remote revocation still runs */ }
    const result = await signOutDesktopSession({ vault: desktopSessionVault, signal })
    desktopSessionState = result.state
    return result
  },
}, () => desktopSessionState === "credential_present_unverified" ? "unverified" : desktopSessionState, () => {
  const service = hostLifecycle.sync()?.conversation() ?? null
  if (service === null) return null
  return {
    catalog: () => ({
      status: service.personalStatus(),
      threads: Effect.runSync(service.listConfirmedThreads()),
    }),
    thread: threadRef => {
      Effect.runSync(service.openThread(threadRef))
      return {
        status: service.threadStatus(threadRef),
        messages: Effect.runSync(service.listConfirmedMessages(threadRef)),
      }
    },
    create: (threadRef, title) => Number(Effect.runSync(service.createThread({
      threadId: threadRef,
      title,
    }))),
    append: (threadRef, messageRef, body) => Number(Effect.runSync(service.appendMessage({
      threadId: threadRef,
      messageId: messageRef,
      body,
    }))),
  }
}, () => {
  const service = hostLifecycle.sync()?.timeline() ?? null
  if (service === null) return null
  return {
    snapshot: runRef => {
      Effect.runSync(service.open(runRef))
      return Effect.runSync(service.snapshot(runRef))
    },
    snapshotForThread: threadRef =>
      Effect.runSync(service.snapshotForThread(threadRef)),
  }
}, () => ({
  catalog: () => hostLifecycle.history()!.run({ kind: "history_catalog", sessionsRoot: codexSessionsRoot(), claudeRoot: claudeProjectsRoot() }) as Promise<import("./codex-history-contract.ts").CodexHistoryCatalog>,
  page: (threadRef, offset, limit) => hostLifecycle.history()!.run({ kind: "history_page", sessionsRoot: codexSessionsRoot(), claudeRoot: claudeProjectsRoot(), threadRef, offset, limit }) as Promise<import("./codex-history-contract.ts").CodexHistoryPage | null>,
  search: (query, limit) => hostLifecycle.history()!.run({ kind: "history_search", sessionsRoot: codexSessionsRoot(), claudeRoot: claudeProjectsRoot(), query, limit }) as Promise<import("./codex-history-contract.ts").CodexHistorySearchResponse>,
}),()=>hostLifecycle.sync()===null?"local_unavailable":hostLifecycle.sync()!.status().identityTier, () => {
  const service = hostLifecycle.sync()?.runtime() ?? null
  if (service === null && smokeMode) {
    return {
      start: (_input, operationContext) => {
        if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
        return 1
      },
      interrupt: (_input, operationContext) => {
        if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
        return 1
      },
      continue: (_input, operationContext) => {
        if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
        return 1
      },
      retry: (_input, operationContext) => {
        if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
        return 1
      },
      close: (_input, operationContext) => {
        if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
        return 1
      },
      outcome: () => null,
    }
  }
  if (service === null) return null
  // CUT-16: control intents must carry the exact confirmed turn lane — the
  // durable lane fence rejects a mismatched target (runtime_target_lane_mismatch),
  // so interrupt/continue/retry/close thread the caller-derived lane through
  // instead of hard-coding the Codex default.
  const context = (lane: "codex_app_server" | "claude_pylon" | "hosted_khala" = "codex_app_server") => {
    const createdAt = new Date()
    return {
      expiresAtIso: new Date(createdAt.getTime() + 5 * 60_000).toISOString(),
      nowIso: createdAt.toISOString(),
      surface: "desktop" as const,
      target: { lane },
    }
  }
  return {
    outcome: input => Effect.runSync(service.outcome(input)),
    start: (input, operationContext) => {
      if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
      return Number(Effect.runSync(service.startTurn(buildStartTurnIntent({
        context: context(input.lane),
        correlationRefs: operationContext === undefined ? [] : [
          operationContext.operationRef,
          operationContext.sessionRef,
          operationContext.correlationRef,
        ],
        messageRef: input.messageRef,
        threadRef: input.threadRef,
        turnRef: input.runRef,
      }))))
    },
    interrupt: (input, operationContext) => {
      if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
      return Number(Effect.runSync(service.interruptTurn(buildInterruptTurnIntent({
        commandRef: input.commandRef,
        context: context(input.lane),
        correlationRefs: operationContext === undefined ? [] : [
          operationContext.operationRef,
          operationContext.sessionRef,
          operationContext.correlationRef,
        ],
        threadRef: input.threadRef,
        turnRef: input.runRef,
      }))))
    },
    continue: (input, operationContext) => {
      if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
      return Number(Effect.runSync(service.continueTurn(buildContinueTurnIntent({
        commandRef: input.commandRef,
        context: context(input.lane),
        correlationRefs: operationContext === undefined ? [] : [operationContext.operationRef, operationContext.sessionRef, operationContext.correlationRef],
        threadRef: input.threadRef,
        turnRef: input.runRef,
      }))))
    },
    retry: (input, operationContext) => {
      if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
      return Number(Effect.runSync(service.retryTurn(buildRetryTurnIntent({
        commandRef: input.commandRef,
        context: context(input.lane),
        correlationRefs: operationContext === undefined ? [] : [operationContext.operationRef, operationContext.sessionRef, operationContext.correlationRef],
        threadRef: input.threadRef,
        turnRef: input.runRef,
      }))))
    },
    close: (input, operationContext) => {
      if (operationContext !== undefined) desktopCorrelationJournal.record("sync.intent", operationContext)
      return Number(Effect.runSync(service.closeTurn(buildCloseTurnIntent({
        commandRef: input.commandRef,
        context: context(input.lane),
        correlationRefs: operationContext === undefined ? [] : [operationContext.operationRef, operationContext.sessionRef, operationContext.correlationRef],
        threadRef: input.threadRef,
        turnRef: input.runRef,
      }))))
    },
  }
}, (stage, context) => desktopCorrelationJournal.record(stage, context), () => runtimeLiveSubscriptions, () => {
  const service = hostLifecycle.sync()?.interactions() ?? null
  if (service === null) return null
  return {
    list: threadRef => Effect.runSync(service.list(threadRef)),
    decide: command => Number(Effect.runSync(service.decide(command))),
  }
}, () => hostLifecycle.voice(), {
  // Typed per-harness maintenance (MAINT-1, #8785). Runs entirely on user
  // action through the gateway — nothing here touches the pre-window startup
  // path. Updates BINARIES only: the engine scrubs CODEX_HOME/CLAUDE_CONFIG_DIR
  // from every spawn and never runs a login flow, so the default ~/.codex
  // login home stays untouched. Receipts persist to the shared Pylon home so
  // Desktop and `pylon accounts maintenance` project one provenance ledger.
  status: async () => {
    const projection = await collectHarnessMaintenanceStatus()
    return {
      observedAt: projection.observedAt,
      harnesses: projection.harnesses.map(entry => ({
        harness: entry.harness,
        installed: entry.installed,
        installedVersion: entry.installedVersion,
        latestVersion: entry.latestVersion,
        channel: entry.channel,
        advisory: entry.advisory,
        updateSupported: entry.updateSupported,
      })),
    }
  },
  update: async harness => {
    const receipt = await runHarnessMaintenanceUpdate({ harness })
    try {
      await persistHarnessMaintenanceReceipt({ paths: resolvePylonHome(process.env) }, receipt)
    } catch {
      // Receipt persistence failure never converts a finished maintenance
      // outcome into a phantom success/failure; the typed outcome stands.
    }
    return {
      outcome: receipt.outcome,
      failureReason: receipt.failureReason,
      beforeVersion: receipt.before.installedVersion,
      afterVersion: receipt.after?.installedVersion ?? null,
      receiptId: receipt.receiptId,
    }
  },
})

const isTrustedRuntimeGatewaySender = (event: IpcMainInvokeEvent): boolean => {
  const frame = event.senderFrame
  if (frame === null || frame !== event.sender.mainFrame) return false
  return isTrustedDesktopRendererUrl({
    trustedEntryUrl: desktopRendererEntry,
    value: frame.url,
  })
}

ipcMain.handle(DesktopCommandReadyChannel, (event) => {
  if (!isTrustedRuntimeGatewaySender(event)) return false
  const window = BrowserWindow.fromWebContents(event.sender)
  if (window === null || window.isDestroyed()) return false
  desktopCommandWindow = window
  desktopCommandHost.attach(command => {
    if (!window.isDestroyed()) window.webContents.send(DesktopCommandEventChannel, command)
  })
  return true
})
ipcMain.handle(DesktopCommandBindingsChannel, (event) =>
  isTrustedRuntimeGatewaySender(event) ? desktopCommandBindings?.snapshot() ?? null : null)
ipcMain.handle(DesktopCommandBindingSaveChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event) || desktopCommandBindings === null) return null
  const update = decodeDesktopCommandBindingUpdateOrNull(value)
  if (update === null) return null
  const next = desktopCommandBindings.save(update)
  installDesktopCommandMenu(next)
  return next
})
ipcMain.handle(DesktopCommandBindingsResetChannel, (event) => {
  if (!isTrustedRuntimeGatewaySender(event) || desktopCommandBindings === null) return null
  const next = desktopCommandBindings.reset()
  installDesktopCommandMenu(next)
  return next
})

ipcMain.handle(DesktopRuntimeGatewayInvokeChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { kind: "request_rejected", reason: "untrusted_renderer" } as const
  }
  const request = decodeDesktopRuntimeGatewayRequest(value)
  if (request === null) return invalidDesktopRuntimeGatewayResponse()
  const context = operationContextFor(request)
  if (context === null) return invalidDesktopRuntimeGatewayResponse()
  desktopCorrelationJournal.record("ipc.received", context)
  const gateway = hostLifecycle.runtime()
  if (gateway === null) return { kind: "request_rejected", reason: "gateway_disposed", context } as const
  const outcome = gateway.request(request, context)
  const recordReturned = <Value>(response: Value): Value => {
    desktopCorrelationJournal.record("ipc.returned", context)
    return response
  }
  return outcome instanceof Promise ? outcome.then(recordReturned) : recordReturned(outcome)
})

// Smoke uses a per-run temporary root before single-instance lock acquisition;
// normal launches use the canonical OpenAgents profile above.

const hardenSession = (target: Session): void => {
  // Deny-by-default: this shell requests no runtime permissions.
  target.setPermissionRequestHandler((_webContents, _permission, callback) => {
    callback(false)
  })
}

/**
 * A deliberately single-purpose IPC boundary. The preload validates first and
 * main validates again; the renderer never gets filesystem, token, arbitrary
 * command, or loopback request authority.
 */
ipcMain.handle(FleetStageChannel, async (_event, value: unknown) => {
  const request = decodeFleetStageRequest(value)
  return request === null ? unavailableFleetStageResult() : submitFleetBrief(request)
})

const threads = () => makeThreadStore(path.join(app.getPath("userData"), "threads.json"))
const localTurnJournal = openLocalTurnJournal(
  path.join(app.getPath("userData"), "local-turns", "journal.json"),
)
const codexHandoffBindings = openCodexHandoffBindings(
  path.join(app.getPath("userData"), "codex-handoff", "bindings.json"),
)
const desktopUpdateRoot = path.join(app.getPath("userData"), "updates")
const desktopUpdateChannel = app.getVersion().includes("-rc.") ? "rc" : "stable"
const desktopUpdateApplier = openMacOSUpdateApplier({
  root: desktopUpdateRoot,
  installedAppPath: path.resolve(path.dirname(app.getPath("exe")), "../.."),
  installedVersion: app.getVersion(),
  channel: desktopUpdateChannel,
  packaged: app.isPackaged,
})
const desktopUpdateHost = openDesktopUpdateStagingHost({
  root: desktopUpdateRoot,
  installedVersion: app.getVersion(),
  channel: desktopUpdateChannel,
  openPath: artifactPath => shell.openPath(artifactPath),
  applier: desktopUpdateApplier,
  restart: () => setTimeout(() => {
    app.relaunch()
    app.quit()
  }, 350),
})
let desktopIsQuitting = false
const localTurnFlushers = new Set<() => unknown>()
const codexSessionsRoot = () => path.resolve(
  process.env.OPENAGENTS_DESKTOP_CODEX_SESSIONS ?? (
    smokeMode
      ? path.join(smokeFixtureRoot, "codex-smoke", "sessions")
      : path.join(app.getPath("home"), ".codex", "sessions")
  ),
)
// Claude Code history projects tree (#8712 H3). Read-only, owner-local; imported
// into the SAME catalog as Codex, tagged by source. Null disables the import.
const claudeProjectsRoot = (): string | null => {
  const explicit = process.env.OPENAGENTS_DESKTOP_CLAUDE_PROJECTS
  if (explicit !== undefined) return explicit === "" ? null : path.resolve(explicit)
  return path.resolve(
    smokeMode
      ? path.join(smokeFixtureRoot, "claude-smoke", "projects")
      : path.join(app.getPath("home"), ".claude", "projects"),
  )
}
const codexHistoryWorkerUrl = desktopWorkerUrl(import.meta.url, "codex-history-worker.js")
const codexHistoryHost = makeCodexHistoryHost(makeCodexHistoryUtilityFactory(
  codexHistoryWorkerUrl,
  utilityProcess.fork,
))
const hostLifecycle = makeDesktopHostLifecycle({
  runtime: runtimeGateway,
  account: codexConnect,
  history: codexHistoryHost,
})
const voiceMedia: VoiceNativeMedia = smokeMode
  ? {
      open: input => {
        let captureEnabled = true
        queueMicrotask(() => {
          input.onState("live")
          input.onControl({ kind: "activity", activity: "listening" })
          input.onControl({ kind: "transcript", utteranceRef: "smoke.utterance.1", text: "Open project home overview", final: false })
          setTimeout(() => input.onControl({ kind: "transcript", utteranceRef: "smoke.utterance.1", text: "Open project home overview", final: true }), 800)
          setTimeout(() => input.onControl({ kind: "playback", speechRef: "smoke.speech.1", state: "speaking" }), 900)
          setTimeout(() => input.onControl({ kind: "activity", activity: "speech_detected" }), 975)
          setTimeout(() => input.onControl({ kind: "playback", speechRef: "smoke.speech.1", state: "canceled", outcomeRef: "outcome.smoke.interrupt.1" }), 1_050)
        })
        return {
          setCaptureEnabled: enabled => { captureEnabled = enabled; void captureEnabled },
          speak: async () => true,
          close: () => { captureEnabled = false },
        }
      },
    }
  : createPackagedVoiceNativeMedia({
      resourcesPath: app.isPackaged ? process.resourcesPath : path.join(app.getAppPath(), "dist"),
      verifySignature: absolutePath => {
        if (!app.isPackaged) return true
        try { execFileSync("/usr/bin/codesign", ["--verify", "--strict", absolutePath], { stdio: "ignore" }); return true } catch { return false }
      },
      connection: async (identity, disclosureRef) => {
        const credential = desktopSessionVault?.load()
        if (credential === null || credential === undefined) throw new Error("voice_session_unavailable")
        const response = await fetch(`${process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com"}/api/desktop/audio/grant`, {
          method: "POST",
          headers: { authorization: `Bearer ${credential.accessToken}`, "content-type": "application/json", "x-openagents-desktop-device-ref": identity.deviceRef },
          body: JSON.stringify({ schema: "openagents.audio.grant.request.v1", identity, disclosureRef }),
        })
        if (!response.ok) {
          let reason = "unknown"
          try {
            const refusal = await response.json() as Record<string, unknown>
            if (typeof refusal.error === "string" && /^[a-z0-9_]{1,64}$/u.test(refusal.error)) reason = refusal.error
          } catch { /* keep the response body opaque */ }
          throw new Error(`voice_grant_refused:${response.status}:${reason}`)
        }
        const value = await response.json() as Record<string, unknown>
        if (value.schema !== "openagents.audio.grant.v1") throw new Error("voice_grant_invalid:schema")
        if (value.disclosureRef !== disclosureRef) throw new Error("voice_grant_invalid:disclosure")
        if (typeof value.gatewayUrl !== "string" || !value.gatewayUrl.startsWith("wss://")) throw new Error("voice_grant_invalid:gateway")
        if (typeof value.grant !== "string" || value.grant.length < 16 || value.grant.length > 4096) throw new Error("voice_grant_invalid:grant")
        // The issuer targets five minutes, but its clock may lead the Desktop's
        // clock. Enforce the AUDIO-2 protocol maximum instead of requiring
        // impossible millisecond clock equality across machines.
        if (typeof value.expiresAtMs !== "number" || !Number.isSafeInteger(value.expiresAtMs) || value.expiresAtMs <= Date.now() || value.expiresAtMs > Date.now() + 15 * 60_000) throw new Error("voice_grant_invalid:expiry")
        return { gatewayUrl: value.gatewayUrl, grant: value.grant }
      },
    })
hostLifecycle.replaceVoice(createDesktopVoiceHost({
  resolveIdentity: ({ threadRef, sessionRef, generation }) => {
    if (smokeMode) return { ownerRef: "owner.smoke", deviceRef: desktopOperationSessionRef, threadRef, sessionRef, generation }
    const credential = desktopSessionVault?.load()
    return credential === null || credential === undefined ? null : {
      ownerRef: credential.ownerUserId,
      deviceRef: desktopOperationSessionRef,
      threadRef,
      sessionRef,
      generation,
    }
  },
  permission: () => {
    if (smokeMode) return "granted"
    if (process.platform !== "darwin") return "denied"
    const status = systemPreferences.getMediaAccessStatus("microphone")
    return status === "granted" ? "granted" : status === "not-determined" ? "not_determined" : "denied"
  },
  requestPermission: async () => smokeMode || process.platform === "darwin" && await systemPreferences.askForMediaAccess("microphone") ? "granted" : "denied",
  media: voiceMedia,
}))
const workspaceSearchRegistry = makeWorkspaceSearchRegistry(() => hostLifecycle.workspace())
const workspaceSearchOwnerRef = (webContentsId: number): string =>
  `webContents.${webContentsId}`
const requestedWorkspaceChangeWindows = new Set<number>()
const workspaceChangeSubscriptions = new Map<number, () => void>()
const closeWorkspaceChangeSubscription = (windowId: number): void => {
  const close = workspaceChangeSubscriptions.get(windowId)
  workspaceChangeSubscriptions.delete(windowId)
  close?.()
}
const bindWorkspaceChangeSubscription = (windowId: number): boolean => {
  closeWorkspaceChangeSubscription(windowId)
  if (!requestedWorkspaceChangeWindows.has(windowId)) return false
  const window = BrowserWindow.fromId(windowId)
  const workspace = hostLifecycle.workspace()
  if (window === null || window.isDestroyed() || workspace === null) return false
  const subscription = workspace.subscribe(change => {
    if (!window.isDestroyed()) {
      window.webContents.send(DesktopWorkspaceChangeChannel, change)
    }
  })
  workspaceChangeSubscriptions.set(windowId, subscription.close)
  return true
}
const rebindWorkspaceChangeSubscriptions = (): void => {
  for (const windowId of requestedWorkspaceChangeWindows) {
    bindWorkspaceChangeSubscription(windowId)
  }
}
const disableWorkspaceChangeSubscription = (windowId: number): void => {
  requestedWorkspaceChangeWindows.delete(windowId)
  closeWorkspaceChangeSubscription(windowId)
}
// Local file authority begins only after an explicit directory-picker choice.
// A process working directory or environment default is not user selection.
const workspaceSnapshot = () => {
  const workspace = hostLifecycle.workspace()
  if (workspace === null) return null
  try { return workspace.summary() } catch { return null }
}
const productSpecUnavailable = (message: string): ProductSpecOperationError => ({
  ok: false,
  reason: "invalid_request",
  message,
})
const currentProductSpecWorkroom = () => {
  const catalog = hostLifecycle.sync()?.codingCatalog()?.snapshot()
  const workspace = hostLifecycle.workspace()
  if (catalog === null || catalog === undefined || catalog.resolution?.state !== "ready" || workspace === null) {
    return null
  }
  const root = hostLifecycle.sync()?.codingCatalog()?.selectedRoot() ?? null
  if (root === null) return null
  const selectedRootProjection = (() => {
    try { return workspace.summary().root } catch { return null }
  })()
  if (selectedRootProjection === null || path.resolve(root) !== path.resolve(selectedRootProjection)) return null
  const workContextRef = catalog.resolution.session.workContextRef
  return {
    workContextRef,
    sessionRef: catalog.resolution.session.sessionRef,
    workspaceRoot: selectedRootProjection,
    service: makeProductSpecWorkroom({
      workspaceRoot: selectedRootProjection,
      stateRoot: path.join(app.getPath("userData"), "product-spec", workContextRef),
    }),
  }
}
const withProductSpecWorkroom = <A>(
  event: IpcMainInvokeEvent,
  work: (authority: NonNullable<ReturnType<typeof currentProductSpecWorkroom>>) => A,
): A | ProductSpecOperationError => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return productSpecUnavailable("The ProductSpec request did not come from the trusted Desktop renderer.")
  }
  const authority = currentProductSpecWorkroom()
  return authority === null
    ? productSpecUnavailable("Choose an admitted coding workspace before using ProductSpec work.")
    : work(authority)
}
const openSelectedWorkspace = (root: string) => openWorkspaceService(root, {
  reveal: absolutePath => {
    shell.showItemInFolder(absolutePath)
    return true
  },
})
const installAdmittedCodingWorkspace = (root: string): boolean => {
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (catalog === null || catalog === undefined) return false
  const admitted = openAdmittedDesktopWorkspace(
    catalog,
    root,
    (selectedRoot, grantRef) => openWorkspaceService(selectedRoot, {
      grantRef,
      reveal: absolutePath => {
        shell.showItemInFolder(absolutePath)
        return true
      },
    }),
  )
  hostLifecycle.replaceWorkspace(admitted.workspace)
  return true
}
const codingCatalogSnapshot = (offset = 0) => {
  const catalog = hostLifecycle.sync()?.codingCatalog()
  return catalog === null || catalog === undefined
    ? emptyDesktopCodingCatalogProjection()
    : projectDesktopCodingCatalog(catalog.snapshot(), offset)
}
const codingThreadCreationAttempted = new Set<string>()
const publishCodingCatalog = (): void => {
  const sync = hostLifecycle.sync()
  const catalog = sync?.codingCatalog()
  const conversation = sync?.conversation()
  if (catalog !== null && catalog !== undefined && conversation !== null && conversation !== undefined) {
    const snapshot = catalog.snapshot()
    const confirmedThreadRefs = new Set(
      Effect.runSync(conversation.listConfirmedThreads()).map(thread => thread.threadRef),
    )
    for (const session of snapshot.catalog.sessions) {
      if (session.state === "archived" || confirmedThreadRefs.has(session.threadRef) ||
        codingThreadCreationAttempted.has(session.threadRef)) continue
      codingThreadCreationAttempted.add(session.threadRef)
      const repository = snapshot.catalog.repositories.find(value =>
        value.repositoryRef === session.repositoryRef)
      try {
        Effect.runSync(conversation.createThread({
          threadId: session.threadRef,
          title: repository === undefined ? "Coding session" : `Coding · ${repository.displayName}`,
        }))
      } catch {
        codingThreadCreationAttempted.delete(session.threadRef)
      }
    }
  }
  sync?.publishCodingCatalog()
}
// A workspace change revokes every terminal bound to the OUTGOING grant: its
// owned process trees are killed exactly once before the new root is authorized.
const revokeOutgoingWorkspaceTerminals = (): void => {
  const outgoing = hostLifecycle.workspace()?.grantRef ?? null
  if (outgoing !== null) terminalHost.revokeWorkspace(outgoing)
}
const activateCodingCatalogRoot = () => {
  const root = hostLifecycle.sync()?.codingCatalog()?.selectedRoot() ?? null
  if (root !== null) {
    revokeOutgoingWorkspaceTerminals()
    if (!installAdmittedCodingWorkspace(root)) {
      hostLifecycle.replaceWorkspace(openSelectedWorkspace(root))
    }
    rebindWorkspaceChangeSubscriptions()
  }
}
const chooseCodingWorkspace = async (registerCatalog = true) => {
  const result = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] })
  if (result.canceled || result.filePaths[0] === undefined) return null
  const root = result.filePaths[0]
  if (registerCatalog) {
    revokeOutgoingWorkspaceTerminals()
    if (!installAdmittedCodingWorkspace(root)) {
      hostLifecycle.replaceWorkspace(openSelectedWorkspace(root))
    }
    rebindWorkspaceChangeSubscriptions()
    publishCodingCatalog()
  }
  return root
}
ipcMain.handle(DesktopWindowFullscreenChannel, (event) => {
  const window = BrowserWindow.fromWebContents(event.sender)
  if (window === null || window.isDestroyed()) return false
  const next = !window.isFullScreen()
  window.setFullScreen(next)
  return next
})
ipcMain.handle(DesktopWorkspaceSummaryChannel, () => workspaceSnapshot())
ipcMain.handle(DesktopWorkspaceFilesChannel, () => workspaceSnapshot())
ipcMain.handle(DesktopWorkspaceTreeChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { state: "unavailable", message: "The workspace tree request is unavailable." }
  }
  const request = decodeWorkspaceTreeRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", message: "Choose a workspace folder before browsing files." }
    : workspace.tree(request)
})
ipcMain.handle(DesktopWorkspaceSearchChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return {
      requestRef: "workspace.search.request.invalid",
      page: { state: "unavailable", message: "Workspace search is unavailable." },
    }
  }
  const request = decodeWorkspaceSearchBridgeRequest(value)
  return request === null
    ? {
        requestRef: "workspace.search.request.invalid",
        page: { state: "unavailable", message: "The workspace search request is invalid." },
      }
    : workspaceSearchRegistry.start(workspaceSearchOwnerRef(event.sender.id), request)
})
ipcMain.handle(DesktopWorkspaceSearchCancelChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { requestRef: "workspace.search.request.invalid", cancelled: false }
  }
  const request = decodeWorkspaceSearchCancelRequest(value)
  return request === null
    ? { requestRef: "workspace.search.request.invalid", cancelled: false }
    : workspaceSearchRegistry.cancel(workspaceSearchOwnerRef(event.sender.id), request.requestRef)
})
ipcMain.handle(DesktopWorkspaceCreateChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { state: "unavailable", message: "Workspace create is unavailable." }
  const request = decodeWorkspaceCreateRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", message: "Choose a workspace folder before creating entries." }
    : workspace.createEntry(request)
})
ipcMain.handle(DesktopWorkspaceRenameChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { state: "unavailable", message: "Workspace rename is unavailable." }
  const request = decodeWorkspaceRenameRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", message: "Choose a workspace folder before renaming entries." }
    : workspace.renameEntry(request)
})
ipcMain.handle(DesktopWorkspaceDeleteChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { state: "unavailable", message: "Workspace delete is unavailable." }
  const request = decodeWorkspaceDeleteRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", message: "Choose a workspace folder before deleting entries." }
    : workspace.deleteEntry(request)
})
ipcMain.handle(DesktopWorkspaceRevealChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { state: "unavailable", message: "Workspace reveal is unavailable." }
  const request = decodeWorkspaceRevealRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", message: "Choose a workspace folder before revealing entries." }
    : workspace.revealEntry(request)
})
ipcMain.handle(DesktopWorkspaceDocumentOpenChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { state: "unavailable", reason: "unavailable", message: "Workspace documents are unavailable." }
  }
  const request = decodeWorkspaceDocumentRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", reason: "unavailable", message: "Choose a workspace folder before opening documents." }
    : workspace.openDocument(request)
})
ipcMain.handle(DesktopWorkspaceDocumentSaveChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { state: "unavailable", reason: "unavailable", message: "Workspace document saving is unavailable." }
  }
  const request = decodeWorkspaceDocumentSaveRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", reason: "unavailable", message: "Choose a workspace folder before saving documents." }
    : workspace.saveDocument(request)
})
ipcMain.handle(DesktopWorkspaceDocumentSaveAsChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { state: "unavailable", reason: "unavailable", message: "Workspace Save As is unavailable." }
  }
  const request = decodeWorkspaceDocumentSaveAsRequest(value)
  const workspace = hostLifecycle.workspace()
  return request === null || workspace === null
    ? { state: "unavailable", reason: "unavailable", message: "Choose a workspace folder before using Save As." }
    : workspace.saveDocumentAs(request)
})
ipcMain.handle(DesktopWorkspaceRefreshChannel, event => {
  if (!isTrustedRuntimeGatewaySender(event)) return false
  const workspace = hostLifecycle.workspace()
  if (workspace === null) return false
  workspace.refresh()
  return true
})
ipcMain.handle(DesktopWorkspaceWatchChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return false
  const request = decodeWorkspaceWatchRequest(value)
  const window = BrowserWindow.fromWebContents(event.sender)
  if (request === null || window === null || window.isDestroyed()) return false
  if (!request.active) {
    disableWorkspaceChangeSubscription(window.id)
    return true
  }
  requestedWorkspaceChangeWindows.add(window.id)
  bindWorkspaceChangeSubscription(window.id)
  return true
})
ipcMain.handle(DesktopWorkspaceChooseChannel, async () => {
  await chooseCodingWorkspace()
  return workspaceSnapshot()
})
ipcMain.handle(ProductSpecOpenChannel, (event, raw: unknown) => {
  const request = decodeProductSpecOpenRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec open request is invalid.")
    : withProductSpecWorkroom(event, authority => request.workContextRef !== authority.workContextRef
      ? productSpecUnavailable("The ProductSpec work context is not the selected coding session.")
      : authority.service.open(request))
})
ipcMain.handle(ProductSpecCreateChannel, (event, raw: unknown) => {
  const request = decodeProductSpecCreateRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec create request is invalid.")
    : withProductSpecWorkroom(event, authority => request.workContextRef !== authority.workContextRef
      ? productSpecUnavailable("The ProductSpec work context is not the selected coding session.")
      : authority.service.create(request))
})
ipcMain.handle(ProductSpecEditProposeChannel, (event, raw: unknown) => {
  const request = decodeProductSpecEditProposalRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec edit proposal is invalid.")
    : withProductSpecWorkroom(event, authority => request.workContextRef !== authority.workContextRef
      ? productSpecUnavailable("The ProductSpec work context is not the selected coding session.")
      : authority.service.proposeEdit(request))
})
ipcMain.handle(ProductSpecEditConfirmChannel, (event, raw: unknown) => {
  const request = decodeProductSpecEditConfirmRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec edit confirmation is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.confirmEdit(request))
})
ipcMain.handle(ProductSpecEvidenceAttachmentProposeChannel, (event, raw: unknown) => {
  const request = decodeProductSpecEvidenceAttachmentProposalRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec evidence-attachment proposal is invalid.")
    : withProductSpecWorkroom(event, authority => request.workContextRef !== authority.workContextRef
      ? productSpecUnavailable("The ProductSpec work context is not the selected coding session.")
      : authority.service.proposeEvidenceAttachment(request))
})
ipcMain.handle(ProductSpecEvidenceAttachmentConfirmChannel, (event, raw: unknown) => {
  const request = decodeProductSpecEvidenceAttachmentConfirmRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec evidence-attachment confirmation is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.confirmEvidenceAttachment(request))
})
ipcMain.handle(ProductSpecPlanProposeChannel, (event, raw: unknown) => {
  const request = decodeProductSpecPlanProposalRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec plan proposal is invalid.")
    : withProductSpecWorkroom(event, authority => request.workContextRef !== authority.workContextRef
      ? productSpecUnavailable("The ProductSpec work context is not the selected coding session.")
      : authority.service.proposePlan(request))
})
ipcMain.handle(ProductSpecPlanAcceptChannel, (event, raw: unknown) => {
  const request = decodeProductSpecPlanAcceptRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec plan acceptance is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.acceptPlan(request))
})
ipcMain.handle(ProductSpecPacketAdmitChannel, (event, raw: unknown) => {
  const request = decodeProductSpecPacketAdmitRequest(raw)
  if (request === null) {
    return productSpecUnavailable("The ProductSpec packet admission is invalid.")
  }
  return withProductSpecWorkroom(event, authority => {
    const result = authority.service.admitPacket(request)
    if (result.ok) codexHandoffBindings.recordPacketAdmission(result.value, request.packetRef)
    return result
  })
})
ipcMain.handle(ProductSpecPacketBlockChannel, (event, raw: unknown) => {
  const request = decodeProductSpecPacketBlockRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec packet block transition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.blockPacket(request))
})
ipcMain.handle(ProductSpecPacketDispositionChannel, (event, raw: unknown) => {
  const request = decodeProductSpecPacketDispositionRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec packet disposition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.disposePacket(request))
})
ipcMain.handle(ProductSpecRunDispositionChannel, (event, raw: unknown) => {
  const request = decodeProductSpecRunDispositionRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec run disposition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.disposeRun(request))
})
ipcMain.handle(ProductSpecEvidenceRecordChannel, (event, raw: unknown) => {
  const request = decodeProductSpecEvidenceRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec evidence transition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.recordEvidence(request))
})
ipcMain.handle(ProductSpecEvidenceVerifyChannel, (event, raw: unknown) => {
  const request = decodeProductSpecVerificationRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec verification transition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.verifyEvidence(request))
})
ipcMain.handle(ProductSpecOwnerDispositionChannel, (event, raw: unknown) => {
  const request = decodeProductSpecOwnerDispositionRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec owner disposition is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.setOwnerDisposition(request))
})
ipcMain.handle(ProductSpecRunGetChannel, (event, raw: unknown) => {
  const request = decodeProductSpecRunGetRequest(raw)
  return request === null
    ? productSpecUnavailable("The ProductSpec run request is invalid.")
    : withProductSpecWorkroom(event, authority => authority.service.run(request.runRef))
})
ipcMain.handle(DesktopCodingCatalogSnapshotChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingCatalogPageRequest(raw)
  return request === null ? codingCatalogSnapshot() : codingCatalogSnapshot(request.offset)
})
ipcMain.handle(DesktopCodingCatalogChooseChannel, async () => {
  await chooseCodingWorkspace()
  return codingCatalogSnapshot()
})
ipcMain.handle(DesktopCodingCatalogOpenChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const snapshot = catalog.openSession(request.sessionRef)
  publishCodingCatalog()
  activateCodingCatalogRoot()
  return projectDesktopCodingCatalog(snapshot)
})
ipcMain.handle(DesktopCodingCatalogArchiveChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const snapshot = catalog.archiveSession(request.sessionRef)
  publishCodingCatalog()
  activateCodingCatalogRoot()
  return projectDesktopCodingCatalog(snapshot)
})
ipcMain.handle(DesktopCodingCatalogDeleteChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const snapshot = catalog.deleteSession(request.sessionRef)
  publishCodingCatalog()
  activateCodingCatalogRoot()
  return projectDesktopCodingCatalog(snapshot)
})
ipcMain.handle(DesktopCodingCatalogRecoverChannel, async (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const root = await chooseCodingWorkspace(false)
  return root === null
    ? codingCatalogSnapshot()
    : (() => {
        const snapshot = catalog.recoverSession(request.sessionRef, root)
        revokeOutgoingWorkspaceTerminals()
        if (!installAdmittedCodingWorkspace(root)) {
          hostLifecycle.replaceWorkspace(openSelectedWorkspace(root))
        }
        rebindWorkspaceChangeSubscriptions()
        publishCodingCatalog()
        return projectDesktopCodingCatalog(snapshot)
      })()
})
ipcMain.handle(DesktopCodingCatalogFocusChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingFocusRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  return request === null || catalog === null || catalog === undefined
    ? codingCatalogSnapshot()
    : (() => {
        const snapshot = catalog.saveFocus(request.sessionRef, request.focus)
        publishCodingCatalog()
        return projectDesktopCodingCatalog(snapshot)
      })()
})
ipcMain.handle(DesktopWorkspaceReadChannel, (_event, value: unknown) => {
  const workspace = hostLifecycle.workspace()
  const request = decodeWorkspaceFileRequest(value)
  return request === null || workspace === null ? null : workspace.read(request.path)
})
ipcMain.handle(DesktopWorkspaceSaveChannel, (_event, value: unknown) => {
  const workspace = hostLifecycle.workspace()
  const request = decodeWorkspaceSaveRequest(value)
  if (request === null) return { state: "unavailable", message: "The file save request is invalid." }
  if (workspace === null) return { state: "unavailable", message: "Choose a workspace folder before saving." }
  return workspace.save(request)
})
ipcMain.handle(DesktopWorkspaceGitStatusChannel, () =>
  hostLifecycle.workspace()?.gitStatus() ?? { state: "unavailable" },
)
ipcMain.handle(DesktopWorkspaceGitDiffChannel, (_event, value: unknown) => {
  const workspace = hostLifecycle.workspace()
  const request = decodeWorkspaceGitDiffRequest(value)
  if (request === null) return { state: "unavailable", message: "The diff request is invalid." }
  if (workspace === null) return { state: "unavailable", message: "Choose a workspace folder before reviewing changes." }
  return workspace.gitDiff(request.path)
})
// Typed Git/GitHub surface (EP250 E2–E5, #8712): one namespaced invoke over
// the closed operation set. The service re-reads the active workspace root per
// call (in smoke it points at the app's own real repo — derived from the
// bundle location, never ambient cwd — so the panel renders real read-only
// status without a directory-picker). The renderer never supplies argv;
// git-github-host.ts owns the fixed argument vectors.
const prepareSmokeGitRoot = (): string => {
  const root = path.join(app.getPath("userData"), "git-review-smoke")
  rmSync(root, { recursive: true, force: true })
  mkdirSync(root, { recursive: true })
  const runGit = (...args: ReadonlyArray<string>): void => {
    execFileSync("git", ["-C", root, ...args], {
      env: workspaceGitEnvironment(),
      stdio: "ignore",
    })
  }
  runGit("init", "--quiet", "-b", "main")
  runGit("config", "user.email", "desktop-smoke@openagents.test")
  runGit("config", "user.name", "OpenAgents Desktop Smoke")
  writeFileSync(path.join(root, "review-smoke.txt"), "base\n")
  runGit("add", "review-smoke.txt")
  runGit("commit", "--quiet", "-m", "smoke base")
  writeFileSync(path.join(root, "review-smoke.txt"), "base\nreview change\n")
  return root
}
const smokeGitRoot = smokeMode ? prepareSmokeGitRoot() : path.resolve(here, "..")
const gitGithubService = openGitGithubService(
  smokeMode
    ? () => smokeGitRoot
    : () => {
        const workspace = hostLifecycle.workspace()
        if (workspace === null) return null
        try { return workspace.summary().root } catch { return null }
      },
)
ipcMain.handle(GitGithubChannel, (_event, value: unknown) => gitGithubService.run(value))

// --- Hidden-ref turn checkpoints (GIT-1, #8781) -----------------------------
// Workspace state is captured at coding-turn boundaries as hidden Git refs
// (refs/openagents/checkpoints/<thread>/<turn>) through an isolated temporary
// GIT_INDEX_FILE — user branches, the user index, and the worktree are never
// written by capture. Capture failures never fail a turn: a non-git or absent
// workspace refuses typed and the turn proceeds. Snapshots can contain
// secrets, so records stay host-local and never enter Sync projections.
const turnCheckpoints = openTurnCheckpointService({
  resolveRoot: smokeMode
    ? () => smokeGitRoot
    : () => {
        const workspace = hostLifecycle.workspace()
        if (workspace === null) return null
        try { return workspace.summary().root } catch { return null }
      },
})
const captureTurnCheckpoint = async (
  threadRef: string,
  turnRef: string,
  boundary: "turn_start" | "turn_completed",
): Promise<void> => {
  try {
    await turnCheckpoints.capture({ threadRef, turnRef, boundary })
  } catch {
    // Typed refusals are the normal path; a defect here must not touch turns.
  }
}

// --- Workspace-bounded PTY terminals (CUT-20, #8700) -----------------------
// A main-only PTY host: each session binds to the currently authorized
// workspace root + a bounded environment. The renderer steers stdin through
// typed intents only; it never chooses the shell, argv, cwd, or env. Output is
// bounded (ring buffer) and redacted (secret env values scrubbed) before it is
// broadcast to the renderer. In smoke it binds to the app's own repo so the
// journey can run a real bounded command without a directory-picker.
const terminalWorkspaceBinding = (): { root: string; grantRef: string } | null => {
  const workspace = hostLifecycle.workspace()
  if (workspace !== null) {
    try {
      return { root: workspace.summary().root, grantRef: workspace.grantRef }
    } catch {
      return null
    }
  }
  return smokeMode ? { root: smokeGitRoot, grantRef: "workspace.grant.smoke" } : null
}
const broadcastTerminalEvent = (event: TerminalEvent): void => {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) window.webContents.send(TerminalEventChannel, event)
  }
}
const confirmTerminalPreview = async (url: string): Promise<boolean> => {
  const window = BrowserWindow.getAllWindows().find((candidate) => !candidate.isDestroyed()) ?? null
  const options = {
    type: "question" as const,
    buttons: ["Cancel", "Open in browser"],
    defaultId: 1,
    cancelId: 0,
    message: "Open local preview",
    detail: `Open ${url}? Only local preview URLs this terminal announced can be opened, and they open in your default browser — never inside the app.`,
  }
  const result = window === null
    ? await dialog.showMessageBox(options)
    : await dialog.showMessageBox(window, options)
  if (result.response !== 1) return false
  await shell.openExternal(url)
  return true
}
const terminalHost = makeTerminalHost({
  workspace: terminalWorkspaceBinding,
  emit: broadcastTerminalEvent,
  persistencePath: path.join(app.getPath("userData"), "terminals.json"),
  openPreview: confirmTerminalPreview,
})
ipcMain.handle(TerminalCreateChannel, (_event, value: unknown) => {
  const request = decodeTerminalCreateRequest(value)
  return request === null
    ? { ok: false, reason: "invalid_request", message: "The terminal request is invalid." }
    : terminalHost.create(request)
})
ipcMain.handle(TerminalInputChannel, (_event, value: unknown) => {
  const request = decodeTerminalInputRequest(value)
  return request === null ? { ok: false, reason: "invalid_request" } : terminalHost.input(request.sessionRef, request.data)
})
ipcMain.handle(TerminalResizeChannel, (_event, value: unknown) => {
  const request = decodeTerminalResizeRequest(value)
  return request === null ? { ok: false, reason: "invalid_request" } : terminalHost.resize(request.sessionRef, request.cols, request.rows)
})
ipcMain.handle(TerminalInterruptChannel, (_event, value: unknown) => {
  const request = decodeTerminalSessionRequest(value)
  return request === null ? { ok: false, reason: "invalid_request" } : terminalHost.interrupt(request.sessionRef)
})
ipcMain.handle(TerminalRestartChannel, (_event, value: unknown) => {
  const request = decodeTerminalSessionRequest(value)
  return request === null ? { ok: false, reason: "invalid_request" } : terminalHost.restart(request.sessionRef)
})
ipcMain.handle(TerminalCloseChannel, (_event, value: unknown) => {
  const request = decodeTerminalSessionRequest(value)
  return request === null ? { ok: false, reason: "invalid_request" } : terminalHost.close(request.sessionRef)
})
ipcMain.handle(TerminalSnapshotChannel, () => terminalHost.snapshot())
ipcMain.handle(TerminalPreviewOpenChannel, (_event, value: unknown) => {
  const request = decodeTerminalPreviewOpenRequest(value)
  return request === null
    ? Promise.resolve({ ok: false, reason: "invalid_request" })
    : terminalHost.openPreview(request.sessionRef, request.port)
})

// List is intentionally metadata-only: a large local history must not
// serialize every transcript into the renderer merely to draw the sidebar.
ipcMain.handle(DesktopThreadsChannel, () => hostLifecycle.history()?.run({ kind: "list", sessionsRoot: codexSessionsRoot(), ...(smokeMode ? { limit: 1 } : {}) }) ?? Promise.resolve(null))
ipcMain.handle(DesktopNewThreadChannel, () => threads().newThread())
// H1 resume picker: app-local threads only. Returning the exact persisted
// thread id lets the next local turn hit fable/codex-local's existing
// per-thread SDK resume seam; imported provider history is never mutated.
ipcMain.handle(DesktopLocalThreadsChannel, () => threads().list())
ipcMain.handle(DesktopResumeLocalThreadChannel, (_event, value: unknown) => {
  const request = decode(DesktopResumeLocalThreadRequestSchema, value) as DesktopResumeLocalThreadRequest | null
  return request === null ? null : threads().open(request.threadRef)
})
// H2 refs-only fork. Main re-reads a bounded provider-history window through
// the history worker, projects only user/assistant prose through the existing
// 12 x 2,000-character local-history bound, then creates a fresh local UUID.
ipcMain.handle(DesktopForkHistoryThreadChannel, async (_event, value: unknown) => {
  const request = decode(DesktopForkHistoryThreadRequestSchema, value) as DesktopForkHistoryThreadRequest | null
  if (request === null) return null
  const history = hostLifecycle.history()
  if (history === null) return null
  const probe = await history.run({
    kind: "history_page",
    sessionsRoot: codexSessionsRoot(),
    claudeRoot: claudeProjectsRoot(),
    threadRef: request.sourceThreadRef,
    offset: 0,
    limit: 1,
  }) as import("./codex-history-contract.ts").CodexHistoryPage | null
  const plan = probe === null ? null : historyForkFetchPlan(probe.totalItems, request.throughSequence)
  if (plan === null) return null
  const page = await history.run({
    kind: "history_page",
    sessionsRoot: codexSessionsRoot(),
    claudeRoot: claudeProjectsRoot(),
    threadRef: request.sourceThreadRef,
    offset: plan.offset,
    limit: plan.limit,
  }) as import("./codex-history-contract.ts").CodexHistoryPage | null
  if (page === null || page.selectedThreadRef !== request.sourceThreadRef) return null
  const seed = historyForkSeed(page.items, plan.throughSequence)
  return seed.length === 0 ? null : threads().forkThread(seed)
})
ipcMain.handle(DesktopOpenThreadChannel, (_event, value: unknown) => {
  const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
  return request === null ? null : hostLifecycle.history()?.run({ kind: "detail", sessionsRoot: codexSessionsRoot(), id: request.id }) ?? null
})
ipcMain.handle(DesktopHydrateThreadChannel, (_event, value: unknown) => {
  const request = decode(DesktopThreadRequestSchema, value) as { id: string } | null
  return request === null ? null : hostLifecycle.history()?.run({ kind: "detail", sessionsRoot: codexSessionsRoot(), id: request.id, messageLimit: 40 }) ?? null
})
ipcMain.handle(DesktopChatTurnChannel, async (_event, value: unknown) => {
  const request = decode(DesktopTurnRequestSchema, value) as { id: string; message: string } | null
  if (request === null || request.message.trim() === "" || request.message.length > 8_000) {
    return { ok: false, error: "That message could not be sent." }
  }
  const store = threads()
  const user: DesktopMessage = { key: randomUUID(), role: "user", text: request.message.trim(), timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) }
  const saved = store.append(request.id, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  try {
    const text = await completeChatTurn(saved.notes)
    const thread = store.append(saved.id, { key: randomUUID(), role: "assistant", text, timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }) })
    return { ok: true, thread }
  } catch (error) {
    return { ok: false, error: error instanceof Error ? error.message : "The model request failed." }
  }
})

// Fable local lane (#8712): a REAL streaming Claude turn on this machine in
// local (not-signed-in) mode, on an isolated `~/.claude-pylon-*` account home
// — never the default `~/.claude`, never a login flow, never the cloud
// gateway. Smoke runs a scripted fixture (clearly logged; never normal runs).
if (smokeMode) {
  console.log("[openagents-desktop] fable-local running in SMOKE FIXTURE mode (no real Claude SDK session)")
  console.log("[openagents-desktop] codex-child running in SMOKE FIXTURE mode (scripted codex exec, no real spawn)")
}
// Session usage ledger (#8712 Lane C): exact per-account token attribution
// for local Fable turns and Codex delegate children. Main-owned; the
// renderer sees only the typed snapshot ("session ledger" evidence label).
const usageLedger = makeUsageLedger()
// CUT-11 (#8691): canonical desktop-local live agent graph. The host folds
// the SAME typed envelopes the renderer stream receives (one applyEvent line
// inside each lane's existing emit callback) into validated
// openagents.live_agent_graph.v1 post-images through the shared reducer,
// broadcast on change and snapshot on invoke. Presentation stays CUT-12.
const broadcastLiveAgentGraphUpdate = (update: LiveAgentGraphUpdateWire): void => {
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) window.webContents.send(LiveAgentGraphUpdateChannel, update)
  }
}
const liveAgentGraph = makeLiveAgentGraphHost({ emit: broadcastLiveAgentGraphUpdate })
// Codex delegate children (#8712 Lane C). Smoke uses the scout's receipted
// scripted streams through the REAL parser: the first registered account
// fails with the exact revoked-refresh-token shape (typed rotation), the
// second completes with exact usage totals.
const codexChildren = makeCodexChildRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  ...(smokeMode
    ? {
        spawnImpl: makeFixtureCodexChildSpawn([
          { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
          { stdout: fixtureCodexSuccessStdout(), exitCode: 0 },
        ]),
        discoverImpl: async () => [
          { ref: "codex", home: "/nonexistent/fixture-codex" },
          { ref: "codex-2", home: "/nonexistent/fixture-codex-2" },
        ],
      }
    : {}),
})
const codexAppServerConfig = {
  binary: resolveBundledCodexExecutable,
  installProductSpecSkill: (account: import("./codex-child-runtime.ts").CodexChildAccount) => {
    if (account.source === "current_session") {
      const verified = verifyBuiltinProductSpecWorkSkill(builtinSkillsRoot)
      return { skillRoot: builtinSkillsRoot, skillPath: verified.skillPath }
    }
    const installed = installBuiltinProductSpecWorkSkill({
      builtinSkillsRoot,
      namedCodexHome: account.home,
      defaultCodexHome: path.join(homedir(), ".codex"),
    })
    return { skillRoot: installed.skillRoot, skillPath: installed.skillPath }
  },
  productSpecDynamicTools: ProductSpecDynamicTools,
  onProductSpecToolCall: async (request: import("./codex-app-server-client.ts").CodexAppServerRequest) => {
    const authority = currentProductSpecWorkroom()
    return handleProductSpecDynamicTool(request, authority === null
      ? null
      : { workContextRef: authority.workContextRef, service: authority.service })
  },
}
// Codex account preflight: an ephemeral read-only app-server turn against
// each named isolated account. `codex login status` is presence-only and can
// report logged-in on revoked homes, so actual protocol success is required.
// Runs on boot (async, non-blocking), on fleet Refresh, after
// reconnect completion, and lazily before the first dispatch this session.
// Results are session-scoped truth feeding the shared account health
// ordering, the fleet readiness projection (via the ledger's typed
// reconnectRequired flag), the composer chip, and the live-proof journal.
if (smokeMode) {
  console.log("[openagents-desktop] codex-preflight running in SMOKE FIXTURE mode (scripted probes, no real spawn)")
  console.log("[openagents-desktop] codex-local running in SMOKE FIXTURE mode (scripted codex exec, no real spawn)")
}
const recordProbeEvidence = (result: CodexProbeResult): void => {
  if (result.state === "verified") {
    usageLedger.markVerified({ provider: "codex", accountRef: result.ref })
    return
  }
  if (result.state === "reconnect_required" || result.state === "credentials_missing" ||
    result.state === "probe_failed") {
    usageLedger.markReconnectRequired({ provider: "codex", accountRef: result.ref })
  }
  // rate_limited: the credential is live — no reconnect mark either way.
}
// Smoke probes are DETERMINISTIC PER ACCOUNT (keyed by the isolated
// CODEX_HOME, not call order): the fleet Refresh trigger re-runs the round,
// and the revoked fixture account must stay revoked across every round.
const smokeProbeSpawnByHome: Record<string, ReturnType<typeof makeFixtureCodexChildSpawn>> = {
  "/nonexistent/fixture-codex": makeFixtureCodexChildSpawn([
    { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
  ]),
  "/nonexistent/fixture-codex-2": makeFixtureCodexChildSpawn([
    { stdout: fixtureCodexSuccessStdout("thread-probe-codex-2"), exitCode: 0 },
  ]),
  [FIXTURE_CODEX_LOCAL_ACCOUNT.home]: makeFixtureCodexChildSpawn([
    { stdout: fixtureCodexSuccessStdout("thread-probe-fixture"), exitCode: 0 },
  ]),
}
const codexPreflight = makeCodexPreflight({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  onResult: recordProbeEvidence,
  ...(!smokeMode ? {
      discoverImpl: async () => (await discoverRegisteredCodexAccounts())
        .filter(account => account.source === "current_session"),
    } : {}),
  ...(!smokeMode ? { appServer: codexAppServerConfig } : {}),
  ...(smokeMode
    ? {
        // Isolated health in smoke so the scripted probe round does not
        // reorder the delegate-child fixture's attempt sequence.
        health: makeCodexAccountHealth(),
        hasAuthImpl: () => true,
        spawnImpl: input =>
          (smokeProbeSpawnByHome[String(input.env.CODEX_HOME)] ??
            smokeProbeSpawnByHome["/nonexistent/fixture-codex"]!)(input),
        discoverImpl: async () => [
          { ref: "codex", home: "/nonexistent/fixture-codex" },
          { ref: "codex-2", home: "/nonexistent/fixture-codex-2" },
          FIXTURE_CODEX_LOCAL_ACCOUNT,
        ],
      }
    : {}),
})
const selectedDesktopWorkspaceRoot = (): string | null => {
  try {
    return hostLifecycle.workspace()?.summary().root ?? null
  } catch {
    return null
  }
}
// Codex local chat lane: the composer's Codex chip uses the pinned app-server
// against the user's ordinary logged-in Codex session with durable
// thread-resume continuity. Pylon accounts are fleet-only, not MVP fallback.
// Smoke alone keeps the legacy scripted JSON fixture parser.
const codexLocal = makeCodexLocalRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  workspaceRoot: () => desktopRuntimeWorkspaceRoot({
    fixtureMode: smokeMode || liveProofDriverMode,
    userDataPath: app.getPath("userData"),
    selectedWorkspaceRoot: selectedDesktopWorkspaceRoot(),
    launchFallbackRoot: desktopLaunchWorkingDirectory,
  }),
  preflight: codexPreflight,
  initialSessions: localTurnJournal.list().flatMap(record =>
    record.lane === "codex-local" && record.providerSessionRef !== null && record.accountRef !== null
      ? [{
          threadRef: record.threadRef,
          threadId: record.providerSessionRef,
          accountRef: record.accountRef,
        }]
      : []),
  onDispatch: input => {
    localTurnJournal.recordDispatch({ ...input, lane: "codex-local" }, input.accountRef)
  },
  onProviderSession: input => {
    localTurnJournal.recordProviderSession(
      { ...input, lane: "codex-local" },
      { accountRef: input.accountRef, providerSessionRef: input.threadId },
    )
  },
  onAccountEvidence: input => {
    if (input.evidence === "verified") {
      usageLedger.markVerified({ provider: "codex", accountRef: input.accountRef })
    } else {
      usageLedger.markReconnectRequired({ provider: "codex", accountRef: input.accountRef })
    }
  },
  ...(!smokeMode ? { appServer: codexAppServerConfig } : {}),
  ...(smokeMode
    ? {
        spawnImpl: input => {
          const resumedThread = input.args[0] === "exec" && input.args[1] === "resume"
            ? input.args[2]
            : undefined
          return makeFixtureCodexChildSpawn([
            { stdout: fixtureCodexLocalTurnStdout(resumedThread), exitCode: 0 },
          ])(input)
        },
        discoverImpl: async () => [FIXTURE_CODEX_LOCAL_ACCOUNT],
      }
    : {}),
})
// User-configured MCP servers (I2, EP250 wave-2). The persistence host owns
// the private JSON file under userData (mode 0600; secret env/header values
// never logged); the settings UI edits it through additive IPC. The runtime
// reads the ENABLED entries fresh per turn via this getter — no restart needed
// for config edits to take effect. In smoke the fable query is a fixture that
// never constructs real SDK MCP servers, so no MCP server is ever spawned.
const mcpConfigStore = openMcpConfigStore(
  path.join(app.getPath("userData"), "mcp", "servers.json"),
)
const pluginConfigStore = openPluginConfigStore(
  path.join(app.getPath("userData"), "plugins", "registry.json"),
)
const fableLocal = makeFableLocalRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  workspaceRoot: () => desktopRuntimeWorkspaceRoot({
    fixtureMode: smokeMode || liveProofDriverMode,
    userDataPath: app.getPath("userData"),
    selectedWorkspaceRoot: selectedDesktopWorkspaceRoot(),
    launchFallbackRoot: desktopLaunchWorkingDirectory,
  }),
  delegate: codexChildren,
  userMcpServers: () => mcpConfigStore.servers(),
  userPlugins: () => pluginConfigStore.enabledPaths(),
  initialSessions: localTurnJournal.list().flatMap(record =>
    record.lane === "fable-local" && record.providerSessionRef !== null && record.accountRef !== null
      ? [{
          threadRef: record.threadRef,
          sessionId: record.providerSessionRef,
          accountRef: record.accountRef,
        }]
      : []),
  onDispatch: input => {
    localTurnJournal.recordDispatch({ ...input, lane: "fable-local" }, input.accountRef)
  },
  onProviderSession: input => {
    localTurnJournal.recordProviderSession(
      { ...input, lane: "fable-local" },
      { accountRef: input.accountRef, providerSessionRef: input.sessionId },
    )
  },
  ...(smokeMode
    ? {
        queryImpl: async () => makeFixtureFableLocalQuery(),
        discoverImpl: async () => [FABLE_LOCAL_FIXTURE_ACCOUNT],
        mcpImpl: async () => makeFixtureFableMcpFactory(),
      }
    : {}),
})
const localTurnRecovery = reconcileLocalTurns({
  journal: localTurnJournal,
  store: threads(),
  codex: codexLocal,
  onThread: thread => {
    for (const window of BrowserWindow.getAllWindows()) {
      if (!window.isDestroyed()) window.webContents.send(DesktopLocalTurnRecoveryUpdateChannel, thread)
    }
  },
}).catch(error => {
  console.error("[openagents-desktop] local turn recovery failed", error instanceof Error ? error.name : "unknown")
  throw error
})
const handoffRef = (prefix: string, value: string): string =>
  `${prefix}.${createHash("sha256").update(value).digest("hex")}`
const runOpen = (args: ReadonlyArray<string>): Promise<boolean> => new Promise(resolve => {
  execFile("/usr/bin/open", [...args], { timeout: 10_000 }, error => resolve(error === null))
})
const codexHandoffHost = makeCodexHandoffHost({
  bindings: codexHandoffBindings,
  ledger: openCodexHandoffLedger(
    path.join(app.getPath("userData"), "codex-handoff", "handoffs.json"),
  ),
  pinnedRuntimeRef: CODEX_LOCAL_RUNTIME_COMPATIBILITY_REF,
  // The installed Codex app has no pinned proof that it can read the named
  // isolated CODEX_HOME and continue that thread. Never manufacture it.
  exactThreadProof: () => null,
  quiesce: async (request, binding, operationRef) => {
    const key = { threadRef: request.threadRef, turnRef: request.turnRef, lane: "codex-local" as const }
    const terminalProof = () => {
      const record = localTurnJournal.get(key)
      if (record === null || record.disposition === null) return null
      return {
        state: "quiescent" as const,
        proof: {
          operationRef,
          workPacketRef: binding.packetRef,
          openAgentsGeneration: binding.generation,
          disposition: record.disposition === "completed" || record.disposition === "resumed_after_restart"
            ? "completed" as const
            : record.disposition === "interrupted_by_restart"
              ? "interrupted" as const
              : "stopped" as const,
          lastDurableEventRef: handoffRef("local-turn-event", `${record.turnRef}\0${record.persistedCursor}`),
          proofRef: handoffRef("quiescence-proof", `${operationRef}\0${record.updatedAt}\0${record.disposition}`),
        },
      }
    }
    const existing = terminalProof()
    if (existing !== null) return existing
    codexLocal.interrupt(request.turnRef)
    const deadline = Date.now() + 5_000
    while (Date.now() < deadline) {
      const terminal = terminalProof()
      if (terminal !== null) return terminal
      await new Promise(resolve => setTimeout(resolve, 25))
    }
    return { state: "not_quiescent" as const }
  },
  repositoryState: async binding => {
    const status = await gitGithubService.run({ op: "status" }) as unknown
    if (typeof status !== "object" || status === null) return null
    const value = status as { ok?: unknown; statusRef?: unknown }
    if (value.ok !== true || typeof value.statusRef !== "string") return null
    return {
      postImageRef: value.statusRef,
      transcriptGapRef: handoffRef("transcript-gap", `${binding.bindingRef}\0${value.statusRef}`),
    }
  },
  launch: async binding => {
    if (process.platform !== "darwin") return "unavailable"
    const authority = currentProductSpecWorkroom()
    if (authority === null || authority.workContextRef !== binding.workContextRef) return "failed"
    const available = existsSync("/Applications/Codex.app") ||
      existsSync(path.join(app.getPath("home"), "Applications", "Codex.app")) ||
      await runOpen(["-Ra", "Codex"])
    if (!available) return "unavailable"
    return await runOpen(["-a", "Codex", authority.workspaceRoot]) ? "opened" : "failed"
  },
})
ipcMain.handle(CodexHandoffOpenChannel, (event, raw: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { state: "refused", reason: "invalid_request", message: "The handoff request did not come from the trusted Desktop renderer." }
  }
  const request = decodeCodexHandoffOpenRequest(raw)
  return request === null
    ? { state: "refused", reason: "invalid_request", message: "The Open in Codex request is invalid." }
    : codexHandoffHost.open(request)
})
ipcMain.handle(DesktopUpdateStagingChannel, (event, raw: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return desktopUpdateHost.snapshot()
  const request = decodeDesktopUpdateStagingAction(raw)
  if (request === null) return desktopUpdateHost.snapshot()
  switch (request.action) {
    case "snapshot": return desktopUpdateHost.snapshot()
    case "check": return desktopUpdateHost.check()
    case "download": return desktopUpdateHost.download()
    case "open_installer": return desktopUpdateHost.openInstaller()
    case "apply": return desktopUpdateHost.apply()
    case "rollback": return desktopUpdateHost.rollback()
  }
})
ipcMain.handle(PluginConfigListChannel, () => pluginConfigStore.list())
ipcMain.handle(PluginConfigChooseChannel, async () => {
  if (liveProofDriverMode || smokeMode) return { state: "cancelled" }
  const selection = await dialog.showOpenDialog({
    title: "Add local Claude plugin",
    properties: ["openDirectory"],
  })
  const pluginPath = selection.canceled ? undefined : selection.filePaths[0]
  return pluginPath === undefined ? { state: "cancelled" } : pluginConfigStore.addPath(pluginPath)
})
ipcMain.handle(PluginConfigToggleChannel, (_event, value: unknown) => {
  const request = decodePluginToggleRequest(value)
  return request === null ? { state: "rejected", reason: "invalid plugin toggle" } : pluginConfigStore.toggle(request.ref, request.enabled)
})
ipcMain.handle(PluginConfigRemoveChannel, (_event, value: unknown) => {
  const request = decodePluginRefRequest(value)
  return request === null ? { state: "rejected", reason: "invalid plugin ref" } : pluginConfigStore.remove(request.ref)
})
// Additive MCP-config IPC (I2). Every request is schema-decoded; an invalid
// payload degrades to a typed rejection and never throws. The renderer only
// ever receives the public-safe projection (no secret values).
ipcMain.handle(McpConfigListChannel, () => mcpConfigStore.list())
ipcMain.handle(McpConfigAddChannel, (_event, value: unknown) => {
  const request = decodeMcpConfigAddRequest(value)
  return request === null
    ? { state: "rejected", reason: "invalid server config" }
    : mcpConfigStore.add(request)
})
ipcMain.handle(McpConfigRemoveChannel, (_event, value: unknown) => {
  const request = decodeMcpConfigNameRequest(value)
  return request === null
    ? { state: "rejected", reason: "invalid server name" }
    : mcpConfigStore.remove(request.name)
})
ipcMain.handle(McpConfigToggleChannel, (_event, value: unknown) => {
  const request = decodeMcpConfigToggleRequest(value)
  return request === null
    ? { state: "rejected", reason: "invalid toggle request" }
    : mcpConfigStore.toggle(request.name, request.enabled)
})

// Typed durable preferences (CUT-24 #8704). The store owns the private JSON
// file under userData (mode 0600), migrating on read. Every mutation crosses
// through the migrator so a hostile patch is field-normalized, never trusted.
const preferencesStore = openDesktopPreferencesStore(
  path.join(app.getPath("userData"), "preferences.json"),
)
ipcMain.handle(DesktopPreferencesGetChannel, () => preferencesStore.snapshot())
ipcMain.handle(DesktopPreferencesUpdateChannel, (_event, value: unknown) =>
  preferencesStore.update(decodeDesktopPreferencesPatch(value)))
ipcMain.handle(DesktopPreferencesResetChannel, () => preferencesStore.reset())

// Diagnostics / watchdog (CUT-24 #8704). Collect live health from the existing
// operability surfaces into the PUBLIC-SAFE report; the export is always
// redacted before it touches disk. Recovery actions map only to safe, typed
// paths (provider re-probe + re-gathers); restart_runtime/reconnect_sync have
// no safe typed restart yet and honestly report "no recovery action available".
const collectDiagnosticsInputs = async (): Promise<DiagnosticsInputs> => {
  const providerList = await providerAccounts.listProviderAccounts().catch(() => null)
  const syncStatus = hostLifecycle.sync()?.status() ?? null
  const workspace = workspaceSnapshot()
  const mcp = mcpConfigStore.list()
  const capabilities = desktopRuntimeCapabilities({
    sessionLocalState: desktopSessionState,
    syncLocalState: syncStatus?.state === "local_ready" ? "ready" : "unavailable",
    syncNetworkPhase: syncStatus?.syncPhase ?? "closed",
  })
  return {
    appVersion: app.getVersion(),
    generatedAt: Date.now(),
    provider:
      providerList === null || providerList.ok !== true
        ? { state: "unavailable", reason: providerList === null ? "list failed" : providerList.reason }
        : { state: "ok", accounts: providerList.accounts.map((account: { ref: string; readiness: string }) => ({ ref: account.ref, readiness: account.readiness })) },
    runtimeGateway:
      hostLifecycle.runtime() === null
        ? { state: "absent" }
        : {
            state: "present",
            lifecycle: "ready",
            sessionPhase: syncStatus?.state === "local_ready" ? "session_ready" : "unavailable",
            capabilities: capabilities.map((capability) => ({ id: capability.id, state: capability.state })),
          },
    sync:
      syncStatus === null
        ? { state: "unobserved" }
        : { state: syncStatus.state, syncPhase: syncStatus.syncPhase, pendingMutationCount: syncStatus.pendingMutationCount },
    workspace:
      workspace === null
        ? { state: "none" }
        : { state: "selected", git: workspace.git, entryCount: workspace.entries.length },
    pty: { state: "available", sessionCount: terminalHost.snapshot().sessions.length },
    extensions:
      mcp.state === "ok"
        ? { state: "ok", enabledCount: mcp.servers.filter((server) => server.enabled).length, totalCount: mcp.servers.length, dropped: mcp.dropped }
        : { state: "unavailable", message: mcp.message },
  }
}
const diagnosticsHost = makeDiagnosticsHost({
  collectInputs: collectDiagnosticsInputs,
  exportDir: path.join(app.getPath("userData"), "diagnostics"),
  recovery: {
    // Safe, typed recovery: re-probe every connected provider account, then
    // the renderer re-gathers so readiness flips without a restart.
    reprobe_providers: async () => {
      try {
        await codexPreflight.probeAll("diagnostics_recovery")
        return { ok: true, notice: "Providers re-checked" }
      } catch {
        return { ok: false, notice: "Provider re-check failed" }
      }
    },
    // These are pure re-gathers of already-fresh sources — safe no-op recovery.
    refresh: async () => ({ ok: true, notice: "Refreshed" }),
    refresh_workspace: async () => ({ ok: true, notice: "Workspace refreshed" }),
    reload_extensions: async () => ({ ok: true, notice: "Extensions reloaded" }),
  },
})
ipcMain.handle(DiagnosticsGatherChannel, () => diagnosticsHost.gather())
ipcMain.handle(DiagnosticsExportChannel, () => diagnosticsHost.exportRedacted())
ipcMain.handle(DiagnosticsActionChannel, (_event, value: unknown) => {
  const action = decodeDiagnosticsAction(value)
  return action === null ? { ok: false, notice: "Unknown action" } : diagnosticsHost.runAction(action)
})

ipcMain.handle(FableLocalAvailabilityChannel, () => fableLocal.availability())
// Image file picker (capability I1): open the native dialog in MAIN, read the
// chosen images from disk here (never the renderer), bound size + count, and
// return decoded base64 attachments matching the boundary contract. Smoke and
// live-proof headless runs cannot open a dialog; the picker simply returns [].
ipcMain.handle(FableLocalPickImagesChannel, async (event) => {
  if (smokeMode || liveProofDriverMode) return []
  const window = BrowserWindow.fromWebContents(event.sender)
  const extensions = FABLE_LOCAL_IMAGE_MEDIA_TYPES.map(type =>
    type === "image/jpeg" ? "jpg" : type.slice("image/".length))
  const result = await (window === null
    ? dialog.showOpenDialog({ properties: ["openFile", "multiSelections"], filters: [{ name: "Images", extensions: [...extensions, "jpeg"] }] })
    : dialog.showOpenDialog(window, { properties: ["openFile", "multiSelections"], filters: [{ name: "Images", extensions: [...extensions, "jpeg"] }] }))
  if (result.canceled) return []
  const mediaTypeForPath = (filePath: string): (typeof FABLE_LOCAL_IMAGE_MEDIA_TYPES)[number] | null => {
    const lower = filePath.toLowerCase()
    if (lower.endsWith(".png")) return "image/png"
    if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg"
    if (lower.endsWith(".webp")) return "image/webp"
    if (lower.endsWith(".gif")) return "image/gif"
    return null
  }
  const attachments: Array<{ mediaType: string; data: string; name: string }> = []
  for (const filePath of result.filePaths.slice(0, FABLE_LOCAL_IMAGE_COUNT_LIMIT)) {
    const mediaType = mediaTypeForPath(filePath)
    if (mediaType === null) continue
    try {
      if (statSync(filePath).size > FABLE_LOCAL_IMAGE_BYTES_LIMIT) continue
      const bytes = readFileSync(filePath)
      if (bytes.length === 0 || bytes.length > FABLE_LOCAL_IMAGE_BYTES_LIMIT) continue
      attachments.push({ mediaType, data: bytes.toString("base64"), name: path.basename(filePath).slice(0, 256) })
    } catch {
      // Skip an unreadable file rather than failing the whole pick.
    }
  }
  return attachments
})
ipcMain.handle(FableLocalInterruptChannel, (_event, value: unknown) => {
  const request = decodeFableLocalInterruptRequest(value)
  return request === null ? false : fableLocal.interrupt(request.turnRef)
})
// EP250 question flow: the renderer's answer to a pending AskUserQuestion
// routes to the runtime's pending-question registry. Schema-checked; an
// unknown/settled questionRef or unmatched answers resolve false (typed
// rejection) and never throw.
ipcMain.handle(FableLocalAnswerQuestionChannel, (_event, value: unknown) => {
  const request = decodeFableLocalAnswerQuestionRequest(value)
  return request === null ? false : fableLocal.answerQuestion(request) || codexLocal.answerQuestion(request)
})
// EP250 runtime-capability substrate (additive; renderer UI is a wave-2 lane).
// Steer/interrupt a running delegate child (G4). Schema-checked; an unknown
// child or turn mismatch returns a typed not_found outcome, never a throw.
ipcMain.handle(FableLocalSteerChildChannel, (_event, value: unknown) => {
  const request = decodeFableLocalSteerChildRequest(value)
  return request === null ? { ok: false, outcome: "not_found" } : fableLocal.steerChild(request)
})
// Enqueue a follow-up while a turn streams (A3). Delivery is queue-until-idle:
// the runtime emits followup_queued now and followup_promoted when the current
// turn ends. Starting the promoted next turn is a wave-2 renderer/host step.
ipcMain.handle(FableLocalQueueFollowupChannel, (_event, value: unknown) => {
  const request = decodeFableLocalQueueFollowupRequest(value)
  return request === null
    ? { ok: false, queued: false, reason: "no_active_turn" }
    : fableLocal.queueFollowup(request)
})
/**
 * Persisted user-note text for a turn (capability I1). An images-only turn
 * (empty message) gets an honest bounded placeholder so the transcript row is
 * never blank; a turn with text keeps the user's text verbatim.
 */
const userNoteText = (message: string, images?: ReadonlyArray<unknown>): string => {
  const trimmed = message.trim()
  if (trimmed !== "") return trimmed
  const count = images?.length ?? 0
  return count === 1 ? "(1 image attached)" : `(${count} images attached)`
}

/**
 * The text block sent to the model (capability I1). Images-only turns get a
 * neutral instruction so the SDK/codex receive non-empty prompt text alongside
 * the image; a turn with text keeps the user's text verbatim.
 */
const turnPromptText = (message: string, images?: ReadonlyArray<unknown>): string => {
  const trimmed = message.trim()
  if (trimmed !== "") return trimmed
  const count = images?.length ?? 0
  return count > 0
    ? `Please look at the attached image${count === 1 ? "" : "s"}.`
    : trimmed
}

ipcMain.handle(FableLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null || !startRequestHasContent(request)) {
    return { ok: false, error: "That message could not be sent." }
  }
  const requestedModel = request.model ?? request.target?.model ?? FABLE_LOCAL_MODEL
  if ((request.target !== undefined && request.target.provider !== "claude_agent") || !isClaudeModel(requestedModel)) {
    return { ok: false, error: "That provider target is not available on the Claude lane." }
  }
  const selectedSkill = request.skill === undefined
    ? null
    : pluginConfigStore.resolveSkill(request.skill.pluginRef, request.skill.name)
  if (request.skill !== undefined && selectedSkill === null) {
    return { ok: false, error: "That local skill is unavailable or disabled." }
  }
  const store = threads()
  if (store.open(request.threadRef) === null) return { ok: false, error: "That conversation no longer exists." }
  const turnKey = { threadRef: request.threadRef, turnRef: request.turnRef, lane: "fable-local" as const }
  const user: DesktopMessage = {
    key: `${request.turnRef}-user`,
    role: "user",
    text: userNoteText(request.message, request.images),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  }
  const accepted = localTurnJournal.accept({
    ...turnKey,
    userMessageKey: user.key,
    assistantMessageKey: `${request.turnRef}-assistant-0`,
    accountRef: request.target?.accountRef ?? null,
    model: requestedModel,
  })
  if (!accepted.accepted) return { ok: false, error: "That turn is already accepted." }
  const saved = store.upsert(request.threadRef, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  // History authority is main's own thread store — the renderer supplies only
  // the new message. The just-appended user note is the prompt, not history.
  const history = saved.notes.filter(note => note.key !== user.key).map(note => ({ role: note.role, text: note.text }))
  const sender = event.sender
  // Message metadata (#8712): record every fact this host observes for the
  // final assistant note so the renderer's inspector can project it later —
  // SDK-reported effective model, lane, account ref, turn ref, exact token
  // total, and wall-clock duration. Bounded public-safe strings only.
  const startedAt = Date.now()
  let effectiveModel: string | null = null
  const textPersistence = makeLocalTurnTextPersistence({
    journal: localTurnJournal,
    store,
    key: turnKey,
    meta: () => ({
      lane: "fable-local",
      turnRef: request.turnRef,
      ...(effectiveModel === null ? {} : { model: effectiveModel }),
    }),
  })
  localTurnFlushers.add(textPersistence.flush)
  // EP250 wave-2 (J2/J4): track the latest plan/todo list so the FINAL plan
  // state persists into the finalized transcript (the live in-place plan card
  // is renderer-only). One persisted plan card, latest wins.
  let latestPlanEntries: ReadonlyArray<{ step: string; status: "pending" | "in_progress" | "completed" }> | null = null
  // CUT-11 (#8691): register the root turn on the canonical live agent
  // graph before its stream events arrive (duplicate turn refs refuse typed
  // inside the assembler; the chat turn itself is never blocked).
  liveAgentGraph.beginTurn({ turnRef: request.turnRef, threadRef: request.threadRef, lane: "fable_claude" })
  // GIT-1 (#8781): checkpoint the pre-turn workspace state as a hidden ref
  // before the model can write files. Awaited so the snapshot cannot race the
  // turn's first edit; refusals (no workspace / not a repo) cost one probe.
  await captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_start")
  const result = await fableLocal.runTurn({
    turnRef: request.turnRef,
    threadRef: request.threadRef,
    history,
    message: turnPromptText(request.message, request.images),
    ...(request.target === undefined ? {} : { accountRef: request.target.accountRef }),
    ...(request.reasoningEffort === undefined ? {} : { reasoningEffort: request.reasoningEffort }),
    model: requestedModel,
    ...(selectedSkill === null ? {} : { skillName: selectedSkill.name }),
    ...(request.permissionMode === "plan_only" ? { planMode: true } : {}),
    ...(request.images !== undefined && request.images.length > 0 ? { images: request.images } : {}),
    emit: turnEvent => {
      // CUT-11 (#8691): fold the SAME typed envelope the renderer receives
      // into the canonical live agent graph (root + codex delegate children).
      liveAgentGraph.applyEvent(request.threadRef, { turnRef: request.turnRef, event: turnEvent })
      if (turnEvent.kind === "model_effective") effectiveModel = turnEvent.model
      if (turnEvent.kind === "text_delta") textPersistence.append(turnEvent.text)
      else textPersistence.boundary()
      // EP250 wave-2 J2/J4: remember the latest todo list; persist it once the
      // turn completes so the finalized transcript keeps a plan/todo card.
      if (turnEvent.kind === "plan_updated") {
        latestPlanEntries = turnEvent.entries.map(entry => ({ step: entry.step, status: entry.status }))
      }
      if (turnEvent.kind === "turn_completed" && latestPlanEntries !== null) {
        store.append(request.threadRef, {
          key: `${request.turnRef}-plan`,
          role: "system",
          text: "Plan updated",
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
          runtime: { kind: "plan", entries: latestPlanEntries },
        })
      }
      // Session usage ledger feed (#8712 Lane C): exact usage from the typed
      // completion events. Fable turns attribute to the Claude account the
      // turn ran on; delegate children attribute to the Codex account with
      // gpt-5.6-sol recorded as spawn-config truth. A child-observed revoked
      // credential flips the account's typed reconnect flag (probe/child
      // evidence supersedes presence-based "ready").
      if (turnEvent.kind === "turn_completed" && turnEvent.accountRef !== undefined) {
        usageLedger.record({
          provider: "claude_agent",
          accountRef: turnEvent.accountRef,
          requestedModel,
          kind: "turn",
          usage: turnEvent.usage ?? (turnEvent.totalTokens === null
            ? null
            // Split unavailable from this emitter: recorded as total only.
            : {
                inputTokens: 0,
                cachedInputTokens: 0,
                outputTokens: 0,
                reasoningTokens: 0,
                totalTokens: turnEvent.totalTokens,
              }),
        })
      }
      if (turnEvent.kind === "child_completed") {
        usageLedger.record({
          provider: "codex",
          accountRef: turnEvent.accountRef,
          requestedModel: CODEX_CHILD_MODEL,
          kind: "child",
          usage: turnEvent.usage,
        })
      }
      if (turnEvent.kind === "child_activity" &&
        turnEvent.activity === "account_reconnect_required" &&
        turnEvent.accountRef !== undefined) {
        usageLedger.markReconnectRequired({ provider: "codex", accountRef: turnEvent.accountRef })
      }
      if (turnEvent.kind === "child_failed" &&
        turnEvent.reason === "account_reconnect_required" &&
        turnEvent.accountRef !== null) {
        usageLedger.markReconnectRequired({ provider: "codex", accountRef: turnEvent.accountRef })
      }
      // Persist tool trace and effective-model lines so the finalized
      // transcript keeps the same evidence the live stream showed (bounded by
      // the store's note cap).
      if (turnEvent.kind === "tool_use" || turnEvent.kind === "tool_result" ||
        turnEvent.kind === "model_effective") {
        store.append(request.threadRef, {
          key: randomUUID(),
          role: "system",
          text: turnEvent.kind === "model_effective"
            ? fableLocalModelNoteText(turnEvent.model)
            : fableLocalTraceNoteText(turnEvent),
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
          // Typed trace facts (EP250 tool cards): the persisted note carries
          // the same typed payload the live stream note does, so the
          // finalized transcript renders the same typed tool cards.
          ...(turnEvent.kind === "model_effective"
            ? {}
            : { meta: { trace: fableLocalTraceNoteMeta(turnEvent) } }),
        })
      }
      // Smoke-only question-card fixture (EP250 question cards): persist ONE
      // pending interactive question after the fixture Read completes so the
      // built-Electron journey proves the interactive card, the real typed
      // answerQuestion IPC round-trip, and the honest typed-rejection revert
      // (this ref is store-persisted, not runtime-pending, so the runtime
      // answers false). Real question events come from the frozen contract.
      if (smokeMode && turnEvent.kind === "tool_result" && turnEvent.toolName === "Read") {
        store.append(request.threadRef, {
          key: randomUUID(),
          role: "system",
          text: "Which fixture path should this smoke turn take?",
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
          question: {
            turnRef: request.turnRef,
            questionRef: "question.fixture.1",
            status: "pending",
            questions: [{
              question: "Which fixture path should this smoke turn take?",
              header: "Fixture",
              multiSelect: false,
              options: [
                { label: "Streamed", description: "Keep the streamed markdown proof path" },
                { label: "Static" },
              ],
            }],
          },
        })
      }
      if (sender.isDestroyed()) return
      // Attach the persisted thread snapshot (user message included) to the
      // start event so the renderer can stream onto real thread state.
      const event = turnEvent.kind === "turn_started" ? { ...turnEvent, thread: saved } : turnEvent
      sender.send(FableLocalEventChannel, { turnRef: request.turnRef, event })
    },
  })
  if (!result.ok) {
    textPersistence.flush()
    localTurnFlushers.delete(textPersistence.flush)
    if (!(desktopIsQuitting && result.reason === "interrupted")) {
      localTurnJournal.terminal(
        turnKey,
        result.reason === "interrupted" ? "interrupted" : "failed",
        result.reason === "interrupted" ? "owner_interrupted" : "failed",
      )
    }
    return { ok: false, error: fableLocalFailureMessage(result.reason, result.detail) }
  }
  textPersistence.complete(result.text.slice(0, FABLE_LOCAL_FINAL_TEXT_LIMIT))
  localTurnFlushers.delete(textPersistence.flush)
  const finalMeta = {
    lane: "fable-local" as const,
    turnRef: request.turnRef,
    ...(effectiveModel === null ? {} : { model: effectiveModel }),
    ...(result.accountRef === undefined ? {} : { accountRef: result.accountRef }),
    totalTokens: result.totalTokens,
    durationMs: Date.now() - startedAt,
  }
  const assistantKeys = new Set(localTurnJournal.get(turnKey)?.assistantSegments.map(segment => segment.key) ?? [])
  for (const assistant of store.open(request.threadRef)?.notes.filter(note => assistantKeys.has(note.key)) ?? []) {
    store.upsert(request.threadRef, { ...assistant, meta: finalMeta })
  }
  const thread = assistantKeys.size === 0 ? null : store.open(request.threadRef)
  localTurnJournal.terminal(turnKey, "completed", "completed")
  // GIT-1 (#8781): checkpoint the completed turn's workspace state. The
  // completion ref supersedes the start ref for this turn.
  await captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_completed")
  return thread === null
    ? { ok: false, error: "That conversation no longer exists." }
    : { ok: true, thread }
})

// Codex local lane (EP250 codex-first-class): a REAL `codex exec --json`
// turn on this machine in local mode. The ordinary authenticated ~/.codex
// session is preferred; isolated registry homes are fallback capacity. The
// cloud gateway is never used. Availability is
// PROBE-VERIFIED evidence (see codexPreflight above). Events reuse the
// frozen fable-local envelope over the codex-local channels.
// SMOKE sequencing gate: the built-Electron journey must assert BOTH chip
// states deterministically — the disabled-reason popover on the codex chip
// (chrome contract, asserted while the chip still reads "verifying") and
// then the verified-enabled chip + streamed codex turn (codex-first-class
// contract). In smoke, the availability invoke parks until the runner
// releases it right after the fable step's popover assertions. Never active
// in normal runs.
let releaseSmokeCodexAvailability: (() => void) | null = null
const smokeCodexAvailabilityGate: Promise<void> | null = smokeMode
  ? new Promise(resolve => {
      releaseSmokeCodexAvailability = resolve
    })
  : null
ipcMain.handle(CodexLocalAvailabilityChannel, async () => {
  if (smokeCodexAvailabilityGate !== null) await smokeCodexAvailabilityGate
  return codexLocal.availability()
})
ipcMain.handle(CodexLocalInterruptChannel, (_event, value: unknown) => {
  const request = decodeFableLocalInterruptRequest(value)
  return request === null ? false : codexLocal.interrupt(request.turnRef)
})
ipcMain.handle(CodexLocalSteerTurnChannel, async (_event, value: unknown) => {
  const request = decodeFableLocalQueueFollowupRequest(value)
  return request === null
    ? { ok: false, outcome: "not_found" }
    : codexLocal.steerCurrent(request)
})
ipcMain.handle(CodexLocalQueueFollowupChannel, (_event, value: unknown) => {
  const request = decodeFableLocalQueueFollowupRequest(value)
  return request === null
    ? { ok: false, queued: false, reason: "no_active_turn" }
    : codexLocal.queueFollowup(request)
})
ipcMain.handle(CodexLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null || !startRequestHasContent(request)) {
    return { ok: false, error: "That message could not be sent." }
  }
  const requestedModel = request.model ?? request.target?.model ?? CODEX_LOCAL_MODEL
  if ((request.target !== undefined && request.target.provider !== "codex") || !isCodexModel(requestedModel)) {
    return { ok: false, error: "That provider target is not available on the Codex lane." }
  }
  if (request.skill !== undefined) {
    return { ok: false, error: "Local Claude skills are not available on the Codex lane." }
  }
  if (request.permissionMode === "plan_only") {
    return { ok: false, error: "Plan-only permission mode is not available on the Codex lane." }
  }
  const store = threads()
  if (store.open(request.threadRef) === null) return { ok: false, error: "That conversation no longer exists." }
  const turnKey = { threadRef: request.threadRef, turnRef: request.turnRef, lane: "codex-local" as const }
  const user: DesktopMessage = {
    key: `${request.turnRef}-user`,
    role: "user",
    text: userNoteText(request.message, request.images),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  }
  const accepted = localTurnJournal.accept({
    ...turnKey,
    userMessageKey: user.key,
    assistantMessageKey: `${request.turnRef}-assistant-0`,
    accountRef: request.target?.accountRef ?? null,
    model: requestedModel,
  })
  if (!accepted.accepted) return { ok: false, error: "That turn is already accepted." }
  const productSpecAuthority = currentProductSpecWorkroom()
  if (productSpecAuthority !== null) {
    codexHandoffBindings.bindNextTurn({
      workContextRef: productSpecAuthority.workContextRef,
      sessionRef: productSpecAuthority.sessionRef,
      threadRef: request.threadRef,
      turnRef: request.turnRef,
    })
  }
  const saved = store.upsert(request.threadRef, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  const history = saved.notes.filter(note => note.key !== user.key).map(note => ({ role: note.role, text: note.text }))
  const sender = event.sender
  // Message metadata (#8712 pattern): lane, spawn-config-truth model,
  // account ref, turn ref, exact usage total, duration — plus the codex
  // thread id (session-receipt continuity) in requestId.
  const startedAt = Date.now()
  let effectiveModel: string | null = null
  const textPersistence = makeLocalTurnTextPersistence({
    journal: localTurnJournal,
    store,
    key: turnKey,
    meta: () => ({
      lane: "codex-local",
      turnRef: request.turnRef,
      model: effectiveModel ?? codexLocalRequestedModelLabel(requestedModel),
    }),
  })
  localTurnFlushers.add(textPersistence.flush)
  // CUT-11 (#8691): the codex-local root turn joins the same canonical
  // live agent graph contract on its own thread graph.
  liveAgentGraph.beginTurn({ turnRef: request.turnRef, threadRef: request.threadRef, lane: "codex_local" })
  // GIT-1 (#8781): same hidden-ref pre-turn checkpoint as the fable lane.
  await captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_start")
  const result = await codexLocal.runTurn({
    turnRef: request.turnRef,
    threadRef: request.threadRef,
    history,
    message: turnPromptText(request.message, request.images),
    ...(request.target === undefined ? {} : { accountRef: request.target.accountRef }),
    model: requestedModel,
    ...(request.reasoningEffort === undefined ? {} : { reasoningEffort: request.reasoningEffort }),
    ...(request.images !== undefined && request.images.length > 0 ? { images: request.images } : {}),
    emit: turnEvent => {
      // CUT-11 (#8691): same one-callback graph fold as the fable lane.
      liveAgentGraph.applyEvent(request.threadRef, { turnRef: request.turnRef, event: turnEvent })
      if (turnEvent.kind === "model_effective") effectiveModel = turnEvent.model
      if (turnEvent.kind === "text_delta") textPersistence.append(turnEvent.text)
      else textPersistence.boundary()
      // Session usage ledger: exact usage from turn.completed, attributed to
      // the Codex account with the owner-selected model as spawn-config truth.
      if (turnEvent.kind === "turn_completed" && turnEvent.accountRef !== undefined) {
        usageLedger.record({
          provider: "codex",
          accountRef: turnEvent.accountRef,
          requestedModel,
          kind: "turn",
          usage: turnEvent.usage ?? (turnEvent.totalTokens === null
            ? null
            : {
                inputTokens: 0,
                cachedInputTokens: 0,
                outputTokens: 0,
                reasoningTokens: 0,
                totalTokens: turnEvent.totalTokens,
              }),
        })
      }
      // Persist trace/model/reasoning/notice lines so the finalized
      // transcript keeps the same evidence the live stream showed.
      if (turnEvent.kind === "tool_use" || turnEvent.kind === "tool_result") {
        store.append(request.threadRef, {
          key: randomUUID(),
          role: "system",
          text: fableLocalTraceNoteText(turnEvent),
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
          meta: { trace: fableLocalTraceNoteMeta(turnEvent) },
        })
      }
      if (turnEvent.kind === "model_effective" || turnEvent.kind === "reasoning" ||
        turnEvent.kind === "lane_notice") {
        store.append(request.threadRef, {
          key: randomUUID(),
          role: "system",
          text: turnEvent.kind === "model_effective"
            ? codexLocalModelNoteText(turnEvent.model)
            : turnEvent.kind === "reasoning"
              ? `Reasoning · ${turnEvent.text}`
              : turnEvent.text,
          timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
        })
      }
      // Interactive and structured cards are host-owned durable state, not a
      // renderer-only projection. Persist every update before forwarding it so
      // renderer reload, final-thread replacement, and app restart preserve
      // questions, plans, complete nested child transcripts, and queue chips.
      const runtimeOperation = localRuntimePersistenceOperation({
        turnRef: request.turnRef,
        event: turnEvent,
        notes: store.open(request.threadRef)?.notes ?? [],
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      })
      if (runtimeOperation.kind === "upsert") store.upsert(request.threadRef, runtimeOperation.note)
      if (runtimeOperation.kind === "remove") store.remove(request.threadRef, runtimeOperation.key)
      if (sender.isDestroyed()) return
      const forwarded = turnEvent.kind === "turn_started" ? { ...turnEvent, thread: saved } : turnEvent
      sender.send(CodexLocalEventChannel, { turnRef: request.turnRef, event: forwarded })
    },
  })
  if (!result.ok) {
    textPersistence.flush()
    localTurnFlushers.delete(textPersistence.flush)
    if (!(desktopIsQuitting && result.reason === "interrupted")) {
      localTurnJournal.terminal(
        turnKey,
        result.reason === "interrupted" ? "interrupted" : "failed",
        result.reason === "interrupted" ? "owner_interrupted" : "failed",
      )
    }
    return { ok: false, error: codexLocalFailureMessage(result.reason, result.detail) }
  }
  textPersistence.complete(result.text.slice(0, FABLE_LOCAL_FINAL_TEXT_LIMIT))
  localTurnFlushers.delete(textPersistence.flush)
  const finalMeta = {
    lane: "codex-local" as const,
    turnRef: request.turnRef,
    model: effectiveModel ?? codexLocalRequestedModelLabel(requestedModel),
    accountRef: result.accountRef,
    ...(result.threadId === null ? {} : { requestId: result.threadId }),
    totalTokens: result.totalTokens,
    durationMs: Date.now() - startedAt,
  }
  const assistantKeys = new Set(localTurnJournal.get(turnKey)?.assistantSegments.map(segment => segment.key) ?? [])
  for (const assistant of store.open(request.threadRef)?.notes.filter(note => assistantKeys.has(note.key)) ?? []) {
    store.upsert(request.threadRef, { ...assistant, meta: finalMeta })
  }
  const thread = assistantKeys.size === 0 ? null : store.open(request.threadRef)
  localTurnJournal.terminal(turnKey, "completed", "completed")
  // GIT-1 (#8781): completed-turn hidden-ref checkpoint, same as fable lane.
  await captureTurnCheckpoint(request.threadRef, request.turnRef, "turn_completed")
  return thread === null
    ? { ok: false, error: "That conversation no longer exists." }
    : { ok: true, thread }
})

// Codex account connect + reconnect (#8640 unblock; EP250 owner mandate:
// the UI owns reconnect). Channels stay renderer-argument-free except the
// reconnect start, which carries ONE grammar-bounded account ref that the
// service re-validates against the refs it listed itself. Main owns the
// pylon child processes and the verification URL; the renderer polls typed
// status and never sees tokens, emails, or raw output.
ipcMain.handle(CodexAccountsChannel, () => hostLifecycle.account()?.listAccounts() ?? Promise.resolve({ state: "unavailable" }))
ipcMain.handle(CodexConnectStartChannel, () => hostLifecycle.account()?.start() ?? { state: "failed", reason: "pylon_runtime_unavailable" })
ipcMain.handle(CodexReconnectStartChannel, (_event, ref: unknown) =>
  typeof ref === "string"
    ? hostLifecycle.account()?.startReconnect(ref) ?? { state: "failed", reason: "pylon_runtime_unavailable" }
    : { state: "failed", reason: "invalid_account_ref" })
// Reconnect-completion probe trigger (EP250 preflight): the renderer polls
// this status channel; a terminal "connected" state means a credential just
// changed, so the session probe round re-runs (async, non-blocking) and the
// chip/fleet/health projections pick up the fresh validity evidence.
let lastProbedConnectedRef: string | null = null
ipcMain.handle(CodexConnectStatusChannel, async () => {
  const status = await (hostLifecycle.account()?.status() ?? { state: "failed", reason: "pylon_runtime_unavailable" })
  const record = status as { state?: unknown; ref?: unknown }
  if (record.state === "connected" && typeof record.ref === "string" &&
    record.ref !== lastProbedConnectedRef) {
    lastProbedConnectedRef = record.ref
    void codexPreflight.probeAll("reconnect_completed").catch(() => {})
  }
  return status
})
ipcMain.handle(CodexConnectOpenChannel, () => hostLifecycle.account()?.openVerification() ?? Promise.resolve(false))

// Session usage ledger (#8712 Lane C): snapshot on invoke, push on change.
// Renderer-argument-free; the snapshot carries only refs, provider names,
// requested models (spawn-config truth), counts, and token totals.
ipcMain.handle(UsageLedgerSnapshotChannel, () => usageLedger.snapshot())

// CUT-11 (#8691): canonical live agent graph snapshot — renderer-argument
// free; returns the retained encoded openagents.live_agent_graph.v1
// post-images per thread. Updates push over LiveAgentGraphUpdateChannel.
ipcMain.handle(LiveAgentGraphSnapshotChannel, () => liveAgentGraph.snapshot())

// Provider-neutral fleet accounts (#8712): read-only projections. List takes
// no renderer arguments; usage validates the account ref on both sides of the
// boundary. Failures stay typed `{ ok: false, reason }` — never a throw.
ipcMain.handle(ProviderAccountsListChannel, () => {
  // Fleet Refresh doubles as a probe trigger (EP250 preflight): the list
  // spawn returns immediately with presence evidence while the validity
  // probes stream fresh session evidence into health/ledger asynchronously.
  void codexPreflight.probeAll("fleet_refresh").catch(() => {})
  return providerAccounts.listProviderAccounts()
})
ipcMain.handle(FleetRunProjectionListChannel, async () => {
  const credential = desktopSessionVault?.load()
  if (credential === undefined || credential === null) return { state: "unauthorized" }
  return fetchFleetRunClientProjection({
    baseUrl: process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com",
    accessToken: credential.accessToken,
  })
})
ipcMain.handle(ProviderAccountsUsageChannel, (_event, value: unknown) => {
  const request = decodeProviderAccountUsageRequest(value)
  return request === null
    ? unavailableProviderAccountUsageResult("unknown", "invalid_request")
    : providerAccounts.fetchProviderAccountUsage(request.ref)
})

// Deny-by-default for every WebContents: no navigation away from the bundled
// renderer, no window.open, no <webview> attachment.
app.on("web-contents-created", (_event, contents) => {
  contents.on("will-navigate", (event) => {
    event.preventDefault()
  })
  contents.on("will-attach-webview", (event) => {
    event.preventDefault()
  })
  contents.setWindowOpenHandler(() => ({ action: "deny" }))
})

const createWindow = (): BrowserWindow => {
  const window = new BrowserWindow({
    width: 1200,
    height: 800,
    minWidth: 720,
    minHeight: 480,
    // khalaTheme color.background — must match @effect-native/tokens so the
    // pre-boot window never flashes an off-palette frame (EP250 #8712).
    backgroundColor: "#05070d",
    show: false,
    title: "OpenAgents",
    icon: desktopIconPath,
    // Integrate macOS window controls into the product chrome. The renderer
    // reserves a token-sized drag/safe area in the blue sidebar, so there is
    // no separate gray titlebar strip above the application.
    ...(process.platform === "darwin" ? {
      titleBarStyle: "hiddenInset" as const,
      trafficLightPosition: { x: 12, y: 12 },
    } : {}),
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      nodeIntegrationInSubFrames: false,
      sandbox: true,
      webviewTag: false,
      webSecurity: true,
      devTools: !app.isPackaged,
      spellcheck: false,
      backgroundThrottling: false,
      preload: path.join(here, "preload.cjs"),
    },
  })
  primaryDesktopWindow = window
  if (isolatedAppProofMode) hardenSession(window.webContents.session)
  const unsubscribeRuntime = runtimeGateway.subscribe(event => {
    if (!window.isDestroyed()) window.webContents.send(DesktopRuntimeGatewayEventChannel, event)
  })
  const searchOwnerRef = workspaceSearchOwnerRef(window.webContents.id)
  const closeWindowScope = hostLifecycle.registerWindow(`window.${window.id}`, () => {
    workspaceSearchRegistry.closeOwner(searchOwnerRef)
    disableWorkspaceChangeSubscription(window.id)
    unsubscribeRuntime()
  })
  const unsubscribeLedger = usageLedger.subscribe(snapshot => {
    if (!window.isDestroyed()) window.webContents.send(UsageLedgerEventChannel, snapshot)
  })
  window.once("closed", () => {
    if (primaryDesktopWindow === window) primaryDesktopWindow = null
    if (desktopCommandWindow === window) {
      desktopCommandWindow = null
      desktopCommandHost.detach()
    }
    unsubscribeLedger()
    closeWindowScope()
  })
  window.once("ready-to-show", () => {
    // Startup-timing: the first instant the window is presentable (first paint
    // of the pre-boot frame). Recorded before show() so the mark reflects the
    // compositor-ready moment, not post-show work.
    recordMainMark("windowReadyToShow")
    window.show()
  })
  void window.loadURL(desktopRendererEntry)
  return window
}

/**
 * Smoke mode (`bun run smoke`): proves the Effect Native intent loop runs
 * inside the real Electron renderer — types into the catalog-rendered
 * composer, submits, and asserts the message row appended AND the composer
 * cleared (the v29 clear-on-submit contract, effect-native#72). When
 * `OPENAGENTS_DESKTOP_SMOKE_SHOTS` names a directory, it captures pixel
 * receipts (shell / composer-typed / composer-cleared). Exits 0/1.
 */
const smokeShotsDir = process.env.OPENAGENTS_DESKTOP_SMOKE_SHOTS

const smokeWaitForShell = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-root"]') === null) {
    await wait(100)
  }
  return document.querySelector('[data-en-key="shell-root"]') !== null
})()`

const smokeRuntimeGatewayBootstrap = `(async () => {
  const bridge = globalThis.openagentsDesktop
  if (typeof bridge?.runtimeRequest !== "function") return { ok: false, reason: "Runtime Gateway bridge missing" }
  const result = await bridge.runtimeRequest({
    kind: "query",
    requestId: "smoke-runtime-bootstrap",
    query: { id: "runtime.bootstrap" },
  })
  return {
    ok: result?.kind === "query_result" &&
      result.requestId === "smoke-runtime-bootstrap" &&
      result.result?.protocolVersion === ${DesktopRuntimeGatewayProtocolVersion} &&
      result.result?.lifecycle === "ready" &&
      result.result?.capabilities?.some((capability) => capability.id === "codex-history" && capability.state === "available"),
    protocolVersion: result?.result?.protocolVersion,
    lifecycle: result?.result?.lifecycle,
    capabilityCount: result?.result?.capabilities?.length ?? 0,
  }
})()`

const smokeWorkspaceTreeBridge = `(async () => {
  const bridge = globalThis.openagentsDesktop
  if (typeof bridge?.workspaceTree !== "function" ||
      typeof bridge?.workspaceSearch !== "function" ||
      typeof bridge?.cancelWorkspaceSearch !== "function" ||
      typeof bridge?.openWorkspaceDocument !== "function" ||
      typeof bridge?.saveWorkspaceDocument !== "function" ||
      typeof bridge?.saveWorkspaceDocumentAs !== "function" ||
      typeof bridge?.refreshWorkspace !== "function" ||
      typeof bridge?.workspaceSubscribe !== "function") {
    throw new Error("workspace capability bridge unavailable")
  }
  const changes = []
  const unsubscribe = bridge.workspaceSubscribe((change) => changes.push(change))
  try {
    const page = await bridge.workspaceTree({ directoryRef: "", offset: 0, limit: 20 })
    if (page?.state !== "available" || !String(page.grantRef).startsWith("workspace.grant.")) {
      throw new Error("workspace tree unavailable")
    }
    const serialized = JSON.stringify(page)
    if (serialized.includes("tests/fixtures/codex-smoke") ||
        page.entries.some((entry) => String(entry.pathRef).startsWith("/"))) {
      throw new Error("workspace tree leaked its selected root")
    }
    if (await bridge.refreshWorkspace() !== true) throw new Error("workspace refresh unavailable")
    for (let attempt = 0; attempt < 40 && !changes.some((change) => change.kind === "refresh"); attempt++) {
      await new Promise((resolve) => setTimeout(resolve, 25))
    }
    const refresh = changes.find((change) => change.kind === "refresh")
    if (refresh === undefined || refresh.pathRef !== null || refresh.epoch <= page.cache.epoch) {
      throw new Error("workspace refresh event missing")
    }
    const requestRef = "workspace.search.request.smoke"
    const search = await bridge.workspaceSearch({
      requestRef,
      query: "session_index",
      mode: "path",
      offset: 0,
      limit: 20,
    })
    if (search?.requestRef !== requestRef || search?.page?.state !== "available" ||
        search.page.cache.epoch !== refresh.epoch ||
        !search.page.matches.some((match) => match.pathRef === "session_index.jsonl")) {
      throw new Error("workspace search result missing")
    }
    if (JSON.stringify(search).includes("tests/fixtures/codex-smoke") ||
        search.page.matches.some((match) => String(match.pathRef).startsWith("/"))) {
      throw new Error("workspace search leaked its selected root")
    }
    const document = await bridge.openWorkspaceDocument({
      grantRef: page.grantRef,
      pathRef: "session_index.jsonl",
    })
    if (document?.state !== "available" || document.document?.pathRef !== "session_index.jsonl" ||
        document.document?.grantRef !== page.grantRef || document.document?.encoding !== "utf-8") {
      throw new Error("workspace document open missing")
    }
    if (JSON.stringify(document).includes("tests/fixtures/codex-smoke") ||
        String(document.document.pathRef).startsWith("/")) {
      throw new Error("workspace document leaked its selected root")
    }
    const staleSave = await bridge.saveWorkspaceDocument({
      grantRef: page.grantRef,
      pathRef: "session_index.jsonl",
      content: document.document.content,
      expectedRevisionRef: "workspace.document.stale",
    })
    if (staleSave?.state !== "conflict" || staleSave.current?.pathRef !== "session_index.jsonl") {
      throw new Error("workspace document conflict fencing missing")
    }
    const saveAsConflict = await bridge.saveWorkspaceDocumentAs({
      grantRef: page.grantRef,
      pathRef: "session_index.jsonl",
      content: "must not overwrite",
    })
    if (saveAsConflict?.state !== "conflict" || saveAsConflict.current?.content !== document.document.content) {
      throw new Error("workspace Save As overwrite fencing missing")
    }
    const foreignCancel = await bridge.cancelWorkspaceSearch({
      requestRef: "workspace.search.request.foreign",
    })
    if (foreignCancel?.requestRef !== "workspace.search.request.foreign" || foreignCancel.cancelled !== false) {
      throw new Error("workspace search cancel fencing failed")
    }
    return {
      ok: true,
      entryCount: page.entries.length,
      matchCount: search.page.matches.length,
      documentLanguage: document.document.languageMode,
      staleSave: staleSave.state,
      saveAsConflict: saveAsConflict.state,
      epoch: refresh.epoch,
      foreignCancel: foreignCancel.cancelled,
    }
  } finally {
    unsubscribe()
  }
})()`

const smokeWorkspaceFilesUi = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const codingCatalog = await globalThis.openagentsDesktop?.codingCatalog?.snapshot?.()
  // UX-4 (#8790): Files has NO dock icon — the workspace is reached through
  // its closed CW-AC-12 command identity (enqueued by the smoke host before
  // this probe runs).
  const filesDockItem = document.querySelector('[data-en-key="workspace-files"]')
  if (filesDockItem !== null) return { ok: false, reason: "swept Files dock icon rendered" }
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && document.querySelector('[data-en-key="workspace-browser-tree-list"]') === null) {
    await wait(50)
  }
  const browser = document.querySelector('[data-en-key="workspace-browser"]')
  const tree = document.querySelector('[data-en-key="workspace-browser-tree-list"]')
  const search = document.querySelector('[data-en-key="workspace-browser-query"] input')
  const boundary = document.querySelector('[data-en-key="workspace-browser-boundary"]')
  const legacyEditor = document.querySelector('[data-en-key="workspace-file-editor"]')
  const file = document.querySelector('[data-en-key="workspace-browser-select-session_index.jsonl"]')
  file?.click()
  while (Date.now() < deadline && document.querySelector('[data-en-key="workspace-editor-host-session_index.jsonl"] textarea') === null) {
    await wait(50)
  }
  const editor = document.querySelector('[data-en-key="workspace-editor-host-session_index.jsonl"] textarea')
  if (editor !== null) {
    editor.value = editor.value + "\\nrecovery-smoke-draft"
    editor.dispatchEvent(new Event("input", { bubbles: true }))
    await wait(300)
  }
  const saveAs = document.querySelector('[data-en-key="workspace-editor-save-as"]')
  saveAs?.click()
  await wait(50)
  const saveAsPath = document.querySelector('[data-en-key="workspace-editor-save-as-path"] input')
  document.querySelector('[data-en-key="workspace-editor-save-as-cancel"]')?.click()
  await wait(300)
  const recoveryKey = typeof codingCatalog?.selectedSessionRef === "string"
    ? "openagents.desktop.workspace-editor.v2." + codingCatalog.selectedSessionRef
    : undefined
  let recoveryStored = false
  let recoveryTabCount = -1
  let recoveryDraftHasMarker = false
  let recoveryDraftTail = ""
  if (recoveryKey !== undefined) {
    try {
      const stored = JSON.parse(localStorage.getItem(recoveryKey) ?? "null")
      recoveryTabCount = Array.isArray(stored?.tabs) ? stored.tabs.length : -1
      recoveryDraftHasMarker = stored?.tabs?.some((tab) => tab.pathRef === "session_index.jsonl" && tab.draft.includes("recovery-smoke-draft")) === true
      recoveryDraftTail = typeof stored?.tabs?.[0]?.draft === "string" ? stored.tabs[0].draft.slice(-32) : ""
      recoveryStored = stored?.version === 2 && recoveryDraftHasMarker
    } catch {}
  }
  const leakedRoot = document.body.textContent?.includes("tests/fixtures/codex-smoke") === true
  // UX-4 (#8790): the browser renders no filesystem mutation affordance.
  const mutationAffordance = ["workspace-browser-new-file", "workspace-browser-new-folder", "workspace-browser-rename", "workspace-browser-delete", "workspace-browser-reveal"]
    .find((key) => document.querySelector('[data-en-key="' + key + '"]') !== null) ?? null
  const chat = document.querySelector('[data-en-key="workspace-chat"]')
  return {
    ok: browser !== null && tree !== null && search !== null && boundary !== null &&
      legacyEditor === null && editor !== null && saveAsPath !== null && recoveryStored && !leakedRoot && chat !== null &&
      mutationAffordance === null,
    mutationAffordance,
    relativeBoundary: boundary?.textContent,
    legacyEditor: legacyEditor !== null,
    editorHost: editor !== null,
    saveAsForm: saveAsPath !== null,
    recoveryStored,
    recoveryKeyPresent: recoveryKey !== undefined,
    recoveryTabCount,
    recoveryDraftHasMarker,
    recoveryDraftTail,
    editorHasMarker: editor?.value?.includes("recovery-smoke-draft") === true,
    catalogSessions: codingCatalog?.sessions?.length ?? -1,
    catalogSelected: codingCatalog?.selectedSessionRef ?? null,
    leakedRoot,
  }
})()`

const smokeWorkspaceEditorRecovery = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const codingCatalog = await globalThis.openagentsDesktop?.codingCatalog?.snapshot?.()
  const deadline = Date.now() + 10000
  // UX-4 (#8790): no Files dock icon — after the reload mounts, re-enter the
  // Files workspace exactly like a keyboard user: canonical ⌘K palette chord,
  // then the closed "Open Files" command row (CW-AC-12 identity).
  while (Date.now() < deadline && document.querySelector('[data-en-key="workspace-chat"]') === null) {
    await wait(50)
  }
  if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  window.dispatchEvent(new KeyboardEvent("keydown", {
    key: "k",
    metaKey: ${JSON.stringify(process.platform === "darwin")},
    ctrlKey: ${JSON.stringify(process.platform !== "darwin")},
    bubbles: true,
    cancelable: true,
  }))
  while (Date.now() < deadline && document.querySelector('[data-en-key="desktop-command-workspace.files"]') === null) {
    await wait(50)
  }
  document.querySelector('[data-en-key="desktop-command-workspace.files"]')?.click()
  while (Date.now() < deadline && document.querySelector('[data-en-key="workspace-editor-host-session_index.jsonl"] textarea') === null) {
    await wait(50)
  }
  const editor = document.querySelector('[data-en-key="workspace-editor-host-session_index.jsonl"] textarea')
  const recovered = editor?.value?.includes("recovery-smoke-draft") === true
  const recoveryKey = typeof codingCatalog?.selectedSessionRef === "string"
    ? "openagents.desktop.workspace-editor.v2." + codingCatalog.selectedSessionRef
    : undefined
  document.querySelector('[data-en-key="workspace-editor-close"]')?.click()
  await wait(50)
  document.querySelector('[data-en-key="workspace-editor-close"]')?.click()
  document.querySelector('[data-en-key="workspace-chat"]')?.click()
  return {
    ok: recovered,
    recovered,
    stored: recoveryKey !== undefined && localStorage.getItem(recoveryKey) !== null,
    editor: editor !== null,
    catalogSelected: codingCatalog?.selectedSessionRef ?? null,
  }
})()`

const smokeLifecycleCorrelation = `(async () => {
  const bridge = globalThis.openagentsDesktop
  if (typeof bridge?.runtimeRequest !== "function") return { ok: false, reason: "Runtime Gateway bridge missing" }
  const bootstrap = await bridge.runtimeRequest({
    kind: "query",
    requestId: "smoke-correlation-bootstrap",
    query: { id: "runtime.bootstrap" },
  })
  const sessionRef = bootstrap?.context?.sessionRef
  if (typeof sessionRef !== "string") return { ok: false, reason: "Session correlation missing" }
  const context = {
    operationRef: "operation.desktop.smoke.start",
    sessionRef,
    correlationRef: "correlation.desktop.smoke",
    runRef: "run.desktop.smoke",
  }
  const response = await bridge.runtimeRequest({
    kind: "command",
    commandId: context.operationRef,
    context,
    command: {
      id: "conversation.start",
      threadRef: "thread.desktop.smoke",
      messageRef: "message.desktop.smoke",
      runRef: context.runRef,
    },
  })
  return {
    ok: response?.kind === "runtime_command_outcome" &&
      response.status === "unknown_pending_reconcile" &&
      response.context?.operationRef === context.operationRef &&
      response.context?.sessionRef === context.sessionRef &&
      response.context?.correlationRef === context.correlationRef &&
      response.context?.runRef === context.runRef,
  }
})()`

const smokeTypeIntoComposer = `(() => {
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Pixel-proof: real chat rows on the shared catalog"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  return { ok: true, typed: input.value }
})()`

const smokeCodexHistoryDetails = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const first = document.querySelector('[data-en-key^="sidebar-thread-"]')
  if (first === null) return { ok: false, reason: "history row never mounted" }
  first.click()
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-key="history-workspace-split"]') === null) {
    await wait(100)
  }
  const sidebar = document.querySelector('[data-en-key="sidebar-history-list"] > [data-en-role="section-label"]')
  const detail = document.querySelector('[data-en-key="history-workspace-split"]')
  // #8789: the header states the projection's REAL scope with a counted
  // disclosure. The smoke fixture catalog holds exactly one root session, all
  // of it shown — so the truthful header reads "all 1", never "all time".
  return { ok: sidebar?.textContent === "Coding history · all 1" && detail !== null, header: sidebar?.textContent ?? null }
})()`

// #8787 (owner verbatim: "the text input should be focused immediately on
// open. so i can start typing right away."): at shell-interactable the
// composer already holds keyboard focus, and it STILL holds it after the
// background history hydration settles (90bce8d89b boot order) — hydration
// must never steal open-time focus. No pointer event happens before this step.
const smokeComposerFocusedOnOpen = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const composer = () => document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && (composer() === null || document.activeElement !== composer())) await wait(50)
  const focusedAtMount = composer() !== null && document.activeElement === composer()
  const marks = () => globalThis.__oaStartupMarks || {}
  const hydrateDeadline = Date.now() + 30000
  while (Date.now() < hydrateDeadline && typeof marks().historyHydrated !== "number") await wait(50)
  await wait(200)
  const focusedAfterHydration = document.activeElement === composer()
  return { ok: focusedAtMount && focusedAfterHydration, focusedAtMount, focusedAfterHydration }
})()`

// #8787 second oracle: a real Chromium keyboard event (sent from the main
// process, no prior pointer event) lands in the composer as typed text.
const smokeFirstKeystrokeLandsInComposer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const composer = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (composer === null) return { ok: false, reason: "composer input never mounted" }
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && composer.value !== "k") await wait(50)
  const typed = composer.value
  // Leave a pristine composer for the later steps.
  composer.value = ""
  composer.dispatchEvent(new Event("input", { bubbles: true }))
  return { ok: typed === "k", typed }
})()`

// #8788 (owner verbatim: "The search doesn't seem to fucking work at all.
// One of the chats is titled Assurance, but when I start typing in the first
// few letters there, it does not show it."): typing a prefix of a visible
// session title into the sidebar search filters the list to that session.
const smokeSessionSearchFilters = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const field = document.querySelector('[data-en-key="history-search-field"] input, [data-en-key="history-search-field"] textarea')
  if (field === null) return { ok: false, reason: "session search field never mounted" }
  field.focus()
  field.value = "cut-02 verif"
  field.dispatchEvent(new Event("input", { bubbles: true }))
  const deadline = Date.now() + 15000
  const row = () => document.querySelector('[data-en-key="sidebar-search-smoke-root"]')
  while (Date.now() < deadline && row() === null) await wait(50)
  return { ok: row() !== null, matched: row()?.textContent ?? null }
})()`

// #8788 continued: a no-match query shows the explicit empty state (after the
// bounded index settles — never a false "no match" while searching), and
// clearing the query restores the full session list.
const smokeSessionSearchNoMatchAndClear = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const field = document.querySelector('[data-en-key="history-search-field"] input, [data-en-key="history-search-field"] textarea')
  if (field === null) return { ok: false, reason: "session search field never mounted" }
  const type = (value) => { field.value = value; field.dispatchEvent(new Event("input", { bubbles: true })) }
  type("zz-no-such-session-zz")
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-key="sidebar-search-empty"]') === null) await wait(50)
  const emptyState = document.querySelector('[data-en-key="sidebar-search-empty"]') !== null
  const noRows = document.querySelector('[data-en-key="sidebar-search-smoke-root"]') === null
  type("")
  while (Date.now() < deadline && document.querySelector('[data-en-key^="sidebar-thread-"]') === null) await wait(50)
  const restored = document.querySelector('[data-en-key^="sidebar-thread-"]') !== null &&
    document.querySelector('[data-en-key="sidebar-search-empty"]') === null
  return { ok: emptyState && noRows && restored, emptyState, noRows, restored }
})()`

const smokeWaitForHostCommandPalette = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && document.querySelector('[data-en-key="desktop-command-palette"]') === null) {
    await wait(50)
  }
  const palette = document.querySelector('[data-en-key="desktop-command-palette"]')
  const files = document.querySelector('[data-en-key="desktop-command-workspace.files"]')
  // Let the 350ms overlay enter animation finish so the pixel receipt
  // shows the settled panel, not a mid-fade frame.
  await wait(450)
  return { ok: palette !== null && files !== null }
})()`

const smokeCloseCommandPalette = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="desktop-command-palette-close"]')
  if (button === null) return { ok: false, reason: "Command palette close button never mounted" }
  button.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && document.querySelector('[data-en-key="desktop-command-palette"]') !== null) {
    await wait(50)
  }
  return { ok: document.querySelector('[data-en-key="desktop-command-palette"]') === null }
})()`

const smokeOpenFleetDesk = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="shell-fleet-toggle"]')
  if (button === null) return { ok: false, reason: "Fleet toggle never mounted" }
  button.click()
  const deadline = Date.now() + 20_000
  while (Date.now() < deadline && document.querySelector('[data-en-key="fleet-desk"]') === null) {
    await wait(50)
  }
  const objective = document.querySelector('[data-en-key="fleet-objective"] input')
  const dispatch = document.querySelector('[data-en-key="fleet-stage-request"]')
  const status = document.querySelector('[data-en-key="fleet-authority-status"]')
  return {
    ok: objective !== null && dispatch !== null && status !== null && status.textContent === "Draft",
    status: status === null ? null : status.textContent,
  }
})()`

// Git/GitHub review panel (EP250 E2–E5, #8712): the review workspace mounts
// the typed Git panel, which renders REAL read-only status of the app's own
// repo (the git-github host points at the bundle's repo in smoke). Fixture-safe:
// this step neither commits nor pushes — it asserts the status header, the
// commit box, the Push control, the branch switcher, and the issues/PRs
// section all render, and that the panel resolved real status.
const smokeOpenGitReview = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const q = (key) => document.querySelector('[data-en-key="' + key + '"]')
  // Wait for the panel to mount AND its async status refresh to resolve: the
  // branch label leaves the placeholder "—" for a real branch / "detached
  // HEAD", or the explicit git-unavailable message appears.
  const deadline = Date.now() + 20000
  const branchText = () => (q("git-status-branch")?.textContent ?? "").trim()
  const resolved = () => (branchText() !== "" && branchText() !== "—") || q("git-unavailable") !== null
  while (Date.now() < deadline && !(q("git-panel") !== null && resolved())) {
    await wait(100)
  }
  const panel = q("git-panel")
  const header = q("git-status-header")
  // UX-4 (#8790): the CW-AC-14 read-only review boundary — NO Git mutation
  // affordance may render (commit, push, stage/unstage, discard, branches,
  // issue/PR authoring).
  const mutationAffordance = ["git-commit-message", "git-commit", "git-push", "git-branches", "git-issues-prs"]
    .find((key) => q(key) !== null)
    ?? (document.querySelector('[data-en-key^="git-stage-toggle-"]') !== null ? "git-stage-toggle" : null)
    ?? (document.querySelector('[data-en-key^="git-discard-"]') !== null ? "git-discard" : null)
  const statusResolved = resolved()
  const branch = branchText().slice(0, 80)
  const review = document.querySelector('[data-en-key^="git-review-u-"]')
  review?.click()
  while (Date.now() < deadline && q("git-review-diff-view") === null && q("git-action-error") === null) {
    await wait(50)
  }
  const diff = q("git-review-diff-view")
  return {
    ok: panel !== null && header !== null && mutationAffordance === null && statusResolved &&
      review !== null && diff !== null,
    branch,
    statusResolved,
    mutationAffordance,
    diff: diff !== null,
  }
})()`

// Attach the reviewed diff to the composer (the CW-AC-14 review -> composer
// seam) — runs AFTER the panel pixel receipt so the shot shows the panel.
const smokeGitReviewAttach = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const q = (key) => document.querySelector('[data-en-key="' + key + '"]')
  const deadline = Date.now() + 10000
  q("git-review-attach")?.click()
  while (Date.now() < deadline && q("shell-composer-review-context") === null) await wait(50)
  const composerContext = q("shell-composer-review-context")
  q("shell-composer-review-remove")?.click()
  return { ok: composerContext !== null, composerContext: composerContext !== null }
})()`

// Workspace-bounded PTY terminal (CUT-20, #8700): the terminal workspace mounts
// (routed by the canonical workspace.terminal command), then a REAL PTY host
// session runs a bounded command through the real preload bridge + real main
// host (bound to the app's own repo in smoke), captures its redacted output,
// and disposes the owned process tree. Built-Electron proof of the D3 seam.
const smokeTerminalWorkspace = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const terminal = window.openagentsDesktop && window.openagentsDesktop.terminal
  if (!terminal || typeof terminal.create !== "function") return { ok: false, reason: "terminal bridge missing" }
  const panelDeadline = Date.now() + 10000
  while (Date.now() < panelDeadline && document.querySelector('[data-en-key="workspace-terminal-panel"]') === null) {
    await wait(50)
  }
  if (document.querySelector('[data-en-key="workspace-terminal-panel"]') === null) {
    return { ok: false, reason: "terminal panel never mounted" }
  }
  let sawReady = false, sawOutput = false, sawClosed = false
  const kinds = []
  const off = terminal.onEvent((event) => {
    kinds.push(event.kind)
    if (event.kind === "ready") sawReady = true
    if (event.kind === "output" && String(event.chunk || "").indexOf("smoke-terminal-echo") !== -1) sawOutput = true
    if (event.kind === "closed") sawClosed = true
  })
  const created = await terminal.create({})
  if (!created || created.ok !== true) { off(); return { ok: false, reason: "create", created } }
  const sessionRef = created.sessionRef
  await terminal.input({ sessionRef, data: "echo smoke-terminal-echo\\n" })
  const outputDeadline = Date.now() + 10000
  while (Date.now() < outputDeadline && !sawOutput) await wait(50)
  const closed = await terminal.close({ sessionRef })
  await wait(250)
  off()
  return {
    ok: sawReady && sawOutput && !!closed && closed.ok === true,
    sawReady, sawOutput, sawClosed,
    kinds: kinds.slice(0, 12),
  }
})()`

const smokeSubmitComposer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  const messageCount = () =>
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  const messagesBefore = messageCount()
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && (messageCount() < messagesBefore + 2 || input.value !== "")) {
    await wait(50)
  }
  const userRow = document.querySelector(
    '[data-en-key="shell-transcript"] [data-en-message][data-en-role="user"]'
  )
  const responseRow = Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"], [data-en-key="shell-transcript"] [data-en-message][data-en-role="system"]')
  ).at(-1)
  const sender = userRow === null ? null : userRow.querySelector('[data-en-role="sender"]')
  const body = userRow === null ? null : userRow.querySelector('[data-en-role="body"]')
  const responseSender = responseRow === undefined ? null : responseRow.querySelector('[data-en-role="sender"]')
  // EP250 (#8712): assistant rows carry NO sender label; system rows keep SYSTEM.
  const responseSenderOk = responseRow !== undefined && (
    responseRow.getAttribute("data-en-role") === "assistant"
      ? responseSender === null
      : responseSender !== null && responseSender.textContent === "SYSTEM"
  )
  return {
    ok:
      messageCount() === messagesBefore + 2 &&
      input.value === "" &&
      sender !== null && sender.textContent === "YOU" &&
      body !== null && body.textContent.includes("Pixel-proof") &&
      !body.textContent.includes("YOU") &&
      responseSenderOk,
    messagesBefore,
    messagesAfter: messageCount(),
    inputAfterSubmit: input.value,
    senderChip: sender === null ? null : sender.textContent,
    responseSenderChip: responseSender === null ? null : responseSender.textContent,
  }
})()`

const smokePingLoop = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="shell-ping"]')
  if (button === null) return { ok: false, reason: "ping button never mounted" }
  const badgeText = () => {
    const badge = document.querySelector('[data-en-key="shell-ping-count"]')
    return badge === null ? null : badge.textContent
  }
  const before = badgeText()
  const messageCount = () =>
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  const notesBefore = messageCount()
  button.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && badgeText() === before) {
    await wait(50)
  }
  const after = badgeText()
  return {
    ok: after !== before && after !== null && after.includes("1") && messageCount() === notesBefore + 1,
    before,
    after,
    notesBefore,
    notesAfter: messageCount(),
  }
})()`

const smokeOpenSettings = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="shell-settings-toggle"]')
  if (button === null) return { ok: false, reason: "Settings toggle never mounted" }
  button.click()
  const deadline = Date.now() + 5000
  while (
    Date.now() < deadline &&
    document.querySelector('[data-en-key="settings-codex-session-copy"]') === null
  ) {
    await wait(50)
  }
  const session = document.querySelector('[data-en-key="settings-codex-session-copy"]')
  const connect = document.querySelector('[data-en-key="settings-connect-codex"]')
  const accountRow = document.querySelector('[data-en-key^="settings-account-"]')
  const openAgentsLink = document.querySelector('[data-en-key="settings-openagents-session-action"]')
  const mcp = document.querySelector('[data-en-key="settings-mcp-title"]')
  const plugins = document.querySelector('[data-en-key="settings-plugins-title"]')
  const screen = document.querySelector('[data-en-key="settings-screen"]')
  const outOfScopeCopy = screen !== null && /pylon|connect codex|device-auth|openagents account|mcp server|claude plugin|fleet/i.test(screen.textContent ?? "")
  return {
    ok: session !== null && /already signed in on this Mac/.test(session.textContent ?? "") &&
      connect === null && accountRow === null && openAgentsLink === null && mcp === null && plugins === null && !outOfScopeCopy,
    currentSessionCopy: session === null ? null : session.textContent,
    connectPresent: connect !== null,
    accountRowPresent: accountRow !== null,
    openAgentsLinkPresent: openAgentsLink !== null,
    mcpPresent: mcp !== null,
    pluginsPresent: plugins !== null,
    outOfScopeCopy,
  }
})()`

// MAINT-1 (#8785): with Settings open, the harness maintenance section must
// leave its loading state (live detection through the typed gateway) and
// render per-harness rows (or the honest unavailable state) plus the update
// affordance for any updatable harness.
const smokeSettingsHarnessMaintenance = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 30000
  while (
    Date.now() < deadline &&
    document.querySelector('[data-en-key="settings-harness-maintenance-loading"]') !== null
  ) {
    await wait(100)
  }
  const title = document.querySelector('[data-en-key="settings-harness-maintenance-title"]')
  const loading = document.querySelector('[data-en-key="settings-harness-maintenance-loading"]')
  const unavailable = document.querySelector('[data-en-key="settings-harness-maintenance-unavailable"]')
  const versions = [...document.querySelectorAll('[data-en-key^="settings-harness-"]')]
    .filter((node) => (node.getAttribute("data-en-key") ?? "").endsWith("-version"))
    .map((node) => ({ key: node.getAttribute("data-en-key"), text: node.textContent }))
  const updateButtons = [...document.querySelectorAll('[data-en-key^="settings-harness-"]')]
    .filter((node) => (node.getAttribute("data-en-key") ?? "").endsWith("-update"))
    .map((node) => node.getAttribute("data-en-key"))
  return {
    ok: title !== null && loading === null && (versions.length > 0 || unavailable !== null),
    loadingStuck: loading !== null,
    unavailablePresent: unavailable !== null,
    versions,
    updateButtons,
  }
})()`

// UX-4 (#8790) pixel receipt for the return-to-chat hop.
const smokeBackToChat = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const chat = document.querySelector('[data-en-key="workspace-chat"]')
  if (chat === null) return { ok: false, reason: "Chat dock item missing" }
  chat.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-transcript"]') === null) {
    await wait(50)
  }
  return { ok: document.querySelector('[data-en-key="shell-transcript"]') !== null }
})()`

const smokeMvpSurfaceAllowlist = `(() => {
  const forbidden = [
    "workspace-fleet",
    "sidebar-accounts-box",
    "shell-attach-image",
    "shell-harness-select",
    "shell-model-select",
    "shell-reasoning-select",
    "shell-voice-toggle",
    // UX-4 (#8790): swept dock affordances and Git mutation controls.
    "workspace-files",
    "workspace-product-spec",
    "workspace-assurance-spec",
    "product-spec-workspace",
    "assurance-spec-document",
    "assurance-spec-invalid",
    "shell-command-palette-toggle",
    "git-commit",
    "git-push",
    "git-branches",
    "git-issues-prs",
  ]
  const present = forbidden.filter((key) => document.querySelector('[data-en-key="' + key + '"]') !== null)
  // UX-4 (#8790): the rendered dock is EXACTLY the MVP allowlist, in order.
  const dockIds = Array.from(document.querySelectorAll('[data-en-key="sidebar-workspace-dock"] > button[data-en-key]'))
    .map((item) => item.getAttribute("data-en-key"))
  const expectedDock = ["workspace-new-chat", "workspace-chat", "workspace-home", "shell-settings-toggle"]
  const dockExact = JSON.stringify(dockIds) === JSON.stringify(expectedDock)
  const codex = document.querySelector('[data-en-key="shell-codex-engine"]')
  return { ok: present.length === 0 && dockExact && codex?.textContent === "Codex", present, dockIds, dockExact, codex: codex?.textContent ?? null }
})()`

// Behavior contract openagents_desktop.settings.mcp_servers.v1 (I2, EP250
// wave-2): the built-Electron settings screen renders the MCP servers section,
// and adding a fixture stdio server through the REAL Add form + typed IPC
// persists it to the real userData store and lists it. No MCP server is ever
// spawned — the fable query is a fixture in smoke.
const smokeMcpAddServer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const title = document.querySelector('[data-en-key="settings-mcp-title"]')
  if (title === null) return { ok: false, reason: "MCP servers section never mounted" }
  const fieldInput = (key) => {
    const host = document.querySelector('[data-en-key="' + key + '"]')
    if (host === null) return null
    if (host.localName === "input" || host.localName === "textarea") return host
    return host.querySelector("input, textarea")
  }
  const setField = (key, value) => {
    const input = fieldInput(key)
    if (input === null) return false
    input.value = value
    input.dispatchEvent(new Event("input", { bubbles: true }))
    return true
  }
  if (!setField("settings-mcp-field-name", "smoke-docs")) return { ok: false, reason: "name field missing" }
  await wait(30)
  if (!setField("settings-mcp-field-command", "docs-mcp")) return { ok: false, reason: "command field missing" }
  await wait(30)
  const add = document.querySelector('[data-en-key="settings-mcp-add"]')
  if (add === null) return { ok: false, reason: "Add button missing" }
  add.click()
  const deadline = Date.now() + 5000
  while (
    Date.now() < deadline &&
    document.querySelector('[data-en-key="settings-mcp-server-smoke-docs-name"]') === null
  ) {
    await wait(50)
  }
  const row = document.querySelector('[data-en-key="settings-mcp-server-smoke-docs-name"]')
  const transport = document.querySelector('[data-en-key="settings-mcp-server-smoke-docs-transport"]')
  const toggle = document.querySelector('[data-en-key="settings-mcp-server-smoke-docs-toggle"]')
  // The draft resets on success (name field cleared).
  const nameAfter = fieldInput("settings-mcp-field-name")
  return {
    ok: row !== null && row.textContent === "smoke-docs" &&
      transport !== null && transport.textContent === "stdio" &&
      toggle !== null &&
      nameAfter !== null && nameAfter.value === "",
    listed: row !== null,
    transport: transport === null ? null : transport.textContent,
  }
})()`

// CUT-24 (#8704): the diagnostics/watchdog panel renders live health rows, its
// redacted export produces a public-safe notice (no path/secret), and the
// durable preferences IPC round-trips (update → read → reset).
const smokeDiagnosticsAndPreferences = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const screen = document.querySelector('[data-en-key="diagnostics-screen"]')
  if (screen === null) return { ok: false, reason: "diagnostics panel never mounted" }
  // The boot gather populates rows; wait for the six domain rows to render.
  const domains = ["provider", "runtimeGateway", "sync", "workspace", "pty", "extensions"]
  const deadline = Date.now() + 6000
  while (Date.now() < deadline && document.querySelector('[data-en-key="diagnostics-row-provider"]') === null) await wait(50)
  const rows = domains.map((d) => document.querySelector('[data-en-key="diagnostics-row-' + d + '"]'))
  const rowsRendered = rows.every((row) => row !== null)
  const levels = domains.map((d) => {
    const badge = document.querySelector('[data-en-key="diagnostics-row-' + d + '-level"]')
    return badge === null ? null : badge.textContent
  })
  // No rendered diagnostics text may carry a path, url, or token-like blob.
  const texts = Array.from(screen.querySelectorAll('*')).map((el) => el.textContent || "")
  const secretLike = texts.some((t) => /(?:^|[\\s=(])[~.]*\\/[\\w.]|:\\/\\/|Bearer|sk-/.test(t))
  // Redacted export → public-safe notice (never a saved path).
  const exportBtn = document.querySelector('[data-en-key="diagnostics-export"]')
  if (exportBtn === null) return { ok: false, reason: "export button missing" }
  exportBtn.click()
  const noticeDeadline = Date.now() + 5000
  while (Date.now() < noticeDeadline && document.querySelector('[data-en-key="diagnostics-notice"]') === null) await wait(50)
  const notice = document.querySelector('[data-en-key="diagnostics-notice"]')
  const noticeText = notice === null ? "" : (notice.textContent || "")
  const noticeSafe = notice !== null && !/(?:^|[\\s=(])[~.]*\\/[\\w.]|:\\/\\//.test(noticeText)
  // Preferences durable IPC round-trip: update → read → reset.
  const bridge = window.openagentsDesktop
  let prefRoundTrip = false
  if (bridge && bridge.preferences) {
    await bridge.preferences.update({ appearance: { density: "compact" } })
    const afterUpdate = await bridge.preferences.get()
    const reset = await bridge.preferences.reset()
    prefRoundTrip = afterUpdate && afterUpdate.appearance && afterUpdate.appearance.density === "compact" &&
      reset && reset.appearance && reset.appearance.density === "comfortable"
  }
  return {
    ok: rowsRendered && !secretLike && noticeSafe && prefRoundTrip,
    rowsRendered,
    levels,
    secretLike,
    noticeSafe,
    prefRoundTrip,
  }
})()`

const smokeCloseSettings = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="settings-back"]')
  if (button === null) return { ok: false, reason: "Settings back button never mounted" }
  button.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-main"]') === null) {
    await wait(50)
  }
  return { ok: document.querySelector('[data-en-key="shell-main"]') !== null }
})()`

const smokeWaitForSecondInstanceSettings = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 8000
  while (Date.now() < deadline && document.querySelector('[data-en-key="desktop-command-bindings"]') === null) {
    await wait(50)
  }
  return {
    ok: document.querySelector('[data-en-key="settings-screen"]') !== null &&
      document.querySelector('[data-en-key="desktop-command-bindings"]') !== null,
  }
})()`

// Regression guard (#8712 polish): New chat from a LOADED Codex history page
// must land in a fresh empty transcript, never the historical conversation.
const smokeNewChatFromHistory = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  if (document.querySelector('[data-en-key="history-workspace-split"]') === null) {
    return { ok: false, reason: "history detail was not loaded before New chat" }
  }
  const button = document.querySelector('[data-en-key="workspace-new-chat"]')
  if (button === null) return { ok: false, reason: "New chat dock button never mounted" }
  button.click()
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && (
    document.querySelector('[data-en-key="shell-transcript"]') === null ||
    document.querySelector('[data-en-key="history-workspace-split"]') !== null
  )) {
    await wait(50)
  }
  const transcript = document.querySelector('[data-en-key="shell-transcript"]')
  const split = document.querySelector('[data-en-key="history-workspace-split"]')
  const messages = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  const composer = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  // Owner contract (EP250): "when i do new chat, clicking button or command
  // N, auto focus the input." The focus retry loop lands AFTER the chat
  // view mounts, so poll for it.
  const focusDeadline = Date.now() + 3000
  while (Date.now() < focusDeadline && document.activeElement !== composer) {
    await wait(50)
  }
  return {
    ok: transcript !== null && split === null && messages === 0 &&
      composer !== null && composer.disabled === false &&
      document.activeElement === composer,
    historyStillLoaded: split !== null,
    messages,
    composerMounted: composer !== null,
    composerFocused: document.activeElement === composer,
  }
})()`

// Composer gestures (EP250 owner statements): "i want shift+tab to togle
// between modes in composer (fable / codex) in this case" and "airplane icon
// in composer OUTSIDE of the button is stupid. put it in , remove text
// 'send'". From the fresh chat: the send control is ONE icon-only button
// (plane INSIDE, no "Send" text anywhere in the composer); with the composer
// focused, Shift+Tab toggles the selected harness BOTH directions with
// preventDefault (dispatchEvent returns false), including onto the
// currently-disabled codex lane (capability truth lives on the chip/Send).
const smokeComposerGestures = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  // Icon-only send control: IconButton variant, plane glyph inside, aria
  // label present, no freestanding icon, no visible "Send" text.
  const send = document.querySelector('[data-en-key="shell-note"]')
  if (send === null) return { ok: false, reason: "send control never mounted" }
  if (send.getAttribute("data-en-variant") !== "icon") {
    return { ok: false, reason: "send control is not the icon-only variant" }
  }
  if (send.querySelector("svg") === null) return { ok: false, reason: "send control has no glyph inside" }
  if ((send.getAttribute("aria-label") || "") === "") return { ok: false, reason: "send control lost its aria-label" }
  const sendControls = document.querySelectorAll('[data-en-key="shell-note"]')
  if (sendControls.length !== 1) return { ok: false, reason: "composer renders more than one send control" }
  const composer = document.querySelector('[data-en-key="shell-composer"]')
  const visibleComposerText = composer === null ? "" : (() => {
    const clone = composer.cloneNode(true)
    for (const bubble of clone.querySelectorAll('[data-en-role="tooltip"]')) bubble.remove()
    return clone.textContent ?? ""
  })()
  if (visibleComposerText.includes("Send")) {
    return { ok: false, reason: "composer still renders Send text" }
  }
  if (document.querySelector('[data-en-key="shell-send-icon"]') !== null) {
    return { ok: false, reason: "freestanding send icon still rendered outside the button" }
  }
  // Shift+Tab toggle, scoped to the focused composer input.
  const selectedHarness = () => {
    const select = document.querySelector('[data-en-key="shell-harness-select"]')
    if (select instanceof HTMLSelectElement) return select.value
    const fable = document.querySelector('[data-en-key="shell-harness-fable"]')
    const codex = document.querySelector('[data-en-key="shell-harness-codex"]')
    if (fable?.getAttribute("data-en-variant") === "secondary") return "fable"
    if (codex?.getAttribute("data-en-variant") === "secondary") return "codex"
    return null
  }
  input.focus()
  const before = selectedHarness()
  if (before !== "fable" && before !== "codex") return { ok: false, reason: "no harness selected before toggle: " + before }
  const other = before === "fable" ? "codex" : "fable"
  const dispatchShiftTab = (target) => target.dispatchEvent(
    new KeyboardEvent("keydown", { key: "Tab", shiftKey: true, bubbles: true, cancelable: true }),
  )
  const firstNotPrevented = dispatchShiftTab(input)
  const flipDeadline = Date.now() + 5000
  while (Date.now() < flipDeadline && selectedHarness() !== other) {
    await wait(25)
  }
  if (selectedHarness() !== other) return { ok: false, reason: "Shift+Tab did not toggle to the other harness" }
  if (firstNotPrevented !== false) return { ok: false, reason: "Shift+Tab in the composer was not preventDefaulted" }
  const secondNotPrevented = dispatchShiftTab(input)
  const backDeadline = Date.now() + 5000
  while (Date.now() < backDeadline && selectedHarness() !== before) {
    await wait(25)
  }
  if (selectedHarness() !== before) return { ok: false, reason: "Shift+Tab did not toggle back to the initial harness" }
  if (secondNotPrevented !== false) return { ok: false, reason: "second Shift+Tab was not preventDefaulted" }
  // Focus elsewhere: Shift+Tab must NOT toggle (normal focus navigation).
  const elsewhere = document.querySelector('[data-en-key="workspace-new-chat"]')
  if (elsewhere === null) return { ok: false, reason: "non-composer focus target missing" }
  elsewhere.focus()
  const outsideNotPrevented = dispatchShiftTab(elsewhere)
  await wait(100)
  if (selectedHarness() !== before) return { ok: false, reason: "Shift+Tab outside the composer hijacked the harness selection" }
  if (outsideNotPrevented !== true) return { ok: false, reason: "Shift+Tab outside the composer was preventDefaulted (focus navigation hijacked)" }
  input.focus()
  if (selectedHarness() !== "fable") {
    dispatchShiftTab(input)
    const fableDeadline = Date.now() + 5000
    while (Date.now() < fableDeadline && selectedHarness() !== "fable") await wait(25)
  }
  if (selectedHarness() !== "fable") return { ok: false, reason: "could not restore Fable for the following smoke turn" }
  return { ok: true, iconOnlySend: true, toggledBoth: true, outsideUntouched: true }
})()`

const smokeVoiceMode = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const find = (key) => document.querySelector('[data-en-key="' + key + '"]')
  const mic = find("shell-voice-toggle")
  if (!(mic instanceof HTMLElement)) return { ok: false, reason: "voice control missing" }
  mic.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && find("shell-voice-capture")?.textContent !== "Mic capturing") await wait(25)
  const truth = ["shell-voice-capture", "shell-voice-egress", "shell-voice-retention", "shell-voice-playback"].map(key => find(key)?.textContent ?? "")
  const mute = find("shell-voice-mute")
  if (!(mute instanceof HTMLElement)) return { ok: false, reason: "mute control missing" }
  mute.click()
  const muteDeadline = Date.now() + 5000
  while (Date.now() < muteDeadline && (find("shell-voice-capture")?.textContent !== "Mic off" || find("shell-voice-egress")?.textContent !== "Not sending")) await wait(25)
  const muted = find("shell-voice-capture")?.textContent === "Mic off" && find("shell-voice-egress")?.textContent === "Not sending"
  const unmute = find("shell-voice-mute")
  if (unmute instanceof HTMLElement) {
    unmute.click()
    const unmuteDeadline = Date.now() + 5000
    while (Date.now() < unmuteDeadline && find("shell-voice-capture")?.textContent !== "Mic capturing") await wait(25)
  }
  while (Date.now() < deadline && find("workspace-home-panel") === null) await wait(25)
  const registeredFocus = find("workspace-home-panel") !== null
  const chat = find("workspace-chat")
  if (chat instanceof HTMLElement) { chat.click(); await wait(50) }
  while (Date.now() < deadline && find("shell-voice-playback-outcome") === null) await wait(25)
  const bargeInOutcome = find("shell-voice-playback-outcome")?.textContent?.includes("outcome.smoke.interrupt.1") === true
  const stop = find("shell-voice-toggle")
  if (stop instanceof HTMLElement) {
    stop.click()
    const stopDeadline = Date.now() + 5000
    while (Date.now() < stopDeadline && find("shell-voice-hud") !== null) await wait(25)
  }
  return { ok: registeredFocus && muted && bargeInOutcome && truth.includes("Not retained") && find("shell-voice-hud") === null, truth, registeredFocus, muted, bargeInOutcome, stopped: find("shell-voice-hud") === null }
})()`

// Fable local streaming journey (#8712, EP250 owner fixes): from the fresh
// chat, the Fable chip must be enabled (fixture account) and Codex visibly
// disabled with its reason ONLY in the accessible label — NO caption text
// anywhere in the composer ("Don't put that shit in the UI ever."). A send
// must stream PROGRESSIVE text (a partial snapshot with a still-unterminated
// ** marker renders gracefully as plain text), then finalize with the
// assistant body rendered as MARKDOWN (a real <strong>, no literal **), no
// ASSISTANT sender label, and the composer re-enabled. Fixture-driven; the
// event mapping, IPC bridge, thread persistence, and renderer streaming path
// are all real.
const smokeFableLocalStreaming = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const harness = document.querySelector('[data-en-key="shell-harness-select"]')
  if (!(harness instanceof HTMLSelectElement)) return { ok: false, reason: "provider select never mounted" }
  const fable = Array.from(harness.options).find((option) => option.value === "fable")
  const codex = Array.from(harness.options).find((option) => option.value === "codex")
  if (fable === undefined || codex === undefined) return { ok: false, reason: "provider options missing" }
  if (fable.disabled !== false) return { ok: false, reason: "Fable option disabled despite fixture account" }
  // EP250 chip-verified-evidence rule: the codex availability invoke is
  // GATED in smoke (released after this step), so at this point the chip is
  // deterministically disabled with its "verifying" reason — the state the
  // popover contract asserts against. The codex-first-class enabled state is
  // asserted by the later codex-local-streamed step.
  if (codex.disabled !== true) return { ok: false, reason: "Codex option enabled before the smoke availability gate released" }
  const composer = document.querySelector('[data-en-key="shell-composer"]')
  // No STANDING caption: visible composer text must not carry the reason.
  // The hover-only disabled-reason popover (owner contract EP250) keeps its
  // content in a [hidden] tooltip bubble — excluded from the visible check.
  const visibleComposerText = composer === null ? "" : (() => {
    const clone = composer.cloneNode(true)
    for (const bubble of clone.querySelectorAll('[data-en-role="tooltip"]')) bubble.remove()
    return clone.textContent ?? ""
  })()
  if (document.querySelector('[data-en-key="shell-harness-caption"]') !== null ||
      visibleComposerText.includes("requires OpenAgents session")) {
    return { ok: false, reason: "composer still renders a standing disabled-reason caption" }
  }
  harness.value = "fable"
  harness.dispatchEvent(new Event("change", { bubbles: true }))
  await wait(50)
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Stream a fable-local proof"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  const assistantBodies = () => {
    const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
    return Array.from(rows)
      .map((row) => row.querySelector('[data-en-role="body"]'))
      .filter((body) => body !== null)
  }
  const assistantText = () => {
    // Provider events are rendered in arrival order, so tool events close the
    // current assistant segment and later text opens another one. Reconstruct
    // the turn text across those ordered assistant segments while excluding
    // each compact details affordance.
    return assistantBodies().map((body) => Array.from(body.childNodes)
        .filter((node) => !(node instanceof HTMLElement &&
          (node.tagName === "BUTTON" || node.querySelector("button") !== null)))
        .map((node) => node.textContent ?? "")
        .join(""))
      .join(" ")
  }
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  const finalText = "Fable local streaming proof."
  let sawPartial = false
  const deadline = Date.now() + 20000
  while (Date.now() < deadline) {
    const text = assistantText()
    if (text.length > 0 && text !== finalText) sawPartial = true
    if (text === finalText && input.disabled === false) break
    await wait(25)
  }
  const systemRows = () => Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="system"]'),
  ).map((row) => row.textContent ?? "")
  // EP250 tool cards: tool invocations render as typed role="tool" cards
  // (humanized primary line + status chip + result line), started and
  // completion folded into ONE updating card, raw JSON collapsed by default.
  const toolRows = () => Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="tool"]'),
  )
  const toolText = (row) => row.textContent ?? ""
  const readCards = toolRows().filter((row) => toolText(row).includes("notes.md"))
  const trace = readCards.length === 1 && toolText(readCards[0]).includes("Read")
  // Codex delegation (#8712 Lane C): the fixture turn calls the REAL
  // mcp__codex__delegate handler once (scripted codex exec child behind it).
  // ONE card carries the humanized task line AND the child's answer text.
  const delegateCards = toolRows().filter((row) => toolText(row).includes("Delegate to Codex"))
  const delegateSingleCard = delegateCards.length === 1
  const delegateUse = delegateSingleCard &&
    toolText(delegateCards[0]).includes("Summarize the fixture delegation task")
  const delegateResult = delegateSingleCard &&
    toolText(delegateCards[0]).includes("Codex child fixture answer.")
  const transcriptText = document.querySelector('[data-en-key="shell-transcript"]')?.textContent ?? ""
  const noRawJson = !transcriptText.includes('{"task"') && !transcriptText.includes('{"file_path"')
  // EP250 wave-2 (J2/J4): the fixture's TodoWrite emits plan_updated, so the
  // transcript must render a compact plan/todo card — the "Plan" header, the
  // progress line, and the step content with a status glyph — never raw JSON.
  const planCards = toolRows().filter((row) => toolText(row).includes("Summarize for the user"))
  const planCard = planCards.length === 1 &&
    toolText(planCards[0]).includes("Plan") &&
    toolText(planCards[0]).includes("1 of 2 done") &&
    toolText(planCards[0]).includes("Read the fixture notes") &&
    !toolText(planCards[0]).includes('{"todos"')
  const noSystemToolLabel = toolRows().every((row) => row.querySelector('[data-en-role="sender"]') === null)
  const toolTimestamps = toolRows().every((row) => row.querySelector('[data-en-role="timestamp"]') !== null)
  const strong = assistantBodies()
    .map((body) => body.querySelector("strong"))
    .find((candidate) => candidate !== null) ?? null
  const markdownRendered = strong !== null && strong.textContent === "streaming" &&
    !assistantText().includes("**")
  const assistantRows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const lastAssistant = assistantRows[assistantRows.length - 1]
  const noAssistantLabel = lastAssistant !== undefined &&
    lastAssistant.querySelector('[data-en-role="sender"]') === null
  return {
    ok: sawPartial && assistantText() === finalText && input.disabled === false && trace &&
      markdownRendered && noAssistantLabel && delegateSingleCard && delegateUse &&
      delegateResult && noRawJson && noSystemToolLabel && toolTimestamps && planCard,
    sawPartial,
    finalized: assistantText() === finalText,
    trace,
    markdownRendered,
    noAssistantLabel,
    delegateSingleCard,
    delegateUse,
    delegateResult,
    noRawJson,
    noSystemToolLabel,
    toolTimestamps,
    planCard,
    text: assistantText(),
  }
})()`

// Capability I1 image attach: drop a fixture PNG onto the composer, assert the
// thumbnail renders, submit a text+image turn, and assert the assistant reply
// carries the fable fixture's image-received marker — proving the image
// content block reached the SDK query payload end-to-end (the fable fixture
// drains the streaming-input prompt and counts image blocks). Runs on a fresh
// chat so its Read-triggered fixture question card does not collide with the
// earlier question-card step. Fable-lane only (the fixture query is Fable's).
const smokeFableImageAttach = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const harness = document.querySelector('[data-en-key="shell-harness-select"]')
  if (harness instanceof HTMLSelectElement && harness.value !== "fable") {
    harness.value = "fable"
    harness.dispatchEvent(new Event("change", { bubbles: true }))
    await wait(50)
  }
  const composer = document.querySelector('[data-en-key="shell-composer"]')
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (composer === null || input === null) return { ok: false, reason: "composer not mounted" }
  // A minimal in-renderer PNG File dropped on the composer (no filesystem read).
  const bytes = new Uint8Array([0x89,0x50,0x4e,0x47,0x0d,0x0a,0x1a,0x0a,0x00,0x01])
  const file = new File([bytes], "smoke.png", { type: "image/png" })
  const dt = new DataTransfer()
  dt.items.add(file)
  input.dispatchEvent(new DragEvent("drop", { bubbles: true, cancelable: true, dataTransfer: dt }))
  const thumbDeadline = Date.now() + 5000
  while (Date.now() < thumbDeadline && document.querySelector('[data-en-key^="composer-image-preview-"]') === null) {
    await wait(50)
  }
  const thumb = document.querySelector('[data-en-key^="composer-image-preview-"]')
  if (thumb === null) return { ok: false, reason: "thumbnail never rendered after drop" }
  const thumbSourceOk = (thumb.getAttribute("src") || thumb.querySelector("img")?.getAttribute("src") || "").startsWith("data:image/png;base64,")
  // Type + submit a text-with-image turn.
  input.focus()
  input.value = "What is in this screenshot?"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  const markerVisible = () => Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"] [data-en-role="body"]')
  ).some(row => (row.textContent || "").includes("fixture-received-images:1"))
  const replyDeadline = Date.now() + 20000
  while (Date.now() < replyDeadline && !markerVisible()) { await wait(100) }
  // Thumbnails clear on submit (attachments moved into the turn payload).
  const cleared = document.querySelector('[data-en-key^="composer-image-preview-"]') === null
  return {
    ok: thumbSourceOk && markerVisible() && cleared,
    thumbSourceOk,
    markerVisible: markerVisible(),
    cleared,
  }
})()`

// EP250 question cards: the smoke fixture persists one pending question note
// after the Read tool completes. The REAL preload answerQuestion bridge is
// live, so the card renders fully interactive (option buttons with label +
// dim description), no SYSTEM label, never raw JSON. Clicking an option
// drives the real typed IPC; the runtime's typed rejection (false — this
// fixture question is store-persisted, not runtime-pending) must revert the
// card to honest pending with the selection retained, never a fake
// "Answered" state.
const smokeQuestionCard = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const cardSelector = '[data-en-key="shell-transcript"] [data-en-message="question-question.fixture.1"]'
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && document.querySelector(cardSelector) === null) {
    await wait(50)
  }
  const card = document.querySelector(cardSelector)
  if (card === null) return { ok: false, reason: "question card never rendered" }
  const text = card.textContent ?? ""
  const headerChip = text.includes("Fixture")
  const questionText = text.includes("Which fixture path should this smoke turn take?")
  const optionSelector = '[data-en-key="question-question.fixture.1-q0-option-0"]'
  const optionA = card.querySelector(optionSelector)
  const optionB = card.querySelector('[data-en-key="question-question.fixture.1-q0-option-1"]')
  const description = text.includes("Keep the streamed markdown proof path")
  // The real answerQuestion bridge exists: options are interactive.
  const interactive = optionA !== null && optionA.disabled === false &&
    optionB !== null && optionB.disabled === false
  const noSenderLabel = card.querySelector('[data-en-role="sender"]') === null
  const noRawJson = !text.includes("{") && !text.includes("questionRef")
  if (!(headerChip && questionText && interactive && description && noSenderLabel && noRawJson &&
    optionA.textContent === "Streamed" && optionB.textContent === "Static")) {
    return { ok: false, headerChip, questionText, description, interactive, noSenderLabel, noRawJson }
  }
  // Click through the REAL typed bridge. The runtime rejects (false: the
  // fixture ref is not runtime-pending), so the card must settle back to
  // pending with the Streamed selection retained — honest, no fake Answered.
  optionA.click()
  const settleDeadline = Date.now() + 10000
  let settled = null
  while (Date.now() < settleDeadline) {
    settled = document.querySelector(optionSelector)
    if (settled !== null && settled.getAttribute("data-en-variant") === "secondary") break
    await wait(50)
  }
  const revertedPending = settled !== null && settled.getAttribute("data-en-variant") === "secondary"
  const cardAfter = document.querySelector(cardSelector)
  const noFakeAnswered = cardAfter !== null && !(cardAfter.textContent ?? "").includes("Answered")
  return {
    ok: revertedPending && noFakeAnswered,
    revertedPending,
    noFakeAnswered,
  }
})()`

// Codex local streamed turn (EP250 codex-first-class): from the same fresh
// chat, selecting the Codex chip and sending must stream a REAL fixture
// `codex exec --json` event sequence through the actual parser, IPC bridge,
// thread persistence, and renderer path — rendering IDENTICALLY to fable
// turns: reasoning line, Bash tool card, markdown assistant body (a real
// <strong>), the "Codex · gpt-5.6-sol (requested)" spawn-config-truth
// caption, no ASSISTANT label, and the composer re-enabled.
const smokeCodexLocalStreaming = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const engine = document.querySelector('[data-en-key="shell-codex-engine"]')
  if (engine?.textContent !== "Codex") return { ok: false, reason: "fixed Codex engine label never mounted" }
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Stream a codex-local proof"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  const readyDeadline = Date.now() + 10000
  while (Date.now() < readyDeadline) {
    const send = document.querySelector('[data-en-key="shell-note"]')
    if (send instanceof HTMLButtonElement && !send.disabled) break
    await wait(50)
  }
  const send = document.querySelector('[data-en-key="shell-note"]')
  if (!(send instanceof HTMLButtonElement) || send.disabled) return { ok: false, reason: "Codex session stayed unavailable" }
  const assistantBodies = () => Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"] [data-en-role="body"]'),
  )
  const bodiesBefore = assistantBodies().length
  input.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }))
  const finalText = "Codex local fixture proof."
  const lastAssistantText = () => {
    const bodies = assistantBodies()
    const last = bodies[bodies.length - 1]
    if (last === undefined || bodies.length <= bodiesBefore) return ""
    return Array.from(last.childNodes)
      .filter((node) => !(node instanceof HTMLElement &&
        (node.tagName === "BUTTON" || node.querySelector("button") !== null)))
      .map((node) => node.textContent ?? "")
      .join("")
  }
  const deadline = Date.now() + 20000
  while (Date.now() < deadline) {
    if (lastAssistantText() === finalText && input.disabled === false) break
    await wait(25)
  }
  if (lastAssistantText() !== finalText) {
    return { ok: false, reason: "codex assistant text never finalized", text: lastAssistantText() }
  }
  const transcriptText = document.querySelector('[data-en-key="shell-transcript"]')?.textContent ?? ""
  // Spawn-config-truth caption: the trace line names the lane AND the
  // "(requested)" labeling — never an unlabeled model echo.
  const modelCaption = transcriptText.includes("Codex · gpt-5.6-sol (requested)")
  const reasoningLine = transcriptText.includes("Reasoning · planned the fixture reply")
  const toolRows = Array.from(
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="tool"]'),
  )
  const bashCard = toolRows.some((row) => (row.textContent ?? "").includes("echo fixture"))
  const bodies = assistantBodies()
  const last = bodies[bodies.length - 1]
  const strong = last === undefined ? null : last.querySelector("strong")
  const markdownRendered = strong !== null && strong.textContent === "fixture" &&
    !lastAssistantText().includes("**")
  const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const lastRow = rows[rows.length - 1]
  const noAssistantLabel = lastRow !== undefined &&
    lastRow.querySelector('[data-en-role="sender"]') === null
  return {
    ok: modelCaption && reasoningLine && bashCard && markdownRendered && noAssistantLabel &&
      input.disabled === false,
    modelCaption,
    reasoningLine,
    bashCard,
    markdownRendered,
    noAssistantLabel,
  }
})()`

// Message metadata inspector (#8712, EP250 owner fix 2): clicking a chat
// message's details affordance opens the right-side inspector with the
// persisted host metadata (lane, SDK-reported effective model, account ref,
// turn ref, exact token total, duration); Close dismisses it through the
// same typed intent.
// Owner bug (verbatim): "the 'details' thing under message ... flashes back in
// every time i type something in the input, WHY??? WHY IS IT CONNECTED TO
// ANOTHER COMPONENT". The per-message details affordance is a hover/focus
// reveal (opacity-0 at rest). Its visibility must be a pure function of
// pointer/focus — never of composer input or any unrelated re-render. This
// asserts that typing in the composer (while NOT hovering the row) does not
// change the affordance's computed opacity and does not replace/re-parent its
// DOM node (a re-parent is what restarted the CSS transition -> the flash).
const smokeDetailsAffordanceStableOnInput = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const last = rows[rows.length - 1]
  if (last === undefined) return { ok: false, reason: "no assistant message row for the details-flash check" }
  const details = last.querySelector('[data-en-key^="note-details-"]')
  if (details === null) return { ok: false, reason: "details affordance never mounted" }
  // Make sure we are in the resting (un-hovered) state.
  last.dispatchEvent(new PointerEvent("pointerleave", { bubbles: false }))
  details.blur?.()
  await wait(200) // allow any (bugged) transition to settle
  const restingOpacity = getComputedStyle(details).opacity
  if (restingOpacity !== "0") {
    return { ok: false, reason: "details affordance is not hidden at rest: opacity=" + restingOpacity }
  }
  // Tag the exact node + parent so we can detect replacement / re-parenting.
  details.dataset.enFlashProbe = "1"
  const parentBefore = details.parentElement
  // Type in the composer — the exact "type something in the input" scenario.
  const input = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  let maxOpacityDuringTyping = 0
  for (const value of ["details-flash-probe a", "details-flash-probe ab", "details-flash-probe abc"]) {
    input.value = value
    input.dispatchEvent(new Event("input", { bubbles: true }))
    // Sample opacity across the frames a replayed 150ms transition would occupy.
    for (let i = 0; i < 8; i += 1) {
      await wait(20)
      const now = last.querySelector('[data-en-key^="note-details-"]')
      if (now !== null) {
        maxOpacityDuringTyping = Math.max(maxOpacityDuringTyping, Number(getComputedStyle(now).opacity) || 0)
      }
    }
  }
  const detailsAfter = last.querySelector('[data-en-key^="note-details-"]')
  if (detailsAfter === null) return { ok: false, reason: "details affordance disappeared after typing" }
  // Restore the composer so later smoke steps start from an empty input.
  input.value = ""
  input.dispatchEvent(new Event("input", { bubbles: true }))
  const sameNode = detailsAfter === details && detailsAfter.dataset.enFlashProbe === "1"
  const sameParent = detailsAfter.parentElement === parentBefore
  const finalOpacity = getComputedStyle(detailsAfter).opacity
  return {
    ok: sameNode && sameParent && finalOpacity === "0" && maxOpacityDuringTyping < 0.01,
    reason: sameNode && sameParent && finalOpacity === "0" && maxOpacityDuringTyping < 0.01
      ? undefined
      : "details flashed / re-parented on composer input",
    sameNode,
    sameParent,
    restingOpacity,
    finalOpacity,
    maxOpacityDuringTyping,
  }
})()`

const smokeMessageInspector = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const last = rows[rows.length - 1]
  if (last === undefined) return { ok: false, reason: "no assistant message row to inspect" }
  const details = last.querySelector('[data-en-key^="note-details-"]')
  if (details === null) return { ok: false, reason: "message details affordance never mounted" }
  // EP250 owner fix: the details affordance must be the compact ghost text
  // button, NOT the 44px IconButton circle ("way smaller ... not a huge
  // ginormous circle").
  if (details.getAttribute("data-en-variant") === "icon") {
    return { ok: false, reason: "details affordance is still the large icon-circle variant" }
  }
  const detailsHeight = details.getBoundingClientRect().height
  if (detailsHeight > 28) {
    return { ok: false, reason: "details affordance is not compact: " + detailsHeight + "px tall" }
  }
  details.click()
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && document.querySelector('[data-en-key="chat-message-inspector"]') === null) {
    await wait(50)
  }
  const inspector = document.querySelector('[data-en-key="chat-message-inspector"]')
  if (inspector === null) return { ok: false, reason: "message inspector never opened" }
  const text = inspector.textContent || ""
  const hasModel = text.includes("gpt-5.6-sol")
  const hasLane = text.includes("codex-local")
  const hidesAccount = !text.includes("Account") && !text.includes("codex-3")
  const hasTokens = text.includes("Tokens (total)") && text.includes("952")
  const close = document.querySelector('[data-en-key="chat-message-inspector-close"]')
  if (close === null) return { ok: false, reason: "inspector close affordance missing" }
  close.click()
  const closeDeadline = Date.now() + 5000
  while (Date.now() < closeDeadline && document.querySelector('[data-en-key="chat-message-inspector"]') !== null) {
    await wait(50)
  }
  return {
    ok: hasModel && hasLane && hidesAccount && hasTokens &&
      document.querySelector('[data-en-key="chat-message-inspector"]') === null,
    hasModel,
    hasLane,
    hidesAccount,
    hasTokens,
  }
})()`

// Re-open the inspector purely for the pixel receipt (the previous step
// proved open + close through the typed intent loop).
const smokeReopenMessageInspector = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const last = rows[rows.length - 1]
  const details = last === undefined ? null : last.querySelector('[data-en-key^="note-details-"]')
  if (details === null) return { ok: false, reason: "message details affordance never mounted" }
  details.click()
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && document.querySelector('[data-en-key="chat-message-inspector"]') === null) {
    await wait(50)
  }
  return { ok: document.querySelector('[data-en-key="chat-message-inspector"]') !== null }
})()`

const smokeCloseMessageInspector = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const close = document.querySelector('[data-en-key="chat-message-inspector-close"]')
  if (close === null) return { ok: false, reason: "inspector close affordance missing" }
  close.click()
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && document.querySelector('[data-en-key="chat-message-inspector"]') !== null) {
    await wait(50)
  }
  return { ok: document.querySelector('[data-en-key="chat-message-inspector"]') === null }
})()`

// Fleet workspace journey (#8712): the dock's Fleet button must open the
// read-only fleet panel with the fixture provider accounts rendered.
const smokeOpenFleetWorkspace = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="workspace-fleet"]')
  if (button === null) return { ok: false, reason: "Fleet dock button never mounted" }
  button.click()
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-key="fleet-accounts-table"]') === null) {
    await wait(50)
  }
  const panel = document.querySelector('[data-en-key="workspace-fleet-panel"]')
  const expected = ["codex-3", "codex", "codex-2", "claude-pylon-3"]
  const refs = expected.map((ref) =>
    document.querySelector('[data-en-key="fleet-ref-' + ref + '"]')?.textContent ?? null)
  const revoked = document.querySelector('[data-en-key="fleet-readiness-codex-2"]')
  const dots = document.querySelector('[data-en-key="fleet-status-dots"]')
  // Session usage ledger (#8712 Lane C): the earlier fixture delegation left
  // exact rows — codex needs reconnect (child observed the revoked token;
  // this SUPERSEDES its presence-based "ready"), codex-2 served the child
  // with exact usage and the requested model recorded as spawn-config truth.
  const ledgerSection = document.querySelector('[data-en-key="fleet-session-usage"]')
  const ledgerTotal = document.querySelector('[data-en-key="fleet-ledger-total-codex-2"]')
  const ledgerModel = document.querySelector('[data-en-key="fleet-ledger-model-codex-2"]')
  const reconnectReadiness = document.querySelector('[data-en-key="fleet-readiness-codex"]')
  return {
    ok: panel !== null && dots !== null &&
      refs.every((value, index) => value === expected[index]) &&
      revoked !== null && revoked.textContent === "credentials-missing" &&
      ledgerSection !== null &&
      ledgerTotal !== null && (ledgerTotal.textContent ?? "").includes("1,440") &&
      ledgerModel !== null && (ledgerModel.textContent ?? "").includes("gpt-5.6-sol") &&
      reconnectReadiness !== null && reconnectReadiness.textContent === "reconnect required",
    refs,
    revokedReadiness: revoked === null ? null : revoked.textContent,
    ledgerTotal: ledgerTotal === null ? null : ledgerTotal.textContent,
    ledgerModel: ledgerModel === null ? null : ledgerModel.textContent,
    reconnectReadiness: reconnectReadiness === null ? null : reconnectReadiness.textContent,
  }
})()`

const smokeCodingCatalog = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const home = document.querySelector('[data-en-key="workspace-home"]')
  if (home === null) return { ok: false, reason: "Project home dock button missing" }
  home.click()
  const deadline = Date.now() + 10000
  while (Date.now() < deadline && document.querySelector('[data-en-key^="workspace-home-session-session.desktop."]') === null) {
    await wait(50)
  }
  const rows = Array.from(document.querySelectorAll('[data-en-key^="workspace-home-session-session.desktop."]'))
  const row = rows.find(candidate =>
    Array.from(candidate.querySelectorAll('[data-en-key^="workspace-home-session-open-"]'))
      .some(button => button.textContent === "Current")) ?? null
  const authority = document.querySelector('[data-en-key="workspace-home-authority"]')
  const current = row?.querySelector('[data-en-key^="workspace-home-session-open-"]')
  return {
    ok: row !== null && authority?.textContent === "This Mac" && current?.textContent === "Current",
    sessionKey: row?.getAttribute("data-en-key") ?? null,
    authority: authority?.textContent ?? null,
    current: current?.textContent ?? null,
  }
})()`

// Owner contract (EP250, verbatim): "when i do new chat, clicking button or
// command N, auto focus the input." Cmd+N (Meta+N on darwin, the canonical
// chat.new binding) must dispatch DesktopNewChat from anywhere outside an
// editable and land with a fresh transcript AND the composer focused.
const smokeCmdNNewChat = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  if (document.activeElement instanceof HTMLElement) document.activeElement.blur()
  const darwin = ${JSON.stringify(process.platform === "darwin")}
  window.dispatchEvent(new KeyboardEvent("keydown", {
    key: "n",
    metaKey: darwin,
    ctrlKey: !darwin,
    bubbles: true,
    cancelable: true,
  }))
  const deadline = Date.now() + 10000
  const messageCount = () =>
    document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message]').length
  while (Date.now() < deadline && (
    document.querySelector('[data-en-key="shell-transcript"]') === null || messageCount() !== 0
  )) {
    await wait(50)
  }
  const composer = document.querySelector('[data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  const focusDeadline = Date.now() + 3000
  while (Date.now() < focusDeadline && document.activeElement !== composer) {
    await wait(50)
  }
  return {
    ok: document.querySelector('[data-en-key="shell-transcript"]') !== null &&
      messageCount() === 0 && composer !== null &&
      document.activeElement === composer,
    messages: messageCount(),
    composerFocused: document.activeElement === composer,
  }
})()`

const captureShot = async (window: BrowserWindow, name: string): Promise<void> => {
  if (smokeShotsDir === undefined || smokeShotsDir === "") return
  const image = await window.webContents.capturePage()
  const { mkdirSync, writeFileSync } = await import("node:fs")
  mkdirSync(smokeShotsDir, { recursive: true })
  writeFileSync(path.join(smokeShotsDir, `${name}.png`), image.toPNG())
  console.log(`[openagents-desktop smoke] shot ${name}.png`)
}

const launchSmokeSecondInstance = async (): Promise<void> => {
  const { spawn } = await import("node:child_process")
  await new Promise<void>((resolve, reject) => {
    const child = spawn(
      process.execPath,
      [app.getAppPath(), "openagents://command/settings.open"],
      {
        env: {
          ...process.env,
          OPENAGENTS_DESKTOP_SMOKE: "0",
          OPENAGENTS_DESKTOP_USER_DATA: app.getPath("userData"),
        },
        stdio: "ignore",
      },
    )
    const timeout = setTimeout(() => {
      child.kill()
      reject(new Error("second-instance command process did not exit"))
    }, 8_000)
    child.once("error", error => {
      clearTimeout(timeout)
      reject(error)
    })
    child.once("exit", code => {
      clearTimeout(timeout)
      code === 0 || code === null ? resolve() : reject(new Error(`second-instance command exited ${code}`))
    })
  })
}

/**
 * Startup-marks driver (measure-constantly discipline; scripts/startup-bench.ts).
 * Waits for the renderer to report its shell-mounted mark, reads the paint
 * timing back out of the sandboxed renderer with executeJavaScript (no boundary
 * change — the same read-only channel smoke uses), times the runtime-gateway
 * bootstrap (capability-ready), writes the milestone chain as JSON to `file`,
 * then tears down and exits. All marks are ms from process start
 * (`desktopStartupT0`, the main perf origin).
 */
const runStartupMarks = (
  window: BrowserWindow,
  file: string,
  options: Readonly<{ preserveUserData: boolean }>,
): void => {
  const finish = (code: 0 | 1): void => {
    try { workspaceSearchRegistry.dispose() } catch { /* best-effort teardown */ }
    try { hostLifecycle.dispose() } catch { /* best-effort teardown */ }
    try { desktopCorrelationJournal.dispose() } catch { /* best-effort teardown */ }
    // Fixture marks runs own a temp profile and wipe it; the real-wiring
    // trace measured a real profile and must never delete it.
    if (!options.preserveUserData) {
      try { rmSync(app.getPath("userData"), { recursive: true, force: true }) } catch { /* temp userData */ }
    }
    app.exit(code)
  }
  const timeout = setTimeout(() => {
    console.error("[openagents-desktop startup-marks] TIMEOUT waiting for renderer")
    finish(1)
  }, 45_000)
  // Pixel receipts (2026-07-13 startup incident): when a shots directory is
  // named, capture the first presentable frame (the branded boot frame) and
  // the mounted shell. Timings-only receipts stay the default.
  const shotsDir = process.env.OPENAGENTS_DESKTOP_STARTUP_TRACE_SHOTS
  const captureShot = (name: string): Promise<void> =>
    shotsDir === undefined || window.isDestroyed()
      ? Promise.resolve()
      : window.webContents.capturePage().then(image => {
          mkdirSync(shotsDir, { recursive: true })
          writeFileSync(path.join(shotsDir, name), image.toPNG())
        }).catch(() => {})
  if (shotsDir !== undefined) {
    window.once("ready-to-show", () => { void captureShot("boot-frame.png") })
  }
  const rendererReadback = `(async () => {
    const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
    const marks = () => globalThis.__oaStartupMarks || {}
    const deadline = Date.now() + 30000
    while (Date.now() < deadline &&
      (typeof marks().shellMounted !== "number" ||
       document.querySelector('[data-en-key="shell-root"]') === null)) {
      await wait(20)
    }
    const bridge = globalThis.openagentsDesktop
    let bootstrapWall = null
    if (typeof bridge?.runtimeRequest === "function") {
      const started = Date.now()
      const result = await bridge.runtimeRequest({
        kind: "query",
        requestId: "startup-marks-bootstrap",
        query: { id: "runtime.bootstrap" },
      })
      if (result?.result?.lifecycle === "ready") bootstrapWall = Date.now()
      else bootstrapWall = started
    }
    // Post-mount hydration (2026-07-13 startup incident): the shell above is
    // already interactable and capabilityReady already measured; now wait
    // (bounded) for the hydrated thread-list content mark.
    const hydrateDeadline = Date.now() + 30000
    while (Date.now() < hydrateDeadline && typeof marks().historyHydrated !== "number") {
      await wait(50)
    }
    const paint = performance.getEntriesByType("paint")
    const fcp = paint.find((entry) => entry.name === "first-contentful-paint")
    const fp = paint.find((entry) => entry.name === "first-paint")
    return {
      bootStart: marks().bootStart ?? null,
      shellMounted: marks().shellMounted ?? null,
      historyHydrated: marks().historyHydrated ?? null,
      shellPresent: document.querySelector('[data-en-key="shell-root"]') !== null,
      timeOrigin: performance.timeOrigin,
      firstPaintOffset: fp ? fp.startTime : null,
      firstContentfulPaintOffset: fcp ? fcp.startTime : null,
      bootstrapWall,
    }
  })()`
  window.webContents.once("did-finish-load", () => {
    void (async () => {
      try {
        const readback = await window.webContents.executeJavaScript(rendererReadback, true) as {
          bootStart: number | null
          shellMounted: number | null
          historyHydrated: number | null
          shellPresent: boolean
          timeOrigin: number
          firstPaintOffset: number | null
          firstContentfulPaintOffset: number | null
          bootstrapWall: number | null
        }
        if (!readback.shellPresent || readback.shellMounted === null) {
          throw new Error(`renderer never mounted: ${JSON.stringify(readback)}`)
        }
        const t0 = desktopStartupT0
        const rel = (wall: number | null): number | null =>
          wall === null ? null : Math.round((wall - t0) * 100) / 100
        const paintWall = readback.firstContentfulPaintOffset !== null
          ? readback.timeOrigin + readback.firstContentfulPaintOffset
          : readback.firstPaintOffset !== null
            ? readback.timeOrigin + readback.firstPaintOffset
            : null
        const marks = {
          // Every value is ms since process start (main performance origin).
          mainModuleEvaluated: rel(desktopMainMarks.mainModuleEvaluated ?? null),
          appWhenReady: rel(desktopMainMarks.appWhenReady ?? null),
          sessionHardened: rel(desktopMainMarks.sessionHardened ?? null),
          syncHostOpened: rel(desktopMainMarks.syncHostOpened ?? null),
          sessionVaultRecovered: rel(desktopMainMarks.sessionVaultRecovered ?? null),
          sessionRecoverySettled: rel(desktopMainMarks.sessionRecoverySettled ?? null),
          windowCreated: rel(desktopMainMarks.windowCreated ?? null),
          windowReadyToShow: rel(desktopMainMarks.windowReadyToShow ?? null),
          rendererBootStart: rel(readback.bootStart),
          firstPaint: rel(paintWall),
          shellMounted: rel(readback.shellMounted),
          historyHydrated: rel(readback.historyHydrated),
          capabilityReady: rel(readback.bootstrapWall),
        }
        mkdirSync(path.dirname(file), { recursive: true })
        writeFileSync(file, JSON.stringify({
          schema: "openagents-desktop-startup-marks/v1",
          capturedAtIso: new Date().toISOString(),
          unit: "ms-from-process-start",
          marks,
        }, null, 2))
        console.log("[openagents-desktop startup-marks] captured", JSON.stringify(marks))
        await captureShot("shell-mounted.png")
        clearTimeout(timeout)
        finish(0)
      } catch (error) {
        console.error("[openagents-desktop startup-marks] FAILED", error instanceof Error ? error.message : error)
        clearTimeout(timeout)
        finish(1)
      }
    })()
  })
}

const runSmoke = (window: BrowserWindow): void => {
  const finish = (code: 0 | 1): void => {
    workspaceSearchRegistry.dispose()
    terminalHost.dispose()
    hostLifecycle.dispose()
    const snapshot = hostLifecycle.snapshot()
    const active = Number(snapshot.runtime) + Number(snapshot.workspace) + Number(snapshot.sync) +
      Number(snapshot.account) + Number(snapshot.history) + snapshot.windowCount +
      workspaceSearchRegistry.activeCount() + terminalHost.liveSessionCount()
    const ok = snapshot.disposed && active === 0
    console.log("[openagents-desktop smoke] lifecycle-teardown", JSON.stringify({ ok, active }))
    desktopCorrelationJournal.dispose()
    if (smokeMode) rmSync(app.getPath("userData"), { recursive: true, force: true })
    app.exit(ok ? code : 1)
  }
  const timeout = setTimeout(() => {
    console.error("[openagents-desktop smoke] TIMEOUT waiting for renderer")
    finish(1)
  }, 45_000)
  let tracePass = 0
  window.webContents.on("did-finish-load", () => {
    void (async () => {
      const step = async (name: string, script: string): Promise<void> => {
        const result: unknown = await window.webContents.executeJavaScript(script, true)
        const ok =
          result === true ||
          (typeof result === "object" && result !== null && (result as { ok?: unknown }).ok === true)
        if (!ok) {
          throw new Error(`${name} failed: ${JSON.stringify(result)}`)
        }
        console.log(`[openagents-desktop smoke] ${name} OK`, JSON.stringify(result))
      }
      try {
        if (tracePass === 1) {
          await step("workspace-editor-reload-recovery", smokeWorkspaceEditorRecovery)
          await step("codex-trace-reload-restoration", traceAcceptanceReload)
          await step("coding-catalog-reload-restoration", smokeCodingCatalog)
          clearTimeout(timeout)
          console.log("[openagents-desktop smoke] OK")
          finish(0)
          return
        }
        await step("shell-mounted", smokeWaitForShell)
        // #8787: BEFORE any pointer event, the composer holds keyboard focus
        // (at shell-interactable AND after hydration), and a real Chromium
        // keystroke from the main process lands in it as typed text.
        await step("composer-focused-on-open", smokeComposerFocusedOnOpen)
        window.webContents.sendInputEvent({ type: "keyDown", keyCode: "K" })
        window.webContents.sendInputEvent({ type: "char", keyCode: "k" })
        window.webContents.sendInputEvent({ type: "keyUp", keyCode: "K" })
        await step("first-keystroke-lands-in-composer", smokeFirstKeystrokeLandsInComposer)
        await step("runtime-gateway-bootstrap", smokeRuntimeGatewayBootstrap)
        await step("workspace-tree-refresh-watch-bridge", smokeWorkspaceTreeBridge)
        // UX-4 (#8790): Files lost its dock icon — route through the closed
        // canonical command identity, exactly like a palette/native-menu user.
        const filesCommand = desktopCanonicalCommandRegistry.find(command => command.id === "workspace.files")
        if (filesCommand === undefined) throw new Error("canonical workspace.files command missing")
        desktopCommandHost.enqueue(deferredDesktopCommand(
          filesCommand,
          "native_menu",
          "command.desktop.smoke.workspace-files",
        ))
        await step("workspace-files-relative-ui", smokeWorkspaceFilesUi)
        await captureShot(window, "05-files-workspace")
        await step("files-back-to-chat", smokeBackToChat)
        await step("lifecycle-correlation", smokeLifecycleCorrelation)
        if (!desktopCorrelationJournal.complete("correlation.desktop.smoke")) {
          throw new Error(`lifecycle-correlation journal incomplete: ${desktopCorrelationJournal.stages("correlation.desktop.smoke").join(",")}`)
        }
        console.log("[openagents-desktop smoke] lifecycle-correlation-journal OK", JSON.stringify({ ok: true, stageCount: 4 }))
        await captureShot(window, "01-shell")
        const paletteCommand = desktopCanonicalCommandRegistry.find(command => command.id === "palette.toggle")
        if (paletteCommand === undefined) throw new Error("canonical palette command missing")
        desktopCommandHost.enqueue(deferredDesktopCommand(
          paletteCommand,
          "native_menu",
          "command.desktop.smoke.host-routing",
        ))
        await step("command-palette-host-routing", smokeWaitForHostCommandPalette)
        await captureShot(window, "02-command-palette")
        await step("command-palette-close", smokeCloseCommandPalette)
        await launchSmokeSecondInstance()
        await step("command-second-instance-deep-link", smokeWaitForSecondInstanceSettings)
        await step("command-second-instance-close-settings", smokeCloseSettings)
        // Duplicate admission and transient duplicate notices are covered by
        // deterministic host/renderer unit oracles. A second OS process is
        // intentionally exercised only once here: Electron may coalesce a
        // same-URL replay before the renderer can observe its transient toast.
        await step("recent-codex-history-selected-detail", smokeCodexHistoryDetails)
        await step("codex-trace-acceptance", traceAcceptanceJourney)
        await captureShot(window, "03-codex-history-detail")
        // The MVP reuses the ordinary logged-in Codex session and exposes no
        // Pylon account-linking or device-auth surface in Settings.
        await step("settings-current-codex-session", smokeOpenSettings)
        await captureShot(window, "04-settings-current-codex-session")
        // MAINT-1 (#8785): the per-harness maintenance rows resolve from live
        // detection and render version/channel truth + the update affordance.
        await step("settings-harness-maintenance", smokeSettingsHarnessMaintenance)
        await captureShot(window, "04b-settings-harness-maintenance")
        // CUT-24 (#8704): diagnostics panel renders live health + redacted
        // export notice (no secrets), and preferences durable IPC round-trips.
        await step("diagnostics-and-preferences", smokeDiagnosticsAndPreferences)
        await captureShot(window, "04c-diagnostics-panel")
        await step("settings-back-to-chat", smokeCloseSettings)
        // With the historical page still loaded, New chat must yield a fresh
        // empty transcript (the on-camera regression).
        await step("new-chat-from-history-empty-transcript", smokeNewChatFromHistory)
        await captureShot(window, "06-new-chat-empty")
        // #8788: sidebar session search actually filters — title-prefix match
        // shows the fixture session; no-match shows the explicit empty state;
        // clearing restores the full list.
        await step("session-search-filters-title-prefix", smokeSessionSearchFilters)
        await captureShot(window, "13-session-search-filtered")
        await step("session-search-no-match-and-clear-restores", smokeSessionSearchNoMatchAndClear)
        await step("mvp-visible-surface-allowlist", smokeMvpSurfaceAllowlist)
        // Release the codex availability gate: the popover assertions above
        // ran against the deterministic disabled/"verifying" chip; from here
        // the fixture PROBE-VERIFIED evidence lights the chip for the
        // codex-local streamed step below.
        releaseSmokeCodexAvailability?.()
        // Codex local turn is FIXTURE-driven in smoke (scripted codex exec
        // JSONL through the REAL parser; EP250 codex-first-class proof).
        await step("codex-local-streamed-turn-FIXTURE", smokeCodexLocalStreaming)
        await captureShot(window, "11-codex-local-streamed")
        // EP250 (#8712): click a message -> right-side metadata inspector
        // (model/lane/account/tokens), close through the same typed intent,
        // then re-open once for the pixel receipt.
        // Owner details-flash fix: typing in the composer must not change the
        // per-message details affordance's visibility (run before the inspector
        // step consumes the affordance by clicking it).
        // A synthetic pointerleave does not update Chromium's CSS :hover
        // state. Move Electron's real pointer into the inert top-left chrome so
        // the oracle genuinely begins from the documented un-hovered state.
        window.webContents.sendInputEvent({ type: "mouseMove", x: 1, y: 1 })
        await step("details-affordance-stable-on-composer-input", smokeDetailsAffordanceStableOnInput)
        await step("message-metadata-inspector", smokeMessageInspector)
        await step("message-metadata-inspector-reopen", smokeReopenMessageInspector)
        await captureShot(window, "08-message-inspector")
        await step("message-metadata-inspector-close", smokeCloseMessageInspector)
        // Git/GitHub review panel (EP250 E2–E5): route to the review workspace
        // through the canonical command host, then assert the typed Git panel
        // rendered real read-only status of the app's own repo (no commit/push).
        const reviewCommand = desktopCanonicalCommandRegistry.find(command => command.id === "workspace.review")
        if (reviewCommand === undefined) throw new Error("canonical workspace.review command missing")
        desktopCommandHost.enqueue(deferredDesktopCommand(
          reviewCommand,
          "native_menu",
          "command.desktop.smoke.git-review",
        ))
        await step("git-review-panel-real-status", smokeOpenGitReview)
        await captureShot(window, "12-git-review-panel")
        await step("git-review-attach-to-composer", smokeGitReviewAttach)
        // Cmd+N from the review workspace: fresh transcript + focused composer.
        await step("cmd-n-new-chat-focuses-composer", smokeCmdNNewChat)
        await step("coding-catalog-host-persistence", smokeCodingCatalog)
        await captureShot(window, "10-coding-catalog")
        await step("workspaces-back-to-chat", smokeBackToChat)
        tracePass = 1
        window.webContents.reload()
      } catch (error) {
        clearTimeout(timeout)
        console.error("[openagents-desktop smoke] ERROR", error instanceof Error ? error.message : "unknown smoke failure")
        finish(1)
      }
    })()
  })
}

const nativeCommandAccelerator = (bindings: ReadonlyArray<string>): string | undefined => {
  const preferred = bindings.find(binding =>
    process.platform === "darwin" ? binding.startsWith("Meta+") : binding.startsWith("Control+"))
  return preferred?.replace(/^Meta\+/, "CmdOrCtrl+").replace(/^Control\+/, "CmdOrCtrl+")
}

const installDesktopCommandMenu = (bindings?: DesktopCommandBindingProjection): void => {
  const commandItems: MenuItemConstructorOptions[] = desktopCanonicalCommandRegistry.map(command => ({
    label: command.label,
    ...(nativeCommandAccelerator(
      bindings === undefined
        ? command.defaultBindings
        : (commandBindingForNativeMenu(bindings, command.id) === undefined
            ? []
            : [commandBindingForNativeMenu(bindings, command.id)!]),
    ) === undefined
      ? {}
      : { accelerator: nativeCommandAccelerator(
          bindings === undefined
            ? command.defaultBindings
            : [commandBindingForNativeMenu(bindings, command.id)!],
        ) }),
    click: () => {
      desktopCommandHost.enqueue(deferredDesktopCommand(command, "native_menu"))
    },
  }))
  const template: MenuItemConstructorOptions[] = [
    ...(process.platform === "darwin"
      ? [{ label: app.name, submenu: [{ role: "about" as const }, { type: "separator" as const }, { role: "quit" as const }] }]
      : []),
    { label: "Commands", submenu: commandItems },
    { label: "Edit", submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }, { role: "selectAll" }] },
    { label: "Window", submenu: [{ role: "minimize" }, { role: "close" }] },
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

void app.whenReady().then(async () => {
  if (!primaryDesktopInstance) return
  recordMainMark("appWhenReady")
  const providerAccountsBootstrapReceipt = isolatedAppProofMode
    ? isolatedProofReceiptPath({ env: process.env, temporaryDirectory: app.getPath("temp") })
    : null
  if (providerAccountsBootstrapReceipt !== null) {
    const result = await providerAccounts.listProviderAccounts()
    writeFileSync(providerAccountsBootstrapReceipt, JSON.stringify({
      schema: "openagents-desktop.provider-accounts-bootstrap.v1",
      result,
      diagnostics: providerAccountsDiagnostics,
    }, null, 2), { encoding: "utf8", mode: 0o600 })
    app.exit(result.ok ? 0 : 2)
    return
  }
  installDesktopRendererProtocol()
  // macOS does not use BrowserWindow's icon for the active Dock tile in a
  // development Electron process. Set both native surfaces to the same mobile
  // PNG so the running desktop application has one product identity.
  if (process.platform === "darwin") app.dock?.setIcon(desktopIconPath)
  if (app.isPackaged) app.setAsDefaultProtocolClient("openagents")
  desktopCommandBindings = openDesktopCommandBindingStore(
    path.join(app.getPath("userData"), "commands", "bindings.json"),
  )
  installDesktopCommandMenu(desktopCommandBindings.snapshot())
  if (isolatedAppProofMode) {
    // Resolving Electron's default session initializes Chromium's persistent
    // cookie encryption and can invoke macOS Keychain before BrowserWindow.
    // The temp-only proof has no account/network session to harden; avoid
    // constructing it entirely instead of weakening the production session.
    console.warn("[openagents-desktop] isolated app proof: persistent browser session disabled")
  } else {
    // `session` is another lazy Electron export on macOS. Resolving it in the
    // top-level import is enough to initialize persistent cookie encryption,
    // so production requests the default session only inside this branch.
    hardenSession((await import("electron")).session.defaultSession)
  }
  recordMainMark("sessionHardened")
  // Local Sync persistence: synchronous SQLite open + migrations. NEVER on
  // the pre-createWindow path (2026-07-13 startup incident).
  const openLocalSyncPersistence = (): void => {
    try {
      const syncHost = openDesktopSyncHost({
        databasePath: path.join(app.getPath("userData"), "sync", "khala-sync.sqlite"),
        randomId: randomUUID,
      })
      hostLifecycle.replaceSync(syncHost)
      const isolatedWorkspaceRoot = isolatedAppProofWorkspaceRoot({
        enabled: isolatedAppProofMode,
        env: process.env,
      })
      if (isolatedWorkspaceRoot !== null) syncHost.codingCatalog()?.selectWorkspace(isolatedWorkspaceRoot)
      if (smokeMode) {
        // Deterministic CUT-13 built-host fixture: real local SQLite/catalog and
        // private binding path, without provider or remote authority claims.
        syncHost.codingCatalog()?.selectWorkspace(path.join(smokeFixtureRoot, "codex-smoke"))
      }
      const restoredRoot = syncHost.codingCatalog()?.selectedRoot() ?? null
      if (restoredRoot !== null && !installAdmittedCodingWorkspace(restoredRoot)) {
        hostLifecycle.replaceWorkspace(openSelectedWorkspace(restoredRoot))
      }
    } catch {
      console.error("[openagents-desktop] local Sync persistence unavailable")
    }
    recordMainMark("syncHostOpened")
  }
  // OS-keychain custody split (2026-07-13 startup incident): the synchronous
  // vault recover stays local-only; the network verification is a SEPARATE
  // step that must never be awaited before the window exists.
  const recoverSessionVaultLocal = async (): Promise<void> => {
    if (isolatedAppProofMode) {
      desktopSessionVault = null
      desktopSessionState = "signed_out"
      console.warn("[openagents-desktop] isolated app proof: native session vault disabled (temporary user data only)")
      return
    }
    try {
      // Electron's `safeStorage` export can initialize macOS Keychain custody as
      // soon as the property is resolved. Keep the getter entirely outside the
      // isolated proof branch so a temp-only local-coding proof never opens OS
      // authorization UI; ordinary launches still require the native backend.
      const nativeSafeStorage = (await import("electron")).safeStorage
      desktopSessionVault = openDesktopSessionVault({
        filePath: path.join(app.getPath("userData"), "session", "native-session.enc"),
        safeStorage: nativeSafeStorage,
      })
      desktopSessionState = desktopSessionVault.recover().state
      recordMainMark("sessionVaultRecovered")
    } catch {
      desktopSessionVault = null
      desktopSessionState = "unavailable"
      console.error("[openagents-desktop] OS-encrypted session custody unavailable")
    }
  }
  // Network session verification (https auth-session check + token rotation).
  // Runs AFTER the window is visible; while it is in flight the renderer sees
  // the honest typed "unverified" phase and the converging chat facade
  // re-admits operations once verified Sync connects (CUT-10).
  const settleSessionRecovery = async (): Promise<void> => {
    if (desktopSessionVault === null || desktopSessionState !== "credential_present_unverified") return
    try {
      const recovery = await recoverVerifiedDesktopSession({
        vault: desktopSessionVault,
      })
      desktopSessionState = recovery.state === "verified"
        ? connectVerifiedDesktopSync() ? "session_ready" : "unavailable"
        : recovery.state
      if(recovery.state==="denied")hostLifecycle.sync()?.unlinkAccount()
    } catch {
      desktopSessionState = "unavailable"
    }
    recordMainMark("sessionRecoverySettled")
  }
  if (localTurnRestartProbe !== null) {
    // Windowless probe mode keeps the original fully-settled ordering.
    openLocalSyncPersistence()
    await recoverSessionVaultLocal()
    await settleSessionRecovery()
    runtimeGateway.start()
    try {
      await localTurnRecovery
      if (localTurnRestartProbe === "seed") {
        const store = threads()
        const thread = store.newThread()
        const key = { threadRef: thread.id, turnRef: "turn.desktop-restart-smoke", lane: "codex-local" as const }
        localTurnJournal.accept({
          ...key,
          userMessageKey: `${key.turnRef}-user`,
          assistantMessageKey: `${key.turnRef}-assistant`,
          accountRef: FIXTURE_CODEX_LOCAL_ACCOUNT.ref,
          model: "gpt-5.6-sol",
        })
        store.upsert(thread.id, {
          key: `${key.turnRef}-user`, role: "user", text: "Continue through a process restart.", timestamp: "11:55 PM",
        })
        localTurnJournal.recordDispatch(key, FIXTURE_CODEX_LOCAL_ACCOUNT.ref)
        localTurnJournal.recordProviderSession(key, {
          accountRef: FIXTURE_CODEX_LOCAL_ACCOUNT.ref,
          providerSessionRef: "thread-desktop-restart-smoke",
        })
        localTurnJournal.appendAssistantText(key, "Persisted prefix. ")
        store.upsert(thread.id, {
          key: `${key.turnRef}-assistant`, role: "assistant", text: "Persisted prefix. ", timestamp: "11:55 PM",
        })
        console.log("[openagents-desktop local-turn-restart] phase-a seeded")
        app.exit(0)
        return
      }
      const record = localTurnJournal.list().find(value => value.turnRef === "turn.desktop-restart-smoke")
      const notes = record === undefined ? [] : threads().open(record.threadRef)?.notes ?? []
      const userCount = notes.filter(note => note.key === "turn.desktop-restart-smoke-user").length
      const assistant = notes.filter(note => note.role === "assistant" && note.meta?.turnRef === "turn.desktop-restart-smoke" ||
        note.key === "turn.desktop-restart-smoke-assistant")
      const recoveryCount = notes.filter(note => note.key === "turn.desktop-restart-smoke-recovery").length
      const ok = record?.phase === "completed" && record.disposition === "resumed_after_restart" &&
        record.providerSessionRef === "thread-desktop-restart-smoke" && record.recoveryGeneration === 1 &&
        userCount === 1 && assistant.length === 2 && new Set(assistant.map(note => note.key)).size === 2 &&
        recoveryCount === 1 && assistant.map(note => note.text).join("").startsWith("Persisted prefix. ")
      console.log("[openagents-desktop local-turn-restart] phase-b", JSON.stringify({ ok, userCount, assistantCount: assistant.length, recoveryCount }))
      app.exit(ok ? 0 : 1)
      return
    } catch (error) {
      console.error("[openagents-desktop local-turn-restart] failed", error instanceof Error ? error.message : "unknown")
      app.exit(1)
      return
    }
  }
  // 2026-07-13 startup incident contract
  // (`openagents_desktop.startup.window_first_no_blank_frame.v1`): the window
  // exists — and its branded boot frame can paint — BEFORE any local database
  // open, OS-keychain custody, or session network verification. Nothing above
  // `createWindow()` on this path may touch SQLite, safeStorage, or the
  // network.
  const window = createWindow()
  recordMainMark("windowCreated")
  openLocalSyncPersistence()
  await recoverSessionVaultLocal()
  runtimeGateway.start()
  void settleSessionRecovery()
  // Boot probe round (EP250 preflight): async and non-blocking — results
  // stream into the shared health ordering, the ledger's typed reconnect
  // flags (fleet readiness), and the composer chip's availability call.
  void codexPreflight.probeAll("boot").catch(() => {})
  // Startup-marks mode (scripts/startup-bench.ts): record the milestone chain
  // and exit. Checked first because it implies smokeMode fixture wiring but must
  // NOT drive the smoke composer flow. The trace variant records the SAME
  // chain against real wiring (real userData/session/history) and must never
  // delete the profile it measured.
  if (startupMarksMode && startupMarksFile !== null) {
    runStartupMarks(window, startupMarksFile, { preserveUserData: false })
    return
  }
  if (startupTraceMode && startupTraceFile !== null) {
    runStartupMarks(window, startupTraceFile, { preserveUserData: true })
    return
  }
  // Episode 250 live-proof driver (#8712): REAL adapters (no smoke fixtures),
  // mutually exclusive with smoke. See ./live-proof.ts — additive only.
  const liveProof = resolveLiveProofConfig(process.env, app.getPath("userData"))
  const mvpProof = resolveMvpProofConfig(process.env, app.getPath("userData"))
  if (liveProof.enabled && liveProof.conflict) {
    console.error("[openagents-desktop live-proof] OPENAGENTS_DESKTOP_LIVE_PROOF and OPENAGENTS_DESKTOP_SMOKE are mutually exclusive; refusing to run either")
    app.exit(1)
    return
  }
  if (mvpProof.enabled && (mvpProof.conflict || !isolatedAppProofMode)) {
    console.error("[openagents-desktop mvp-proof] the MVP proof requires an isolated temp profile, an isolated workspace, a safe ProductSpec path, and no other driver mode")
    app.exit(1)
    return
  }
  const closeProof = (code: number): void => {
    workspaceSearchRegistry.dispose()
    terminalHost.dispose()
    hostLifecycle.dispose()
    providerAccounts.dispose()
    desktopCorrelationJournal.dispose()
    app.exit(code)
  }
  if (smokeMode) runSmoke(window)
  else if (mvpProof.enabled) {
    const workspaceRoot = isolatedAppProofWorkspaceRoot({ enabled: isolatedAppProofMode, env: process.env })
    runMvpProof(window, {
      outDir: mvpProof.outDir,
      specPath: mvpProof.specPath,
      phase: process.env.OPENAGENTS_DESKTOP_MVP_PROOF_PHASE === "restart" ? "restart" : "initial",
      verifyArtifact: packet => {
        if (workspaceRoot === null) return { ok: false, receiptRef: `receipt.mvp-proof.${packet}.unavailable` }
        const expected = `${packet} packet complete\n`
        try {
          const bytes = readFileSync(path.join(workspaceRoot, "mvp-proof", `${packet}-output.txt`), "utf8")
          const digest = createHash("sha256").update(bytes).digest("hex").slice(0, 16)
          return { ok: bytes === expected, receiptRef: `receipt.mvp-proof.${packet}.sha256.${digest}` }
        } catch {
          return { ok: false, receiptRef: `receipt.mvp-proof.${packet}.missing` }
        }
      },
      exit: closeProof,
    })
  }
  else if (liveProof.enabled) {
    runLiveProof(window, {
      outDir: liveProof.outDir,
      // Step 0 (EP250): the REAL account preflight over the real registry —
      // per-account verified/broken journal entries with reasons.
      preflight: () => codexPreflight.probeAll("live_proof"),
      exit: closeProof,
    })
  }
})

app.on("window-all-closed", () => {
  if (process.platform !== "darwin" || smokeMode) {
    app.quit()
  }
})

app.on("before-quit", () => {
  desktopIsQuitting = true
  for (const flush of localTurnFlushers) flush()
  localTurnFlushers.clear()
  workspaceSearchRegistry.dispose()
  terminalHost.dispose()
  hostLifecycle.dispose()
  providerAccounts.dispose()
  fableLocal.dispose()
  codexLocal.dispose()
  usageLedger.dispose()
  desktopCorrelationJournal.dispose()
})

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow()
  }
})
