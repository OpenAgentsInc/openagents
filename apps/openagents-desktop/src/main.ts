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
import { homedir, release as osRelease } from "node:os"
import { createHash, randomUUID } from "node:crypto"
import { cpSync, existsSync, mkdirSync, readFileSync, renameSync, rmSync, statSync, writeFileSync } from "node:fs"
import { execFile, execFileSync } from "node:child_process"
import { BrowserWindow, Menu, app, dialog, ipcMain, net, protocol, screen as electronScreen, shell, systemPreferences, utilityProcess, type IpcMainInvokeEvent, type MenuItemConstructorOptions, type Session, type WebContents } from "electron"
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
import { openDesktopUpdateStagingHost, updateRecoveryRequiresStartupExit } from "./update-staging-host.ts"
import { openMacOSUpdateApplier } from "./macos-update-applier.ts"
import { drainChildRuntimes } from "./update-runtime-drain.ts"
import { evaluateNoMigrationInvariant } from "./update-migration-evidence.ts"
import {
  DesktopUpdateStagingChannel,
  decodeDesktopUpdateStagingAction,
} from "./update-staging-contract.ts"

// macOS derives Electron safeStorage's Keychain service from the application
// name. Keep production stable while isolating unsigned development and smoke
// launches so they can never contest the signed app's Keychain ACL.
const desktopPreviewMode = !app.isPackaged && process.env.OPENAGENTS_DESKTOP_PREVIEW === "1"
const desktopPreviewLabel = /^[a-f0-9]{7,12}$/.test(process.env.OPENAGENTS_DESKTOP_PREVIEW_LABEL ?? "")
  ? process.env.OPENAGENTS_DESKTOP_PREVIEW_LABEL
  : null
