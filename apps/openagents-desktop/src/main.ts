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
import { randomUUID } from "node:crypto"
import { rmSync } from "node:fs"
import { BrowserWindow, Menu, app, dialog, ipcMain, safeStorage, session, shell, type IpcMainInvokeEvent, type MenuItemConstructorOptions } from "electron"
import { Effect } from "effect"
import {
  buildInterruptTurnIntent,
  buildStartTurnIntent,
} from "@openagentsinc/khala-sync-client"

// macOS derives the running application/menu identity before `ready`; set
// both Electron's application name and the process title at module startup so
// development (`electron .`) and packaged launches agree on "OpenAgents".
app.setName("OpenAgents")
process.title = "OpenAgents"

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
import { DesktopChatTurnChannel, DesktopHydrateThreadChannel, DesktopNewThreadChannel, DesktopOpenThreadChannel, DesktopThreadsChannel, decode, DesktopThreadRequestSchema, DesktopTurnRequestSchema, type DesktopMessage } from "./chat-contract.ts"
import {
  FABLE_LOCAL_FINAL_TEXT_LIMIT,
  FableLocalAnswerQuestionChannel,
  FableLocalAvailabilityChannel,
  FableLocalEventChannel,
  FableLocalInterruptChannel,
  FableLocalStartChannel,
  decodeFableLocalAnswerQuestionRequest,
  decodeFableLocalInterruptRequest,
  decodeFableLocalStartRequest,
  fableLocalFailureMessage,
  fableLocalModelNoteText,
  fableLocalTraceNoteMeta,
  fableLocalTraceNoteText,
} from "./fable-local-contract.ts"
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
  makeCodexAccountHealth,
  makeCodexChildRuntime,
  makeFixtureCodexChildSpawn,
} from "./codex-child-runtime.ts"
import { CODEX_CHILD_MODEL } from "./codex-child-contract.ts"
import {
  CodexLocalAvailabilityChannel,
  CodexLocalEventChannel,
  CodexLocalInterruptChannel,
  CodexLocalStartChannel,
  codexLocalFailureMessage,
  codexLocalModelNoteText,
  codexLocalRequestedModelLabel,
} from "./codex-local-contract.ts"
import {
  FIXTURE_CODEX_LOCAL_ACCOUNT,
  FIXTURE_CODEX_LOCAL_TEXT,
  fixtureCodexLocalTurnStdout,
  makeCodexLocalRuntime,
} from "./codex-local-runtime.ts"
import { makeCodexPreflight, type CodexProbeResult } from "./codex-preflight.ts"
import {
  UsageLedgerEventChannel,
  UsageLedgerSnapshotChannel,
} from "./usage-ledger-contract.ts"
import { makeUsageLedger } from "./usage-ledger.ts"
import { makeThreadStore } from "./thread-store.ts"
import {
  DesktopCodingCatalogArchiveChannel,
  DesktopCodingCatalogChooseChannel,
  DesktopCodingCatalogFocusChannel,
  DesktopCodingCatalogOpenChannel,
  DesktopCodingCatalogRecoverChannel,
  DesktopCodingCatalogSnapshotChannel,
  decodeDesktopCodingFocusRequest,
  decodeDesktopCodingSessionRequest,
  emptyDesktopCodingCatalogProjection,
  projectDesktopCodingCatalog,
} from "./coding-catalog-contract.ts"
import { makeCodexHistoryHost } from "./codex-history-host.ts"
import { makeDesktopHostLifecycle } from "./desktop-host-lifecycle.ts"
import {
  DesktopWorkspaceChooseChannel,
  DesktopWorkspaceFilesChannel,
  DesktopWorkspaceGitDiffChannel,
  DesktopWorkspaceGitStatusChannel,
  DesktopWorkspaceReadChannel,
  DesktopWorkspaceSaveChannel,
  DesktopWorkspaceSummaryChannel,
  decodeWorkspaceFileRequest,
  decodeWorkspaceGitDiffRequest,
  decodeWorkspaceSaveRequest,
} from "./workspace-contract.ts"
import { DesktopWindowFullscreenChannel } from "./window-contract.ts"
import { openWorkspaceService } from "./workspace-service.ts"
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
const smokeMode = process.env.OPENAGENTS_DESKTOP_SMOKE === "1"
const liveProofDriverMode = process.env.OPENAGENTS_DESKTOP_LIVE_PROOF === "1"
const desktopUserDataPath = process.env.OPENAGENTS_DESKTOP_USER_DATA ?? (
  smokeMode || liveProofDriverMode
    ? path.join(
        app.getPath("temp"),
        `openagents-desktop-${smokeMode ? "smoke" : "live-proof"}-${process.pid}`,
      )
    : path.join(app.getPath("appData"), "OpenAgentsDesktopDev")
)
app.setPath("userData", desktopUserDataPath)
const primaryDesktopInstance = app.requestSingleInstanceLock()
if (!primaryDesktopInstance) app.quit()
const desktopCommandHost = makeDesktopCommandHost()
let desktopCommandWindow: BrowserWindow | null = null
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
const providerAccounts = makeProviderAccountsService(
  here,
  smokeMode ? { spawnPylon: makeFixtureProviderAccountsSpawn() } : {},
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
  catalog: () => hostLifecycle.history()!.run({ kind: "history_catalog", sessionsRoot: codexSessionsRoot() }) as Promise<import("./codex-history-contract.ts").CodexHistoryCatalog>,
  page: (threadRef, offset, limit) => hostLifecycle.history()!.run({ kind: "history_page", sessionsRoot: codexSessionsRoot(), threadRef, offset, limit }) as Promise<import("./codex-history-contract.ts").CodexHistoryPage | null>,
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
      outcome: () => null,
    }
  }
  if (service === null) return null
  const context = (lane: "codex_app_server" | "claude_pylon" = "codex_app_server") => {
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
        context: context(),
        correlationRefs: operationContext === undefined ? [] : [
          operationContext.operationRef,
          operationContext.sessionRef,
          operationContext.correlationRef,
        ],
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
})