const desktopApplicationName = app.isPackaged
  ? "OpenAgents"
  : desktopPreviewMode
    ? `OpenAgents Preview${desktopPreviewLabel === null ? "" : ` ${desktopPreviewLabel}`}`
    : "OpenAgents Dev"
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
  DesktopRenameLocalThreadChannel,
  DesktopThreadsChannel,
  decodeDesktopRenameLocalThreadRequest,
  decode,
  DesktopThreadRequestSchema,
  DesktopTurnRequestSchema,
  type DesktopForkHistoryThreadRequest,
  type DesktopMessage,
  type DesktopResumeLocalThreadRequest,
  type DesktopRenameLocalThreadRequest,
  type DesktopThread,
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
import {
  VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
  VISUAL_BASELINE_STATES,
  VISUAL_BASELINE_WINDOW,
  type VisualBaselineCaptureReceipt,
} from "./visual-baseline-contract.ts"
import { desktopRuntimeWorkspaceRoot } from "./desktop-runtime-workspace.ts"
import { desktopLaunchWorkspaceRoot } from "./desktop-launch-workspace.ts"
import {
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
  isClaudeModel,
  isCodexModel,
} from "./fable-local-contract.ts"
import { makeProviderLaneDispatcher, type ProviderLane } from "./provider-lane.ts"
import { makeAcpProviderLane } from "./provider-lane-acp.ts"
import {
  projectSpecLaneTurn,
  specLaneRevalidationNote,
} from "./spec-lane-workflow.ts"
import {
  ProviderLaneCapabilitiesChannel,
  projectProviderLaneCapabilities,
} from "./provider-lane-capabilities.ts"
import {
  ProviderLaneRegistryListChannel,
  ProviderLaneRegistrySelectChannel,
  decodeProviderLaneSelectRequest,
  makeProviderLaneRegistry,
  type ProviderLaneRegistryEntry,
} from "./provider-lane-registry.ts"
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
  AcpProviderActionChannel,
  AcpProviderStatusChannel,
  AcpProviderSupportExportChannel,
  decodeAcpProviderHostAction,
} from "./acp-provider-contract.ts"
import { createAcpProviderHost } from "./acp-provider-host.ts"
import { openAcpProviderPathStore } from "./acp-provider-path-store.ts"
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
  codexProviderEnvironment,
  discoverRegisteredCodexAccounts,
  defaultSpawnCodex,
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
} from "./codex-child-runtime.ts"
import { checkCodexConfiguration } from "./codex-config-health.ts"
import { CODEX_CHILD_MODEL } from "./codex-child-contract.ts"
import {
  CodexLocalAvailabilityChannel,
  CodexLocalEventChannel,
  CodexLocalInterruptChannel,
  CodexLocalQueueFollowupChannel,
  CodexLocalQueueListChannel,
  CodexLocalQueueEditChannel,
  CodexLocalQueueCancelChannel,
  decodeCodexQueueMutation,
  CodexLocalStartChannel,
  CodexLocalSteerTurnChannel,
  codexLocalFailureMessage,
  codexLocalModelNoteText,
  codexLocalRequestedModelLabel,
  CODEX_LOCAL_MODEL,
  CODEX_LOCAL_RUNTIME_COMPATIBILITY_REF,
  CodexLocalFullAutoGetChannel,
  CodexLocalFullAutoInterruptChannel,
  CodexLocalFullAutoSetChannel,
  CodexLocalFullAutoStateChannel,
  CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT,
  decodeCodexLocalContinuationProfile,
  decodeCodexLocalFullAutoGetRequest,
  decodeCodexLocalFullAutoInterruptRequest,
  decodeCodexLocalFullAutoSetRequest,
  type CodexLocalFullAutoLiveState,
} from "./codex-local-contract.ts"
import {
  FIXTURE_CODEX_LOCAL_ACCOUNT,
  fixtureCodexLocalTurnStdout,
  makeCodexLocalRuntime,
} from "./codex-local-runtime.ts"
import { makeCodexPreflight, type CodexProbeResult } from "./codex-preflight.ts"
import {
  codexRuntimeAuthority,
  publicCodexRuntimeProjection,
} from "./provider-runtime-host.ts"
import { makeCodexAppServerSmokeHarness } from "./codex-app-server-smoke-fixture.ts"
import { createCodexAppServerSupervisor } from "./codex-app-server-supervisor.ts"
import { makeCodexControlPlaneRegistry } from "./codex-control-plane.ts"
import { makeCodexEcosystemRegistry } from "./codex-ecosystem.ts"
import { makeCodexHostServiceRegistry } from "./codex-host-services.ts"
import { CodexHostRequestChannel, CodexHostSnapshotChannel, decodeCodexHostRequest } from "./codex-host-contract.ts"
import { makeCodexExperimentalRuntimeRegistry } from "./codex-experimental-runtime.ts"
import { CodexExperimentalRequestChannel, CodexExperimentalSnapshotChannel, decodeCodexExperimentalRequest } from "./codex-experimental-contract.ts"
import { makeCodexConformanceReport } from "./codex-conformance.ts"
import { CodexConformanceSnapshotChannel } from "./codex-conformance-contract.ts"
import {
  CodexEcosystemMutationChannel,
  CodexEcosystemSnapshotChannel,
  decodeCodexEcosystemMutationRequest,
} from "./codex-ecosystem-contract.ts"
import { makeCodexThreadLifecycleRegistry, type CodexThreadLifecycle } from "./codex-thread-lifecycle.ts"
import { activeCodexQueuedIntents, openCodexDurableQueue } from "./codex-durable-queue.ts"
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
import { makeDesktopCodexUsageReporter } from "./desktop-codex-usage-reporter.ts"
import { openDesktopCodexUsageOutbox } from "./desktop-codex-usage-outbox.ts"
import { makeThreadStore } from "./thread-store.ts"
import { openDesktopRuntimeControlOutcomeStore } from "./runtime-control-outcome-store.ts"
import {
  DesktopRuntimeControlOutcomeLookupChannel,
  DesktopRuntimeControlOutcomeRecordChannel,
  decodeDesktopRuntimeControlOutcomeLookup,
  decodeDesktopRuntimeControlOutcomeRecord,
} from "./runtime-control-outcome-contract.ts"
import { localRuntimePersistenceOperation } from "./local-runtime-event-persistence.ts"
import { openLocalTurnJournal } from "./local-turn-journal.ts"
import {
  filterLocallyOwnedCodexHistoryCatalog,
  filterLocallyOwnedCodexHistorySearch,
  localThreadRefForProviderSession,
  reconcileLocalTurns,
} from "./local-turn-recovery.ts"
import { openFullAutoRegistry } from "./full-auto-registry.ts"
import { applyFullAutoComposerToggle, FULL_AUTO_MAX_CONTINUATIONS, makeSerialTaskQueue, reconcileFullAutoThreads } from "./full-auto-reconcile.ts"
import { FULL_AUTO_DEFAULT_LANE, fullAutoLanePolicy, fullAutoPrompt } from "./full-auto-lane.ts"
import { makeFullAutoFollowupHandoff } from "./full-auto-followup.ts"
import { FULL_AUTO_CONTROL_PORT_ENV } from "./full-auto-control-contract.ts"
import { isFullAutoControlEnabled, startFullAutoControlServer } from "./full-auto-control-server.ts"
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
  DesktopWorkspaceWorkingDirectoryChannel,
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
import { fetchCodexReleaseNotes } from "./codex-release-notes.ts"
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
import { traceAcceptanceJourney } from "./electron-trace-acceptance.ts"
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
  type DesktopCommandDefinition,
} from "./desktop-command-contract.ts"
import {
  deferredDesktopCommand,
  dispatchNativeDesktopCommand,
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
const desktopDevServerUrl = (() => {
  if (app.isPackaged) return null
  const raw = process.env.OPENAGENTS_DESKTOP_DEV_SERVER_URL?.trim()
  if (raw === undefined || raw === "") return null
  const url = new URL(raw)
  if (url.protocol !== "http:" || url.hostname !== "127.0.0.1" || url.username !== "" || url.password !== "") {
    throw new Error("OPENAGENTS_DESKTOP_DEV_SERVER_URL must be an unauthenticated 127.0.0.1 HTTP origin")
  }
  return url
})()
const installDesktopRendererProtocol = (): void => {
  protocol.handle(DesktopRendererScheme, async request => {
    const url = new URL(request.url)
    const asset = url.hostname === "renderer" ? url.pathname.replace(/^\/+/, "") : ""
    if (desktopDevServerUrl !== null && asset !== "") {
      const targetPath = asset === "index.html" ? "/index.dev.html" : `${url.pathname}${url.search}`
      return net.fetch(new URL(targetPath, desktopDevServerUrl).toString(), {
        bypassCustomProtocolHandlers: true,
      })
    }
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
// FA-H12 (#8885): the Full Auto two-process restart probe. Same fixture-mode
// posture as the local-turn restart probe: seed phases write durable state
// and quit; resume phases relaunch against the SAME userData directory and
// observe startup reconciliation dispatch (or fail closed) for real.
const FULL_AUTO_RESTART_PROBE_PHASES = [
  "seed", "resume", "seed-mismatch", "resume-mismatch", "seed-claude", "resume-claude",
] as const
const fullAutoRestartProbe: (typeof FULL_AUTO_RESTART_PROBE_PHASES)[number] | null =
  (FULL_AUTO_RESTART_PROBE_PHASES as ReadonlyArray<string>).includes(
      process.env.OPENAGENTS_DESKTOP_FULL_AUTO_RESTART_PROBE ?? "",
    )
    ? process.env.OPENAGENTS_DESKTOP_FULL_AUTO_RESTART_PROBE as (typeof FULL_AUTO_RESTART_PROBE_PHASES)[number]
    : null
// FA-H13 (#8886): the Full Auto control live-proof probe. Windowless fixture
// mode (like the restart probes) that seeds an enabled registry record, then
// keeps the process alive so an external client can exercise the loopback
// control API against the REAL running Electron main. Only meaningful in
// combination with OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1.
const fullAutoControlProbe = process.env.OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL_PROBE === "1"
// ACP-10 (#8897): the packaged release-proof runner owns its lifecycle from a
// second process. This flag only hides the window; it does not substitute
// fixtures, seed state, change provider admission, or drive a turn in main.
const acpReleaseProofMode = process.env.OPENAGENTS_DESKTOP_ACP_RELEASE_PROOF === "1"
// QA-3 (#8908): the visual-baseline capture probe (scripts/
// visual-baseline-smoke.ts). Windowless fixture-mode posture like the probes
// above: an OFFSCREEN window renders each frozen fixture shell state
// (renderer `?visualBaseline=<name>`) and `webContents.capturePage` writes
// PNG receipts into OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS, then exits.
const visualBaselineProbe = process.env.OPENAGENTS_DESKTOP_VISUAL_BASELINE_PROBE === "1"
if (visualBaselineProbe) {
  // Deterministic rasterization: software rendering plus a forced device
  // scale of 1 keeps captured pixels stable across GPUs and Retina scales.
  app.disableHardwareAcceleration()
  app.commandLine.appendSwitch(
    "force-device-scale-factor",
    String(VISUAL_BASELINE_DEVICE_SCALE_FACTOR),
  )
}
const smokeMode = process.env.OPENAGENTS_DESKTOP_SMOKE === "1" || startupMarksMode || localTurnRestartProbe !== null ||
  fullAutoRestartProbe !== null || fullAutoControlProbe || visualBaselineProbe
const reactSmokeMode = process.env.OPENAGENTS_DESKTOP_SMOKE_REACT === "1"
const liveProofDriverMode = process.env.OPENAGENTS_DESKTOP_LIVE_PROOF === "1"
const mvpProofDriverMode = process.env.OPENAGENTS_DESKTOP_MVP_PROOF === "1"
// Automated smoke/benchmark/proof drivers must never steal the operator's
// screen. Headed presentation is one explicit, manual-only opt-in for the
// narrow cases that genuinely need a visible native-window observation.
const hiddenAutomationMode = (
  smokeMode || startupTraceMode || liveProofDriverMode || mvpProofDriverMode || acpReleaseProofMode
) && process.env.OPENAGENTS_DESKTOP_HEADED !== "1"
// Capture before any host lifecycle can change process state. Launchers that
// need to enter their own managed source tree first preserve the user's
// original directory in OPENAGENTS_DESKTOP_LAUNCH_CWD. A direct executable
// launch naturally falls back to process.cwd(). The host validates both and
// never exposes the absolute root to the renderer.
const desktopLaunchWorkingDirectory = desktopLaunchWorkspaceRoot({
  explicitRoot: process.env.OPENAGENTS_DESKTOP_LAUNCH_CWD,
  processWorkingDirectory: process.cwd(),
  homeRoot: app.getPath("home"),
  isDirectory: candidate => {
    try { return statSync(candidate).isDirectory() } catch { return false }
  },
})
const productionUserDataPath = path.join(app.getPath("appData"), "OpenAgents")
const developmentUserDataPath = path.join(app.getPath("appData"), "OpenAgents Dev")
const legacyDevelopmentUserDataPath = path.join(app.getPath("appData"), "OpenAgentsDesktopDev")
if (desktopDevServerUrl === null && !smokeMode && !liveProofDriverMode && !mvpProofDriverMode && process.env.OPENAGENTS_DESKTOP_USER_DATA === undefined &&
    !existsSync(productionUserDataPath) && existsSync(legacyDevelopmentUserDataPath)) {
  try {
    // Same-parent rename preserves the complete durable profile atomically.
    // Failure is non-destructive: production still starts at its canonical
    // path and the legacy directory remains untouched for manual recovery.
    renameSync(legacyDevelopmentUserDataPath, productionUserDataPath)
  } catch { /* retain the legacy profile without deleting or partially copying it */ }
}
const desktopUserDataPath = process.env.OPENAGENTS_DESKTOP_USER_DATA ?? (
  desktopDevServerUrl !== null
    ? developmentUserDataPath
    : smokeMode || liveProofDriverMode || mvpProofDriverMode
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
if (desktopPreviewMode && !isolatedAppProofMode) {
  throw new Error("OpenAgents Desktop preview requires an isolated OS-temporary userData profile")
}
// The installed application uses React by default. Existing broad smoke keeps
// the explicit compatibility oracle until its specialist surfaces are ported;
// `smoke:react` exercises the installed default backend itself.
const desktopRendererEntry = smokeMode && !reactSmokeMode
  ? `${desktopRendererEntryUrl}?renderer=compatibility`
  : desktopRendererEntryUrl
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
  if (hiddenAutomationMode) return
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
  // MVP is Codex-only. Do not scan or transfer a Claude graph that every
  // current renderer projection discards; merged-history remains an internal
  // future-phase capability, outside the startup path.
  catalog: () => hostLifecycle.history()!.run({ kind: "history_catalog", sessionsRoot: codexSessionsRoot(), claudeRoot: null }) as Promise<import("./codex-history-contract.ts").CodexHistoryCatalog>,
  page: (threadRef, offset, limit) => hostLifecycle.history()!.run({ kind: "history_page", sessionsRoot: codexSessionsRoot(), claudeRoot: null, threadRef, offset, limit }) as Promise<import("./codex-history-contract.ts").CodexHistoryPage | null>,
  search: (query, limit) => hostLifecycle.history()!.run({ kind: "history_search", sessionsRoot: codexSessionsRoot(), claudeRoot: null, query, limit }) as Promise<import("./codex-history-contract.ts").CodexHistorySearchResponse>,
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
  // Desktop Codex is application-owned: status comes from the exact authority
  // used by turns and repair means updating/reinstalling OpenAgents. Desktop
  // never probes the ambient Claude/OpenCode maintenance catalog.
  status: async () => {
    const codexResolution = await codexRuntimeAuthority.inspect()
    const codex = publicCodexRuntimeProjection(codexResolution)
    return {
      observedAt: new Date().toISOString(),
      harnesses: [{
          harness: "codex" as const,
          installed: codexResolution.executablePath !== null,
          installedVersion: codex.observedVersion,
          latestVersion: codex.expectedVersion,
          channel: "desktop-bundle" as const,
          advisory: codex.compatible ? "current" as const : "unknown" as const,
          updateSupported: !codex.compatible,
          runtimeState: codex.state,
          recoveryMessage: codex.recoveryMessage,
        }],
      codexReleaseNotes: await fetchCodexReleaseNotes(codex.expectedVersion).then(notes =>
        notes === null ? null : {
          version: notes.version,
          title: notes.title,
          body: notes.body,
          publishedAt: notes.publishedAt,
        }),
    }
  },
  update: async () => {
    const resolution = await codexRuntimeAuthority.inspect()
    const update = resolution.state === "ready" ? null : await desktopUpdateHost.check()
    return {
      outcome: resolution.state === "ready" ? "already_current" as const : "failed" as const,
      failureReason: resolution.state === "ready"
        ? null
        : update?.phase === "available" || update?.phase === "staged"
          ? "repair_openagents_update_available"
          : "repair_openagents",
      beforeVersion: resolution.observedVersion,
      afterVersion: resolution.observedVersion,
      receiptId: null,
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
const runtimeControlOutcomes = openDesktopRuntimeControlOutcomeStore(
  path.join(app.getPath("userData"), "runtime-control-outcomes", "ledger.json"),
)
ipcMain.handle(DesktopRuntimeControlOutcomeRecordChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { status: "rejected", reason: "invalid_request" }
  const request = decodeDesktopRuntimeControlOutcomeRecord(value)
  return request === null
    ? { status: "rejected", reason: "invalid_request" }
    : runtimeControlOutcomes.record(request)
})
ipcMain.handle(DesktopRuntimeControlOutcomeLookupChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { status: "rejected", reason: "invalid_request" }
  const request = decodeDesktopRuntimeControlOutcomeLookup(value)
  return request === null
    ? { status: "rejected", reason: "invalid_request" }
    : runtimeControlOutcomes.lookup(request)
})
const localTurnJournal = openLocalTurnJournal(
  path.join(app.getPath("userData"), "local-turns", "journal.json"),
)
const fullAutoRegistry = openFullAutoRegistry(
  path.join(app.getPath("userData"), "full-auto", "registry.json"),
)
const providerLaneRegistry = makeProviderLaneRegistry({
  file: path.join(app.getPath("userData"), "provider-lanes", "registry.json"),
})
const providerLaneAuthentication = new Map<string, "ready" | "missing" | "unknown">([
  ["fable-local", "unknown"],
  ["codex-local", "unknown"],
])
/** Full Auto (#8853): broadcasts an updated thread the same way local turn
 * recovery already does, so any open window's existing localTurnRecovery
 * subscription picks up a background continuation without new renderer wiring. */
const broadcastFullAutoThreadUpdate = (threadRef: string): void => {
  const thread = threads().open(threadRef)
  if (thread === null) return
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) window.webContents.send(DesktopLocalTurnRecoveryUpdateChannel, thread)
  }
}
const fullAutoProgressTimers = new Map<string, ReturnType<typeof setTimeout>>()
const scheduleFullAutoThreadUpdate = (threadRef: string): void => {
  if (fullAutoProgressTimers.has(threadRef)) return
  fullAutoProgressTimers.set(threadRef, setTimeout(() => {
    fullAutoProgressTimers.delete(threadRef)
    broadcastFullAutoThreadUpdate(threadRef)
  }, 75))
}
/**
 * FA-H4 (#8877): main-owned in-memory coarse live state per Full Auto
 * thread. A background continuation dispatches with `sender: null`, so no
 * live turn events reach any renderer; this map plus its broadcast are the
 * ONLY renderer-visible signal that a background turn is running right now
 * (with its interruptible turn ref) or how the last one ended. Terminal
 * states persist in the map until the next transition — the renderer keeps
 * showing the last outcome instead of snapping to a fabricated idle.
 * Deliberately NOT durable: after a restart the startup reconciliation
 * re-derives reality (either a fresh dispatch re-enters turn_running, or the
 * blocked/cap notes already persisted on the thread itself).
 */
const fullAutoLiveState = new Map<string, Readonly<{
  state: CodexLocalFullAutoLiveState
  turnRef: string | null
  detail?: string
}>>()
const setFullAutoLiveState = (
  threadRef: string,
  state: CodexLocalFullAutoLiveState,
  turnRef: string | null,
  detail?: string,
): void => {
  const bounded = detail === undefined || detail.trim() === ""
    ? undefined
    : detail.slice(0, CODEX_LOCAL_FULL_AUTO_DETAIL_LIMIT)
  const entry = { state, turnRef, ...(bounded === undefined ? {} : { detail: bounded }) } as const
  fullAutoLiveState.set(threadRef, entry)
  // Same all-windows loop shape as broadcastFullAutoThreadUpdate above.
  for (const window of BrowserWindow.getAllWindows()) {
    if (!window.isDestroyed()) window.webContents.send(CodexLocalFullAutoStateChannel, { threadRef, ...entry })
  }
}
const codexDurableQueue = openCodexDurableQueue(
  path.join(app.getPath("userData"), "codex-turn-queue", "queue.json"),
)
const codexHandoffBindings = openCodexHandoffBindings(
  path.join(app.getPath("userData"), "codex-handoff", "bindings.json"),
)
const desktopUpdateRoot = path.join(app.getPath("userData"), "updates")
const desktopUpdateChannel = app.getVersion().includes("-rc.") ? "rc" : "stable"
const desktopHostVersion = (): string => {
  if (process.platform === "darwin") {
    try { return execFileSync("/usr/bin/sw_vers", ["-productVersion"], { encoding: "utf8" }).trim() }
    catch { return "0" }
  }
  if (process.platform === "linux") {
    const report = process.report?.getReport() as { header?: { glibcVersionRuntime?: unknown } }
    const glibc = report.header?.glibcVersionRuntime
    return typeof glibc === "string" ? `glibc ${glibc}` : "glibc 0"
  }
  return osRelease()
}
const desktopUpdateApplier = openMacOSUpdateApplier({
  root: desktopUpdateRoot,
  installedAppPath: path.resolve(path.dirname(app.getPath("exe")), "../.."),
  installedVersion: app.getVersion(),
  channel: desktopUpdateChannel,
  packaged: app.isPackaged,
  targetArchitecture: process.platform === "darwin" && app.runningUnderARM64Translation
    ? "arm64"
    : process.arch === "arm64" || process.arch === "x64" ? process.arch : undefined,
})
let desktopUpdateDrainActive = false
const drainDesktopUpdateRuntimes = async () => {
  desktopUpdateDrainActive = true
  return await drainChildRuntimes({
    timeoutMs: 15_000,
    drainers: [
      { kind: "agent", drain: async () => { fableLocal.dispose(); codexLocal.dispose(); await acpProviderHost.shutdown() } },
      { kind: "pty", drain: () => terminalHost.dispose() },
      { kind: "local_server", drain: () => { codexControlPlanes.close(); codexThreadLifecycles.close(); codexEcosystems.close(); codexHostServices.close(); codexExperimentalRuntimes.close(); codexAppServerSupervisor.close() } },
      { kind: "helper", drain: () => {
        for (const flush of localTurnFlushers) flush()
        localTurnFlushers.clear()
        workspaceSearchRegistry.dispose()
        hostLifecycle.dispose()
        providerAccounts.dispose()
        codexDurableQueue.close()
        usageLedger.dispose()
        desktopCorrelationJournal.dispose()
        runtimeGateway.dispose()
      } },
      { kind: "window", drain: () => { for (const window of BrowserWindow.getAllWindows()) window.destroy() } },
      { kind: "wsl", drain: () => process.platform !== "win32" ? undefined : new Promise<void>((resolve, reject) => {
        execFile("C:\\Windows\\System32\\wsl.exe", ["--shutdown"], error => error === null ? resolve() : reject(error))
      }) },
    ],
  })
}
const desktopUpdateHost = openDesktopUpdateStagingHost({
  root: desktopUpdateRoot,
  installedVersion: app.getVersion(),
  channel: desktopUpdateChannel,
  platform: process.platform === "darwin" || process.platform === "win32" || process.platform === "linux"
    ? process.platform
    : undefined,
  hostArchitecture: process.platform === "darwin" && app.runningUnderARM64Translation
    ? "arm64"
    : process.arch === "arm64" || process.arch === "x64" ? process.arch : undefined,
  applicationArchitecture: process.arch === "arm64" || process.arch === "x64" ? process.arch : undefined,
  hostVersion: desktopHostVersion(),
  openPath: artifactPath => shell.openPath(artifactPath),
  applier: desktopUpdateApplier,
  drainChildren: drainDesktopUpdateRuntimes,
  migrationEvidence: () => evaluateNoMigrationInvariant({
    installedApplicationRoot: path.resolve(path.dirname(app.getPath("exe")), "../.."),
    categoryRoots: {
      sessions: codexSessionsRoot(),
      vaultRefs: path.join(app.getPath("userData"), "session", "native-session.enc"),
      settings: app.getPath("userData"),
      drafts: path.join(app.getPath("userData"), "sync", "khala-sync.sqlite"),
    },
    categoryKinds: { sessions: "directory", vaultRefs: "file", settings: "directory", drafts: "file" },
    absentDispositions: {
      sessions: "no_sessions",
      ...(desktopSessionState === "signed_out" ? { vaultRefs: "signed_out" as const } : {}),
    },
  }),
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
let codexLifecycleAuthority: (() => Promise<CodexThreadLifecycle>) | null = null
const authoritativeCodexHistoryHost: import("./codex-history-host.ts").CodexHistoryHost = {
  run: request => {
    if (smokeMode) return codexHistoryHost.run(request)
    const authority = codexLifecycleAuthority
    if (authority !== null) return authority().then(async lifecycle => {
      const result = await lifecycle.runHistory(request)
      if (result === null) return null
      const localThreadRefs = new Set(threads().list().map(thread => thread.id))
      if (request.kind === "history_catalog") {
        return filterLocallyOwnedCodexHistoryCatalog(
          result as import("./codex-history-contract.ts").CodexHistoryCatalog,
          localTurnJournal.list(),
          localThreadRefs,
        )
      }
      if (request.kind === "history_search") {
        return filterLocallyOwnedCodexHistorySearch(
          result as import("./codex-history-contract.ts").CodexHistorySearchResponse,
          localTurnJournal.list(),
          localThreadRefs,
        )
      }
      return result
    }).catch(error => {
      console.error("[openagents-desktop] app-server history unavailable", error instanceof Error ? error.name : "unknown")
      return process.env.OPENAGENTS_DESKTOP_CODEX_ROLLOUT_FALLBACK === "1"
        ? (console.warn("[openagents-desktop] using labeled Codex rollout migration fallback"), codexHistoryHost.run(request))
        : null
    })
    return Promise.resolve(null)
  },
  dispose: () => codexHistoryHost.dispose(),
}
const hostLifecycle = makeDesktopHostLifecycle({
  runtime: runtimeGateway,
  account: codexConnect,
  history: authoritativeCodexHistoryHost,
})
const voiceMedia: VoiceNativeMedia = smokeMode
  ? {
      open: input => {
        let captureEnabled = true
        queueMicrotask(() => {
          input.onState("live")
          input.onControl({ kind: "activity", activity: "listening" })
          input.onControl({ kind: "transcript", utteranceRef: "smoke.utterance.1", text: "Open chat conversation", final: false })
          setTimeout(() => input.onControl({ kind: "transcript", utteranceRef: "smoke.utterance.1", text: "Open chat conversation", final: true }), 800)
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
// Local file authority begins with the directory the owner explicitly launched
// the app from, or with a later directory-picker choice. The launch root is
// captured and validated in main; renderer input never selects an absolute
// path.
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
  const currentRoot = workspaceSnapshot()?.root
  const result = await dialog.showOpenDialog({
    properties: ["openDirectory", "createDirectory"],
    ...(currentRoot === undefined ? {} : { defaultPath: currentRoot }),
  })
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
ipcMain.handle(DesktopWorkspaceWorkingDirectoryChannel, (event) => {
  if (!isTrustedRuntimeGatewaySender(event)) return null
  return workspaceSnapshot()?.root ?? null
})
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
  const selectedRoot = await chooseCodingWorkspace()
  return selectedRoot === null ? null : workspaceSnapshot()
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
ipcMain.handle(DesktopNewThreadChannel, (_event, value: unknown) => {
  const requestedLane = typeof value === "object" && value !== null &&
    typeof (value as { laneRef?: unknown }).laneRef === "string"
    ? (value as { laneRef: string }).laneRef
    : "codex-local"
  const thread = threads().newThread()
  providerLaneRegistry.bind(thread.id, requestedLane)
  return thread
})
// H1 resume picker: app-local threads only. Returning the exact persisted
// thread id lets the next local turn hit fable/codex-local's existing
// per-thread SDK resume seam; imported provider history is never mutated.
ipcMain.handle(DesktopLocalThreadsChannel, () => threads().list())
ipcMain.handle(DesktopRenameLocalThreadChannel, (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { ok: false, error: "Rename is unavailable from this frame." }
  const request = decodeDesktopRenameLocalThreadRequest(value) as DesktopRenameLocalThreadRequest | null
  if (request === null) return { ok: false, error: "Enter a title before saving." }
  try {
    const thread = threads().rename(request.threadRef, request.title)
    return thread === null
      ? { ok: false, error: "That conversation could not be renamed." }
      : { ok: true, thread }
  } catch (error) {
    console.error("[openagents-desktop] local thread rename failed", error instanceof Error ? error.name : "unknown")
    return { ok: false, error: "The conversation title could not be saved." }
  }
})
ipcMain.handle(DesktopResumeLocalThreadChannel, async (_event, value: unknown) => {
  const request = decode(DesktopResumeLocalThreadRequestSchema, value) as DesktopResumeLocalThreadRequest | null
  if (request === null) return null
  const store = threads()
  const direct = store.open(request.threadRef)
  if (direct !== null) return direct

  // Codex history rows use the provider-native thread id, not the Desktop
  // store id. Recover that identity from the durable turn journal before
  // deciding this is provider-only history. This is also the restart path for
  // a Desktop-owned chat that aged out of the five-row local cache.
  const localThreadRef = localThreadRefForProviderSession(
    localTurnJournal.list(),
    request.threadRef,
  )
  if (localThreadRef === null) return null
  const cached = store.open(localThreadRef)
  if (cached !== null) return cached

  // The bounded local store may have evicted the transcript even though its
  // journal still proves exact provider continuity. Re-read the provider-owned
  // transcript in main, translate only its verified identity, and re-admit it
  // to the local cache. The renderer never supplies transcript content.
  const history = hostLifecycle.history()
  if (history === null) return null
  const providerThread = await history.run({
    kind: "detail",
    sessionsRoot: codexSessionsRoot(),
    id: request.threadRef,
    messageLimit: 80,
  }) as DesktopThread | null
  if (providerThread === null) return null
  return store.restoreThread({
    ...providerThread,
    id: localThreadRef,
    // Selection is an access, so keep the restored entry in the bounded LRU
    // instead of immediately evicting it again for its historical timestamp.
    updatedAt: new Date().toISOString(),
  })
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
// Owner-approved at 8809f79b56 (#8911). The control now ships in ordinary
// builds, while the durable user preference remains independently default-off
// and revocable. No credential read or network request occurs until opt-in.
const desktopUsageConsentControlAvailable = true
const preferencesStore = openDesktopPreferencesStore(
  path.join(app.getPath("userData"), "preferences.json"),
)
const desktopCodexUsageOutbox = openDesktopCodexUsageOutbox(
  path.join(app.getPath("userData"), "usage", "codex-outbox.json"),
)
const desktopCodexUsageReporter = makeDesktopCodexUsageReporter({
  consentEnabled: () =>
    desktopUsageConsentControlAvailable &&
    preferencesStore.snapshot().privacy.shareLocalCodexUsage,
  sessionReady: () => desktopSessionState === "session_ready",
  credential: () => desktopSessionVault?.load() ?? null,
  outbox: desktopCodexUsageOutbox,
  baseUrl: process.env.OPENAGENTS_COM_BASE_URL ?? "https://openagents.com",
})
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
const codexAppServerSupervisor = createCodexAppServerSupervisor({
  nativeJournalRoot: path.join(app.getPath("userData"), "codex-native"),
  reverseRpcJournalPath: path.join(app.getPath("userData"), "codex-reverse-rpc", "receipts.json"),
  strictGeneratedDecoding: true,
})
const codexControlPlanes = makeCodexControlPlaneRegistry({
  supervisor: codexAppServerSupervisor,
  receiptRoot: path.join(app.getPath("userData"), "codex-control-plane"),
})
const codexThreadLifecycles = makeCodexThreadLifecycleRegistry({
  supervisor: codexAppServerSupervisor,
  receiptRoot: path.join(app.getPath("userData"), "codex-thread-lifecycle"),
})
const codexEcosystems = makeCodexEcosystemRegistry({
  supervisor: codexAppServerSupervisor,
  roots: () => {
    const authority = currentProductSpecWorkroom()
    return authority === null ? [] : [authority.workspaceRoot]
  },
  authorizeWorkContext: workContextRef => currentProductSpecWorkroom()?.workContextRef === workContextRef,
  authorizeRoot: (root, workContextRef) => {
    const authority = currentProductSpecWorkroom()
    if (authority === null || authority.workContextRef !== workContextRef) return false
    const relative = path.relative(path.resolve(authority.workspaceRoot), path.resolve(root))
    return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative))
  },
  authorizeNamespace: namespace => namespace === "productspec",
})
const codexHostServices = makeCodexHostServiceRegistry({
  supervisor: codexAppServerSupervisor,
  spoolRoot: path.join(app.getPath("userData"), "codex-host-spool"),
  receiptRoot: path.join(app.getPath("userData"), "codex-host-receipts"),
})
const codexExperimentalRuntimes = makeCodexExperimentalRuntimeRegistry({
  supervisor: codexAppServerSupervisor,
  spoolRoot: path.join(app.getPath("userData"), "codex-experimental-spool"),
  receiptRoot: path.join(app.getPath("userData"), "codex-experimental-receipts"),
})
codexLifecycleAuthority = async () => {
  const binary = codexRuntimeAuthority.executable()
  if (binary === null) throw new Error("Codex runtime is unavailable")
  const runtimeCwd = path.join(app.getPath("userData"), "fable-local", "codex-app-server-runtime")
  mkdirSync(runtimeCwd, { recursive: true })
  return codexThreadLifecycles.forTarget({
    binary,
    env: codexProviderEnvironment(process.env, { clearCodexHome: true }),
    cwd: runtimeCwd,
    accountRef: "codex-current",
    hostTarget: "local-desktop",
  })
}
const currentCodexEcosystem = async () => {
  const binary = codexRuntimeAuthority.executable()
  if (binary === null) throw new Error("Codex runtime is unavailable")
  const runtimeCwd = path.join(app.getPath("userData"), "fable-local", "codex-app-server-runtime")
  mkdirSync(runtimeCwd, { recursive: true })
  return codexEcosystems.forTarget({ binary, env: codexProviderEnvironment(process.env, { clearCodexHome: true }), cwd: runtimeCwd, accountRef: "codex-current", hostTarget: "local-desktop" })
}
const currentCodexHostServices = async () => {
  const binary = codexRuntimeAuthority.executable()
  const workroom = currentProductSpecWorkroom()
  if (binary === null || workroom === null) throw new Error("Codex runtime and WorkContext are required")
  const runtimeCwd = path.join(app.getPath("userData"), "fable-local", "codex-app-server-runtime")
  mkdirSync(runtimeCwd, { recursive: true })
  return codexHostServices.forTarget({ binary, env: codexProviderEnvironment(process.env, { clearCodexHome: true }), cwd: runtimeCwd, accountRef: "codex-current", hostTarget: "local-desktop" }, workroom.workspaceRoot)
}
const currentCodexExperimentalRuntime = async () => {
  const binary = codexRuntimeAuthority.executable()
  if (binary === null) throw new Error("Codex runtime is unavailable")
  const runtimeCwd = path.join(app.getPath("userData"), "fable-local", "codex-app-server-runtime")
  mkdirSync(runtimeCwd, { recursive: true })
  return codexExperimentalRuntimes.forTarget({ binary, env: codexProviderEnvironment(process.env, { clearCodexHome: true }), cwd: runtimeCwd, accountRef: "codex-current", hostTarget: "local-desktop" })
}
const codexAppServerConfig = {
  binary: codexRuntimeAuthority.executable,
  supervisor: codexAppServerSupervisor,
  ...(!smokeMode ? { controlPlanes: codexControlPlanes } : {}),
  ...(!smokeMode ? { ecosystems: codexEcosystems } : {}),
  turnReceiptPath: (account: import("./codex-child-runtime.ts").CodexChildAccount, threadRef: string) =>
    path.join(app.getPath("userData"), "codex-turn-admission", `${createHash("sha256").update(`${account.ref}\0${threadRef}`).digest("hex")}.json`),
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
  // ProductSpec is intentionally absent from the MVP UI. Re-enable this only
  // with a typed per-turn admitted-context binding; workspace selection alone
  // is not ProductSpec authority.
  productSpecEnabled: () => false,
  productSpecDynamicTools: ProductSpecDynamicTools,
  onProductSpecToolCall: async (request: import("./codex-app-server-client.ts").CodexAppServerRequest) => {
    const authority = currentProductSpecWorkroom()
    return handleProductSpecDynamicTool(request, authority === null
      ? null
      : { workContextRef: authority.workContextRef, service: authority.service })
  },
}
const codexAppServerSmoke = reactSmokeMode ? makeCodexAppServerSmokeHarness() : null
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
  console.log(reactSmokeMode
    ? "[openagents-desktop] codex-local running in SMOKE FIXTURE mode (app-server protocol, no real spawn)"
    : "[openagents-desktop] codex-local running in SMOKE FIXTURE mode (scripted codex exec, no real spawn)")
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
      configCheck: async (account: { home: string; source?: "current_session" | "pylon" }) => {
        const configEnv = { ...process.env } as Record<string, string | undefined>
        if (account.source === "current_session") delete configEnv.CODEX_HOME
        else configEnv.CODEX_HOME = account.home
        return checkCodexConfiguration({
          spawn: defaultSpawnCodex,
          env: configEnv,
          cwd: account.home,
          autoRepair: true,
        })
      },
    } : {}),
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
        discoverImpl: async () => reactSmokeMode
          ? [{ ...FIXTURE_CODEX_LOCAL_ACCOUNT, source: "current_session" as const }]
          : [
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
/**
 * FA-H2 (#8875): the ONE workspace resolution both local runtimes execute
 * against. Full Auto binds this exact value onto the durable record at enable
 * time (main resolves it itself -- a renderer-supplied path is never trusted)
 * and reconciliation refuses to dispatch a continuation when the current
 * resolution no longer matches what was granted.
 */
const resolveDesktopLocalWorkspaceRoot = (): string => desktopRuntimeWorkspaceRoot({
  fixtureMode: smokeMode || liveProofDriverMode,
  userDataPath: app.getPath("userData"),
  selectedWorkspaceRoot: selectedDesktopWorkspaceRoot(),
  launchFallbackRoot: desktopLaunchWorkingDirectory,
})
// Codex local chat lane: the composer's Codex chip uses the pinned app-server
// against the user's ordinary logged-in Codex session with durable
// thread-resume continuity. Pylon accounts are fleet-only, not MVP fallback.
// Smoke alone keeps the legacy scripted JSON fixture parser.
const codexLocal = makeCodexLocalRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  workspaceRoot: resolveDesktopLocalWorkspaceRoot,
  preflight: codexPreflight,
  durableQueue: codexDurableQueue,
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
  ...(!smokeMode
    ? { appServer: codexAppServerConfig }
    : reactSmokeMode && codexAppServerSmoke !== null
      ? {
          appServer: { ...codexAppServerConfig, binary: () => "/packaged/codex", spawnImpl: codexAppServerSmoke.spawn },
          discoverImpl: async () => [{ ...FIXTURE_CODEX_LOCAL_ACCOUNT, source: "current_session" as const }],
        }
      : {}),
  ...(smokeMode && !reactSmokeMode
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
  workspaceRoot: resolveDesktopLocalWorkspaceRoot,
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
  codexState: async threadId => {
    const lifecycle = await (codexLifecycleAuthority?.() ?? Promise.reject(new Error("Codex lifecycle unavailable")))
    await lifecycle.read(threadId, true)
    const turns = await lifecycle.pageTurns(threadId)
    const last = turns.at(-1)
    const statusValue = last !== null && typeof last === "object" ? (last as Record<string, unknown>).status : null
    const status = typeof statusValue === "string"
      ? statusValue
      : statusValue !== null && typeof statusValue === "object" && typeof (statusValue as Record<string, unknown>).type === "string"
        ? (statusValue as Record<string, unknown>).type as string
        : "unknown"
    return status === "completed" ? "completed"
      : status === "inProgress" || status === "running" ? "running"
        : status === "interrupted" || status === "failed" || status === "cancelled" ? "interrupted"
          : "unknown"
  },
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
const projectedDesktopPreferences = () => {
  const preferences = preferencesStore.snapshot()
  return {
    ...preferences,
    privacy: {
      ...preferences.privacy,
      localCodexUsageControlAvailable: desktopUsageConsentControlAvailable,
    },
  }
}
ipcMain.handle(DesktopPreferencesGetChannel, projectedDesktopPreferences)
ipcMain.handle(DesktopPreferencesUpdateChannel, (_event, value: unknown) =>
  {
    const preferences = preferencesStore.update(decodeDesktopPreferencesPatch(value))
    if (!preferences.privacy.shareLocalCodexUsage) desktopCodexUsageOutbox.clear()
    else Effect.runFork(desktopCodexUsageReporter.flush())
    return {
      ...preferences,
      privacy: {
        ...preferences.privacy,
        localCodexUsageControlAvailable: desktopUsageConsentControlAvailable,
      },
    }
  })
ipcMain.handle(DesktopPreferencesResetChannel, () => {
  const preferences = preferencesStore.reset()
  desktopCodexUsageOutbox.clear()
  return preferences
})

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
const acpProviderPathStore = openAcpProviderPathStore(path.join(app.getPath("userData"), "acp", "provider-paths.json"))
const acpProviderHost = createAcpProviderHost({
  cwd: async () => resolveDesktopLocalWorkspaceRoot(),
  loadAlternatePaths: async () => {
    await acpProviderPathStore.load()
    const grok = acpProviderPathStore.get("grok")
    const cursor = acpProviderPathStore.get("cursor")
    return { ...(grok === undefined ? {} : { grok }), ...(cursor === undefined ? {} : { cursor }) }
  },
  saveAlternatePath: (provider, candidate) => acpProviderPathStore.save(provider, candidate),
  chooseExecutable: async provider => {
    if (smokeMode || liveProofDriverMode) return null
    const result = await dialog.showOpenDialog({
      title: `Choose ${provider === "grok" ? "Grok CLI" : "Cursor Agent CLI"} executable`,
      properties: ["openFile"],
      buttonLabel: "Probe executable",
    })
    return result.canceled ? null : result.filePaths[0] ?? null
  },
})
let acpProviderInitialization: Promise<unknown> | null = null
const ensureAcpProviders = (): Promise<unknown> => {
  acpProviderInitialization ??= acpProviderHost.initialize()
  return acpProviderInitialization
}
ipcMain.handle(AcpProviderStatusChannel, async () => {
  await ensureAcpProviders()
  return acpProviderHost.status()
})
ipcMain.handle(AcpProviderActionChannel, async (_event, value: unknown) => {
  const request = decodeAcpProviderHostAction(value)
  if (request === null) return { state: "unavailable", message: "Invalid ACP provider action." }
  await ensureAcpProviders()
  return acpProviderHost.action(request.provider, request.action)
})
ipcMain.handle(AcpProviderSupportExportChannel, async () => {
  await ensureAcpProviders()
  try {
    const directory = path.join(app.getPath("userData"), "diagnostics")
    mkdirSync(directory, { recursive: true, mode: 0o700 })
    const destination = path.join(directory, "openagents-acp-support.json")
    writeFileSync(destination, `${JSON.stringify(acpProviderHost.supportBundle(), null, 2)}\n`, { mode: 0o600 })
    return { ok: true, notice: "Redacted ACP support bundle exported." }
  } catch {
    return { ok: false, notice: "ACP support bundle export failed." }
  }
})

const diagnosticsHost = makeDiagnosticsHost({
  collectInputs: collectDiagnosticsInputs,
  exportDir: path.join(app.getPath("userData"), "diagnostics"),
  recovery: {
    // Safe, typed recovery: re-probe every connected provider account, then
    // the renderer re-gathers so readiness flips without a restart.
    reprobe_providers: async () => {
      try {
        await Promise.all([codexPreflight.probeAll("diagnostics_recovery"), acpProviderHost.initialize()])
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

ipcMain.handle(FableLocalAvailabilityChannel, async () => {
  const availability = await fableLocal.availability()
  providerLaneAuthentication.set("fable-local", availability.state === "available" ? "ready" : "missing")
  return availability
})
// Image file picker (capability I1): open the native dialog in MAIN, read the
// chosen images from disk here (never the renderer), bound size + count, and
// return decoded base64 attachments plus the first honest rejection. Smoke and
// live-proof headless runs cannot open a dialog; the picker returns no images.
ipcMain.handle(FableLocalPickImagesChannel, async (event) => {
  if (smokeMode || liveProofDriverMode) return { images: [], rejection: null }
  const window = BrowserWindow.fromWebContents(event.sender)
  const extensions = FABLE_LOCAL_IMAGE_MEDIA_TYPES.map(type =>
    type === "image/jpeg" ? "jpg" : type.slice("image/".length))
  const result = await (window === null
    ? dialog.showOpenDialog({ properties: ["openFile", "multiSelections"], filters: [{ name: "Images", extensions: [...extensions, "jpeg"] }] })
    : dialog.showOpenDialog(window, { properties: ["openFile", "multiSelections"], filters: [{ name: "Images", extensions: [...extensions, "jpeg"] }] }))
  if (result.canceled) return { images: [], rejection: null }
  const mediaTypeForPath = (filePath: string): (typeof FABLE_LOCAL_IMAGE_MEDIA_TYPES)[number] | null => {
    const lower = filePath.toLowerCase()
    if (lower.endsWith(".png")) return "image/png"
    if (lower.endsWith(".jpg") || lower.endsWith(".jpeg")) return "image/jpeg"
    if (lower.endsWith(".webp")) return "image/webp"
    if (lower.endsWith(".gif")) return "image/gif"
    return null
  }
  const attachments: Array<{ mediaType: string; data: string; name: string }> = []
  let rejection: "wrong_type" | "too_large" | "count_limit" | "unreadable" | null =
    result.filePaths.length > FABLE_LOCAL_IMAGE_COUNT_LIMIT ? "count_limit" : null
  for (const filePath of result.filePaths.slice(0, FABLE_LOCAL_IMAGE_COUNT_LIMIT)) {
    const mediaType = mediaTypeForPath(filePath)
    if (mediaType === null) {
      rejection ??= "wrong_type"
      continue
    }
    try {
      const sizeBytes = statSync(filePath).size
      if (sizeBytes <= 0 || sizeBytes > FABLE_LOCAL_IMAGE_BYTES_LIMIT) {
        rejection ??= "too_large"
        continue
      }
      const bytes = readFileSync(filePath)
      if (bytes.length === 0 || bytes.length > FABLE_LOCAL_IMAGE_BYTES_LIMIT) {
        rejection ??= "too_large"
        continue
      }
      attachments.push({ mediaType, data: bytes.toString("base64"), name: path.basename(filePath).slice(0, 256) })
    } catch {
      rejection ??= "unreadable"
    }
  }
  return { images: attachments, rejection }
})
ipcMain.handle(FableLocalInterruptChannel, (_event, value: unknown) => {
  const request = decodeFableLocalInterruptRequest(value)
  return request === null
    ? false
    : fableLocal.interrupt(request.turnRef) ||
      grokAcpDriver.interrupt(request.turnRef) ||
      cursorAcpDriver.interrupt(request.turnRef)
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
// ---------------------------------------------------------------------------
// Provider lane SPI (L1 #8899, epic #8898): every local agent lane dispatches
// through ONE shared engine (`makeProviderLaneDispatcher`), and each lane is a
// plain typed adapter value implementing `ProviderLane` — dispatch, the frozen
// stream envelope, interrupt, journal-mirrored recovery, exact usage
// attribution, and a capability report (input to L2). The engine owns the
// plumbing the two lanes previously duplicated here; the lane values below
// contribute only what is genuinely lane-specific.
// ---------------------------------------------------------------------------
const fullAutoFollowupHandoff = makeFullAutoFollowupHandoff()
const laneDispatcher = makeProviderLaneDispatcher({
  threads,
  journal: localTurnJournal,
  liveAgentGraph: {
    // CUT-11 (#8691): the canonical live agent graph currently models the two
    // built-in lanes; an SPI lane outside that set skips graph registration
    // rather than corrupting the typed graph vocabulary.
    beginTurn: ({ turnRef, threadRef, lane }) => {
      if (lane === "fable_claude" || lane === "codex_local") {
        liveAgentGraph.beginTurn({ turnRef, threadRef, lane })
      }
    },
    applyEvent: (threadRef, envelope) => {
      liveAgentGraph.applyEvent(threadRef, envelope)
    },
  },
  usageLedger: {
    record: input => {
      if (input.provider === "claude_agent" || input.provider === "codex") {
        usageLedger.record({ ...input, provider: input.provider })
      }
    },
  },
  captureTurnCheckpoint,
  localTurnFlushers,
  isQuitting: () => desktopIsQuitting,
  onTurnEventProjected: (request, event, background) => {
    if (!background || request.fullAuto !== true) return
    scheduleFullAutoThreadUpdate(request.threadRef)
    fullAutoFollowupHandoff.observe({
      threadRef: request.threadRef,
      background,
      fullAuto: request.fullAuto === true,
      event,
    })
  },
  specWorkflow: {
    beforeTurn: () => projectSpecLaneTurn(resolveDesktopLocalWorkspaceRoot()),
    afterTurn: (laneRef, request, before) => {
      const after = projectSpecLaneTurn(resolveDesktopLocalWorkspaceRoot())
      const note = specLaneRevalidationNote(laneRef, before.snapshot, after.snapshot)
      if (note === null) return
      threads().append(request.threadRef, {
        key: randomUUID(),
        role: "system",
        text: note,
        timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
      })
    },
  },
})

/**
 * The Claude lane (`claude_agent`) as a provider-lane value. Message metadata
 * (#8712): every fact this host observes for the final assistant note is
 * recorded so the renderer's inspector can project it later — SDK-reported
 * effective model, lane, account ref, turn ref, exact token total, and
 * wall-clock duration. Bounded public-safe strings only.
 */
const fableLocalLane: ProviderLane<Readonly<{ skillName: string | null }>> = {
  laneRef: "fable-local",
  graphLaneRef: "fable_claude",
  eventChannel: FableLocalEventChannel,
  usageProvider: "claude_agent",
  capabilities: () => ({
    laneRef: "fable-local",
    provider: "claude_agent",
    // Spawn-config truth: the ClaudeModelSchema contract literals.
    models: ["claude-fable-5", "claude-opus-4-8", "claude-sonnet-5"],
    features: {
      skills: true,
      planOnly: true,
      reasoningEffort: false,
      images: true,
      fullAuto: true,
      interrupt: true,
      queueFollowup: true,
      steerTurn: false,
      steerChild: true,
      answerQuestion: true,
    },
    composer: {
      displayName: "Claude",
      reasoningEfforts: [],
      permissionModes: ["owner_full", "plan_only"],
      approvals: "provider_native",
      extensions: ["skills"],
    },
    policy: {
      source: "native-static-declaration",
      profileRef: "native:claude-agent:v1",
      evidence: "conformant",
      allowedModels: ["claude-fable-5", "claude-opus-4-8", "claude-sonnet-5"],
      allowedFeatures: ["skills", "planOnly", "images", "fullAuto", "interrupt", "queueFollowup", "steerChild", "answerQuestion"],
      allowedExtensions: ["skills"],
    },
    recovery: "interrupt_on_restart",
  }),
  admit: request => {
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
    return { ok: true, model: requestedModel, context: { skillName: selectedSkill?.name ?? null } }
  },
  prepare: (request, sender, model) => {
    if (request.fullAuto === true && sender !== null) {
      fullAutoRegistry.bindProfile(request.threadRef, {
        lane: "fable-local",
        ...(request.target?.accountRef === undefined ? {} : { accountRef: request.target.accountRef }),
        model,
      })
    }
  },
  streamMeta: ctx => {
    const model = ctx.effectiveModel()
    return {
      lane: "fable-local",
      turnRef: ctx.request.turnRef,
      ...(model === null ? {} : { model }),
    }
  },
  modelNoteText: fableLocalModelNoteText,
  runTurn: async ({ request, model, context, history, message, background, emit }) => {
    if (!isClaudeModel(model)) {
      return { ok: false, reason: "session_failed", detail: "non-Claude model admitted to the Claude lane" }
    }
    return fableLocal.runTurn({
      turnRef: request.turnRef,
      threadRef: request.threadRef,
      history,
      message: request.fullAuto === true ? fullAutoPrompt("fable-local", message) : message,
      ...(request.queueRef === undefined ? {} : { queueRef: request.queueRef }),
      ...(request.clientUserMessageId === undefined ? {} : { clientUserMessageId: request.clientUserMessageId }),
      ...(request.target === undefined ? {} : { accountRef: request.target.accountRef }),
      ...(request.reasoningEffort === undefined ? {} : { reasoningEffort: request.reasoningEffort }),
      ...(request.extensions === undefined ? {} : { extensionSelection: request.extensions }),
      model,
      ...(context.skillName === null ? {} : { skillName: context.skillName }),
      ...(request.permissionMode === "plan_only" ? { planMode: true } : {}),
      ...(request.images !== undefined && request.images.length > 0 ? { images: request.images } : {}),
      ...(request.fullAuto === true && background ? { autoResolveQuestions: true } : {}),
      emit,
    })
  },
  interrupt: turnRef => fableLocal.interrupt(turnRef),
  makeTurnProjector: ctx => {
    // EP250 wave-2 (J2/J4): track the latest plan/todo list so the FINAL plan
    // state persists into the finalized transcript (the live in-place plan
    // card is renderer-only). One persisted plan card, latest wins.
    let latestPlanEntries: ReadonlyArray<{ step: string; status: "pending" | "in_progress" | "completed" }> | null = null
    return turnEvent => {
      if (turnEvent.kind === "plan_updated") {
        latestPlanEntries = turnEvent.entries.map(entry => ({ step: entry.step, status: entry.status }))
      }
      if (turnEvent.kind === "turn_completed" && latestPlanEntries !== null) {
        ctx.store.append(ctx.request.threadRef, {
          key: `${ctx.request.turnRef}-plan`,
          role: "system",
          text: "Plan updated",
          timestamp: ctx.timestamp(),
          runtime: { kind: "plan", entries: latestPlanEntries },
        })
      }
      // Session usage ledger feed (#8712 Lane C): delegate children attribute
      // to the Codex account with gpt-5.6-sol recorded as spawn-config truth.
      // A child-observed revoked credential flips the account's typed
      // reconnect flag (probe/child evidence supersedes presence-based
      // "ready").
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
    }
  },
  finalMeta: ctx => {
    const model = ctx.effectiveModel()
    return {
      lane: "fable-local",
      turnRef: ctx.request.turnRef,
      ...(model === null ? {} : { model }),
      ...(ctx.result.accountRef === undefined ? {} : { accountRef: ctx.result.accountRef }),
      totalTokens: ctx.result.totalTokens,
      durationMs: ctx.durationMs,
    }
  },
  failureMessage: fableLocalFailureMessage,
  completed: request => {
    if (request.fullAuto === true) void runFullAutoReconciliation()
  },
}

ipcMain.handle(FableLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null) return { ok: false, error: "That message could not be sent." }
  const laneRef = providerLaneRegistry.selection(request.threadRef)
  if (laneRef === "fable-local") return laneDispatcher.dispatchTurn(fableLocalLane, request, event.sender)
  if (laneRef === "acp:grok-cli") return laneDispatcher.dispatchTurn(grokAcpFableEventLane, request, event.sender)
  if (laneRef === "acp:cursor-agent") return laneDispatcher.dispatchTurn(cursorAcpFableEventLane, request, event.sender)
  return { ok: false, error: "This thread is assigned to a different provider lane." }
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
// The compatibility smoke deliberately observes the intermediate disabled
// chip before releasing this gate. The React-only smoke begins at the installed
// Codex workbench and can release immediately.
if (reactSmokeMode) (releaseSmokeCodexAvailability as (() => void) | null)?.()
ipcMain.handle(CodexLocalAvailabilityChannel, async () => {
  if (smokeCodexAvailabilityGate !== null) await smokeCodexAvailabilityGate
  const availability = await codexLocal.availability()
  providerLaneAuthentication.set("codex-local", availability.state === "available" ? "ready" : "missing")
  return availability
})
ipcMain.handle(CodexLocalInterruptChannel, (_event, value: unknown) => {
  const request = decodeFableLocalInterruptRequest(value)
  return request === null
    ? false
    : codexLocal.interrupt(request.turnRef) ||
      grokAcpDriver.interrupt(request.turnRef) ||
      cursorAcpDriver.interrupt(request.turnRef)
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
ipcMain.handle(CodexLocalQueueListChannel, (_event, threadRef: unknown) =>
  typeof threadRef === "string"
    ? activeCodexQueuedIntents(codexDurableQueue.list(threadRef))
    : [])
ipcMain.handle(CodexLocalQueueEditChannel, (_event, value: unknown) => {
  const request = decodeCodexQueueMutation(value)
  if (request === null || request.message === undefined) return { ok: false, reason: "invalid" }
  try { return { ok: true, entry: codexDurableQueue.edit(request.queueRef, request.message, request.expectedRevision) } }
  catch (error) { return { ok: false, reason: error instanceof Error ? error.message : "Queue edit failed" } }
})
ipcMain.handle(CodexLocalQueueCancelChannel, (_event, value: unknown) => {
  const request = decodeCodexQueueMutation(value)
  if (request === null) return { ok: false, reason: "invalid" }
  try { return { ok: true, entry: codexDurableQueue.cancel(request.queueRef, request.expectedRevision) } }
  catch (error) { return { ok: false, reason: error instanceof Error ? error.message : "Queue cancellation failed" } }
})
ipcMain.handle(CodexEcosystemSnapshotChannel, async event => {
  if (!isTrustedRuntimeGatewaySender(event)) return null
  try { return (await currentCodexEcosystem()).snapshot() } catch { return null }
})
ipcMain.handle(CodexEcosystemMutationChannel, async (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { ok: false, reason: "untrusted_sender" }
  const request = decodeCodexEcosystemMutationRequest(value)
  const workroom = currentProductSpecWorkroom()
  if (request === null || workroom === null) return { ok: false, reason: "work_context_required" }
  try {
    const ecosystem = await currentCodexEcosystem()
    const authority = ecosystem.authorize(request.operation, workroom.workContextRef, ecosystem.snapshot().revision)
    switch (request.operation) {
      case "skill_config": await ecosystem.configureSkill({ id: request.id, enabled: request.enabled }, authority); break
      case "marketplace_add": await ecosystem.addMarketplace({ source: request.source, ...(request.refName === undefined ? {} : { refName: request.refName }) }, authority); break
      case "marketplace_remove": await ecosystem.removeMarketplace(request.name, authority); break
      case "marketplace_upgrade": await ecosystem.upgradeMarketplace(request.name, authority); break
      case "plugin_install": await ecosystem.installPlugin({ pluginName: request.pluginName, ...(request.remoteMarketplaceName === undefined ? {} : { remoteMarketplaceName: request.remoteMarketplaceName }) }, authority); break
      case "plugin_uninstall": await ecosystem.uninstallPlugin(request.id, authority); break
      case "mcp_reload": await ecosystem.reloadMcp(authority); break
      case "mcp_oauth": {
        const response = await ecosystem.startMcpOauth(request.name, request.threadId, authority)
        const row = typeof response === "object" && response !== null ? response as Record<string, unknown> : {}
        const url = typeof row.authorizationUrl === "string" ? row.authorizationUrl : typeof row.url === "string" ? row.url : null
        if (url !== null && /^https:\/\//u.test(url)) await shell.openExternal(url)
        break
      }
    }
    return { ok: true, snapshot: ecosystem.snapshot() }
  } catch (error) { return { ok: false, reason: typeof error === "object" && error !== null && "reason" in error && typeof error.reason === "string" ? error.reason : "ecosystem_request_failed" } }
})
ipcMain.handle(CodexHostSnapshotChannel, async event => {
  if (!isTrustedRuntimeGatewaySender(event)) return null
  try { return (await currentCodexHostServices()).snapshot() } catch { return null }
})
ipcMain.handle(CodexHostRequestChannel, async (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { ok: false, reason: "untrusted_sender" }
  const request = decodeCodexHostRequest(value)
  if (request === null) return { ok: false, reason: "invalid_request" }
  try {
    const host = await currentCodexHostServices()
    const authorize = (kind: Parameters<typeof host.authorize>[0], payload: unknown) => host.authorize(kind, payload, host.snapshot().revision)
    let result: unknown
    switch (request.operation) {
      case "fs_read": result = await host.readFile(request.path); break
      case "fs_write": { const payload = { relativePath: request.path, dataBase64: request.dataBase64 }; await host.writeFile(request.path, request.dataBase64, authorize("fs_mutation", payload)); break }
      case "fs_mkdir": { const payload = { relativePath: request.path, recursive: request.recursive }; await host.createDirectory(request.path, request.recursive, authorize("fs_mutation", payload)); break }
      case "fs_list": result = await host.readDirectory(request.path); break
      case "fs_metadata": result = await host.metadata(request.path); break
      case "fs_remove": { const payload = { relativePath: request.path, recursive: request.recursive }; await host.remove(request.path, request.recursive, authorize("fs_mutation", payload)); break }
      case "fs_copy": { const payload = { sourceRelativePath: request.sourcePath, destinationRelativePath: request.destinationPath, recursive: request.recursive }; await host.copy(request.sourcePath, request.destinationPath, request.recursive, authorize("fs_mutation", payload)); break }
      case "fs_watch": result = await host.watch(request.path); break
      case "fs_unwatch": await host.unwatch(request.watchId); break
      case "command_exec": { const input = { command: request.command, ...(request.cwd === undefined ? {} : { cwd: request.cwd }), ...(request.timeoutMs === undefined ? {} : { timeoutMs: request.timeoutMs }), ...(request.tty === undefined ? {} : { tty: request.tty }), ...(request.rows === undefined ? {} : { rows: request.rows }), ...(request.cols === undefined ? {} : { cols: request.cols }) }; result = await host.exec(input, authorize("command", input)); break }
      case "command_write": await host.writeCommand(request.processId, request.deltaBase64, request.closeStdin); break
      case "command_resize": await host.resizeCommand(request.processId, request.rows, request.cols); break
      case "command_terminate": await host.terminateCommand(request.processId); break
      case "search_fuzzy": result = await host.fuzzySearch(request.query); break
      case "search_start": result = await host.startSearch(); break
      case "search_update": await host.updateSearch(request.sessionId, request.query); break
      case "search_stop": await host.stopSearch(request.sessionId); break
      case "external_detect": result = await host.detectExternalConfig(); break
      case "external_histories": result = await host.readExternalHistories(); break
      case "external_import": { const payload = { migrationItems: request.migrationItems, source: request.source }; result = await host.importExternalConfig(request.migrationItems, request.source, authorize("external_import", payload)); break }
      case "windows_readiness": result = await host.windowsReadiness(); break
      case "windows_setup": await host.startWindowsSetup(request.mode, authorize("windows_setup", { mode: request.mode })); break
      case "feedback_upload": { const input = { classification: request.classification, reason: request.reason, attachments: request.attachments, includeLogs: request.includeLogs }; await host.uploadFeedback(input, authorize("feedback", input)); break }
    }
    return { ok: true, result, snapshot: host.snapshot() }
  } catch (error) { return { ok: false, reason: typeof error === "object" && error !== null && "reason" in error && typeof error.reason === "string" ? error.reason : "host_request_failed" } }
})
ipcMain.handle(CodexExperimentalSnapshotChannel, async event => {
  if (!isTrustedRuntimeGatewaySender(event)) return null
  try { return (await currentCodexExperimentalRuntime()).snapshot() } catch { return null }
})
ipcMain.handle(CodexConformanceSnapshotChannel, event => isTrustedRuntimeGatewaySender(event) ? makeCodexConformanceReport() : null)
ipcMain.handle(CodexExperimentalRequestChannel, async (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) return { ok: false, reason: "untrusted_sender" }
  const request = decodeCodexExperimentalRequest(value)
  if (request === null) return { ok: false, reason: "invalid_request" }
  try {
    const runtime = await currentCodexExperimentalRuntime()
    const authorize = (kind: Parameters<typeof runtime.authorize>[0], payload: unknown) => runtime.authorize(kind, payload, runtime.snapshot().revision)
    let result: unknown
    switch (request.operation) {
      case "environment_add": { const input = { environmentId: request.environmentId, execServerUrl: request.execServerUrl, ...(request.connectTimeoutMs === undefined ? {} : { connectTimeoutMs: request.connectTimeoutMs }) }; await runtime.addEnvironment(input, authorize("environment_add", input)); break }
      case "environment_reconnect": { const input = { environmentId: request.environmentId, execServerUrl: request.execServerUrl }; await runtime.reconnectEnvironment(request.environmentId, request.execServerUrl, authorize("environment_add", input)); break }
      case "environment_target": result = runtime.turnEnvironment(request.environmentRef, request.cwd); break
      case "process_spawn": { const input = { command: request.command, cwd: request.cwd, ...(request.tty === undefined ? {} : { tty: request.tty }), ...(request.rows === undefined ? {} : { rows: request.rows }), ...(request.cols === undefined ? {} : { cols: request.cols }), ...(request.timeoutMs === undefined ? {} : { timeoutMs: request.timeoutMs }) }; result = await runtime.spawnProcess(input, authorize("process_spawn", input)); break }
      case "process_write": await runtime.writeProcess(request.processRef, request.dataBase64, request.closeStdin); break
      case "process_resize": await runtime.resizeProcess(request.processRef, request.rows, request.cols); break
      case "process_kill": await runtime.killProcess(request.processRef, authorize("process_control", { processRef: request.processRef, operation: "kill" })); break
      case "terminal_list": await runtime.listBackgroundTerminals(request.threadId); break
      case "terminal_clean": { const payload = { threadId: request.threadId, operation: "clean" }; await runtime.cleanBackgroundTerminals(request.threadId, authorize("terminal_mutation", payload)); break }
      case "terminal_terminate": { const payload = { threadId: request.threadId, processRef: request.processRef, operation: "terminate" }; result = await runtime.terminateBackgroundTerminal(request.threadId, request.processRef, authorize("terminal_mutation", payload)); break }
      case "realtime_start": { const input = { threadId: request.threadId, outputModality: request.outputModality, transport: request.transport, ...(request.voice === undefined ? {} : { voice: request.voice }) }; await runtime.startRealtime(input, authorize("realtime_start", input)); break }
      case "realtime_audio": { const payload = { threadId: request.threadId, audio: request.audio }; await runtime.appendRealtimeAudio(request.threadId, request.audio, authorize("realtime_audio", payload)); break }
      case "realtime_text": await runtime.appendRealtimeText(request.threadId, request.text, request.role); break
      case "realtime_speech": await runtime.appendRealtimeSpeech(request.threadId, request.text); break
      case "realtime_stop": await runtime.stopRealtime(request.threadId); break
      case "realtime_voices": result = await runtime.listVoices(); break
      case "remote_enable": await runtime.enableRemoteControl(authorize("remote_control", { operation: "enable" })); break
      case "remote_disable": await runtime.disableRemoteControl(authorize("remote_control", { operation: "disable" })); break
      case "remote_status": await runtime.remoteStatus(); break
      case "remote_pair": { const payload = { operation: "pair", manualCode: request.manualCode }; result = await runtime.startPairing(request.manualCode, authorize("remote_control", payload)); break }
      case "remote_pair_status": result = await runtime.pairingStatus(request.pairingRef); break
      case "remote_clients": await runtime.listRemoteClients(request.environmentId); break
      case "remote_revoke": { const payload = { environmentId: request.environmentId, clientRef: request.clientRef }; await runtime.revokeRemoteClient(request.environmentId, request.clientRef, authorize("remote_revoke", payload)); break }
      case "memory_reset": await runtime.resetMemory(request.confirmation, authorize("memory_reset", { confirmation: request.confirmation })); break
      case "thread_elicitation": { const payload = { threadId: request.threadId, direction: request.direction }; await runtime.adjustElicitation(request.threadId, request.direction, authorize("thread_control", payload)); break }
    }
    return { ok: true, result, snapshot: runtime.snapshot() }
  } catch (error) { return { ok: false, reason: typeof error === "object" && error !== null && "reason" in error && typeof error.reason === "string" ? error.reason : "experimental_request_failed" } }
})
/**
 * The Codex lane as a provider-lane value. Message metadata (#8712 pattern):
 * lane, spawn-config-truth model, account ref, turn ref, exact usage total,
 * duration — plus the codex thread id (session-receipt continuity) in
 * requestId.
 */
const codexLocalLane: ProviderLane<null> = {
  laneRef: "codex-local",
  graphLaneRef: "codex_local",
  eventChannel: CodexLocalEventChannel,
  usageProvider: "codex",
  capabilities: () => ({
    laneRef: "codex-local",
    provider: "codex",
    // Spawn-config truth: the CodexModelSchema contract literals.
    models: ["gpt-5.6-sol", "gpt-5.5"],
    features: {
      skills: false,
      planOnly: false,
      reasoningEffort: true,
      images: true,
      fullAuto: true,
      interrupt: true,
      queueFollowup: true,
      steerTurn: true,
      steerChild: false,
      answerQuestion: true,
    },
    composer: {
      displayName: "Codex",
      reasoningEfforts: ["low", "medium", "high", "xhigh"],
      permissionModes: ["owner_full"],
      approvals: "host_mediated",
      extensions: [],
    },
    policy: {
      source: "native-static-declaration",
      profileRef: "native:codex-local:v1",
      evidence: "conformant",
      allowedModels: ["gpt-5.6-sol", "gpt-5.5"],
      allowedFeatures: ["reasoningEffort", "images", "fullAuto", "interrupt", "queueFollowup", "steerTurn", "answerQuestion"],
      allowedExtensions: [],
    },
    recovery: "provider_session_replay",
  }),
  admit: request => {
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
    return { ok: true, model: requestedModel, context: null }
  },
  // FA-H6 (#8879): a renderer-initiated flagged turn defines the loop's
  // execution profile -- bind account/model/effort onto the durable record so
  // continuations (including post-restart resumes) replay the same profile
  // instead of falling back to lane defaults and account rotation. A
  // main-initiated continuation (sender === null) never rebinds: its profile
  // CAME from the record.
  prepare: (request, sender, model) => {
    if (request.fullAuto === true && sender !== null) {
      fullAutoRegistry.bindProfile(request.threadRef, {
        lane: "codex-local",
        ...(request.target?.accountRef === undefined ? {} : { accountRef: request.target.accountRef }),
        model,
        ...(request.reasoningEffort === undefined ? {} : { reasoningEffort: request.reasoningEffort }),
      })
    }
  },
  bound: request => {
    const productSpecAuthority = currentProductSpecWorkroom()
    if (productSpecAuthority !== null) {
      codexHandoffBindings.bindNextTurn({
        workContextRef: productSpecAuthority.workContextRef,
        sessionRef: productSpecAuthority.sessionRef,
        threadRef: request.threadRef,
        turnRef: request.turnRef,
      })
    }
  },
  streamMeta: ctx => ({
    lane: "codex-local",
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? codexLocalRequestedModelLabel(ctx.requestedModel),
  }),
  modelNoteText: codexLocalModelNoteText,
  runTurn: async ({ request, model, history, message, background, emit }) => {
    if (!isCodexModel(model)) {
      return { ok: false, reason: "session_failed", detail: "non-Codex model admitted to the Codex lane" }
    }
    // Admission happens before local execution and is fail-soft: telemetry can
    // never block a turn, while completion can never invent server authority
    // after the fact. Full Auto uses this same lane seam.
    await desktopCodexUsageReporter.admit({
      turnRef: request.turnRef,
      model,
    })
    const result = await codexLocal.runTurn({
      turnRef: request.turnRef,
      threadRef: request.threadRef,
      history,
      message,
      ...(request.target === undefined ? {} : { accountRef: request.target.accountRef }),
      model,
      ...(request.reasoningEffort === undefined ? {} : { reasoningEffort: request.reasoningEffort }),
      ...(request.images !== undefined && request.images.length > 0 ? { images: request.images } : {}),
      ...(request.fullAuto === true ? { fullAuto: true } : {}),
      // Full Auto #8884: a main-initiated background continuation (sender ===
      // null) has no renderer capable of answering item/tool/requestUserInput —
      // a pending question would hang the turn forever. Auto-resolve instead.
      ...(request.fullAuto === true && background ? { autoResolveQuestions: true } : {}),
      emit,
    })
    // The codex thread id is the lane's provider-native session ref
    // (session-receipt continuity, journal-recorded for restart replay).
    return result.ok ? { ...result, providerSessionRef: result.threadId } : result
  },
  interrupt: turnRef => codexLocal.interrupt(turnRef),
  makeTurnProjector: ctx => turnEvent => {
    // #8911: consent-gated reporting remains a Codex-lane concern while the
    // shared dispatcher owns provider-agnostic usage-ledger attribution.
    if (turnEvent.kind === "turn_completed" && turnEvent.usage !== null && turnEvent.usage !== undefined) {
      Effect.runFork(desktopCodexUsageReporter.report({
        turnRef: ctx.request.turnRef,
        model: ctx.effectiveModel() ?? ctx.requestedModel,
        observedAt: new Date().toISOString(),
        usage: turnEvent.usage,
      }))
    }
    // Persist reasoning/notice lines so the finalized transcript keeps the
    // same evidence the live stream showed (the shared engine already owns
    // the tool-trace and effective-model notes).
    if (turnEvent.kind === "reasoning" || turnEvent.kind === "lane_notice") {
      ctx.store.append(ctx.request.threadRef, {
        key: randomUUID(),
        role: "system",
        text: turnEvent.kind === "reasoning" ? `Reasoning · ${turnEvent.text}` : turnEvent.text,
        timestamp: ctx.timestamp(),
      })
    }
    // Interactive and structured cards are host-owned durable state, not a
    // renderer-only projection. Persist every update before forwarding it so
    // renderer reload, final-thread replacement, and app restart preserve
    // questions, plans, complete nested child transcripts, and queue chips.
    const runtimeOperation = localRuntimePersistenceOperation({
      turnRef: ctx.request.turnRef,
      event: turnEvent,
      notes: ctx.store.open(ctx.request.threadRef)?.notes ?? [],
      timestamp: ctx.timestamp(),
    })
    if (runtimeOperation.kind === "upsert") ctx.store.upsert(ctx.request.threadRef, runtimeOperation.note)
    if (runtimeOperation.kind === "remove") ctx.store.remove(ctx.request.threadRef, runtimeOperation.key)
  },
  finalMeta: ctx => ({
    lane: "codex-local",
    turnRef: ctx.request.turnRef,
    model: ctx.effectiveModel() ?? codexLocalRequestedModelLabel(ctx.requestedModel),
    ...(ctx.result.accountRef === undefined ? {} : { accountRef: ctx.result.accountRef }),
    ...(ctx.result.providerSessionRef === undefined || ctx.result.providerSessionRef === null
      ? {}
      : { requestId: ctx.result.providerSessionRef }),
    totalTokens: ctx.result.totalTokens,
    durationMs: ctx.durationMs,
  }),
  failureMessage: codexLocalFailureMessage,
  // Full Auto (#8853): fire-and-forget -- do not make the caller (a real
  // renderer send, or a prior continuation in this same chain) wait on the
  // NEXT turn. runFullAutoReconciliation re-checks the durable registry
  // fresh, so a toggle-off that lands right after this line still stops it.
  completed: request => {
    if (request.fullAuto === true) void runFullAutoReconciliation()
  },
}

const acpLaneCapabilities = (
  provider: "grok" | "cursor",
): import("./provider-lane.ts").ProviderLaneCapabilityReport => {
  const grok = provider === "grok"
  const laneRef = grok ? "acp:grok-cli" : "acp:cursor-agent"
  const model = grok ? "grok-default" : "cursor-auto"
  return {
    laneRef,
    provider,
    models: [model],
    features: {
      skills: false,
      planOnly: false,
      reasoningEffort: false,
      images: false,
      fullAuto: true,
      interrupt: true,
      queueFollowup: false,
      steerTurn: false,
      steerChild: false,
      answerQuestion: false,
    },
    composer: {
      displayName: grok ? "Grok CLI" : "Cursor Agent CLI",
      reasoningEfforts: [],
      permissionModes: ["owner_full"],
      approvals: "none",
      extensions: [],
    },
    policy: {
      source: "trusted-acp-peer-profile",
      profileRef: grok ? "grok-cli" : "cursor-agent",
      evidence: "experimental",
      allowedModels: [model],
      allowedFeatures: ["fullAuto", "interrupt"],
      allowedExtensions: [],
    },
    recovery: grok ? "provider_session_replay" : "interrupt_on_restart",
  }
}

const grokAcpDriver = acpProviderHost.driver("grok")
const cursorAcpDriver = acpProviderHost.driver("cursor")
const grokAcpLane = makeAcpProviderLane({
  laneRef: "acp:grok-cli",
  graphLaneRef: "grok_acp",
  eventChannel: CodexLocalEventChannel,
  capabilities: acpLaneCapabilities("grok"),
  driver: grokAcpDriver,
})
const cursorAcpLane = makeAcpProviderLane({
  laneRef: "acp:cursor-agent",
  graphLaneRef: "cursor_acp",
  eventChannel: CodexLocalEventChannel,
  capabilities: acpLaneCapabilities("cursor"),
  driver: cursorAcpDriver,
})
// The renderer's local harness subscribes to the channel associated with the
// currently selected built-in shell harness. These event-channel-only views
// keep an ACP-selected thread stream visible regardless of which built-in
// harness was selected before the registry switch; execution remains the one
// shared main-owned driver and ProviderLane implementation.
const grokAcpFableEventLane: ProviderLane<null> = {
  ...grokAcpLane,
  eventChannel: FableLocalEventChannel,
}
const cursorAcpFableEventLane: ProviderLane<null> = {
  ...cursorAcpLane,
  eventChannel: FableLocalEventChannel,
}

const providerLaneCapabilityByRef = (laneRef: string) =>
  laneRef === "codex-local" ? codexLocalLane.capabilities()
    : laneRef === "fable-local" ? fableLocalLane.capabilities()
      : laneRef === "acp:grok-cli" ? grokAcpLane.capabilities()
        : laneRef === "acp:cursor-agent" ? cursorAcpLane.capabilities()
          : null

const providerLaneEntries = async (): Promise<ReadonlyArray<ProviderLaneRegistryEntry>> => {
  const lanes = [fableLocalLane, codexLocalLane] as const
  const nativeEntries: ReadonlyArray<ProviderLaneRegistryEntry> = lanes.map(lane => {
    const report = lane.capabilities()
    const capabilities = projectProviderLaneCapabilities(report)
    return {
      laneRef: lane.laneRef,
      provider: report.provider,
      profileRef: report.policy.profileRef,
      configuration: "configured",
      authentication: providerLaneAuthentication.get(lane.laneRef) ?? "unknown",
      admission: capabilities.admission,
      reason: capabilities.reason,
      capabilities,
    }
  })
  await ensureAcpProviders()
  const providerStatus = acpProviderHost.status()
  const acpPeerEntry = (lane: ProviderLane<null>): ProviderLaneRegistryEntry => {
    const report = lane.capabilities()
    const capabilities = projectProviderLaneCapabilities(report)
    const status = providerStatus.providers.find(provider => provider.profileRef === report.policy.profileRef)
    const detected = status?.install === "detected"
    const compatible = status?.profileState === "supported" || status?.profileState === "experimental"
    const reason = capabilities.reason
      ?? (!detected ? `${report.composer.displayName} is not installed or has not passed its executable probe.`
        : !compatible ? `${report.composer.displayName} does not match an admitted pinned peer profile.`
          : null)
    const admission = reason === null ? capabilities.admission : "quarantined"
    return {
      laneRef: lane.laneRef,
      provider: report.provider,
      profileRef: report.policy.profileRef,
      configuration: detected && compatible ? "configured" : "unconfigured",
      authentication: status?.auth.state === "authenticated"
        ? "ready"
        : status?.auth.state === "required" || status?.auth.state === "failed"
          ? "missing"
          : "unknown",
      admission,
      reason,
      capabilities: { ...capabilities, admission, reason },
    }
  }
  return [
    ...nativeEntries,
    acpPeerEntry(grokAcpLane),
    acpPeerEntry(cursorAcpLane),
  ]
}

// L2 #8900 + L8 #8903: the renderer receives only policy-intersected,
// public-safe registry projections. Authentication is explicit even when a
// probe fails; no lane is silently omitted because it is unavailable.
ipcMain.handle(ProviderLaneCapabilitiesChannel, async event =>
  isTrustedRuntimeGatewaySender(event)
    ? (await providerLaneEntries()).map(entry => entry.capabilities)
    : [])
ipcMain.handle(ProviderLaneRegistryListChannel, async event =>
  isTrustedRuntimeGatewaySender(event)
    ? { lanes: await providerLaneEntries(), selections: providerLaneRegistry.listSelections() }
    : { lanes: [], selections: [] })
ipcMain.handle(ProviderLaneRegistrySelectChannel, async (event, value: unknown) => {
  if (!isTrustedRuntimeGatewaySender(event)) {
    return { ok: false, reason: "unadmitted_peer", message: "Untrusted renderer.", missingCapabilities: [] }
  }
  const request = decodeProviderLaneSelectRequest(value)
  if (request === null) {
    return { ok: false, reason: "unknown_lane", message: "Invalid lane selection.", missingCapabilities: [] }
  }
  const thread = threads().open(request.threadRef)
  const requiredCapabilities = [
    ...(fullAutoRegistry.get(request.threadRef) ? ["fullAuto" as const] : []),
    ...(thread?.notes.some(note => note.question?.status === "pending") ? ["answerQuestion" as const] : []),
  ]
  const result = providerLaneRegistry.switchThread({
    ...request,
    lanes: await providerLaneEntries(),
    thread,
    requiredCapabilities,
  })
  if (result.ok && result.previousLaneRef !== result.laneRef) {
    if (result.laneRef === "codex-local") codexLocal.resetContinuity(result.threadRef)
    if (result.laneRef === "fable-local") fableLocal.resetContinuity(result.threadRef)
  }
  if (!result.ok && thread !== null) {
    const updated = threads().append(thread.id, {
      key: randomUUID(),
      role: "system",
      text: `Provider switch refused (${result.reason}): ${result.message}`,
      timestamp: new Date().toISOString(),
      meta: { lane: providerLaneRegistry.selection(thread.id) },
    })
    if (updated !== null && !event.sender.isDestroyed()) {
      event.sender.send(DesktopLocalTurnRecoveryUpdateChannel, updated)
    }
  }
  return result.ok
    ? { ok: true, threadRef: result.threadRef, laneRef: result.laneRef, previousLaneRef: result.previousLaneRef, truncated: result.truncated }
    : result
})

/**
 * Full Auto (#8853): extracted so both a renderer-initiated send (via the IPC
 * handler below, sender = event.sender) and a main-initiated continuation
 * (sender = null; see reconcileFullAutoThreads wiring) share the exact same
 * turn-dispatch path -- no second, divergent execution route for background
 * continuations. Since L1 (#8899) the shared path is the provider-lane
 * dispatcher over the codex lane value above.
 */
const dispatchCodexLocalTurn = async (
  request: import("./fable-local-contract.ts").FableLocalStartRequest,
  sender: WebContents | null,
): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>> =>
  laneDispatcher.dispatchTurn(codexLocalLane, request, sender)

const dispatchFableLocalTurn = async (
  request: import("./fable-local-contract.ts").FableLocalStartRequest,
  sender: WebContents | null,
): Promise<Readonly<{ ok: boolean; thread?: DesktopThread | null; error?: string }>> =>
  laneDispatcher.dispatchTurn(fableLocalLane, request, sender)

/** Owner-visible Full Auto outcome notes reuse the same system-note +
 * recovery-broadcast shape the cap note already shipped with. */
const appendFullAutoSystemNote = (threadRef: string, text: string): void => {
  threads().append(threadRef, {
    key: randomUUID(),
    role: "system",
    text,
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  })
  broadcastFullAutoThreadUpdate(threadRef)
}
/**
 * Full Auto (#8853): the one place "should the next turn start" is decided,
 * called from two trigger points -- right after a Full-Auto turn completes
 * (above) and once at startup after existing turn-recovery settles (below).
 * Both share this exact wiring so a background continuation and a
 * post-restart resume are the same durable decision, not two.
 *
 * FA-H3 (#8876): every invocation runs through a promise-chain mutex, so
 * overlapping triggers (turn completion + startup + any future continue-now)
 * serialize instead of interleaving the snapshot/dispatch sequence. The
 * durable per-thread lease inside reconcileFullAutoThreads is the second,
 * restart-surviving half of the same exactly-once guarantee.
 */
const fullAutoReconcileQueue = makeSerialTaskQueue()
const runFullAutoReconciliation = (options?: Readonly<{ startup?: boolean }>): Promise<void> =>
  fullAutoReconcileQueue(() => reconcileFullAutoThreads({
    registry: fullAutoRegistry,
    nonterminalThreadRefs: () => new Set(localTurnJournal.nonterminal().map(record => record.threadRef)),
    // FA-H2 (#8875): the exact same workspace resolution the codex-local
    // runtime executes against -- never a renderer-supplied path.
    resolveWorkspaceRef: resolveDesktopLocalWorkspaceRoot,
    journalHasNonterminalTurn: turnRef =>
      localTurnJournal.nonterminal().some(record => record.turnRef === turnRef),
    // FA-H3: only the startup pass may clear a stale (crashed mid-dispatch)
    // lease; a mid-session pass treats a held lease as in-flight and skips.
    ...(options?.startup === true ? { clearStaleLeases: true } : {}),
    dispatch: async ({ threadRef, turnRef, message, profile }) => {
      // FA-H3 defense in depth: even if a lease were somehow bypassed, never
      // start a Full Auto continuation on a thread that already has a
      // nonterminal turn in the local-turn journal.
      if (localTurnJournal.nonterminal().some(record => record.threadRef === threadRef)) {
        return { ok: false, reason: "turn_already_in_flight" }
      }
      // FA-H6 (#8879): replay the bound execution profile, revalidated
      // against the live contract enums (a field that no longer decodes
      // falls back to lane defaults instead of failing the loop).
      const laneRef = profile?.lane ?? FULL_AUTO_DEFAULT_LANE
      const report = providerLaneCapabilityByRef(laneRef)
      const policy = fullAutoLanePolicy(laneRef)
      const projection = report === null ? null : projectProviderLaneCapabilities(report)
      if (
        report === null || policy === null || !policy.autoResolveQuestions ||
        projection?.admission !== "admitted" || projection.fullAuto !== true
      ) {
        return { ok: false, reason: `full_auto_lane_not_eligible:${laneRef}` }
      }
      // FA-H4 (#8877): the background turn becomes a rendered fact the moment
      // it dispatches, carrying the lease turn ref so the composer's stop
      // control can target the ACTUAL running background turn.
      setFullAutoLiveState(threadRef, "turn_running", turnRef)
      const promotedFollowup = fullAutoFollowupHandoff.take(threadRef)
      const result = laneRef === "codex-local"
        ? await (async () => {
            const bound = decodeCodexLocalContinuationProfile(profile)
            return dispatchCodexLocalTurn({
              turnRef,
              threadRef,
              message: promotedFollowup?.message ?? message,
              fullAuto: true,
              ...(promotedFollowup === null ? {} : {
                queueRef: promotedFollowup.queueRef,
                clientUserMessageId: promotedFollowup.clientUserMessageId,
              }),
              ...(bound.model === null ? {} : { model: bound.model }),
              ...(bound.reasoningEffort === null ? {} : { reasoningEffort: bound.reasoningEffort }),
              ...(bound.model !== null && bound.accountRef !== null
                ? { target: { provider: "codex" as const, accountRef: bound.accountRef, model: bound.model } }
                : {}),
            }, null)
          })()
        : laneRef === "fable-local"
        ? await (async () => {
            const model = profile?.model !== undefined && isClaudeModel(profile.model)
              ? profile.model
              : FABLE_LOCAL_MODEL
            return dispatchFableLocalTurn({
              turnRef,
              threadRef,
              message,
              model,
              fullAuto: true,
              ...(profile?.accountRef === undefined
                ? {}
                : { target: { provider: "claude_agent" as const, accountRef: profile.accountRef, model } }),
            }, null)
          })()
        : await laneDispatcher.dispatchTurn(
          laneRef === "acp:grok-cli" ? grokAcpLane : cursorAcpLane,
          {
            turnRef,
            threadRef,
            message,
            fullAuto: true,
          },
          null,
        )
      // FA-H4: success transitions here; every failure path (thrown OR
      // ok:false, including the in-flight refusal above) transitions in
      // onDispatchFailed below, so the two never double-report.
      if (result.ok) setFullAutoLiveState(threadRef, "turn_completed", turnRef)
      return result.ok ? { ok: true } : { ok: false, reason: result.error ?? "dispatch_failed" }
    },
    onCapReached: capThreadRef => {
      // FA-H4 (#8877): the cap stop is a typed live state, not just a note.
      setFullAutoLiveState(capThreadRef, "cap_reached", null)
      appendFullAutoSystemNote(
        capThreadRef,
        `Full Auto stopped after ${FULL_AUTO_MAX_CONTINUATIONS} turns in a row. Turn it back on to continue.`,
      )
    },
    // FA-H2 (#8875): a continuation whose granted workspace no longer matches
    // (or was never bound) does not dispatch -- the record is disabled with a
    // typed blockedReason and the owner sees why, on the thread itself.
    onWorkspaceBlocked: (blockedThreadRef, block) => {
      // FA-H4 (#8877): the typed disable is also a live state transition.
      setFullAutoLiveState(blockedThreadRef, "blocked", null, block.reason)
      appendFullAutoSystemNote(
        blockedThreadRef,
        block.reason === "workspace_unbound"
          ? "Full Auto was turned off for this thread: it has no recorded granted workspace, so a continuation cannot be dispatched safely. Turn Full Auto back on from the workspace you want it to work in."
          : "Full Auto was turned off for this thread: the selected workspace no longer matches the workspace that was granted when Full Auto was enabled. Turn Full Auto back on from the workspace you want it to work in.",
      )
    },
    // FA-H5 (#8878): a failed dispatch (thrown OR ok:false) is a typed,
    // owner-visible outcome -- never a silently dormant enabled record.
    onDispatchFailed: (failedThreadRef, failure) => {
      console.error(
        "[openagents-desktop] full auto continuation dispatch failed",
        failedThreadRef,
        failure.reason,
      )
      // FA-H4 (#8877): a failure-limit disable renders as blocked; an
      // ordinary failure renders as turn_failed with the typed reason.
      if (failure.disabled) setFullAutoLiveState(failedThreadRef, "blocked", null, failure.reason)
      else setFullAutoLiveState(failedThreadRef, "turn_failed", null, failure.reason)
      appendFullAutoSystemNote(
        failedThreadRef,
        failure.disabled
          ? `Full Auto continuation failed ${failure.consecutiveFailures} times in a row (${failure.reason}). Full Auto was turned off for this thread.`
          : `Full Auto continuation failed: ${failure.reason}. It will retry with backoff.`,
      )
    },
  }).then(dispatched => {
    for (const dispatchedThreadRef of dispatched) broadcastFullAutoThreadUpdate(dispatchedThreadRef)
  }))
// Startup resume: once existing interrupted-turn recovery settles, any
// thread still marked enabled with nothing in flight gets its next
// continuation dispatched here -- this is what survives a full app
// quit+relaunch, not just a renderer reload. Only this pass clears stale
// dispatch leases (FA-H3): a lease whose turn ref never reached the journal
// belongs to a dispatch that crashed before its turn was accepted.
void localTurnRecovery.then(() => runFullAutoReconciliation({ startup: true })).catch(() => {})

ipcMain.handle(CodexLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null) return { ok: false, error: "That message could not be sent." }
  const laneRef = providerLaneRegistry.selection(request.threadRef)
  if (laneRef === "codex-local") return dispatchCodexLocalTurn(request, event.sender)
  if (laneRef === "acp:grok-cli") return laneDispatcher.dispatchTurn(grokAcpLane, request, event.sender)
  if (laneRef === "acp:cursor-agent") return laneDispatcher.dispatchTurn(cursorAcpLane, request, event.sender)
  return { ok: false, error: "This thread is assigned to a different provider lane." }
})

// Full Auto (#8853): the composer toggle persists here immediately, whether
// or not a turn is in flight, so quitting the app right after a toggle-off
// still stops the loop durably (no window round trip required to make the
// stop real).
ipcMain.handle(CodexLocalFullAutoSetChannel, async (_event, value: unknown) => {
  const request = decodeCodexLocalFullAutoSetRequest(value)
  if (request === null) return { ok: false }
  // FA-H2 (#8875): enabling binds the CURRENTLY resolved workspace onto the
  // durable record -- resolved by main from the same source of truth the
  // codex-local runtime executes against, never a renderer-supplied path.
  // Reconciliation later refuses to dispatch when this binding no longer
  // matches. A pre-upgrade record with no binding is rebound here on its
  // next enable; until then it fails closed at dispatch.
  applyFullAutoComposerToggle({
    registry: fullAutoRegistry,
    threadRef: request.threadRef,
    enabled: request.enabled,
    workspaceRef: resolveDesktopLocalWorkspaceRoot(),
    profile: { lane: FULL_AUTO_DEFAULT_LANE },
    // Enabling is an immediate trigger. Reconciliation is fire-and-forget
    // because it owns the whole background turn lifetime; the toggle IPC must
    // acknowledge promptly while the serialized queue starts work now.
    scheduleReconciliation: () => { void runFullAutoReconciliation() },
  })
  if (!request.enabled) {
    appendFullAutoSystemNote(
      request.threadRef,
      "Full Auto disabled from the composer toggle (caller: ui-toggle).",
    )
  }
  return { ok: true }
})
ipcMain.handle(CodexLocalFullAutoGetChannel, async (_event, value: unknown) => {
  const request = decodeCodexLocalFullAutoGetRequest(value)
  if (request === null) return { enabled: false, state: "idle", turnRef: null }
  // FA-H4 (#8877): additive live-state fields ride alongside `enabled` so
  // the renderer's existing wave-1 hydration keeps working unchanged while
  // a thread switch also picks up an in-flight background turn immediately.
  const live = fullAutoLiveState.get(request.threadRef)
  return {
    enabled: fullAutoRegistry.get(request.threadRef),
    state: live?.state ?? "idle",
    turnRef: live?.turnRef ?? null,
    ...(live?.detail === undefined ? {} : { detail: live.detail }),
  }
})
// FA-H4 (#8877): thread-scoped stop for the background continuation turn.
// The renderer names only the thread; main resolves the live running turn
// ref itself and signals the exact same runtime interrupt path the existing
// CodexLocalInterruptChannel handler uses (codexLocal.interrupt).
ipcMain.handle(CodexLocalFullAutoInterruptChannel, async (_event, value: unknown) => {
  const request = decodeCodexLocalFullAutoInterruptRequest(value)
  if (request === null) return { ok: false }
  const live = fullAutoLiveState.get(request.threadRef)
  if (live === undefined || live.state !== "turn_running" || live.turnRef === null) return { ok: false }
  return {
    ok: codexLocal.interrupt(live.turnRef) ||
      grokAcpDriver.interrupt(live.turnRef) ||
      cursorAcpDriver.interrupt(live.turnRef),
  }
})

/**
 * FA-H13 (#8886): the opt-in, loopback-only Phase 1 Full Auto control
 * surface. Off by default -- constructed ONLY under
 * OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1 -- and handed a narrow capability
 * options object (never main's internals) so the server module stays
 * testable with fakes. Every capability is the exact function the IPC
 * handlers above already use: the same registry, the same workspace
 * resolution, the same serialized reconciliation trigger, the same live-state
 * map, the same journal, and the same durable system-note appender.
 */
if (isFullAutoControlEnabled(process.env)) {
  const pinnedControlPort = Number.parseInt(process.env[FULL_AUTO_CONTROL_PORT_ENV] ?? "", 10)
  void startFullAutoControlServer({
    capabilities: {
      registry: fullAutoRegistry,
      resolveWorkspaceRef: resolveDesktopLocalWorkspaceRoot,
      // continue-now is a new TRIGGER into the same promise-chain mutex --
      // the exact function every other Full Auto trigger point calls.
      triggerReconciliation: () => runFullAutoReconciliation(),
      liveState: threadRef => fullAutoLiveState.get(threadRef) ?? null,
      listTurns: threadRef => localTurnJournal.list().filter(record => record.threadRef === threadRef),
      appendSystemNote: appendFullAutoSystemNote,
      // start bootstrap: main mints the thread in its own store -- callers
      // never name a ref -- so the reconcile dispatcher finds a real thread
      // and the first continuation opens a brand-new provider conversation.
      createThread: (title, laneRef) => {
        const thread = threads().newThread(title ?? "Full Auto")
        providerLaneRegistry.bind(thread.id, laneRef)
        return thread.id
      },
      listLanes: providerLaneEntries,
      isLaneEligible: laneRef => {
        const report = providerLaneCapabilityByRef(laneRef)
        const policy = fullAutoLanePolicy(laneRef)
        if (report === null || policy?.autoResolveQuestions !== true) return false
        const projection = projectProviderLaneCapabilities(report)
        return projection.admission === "admitted" && projection.fullAuto === true
      },
    },
    controlFilePath: path.join(app.getPath("userData"), "full-auto", "control.json"),
    ...(Number.isInteger(pinnedControlPort) && pinnedControlPort > 0 && pinnedControlPort <= 65535
      ? { port: pinnedControlPort }
      : {}),
  }).then(server => {
    // Public-safe: the URL and file location only -- never the token.
    console.log(
      `[openagents-desktop full-auto-control] listening ${server.url} (connection info: ${server.controlFilePath})`,
    )
    app.on("will-quit", () => { void server.stop() })
  }).catch(error => {
    console.error(
      "[openagents-desktop full-auto-control] failed to start",
      error instanceof Error ? error.message : "unknown",
    )
  })
}

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
  // Fill the display the owner is currently using without entering native
  // fullscreen. `workArea` deliberately preserves the menu bar and Dock.
  const launchWorkArea = electronScreen.getDisplayNearestPoint(
    electronScreen.getCursorScreenPoint(),
  ).workArea
  const window = new BrowserWindow({
    x: launchWorkArea.x,
    y: launchWorkArea.y,
    width: launchWorkArea.width,
    height: launchWorkArea.height,
    minWidth: 720,
    minHeight: 480,
    fullscreen: false,
    // khalaTheme color.background — must match @effect-native/tokens so the
    // pre-boot window never flashes an off-palette frame (EP250 #8712).
    backgroundColor: "#05070d",
    show: false,
    title: desktopApplicationName,
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
    if (!hiddenAutomationMode) window.show()
  })
  void window.loadURL(desktopRendererEntry)
  return window
}

/**
 * Smoke mode (`pnpm run smoke`): proves the Effect Native intent loop runs
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

const smokeReactWorkbench = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-react-surface="true"]') === null) {
    await wait(50)
  }
  const surface = document.querySelector('[data-en-react-surface="true"]')
  let textarea = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  while (Date.now() < deadline && (textarea === null || textarea.disabled)) {
    await wait(50)
    textarea = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  }
  const marks = () => globalThis.__oaStartupMarks || {}
  while (Date.now() < deadline && typeof marks().historyHydrated !== "number") await wait(50)
  const newSession = [...document.querySelectorAll('button')]
    .find((button) => button.textContent?.trim() === "New session")
  newSession?.click()
  await wait(200)
  textarea = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  textarea?.focus()
  await wait(200)
  return {
    ok: surface !== null && textarea !== null &&
      document.documentElement.dataset.desktopRenderer === "react" &&
      document.querySelector('[data-en-key="shell-root"]') === null &&
      document.querySelectorAll('#openagents-desktop-root > *').length === 1,
    backend: document.documentElement.dataset.desktopRenderer,
    reactSurfaces: document.querySelectorAll('[data-en-react-surface="true"]').length,
    compatibilityRoots: document.querySelectorAll('[data-en-key="shell-root"]').length,
    composerFocused: document.activeElement === textarea,
    newSession: newSession !== undefined,
  }
})()`

const smokeReactFirstInput = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const textarea = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  const deadline = Date.now() + 5000
  while (Date.now() < deadline && textarea instanceof HTMLElement && !textarea.value.toLowerCase().includes("k")) {
    await wait(50)
  }
  return { ok: textarea instanceof HTMLElement && textarea.value.toLowerCase().includes("k"), value: textarea?.value ?? null, probe: globalThis.__oaReactInputProbe ?? null, intent: document.documentElement.dataset.reactInputIntent ?? null }
})()`

const smokeReactSidebarDestinations = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 30000
  const rows = () => [...document.querySelectorAll('[data-sidebar-destination-id]')]
  const ids = () => rows().map(row => row.getAttribute('data-sidebar-destination-id'))
  const expected = ['workspace-new-chat', 'shell-settings-toggle']
  const click = (id) => document.querySelector('[data-sidebar-destination-id="' + id + '"]')?.click()
  const waitFor = async (selector) => {
    while (Date.now() < deadline && document.querySelector(selector) === null) await wait(50)
    return document.querySelector(selector)
  }
  if (JSON.stringify(ids()) !== JSON.stringify(expected)) return { ok: false, reason: 'destination order', ids: ids() }
  const icons = rows().map(row => row.querySelector('[data-icon-name]')?.getAttribute('data-icon-name'))
  click('shell-settings-toggle')
  const settings = await waitFor('[data-react-workspace="settings"]')
  const settingsVisible = settings !== null && (settings.textContent ?? '').includes('Codex CLI')
  const settingsBack = document.querySelector('[data-sidebar-destination-id="shell-settings-toggle"]')?.textContent?.trim() === 'Back'
  const settingsIds = ids()
  click('shell-settings-toggle')
  const chat = await waitFor('[data-react-workspace="chat"]')
  const searchTrigger = document.querySelector('[aria-label="Search sessions"]')
  searchTrigger?.click()
  const search = await waitFor('input[type="search"]')
  search?.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  while (Date.now() < deadline && document.querySelector('input[type="search"]') !== null) await wait(50)
  const chatVisible = chat !== null
  const searchClosed = document.querySelector('input[type="search"]') === null
  return {
    ok: settingsVisible && settingsBack && settingsIds.includes('settings-general') && settingsIds.includes('settings-codex') &&
      settingsIds.includes('settings-account') && chatVisible && searchClosed && document.querySelector('[data-react-workspace="home"]') === null,
    ids: ids(), settingsIds, icons, settingsBack, settingsVisible, chatVisible, searchClosed,
  }
})()`

const smokeReactCollapseForReload = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 10000
  document.querySelector('.oa-react-rail-collapse')?.click()
  while (Date.now() < deadline && document.querySelector('.oa-react-workbench')?.getAttribute('data-rail-collapsed') !== 'true') await wait(50)
  const preferences = await globalThis.openagentsDesktop?.preferences?.get?.()
  return {
    ok: document.querySelector('.oa-react-workbench')?.getAttribute('data-rail-collapsed') === 'true' &&
      document.querySelector('.oa-react-sidebar-expand') !== null &&
      preferences?.presentation?.sidebarCollapsed === true,
    persisted: preferences?.presentation?.sidebarCollapsed ?? null,
    searchOpen: document.querySelector('input[type="search"]') !== null,
  }
})()`

const smokeReactArmInputProbe = `(() => {
  const editor = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  if (!(editor instanceof HTMLElement)) return false
  globalThis.__oaReactInputProbe = { keydown: 0, input: 0, change: 0, inputValue: null }
  editor.addEventListener("keydown", () => globalThis.__oaReactInputProbe.keydown++)
  editor.addEventListener("input", (event) => {
    globalThis.__oaReactInputProbe.input++
    globalThis.__oaReactInputProbe.inputValue = event.currentTarget.value
  })
  editor.addEventListener("change", () => globalThis.__oaReactInputProbe.change++)
  editor.focus()
  return true
})()`

const smokeReactImageAttachment = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 60000
  const composer = document.querySelector('[data-en-key="shell-composer"]')
  const editor = composer?.querySelector('[data-lexical-composer="true"], textarea')
  if (!(composer instanceof HTMLElement) || !(editor instanceof HTMLElement)) {
    return { ok: false, reason: 'composer unavailable' }
  }
  const valueSetter = Object.getOwnPropertyDescriptor(editor, 'value')?.set ??
    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(editor), 'value')?.set
  valueSetter?.call(editor, '')
  editor.dispatchEvent(new InputEvent('input', { bubbles: true, inputType: 'deleteContentBackward' }))
  await wait(0)
  const imageOnlyComposer = editor.value.trim() === ''
  const bytes = Uint8Array.from(atob('iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M/wHwAF/gL+Xj6mAAAAAElFTkSuQmCC'), char => char.charCodeAt(0))
  const transfer = new DataTransfer()
  transfer.items.add(new File([bytes], 'smoke-image.png', { type: 'image/png' }))
  composer.dispatchEvent(new DragEvent('drop', { bubbles: true, cancelable: true, dataTransfer: transfer }))
  let preview = document.querySelector('[data-en-key^="composer-image-preview-"]')
  while (Date.now() < deadline && preview === null) {
    await wait(50)
    preview = document.querySelector('[data-en-key^="composer-image-preview-"]')
  }
  const dataUrlRendered = preview instanceof HTMLImageElement && preview.src.startsWith('data:image/png;base64,')
  const base64Leaked = (composer.textContent ?? '').includes('iVBORw0KGgo')
  let send = composer.querySelector('button[aria-label="Send"]')
  while (Date.now() < deadline && (!(send instanceof HTMLButtonElement) || send.disabled)) {
    await wait(50)
    send = composer.querySelector('button[aria-label="Send"]')
  }
  if (!(send instanceof HTMLButtonElement) || send.disabled) {
    return { ok: false, reason: 'image-only send unavailable', preview: preview !== null, dataUrlRendered, base64Leaked }
  }
  send.click()
  while (Date.now() < deadline && document.querySelector('.oa-react-decision') === null) await wait(50)
  const decision = document.querySelector('.oa-react-decision')
  // T9 #8866: the decision surface now renders the shared DesktopApprovalCard
  // (a plain-text button, no nested <span> label) instead of the bespoke
  // fieldset/Button markup this used to target.
  const approve = decision === null ? undefined : [...decision.querySelectorAll('button')]
    .find((button) => button.textContent?.trim() === 'Approve')
  approve?.click()
  while (Date.now() < deadline && document.querySelector('.oa-react-decision') !== null) await wait(50)
  while (Date.now() < deadline && document.querySelector('[data-en-key^="composer-image-preview-"]') !== null) await wait(50)
  const previewCleared = document.querySelector('[data-en-key^="composer-image-preview-"]') === null
  return {
    ok: imageOnlyComposer && preview !== null && dataUrlRendered && !base64Leaked && approve !== undefined &&
      document.querySelector('.oa-react-decision') === null && previewCleared,
    imageOnlyComposer,
    previewRendered: preview !== null,
    dataUrlRendered,
    base64Leaked,
    approved: approve !== undefined,
    previewCleared,
  }
})()`

const smokeReactTurn = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const buttons = () => [...document.querySelectorAll('button')]
  const deadline = Date.now() + 60000
  let send = buttons().find((button) => button.textContent?.trim() === "Send" && !button.disabled)
  while (Date.now() < deadline && send === undefined) {
    await wait(50)
    send = buttons().find((button) => button.textContent?.trim() === "Send" && !button.disabled)
  }
  if (send === undefined) return {
    ok: false,
    reason: "Send never became available",
    buttons: buttons().map((button) => ({ text: button.textContent?.trim(), disabled: button.disabled })),
    composerStatus: document.querySelector('.oa-react-composer-status')?.textContent ?? null,
    heading: document.querySelector('.oa-react-conversation-heading h1')?.textContent ?? null,
    input: document.querySelector('.oa-react-composer [data-lexical-composer="true"]')?.value ?? null,
  }
  send.click()
  while (Date.now() < deadline && document.querySelector('.oa-react-decision') === null) await wait(50)
  const decision = document.querySelector('.oa-react-decision')
  const decisionOpened = decision !== null && decision.textContent?.includes("Command approval") &&
    decision.textContent?.includes("echo fixture")
  // T9 #8866: the decision surface now renders the shared DesktopApprovalCard
  // (a plain-text button, no nested <span> label) instead of the bespoke
  // fieldset/Button markup this used to target.
  const approve = decision === null ? undefined : [...decision.querySelectorAll('button')]
    .find((button) => button.textContent?.trim() === "Approve")
  approve?.click()
  while (Date.now() < deadline && document.querySelector('.oa-react-decision') !== null) await wait(50)
  const decisionReconciled = decisionOpened && approve !== undefined &&
    document.querySelector('.oa-react-decision') === null
  while (Date.now() < deadline && ![...document.querySelectorAll('.oa-react-timeline-item')]
    .some((item) => item.textContent?.includes("Codex local fixture proof."))) await wait(50)
  const assistant = [...document.querySelectorAll('.oa-react-timeline-item')]
    .find((item) => item.textContent?.includes("Codex local fixture proof."))
  const turnVisible = assistant !== undefined
  const messageItem = assistant?.closest('[data-slot="message-scroller-item"]')
  const messageContent = assistant?.closest('[data-slot="message-scroller-content"]')
  const assistantWidth = assistant?.getBoundingClientRect().width ?? 0
  const itemWidth = messageItem?.getBoundingClientRect().width ?? 0
  const contentWidth = messageContent?.getBoundingClientRect().width ?? 0
  // Real Chromium geometry falsifier for #8934: the shadcn primitive wrappers
  // must establish a definite inline size before the 720px transcript row's
  // percentage width resolves. A collapsed wrapper produced one-word lines.
  const transcriptWidthStable = assistantWidth >= 600 && assistantWidth <= 721 &&
    itemWidth >= assistantWidth && contentWidth >= itemWidth
  const reviewTrigger = buttons().find((button) => button.textContent?.trim() === "Review changes")
  const reviewSurface = document.querySelector('.oa-react-review-drawer, [data-slot="sheet-content"]')
  const forbidden = ["Stage", "Discard", "Commit", "Push", "Terminal"]
    .filter((label) => buttons().some((button) => button.textContent?.trim() === label))
  return {
    ok: decisionOpened && decisionReconciled && turnVisible && transcriptWidthStable && reviewTrigger === undefined &&
      reviewSurface === null && forbidden.length === 0,
    decisionOpened,
    decisionReconciled,
    turnVisible,
    transcriptWidthStable,
    assistantWidth,
    itemWidth,
    contentWidth,
    reviewAbsent: reviewTrigger === undefined && reviewSurface === null,
    forbidden,
  }
})()`

// Full Auto immediate-start regression: from a genuinely blank new session,
// the only action performed is clicking the composer toggle. Main must bind
// the session and start the first autonomous turn without a Send click. Turn
// the toggle back off as soon as dispatch is observed so this bounded smoke
// proves one turn rather than intentionally exercising the continuation cap.
const smokeReactFullAutoImmediate = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 60000
  const buttons = () => [...document.querySelectorAll('button')]
  const newSession = buttons().find(button => button.textContent?.trim() === 'New session')
  if (!(newSession instanceof HTMLButtonElement)) return { ok: false, reason: 'new session unavailable' }
  newSession.click()
  const timeline = () => [...document.querySelectorAll('.oa-react-timeline-item')]
  while (Date.now() < deadline && timeline().length !== 0) await wait(25)
  const toggle = document.querySelector('[data-en-key="shell-full-auto-toggle"]')
  if (!(toggle instanceof HTMLButtonElement)) return { ok: false, reason: 'Full Auto toggle unavailable' }
  const completedTurns = () => timeline().filter(item => (item.textContent ?? '').includes('Codex local fixture proof.')).length
  const completedBefore = completedTurns()
  toggle.click()
  let runningObserved = false
  let pressedObserved = false
  let turnObserved = false
  while (Date.now() < deadline && !(runningObserved || turnObserved)) {
    runningObserved = document.querySelector('[data-full-auto-status="running"]') !== null
    pressedObserved ||= toggle.getAttribute('aria-pressed') === 'true'
    turnObserved = timeline().length > 0
    if (!(runningObserved || turnObserved)) await wait(10)
  }
  // Stop the loop while its first turn is running; this click is the only
  // action after toggle-on and cannot be mistaken for a composer Send.
  const activeToggle = document.querySelector('[data-en-key="shell-full-auto-toggle"]')
  if (activeToggle instanceof HTMLButtonElement && activeToggle.getAttribute('aria-pressed') === 'true') activeToggle.click()
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-full-auto-toggle"]')?.getAttribute('aria-pressed') !== 'false') {
    await wait(25)
  }
  while (Date.now() < deadline && completedTurns() <= completedBefore) await wait(50)
  const finalToggle = document.querySelector('[data-en-key="shell-full-auto-toggle"]')
  return {
    ok: (pressedObserved || runningObserved || turnObserved) && completedTurns() > completedBefore &&
      finalToggle?.getAttribute('aria-pressed') === 'false',
    pressedObserved,
    runningObserved,
    turnObserved,
    assistantCompleted: completedTurns() > completedBefore,
    toggleStopped: finalToggle?.getAttribute('aria-pressed') === 'false',
  }
})()`

const smokeReactNavigationHistory = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 30000
  const snapshot = () => ({
    title: document.querySelector('.oa-react-conversation-heading h1')?.textContent?.trim() ?? null,
    transcript: [...document.querySelectorAll('.oa-react-timeline-item')].map(item => item.textContent?.trim() ?? ""),
  })
  const equal = (left, right) => JSON.stringify(left) === JSON.stringify(right)
  const first = snapshot()
  const alternates = [...document.querySelectorAll('[data-session-row]')]
    .filter(row => row.getAttribute('data-selected') !== 'true')
  const distinctTitle = alternates.find(row =>
    first.title !== null && !(row.textContent ?? '').includes(first.title))
  const candidates = distinctTitle === undefined
    ? alternates
    : [distinctTitle, ...alternates.filter(row => row !== distinctTitle)]
  let alternateFound = false
  for (const alternate of candidates) {
    if (!(alternate instanceof HTMLButtonElement)) continue
    alternate.click()
    const candidateDeadline = Math.min(deadline, Date.now() + 1000)
    while (Date.now() < candidateDeadline && equal(snapshot(), first)) await wait(50)
    if (!equal(snapshot(), first)) {
      alternateFound = true
      break
    }
  }
  if (!alternateFound) return { ok: false, reason: "alternate destination missing", first }
  const second = snapshot()
  const clickNavigation = async (direction, predicate) => {
    let button = [...document.querySelectorAll('button')]
      .find(candidate => candidate.getAttribute('aria-label')?.startsWith(direction))
    while (Date.now() < deadline && (!(button instanceof HTMLButtonElement) || button.disabled)) {
      await wait(50)
      button = [...document.querySelectorAll('button')]
        .find(candidate => candidate.getAttribute('aria-label')?.startsWith(direction))
    }
    if (!(button instanceof HTMLButtonElement) || button.disabled) return false
    button.click()
    while (Date.now() < deadline && !predicate(snapshot())) await wait(50)
    return predicate(snapshot())
  }
  const backFirst = await clickNavigation('Back', current => equal(current, first))
  const forwardChanged = await clickNavigation('Forward', current => !equal(current, first))
  const newSession = [...document.querySelectorAll('button')]
    .find(button => button.textContent?.trim() === 'New session')
  if (!(newSession instanceof HTMLButtonElement)) return { ok: false, reason: "new-session destination missing", first, second }
  newSession.click()
  const isBlankNewSession = current =>
    (current.title === 'New chat' || current.title === 'New session') && current.transcript.length === 0
  while (Date.now() < deadline && !isBlankNewSession(snapshot())) await wait(50)
  const third = snapshot()
  const forwardAtEnd = [...document.querySelectorAll('button')]
    .find(candidate => candidate.getAttribute('aria-label')?.startsWith('Forward'))
  return {
    ok: !equal(first, second) && backFirst && forwardChanged && isBlankNewSession(third) &&
      forwardAtEnd instanceof HTMLButtonElement && forwardAtEnd.disabled,
    first,
    second,
    third,
    backFirst,
    forwardChanged,
    forwardDisabledAtEnd: forwardAtEnd instanceof HTMLButtonElement ? forwardAtEnd.disabled : null,
  }
})()`

const smokeReactReloadNewSession = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 15000
  while (Date.now() < deadline && document.querySelector('[data-en-react-surface="true"]') === null) await wait(50)
  const collapsedAtMount = document.querySelector('.oa-react-workbench')?.getAttribute('data-rail-collapsed') === 'true'
  const searchClosedAtMount = document.querySelector('input[type="search"]') === null
  const composerAtMount = document.querySelector('.oa-react-composer [data-lexical-composer="true"]')
  const composerFocusedAtMount = composerAtMount !== null && document.activeElement === composerAtMount
  await wait(2000)
  const timeline = [...document.querySelectorAll('.oa-react-timeline-item')]
  const heading = document.querySelector('.oa-react-conversation-heading h1')?.textContent?.trim() ?? null
  const newSessionPreserved = (heading === "New chat" || heading === "New session") && timeline.length === 0
  return {
    ok: newSessionPreserved && collapsedAtMount && searchClosedAtMount && composerFocusedAtMount &&
      document.documentElement.dataset.desktopRenderer === "react" &&
      document.querySelector('[data-en-key="shell-root"]') === null,
    newSessionPreserved,
    collapsedAtMount,
    searchClosedAtMount,
    composerFocusedAtMount,
    backend: document.documentElement.dataset.desktopRenderer,
    heading,
    timeline: timeline.map((item) => item.textContent),
    sessions: [...document.querySelectorAll('[data-session-row]')].map((item) => item.textContent),
  }
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
  return {
    ok: browser !== null && tree !== null && search !== null && boundary !== null &&
      legacyEditor === null && editor !== null && saveAsPath !== null && recoveryStored && !leakedRoot &&
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
  while (Date.now() < deadline && document.querySelector('[data-en-key="shell-transcript"]') === null) {
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
  document.querySelector('[data-en-key="workspace-new-chat"]')?.click()
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Pixel-proof: real chat rows on the shared catalog"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  return { ok: true, typed: input.value }
})()`

const smokeCodexHistoryDetails = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const first = document.querySelector('[data-session-row], [data-en-key^="sidebar-thread-"]')
  if (first === null) return { ok: false, reason: "history row never mounted" }
  first.click()
  const deadline = Date.now() + 15000
  while (Date.now() < deadline &&
    first.getAttribute("data-selected") !== "true" &&
    first.getAttribute("aria-current") !== "page" &&
    document.querySelector('[data-en-key="history-workspace-split"]') === null) {
    await wait(100)
  }
  const sidebar = document.querySelector('[data-en-key="sidebar-history-list"] > [data-en-role="section-label"]')
  const selectedInReact = first.getAttribute("data-selected") === "true" || first.getAttribute("aria-current") === "page"
  const detailVisible = document.querySelector('[data-en-key="history-workspace-split"]') !== null ||
    (selectedInReact && document.querySelector('[data-en-key="shell-transcript"]') !== null)
  // The header states the bounded recent scope. Prior smoke passes may persist
  // another fixture session, so accept the truthful one-through-ten count.
  const truthfulCount = /^Recent chats · (?:[1-9]|10)$/.test(sidebar?.textContent ?? "")
  return { ok: truthfulCount && detailVisible, header: sidebar?.textContent ?? null, selectedInReact }
})()`

// #8787 (owner verbatim: "the text input should be focused immediately on
// open. so i can start typing right away."): at shell-interactable the
// composer already holds keyboard focus, and it STILL holds it after the
// background history hydration settles (90bce8d89b boot order) — hydration
// must never steal open-time focus. No pointer event happens before this step.
const smokeComposerFocusedOnOpen = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const composer = () => document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const composer = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const newChat = document.querySelector('[data-en-key="workspace-new-chat"]')
  if (newChat === null) return { ok: false, reason: "New session control missing" }
  newChat.click()
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
    "workspace-chat",
    "workspace-home",
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
  const expectedDock = ["workspace-new-chat", "shell-settings-toggle"]
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
  const historyDeadline = Date.now() + 10000
  while (Date.now() < historyDeadline && document.querySelector('[data-en-key="history-workspace-split"]') === null) {
    await wait(50)
  }
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
  const composer = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  // Owner contract (EP250): "when i do new chat, clicking button or command
  // N, auto focus the input." The focus retry loop lands AFTER the chat
  // view mounts, so poll for it.
  const focusDeadline = Date.now() + 3000
  while (Date.now() < focusDeadline && document.activeElement !== composer) {
    await wait(50)
  }
  return {
    ok: transcript !== null && split === null && messages === 0 &&
      composer !== null && !("disabled" in composer && composer.disabled === true) &&
      composer.getAttribute("contenteditable") !== "false" &&
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  while (Date.now() < deadline && find("shell-transcript") === null) await wait(25)
  const registeredFocus = find("shell-transcript") !== null
  const newChat = find("workspace-new-chat")
  if (newChat instanceof HTMLElement) { newChat.click(); await wait(50) }
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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

const smokeAskUserQuestionOpen = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const newChat = document.querySelector('[data-en-key="workspace-new-chat"], button[aria-label="New session"]')
  const hasConversationContent = document.querySelector('.oa-react-timeline-item') !== null
  if (hasConversationContent && newChat instanceof HTMLButtonElement) {
    newChat.click()
    await wait(300)
  }
  const harness = document.querySelector('[data-en-key="shell-harness-select"]')
  const providerButton = document.querySelector('[data-en-key="shell-provider-select"], button[aria-label^="Provider:"]')
  if (harness instanceof HTMLSelectElement) {
    harness.value = "fable"
    harness.dispatchEvent(new Event("change", { bubbles: true }))
  } else if (providerButton instanceof HTMLButtonElement) {
    const providerLabel = () => providerButton.getAttribute("aria-label") ?? providerButton.textContent ?? ""
    if (providerLabel().includes("Provider: Codex")) providerButton.click()
    const providerDeadline = Date.now() + 5000
    while (Date.now() < providerDeadline && providerLabel().includes("Provider: Codex")) await wait(50)
    if (providerLabel().includes("Provider: Codex")) {
      return { ok: false, reason: "Fable provider was not selectable", disabled: providerButton.disabled, providerLabel: providerLabel() }
    }
  } else return { ok: false, reason: "provider control unavailable" }
  await wait(50)
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
  if (!(input instanceof HTMLElement)) return { ok: false, reason: "composer unavailable" }
  input.focus()
  const valueSetter = Object.getOwnPropertyDescriptor(input, "value")?.set ??
    Object.getOwnPropertyDescriptor(Object.getPrototypeOf(input), "value")?.set
  valueSetter?.call(input, "Prove AskUserQuestion round trip")
  input.dispatchEvent(new Event("input", { bubbles: true }))
  let send
  const sendDeadline = Date.now() + 5000
  while (Date.now() < sendDeadline) {
    send = document.querySelector('[data-en-key="shell-composer"] button[aria-label="Send"]')
    if (send instanceof HTMLButtonElement && !send.disabled) break
    await wait(50)
  }
  if (!(send instanceof HTMLButtonElement) || send.disabled) return { ok: false, reason: "Send stayed disabled" }
  send.click()
  const deadline = Date.now() + 20000
  while (Date.now() < deadline && document.querySelector('.oa-react-decision') === null) await wait(50)
  // Prove the card survives the immediate post-event render/focus cycle, not
  // merely one transient DOM frame that disappears before a user can act.
  await wait(250)
  const decision = document.querySelector('.oa-react-decision')
  const other = decision?.querySelector('textarea[aria-label="Other answer for Which implementation should the agent use?"]')
  const text = decision?.textContent ?? ""
  const waiting = document.querySelector('[aria-label="Waiting for your answer"]')
  return {
    ok: decision !== null && other !== null && text.includes("Which implementation should the agent use?") &&
      text.includes("Typed") && text.includes("Keep the schema-decoded bridge.") &&
      text.includes("Direct") && text.includes("Other") && waiting !== null &&
      document.querySelector('.oa-react-working') === null,
    decision: decision !== null,
    other: other !== null && other !== undefined,
    waiting: waiting !== null,
    genericWorking: document.querySelector('.oa-react-working') !== null,
    provider: providerButton?.getAttribute("aria-label") ?? providerButton?.textContent ?? null,
    composerValue: input.value ?? null,
    transcript: (document.querySelector('[data-en-key="shell-transcript"]')?.textContent ?? "").slice(-1200),
  }
})()`

const smokeAskUserQuestionAnswer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const other = document.querySelector('.oa-react-decision textarea[aria-label="Other answer for Which implementation should the agent use?"]')
  if (!(other instanceof HTMLTextAreaElement)) return { ok: false, reason: "Other answer field unavailable" }
  const setter = Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype, "value")?.set
  setter?.call(other, "Use the typed path")
  other.dispatchEvent(new Event("input", { bubbles: true }))
  const buttonDeadline = Date.now() + 5000
  let submit
  while (Date.now() < buttonDeadline) {
    submit = [...document.querySelectorAll('.oa-react-decision button')]
      .find(button => button.textContent?.trim() === "Submit answer")
    if (submit instanceof HTMLButtonElement && !submit.disabled) break
    await wait(50)
  }
  if (!(submit instanceof HTMLButtonElement) || submit.disabled) return { ok: false, reason: "Submit answer stayed disabled" }
  submit.click()
  const transcriptText = () => document.querySelector('.oa-react-timeline-region, [data-en-key="shell-transcript"]')?.textContent ?? ""
  const deadline = Date.now() + 20000
  while (Date.now() < deadline && (document.querySelector('.oa-react-decision') !== null ||
    !transcriptText().includes("Answer received: Use the typed path.") ||
    document.querySelector('.oa-react-working') !== null)) {
    await wait(50)
  }
  const transcript = transcriptText()
  const agentReceivedAnswer = transcript.includes("Answer received: Use the typed path.")
  const turnSettled = document.querySelector('.oa-react-working') === null
  return {
    ok: document.querySelector('.oa-react-decision') === null && agentReceivedAnswer && turnSettled,
    dialogClosed: document.querySelector('.oa-react-decision') === null,
    answeredCard: transcript.includes("Answered") && transcript.includes("Use the typed path"),
    agentReceivedAnswer,
    turnSettled,
    transcript: transcript.slice(-1200),
    body: document.body.innerText.slice(-2000),
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const input = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  let inspector = document.querySelector('[data-en-key="chat-message-inspector"]')
  let text = inspector === null ? "" : (inspector.textContent || "")
  while (Date.now() < deadline && (
    inspector === null ||
    !text.includes("gpt-5.6-sol") ||
    !text.includes("codex-local") ||
    !text.includes("Tokens (total)") ||
    !text.includes("952")
  )) {
    await wait(50)
    inspector = document.querySelector('[data-en-key="chat-message-inspector"]')
    text = inspector === null ? "" : (inspector.textContent || "")
  }
  if (inspector === null) return { ok: false, reason: "message inspector never opened" }
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
  const composer = document.querySelector('[data-en-key="shell-input"] [data-lexical-composer="true"], [data-en-key="shell-input"] textarea, [data-en-key="shell-input"] input')
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
  const assertHeadlessPresentation = (): void => {
    if (hiddenAutomationMode && window.isVisible()) {
      throw new Error("headless smoke unexpectedly exposed the desktop window")
    }
  }
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
    if (smokeMode) {
      try {
        rmSync(app.getPath("userData"), { recursive: true, force: true, maxRetries: 5, retryDelay: 50 })
      } catch {
        // This is an OS-temporary smoke profile. Chromium may finish one last
        // cache write after host teardown; cleanup cannot invalidate an
        // otherwise green lifecycle receipt or crash the smoke coordinator.
      }
    }
    app.exit(ok ? code : 1)
  }
  const timeout = setTimeout(() => {
    console.error("[openagents-desktop smoke] TIMEOUT waiting for renderer")
    finish(1)
  }, reactSmokeMode ? 90_000 : 45_000)
  let tracePass = 0
  window.webContents.on("did-finish-load", () => {
    void (async () => {
      const step = async (name: string, script: string): Promise<void> => {
        assertHeadlessPresentation()
        const result: unknown = await window.webContents.executeJavaScript(script, true)
        const ok =
          result === true ||
          (typeof result === "object" && result !== null && (result as { ok?: unknown }).ok === true)
        if (!ok) {
          throw new Error(`${name} failed: ${JSON.stringify(result)}`)
        }
        assertHeadlessPresentation()
        console.log(`[openagents-desktop smoke] ${name} OK`, JSON.stringify(result))
      }
      try {
        if (reactSmokeMode) {
          if (tracePass === 1) {
            await step("react-reload-new-session", smokeReactReloadNewSession)
            clearTimeout(timeout)
            console.log("[openagents-desktop smoke] REACT OK")
            finish(0)
            return
          }
          await step("react-workbench-exclusive", smokeReactWorkbench)
          if (process.env.OPENAGENTS_DESKTOP_SMOKE_QUESTION_ONLY === "1") {
            await step("ask-user-question-opens", smokeAskUserQuestionOpen)
            await captureShot(window, "14-ask-user-question-pending")
            await step("ask-user-question-round-trip", smokeAskUserQuestionAnswer)
            await captureShot(window, "15-ask-user-question-answered")
            clearTimeout(timeout)
            finish(0)
            return
          }
          await step("react-sidebar-destinations", smokeReactSidebarDestinations)
          await captureShot(window, "react-sidebar-expanded")
          await step("react-image-attachment", smokeReactImageAttachment)
          const imageReceipt = codexAppServerSmoke?.receipt()
          if (imageReceipt?.localImageTurns !== 1 ||
              imageReceipt.maxLocalImageCount !== 1) {
            throw new Error(`react-image-attachment-receipt failed: ${JSON.stringify(imageReceipt)}`)
          }
          console.log(
            "[openagents-desktop smoke] react-image-attachment-receipt OK",
            JSON.stringify(imageReceipt),
          )
          await step("react-input-probe-armed", smokeReactArmInputProbe)
          window.webContents.focus()
          window.webContents.sendInputEvent({ type: "keyDown", keyCode: "K" })
          window.webContents.sendInputEvent({ type: "char", keyCode: "k" })
          window.webContents.sendInputEvent({ type: "keyUp", keyCode: "K" })
          await step("react-first-keystroke", smokeReactFirstInput)
          await step("react-turn", smokeReactTurn)
          const authoritativeDecision = codexAppServerSmoke?.receipt()
          if (authoritativeDecision?.requestId !== 92 ||
              authoritativeDecision.decision !== "accept" ||
              !authoritativeDecision.completionEmitted) {
            throw new Error(`react-authoritative-decision failed: ${JSON.stringify(authoritativeDecision)}`)
          }
          console.log(
            "[openagents-desktop smoke] react-authoritative-decision OK",
            JSON.stringify(authoritativeDecision),
          )
          await step("react-full-auto-immediate", smokeReactFullAutoImmediate)
          await step("react-navigation-history", smokeReactNavigationHistory)
          await step("runtime-gateway-bootstrap", smokeRuntimeGatewayBootstrap)
          await step("react-sidebar-collapse-persisted", smokeReactCollapseForReload)
          await captureShot(window, "react-sidebar-collapsed")
          tracePass = 1
          window.webContents.reload()
          return
        }
        if (tracePass === 1) {
          await step("workspace-editor-reload-recovery", smokeWorkspaceEditorRecovery)
          clearTimeout(timeout)
          console.log("[openagents-desktop smoke] OK")
          finish(0)
          return
        }
        await step("shell-mounted", smokeWaitForShell)
        if (process.env.OPENAGENTS_DESKTOP_SMOKE_QUESTION_ONLY === "1") {
          await step("ask-user-question-opens", smokeAskUserQuestionOpen)
          await captureShot(window, "14-ask-user-question-pending")
          await step("ask-user-question-round-trip", smokeAskUserQuestionAnswer)
          await captureShot(window, "15-ask-user-question-answered")
          clearTimeout(timeout)
          finish(0)
          return
        }
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
        // The broad compatibility smoke retains the bounded codex-exec JSONL
        // fixture; the React smoke uses the protocol-speaking app-server peer
        // installed above. Production builds never set this env var.
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
  const bindingForNativeMenu = (command: DesktopCommandDefinition): string | undefined =>
    nativeCommandAccelerator(
      bindings === undefined
        ? command.defaultBindings
        : (commandBindingForNativeMenu(bindings, command.id) === undefined
            ? []
            : [commandBindingForNativeMenu(bindings, command.id)!]),
    )
  const fullscreenCommand = desktopCanonicalCommandRegistry.find(command =>
    command.id === "window.fullscreen_toggle")
  const commandItems: MenuItemConstructorOptions[] = desktopCanonicalCommandRegistry
    // Fullscreen is native window chrome, not a generic app command. Keeping
    // it out of this list also prevents two menu items from claiming Cmd+F.
    .filter(command => command.id !== "window.fullscreen_toggle")
    .filter(command => command.palette || command.defaultBindings.length > 0 ||
      (bindings !== undefined && commandBindingForNativeMenu(bindings, command.id) !== undefined))
    .map(command => ({
    label: command.label,
    ...(bindingForNativeMenu(command) === undefined
      ? {}
      : { accelerator: bindingForNativeMenu(command) }),
    click: () => {
      dispatchNativeDesktopCommand(command, {
        hasOpenWindow: () => BrowserWindow.getAllWindows().some(window => !window.isDestroyed()),
        openWindow: () => { createWindow() },
        enqueue: desktopCommandHost.enqueue,
      })
    },
    }))
  const template: MenuItemConstructorOptions[] = [
    ...(process.platform === "darwin"
      ? [{ label: app.name, submenu: [{ role: "about" as const }, { type: "separator" as const }, { role: "quit" as const }] }]
      : []),
    { label: "Commands", submenu: commandItems },
    { label: "Edit", submenu: [{ role: "undo" }, { role: "redo" }, { type: "separator" }, { role: "cut" }, { role: "copy" }, { role: "paste" }, { role: "selectAll" }] },
    {
      label: "Window",
      submenu: [
        { role: "minimize" },
        { type: "separator" },
        {
          role: "togglefullscreen",
          ...(fullscreenCommand === undefined || bindingForNativeMenu(fullscreenCommand) === undefined
            ? {}
            : { accelerator: bindingForNativeMenu(fullscreenCommand) }),
        },
        { type: "separator" },
        { role: "close" },
      ],
    },
  ]
  Menu.setApplicationMenu(Menu.buildFromTemplate(template))
}

void app.whenReady().then(async () => {
  if (!primaryDesktopInstance) return
  const updateRecovery = await desktopUpdateHost.reconcile()
  if (updateRecoveryRequiresStartupExit(updateRecovery)) return
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
  if (process.platform === "darwin") {
    if (hiddenAutomationMode) app.dock?.hide()
    else app.dock?.setIcon(desktopIconPath)
  }
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
      if (isolatedWorkspaceRoot === null && !smokeMode) {
        // Every ordinary launch starts in the directory that launched it.
        // This deliberately supersedes stale persisted navigation; opening a
        // different catalog session or choosing another folder can replace it
        // after startup.
        syncHost.codingCatalog()?.selectWorkspace(desktopLaunchWorkingDirectory)
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
      if (desktopSessionState === "session_ready") {
        Effect.runFork(desktopCodexUsageReporter.flush())
      }
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
  // FA-H12 (#8885): Full Auto two-process restart probe
  // (scripts/full-auto-restart-smoke.ts). Mirrors the local-turn restart probe
  // above: a seed process writes durable state (thread + COMPLETED fixture
  // turn in the local-turn journal + enabled Full Auto registry record bound
  // to the fixture workspace) and quits; a separate resume process relaunches
  // against the same userData directory and observes the REAL startup
  // reconciliation (localTurnRecovery -> runFullAutoReconciliation wiring
  // below) dispatch a fixture continuation -- or fail closed on the
  // deliberately mismatched workspace variant.
  if (fullAutoRestartProbe !== null) {
    // Windowless probe mode keeps the original fully-settled ordering.
    openLocalSyncPersistence()
    await recoverSessionVaultLocal()
    await settleSessionRecovery()
    runtimeGateway.start()
    try {
      await localTurnRecovery
      const seedTurnRef = "turn.full-auto-restart-smoke.seed"
      if (
        fullAutoRestartProbe === "seed" || fullAutoRestartProbe === "seed-mismatch" ||
        fullAutoRestartProbe === "seed-claude"
      ) {
        const store = threads()
        const thread = store.newThread()
        const claudeVariant = fullAutoRestartProbe === "seed-claude"
        const key = {
          threadRef: thread.id,
          turnRef: seedTurnRef,
          lane: claudeVariant ? "fable-local" as const : "codex-local" as const,
        }
        const accountRef = claudeVariant ? FABLE_LOCAL_FIXTURE_ACCOUNT.ref : FIXTURE_CODEX_LOCAL_ACCOUNT.ref
        const model = claudeVariant ? FABLE_LOCAL_MODEL : "gpt-5.6-sol"
        localTurnJournal.accept({
          ...key,
          userMessageKey: `${seedTurnRef}-user`,
          assistantMessageKey: `${seedTurnRef}-assistant`,
          accountRef,
          model,
        })
        store.upsert(thread.id, {
          key: `${seedTurnRef}-user`, role: "user", text: "Seed a Full Auto loop that must survive a restart.", timestamp: "11:55 PM",
        })
        localTurnJournal.recordDispatch(key, accountRef)
        localTurnJournal.recordProviderSession(key, {
          accountRef,
          providerSessionRef: "thread-full-auto-restart-smoke",
        })
        localTurnJournal.appendAssistantText(key, "Seed turn complete. ")
        store.upsert(thread.id, {
          key: `${seedTurnRef}-assistant`, role: "assistant", text: "Seed turn complete. ", timestamp: "11:55 PM",
        })
        // COMPLETED terminal seed turn: nothing is in flight, so the resume
        // phase's dispatch decision is purely the durable registry's.
        localTurnJournal.terminal(key, "completed", "completed")
        // Happy path binds the EXACT workspace the resume process will resolve
        // (fixture-mode resolution is a pure function of the shared userData
        // path). The mismatch variant binds a different absolute path so the
        // resume phase must fail CLOSED (FA-H2) instead of dispatching.
        const grantedWorkspaceRef = fullAutoRestartProbe === "seed" || claudeVariant
          ? resolveDesktopLocalWorkspaceRoot()
          : path.join(app.getPath("userData"), "not-the-granted-workspace")
        fullAutoRegistry.set(thread.id, true, {
          workspaceRef: grantedWorkspaceRef,
          ...(claudeVariant
            ? { profile: { lane: "fable-local", accountRef, model } }
            : {}),
        })
        // Happy path: consume all but ONE cap slot so the resume phase
        // dispatches exactly one continuation and then deterministically
        // disables at the cap -- a bounded single-dispatch observation
        // instead of a 20-turn loop.
        if (fullAutoRestartProbe === "seed" || claudeVariant) {
          for (let index = 0; index < FULL_AUTO_MAX_CONTINUATIONS - 1; index++) {
            fullAutoRegistry.incrementContinuation(thread.id)
          }
        }
        console.log(`[openagents-desktop full-auto-restart] phase-a seeded ${JSON.stringify({
          variant: fullAutoRestartProbe,
          enabled: fullAutoRegistry.get(thread.id),
          continuationCount: fullAutoRegistry.record(thread.id)?.continuationCount ?? null,
        })}`)
        app.exit(0)
        return
      }
      // Resume phases: the startup pass queued below (localTurnRecovery ->
      // runFullAutoReconciliation({ startup: true })) plus the chained
      // post-completion pass drive the registry to a deterministic terminal
      // state; poll durable state (bounded) instead of racing the queue.
      const expectMismatch = fullAutoRestartProbe === "resume-mismatch"
      const expectClaude = fullAutoRestartProbe === "resume-claude"
      const seedRecord = localTurnJournal.list().find(value => value.turnRef === seedTurnRef) ?? null
      const seeded = seedRecord !== null && seedRecord.phase === "completed"
      const threadRef = seedRecord?.threadRef ?? ""
      const deadline = Date.now() + 120_000
      const settled = (): boolean => {
        const current = fullAutoRegistry.record(threadRef)
        if (current === null || current.enabled) return false
        return current.blockedReason === (expectMismatch ? "workspace_mismatch" : "continuation_cap_reached")
      }
      while (!settled() && Date.now() < deadline) await new Promise(resolve => setTimeout(resolve, 50))
      const record = fullAutoRegistry.record(threadRef)
      const continuationRecords = localTurnJournal.list().filter(value =>
        value.threadRef === threadRef && value.turnRef.startsWith("turn.full-auto."))
      const dispatchedTurnRef = continuationRecords[0]?.turnRef ?? null
      const dispatchedLane = continuationRecords[0]?.lane ?? null
      const notes = threads().open(threadRef)?.notes ?? []
      const continuationAssistantText = dispatchedTurnRef === null
        ? ""
        : notes.filter(note => note.role === "assistant" && note.key.startsWith(`${dispatchedTurnRef}-assistant`))
          .map(note => note.text).join("")
      // FA-H7 pinned semantic: DISABLING zeroes continuationCount, so the
      // cap-disabled terminal record reads 0. The advancement 19 -> 20 is
      // still durably proven twice over: the journal holds exactly one
      // completed dispatched continuation, and the cap disable itself only
      // fires when the registry count reached FULL_AUTO_MAX_CONTINUATIONS.
      const advancedContinuationCount = (FULL_AUTO_MAX_CONTINUATIONS - 1) + continuationRecords.length
      const ok = expectMismatch
        ? seeded && record !== null && !record.enabled && record.blockedReason === "workspace_mismatch" &&
          continuationRecords.length === 0 && record.continuationCount === 0 &&
          notes.some(note => note.role === "system" && note.text.includes("no longer matches"))
        : seeded && record !== null && !record.enabled && record.blockedReason === "continuation_cap_reached" &&
          record.continuationCount === 0 &&
          advancedContinuationCount === FULL_AUTO_MAX_CONTINUATIONS &&
          typeof record.pendingTurnRef !== "string" &&
          continuationRecords.length === 1 && continuationRecords[0]!.phase === "completed" &&
          dispatchedTurnRef !== null &&
          dispatchedLane === (expectClaude ? "fable-local" : "codex-local") &&
          (expectClaude
            ? continuationAssistantText.includes("Fable local")
            : continuationAssistantText.includes("fixture")) &&
          notes.some(note => note.key === `${dispatchedTurnRef}-user`)
      console.log(`[openagents-desktop full-auto-restart] phase-b ${JSON.stringify({
        variant: fullAutoRestartProbe,
        seeded,
        resumed: continuationRecords.length > 0,
        dispatchedTurnRefPresent: dispatchedTurnRef !== null,
        dispatchedLane,
        continuationCount: expectMismatch ? record?.continuationCount ?? null : advancedContinuationCount,
        blockedReason: record?.blockedReason ?? null,
        ok,
      })}`)
      app.exit(ok ? 0 : 1)
      return
    } catch (error) {
      console.error("[openagents-desktop full-auto-restart] failed", error instanceof Error ? error.message : "unknown")
      app.exit(1)
      return
    }
  }
  // FA-H13 (#8886): the Full Auto control live-proof probe
  // (scripts/full-auto-control-smoke.ts). Windowless fixture mode, same
  // posture as the restart probes above: seed one thread with an enabled,
  // workspace-bound Full Auto record, print a ready line, then keep the
  // process alive (bounded) so an external client can exercise the loopback
  // control API -- started by the OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL wiring
  // above -- against the REAL running Electron main. The outer smoke script
  // ends the probe by writing full-auto/control-probe-stop under userData.
  if (fullAutoControlProbe) {
    openLocalSyncPersistence()
    await recoverSessionVaultLocal()
    await settleSessionRecovery()
    runtimeGateway.start()
    try {
      await localTurnRecovery
      // Let the startup reconciliation pass (queued at module scope) settle
      // FIRST, against the still-empty registry, so seeding the enabled
      // record afterwards cannot start a fixture dispatch loop: the probe
      // proves the control wire end to end, not another dispatch.
      await runFullAutoReconciliation()
      const store = threads()
      const thread = store.newThread()
      store.upsert(thread.id, {
        key: "turn.full-auto-control-probe-user",
        role: "user",
        text: "Seed a Full Auto thread for the control-surface live proof.",
        timestamp: "11:55 PM",
      })
      fullAutoRegistry.set(thread.id, true, { workspaceRef: resolveDesktopLocalWorkspaceRoot() })
      console.log(`[openagents-desktop full-auto-control] probe ready ${JSON.stringify({ threadRef: thread.id })}`)
      const stopFile = path.join(app.getPath("userData"), "full-auto", "control-probe-stop")
      const probeDeadline = Date.now() + 120_000
      while (!existsSync(stopFile) && Date.now() < probeDeadline) {
        await new Promise(resolve => setTimeout(resolve, 250))
      }
      app.exit(0)
      return
    } catch (error) {
      console.error("[openagents-desktop full-auto-control] probe failed", error instanceof Error ? error.message : "unknown")
      app.exit(1)
      return
    }
  }
  // QA-3 (#8908): the visual-baseline capture probe
  // (scripts/visual-baseline-smoke.ts). Windowless like the probes above, but
  // it needs NO runtime wiring at all: the renderer's `?visualBaseline=<name>`
  // mode mounts a frozen fixture shell state with no preload bridge, so the
  // probe only drives navigation, readiness polling, and capturePage.
  if (visualBaselineProbe) {
    const shotsDir = process.env.OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS
    if (shotsDir === undefined || shotsDir === "") {
      console.error("[openagents-desktop visual-baseline] OPENAGENTS_DESKTOP_VISUAL_BASELINE_SHOTS is required")
      app.exit(1)
      return
    }
    try {
      mkdirSync(shotsDir, { recursive: true })
      const window = new BrowserWindow({
        width: VISUAL_BASELINE_WINDOW.width,
        height: VISUAL_BASELINE_WINDOW.height,
        useContentSize: true,
        show: false,
        frame: false,
        // khalaTheme color.background, same as createWindow.
        backgroundColor: "#05070d",
        webPreferences: {
          offscreen: true,
          contextIsolation: true,
          nodeIntegration: false,
          nodeIntegrationInSubFrames: false,
          sandbox: true,
          webviewTag: false,
          webSecurity: true,
          spellcheck: false,
          backgroundThrottling: false,
        },
      })
      const captured: Array<VisualBaselineCaptureReceipt> = []
      for (const stateName of VISUAL_BASELINE_STATES) {
        await window.loadURL(`${desktopRendererEntryUrl}?visualBaseline=${stateName}`)
        const deadline = Date.now() + 30_000
        let ready = false
        while (Date.now() < deadline) {
          const status = await window.webContents.executeJavaScript(
            "({ ready: document.documentElement.dataset.visualBaselineReady ?? null, error: document.documentElement.dataset.visualBaselineError ?? null })",
          ) as { ready: string | null; error: string | null }
          if (status.error !== null) throw new Error(`renderer fixture failed for ${stateName}: ${status.error}`)
          if (status.ready === "1") {
            ready = true
            break
          }
          await new Promise(resolve => setTimeout(resolve, 50))
        }
        if (!ready) throw new Error(`renderer never signaled visual-baseline ready for ${stateName}`)
        const rawImage = await window.webContents.capturePage()
        const rawSize = rawImage.getSize()
        // macOS offscreen rendering ignores force-device-scale-factor and
        // rasters at the primary display's backing scale. Normalize to the
        // fixed 1x geometry so baselines are Retina-independent: resize is a
        // pure function of the captured bitmap (same input -> same bytes) and
        // a no-op when the capture is already 1x.
        const image = rawSize.width === VISUAL_BASELINE_WINDOW.width && rawSize.height === VISUAL_BASELINE_WINDOW.height
          ? rawImage
          : rawImage.resize({
              width: VISUAL_BASELINE_WINDOW.width,
              height: VISUAL_BASELINE_WINDOW.height,
              quality: "best",
            })
        const png = image.toPNG()
        const size = image.getSize()
        if (size.width !== VISUAL_BASELINE_WINDOW.width || size.height !== VISUAL_BASELINE_WINDOW.height) {
          throw new Error(`capture for ${stateName} is ${size.width}x${size.height}, expected ${VISUAL_BASELINE_WINDOW.width}x${VISUAL_BASELINE_WINDOW.height}`)
        }
        const file = `${stateName}.png`
        writeFileSync(path.join(shotsDir, file), png)
        captured.push({
          state: stateName,
          file,
          sha256: createHash("sha256").update(png).digest("hex"),
          width: size.width,
          height: size.height,
        })
      }
      console.log(`[openagents-desktop visual-baseline] captured ${JSON.stringify({
        ok: true,
        window: VISUAL_BASELINE_WINDOW,
        deviceScaleFactor: VISUAL_BASELINE_DEVICE_SCALE_FACTOR,
        states: captured,
      })}`)
      app.exit(0)
      return
    } catch (error) {
      console.error("[openagents-desktop visual-baseline] probe failed", error instanceof Error ? error.message : "unknown")
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
  // Register the smoke driver before any readiness await. `did-finish-load`
  // may fire while persistence/provider initialization is in flight; a late
  // listener would miss the one-shot event and report a false timeout.
  if (smokeMode && !startupMarksMode) runSmoke(window)
  const rendererReady = new Promise<string>((resolve, reject) => {
    const timer = setTimeout(() => reject(new Error("renderer_ready_timeout")), 30_000)
    window.webContents.once("did-finish-load", () => { clearTimeout(timer); resolve(new Date().toISOString()) })
    if (!window.webContents.isLoading()) { clearTimeout(timer); resolve(new Date().toISOString()) }
  })
  recordMainMark("windowCreated")
  openLocalSyncPersistence()
  await recoverSessionVaultLocal()
  runtimeGateway.start()
  const rendererReadyAt = await rendererReady
  await ensureAcpProviders()
  const providerReadyAt = new Date().toISOString()
  // Reaching this point proves the replacement build initialized its native
  // main host, renderer window, local persistence, session custody, and
  // provider gateway. Only now may the retained previous slot become a
  // user-visible rollback option rather than an automatic health fallback.
  const launchHealth = await desktopUpdateHost.recordHealthyLaunch({ rendererReadyAt, providerReadyAt })
  if (launchHealth.phase === "restarting") {
    const drain = await drainDesktopUpdateRuntimes()
    if (!drain.ok) {
      app.exit(1)
      return
    }
    if (!desktopUpdateHost.recordCleanShutdown(drain)) {
      app.exit(1)
      return
    }
    app.relaunch()
    app.exit(0)
    return
  }
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
    console.error("[openagents-desktop mvp-proof] the MVP proof requires an isolated temp profile, an isolated workspace, and no other driver mode")
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
  if (smokeMode) return
  if (mvpProof.enabled) {
    const workspaceRoot = isolatedAppProofWorkspaceRoot({ enabled: isolatedAppProofMode, env: process.env })
    runMvpProof(window, {
      outDir: mvpProof.outDir,
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
  if (!desktopUpdateDrainActive && (process.platform !== "darwin" || smokeMode)) {
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
  codexControlPlanes.close()
  codexThreadLifecycles.close()
  codexEcosystems.close()
  codexHostServices.close()
  codexExperimentalRuntimes.close()
  codexDurableQueue.close()
  codexAppServerSupervisor.close()
  usageLedger.dispose()
  desktopCorrelationJournal.dispose()
  void acpProviderHost.shutdown()
})

app.on("activate", () => {
  if (BrowserWindow.getAllWindows().length === 0) {
    createWindow()
  }
})