const isTrustedRuntimeGatewaySender = (event: IpcMainInvokeEvent): boolean => {
  const frame = event.senderFrame
  if (frame === null || frame !== event.sender.mainFrame) return false
  try {
    const url = new URL(frame.url)
    return url.protocol === "file:" && path.basename(url.pathname) === "index.html"
  } catch {
    return false
  }
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

// Interim development identity ONLY. Smoke uses a per-run userData root set
// before single-instance lock acquisition; its spawned receipt process is
// explicitly given the same root. Normal development retains this stable path.
// The frozen packaged identity remains the owner decision tracked by #8574.

const hardenSession = (): void => {
  // Deny-by-default: this shell requests no runtime permissions.
  session.defaultSession.setPermissionRequestHandler((_webContents, _permission, callback) => {
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
const codexSessionsRoot = () => path.resolve(
  process.env.OPENAGENTS_DESKTOP_CODEX_SESSIONS ?? (
    smokeMode
      ? path.join(here, "..", "tests", "fixtures", "codex-smoke", "sessions")
      : path.join(app.getPath("home"), ".codex", "sessions")
  ),
)
const codexHistoryHost = makeCodexHistoryHost(new URL("./codex-history-worker.js", import.meta.url))
const hostLifecycle = makeDesktopHostLifecycle({
  runtime: runtimeGateway,
  account: codexConnect,
  history: codexHistoryHost,
})
// Local file authority begins only after an explicit directory-picker choice.
// A process working directory or environment default is not user selection.
const workspaceSnapshot = () => {
  const workspace = hostLifecycle.workspace()
  if (workspace === null) return null
  try { return workspace.summary() } catch { return null }
}
const codingCatalogSnapshot = () => {
  const catalog = hostLifecycle.sync()?.codingCatalog()
  return catalog === null || catalog === undefined
    ? emptyDesktopCodingCatalogProjection()
    : projectDesktopCodingCatalog(catalog.snapshot())
}
const activateCodingCatalogRoot = () => {
  const root = hostLifecycle.sync()?.codingCatalog()?.selectedRoot() ?? null
  if (root !== null) hostLifecycle.replaceWorkspace(openWorkspaceService(root))
}
const chooseCodingWorkspace = async (registerCatalog = true) => {
  const result = await dialog.showOpenDialog({ properties: ["openDirectory", "createDirectory"] })
  if (result.canceled || result.filePaths[0] === undefined) return null
  const root = result.filePaths[0]
  hostLifecycle.replaceWorkspace(openWorkspaceService(root))
  if (registerCatalog) hostLifecycle.sync()?.codingCatalog()?.selectWorkspace(root)
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
ipcMain.handle(DesktopWorkspaceChooseChannel, async () => {
  await chooseCodingWorkspace()
  return workspaceSnapshot()
})
ipcMain.handle(DesktopCodingCatalogSnapshotChannel, () => codingCatalogSnapshot())
ipcMain.handle(DesktopCodingCatalogChooseChannel, async () => {
  await chooseCodingWorkspace()
  return codingCatalogSnapshot()
})
ipcMain.handle(DesktopCodingCatalogOpenChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const snapshot = catalog.openSession(request.sessionRef)
  activateCodingCatalogRoot()
  return projectDesktopCodingCatalog(snapshot)
})
ipcMain.handle(DesktopCodingCatalogArchiveChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingSessionRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  if (request === null || catalog === null || catalog === undefined) return codingCatalogSnapshot()
  const snapshot = catalog.archiveSession(request.sessionRef)
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
    : projectDesktopCodingCatalog(catalog.recoverSession(request.sessionRef, root))
})
ipcMain.handle(DesktopCodingCatalogFocusChannel, (_event, raw: unknown) => {
  const request = decodeDesktopCodingFocusRequest(raw)
  const catalog = hostLifecycle.sync()?.codingCatalog()
  return request === null || catalog === null || catalog === undefined
    ? codingCatalogSnapshot()
    : projectDesktopCodingCatalog(catalog.saveFocus(request.sessionRef, request.focus))
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
// List is intentionally metadata-only: a large local history must not
// serialize every transcript into the renderer merely to draw the sidebar.
ipcMain.handle(DesktopThreadsChannel, () => hostLifecycle.history()?.run({ kind: "list", sessionsRoot: codexSessionsRoot(), ...(smokeMode ? { limit: 1 } : {}) }) ?? Promise.resolve(null))
ipcMain.handle(DesktopNewThreadChannel, () => threads().newThread())
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
// Codex account PREFLIGHT (EP250 anti-speedbump core): a cheap REAL validity
// probe per registered account — a minimal bounded `codex exec` turn in a
// read-only sandbox (receipted ~3.5s on a live account; `codex login status`
// is presence-only and reports "Logged in" on revoked homes, so it can never
// be the probe). Runs on boot (async, non-blocking), on fleet Refresh, after
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
const codexPreflight = makeCodexPreflight({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  onResult: recordProbeEvidence,
  ...(smokeMode
    ? {
        // Isolated health in smoke so the scripted probe round does not
        // reorder the delegate-child fixture's attempt sequence.
        health: makeCodexAccountHealth(),
        hasAuthImpl: () => true,
        spawnImpl: makeFixtureCodexChildSpawn([
          { stdout: fixtureCodexRevokedStdout, stderr: fixtureCodexRevokedStderr, exitCode: 1 },
          { stdout: fixtureCodexSuccessStdout("thread-probe-codex-2"), exitCode: 0 },
          { stdout: fixtureCodexSuccessStdout("thread-probe-fixture"), exitCode: 0 },
        ]),
        discoverImpl: async () => [
          { ref: "codex", home: "/nonexistent/fixture-codex" },
          { ref: "codex-2", home: "/nonexistent/fixture-codex-2" },
          FIXTURE_CODEX_LOCAL_ACCOUNT,
        ],
      }
    : {}),
})
// Codex local chat lane (EP250 codex-first-class): the composer's Codex chip
// in local mode — a real `codex exec --json` turn per send, on the isolated
// registry homes, with session-resume continuity (no --ephemeral; children
// keep --ephemeral). Smoke drives a scripted stream through the REAL parser.
const codexLocal = makeCodexLocalRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  preflight: codexPreflight,
  onAccountEvidence: input => {
    if (input.evidence === "verified") {
      usageLedger.markVerified({ provider: "codex", accountRef: input.accountRef })
    } else {
      usageLedger.markReconnectRequired({ provider: "codex", accountRef: input.accountRef })
    }
  },
  ...(smokeMode
    ? {
        spawnImpl: makeFixtureCodexChildSpawn([
          { stdout: fixtureCodexLocalTurnStdout(), exitCode: 0 },
        ]),
        discoverImpl: async () => [FIXTURE_CODEX_LOCAL_ACCOUNT],
      }
    : {}),
})
const fableLocal = makeFableLocalRuntime({
  scratchRoot: () => path.join(app.getPath("userData"), "fable-local"),
  delegate: codexChildren,
  ...(smokeMode
    ? {
        queryImpl: async () => makeFixtureFableLocalQuery(),
        discoverImpl: async () => [FABLE_LOCAL_FIXTURE_ACCOUNT],
        mcpImpl: async () => makeFixtureFableMcpFactory(),
      }
    : {}),
})
ipcMain.handle(FableLocalAvailabilityChannel, () => fableLocal.availability())
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
  return request === null ? false : fableLocal.answerQuestion(request)
})
ipcMain.handle(FableLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null || request.message.trim() === "") {
    return { ok: false, error: "That message could not be sent." }
  }
  const store = threads()
  const user: DesktopMessage = {
    key: randomUUID(),
    role: "user",
    text: request.message.trim(),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  }
  const saved = store.append(request.threadRef, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  // History authority is main's own thread store — the renderer supplies only
  // the new message. The just-appended user note is the prompt, not history.
  const history = saved.notes.slice(0, -1).map(note => ({ role: note.role, text: note.text }))
  const sender = event.sender
  // Message metadata (#8712): record every fact this host observes for the
  // final assistant note so the renderer's inspector can project it later —
  // SDK-reported effective model, lane, account ref, turn ref, exact token
  // total, and wall-clock duration. Bounded public-safe strings only.
  const startedAt = Date.now()
  let effectiveModel: string | null = null
  const result = await fableLocal.runTurn({
    turnRef: request.turnRef,
    threadRef: request.threadRef,
    history,
    message: request.message.trim(),
    emit: turnEvent => {
      if (turnEvent.kind === "model_effective") effectiveModel = turnEvent.model
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
          requestedModel: FABLE_LOCAL_MODEL,
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
  if (!result.ok) return { ok: false, error: fableLocalFailureMessage(result.reason, result.detail) }
  const thread = store.append(request.threadRef, {
    key: randomUUID(),
    role: "assistant",
    text: result.text.slice(0, FABLE_LOCAL_FINAL_TEXT_LIMIT),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    meta: {
      lane: "fable-local",
      turnRef: request.turnRef,
      ...(effectiveModel === null ? {} : { model: effectiveModel }),
      ...(result.accountRef === undefined ? {} : { accountRef: result.accountRef }),
      totalTokens: result.totalTokens,
      durationMs: Date.now() - startedAt,
    },
  })
  return thread === null
    ? { ok: false, error: "That conversation no longer exists." }
    : { ok: true, thread }
})

// Codex local lane (EP250 codex-first-class): a REAL `codex exec --json`
// turn on this machine in local mode, on the isolated registry homes —
// never the default ~/.codex, never the cloud gateway. Availability is
// PROBE-VERIFIED evidence (see codexPreflight above). Events reuse the
// frozen fable-local envelope over the codex-local channels.
ipcMain.handle(CodexLocalAvailabilityChannel, () => codexLocal.availability())
ipcMain.handle(CodexLocalInterruptChannel, (_event, value: unknown) => {
  const request = decodeFableLocalInterruptRequest(value)
  return request === null ? false : codexLocal.interrupt(request.turnRef)
})
ipcMain.handle(CodexLocalStartChannel, async (event, value: unknown) => {
  const request = decodeFableLocalStartRequest(value)
  if (request === null || request.message.trim() === "") {
    return { ok: false, error: "That message could not be sent." }
  }
  const store = threads()
  const user: DesktopMessage = {
    key: randomUUID(),
    role: "user",
    text: request.message.trim(),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
  }
  const saved = store.append(request.threadRef, user)
  if (saved === null) return { ok: false, error: "That conversation no longer exists." }
  const history = saved.notes.slice(0, -1).map(note => ({ role: note.role, text: note.text }))
  const sender = event.sender
  // Message metadata (#8712 pattern): lane, spawn-config-truth model,
  // account ref, turn ref, exact usage total, duration — plus the codex
  // thread id (session-receipt continuity) in requestId.
  const startedAt = Date.now()
  let effectiveModel: string | null = null
  const result = await codexLocal.runTurn({
    turnRef: request.turnRef,
    threadRef: request.threadRef,
    history,
    message: request.message.trim(),
    emit: turnEvent => {
      if (turnEvent.kind === "model_effective") effectiveModel = turnEvent.model
      // Session usage ledger: exact usage from turn.completed, attributed to
      // the Codex account with gpt-5.6-sol recorded as spawn-config truth.
      if (turnEvent.kind === "turn_completed" && turnEvent.accountRef !== undefined) {
        usageLedger.record({
          provider: "codex",
          accountRef: turnEvent.accountRef,
          requestedModel: CODEX_CHILD_MODEL,
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
      if (sender.isDestroyed()) return
      const forwarded = turnEvent.kind === "turn_started" ? { ...turnEvent, thread: saved } : turnEvent
      sender.send(CodexLocalEventChannel, { turnRef: request.turnRef, event: forwarded })
    },
  })
  if (!result.ok) return { ok: false, error: codexLocalFailureMessage(result.reason, result.detail) }
  const thread = store.append(request.threadRef, {
    key: randomUUID(),
    role: "assistant",
    text: result.text.slice(0, FABLE_LOCAL_FINAL_TEXT_LIMIT),
    timestamp: new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" }),
    meta: {
      lane: "codex-local",
      turnRef: request.turnRef,
      model: effectiveModel ?? codexLocalRequestedModelLabel(),
      accountRef: result.accountRef,
      ...(result.threadId === null ? {} : { requestId: result.threadId }),
      totalTokens: result.totalTokens,
      durationMs: Date.now() - startedAt,
    },
  })
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
  const closeWindowScope = hostLifecycle.registerWindow(`window.${window.id}`, runtimeGateway.subscribe(event => {
    if (!window.isDestroyed()) window.webContents.send(DesktopRuntimeGatewayEventChannel, event)
  }))
  const unsubscribeLedger = usageLedger.subscribe(snapshot => {
    if (!window.isDestroyed()) window.webContents.send(UsageLedgerEventChannel, snapshot)
  })
  window.once("closed", () => {
    if (desktopCommandWindow === window) {
      desktopCommandWindow = null
      desktopCommandHost.detach()
    }
    unsubscribeLedger()
    closeWindowScope()
  })
  window.once("ready-to-show", () => {
    window.show()
  })
  void window.loadFile(path.join(here, "renderer/index.html"))
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
  const input = document.querySelector('[data-en-key="shell-input"] input')
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
  return { ok: sidebar?.textContent === "Codex history · all time" && detail !== null }
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

const smokeSubmitComposer = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const input = document.querySelector('[data-en-key="shell-input"] input')
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
    document.querySelector('[data-en-key="settings-account-codex-2-readiness"]') === null
  ) {
    await wait(50)
  }
  const revoked = document.querySelector('[data-en-key="settings-account-codex-2-readiness"]')
  const connect = document.querySelector('[data-en-key="settings-connect-codex"]')
  // EP250 UI-owned reconnect: the revoked fixture account must render its
  // per-account Reconnect button, and the screen must carry no CLI copy.
  const reconnect = document.querySelector('[data-en-key="settings-account-codex-2-reconnect"]')
  const screen = document.querySelector('[data-en-key="settings-screen"]')
  const cliCopy = screen !== null && /pylon auth|khala fleet|codex login/i.test(screen.textContent ?? "")
  return {
    ok: revoked !== null && revoked.textContent === "credentials_revoked" &&
      connect !== null && connect.textContent === "Connect Codex account" &&
      reconnect !== null && reconnect.textContent === "Reconnect" && !cliCopy,
    revokedChip: revoked === null ? null : revoked.textContent,
    connectLabel: connect === null ? null : connect.textContent,
    reconnectLabel: reconnect === null ? null : reconnect.textContent,
    cliCopy,
  }
})()`

const smokeConnectCodex = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const button = document.querySelector('[data-en-key="settings-connect-codex"]')
  if (button === null) return { ok: false, reason: "Connect button never mounted" }
  button.click()
  const deadline = Date.now() + 10000
  while (
    Date.now() < deadline &&
    document.querySelector('[data-en-key="settings-connect-code"]') === null
  ) {
    await wait(100)
  }
  const code = document.querySelector('[data-en-key="settings-connect-code"]')
  const link = document.querySelector('[data-en-key="settings-connect-link"]')
  return {
    ok: code !== null && code.textContent === "1234-ABCDE" &&
      link !== null && link.textContent === "https://auth.openai.com/codex/device",
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

const smokeWaitForDuplicateCommandNotice = `(async () => {
  const wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
  const deadline = Date.now() + 8000
  while (Date.now() < deadline && !(document.querySelector('[data-en-key="desktop-command-notice"]')?.textContent ?? '').includes('duplicate')) {
    await wait(50)
  }
  const notice = document.querySelector('[data-en-key="desktop-command-notice"]')?.textContent ?? ''
  return { ok: notice.includes('duplicate') && document.querySelector('[data-en-key="settings-screen"]') === null, notice }
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
  const composer = document.querySelector('[data-en-key="shell-input"] input')
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
  const fable = document.querySelector('[data-en-key="shell-harness-fable"]')
  const codex = document.querySelector('[data-en-key="shell-harness-codex"]')
  if (fable === null || codex === null) return { ok: false, reason: "harness chips never mounted" }
  if (fable.disabled !== false) return { ok: false, reason: "fable chip disabled despite fixture account" }
  // EP250 codex-first-class: the fixture preflight VERIFIES an account, so
  // the codex chip must light on that evidence (poll: the availability
  // promise resolves asynchronously after mount).
  {
    const deadline = Date.now() + 10000
    while (Date.now() < deadline && codex.disabled !== false) {
      await wait(50)
    }
  }
  if (codex.disabled !== false) return { ok: false, reason: "codex chip stayed disabled despite a fixture PROBE-VERIFIED account" }
  const codexAria = codex.getAttribute("aria-label") || ""
  if (!codexAria.includes("Codex")) {
    return { ok: false, reason: "codex chip lost its accessible label" }
  }
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
  // Disabled-control reason popover (owner verbatim: "i can't tell why the
  // Codex option is disabled in the composer… put a popover on hover over
  // the disabled button explaining why"): the disabled codex chip is
  // wrapped in a tooltip whose content is the SAME reason the accessible
  // label carries; pointerenter reveals it, pointerleave hides it.
  const codexWrap = document.querySelector('[data-en-key="shell-harness-codex-reason"]')
  if (codexWrap === null) return { ok: false, reason: "disabled codex chip has no reason popover wrapper" }
  const bubble = codexWrap.querySelector('[data-en-role="tooltip"]')
  if (bubble === null) return { ok: false, reason: "reason popover bubble missing" }
  if (bubble.hidden !== true) return { ok: false, reason: "reason popover visible at rest (standing caption ban)" }
  if ((bubble.textContent ?? "") !== codexAria) {
    return { ok: false, reason: "popover reason does not match the accessible reason: " + bubble.textContent }
  }
  codexWrap.dispatchEvent(new PointerEvent("pointerenter", { bubbles: false }))
  await wait(50)
  if (bubble.hidden !== false) return { ok: false, reason: "hover did not reveal the reason popover" }
  codexWrap.dispatchEvent(new PointerEvent("pointerleave", { bubbles: false }))
  await wait(50)
  if (bubble.hidden !== true) return { ok: false, reason: "leave did not dismiss the reason popover" }
  fable.click()
  const input = document.querySelector('[data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Stream a fable-local proof"
  input.dispatchEvent(new Event("input", { bubbles: true }))
  const assistantBody = () => {
    const rows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
    const last = rows[rows.length - 1]
    return last === undefined ? null : last.querySelector('[data-en-role="body"]')
  }
  const assistantText = () => {
    const body = assistantBody()
    if (body === null) return ""
    // Exclude the compact details affordance (a real text Button now) and
    // any wrapper row that only hosts it.
    return Array.from(body.childNodes)
      .filter((node) => !(node instanceof HTMLElement &&
        (node.tagName === "BUTTON" || node.querySelector("button") !== null)))
      .map((node) => node.textContent ?? "")
      .join("")
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
  const noSystemToolLabel = toolRows().every((row) => row.querySelector('[data-en-role="sender"]') === null)
  const toolTimestamps = toolRows().every((row) => row.querySelector('[data-en-role="timestamp"]') !== null)
  const body = assistantBody()
  const strong = body === null ? null : body.querySelector("strong")
  const markdownRendered = strong !== null && strong.textContent === "streaming" &&
    !assistantText().includes("**")
  const assistantRows = document.querySelectorAll('[data-en-key="shell-transcript"] [data-en-message][data-en-role="assistant"]')
  const lastAssistant = assistantRows[assistantRows.length - 1]
  const noAssistantLabel = lastAssistant !== undefined &&
    lastAssistant.querySelector('[data-en-role="sender"]') === null
  return {
    ok: sawPartial && assistantText() === finalText && input.disabled === false && trace &&
      markdownRendered && noAssistantLabel && delegateSingleCard && delegateUse &&
      delegateResult && noRawJson && noSystemToolLabel && toolTimestamps,
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
    text: assistantText(),
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
  const codex = document.querySelector('[data-en-key="shell-harness-codex"]')
  if (codex === null) return { ok: false, reason: "codex chip never mounted" }
  if (codex.disabled !== false) return { ok: false, reason: "codex chip disabled despite fixture verified account" }
  codex.click()
  const input = document.querySelector('[data-en-key="shell-input"] input')
  if (input === null) return { ok: false, reason: "composer input never mounted" }
  input.focus()
  input.value = "Stream a codex-local proof"
  input.dispatchEvent(new Event("input", { bubbles: true }))
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
  const hasModel = text.includes("claude-fable-5")
  const hasLane = text.includes("fable-local")
  const hasAccount = text.includes("claude-pylon-fixture")
  const hasTokens = text.includes("Tokens (total)") && text.includes("49")
  const close = document.querySelector('[data-en-key="chat-message-inspector-close"]')
  if (close === null) return { ok: false, reason: "inspector close affordance missing" }
  close.click()
  const closeDeadline = Date.now() + 5000
  while (Date.now() < closeDeadline && document.querySelector('[data-en-key="chat-message-inspector"]') !== null) {
    await wait(50)
  }
  return {
    ok: hasModel && hasLane && hasAccount && hasTokens &&
      document.querySelector('[data-en-key="chat-message-inspector"]') === null,
    hasModel,
    hasLane,
    hasAccount,
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
  const expected = ["codex", "codex-2", "claude-pylon-3"]
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
  const composer = document.querySelector('[data-en-key="shell-input"] input')
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

const runSmoke = (window: BrowserWindow): void => {
  const finish = (code: 0 | 1): void => {
    hostLifecycle.dispose()
    const snapshot = hostLifecycle.snapshot()
    const active = Number(snapshot.runtime) + Number(snapshot.workspace) + Number(snapshot.sync) +
      Number(snapshot.account) + Number(snapshot.history) + snapshot.windowCount
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
          await step("codex-trace-reload-restoration", traceAcceptanceReload)
          await step("coding-catalog-reload-restoration", smokeCodingCatalog)
          clearTimeout(timeout)
          console.log("[openagents-desktop smoke] OK")
          finish(0)
          return
        }
        await step("shell-mounted", smokeWaitForShell)
        await step("runtime-gateway-bootstrap", smokeRuntimeGatewayBootstrap)
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
        await launchSmokeSecondInstance()
        await step("command-duplicate-visible-rejection", smokeWaitForDuplicateCommandNotice)
        await step("recent-codex-history-selected-detail", smokeCodexHistoryDetails)
        await step("codex-trace-acceptance", traceAcceptanceJourney)
        await captureShot(window, "03-codex-history-detail")
        // Settings / Codex reconnect (#8640 unblock). Headless smoke cannot
        // complete a real browser device-auth, so main runs a FIXTURE spawn:
        // the awaiting_browser receipt below shows scripted fixture data.
        await step("settings-open-accounts", smokeOpenSettings)
        await captureShot(window, "04-settings-accounts")
        await step("settings-connect-awaiting-browser-FIXTURE", smokeConnectCodex)
        await captureShot(window, "05-settings-awaiting-browser-fixture")
        await step("settings-back-to-chat", smokeCloseSettings)
        // With the historical page still loaded, New chat must yield a fresh
        // empty transcript (the on-camera regression), then the Fleet dock
        // button must render the fixture accounts panel.
        await step("new-chat-from-history-empty-transcript", smokeNewChatFromHistory)
        await captureShot(window, "06-new-chat-empty")
        // Fable local turn is FIXTURE-driven in smoke (no real Claude SDK
        // session; a scripted delta sequence flows through the real mapping).
        await step("fable-local-streamed-turn-FIXTURE", smokeFableLocalStreaming)
        await captureShot(window, "07-fable-local-streamed")
        // EP250 question cards: the persisted fixture question renders as a
        // read-only pending interactive card (no answer bridge in smoke).
        await step("question-card-interactive-typed-answer-FIXTURE", smokeQuestionCard)
        await captureShot(window, "10-question-card")
        // EP250 (#8712): click a message -> right-side metadata inspector
        // (model/lane/account/tokens), close through the same typed intent,
        // then re-open once for the pixel receipt.
        await step("message-metadata-inspector", smokeMessageInspector)
        await step("message-metadata-inspector-reopen", smokeReopenMessageInspector)
        await captureShot(window, "08-message-inspector")
        await step("message-metadata-inspector-close", smokeCloseMessageInspector)
        // Codex local turn is FIXTURE-driven in smoke (scripted codex exec
        // JSONL through the REAL parser; EP250 codex-first-class proof).
        await step("codex-local-streamed-turn-FIXTURE", smokeCodexLocalStreaming)
        await captureShot(window, "11-codex-local-streamed")
        await step("fleet-workspace-fixture-accounts", smokeOpenFleetWorkspace)
        await captureShot(window, "09-fleet-workspace")
        // Cmd+N from the fleet workspace: fresh transcript + focused composer.
        await step("cmd-n-new-chat-focuses-composer", smokeCmdNNewChat)
        await step("coding-catalog-host-persistence", smokeCodingCatalog)
        await captureShot(window, "10-coding-catalog")
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
  // macOS does not use BrowserWindow's icon for the active Dock tile in a
  // development Electron process. Set both native surfaces to the same mobile
  // PNG so the running desktop application has one product identity.
  if (process.platform === "darwin") app.dock?.setIcon(desktopIconPath)
  if (app.isPackaged) app.setAsDefaultProtocolClient("openagents")
  desktopCommandBindings = openDesktopCommandBindingStore(
    path.join(app.getPath("userData"), "commands", "bindings.json"),
  )
  installDesktopCommandMenu(desktopCommandBindings.snapshot())
  hardenSession()
  try {
    const syncHost = openDesktopSyncHost({
      databasePath: path.join(app.getPath("userData"), "sync", "khala-sync.sqlite"),
      randomId: randomUUID,
    })
    hostLifecycle.replaceSync(syncHost)
    if (smokeMode) {
      // Deterministic CUT-13 built-host fixture: real local SQLite/catalog and
      // private binding path, without provider or remote authority claims.
      syncHost.codingCatalog()?.selectWorkspace(path.join(here, "..", "tests", "fixtures", "codex-smoke"))
    }
    const restoredRoot = syncHost.codingCatalog()?.selectedRoot() ?? null
    if (restoredRoot !== null) hostLifecycle.replaceWorkspace(openWorkspaceService(restoredRoot))
  } catch {
    console.error("[openagents-desktop] local Sync persistence unavailable")
  }
  try {
    desktopSessionVault = openDesktopSessionVault({
      filePath: path.join(app.getPath("userData"), "session", "native-session.enc"),
      safeStorage,
    })
    desktopSessionState = desktopSessionVault.recover().state
    if (desktopSessionState === "credential_present_unverified") {
      const recovery = await recoverVerifiedDesktopSession({
        vault: desktopSessionVault,
      })
      desktopSessionState = recovery.state === "verified"
        ? connectVerifiedDesktopSync() ? "session_ready" : "unavailable"
        : recovery.state
      if(recovery.state==="denied")hostLifecycle.sync()?.unlinkAccount()
    }
  } catch {
    desktopSessionVault = null
    desktopSessionState = "unavailable"
    console.error("[openagents-desktop] OS-encrypted session custody unavailable")
  }
  runtimeGateway.start()
  // Boot probe round (EP250 preflight): async and non-blocking — results
  // stream into the shared health ordering, the ledger's typed reconnect
  // flags (fleet readiness), and the composer chip's availability call.
  void codexPreflight.probeAll("boot").catch(() => {})
  const window = createWindow()
  // Episode 250 live-proof driver (#8712): REAL adapters (no smoke fixtures),
  // mutually exclusive with smoke. See ./live-proof.ts — additive only.
  const liveProof = resolveLiveProofConfig(process.env, app.getPath("userData"))
  if (liveProof.enabled && liveProof.conflict) {
    console.error("[openagents-desktop live-proof] OPENAGENTS_DESKTOP_LIVE_PROOF and OPENAGENTS_DESKTOP_SMOKE are mutually exclusive; refusing to run either")
    app.exit(1)
    return
  }
  if (smokeMode) runSmoke(window)
  else if (liveProof.enabled) {
    runLiveProof(window, {
      outDir: liveProof.outDir,
      // Step 0 (EP250): the REAL account preflight over the real registry —
      // per-account verified/broken journal entries with reasons.
      preflight: () => codexPreflight.probeAll("live_proof"),
      exit: (code) => {
        hostLifecycle.dispose()
        providerAccounts.dispose()
        desktopCorrelationJournal.dispose()
        app.exit(code)
      },
    })
  }
})

app.on("window-all-closed", () => {
  if (process.platform !== "darwin" || smokeMode) {
    app.quit()
  }
})

app.on("before-quit", () => {
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
