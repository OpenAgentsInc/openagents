import {
  DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  applyComposerTransaction,
  composerAttachmentId,
  emptyComposerState,
  offerComposerLargeTextPaste,
  planComposerAttachmentUpload,
  projectComposerAttachmentUploadReceipt,
  readyComposerAttachmentTransaction,
  retryComposerAttachmentTransaction,
  setComposerAttachmentStatusTransaction,
  stageComposerAttachmentFiles,
  stageComposerDroppedFiles,
  stageComposerPastedFiles,
  type ComposerAttachment,
  type ComposerAttachmentSource,
  type ComposerAttachmentUploadReceipt,
  type ComposerFileLike,
  type ComposerTransaction,
} from "@openagentsinc/composer-state"
import {
  commandComposerClassName,
  type CommandComposerAttachmentProps,
  type CommandComposerStatus,
} from "@openagentsinc/ui/ai-elements/command-composer"
import { iconElement } from "@openagentsinc/ui/icon-dom"
import type { IconName } from "@openagentsinc/ui/icon"
import {
  shimmerBaseTag,
  shimmerClass,
} from "@openagentsinc/ui/ai-elements/shimmer"
import {
  commandComposerHudLayoutFromCssRect,
  createCommandComposerHud,
  type CommandComposerAttachmentProjection,
  type CommandComposerHudHandle,
} from "@openagentsinc/three-effect/core"
import { Electroview } from "electrobun/view"
import { Schema as S } from "effect"
import * as Three from "three"

import {
  KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT,
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  KhalaCodeDesktopRpcBridgeFailure,
  decodeKhalaCodeDesktopRpcParameters,
  decodeKhalaCodeDesktopRpcResult,
  type KhalaCodeDesktopRpcMethodName,
  type KhalaCodeDesktopChatTurnAttachment,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopFleetLifecycleEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopMessageRole,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopRuntimeMode,
  type KhalaCodeDesktopThreadTokenSummary,
} from "../shared/rpc"
import {
  evaluateKhalaCodeQaMetricBudgets,
  khalaCodeQaMetricBudgets,
  khalaCodeQaMetricDefinitions,
  type KhalaCodeQaMetricName,
  type KhalaCodeQaMetricSample,
  type KhalaCodeQaMetricsSnapshot,
} from "../shared/qa-metrics"
import { renderMessageBody } from "./transcript-render"
import { mountFleetPanel } from "./fleet-status"
import { mountKhalaCodeForumPanel } from "./forum-panel"
import { mountKhalaCodePlansPanel } from "./plans-panel"
import { mountCodexSettingsPanel } from "./codex-settings-panel"
import { mountClaudeSettingsSection } from "./claude-settings-panel"
import {
  mountCodexThreadSidebar,
  type CodexThreadSelectionSource,
} from "./codex-thread-sidebar"
import {
  gymPaneStateFromBridgeProof,
  gymOptimizationRunFromBridgeProof,
  gymPaneStateFromLocation,
  initialKhalaCodeViewFromLocation,
  khalaCodeGymDemoBridgeProof,
  type KhalaGymProofLoadRequest,
} from "./gym-proof-loader"
import type { KhalaGymBridgeProofLike } from "./gym-graph-projection"
import { mountGymPane, type GymPaneState } from "./gym-pane"
import {
  mountKhalaCodeSidebar,
  projectKhalaCodeSidebarFleetCounts,
} from "./sidebar"
import { mountUnifiedInboxPanel } from "./inbox"
import type { KhalaCodeDesktopCodexThreadSummary } from "../shared/codex-threads"
import { sessionCatalogEntryToThreadSummary } from "../shared/session-catalog"
import {
  type RecentThreadCycleDirection,
  recentThreadIndexForDigitKey,
  recentThreadsForHotkeys,
} from "./thread-hotkeys"
import {
  initialKhalaCodeMainShellModel,
  updateKhalaCodeMainShellModel,
  type KhalaCodeMainShellMessage,
  type KhalaCodeMainShellSlashCommand,
} from "./main-shell-model"
import "./styles.css"

type DesktopRpc = ReturnType<typeof Electroview.defineRPC<KhalaCodeDesktopRPCSchema>>
type DesktopRpcRequests = KhalaCodeDesktopRPCSchema["requests"]
const rpcFailureDetail = (payload: unknown): string => {
  if (payload !== null && typeof payload === "object") {
    const record = payload as Record<string, unknown>
    if (typeof record.detail === "string" && record.detail.length > 0) return record.detail
    if (typeof record.error === "string" && record.error.length > 0) return record.error
    if (typeof record.message === "string" && record.message.length > 0) return record.message
  }
  if (typeof payload === "string" && payload.length > 0) return payload
  return "unknown error"
}

const postPreviewRpc = async <Result>(
  method: KhalaCodeDesktopRpcMethodName,
  ...args: readonly unknown[]
): Promise<Result> => {
  const decodedArgs = decodeKhalaCodeDesktopRpcParameters(method, args)
  const response = await fetch(`/rpc/${encodeURIComponent(method)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ args: decodedArgs }),
  })
  if (!response.ok) {
    const text = await response.text()
    let payload: unknown = text
    try {
      payload = text.length === 0 ? "" : JSON.parse(text) as unknown
    } catch {
      payload = text
    }
    const failure = S.decodeUnknownSync(KhalaCodeDesktopRpcBridgeFailure)
    try {
      const decodedFailure = failure(payload)
      throw new Error(`${method} failed with ${response.status}: ${decodedFailure.tag}: ${decodedFailure.error}`)
    } catch (error) {
      if (error instanceof Error && error.message.startsWith(`${method} failed with ${response.status}:`)) {
        throw error
      }
      throw new Error(`${method} failed with ${response.status}: ${rpcFailureDetail(payload)}`)
    }
  }
  const payload = await response.json() as unknown
  return decodeKhalaCodeDesktopRpcResult(method, payload) as Result
}

const previewRpc = (): DesktopRpc => ({
  request: {
    appInfo: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["appInfo"]>>>(
        "appInfo",
      ),
    appleFmReadiness: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["appleFmReadiness"]>>
      >("appleFmReadiness"),
    codexAccountsStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAccountsStatus"]>>
      >("codexAccountsStatus"),
    codexAppServerRestart: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerRestart"]>>
      >("codexAppServerRestart"),
    codexAppServerStart: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStart"]>>
      >("codexAppServerStart"),
    codexAppServerStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStatus"]>>
      >("codexAppServerStatus"),
    codexAppServerStop: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexAppServerStop"]>>
      >("codexAppServerStop"),
    codexFleetDelegateRun: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetDelegateRun"]>>
      >("codexFleetDelegateRun", request),
    codexFleetStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetStatus"]>>
      >("codexFleetStatus"),
    codexFleetPromoteThread: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFleetPromoteThread"]>>
      >("codexFleetPromoteThread", request),
    fleetRunControl: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunControl"]>>
      >("fleetRunControl", request),
    fleetRunList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunList"]>>
      >("fleetRunList", request),
    fleetRunStart: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunStart"]>>
      >("fleetRunStart", request),
    fleetRunStatus: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetRunStatus"]>>
      >("fleetRunStatus", request),
    fleetWorkerControl: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["fleetWorkerControl"]>>
      >("fleetWorkerControl", request),
    forumRequest: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["forumRequest"]>>
      >("forumRequest", request),
    khalaCodePlanCatalog: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanCatalog"]>>
      >("khalaCodePlanCatalog"),
    khalaCodePlanStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanStatus"]>>
      >("khalaCodePlanStatus"),
    khalaCodePlanPurchase: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["khalaCodePlanPurchase"]>>
      >("khalaCodePlanPurchase", request),
    claudeApprovalPending: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeApprovalPending"]>>
      >("claudeApprovalPending"),
    claudeApprovalRespond: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeApprovalRespond"]>>
      >("claudeApprovalRespond", request),
    claudeSettingsRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["claudeSettingsRead"]>>
      >("claudeSettingsRead"),
    codexHarnessStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexHarnessStatus"]>>
      >("codexHarnessStatus"),
    codexApprovalRespond: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexApprovalRespond"]>>
      >("codexApprovalRespond", request),
    codexBackgroundTerminalsClean: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsClean"]>>
      >("codexBackgroundTerminalsClean", request),
    codexBackgroundTerminalsList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsList"]>>
      >("codexBackgroundTerminalsList", request),
    codexBackgroundTerminalsTerminate: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexBackgroundTerminalsTerminate"]>>
      >("codexBackgroundTerminalsTerminate", request),
    codexConfigValueWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexConfigValueWrite"]>>
      >("codexConfigValueWrite", request),
    codexEcosystemRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexEcosystemRead"]>>
      >("codexEcosystemRead", request),
    codexExternalAgentConfigDetect: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigDetect"]>>
      >("codexExternalAgentConfigDetect", request),
    codexExternalAgentConfigImport: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigImport"]>>
      >("codexExternalAgentConfigImport", request),
    codexExternalAgentConfigImportHistoriesRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexExternalAgentConfigImportHistoriesRead"]>>
      >("codexExternalAgentConfigImportHistoriesRead"),
    codexFsGetMetadata: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsGetMetadata"]>>
      >("codexFsGetMetadata", request),
    codexFsReadFile: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsReadFile"]>>
      >("codexFsReadFile", request),
    codexFsWriteFile: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexFsWriteFile"]>>
      >("codexFsWriteFile", request),
    codexMarketplaceAdd: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceAdd"]>>
      >("codexMarketplaceAdd", request),
    codexMarketplaceRemove: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceRemove"]>>
      >("codexMarketplaceRemove", request),
    codexMarketplaceUpgrade: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMarketplaceUpgrade"]>>
      >("codexMarketplaceUpgrade", request),
    codexMentionCandidates: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMentionCandidates"]>>
      >("codexMentionCandidates", request),
    codexMcpOauthLogin: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpOauthLogin"]>>
      >("codexMcpOauthLogin", request),
    codexMcpResourceRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpResourceRead"]>>
      >("codexMcpResourceRead", request),
    codexMcpServerReload: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpServerReload"]>>
      >("codexMcpServerReload"),
    codexMcpToolCall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexMcpToolCall"]>>
      >("codexMcpToolCall", request),
    codexPluginInstall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexPluginInstall"]>>
      >("codexPluginInstall", request),
    codexPluginUninstall: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexPluginUninstall"]>>
      >("codexPluginUninstall", request),
    codexSettingsRead: (request?: Parameters<DesktopRpcRequests["codexSettingsRead"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSettingsRead"]>>
      >("codexSettingsRead", request),
    codexSkillsConfigWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSkillsConfigWrite"]>>
      >("codexSkillsConfigWrite", request),
    codexSkillsExtraRootsSet: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexSkillsExtraRootsSet"]>>
      >("codexSkillsExtraRootsSet", request),
    codexThreadArchive: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadArchive"]>>
      >("codexThreadArchive", request),
    codexThreadCompact: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadCompact"]>>
      >("codexThreadCompact", request),
    codexThreadDelete: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadDelete"]>>
      >("codexThreadDelete", request),
    codexThreadFork: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadFork"]>>
      >("codexThreadFork", request),
    codexThreadList: (request?: Parameters<DesktopRpcRequests["codexThreadList"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadList"]>>
      >("codexThreadList", request),
    codexThreadRead: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadRead"]>>
      >("codexThreadRead", request),
    codexThreadRename: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadRename"]>>
      >("codexThreadRename", request),
    codexThreadResume: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadResume"]>>
      >("codexThreadResume", request),
    codexThreadStart: (request?: Parameters<DesktopRpcRequests["codexThreadStart"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadStart"]>>
      >("codexThreadStart", request),
    codexThreadUnarchive: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexThreadUnarchive"]>>
      >("codexThreadUnarchive", request),
    sessionCatalog: (request?: Parameters<DesktopRpcRequests["sessionCatalog"]>[0]) =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>
      >("sessionCatalog", request),
    codexTurnInterrupt: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnInterrupt"]>>
      >("codexTurnInterrupt", request),
    codexTurnStart: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnStart"]>>
      >("codexTurnStart", request),
    codexTurnSteer: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codexTurnSteer"]>>
      >("codexTurnSteer", request),
    codingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codingStatus"]>>
      >("codingStatus"),
    connectCodexAccount: accountRef =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["connectCodexAccount"]>>
      >("connectCodexAccount", accountRef),
    harnessSettingRead: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["harnessSettingRead"]>>
      >("harnessSettingRead"),
    harnessSettingWrite: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["harnessSettingWrite"]>>
      >("harnessSettingWrite", request),
    openExternalUrl: url =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["openExternalUrl"]>>
      >("openExternalUrl", url),
    consumeCodexRateLimitResetCredit: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["consumeCodexRateLimitResetCredit"]>>
      >("consumeCodexRateLimitResetCredit", request),
    onDeviceDeciderStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["onDeviceDeciderStatus"]>>
      >("onDeviceDeciderStatus"),
    pylonStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["pylonStatus"]>>
      >("pylonStatus"),
    qaMetrics: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["qaMetrics"]>>
      >("qaMetrics"),
    removeCodexAccount: accountRef =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["removeCodexAccount"]>>
      >("removeCodexAccount", accountRef),
    setCodexAccountPaused: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["setCodexAccountPaused"]>>
      >("setCodexAccountPaused", request),
    slashCommandDispatch: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>
      >("slashCommandDispatch", request),
    slashCommandList: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["slashCommandList"]>>
      >("slashCommandList", request),
    submitChatMessage: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["submitChatMessage"]>>
      >("submitChatMessage", request),
    tokenAccountingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["tokenAccountingStatus"]>>
      >("tokenAccountingStatus"),
    threadTokenSummary: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["threadTokenSummary"]>>
      >("threadTokenSummary", request),
    toolCatalog: () =>
      postPreviewRpc<Awaited<ReturnType<DesktopRpcRequests["toolCatalog"]>>>(
        "toolCatalog",
      ),
  },
  send: {
    chatTurnEvent: () => undefined,
    fleetLifecycleEvent: () => undefined,
  },
})

const createAsyncLineQueue = (): {
  readonly iterable: () => AsyncIterable<string>
  readonly push: (line: string) => void
} => {
  const values: string[] = []
  const waiters: Array<(value: IteratorResult<string>) => void> = []
  const push = (line: string): void => {
    const waiter = waiters.shift()
    if (waiter === undefined) {
      values.push(line)
      return
    }
    waiter({ done: false, value: line })
  }
  return {
    iterable: async function* () {
      while (true) {
        if (values.length > 0) {
          yield values.shift()!
          continue
        }
        yield await new Promise<string>(resolve => {
          waiters.push(result => {
            if (!result.done) resolve(result.value)
          })
        })
      }
    },
    push,
  }
}

const fleetLifecycleLines = createAsyncLineQueue()
const applyFleetLifecycleEvent = (event: KhalaCodeDesktopFleetLifecycleEvent): void => {
  fleetLifecycleLines.push(event.line)
}

const startPreviewFleetLifecycleEvents = (): void => {
  if (!isKhalaPreviewWindow) return
  const eventSource = new EventSource("/rpc/events")
  eventSource.addEventListener("fleetLifecycleEvent", event => {
    try {
      const payload = JSON.parse((event as MessageEvent<string>).data) as {
        readonly detail?: { readonly line?: unknown }
      }
      if (typeof payload.detail?.line === "string") {
        fleetLifecycleLines.push(payload.detail.line)
      }
    } catch {
      // Ignore malformed preview diagnostics; lifecycle decoding is lossy-safe.
    }
  })
}

const nativeRpc = Electroview.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {
      chatTurnEvent(event) {
        applyChatTurnEvent(event)
      },
      fleetLifecycleEvent(event) {
        applyFleetLifecycleEvent(event)
      },
    },
  },
})

const currentPort = Number(globalThis.location?.port ?? "0")
const isKhalaPreviewWindow =
  globalThis.location?.protocol === "http:" &&
  Number.isInteger(currentPort) &&
  currentPort >= KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT &&
  currentPort < KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT + 10

// Only the explicit Khala preview server handles /rpc HTTP fallbacks. Native
// Electrobun internal ports also run on localhost, but they only accept socket
// RPC and will log noisy fallthroughs for /rpc/* requests.
const rpc = isKhalaPreviewWindow ? previewRpc() : nativeRpc
startPreviewFleetLifecycleEvents()
if (!isKhalaPreviewWindow) {
  new Electroview({ rpc: nativeRpc })
}

const composerClasses = {
  attachment: commandComposerClassName("attachment"),
  attachmentAction: commandComposerClassName("attachmentAction"),
  dropcursor: commandComposerClassName("dropcursor"),
  status: commandComposerClassName("status"),
} as const

type ComposerIconName =
  | "attach"
  | "code"
  | "file"
  | "image"
  | "preview"
  | "remove"
  | "retry"
  | "send"
  | "stop"
  | "text"

const composerIconCatalog = {
  attach: "Paperclip",
  code: "Code",
  file: "File",
  image: "FileImage",
  preview: "Eye",
  remove: "Trash",
  retry: "ArrowRotateCcw",
  send: "ArrowUp",
  stop: "Stop",
  text: "Text",
} satisfies Record<ComposerIconName, IconName>

const composerIconElement = (name: ComposerIconName): HTMLSpanElement => {
  const icon = iconElement(composerIconCatalog[name], {
    className: "oa-ai-command-composer-icon",
    dataIcon: name,
  })
  icon.dataset.oaCommandComposerIcon = name
  return icon
}

const attachmentIconName = (
  kind: CommandComposerAttachmentProps["kind"],
): ComposerIconName =>
  kind === "image"
    ? "image"
    : kind === "text"
      ? "text"
      : kind === "snippet"
        ? "code"
        : "file"

const setButtonIcon = (
  button: HTMLButtonElement,
  name: ComposerIconName,
): void => {
  const current = button.querySelector(".oa-ai-command-composer-icon")
  const next = composerIconElement(name)
  if (current === null) {
    button.prepend(next)
    return
  }
  current.replaceWith(next)
}

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector)
  if (element === null) throw new Error(`Missing ${selector}`)
  return element
}

const messageList = requireElement<HTMLElement>("#message-list")
const threadTokenMeter = requireElement<HTMLElement>("#thread-token-meter")
const threadTokenCounter = requireElement<HTMLButtonElement>("#thread-token-counter")
const threadTokenCounterValue = requireElement<HTMLElement>("#thread-token-counter-value")
const threadTokenPopover = requireElement<HTMLElement>("#thread-token-popover")
const composerForm = requireElement<HTMLFormElement>("#composer-form")
const composerFrame = requireElement<HTMLElement>("#composer-frame")
const composerHudMount = requireElement<HTMLElement>("#composer-hud")
const composerInput = requireElement<HTMLTextAreaElement>("#composer-input")
const composerRail = requireElement<HTMLElement>("#composer-rail")
const slashCommandPalette = requireElement<HTMLElement>("#slash-command-palette")
const composerPreview = requireElement<HTMLElement>("#composer-preview")
const composerStatus = requireElement<HTMLElement>("#composer-status")
const composerA11y = requireElement<HTMLElement>("#composer-a11y")
const sendButton = requireElement<HTMLButtonElement>("#send-button")
const attachButton = requireElement<HTMLButtonElement>("#attach-button")
const fileInput = requireElement<HTMLInputElement>("#file-input")

const activeTurnIds = new Set<string>()
const objectUrls = new Set<string>()
const localTextAttachments = new Map<string, string>()
const localAttachmentFiles = new Map<string, File>()
const sessionIdStorageKey = "khala-code-desktop.session-id.v1"
const activeThreadIdStorageKey = "khala-code-desktop.active-thread-id.v1"
const storedSessionId = localStorage.getItem(sessionIdStorageKey)
const sessionId =
  storedSessionId?.startsWith("khala-code-desktop-") === true
    ? storedSessionId
    : `khala-code-desktop-${Date.now().toString(36)}`
localStorage.setItem(sessionIdStorageKey, sessionId)
localStorage.removeItem(activeThreadIdStorageKey)

const harnessOptions: readonly {
  readonly label: string
  readonly mode: KhalaCodeDesktopRuntimeMode
}[] = [
  { label: "Codex", mode: "codex_harness" },
  { label: "Claude", mode: "claude_runtime" },
  { label: "Khala", mode: "khala_native_runtime" },
]

const harnessLabel = (mode: KhalaCodeDesktopRuntimeMode): string =>
  harnessOptions.find(option => option.mode === mode)?.label ?? "Codex"

type ThreadSwitchPerformanceSample = {
  cacheHit: boolean
  fullMessageCount?: number
  fullRenderMs?: number
  hydratedRenderMs?: number
  optimisticMessageCount: number
  optimisticRenderMs?: number
  rpcMs?: number
  selectionId: number
  source: CodexThreadSelectionSource
  startedAt: number
  threadId: string
}

const THREAD_MESSAGE_CACHE_LIMIT = 16
const THREAD_PREFETCH_LIMIT = 4
const THREAD_LIST_CACHE_TTL_MS = 2000
const THREAD_SWITCH_INITIAL_MESSAGE_LIMIT = 80
const THREAD_SWITCH_PERFORMANCE_SAMPLE_LIMIT = 60
const QA_METRIC_SAMPLE_LIMIT = 240
const threadMessageCache = new Map<string, readonly KhalaCodeDesktopMessage[]>()
const threadPrefetches = new Map<string, Promise<void>>()
const threadListCache = new Map<string, {
  readonly cachedAt: number
  readonly result: Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>
}>()
const threadListRequests = new Map<string, Promise<Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>>>()
const threadSwitchPerformanceSamples: ThreadSwitchPerformanceSample[] = []
const pendingThreadSwitches = new Map<number, ThreadSwitchPerformanceSample>()
const qaMetricSamples: KhalaCodeQaMetricSample[] = []

const pushQaMetricSample = (
  metric: KhalaCodeQaMetricName,
  value: number,
  context?: KhalaCodeQaMetricSample["context"],
): void => {
  if (!Number.isFinite(value)) return
  qaMetricSamples.push({
    ...(context === undefined ? {} : { context }),
    metric,
    observedAt: new Date().toISOString(),
    unit: metric === "cache.hit" ? "count" : "ms",
    value,
  })
  while (qaMetricSamples.length > QA_METRIC_SAMPLE_LIMIT) qaMetricSamples.shift()
}

const markQaTimer = (
  metric: KhalaCodeQaMetricName,
  startedAt: number,
  context?: KhalaCodeQaMetricSample["context"],
): void => {
  requestAnimationFrame(() => pushQaMetricSample(metric, performance.now() - startedAt, context))
}

const emptyThreadTokenSummary = (
  threadId: string | null,
): KhalaCodeDesktopThreadTokenSummary => ({
  auditRows: 0,
  codexStateDbPath: "",
  codexStateTokens: 0,
  leaderboardLabel: "OpenAgents Stats",
  leaderboardSyncedTokens: 0,
  localLedgerPath: "",
  localMessageAuditLedgerPath: "",
  missingUsageTurns: 0,
  ok: true,
  pendingSyncTokens: 0,
  remoteConfigured: false,
  remoteDisabled: false,
  threadId,
  totalTokens: 0,
  updatedAt: null,
  usageEventRows: 0,
})

const cloneMessages = (
  value: readonly KhalaCodeDesktopMessage[],
): KhalaCodeDesktopMessage[] =>
  value.map(message => ({ ...message }))

const cacheThreadMessages = (
  threadId: string,
  value: readonly KhalaCodeDesktopMessage[],
): void => {
  if (value.length === 0) return
  threadMessageCache.delete(threadId)
  threadMessageCache.set(threadId, cloneMessages(value))
  while (threadMessageCache.size > THREAD_MESSAGE_CACHE_LIMIT) {
    const oldest = threadMessageCache.keys().next().value
    if (typeof oldest !== "string") break
    threadMessageCache.delete(oldest)
  }
}

const cachedThreadMessages = (
  threadId: string,
): KhalaCodeDesktopMessage[] | null => {
  const cached = threadMessageCache.get(threadId)
  if (cached === undefined) return null
  cacheThreadMessages(threadId, cached)
  return cloneMessages(cached)
}

const cacheVisibleThreadMessages = (): void => {
  const threadId = shellModel().activeCodexThreadId
  if (threadId === null) return
  cacheThreadMessages(threadId, shellModel().messages)
}

const recentMessagesForInitialThreadRender = (
  value: readonly KhalaCodeDesktopMessage[],
): KhalaCodeDesktopMessage[] => {
  if (value.length <= THREAD_SWITCH_INITIAL_MESSAGE_LIMIT) return cloneMessages(value)
  return cloneMessages(value.slice(-THREAD_SWITCH_INITIAL_MESSAGE_LIMIT))
}

const pushThreadSwitchPerformanceSample = (
  sample: ThreadSwitchPerformanceSample,
): void => {
  threadSwitchPerformanceSamples.push(sample)
  while (threadSwitchPerformanceSamples.length > THREAD_SWITCH_PERFORMANCE_SAMPLE_LIMIT) {
    threadSwitchPerformanceSamples.shift()
  }
}

const markThreadSwitchPaint = (
  selectionId: number,
  field: "fullRenderMs" | "hydratedRenderMs" | "optimisticRenderMs",
): void => {
  requestAnimationFrame(() => {
    const sample = pendingThreadSwitches.get(selectionId)
    if (sample === undefined) return
    const durationMs = performance.now() - sample.startedAt
    sample[field] = durationMs
    const metric =
      field === "fullRenderMs"
        ? "thread_switch.full_render_ms"
        : field === "hydratedRenderMs"
          ? "thread_switch.hydrated_render_ms"
          : "thread_switch.optimistic_render_ms"
    pushQaMetricSample(metric, durationMs, { threadId: sample.threadId })
    if (field !== "hydratedRenderMs") return
    pendingThreadSwitches.delete(selectionId)
  })
}

const beginThreadSwitchPerformanceSample = (
  input: {
    readonly cacheHit: boolean
    readonly optimisticMessageCount: number
    readonly selectionId: number
    readonly source: CodexThreadSelectionSource
    readonly threadId: string
  },
): void => {
  const sample: ThreadSwitchPerformanceSample = {
    cacheHit: input.cacheHit,
    optimisticMessageCount: input.optimisticMessageCount,
    selectionId: input.selectionId,
    source: input.source,
    startedAt: performance.now(),
    threadId: input.threadId,
  }
  pendingThreadSwitches.set(input.selectionId, sample)
  pushThreadSwitchPerformanceSample(sample)
  if (input.cacheHit) pushQaMetricSample("cache.hit", 1, { threadId: input.threadId })
  markThreadSwitchPaint(input.selectionId, "optimisticRenderMs")
}

const completeThreadSwitchPerformanceSample = (
  input: {
    readonly fullMessageCount: number
    readonly selectionId?: number
  },
): void => {
  if (input.selectionId === undefined) return
  const sample = pendingThreadSwitches.get(input.selectionId)
  if (sample === undefined) return
  sample.fullMessageCount = input.fullMessageCount
  sample.rpcMs = performance.now() - sample.startedAt
  pushQaMetricSample("thread_switch.rpc_ms", sample.rpcMs, { threadId: sample.threadId })
  markThreadSwitchPaint(input.selectionId, "fullRenderMs")
}

const qaMetricsSnapshot = (): KhalaCodeQaMetricsSnapshot => {
  const samples = [...qaMetricSamples]
  return {
    budgets: khalaCodeQaMetricBudgets,
    definitions: khalaCodeQaMetricDefinitions,
    evaluations: evaluateKhalaCodeQaMetricBudgets(samples),
    ok: true,
    observedAt: new Date().toISOString(),
    samples,
    schema: "openagents.khala_code.qa_metrics.v1",
  }
}

const scheduleFullThreadHydration = (
  input: {
    readonly fullMessages: readonly KhalaCodeDesktopMessage[]
    readonly selectionId?: number
    readonly threadId: string
    readonly visibleMessages: readonly KhalaCodeDesktopMessage[]
  },
): void => {
  if (input.fullMessages.length === input.visibleMessages.length) {
    if (input.selectionId !== undefined) {
      const selectionId = input.selectionId
      requestAnimationFrame(() => pendingThreadSwitches.delete(selectionId))
    }
    return
  }
  const hydrate = (): void => {
    if (shellModel().activeCodexThreadId !== input.threadId || shellModel().messages !== input.visibleMessages) return
    setShellMessages(cloneMessages(input.fullMessages))
    render()
    if (input.selectionId !== undefined) {
      markThreadSwitchPaint(input.selectionId, "hydratedRenderMs")
    }
  }
  if (typeof requestIdleCallback === "function") {
    requestIdleCallback(hydrate, { timeout: 700 })
  } else {
    window.setTimeout(hydrate, 80)
  }
}

const mainShellStore = {
  model: initialKhalaCodeMainShellModel({
    threadTokenSummary: emptyThreadTokenSummary(null),
  }),
}

const dispatchMainShell = (message: KhalaCodeMainShellMessage): void => {
  mainShellStore.model = updateKhalaCodeMainShellModel(
    mainShellStore.model,
    message,
  )
}

const shellModel = (): typeof mainShellStore.model => mainShellStore.model

const setShellMessages = (
  messages: readonly KhalaCodeDesktopMessage[],
): void => dispatchMainShell({ _tag: "MessagesChanged", messages })

const setShellComposerState = (
  state: typeof mainShellStore.model.composerState,
): void => dispatchMainShell({ _tag: "ComposerStateChanged", state })

const compactTokenFormatter = new Intl.NumberFormat("en-US", {
  maximumFractionDigits: 1,
  notation: "compact",
})
const exactTokenFormatter = new Intl.NumberFormat("en-US")
const tokenTimestampFormatter = new Intl.DateTimeFormat("en-US", {
  hour: "numeric",
  minute: "2-digit",
  month: "short",
  day: "numeric",
})

const prefersReducedMotion =
  typeof matchMedia === "function"
    ? matchMedia("(prefers-reduced-motion: reduce)")
    : null

type ComposerHudRuntime = Readonly<{
  renderer: Three.WebGLRenderer
  scene: Three.Scene
  camera: Three.OrthographicCamera
  handle: CommandComposerHudHandle
  dispose: () => void
}>

let composerHudRuntime: ComposerHudRuntime | null = null

const messageClass = (role: KhalaCodeDesktopMessageRole): string =>
  `message-bubble message-bubble--${role}`

const renderMessage = (message: KhalaCodeDesktopMessage): HTMLElement => {
  const article = document.createElement("article")
  article.className = messageClass(message.role)
  article.dataset.messageId = message.id
  if (message.codexItem !== undefined) {
    article.dataset.codexItemId = message.codexItem.itemId
    article.dataset.codexItemType = message.codexItem.itemType
    article.dataset.codexItemStatus = message.codexItem.status
  }

  const body = document.createElement("div")
  body.className = "message-body"
  body.append(...renderMessageBody(message.body, message.role, message.codexItem))

  article.append(body)
  return article
}

const setHarnessMode = async (mode: KhalaCodeDesktopRuntimeMode): Promise<void> => {
  shellModel().selectedHarnessMode = mode
  renderComposer()
  try {
    const setting = await rpc.request.harnessSettingWrite({ mode })
    shellModel().selectedHarnessMode = setting.mode
    shellModel().harnessEnvOverride = setting.envOverride
  } catch {
    shellModel().selectedHarnessMode = mode
  }
  renderComposer()
}

const refreshHarnessSetting = async (): Promise<void> => {
  try {
    const setting = await rpc.request.harnessSettingRead()
    shellModel().selectedHarnessMode = setting.mode
    shellModel().harnessEnvOverride = setting.envOverride
    shellModel().lastResponseRuntimeMode = setting.mode
  } catch {
    shellModel().selectedHarnessMode = "codex_harness"
    shellModel().harnessEnvOverride = null
    shellModel().lastResponseRuntimeMode = "codex_harness"
  }
  renderComposer()
}

void refreshHarnessSetting()

const renderHarnessPill = (): HTMLElement => {
  const pill = document.createElement("div")
  pill.className = "khala-harness-pill"
  pill.dataset.envOverride = shellModel().harnessEnvOverride === null ? "false" : "true"
  pill.setAttribute("role", "group")
  for (const option of harnessOptions) {
    const button = document.createElement("button")
    button.type = "button"
    button.className = "khala-harness-pill-button"
    button.dataset.active = shellModel().selectedHarnessMode === option.mode ? "true" : "false"
    button.textContent = option.label
    button.disabled = shellModel().harnessEnvOverride !== null
    button.addEventListener("click", () => void setHarnessMode(option.mode))
    pill.append(button)
  }
  return pill
}

const renderRuntimeBadge = (): HTMLElement => {
  const badge = document.createElement("span")
  badge.className = "khala-runtime-badge"
  badge.dataset.runtimeMode = shellModel().lastResponseRuntimeMode
  badge.textContent = harnessLabel(shellModel().lastResponseRuntimeMode)
  return badge
}

const renderThinkingIndicator = (): HTMLElement | null => {
  if (shellModel().thinkingTurnId === null) return null

  const article = document.createElement("article")
  article.className = `${messageClass("assistant")} message-bubble--thinking`
  article.dataset.messageId = `thinking-${shellModel().thinkingTurnId}`
  article.dataset.khalaThinking = "true"

  const body = document.createElement("div")
  body.className = "message-body"

  const shimmer = document.createElement("span")
  shimmer.className = shimmerClass
  shimmer.dataset.uiBase = shimmerBaseTag
  shimmer.dataset.oaAiShimmer = ""
  shimmer.setAttribute("role", "status")
  shimmer.setAttribute("aria-live", "polite")
  shimmer.setAttribute("aria-label", "Thinking")
  shimmer.textContent = "Thinking"

  body.append(shimmer)
  article.append(body)
  return article
}

const isNearTranscriptEnd = (): boolean =>
  messageList.scrollHeight - messageList.scrollTop - messageList.clientHeight <= 48

const maxTranscriptScrollTop = (): number =>
  Math.max(0, messageList.scrollHeight - messageList.clientHeight)

const canScrollTranscript = (): boolean =>
  maxTranscriptScrollTop() > 1

const scrollToEnd = (behavior: ScrollBehavior = "auto"): void => {
  shellModel().transcriptPinnedToEnd = true
  messageList.scrollTo({
    top: messageList.scrollHeight,
    behavior,
  })
}

const setTranscriptScrollTop = (top: number): void => {
  messageList.scrollTop = Math.max(0, Math.min(top, maxTranscriptScrollTop()))
  shellModel().transcriptPinnedToEnd = isNearTranscriptEnd()
}

const wheelDeltaPixels = (event: WheelEvent): number => {
  if (event.deltaMode === WheelEvent.DOM_DELTA_LINE) return event.deltaY * 16
  if (event.deltaMode === WheelEvent.DOM_DELTA_PAGE) return event.deltaY * messageList.clientHeight
  return event.deltaY
}

const isComposerScrollTarget = (target: EventTarget | null): boolean =>
  target instanceof Element &&
  target.closest("#composer-form, #composer-input, input, textarea, select, button, [contenteditable='true']") !== null

const proxyTranscriptWheel = (event: WheelEvent): void => {
  if (event.defaultPrevented || isComposerScrollTarget(event.target) || !canScrollTranscript()) return
  const before = messageList.scrollTop
  setTranscriptScrollTop(before + wheelDeltaPixels(event))
  if (messageList.scrollTop !== before) event.preventDefault()
}

const proxyTranscriptKeyScroll = (event: KeyboardEvent): void => {
  if (event.defaultPrevented || isComposerScrollTarget(event.target) || !canScrollTranscript()) return
  const page = Math.max(80, Math.floor(messageList.clientHeight * 0.82))
  const delta =
    event.key === "PageDown" || event.key === " "
      ? page
      : event.key === "PageUp"
        ? -page
        : event.key === "ArrowDown"
          ? 40
          : event.key === "ArrowUp"
            ? -40
            : null
  if (delta === null) return
  const before = messageList.scrollTop
  setTranscriptScrollTop(before + delta)
  if (messageList.scrollTop !== before) event.preventDefault()
}

const recentThreadHotkeyIndexForEvent = (event: KeyboardEvent): number | null => {
  if (
    event.defaultPrevented ||
    event.altKey ||
    event.ctrlKey ||
    event.metaKey ||
    event.shiftKey ||
    isComposerScrollTarget(event.target)
  ) {
    return null
  }
  return recentThreadIndexForDigitKey(event.key)
}

const recentThreadCycleDirectionForEvent = (
  event: KeyboardEvent,
): RecentThreadCycleDirection | null => {
  if (
    event.defaultPrevented ||
    event.altKey ||
    event.ctrlKey ||
    !event.metaKey ||
    event.shiftKey
  ) {
    return null
  }
  if (event.key === "ArrowUp") return "newer"
  if (event.key === "ArrowDown") return "older"
  return null
}

const statusForComposer = (): CommandComposerStatus => {
  if (shellModel().pendingTurn) return "streaming"
  if (shellModel().lastTurnFailed) return "error"
  return "ready"
}

const statusLabelFor = (status: CommandComposerStatus): string => {
  if (status === "submitted") return "Submitted"
  if (status === "streaming") return "Streaming"
  if (status === "error") return "Needs attention"
  return "Ready"
}

const formatBytes = (bytes: number): string => {
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B"
  const units = ["B", "KB", "MB", "GB"] as const
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024
    unit += 1
  }
  const digits = value >= 10 || unit === 0 ? 0 : 1
  return `${value.toFixed(digits)} ${units[unit]}`
}

const formatCompactTokens = (tokens: number): string =>
  tokens >= 10_000
    ? compactTokenFormatter.format(tokens)
    : exactTokenFormatter.format(Math.max(0, Math.trunc(tokens)))

const formatExactTokens = (tokens: number): string =>
  exactTokenFormatter.format(Math.max(0, Math.trunc(tokens)))

const formatThreadTokenUpdatedAt = (value: string | null): string => {
  if (value === null) return "Not recorded"
  const date = new Date(value)
  if (!Number.isFinite(date.getTime())) return "Not recorded"
  return tokenTimestampFormatter.format(date)
}

const appendThreadTokenRow = (
  root: HTMLElement,
  label: string,
  value: string,
): void => {
  const row = document.createElement("div")
  row.className = "khala-thread-token-popover-row"

  const term = document.createElement("dt")
  term.textContent = label

  const definition = document.createElement("dd")
  definition.textContent = value

  row.append(term, definition)
  root.append(row)
}

const renderThreadTokenCounter = (): void => {
  const summary = shellModel().threadTokenSummary
  threadTokenCounterValue.textContent = formatCompactTokens(summary.totalTokens)
  threadTokenCounter.setAttribute(
    "aria-label",
    `${formatExactTokens(summary.totalTokens)} thread tokens, ${formatExactTokens(summary.leaderboardSyncedTokens)} synced to leaderboard`,
  )
  threadTokenCounter.setAttribute("aria-expanded", String(shellModel().threadTokenPopoverOpen))
  threadTokenPopover.hidden = !shellModel().threadTokenPopoverOpen
  if (!shellModel().threadTokenPopoverOpen) return

  const title = document.createElement("div")
  title.className = "khala-thread-token-popover-title"
  title.textContent = summary.threadId === null ? "No active thread" : "Thread tokens"

  const rows = document.createElement("dl")
  rows.className = "khala-thread-token-popover-grid"
  appendThreadTokenRow(rows, "Total local", formatExactTokens(summary.totalTokens))
  appendThreadTokenRow(
    rows,
    "Leaderboard synced",
    formatExactTokens(summary.leaderboardSyncedTokens),
  )
  appendThreadTokenRow(rows, "Pending sync", formatExactTokens(summary.pendingSyncTokens))
  appendThreadTokenRow(rows, "Audit turns", exactTokenFormatter.format(summary.auditRows))
  appendThreadTokenRow(rows, "Usage events", exactTokenFormatter.format(summary.usageEventRows))
  if (summary.codexStateTokens > 0) {
    appendThreadTokenRow(
      rows,
      summary.totalTokens === 0 && summary.auditRows === 0 && summary.usageEventRows === 0
        ? "Codex state only"
        : "Codex state",
      formatExactTokens(summary.codexStateTokens),
    )
  }
  if (summary.missingUsageTurns > 0) {
    appendThreadTokenRow(
      rows,
      "Missing usage",
      exactTokenFormatter.format(summary.missingUsageTurns),
    )
  }

  const meta = document.createElement("div")
  meta.className = "khala-thread-token-popover-meta"
  const codexOnlyState =
    summary.codexStateTokens > 0 &&
    summary.totalTokens === 0 &&
    summary.auditRows === 0 &&
    summary.usageEventRows === 0
  meta.textContent = codexOnlyState
    ? "Not Khala Code usage"
    : summary.remoteDisabled
    ? "Sync disabled"
    : summary.remoteConfigured
      ? `Updated ${formatThreadTokenUpdatedAt(summary.updatedAt)}`
      : "Remote sync not configured"

  threadTokenPopover.replaceChildren(title, rows, meta)
}

const refreshThreadTokenSummary = async (): Promise<void> => {
  const threadId = shellModel().activeCodexThreadId
  if (threadId === null) {
    shellModel().threadTokenSummary = emptyThreadTokenSummary(null)
    renderThreadTokenCounter()
    return
  }
  if (shellModel().threadTokenRefreshInFlight) {
    shellModel().threadTokenRefreshQueued = true
    return
  }

  shellModel().threadTokenRefreshInFlight = true
  try {
    const summary = await rpc.request.threadTokenSummary({ threadId })
    if (shellModel().activeCodexThreadId === threadId) {
      shellModel().threadTokenSummary = summary
      renderThreadTokenCounter()
    }
  } catch {
    if (shellModel().activeCodexThreadId === threadId) {
      shellModel().threadTokenSummary = {
        ...emptyThreadTokenSummary(threadId),
        totalTokens: shellModel().threadTokenSummary.threadId === threadId
          ? shellModel().threadTokenSummary.totalTokens
          : 0,
      }
      renderThreadTokenCounter()
    }
  } finally {
    shellModel().threadTokenRefreshInFlight = false
    if (shellModel().threadTokenRefreshQueued) {
      shellModel().threadTokenRefreshQueued = false
      void refreshThreadTokenSummary()
    }
  }
}

const attachmentPropsFor = (
  attachment: ComposerAttachment,
): CommandComposerAttachmentProps => ({
  id: attachment.id,
  kind: attachment.kind,
  name: attachment.name,
  mime: attachment.mime,
  sizeBytes: attachment.sizeBytes,
  sizeLabel: formatBytes(attachment.sizeBytes),
  status: attachment.status,
  ...(attachment.previewUrl === undefined
    ? {}
    : { previewUrl: attachment.previewUrl }),
  ...(attachment.dimensions === undefined
    ? {}
    : { dimensions: attachment.dimensions }),
  ...(attachment.contentRef === undefined
    ? {}
    : { contentRef: attachment.contentRef }),
  ...(attachment.source === undefined ? {} : { source: attachment.source }),
  ...(attachment.errorText === undefined
    ? {}
    : { errorText: attachment.errorText }),
})

const statusTextForAttachment = (
  status: CommandComposerAttachmentProps["status"],
): string => {
  if (status === "uploading") return "Uploading"
  if (status === "ready") return "Ready"
  if (status === "error") return "Error"
  return "Staged"
}

const canSubmitComposer = (): boolean =>
  composerInput.value.trim() !== "" ||
  shellModel().composerState.doc.attachments.length > 0 ||
  (shellModel().lastTurnFailed && shellModel().lastSubmittedDraft.trim() !== "")

const renderMessages = (): void => {
  cacheVisibleThreadMessages()
  const stickToEnd = shellModel().transcriptPinnedToEnd && isNearTranscriptEnd()
  const previousScrollTop = messageList.scrollTop
  const thinking = renderThinkingIndicator()
  messageList.replaceChildren(
    ...shellModel().messages.map(renderMessage),
    ...(thinking === null ? [] : [thinking]),
  )
  requestAnimationFrame(() => {
    if (stickToEnd) {
      scrollToEnd()
    } else {
      setTranscriptScrollTop(previousScrollTop)
    }
  })
}

const buttonLabel = (status: CommandComposerStatus): string => {
  if (status === "streaming" || status === "submitted") return "Stop"
  if (status === "error" && composerInput.value.trim() === "") return "Retry"
  return "Send"
}

const updateComposerHudProjection = (): void => {
  if (composerHudRuntime === null) return
  const attachments: ReadonlyArray<CommandComposerAttachmentProjection> =
    shellModel().composerState.doc.attachments.map((attachment) => ({
      id: attachment.id,
      kind:
        attachment.kind === "snippet"
          ? "code"
          : attachment.kind === "file"
            ? "file"
            : attachment.kind,
      status: attachment.status,
      selected:
        shellModel().composerState.selection.selectedAttachmentId === attachment.id,
    }))
  composerHudRuntime.handle.setProjection({
    focused: document.activeElement === composerInput,
    dragActive: shellModel().dragActive,
    reducedMotion: prefersReducedMotion?.matches === true,
    attachments,
    dropcursor: {
      visible: shellModel().dragActive,
      x: 0,
      intensity: shellModel().dragActive ? 0.9 : 0,
    },
  })
}

const renderAttachmentAction = (
  label: string,
  action: "preview" | "retry" | "remove",
  attachment: CommandComposerAttachmentProps,
): HTMLButtonElement => {
  const button = document.createElement("button")
  button.type = "button"
  button.className = composerClasses.attachmentAction
  button.title = label
  button.setAttribute("aria-label", `${label} attachment: ${attachment.name}`)
  button.dataset.oaCommandComposerAttachmentAction = action
  button.dataset.attachmentId = attachment.id
  button.replaceChildren(composerIconElement(action))
  return button
}

const openAttachmentPreview = (attachment: CommandComposerAttachmentProps): void => {
  if (attachment.previewUrl !== undefined) {
    window.open(attachment.previewUrl, "_blank", "noopener,noreferrer")
    return
  }
  if (attachment.contentRef !== undefined) {
    const text = localTextAttachments.get(attachment.contentRef)
    if (text !== undefined) {
      const url = URL.createObjectURL(new Blob([text], { type: "text/plain" }))
      objectUrls.add(url)
      window.open(url, "_blank", "noopener,noreferrer")
    }
  }
}

const applyComposerStateTransaction = (
  transaction: ComposerTransaction,
): boolean => {
  const result = applyComposerTransaction(shellModel().composerState, transaction)
  if (!result.ok) {
    shellModel().lastTurnFailed = true
    renderComposer()
    return false
  }
  setShellComposerState(result.state)
  shellModel().lastTurnFailed = false
  renderComposer()
  return true
}

const hexFromBytes = (bytes: ArrayBuffer): string =>
  [...new Uint8Array(bytes)]
    .map(byte => byte.toString(16).padStart(2, "0"))
    .join("")

const sha256DigestForBytes = async (bytes: ArrayBuffer): Promise<string> => {
  const digest = await crypto.subtle.digest("SHA-256", bytes)
  return `sha256:${hexFromBytes(digest)}`
}

const arrayBufferForText = (text: string): ArrayBuffer => {
  const bytes = new TextEncoder().encode(text)
  return bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer
}

const base64FromArrayBuffer = (bytes: ArrayBuffer): string => {
  const view = new Uint8Array(bytes)
  let binary = ""
  const chunkSize = 0x8000
  for (let offset = 0; offset < view.length; offset += chunkSize) {
    binary += String.fromCharCode(...view.subarray(offset, offset + chunkSize))
  }
  return btoa(binary)
}

const pushAttachmentReceipt = (
  receipt: ComposerAttachmentUploadReceipt,
): void => {
  dispatchMainShell({ _tag: "ComposerAttachmentReceiptPushed", receipt })
}

const attachmentById = (attachmentId: string): ComposerAttachment | undefined =>
  shellModel().composerState.doc.attachments.find(attachment => attachment.id === attachmentId)

const failLocalAttachmentUpload = (
  attachmentId: string,
  errorText: string,
): void => {
  const transaction = setComposerAttachmentStatusTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    { status: "error", errorText },
  )
  if (transaction !== null) applyComposerStateTransaction(transaction)
  const attachment = attachmentById(attachmentId)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment,
      surface: "desktop-local",
      event: "error",
      errorCode: "local_register_failed",
    }))
  }
}

const runDesktopLocalAttachmentUpload = async (
  attachmentId: string,
  bytes: () => Promise<ArrayBuffer>,
): Promise<void> => {
  const planned = planComposerAttachmentUpload(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    DEFAULT_DESKTOP_LOCAL_ATTACHMENT_UPLOAD_POLICY,
  )
  if (!planned.ok) {
    pushAttachmentReceipt(planned.receipt)
    applyComposerStateTransaction(planned.transaction)
    return
  }

  pushAttachmentReceipt(planned.plan.receipt)
  if (!applyComposerStateTransaction(planned.plan.transaction)) return

  try {
    const digest = await sha256DigestForBytes(await bytes())
    const current = attachmentById(attachmentId)
    if (current === undefined) return
    const readyTransaction = readyComposerAttachmentTransaction(
      shellModel().composerState,
      composerAttachmentId(attachmentId),
      {
        surface: "desktop-local",
        digest,
        ...(current.kind === "image" ? { thumbnailDigest: digest } : {}),
        ...(current.dimensions === undefined
          ? {}
          : { dimensions: current.dimensions }),
      },
    )
    if (readyTransaction === null) return
    if (!applyComposerStateTransaction(readyTransaction)) return
    const readyAttachment = attachmentById(attachmentId)
    if (readyAttachment !== undefined) {
      pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
        attachment: readyAttachment,
        surface: "desktop-local",
        event: "ready",
      }))
    }
  } catch (error) {
    failLocalAttachmentUpload(
      attachmentId,
      error instanceof Error ? error.message : "Attachment upload failed.",
    )
  }
}

const removeAttachment = (attachmentId: string): void => {
  const attachment = shellModel().composerState.doc.attachments.find(
    item => item.id === attachmentId,
  )
  if (attachment?.previewUrl !== undefined && objectUrls.has(attachment.previewUrl)) {
    URL.revokeObjectURL(attachment.previewUrl)
    objectUrls.delete(attachment.previewUrl)
  }
  if (attachment?.contentRef !== undefined) {
    localTextAttachments.delete(attachment.contentRef)
  }
  localAttachmentFiles.delete(attachmentId)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment,
      surface: "desktop-local",
      event: "removed",
    }))
  }
  applyComposerStateTransaction({
    steps: [
      {
        _tag: "RemoveAttachment",
        attachmentId: composerAttachmentId(attachmentId),
      },
    ],
    meta: { source: "manual", time: Date.now() },
  })
}

const retryAttachment = (attachmentId: string): void => {
  const attachment = attachmentById(attachmentId)
  const transaction = retryComposerAttachmentTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
  )
  if (transaction !== null) {
    applyComposerStateTransaction(transaction)
    const retried = attachmentById(attachmentId)
    if (retried !== undefined) {
      pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
        attachment: retried,
        surface: "desktop-local",
        event: "retry",
      }))
      const file = localAttachmentFiles.get(attachmentId)
      if (file !== undefined) {
        void runDesktopLocalAttachmentUpload(attachmentId, () => file.arrayBuffer())
      } else if (retried.contentRef !== undefined) {
        const text = localTextAttachments.get(retried.contentRef)
        if (text !== undefined) {
          void runDesktopLocalAttachmentUpload(
            attachmentId,
            () => Promise.resolve(arrayBufferForText(text)),
          )
        }
      }
    }
    return
  }
  const fallback = setComposerAttachmentStatusTransaction(
    shellModel().composerState,
    composerAttachmentId(attachmentId),
    { status: "staged", errorText: null },
  )
  if (fallback !== null) applyComposerStateTransaction(fallback)
  if (attachment !== undefined) {
    pushAttachmentReceipt(projectComposerAttachmentUploadReceipt({
      attachment: { ...attachment, status: "staged" },
      surface: "desktop-local",
      event: "retry",
    }))
  }
}

const renderAttachment = (
  attachment: CommandComposerAttachmentProps,
): HTMLElement => {
  const chip = document.createElement("div")
  chip.className = composerClasses.attachment
  chip.dataset.oaCommandComposerAttachment = attachment.id
  chip.dataset.kind = attachment.kind
  chip.dataset.status = attachment.status
  chip.dataset.selected = "false"
  chip.role = "listitem"
  chip.tabIndex = 0
  chip.ariaSelected = "false"

  const before = document.createElement("span")
  before.className = "oa-ai-command-composer-gapcursor"
  before.tabIndex = 0
  before.dataset.oaCommandComposerGapcursor = "before"
  before.dataset.attachmentId = attachment.id
  before.setAttribute("aria-label", `Before ${attachment.name}`)

  const icon = document.createElement("span")
  icon.className = "oa-ai-command-composer-attachment-icon"
  icon.setAttribute("aria-hidden", "true")
  icon.replaceChildren(composerIconElement(attachmentIconName(attachment.kind)))

  const preview =
    attachment.kind === "image" && attachment.previewUrl !== undefined
      ? document.createElement("img")
      : null
  if (preview !== null) {
    preview.src = attachment.previewUrl ?? ""
    preview.alt = `${attachment.name} preview`
    preview.loading = "lazy"
    preview.decoding = "async"
    preview.className = "oa-ai-command-composer-attachment-thumb"
    if (attachment.dimensions !== undefined) {
      preview.width = attachment.dimensions.width
      preview.height = attachment.dimensions.height
    }
  }

  const main = document.createElement("span")
  main.className = "oa-ai-command-composer-attachment-main"
  const name = document.createElement("span")
  name.className = "oa-ai-command-composer-attachment-name"
  name.textContent = attachment.name
  const meta = document.createElement("span")
  meta.className = "oa-ai-command-composer-attachment-meta"
  meta.textContent = `${attachment.mime} - ${attachment.sizeLabel ?? formatBytes(attachment.sizeBytes)}`
  const status = document.createElement("span")
  status.className = "oa-ai-command-composer-attachment-status"
  status.textContent = statusTextForAttachment(attachment.status)
  main.append(name, meta, status)
  if (attachment.errorText !== undefined) {
    const error = document.createElement("span")
    error.className = "oa-ai-command-composer-attachment-error"
    error.textContent = attachment.errorText
    main.append(error)
  }

  const actions = document.createElement("span")
  actions.className = "oa-ai-command-composer-attachment-actions"
  if (attachment.previewUrl !== undefined || attachment.contentRef !== undefined || attachment.status === "ready") {
    const open = renderAttachmentAction("Open", "preview", attachment)
    open.addEventListener("click", () => openAttachmentPreview(attachment))
    actions.append(open)
  }
  if (attachment.status === "error") {
    const retry = renderAttachmentAction("Retry", "retry", attachment)
    retry.addEventListener("click", () => retryAttachment(attachment.id))
    actions.append(retry)
  }
  const remove = renderAttachmentAction("Remove", "remove", attachment)
  remove.addEventListener("click", () => removeAttachment(attachment.id))
  actions.append(remove)

  const after = document.createElement("span")
  after.className = "oa-ai-command-composer-gapcursor"
  after.tabIndex = 0
  after.dataset.oaCommandComposerGapcursor = "after"
  after.dataset.attachmentId = attachment.id
  after.setAttribute("aria-label", `After ${attachment.name}`)

  chip.append(
    before,
    icon,
    ...(preview === null ? [] : [preview]),
    main,
    actions,
    after,
  )
  return chip
}

const renderAttachmentRail = (): void => {
  const attachments = shellModel().composerState.doc.attachments.map(attachmentPropsFor)
  composerRail.hidden = attachments.length === 0 && !shellModel().dragActive
  composerRail.dataset.oaCommandComposerDragActive = shellModel().dragActive ? "true" : "false"
  composerRail.replaceChildren(
    ...attachments.map(renderAttachment),
    ...(shellModel().dragActive
      ? [
          Object.assign(document.createElement("div"), {
            className: composerClasses.dropcursor,
          }),
        ]
      : []),
  )
}

const renderComposerPreview = (): void => {
  composerPreview.hidden = true
  composerPreview.replaceChildren()
}

const slashCommandPlatform = (): string => {
  const userAgent = navigator.userAgent.toLowerCase()
  const platform = navigator.platform.toLowerCase()
  if (userAgent.includes("android")) return "android"
  if (platform.includes("win")) return "win32"
  if (platform.includes("mac")) return "darwin"
  if (platform.includes("linux")) return "linux"
  return "unknown"
}

const slashCommandLoadKey = (): string =>
  `${slashCommandPlatform()}:${shellModel().pendingTurn ? "active" : "idle"}`

const ensureSlashCommandsLoaded = (): void => {
  const key = slashCommandLoadKey()
  if (shellModel().loadedSlashCommandKey === key && shellModel().slashCommands.length > 0) return
  if (shellModel().slashCommandLoadInFlight !== null) return
  shellModel().slashCommandLoadInFlight = rpc.request.slashCommandList({
    activeTurn: shellModel().pendingTurn,
    platform: slashCommandPlatform(),
    sideConversation: false,
  }).then(response => {
    shellModel().slashCommands = [...response.commands]
    shellModel().loadedSlashCommandKey = key
  }).catch(() => {
    shellModel().slashCommands = []
    shellModel().loadedSlashCommandKey = key
  }).finally(() => {
    shellModel().slashCommandLoadInFlight = null
    renderSlashCommandPalette()
  })
}

const slashCommandQuery = (): string | null => {
  const value = composerInput.value.trimStart()
  if (!value.startsWith("/")) return null
  return value.slice(1).split(/\s+/)[0]?.toLowerCase() ?? ""
}

const matchingSlashCommands = (): readonly KhalaCodeMainShellSlashCommand[] => {
  const query = slashCommandQuery()
  if (query === null) return []
  return shellModel().slashCommands.filter(command =>
    command.command.includes(query) ||
    command.aliases.some(alias => alias.includes(query))
  ).slice(0, 8)
}

const selectSlashCommand = (command: KhalaCodeMainShellSlashCommand): void => {
  composerInput.value = command.supportsInlineArgs ? `/${command.command} ` : `/${command.command}`
  composerInput.focus({ preventScroll: true })
  const position = composerInput.value.length
  composerInput.setSelectionRange(position, position)
  renderComposer()
}

function renderSlashCommandPalette(): void {
  const query = slashCommandQuery()
  if (query === null) {
    slashCommandPalette.hidden = true
    slashCommandPalette.replaceChildren()
    return
  }
  ensureSlashCommandsLoaded()
  const matches = matchingSlashCommands()
  slashCommandPalette.hidden = matches.length === 0
  slashCommandPalette.replaceChildren(
    ...matches.map(command => {
      const button = document.createElement("button")
      button.type = "button"
      button.className = "khala-code-slash-command-option"
      button.disabled = !command.availability.available
      button.dataset.commandGroup = command.group
      button.setAttribute("role", "option")
      button.title = command.availability.reason ?? command.description
      button.addEventListener("mousedown", event => event.preventDefault())
      button.addEventListener("click", () => selectSlashCommand(command))

      const name = document.createElement("span")
      name.className = "khala-code-slash-command-name"
      name.textContent = `/${command.command}`

      const description = document.createElement("span")
      description.className = "khala-code-slash-command-description"
      description.textContent = command.availability.reason ?? command.description

      button.replaceChildren(name, description)
      return button
    }),
  )
}

function renderComposer(): void {
  const status = statusForComposer()
  const sendLabel = buttonLabel(status)
  const attachmentCount = shellModel().composerState.doc.attachments.length

  composerForm.dataset.oaCommandComposerStatus = status
  composerFrame.dataset.oaCommandComposerFrame = ""
  sendButton.disabled = !shellModel().pendingTurn && !canSubmitComposer()
  sendButton.type = shellModel().pendingTurn ? "button" : "submit"
  sendButton.title = sendLabel
  sendButton.setAttribute("aria-label", `${sendLabel} message`)
  sendButton.dataset.oaCommandComposerSubmit = shellModel().pendingTurn ? "stop" : "send"
  sendButton.dataset.status = status
  setButtonIcon(sendButton, shellModel().pendingTurn ? "stop" : "send")
  setButtonIcon(attachButton, "attach")
  sendButton.querySelector(".oa-ai-command-composer-submit-label")!.textContent =
    sendLabel

  const details = [
    statusLabelFor(status),
    ...(attachmentCount === 0 ? [] : [`${attachmentCount} attached`]),
    `${composerInput.value.length.toLocaleString()} chars`,
  ]
  composerStatus.className = composerClasses.status
  composerStatus.dataset.oaCommandComposerStatusLabel = status
  composerStatus.replaceChildren(
    renderHarnessPill(),
    renderRuntimeBadge(),
    ...details.map((detail, index) => {
      const span = document.createElement("span")
      span.dataset.slot = index === 0 ? "status" : "detail"
      if (index === 0) span.dataset.status = status
      span.textContent = detail
      return span
    }),
  )
  composerA11y.textContent =
    `${statusLabelFor(status)}. ${attachmentCount} attachments. ` +
    `${composerInput.value.length} characters.`

  renderAttachmentRail()
  renderComposerPreview()
  renderSlashCommandPalette()
  updateComposerHudProjection()
}

const render = (): void => {
  renderMessages()
  renderComposer()
}

const focusComposerInput = (): void => {
  composerInput.focus({ preventScroll: true })
  updateComposerHudProjection()
}

const nextMessageId = (role: KhalaCodeDesktopMessageRole): string =>
  `${role}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const nextTurnId = (): string =>
  `turn-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

const addMessage = (
  role: KhalaCodeDesktopMessageRole,
  body: string,
): KhalaCodeDesktopMessage => {
  const message = {
    id: nextMessageId(role),
    role,
    body,
  }
  setShellMessages([...shellModel().messages, message])
  render()
  return message
}

const appendMessages = (nextMessages: readonly KhalaCodeDesktopMessage[]): void => {
  if (nextMessages.length === 0) return
  const merged = [...shellModel().messages]
  const indexById = new Map(merged.map((message, index) => [message.id, index]))
  for (const message of nextMessages) {
    const index = indexById.get(message.id)
    if (index === undefined) {
      indexById.set(message.id, merged.length)
      merged.push(message)
    } else {
      merged[index] = message
    }
  }
  setShellMessages(merged)
  render()
}

const latestUserMessagePreview = (): string =>
  [...shellModel().messages].reverse().find(message => message.role === "user")?.body.trim() ?? ""

function applyChatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void {
  if (!activeTurnIds.has(event.turnId)) return
  if (event.type === "thread_ready") {
    setActiveCodexThreadId(event.threadId)
    threadSidebar?.upsertPendingThread({
      preview: latestUserMessagePreview(),
      threadId: event.threadId,
    })
    void refreshThreadTokenSummary()
    void threadSidebar?.refresh()
    return
  }
  if (
    event.type === "message_start" ||
    event.type === "message_delta" ||
    event.type === "message_replace"
  ) {
    shellModel().thinkingTurnId = null
  }
  switch (event.type) {
    case "message_start":
      shellModel().pendingTurn = true
      appendMessages([event.message])
      break
    case "message_delta":
      setShellMessages(shellModel().messages.map(message =>
        message.id === event.messageId
          ? { ...message, body: `${message.body}${event.delta}` }
          : message
      ))
      render()
      break
    case "message_replace":
      appendMessages([event.message])
      break
    case "message_done":
      break
    case "tool_event":
      break
  }
}

const parseDatasetJson = <T>(value: string | undefined): T | undefined => {
  if (value === undefined || value.length === 0) return undefined
  try {
    return JSON.parse(value) as T
  } catch {
    return undefined
  }
}

const respondToCodexApproval = async (button: HTMLButtonElement): Promise<void> => {
  const requestId = parseDatasetJson<Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["requestId"]>(
    button.dataset.codexApprovalRequestId,
  )
  const method = button.dataset.codexApprovalMethod as Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["method"] | undefined
  const action = button.dataset.codexApprovalAction as Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["action"] | undefined
  if (requestId === undefined || method === undefined || action === undefined) return

  button.disabled = true
  const execpolicyAmendment = parseDatasetJson<readonly string[]>(
    button.dataset.codexApprovalExecpolicyAmendment,
  )
  const networkPolicyAmendment = parseDatasetJson<
    Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["networkPolicyAmendment"]
  >(button.dataset.codexApprovalNetworkPolicyAmendment)
  const permissions = parseDatasetJson<
    Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]["permissions"]
  >(button.dataset.codexApprovalPermissions)
  const result = await rpc.request.codexApprovalRespond({
    action,
    method,
    requestId,
    ...(execpolicyAmendment === undefined ? {} : { execpolicyAmendment }),
    ...(networkPolicyAmendment === undefined ? {} : { networkPolicyAmendment }),
    ...(permissions === undefined ? {} : { permissions }),
  })
  appendMessages([{
    body: result.ok
      ? `Sent Codex approval decision: ${action}.`
      : `Codex approval decision failed: ${result.error ?? "unknown error"}`,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const handledClaudeApprovals = new Set<string>()
const respondToClaudeApprovalRequest = async (
  request: Awaited<ReturnType<DesktopRpcRequests["claudeApprovalPending"]>>["requests"][number],
): Promise<void> => {
  const title = request.options.title ?? request.options.displayName ?? `Claude wants to use ${request.toolName}`
  const detail = [
    title,
    request.options.description,
    request.options.decisionReason,
    request.options.blockedPath === undefined ? null : `Path: ${request.options.blockedPath}`,
    "",
    "Type always to allow and remember suggested permissions, allow to allow once, or deny.",
  ].filter((line): line is string => line !== null && line !== undefined).join("\n")
  const answer = window.prompt(detail, "allow")?.trim().toLowerCase() ?? "deny"
  const allow = answer === "allow" || answer === "always"
  const result = await controls.claudeApprovalRespond({
    requestId: request.id,
    decision: allow
      ? {
          behavior: "allow",
          decisionClassification: answer === "always" ? "always_allow" : "allow_once",
          ...(answer === "always" && request.options.suggestions !== undefined
            ? { updatedPermissions: request.options.suggestions }
            : {}),
        }
      : {
          behavior: "deny",
          decisionClassification: "deny",
          message: "Denied by the Khala Code Desktop operator.",
        },
  })
  appendMessages([{
    body: result.ok
      ? `Sent Claude approval decision: ${allow ? "allow" : "deny"}.`
      : `Claude approval decision failed: ${result.error ?? "unknown error"}`,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const pollClaudeApprovals = async (): Promise<void> => {
  if (shellModel().claudeApprovalDialogOpen) return
  const pending = await controls.claudeApprovalPending().catch(() => null)
  const request = pending?.requests.find(item => !handledClaudeApprovals.has(item.id))
  if (request === undefined) return
  handledClaudeApprovals.add(request.id)
  shellModel().claudeApprovalDialogOpen = true
  try {
    await respondToClaudeApprovalRequest(request)
  } finally {
    shellModel().claudeApprovalDialogOpen = false
  }
}

const attachmentSummaryForSubmit = (
  attachments: readonly ComposerAttachment[],
): string => {
  if (attachments.length === 0) return ""
  return [
    "Attachments:",
    ...attachments.map(
      attachment =>
        `- ${attachment.name} (${attachment.mime}, ${formatBytes(attachment.sizeBytes)}, ${attachment.status})`,
    ),
  ].join("\n")
}

const submittedBody = (
  text: string,
  attachments: readonly ComposerAttachment[],
): string => {
  const summary = attachmentSummaryForSubmit(attachments)
  if (summary === "") return text
  if (text.trim() === "") return summary
  return `${text}\n\n${summary}`
}

const imageAttachmentsForSubmit = async (
  attachments: readonly ComposerAttachment[],
): Promise<readonly KhalaCodeDesktopChatTurnAttachment[]> => {
  const payloads = await Promise.all(attachments.map(
    async (attachment): Promise<KhalaCodeDesktopChatTurnAttachment | null> => {
      if (attachment.kind !== "image" || attachment.status !== "ready") return null
      const file = localAttachmentFiles.get(attachment.id)
      if (file === undefined || !file.type.startsWith("image/")) return null
      const bytes = await file.arrayBuffer()
      return {
        dataBase64: base64FromArrayBuffer(bytes),
        id: attachment.id,
        kind: "image",
        mime: attachment.mime || file.type,
        name: attachment.name || file.name,
        sizeBytes: attachment.sizeBytes || file.size,
      }
    },
  ))
  return payloads.filter(
    (payload): payload is KhalaCodeDesktopChatTurnAttachment => payload !== null,
  )
}

const resetComposerDraft = (): void => {
  for (const url of objectUrls) URL.revokeObjectURL(url)
  objectUrls.clear()
  localAttachmentFiles.clear()
  localTextAttachments.clear()
  composerInput.value = ""
  setShellComposerState(emptyComposerState())
  renderComposer()
}

const stopActiveTurn = (): void => {
  if (!shellModel().pendingTurn) return
  const stoppedTurnIds = [...activeTurnIds]
  for (const turnId of stoppedTurnIds) {
    void rpc.request.codexTurnInterrupt({ sessionId, turnId }).catch(() => undefined)
  }
  activeTurnIds.clear()
  shellModel().pendingTurn = false
  shellModel().thinkingTurnId = null
  threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
  appendMessages([
    {
      body: "Requested Codex interrupt for the active turn. You can keep typing.",
      id: nextMessageId("system"),
      role: "system",
    },
  ])
  requestAnimationFrame(focusComposerInput)
}

const handleSlashCommandClientAction = async (
  result: Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>,
): Promise<string> => {
  switch (result.action) {
    case "clear_visible_transcript":
      setShellMessages([])
      activeTurnIds.clear()
      render()
      return "Cleared the visible transcript."
    case "copy_last_assistant_message": {
      const assistant = [...shellModel().messages].reverse().find(message => message.role === "assistant")
      if (assistant === undefined) return "No assistant message is available to copy."
      await navigator.clipboard?.writeText(assistant.body)
      return "Copied the last assistant message."
    }
    case "show_desktop_status": {
      const [harness, appServer] = await Promise.all([
        rpc.request.codexHarnessStatus(),
        rpc.request.codexAppServerStatus(),
      ])
      return [
        `Codex harness: ${harness.status}`,
        `Codex app-server: ${appServer.state}`,
        `Codex auth: ${harness.auth.state}`,
      ].join("\n")
    }
    default:
      return result.message
  }
}

const appendSlashCommandResult = async (
  result: Awaited<ReturnType<DesktopRpcRequests["slashCommandDispatch"]>>,
): Promise<void> => {
  const body =
    result.status === "client_action"
      ? await handleSlashCommandClientAction(result)
      : result.message
  appendMessages([{
    body,
    id: nextMessageId("system"),
    role: "system",
  }])
}

const submitSlashCommand = async (
  draftText: string,
  body: string,
): Promise<KhalaCodeDesktopMessage> => {
  shellModel().lastSubmittedDraft = draftText
  shellModel().lastTurnFailed = false
  resetComposerDraft()
  const message = addMessage("user", body)
  try {
    const result = await rpc.request.slashCommandDispatch({
      activeTurn: shellModel().pendingTurn,
      platform: slashCommandPlatform(),
      raw: draftText,
      sessionId,
      sideConversation: false,
    })
    await appendSlashCommandResult(result)
  } catch (error) {
    appendMessages([{
      body: `Slash command failed: ${error instanceof Error ? error.message : String(error)}`,
      id: nextMessageId("system"),
      role: "system",
    }])
  } finally {
    requestAnimationFrame(focusComposerInput)
  }
  return message
}

const submitComposer = async (): Promise<KhalaCodeDesktopMessage | null> => {
  if (shellModel().pendingTurn) {
    stopActiveTurn()
    return null
  }
  const draftText =
    composerInput.value.trim() === "" && shellModel().lastTurnFailed
      ? shellModel().lastSubmittedDraft
      : composerInput.value.trim()
  if (draftText === "" && shellModel().composerState.doc.attachments.length === 0) return null

  const attachments = [...shellModel().composerState.doc.attachments]
  const body = submittedBody(draftText, attachments)
  if (draftText.startsWith("/")) {
    return submitSlashCommand(draftText, body)
  }
  const imageAttachments = await imageAttachmentsForSubmit(attachments)
  shellModel().lastSubmittedDraft = draftText
  shellModel().lastTurnFailed = false
  resetComposerDraft()
  const message = addMessage("user", body)
  const turnId = nextTurnId()
  activeTurnIds.add(turnId)
  shellModel().pendingTurn = true
  shellModel().thinkingTurnId = turnId
  threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
  render()
  requestAnimationFrame(focusComposerInput)
  const turnStartedAt = performance.now()
  try {
    const activeThreadId = shellModel().activeCodexThreadId
    const request: KhalaCodeDesktopChatTurnRequest = {
      ...(imageAttachments.length === 0 ? {} : { attachments: imageAttachments }),
      messages: shellModel().messages,
      sessionId,
      ...(activeThreadId === null ? { startNewThread: true } : { threadId: activeThreadId }),
      turnId,
    }
    const response = await rpc.request.submitChatMessage(request)
    pushQaMetricSample("turn_start.latency_ms", performance.now() - turnStartedAt, {
      runtimeMode: response.backend.runtimeMode ?? shellModel().lastResponseRuntimeMode,
    })
    if (activeTurnIds.has(turnId)) {
      if (response.backend.runtimeMode !== undefined) {
        shellModel().lastResponseRuntimeMode = response.backend.runtimeMode
      }
      if (response.backend.threadId !== undefined) {
        setActiveCodexThreadId(response.backend.threadId)
        threadSidebar?.upsertPendingThread({
          preview: message.body,
          threadId: response.backend.threadId,
        })
        void threadSidebar?.refresh()
      }
      if (shellModel().thinkingTurnId === turnId) shellModel().thinkingTurnId = null
      appendMessages(response.messages)
    }
  } catch (error) {
    if (activeTurnIds.has(turnId)) {
      shellModel().lastTurnFailed = true
      appendMessages([
        {
          body: `Khala Code turn failed: ${error instanceof Error ? error.message : String(error)}`,
          id: nextMessageId("system"),
          role: "system",
        },
      ])
    }
  } finally {
    activeTurnIds.delete(turnId)
    if (shellModel().thinkingTurnId === turnId) shellModel().thinkingTurnId = null
    shellModel().pendingTurn = activeTurnIds.size > 0
    threadSidebar?.setActiveThreadId(shellModel().activeCodexThreadId)
    void threadSidebar?.refresh()
    renderComposer()
    requestAnimationFrame(focusComposerInput)
  }
  return message
}

const fileLikeFor = (file: File): ComposerFileLike => {
  const previewUrl = file.type.startsWith("image/")
    ? URL.createObjectURL(file)
    : undefined
  if (previewUrl !== undefined) objectUrls.add(previewUrl)
  return {
    name: file.name,
    type: file.type,
    size: file.size,
    ...(previewUrl === undefined ? {} : { previewUrl }),
  }
}

const stageFiles = (
  files: readonly File[],
  source: ComposerAttachmentSource,
): void => {
  if (files.length === 0) return
  const fileLikes = files.map(fileLikeFor)
  const staged =
    source === "paste"
      ? stageComposerPastedFiles(fileLikes)
      : source === "drop"
        ? stageComposerDroppedFiles(fileLikes)
        : stageComposerAttachmentFiles(fileLikes, { source: "manual" })
  if (!applyComposerStateTransaction(staged.transaction)) return
  staged.attachments.forEach((attachment, index) => {
    const file = files[index]
    if (file === undefined) return
    localAttachmentFiles.set(attachment.id, file)
    void runDesktopLocalAttachmentUpload(attachment.id, () => file.arrayBuffer())
  })
}

const stageLargeTextPaste = (text: string): boolean => {
  const offer = offerComposerLargeTextPaste(text)
  if (!offer.offered || offer.transaction === null || offer.attachment === undefined) {
    return false
  }
  if (offer.attachment.contentRef !== undefined) {
    localTextAttachments.set(offer.attachment.contentRef, text)
  }
  const applied = applyComposerStateTransaction(offer.transaction)
  if (applied) {
    void runDesktopLocalAttachmentUpload(
      offer.attachment.id,
      () => Promise.resolve(arrayBufferForText(text)),
    )
  }
  return applied
}

const setDragActive = (active: boolean): void => {
  shellModel().dragActive = active
  renderComposer()
}

const resizeComposerHud = (): void => {
  if (composerHudRuntime === null) return
  const rect = composerFrame.getBoundingClientRect()
  const width = Math.max(1, Math.ceil(rect.width))
  const height = Math.max(1, Math.ceil(rect.height))
  composerHudRuntime.renderer.setSize(width, height, false)
  const layout = commandComposerHudLayoutFromCssRect(rect, {
    pixelsPerWorldUnit: 100,
    minWorldWidth: 2,
    minWorldHeight: 1,
  })
  composerHudRuntime.camera.left = -layout.width / 2 - 0.18
  composerHudRuntime.camera.right = layout.width / 2 + 0.18
  composerHudRuntime.camera.top = layout.height / 2 + 0.18
  composerHudRuntime.camera.bottom = -layout.height / 2 - 0.18
  composerHudRuntime.camera.updateProjectionMatrix()
  composerHudRuntime.handle.setLayout(layout)
  composerHudRuntime.handle.setResolution(width, height)
  updateComposerHudProjection()
}

const mountComposerHud = (): ComposerHudRuntime | null => {
  try {
    const renderer = new Three.WebGLRenderer({
      antialias: true,
      alpha: true,
    })
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2))
    renderer.setClearColor(0x000000, 0)
    composerHudMount.replaceChildren(renderer.domElement)

    const scene = new Three.Scene()
    const camera = new Three.OrthographicCamera(-4, 4, 1.2, -1.2, 0.1, 20)
    camera.position.set(0, 0, 5)
    camera.lookAt(0, 0, 0)
    const handle = createCommandComposerHud({
      focused: true,
      reducedMotion: prefersReducedMotion?.matches === true,
      layout: commandComposerHudLayoutFromCssRect(
        composerFrame.getBoundingClientRect(),
        { pixelsPerWorldUnit: 100, minWorldWidth: 2, minWorldHeight: 1 },
      ),
    })
    scene.add(handle.object3D)

    let disposed = false
    let lastTime = performance.now()
    const frame = (time: number): void => {
      if (disposed) return
      const delta = Math.max(0, (time - lastTime) / 1000)
      lastTime = time
      handle.update(delta)
      renderer.render(scene, camera)
      requestAnimationFrame(frame)
    }
    requestAnimationFrame(frame)

    const observer =
      typeof ResizeObserver === "undefined"
        ? null
        : new ResizeObserver(() => resizeComposerHud())

    const runtime: ComposerHudRuntime = {
      renderer,
      scene,
      camera,
      handle,
      dispose: () => {
        if (disposed) return
        disposed = true
        observer?.disconnect()
        handle.dispose()
        renderer.dispose()
        renderer.domElement.remove()
      },
    }
    composerHudRuntime = runtime
    observer?.observe(composerFrame)
    resizeComposerHud()
    return runtime
  } catch {
    composerHudMount.hidden = true
    return null
  }
}

composerForm.addEventListener("submit", event => {
  event.preventDefault()
  void submitComposer().finally(() => composerInput.focus())
})

sendButton.addEventListener("click", event => {
  if (!shellModel().pendingTurn) return
  event.preventDefault()
  stopActiveTurn()
})

attachButton.addEventListener("click", () => {
  fileInput.click()
})

fileInput.addEventListener("change", () => {
  stageFiles(Array.from(fileInput.files ?? []), "manual")
  fileInput.value = ""
  requestAnimationFrame(focusComposerInput)
})

composerInput.addEventListener("input", () => {
  shellModel().lastTurnFailed = false
  renderComposer()
})

composerInput.addEventListener("focus", updateComposerHudProjection)
composerInput.addEventListener("blur", updateComposerHudProjection)

composerInput.addEventListener("keydown", event => {
  if (event.metaKey || event.ctrlKey || event.altKey) return
  if (
    event.key === "Enter" &&
    !event.shiftKey &&
    !shellModel().pendingTurn &&
    canSubmitComposer()
  ) {
    event.preventDefault()
    void submitComposer()
  }
})

composerInput.addEventListener("paste", event => {
  const data = event.clipboardData
  if (data === null) return
  const files = Array.from(data.files)
  if (files.length > 0) {
    event.preventDefault()
    stageFiles(files, "paste")
    requestAnimationFrame(focusComposerInput)
    return
  }
  const text = data.getData("text/plain")
  if (text !== "" && stageLargeTextPaste(text)) {
    event.preventDefault()
    requestAnimationFrame(focusComposerInput)
  }
})

for (const target of [composerForm, composerRail]) {
  target.addEventListener("dragenter", event => {
    event.preventDefault()
    setDragActive(true)
  })
  target.addEventListener("dragover", event => {
    event.preventDefault()
    setDragActive(true)
  })
  target.addEventListener("dragleave", event => {
    if (!composerForm.contains(event.relatedTarget as Node | null)) {
      setDragActive(false)
    }
  })
  target.addEventListener("drop", event => {
    event.preventDefault()
    setDragActive(false)
    stageFiles(Array.from(event.dataTransfer?.files ?? []), "drop")
    requestAnimationFrame(focusComposerInput)
  })
}

window.addEventListener("resize", resizeComposerHud)
window.setInterval(() => {
  void pollClaudeApprovals()
}, 1000)
messageList.addEventListener("click", event => {
  const target = event.target
  if (!(target instanceof Element)) return
  const button = target.closest<HTMLButtonElement>(".codex-approval-button")
  if (button === null) return
  event.preventDefault()
  event.stopPropagation()
  void respondToCodexApproval(button)
})
messageList.addEventListener("scroll", () => {
  shellModel().transcriptPinnedToEnd = isNearTranscriptEnd()
}, { passive: true })
threadTokenCounter.addEventListener("click", () => {
  shellModel().threadTokenPopoverOpen = !shellModel().threadTokenPopoverOpen
  renderThreadTokenCounter()
  if (shellModel().threadTokenPopoverOpen) void refreshThreadTokenSummary()
})
document.addEventListener("click", event => {
  if (!shellModel().threadTokenPopoverOpen) return
  const target = event.target
  if (target instanceof Node && threadTokenMeter.contains(target)) return
  shellModel().threadTokenPopoverOpen = false
  renderThreadTokenCounter()
})
window.addEventListener("wheel", proxyTranscriptWheel, { passive: false })
window.addEventListener("keydown", event => {
  if (event.key === "Escape" && shellModel().threadTokenPopoverOpen) {
    shellModel().threadTokenPopoverOpen = false
    renderThreadTokenCounter()
    return
  }
  proxyTranscriptKeyScroll(event)
})
window.setInterval(() => {
  if (document.hidden || shellModel().activeCodexThreadId === null) return
  void refreshThreadTokenSummary()
}, 2_000)
window.addEventListener("beforeunload", () => {
  for (const url of objectUrls) URL.revokeObjectURL(url)
  composerHudRuntime?.dispose()
})

const controls = {
  addMessage,
  appInfo: () => rpc.request.appInfo(),
  attachments: () => shellModel().composerState.doc.attachments.map(attachment => ({ ...attachment })),
  attachmentReceipts: () => shellModel().composerAttachmentReceipts.map(receipt => ({ ...receipt })),
  clearGymProof: (): GymPaneState => {
    const state = gymPaneStateFromBridgeProof(null)
    gymPanel?.setState(state)
    gymPanel?.setVisible(false)
    return state
  },
  codexAccountsStatus: () => rpc.request.codexAccountsStatus(),
  codexAppServerRestart: () => rpc.request.codexAppServerRestart(),
  codexAppServerStart: () => rpc.request.codexAppServerStart(),
  codexAppServerStatus: () => rpc.request.codexAppServerStatus(),
  codexAppServerStop: () => rpc.request.codexAppServerStop(),
  codexFleetDelegateRun: (request: Parameters<DesktopRpcRequests["codexFleetDelegateRun"]>[0]) =>
    rpc.request.codexFleetDelegateRun(request),
  codexFleetStatus: () => rpc.request.codexFleetStatus(),
  codexFleetPromoteThread: (request: Parameters<DesktopRpcRequests["codexFleetPromoteThread"]>[0]) =>
    rpc.request.codexFleetPromoteThread(request),
  fleetRunControl: (request: Parameters<DesktopRpcRequests["fleetRunControl"]>[0]) =>
    rpc.request.fleetRunControl(request),
  fleetRunList: (request?: Parameters<DesktopRpcRequests["fleetRunList"]>[0]) =>
    rpc.request.fleetRunList(request),
  fleetRunStart: (request: Parameters<DesktopRpcRequests["fleetRunStart"]>[0]) =>
    rpc.request.fleetRunStart(request),
  fleetWorkerControl: (request: Parameters<DesktopRpcRequests["fleetWorkerControl"]>[0]) =>
    rpc.request.fleetWorkerControl(request),
  forumRequest: (request: Parameters<DesktopRpcRequests["forumRequest"]>[0]) =>
    rpc.request.forumRequest(request),
  khalaCodePlanCatalog: () => rpc.request.khalaCodePlanCatalog(),
  khalaCodePlanStatus: () => rpc.request.khalaCodePlanStatus(),
  khalaCodePlanPurchase: (request?: Parameters<DesktopRpcRequests["khalaCodePlanPurchase"]>[0]) =>
    rpc.request.khalaCodePlanPurchase(request),
  claudeApprovalPending: () => rpc.request.claudeApprovalPending(),
  claudeApprovalRespond: (request: Parameters<DesktopRpcRequests["claudeApprovalRespond"]>[0]) =>
    rpc.request.claudeApprovalRespond(request),
  claudeSettingsRead: () => rpc.request.claudeSettingsRead(),
  codexHarnessStatus: () => rpc.request.codexHarnessStatus(),
  codexApprovalRespond: (request: Parameters<DesktopRpcRequests["codexApprovalRespond"]>[0]) =>
    rpc.request.codexApprovalRespond(request),
  codexConfigValueWrite: (request: Parameters<DesktopRpcRequests["codexConfigValueWrite"]>[0]) =>
    rpc.request.codexConfigValueWrite(request),
  codexEcosystemRead: (request?: Parameters<DesktopRpcRequests["codexEcosystemRead"]>[0]) =>
    rpc.request.codexEcosystemRead(request),
  codexExternalAgentConfigDetect: (request?: Parameters<DesktopRpcRequests["codexExternalAgentConfigDetect"]>[0]) =>
    rpc.request.codexExternalAgentConfigDetect(request),
  codexExternalAgentConfigImport: (request: Parameters<DesktopRpcRequests["codexExternalAgentConfigImport"]>[0]) =>
    rpc.request.codexExternalAgentConfigImport(request),
  codexExternalAgentConfigImportHistoriesRead: () =>
    rpc.request.codexExternalAgentConfigImportHistoriesRead(),
  codexFsGetMetadata: (request: Parameters<DesktopRpcRequests["codexFsGetMetadata"]>[0]) =>
    rpc.request.codexFsGetMetadata(request),
  codexFsReadFile: (request: Parameters<DesktopRpcRequests["codexFsReadFile"]>[0]) =>
    rpc.request.codexFsReadFile(request),
  codexFsWriteFile: (request: Parameters<DesktopRpcRequests["codexFsWriteFile"]>[0]) =>
    rpc.request.codexFsWriteFile(request),
  codexMarketplaceAdd: (request: Parameters<DesktopRpcRequests["codexMarketplaceAdd"]>[0]) =>
    rpc.request.codexMarketplaceAdd(request),
  codexMarketplaceRemove: (request: Parameters<DesktopRpcRequests["codexMarketplaceRemove"]>[0]) =>
    rpc.request.codexMarketplaceRemove(request),
  codexMarketplaceUpgrade: (request?: Parameters<DesktopRpcRequests["codexMarketplaceUpgrade"]>[0]) =>
    rpc.request.codexMarketplaceUpgrade(request),
  codexMentionCandidates: (request?: Parameters<DesktopRpcRequests["codexMentionCandidates"]>[0]) =>
    rpc.request.codexMentionCandidates(request),
  codexMcpOauthLogin: (request: Parameters<DesktopRpcRequests["codexMcpOauthLogin"]>[0]) =>
    rpc.request.codexMcpOauthLogin(request),
  codexMcpResourceRead: (request: Parameters<DesktopRpcRequests["codexMcpResourceRead"]>[0]) =>
    rpc.request.codexMcpResourceRead(request),
  codexMcpServerReload: () => rpc.request.codexMcpServerReload(),
  codexMcpToolCall: (request: Parameters<DesktopRpcRequests["codexMcpToolCall"]>[0]) =>
    rpc.request.codexMcpToolCall(request),
  codexPluginInstall: (request: Parameters<DesktopRpcRequests["codexPluginInstall"]>[0]) =>
    rpc.request.codexPluginInstall(request),
  codexPluginUninstall: (request: Parameters<DesktopRpcRequests["codexPluginUninstall"]>[0]) =>
    rpc.request.codexPluginUninstall(request),
  codexSettingsRead: (request?: Parameters<DesktopRpcRequests["codexSettingsRead"]>[0]) =>
    rpc.request.codexSettingsRead(request),
  codexSkillsConfigWrite: (request: Parameters<DesktopRpcRequests["codexSkillsConfigWrite"]>[0]) =>
    rpc.request.codexSkillsConfigWrite(request),
  codexSkillsExtraRootsSet: (request: Parameters<DesktopRpcRequests["codexSkillsExtraRootsSet"]>[0]) =>
    rpc.request.codexSkillsExtraRootsSet(request),
  codexThreadArchive: (request: Parameters<DesktopRpcRequests["codexThreadArchive"]>[0]) =>
    rpc.request.codexThreadArchive(request),
  codexThreadCompact: (request: Parameters<DesktopRpcRequests["codexThreadCompact"]>[0]) =>
    rpc.request.codexThreadCompact(request),
  codexThreadDelete: (request: Parameters<DesktopRpcRequests["codexThreadDelete"]>[0]) =>
    rpc.request.codexThreadDelete(request),
  codexThreadFork: (request: Parameters<DesktopRpcRequests["codexThreadFork"]>[0]) =>
    rpc.request.codexThreadFork(request),
  codexThreadList: (request?: Parameters<DesktopRpcRequests["codexThreadList"]>[0]) =>
    rpc.request.codexThreadList(request),
  codexThreadRead: (request: Parameters<DesktopRpcRequests["codexThreadRead"]>[0]) =>
    rpc.request.codexThreadRead(request),
  codexThreadRename: (request: Parameters<DesktopRpcRequests["codexThreadRename"]>[0]) =>
    rpc.request.codexThreadRename(request),
  codexThreadResume: (request: Parameters<DesktopRpcRequests["codexThreadResume"]>[0]) =>
    rpc.request.codexThreadResume(request),
  codexThreadStart: (request?: Parameters<DesktopRpcRequests["codexThreadStart"]>[0]) =>
    rpc.request.codexThreadStart(request),
  codexThreadUnarchive: (request: Parameters<DesktopRpcRequests["codexThreadUnarchive"]>[0]) =>
    rpc.request.codexThreadUnarchive(request),
  sessionCatalog: (request?: Parameters<DesktopRpcRequests["sessionCatalog"]>[0]) =>
    rpc.request.sessionCatalog(request),
  codexTurnInterrupt: (request: Parameters<DesktopRpcRequests["codexTurnInterrupt"]>[0]) =>
    rpc.request.codexTurnInterrupt(request),
  codexTurnStart: (request: Parameters<DesktopRpcRequests["codexTurnStart"]>[0]) =>
    rpc.request.codexTurnStart(request),
  codexTurnSteer: (request: Parameters<DesktopRpcRequests["codexTurnSteer"]>[0]) =>
    rpc.request.codexTurnSteer(request),
  codingStatus: () => rpc.request.codingStatus(),
  connectCodexAccount: (accountRef: string) =>
    rpc.request.connectCodexAccount(accountRef),
  gymState: (): GymPaneState | null => gymPanel?.snapshot() ?? null,
  openExternalUrl: (url: string) => rpc.request.openExternalUrl(url),
  composerStatus: statusForComposer,
  consumeCodexRateLimitResetCredit: (request: Parameters<DesktopRpcRequests["consumeCodexRateLimitResetCredit"]>[0]) =>
    rpc.request.consumeCodexRateLimitResetCredit(request),
  focusComposer: focusComposerInput,
  isComposerFocused: () => document.activeElement === composerInput,
  isPending: () => shellModel().pendingTurn,
  loadGymDemoProof: (): GymPaneState => {
    const state = gymPaneStateFromBridgeProof({
      proof: khalaCodeGymDemoBridgeProof,
      generatedAt: "time.khala_gym_projection.fixture",
      sourceRef: "fixture.khala_code.gym.part2_demo",
    })
    gymPanel?.setState(state)
    showGymProofPane()
    return state
  },
  loadGymProof: (
    input: KhalaGymBridgeProofLike | KhalaGymProofLoadRequest | string | null,
  ): GymPaneState => {
    const parsed =
      typeof input === "string"
        ? (JSON.parse(input) as KhalaGymBridgeProofLike | KhalaGymProofLoadRequest)
        : input
    const state = gymPaneStateFromBridgeProof(parsed)
    gymPanel?.setState(state)
    showGymProofPane()
    return state
  },
  messages: () => shellModel().messages.map(message => ({ ...message })),
  pylonStatus: () => rpc.request.pylonStatus(),
  qaMetrics: qaMetricsSnapshot,
  removeCodexAccount: (accountRef: string) =>
    rpc.request.removeCodexAccount(accountRef),
  setCodexAccountPaused: (request: { accountRef: string; paused: boolean }) =>
    rpc.request.setCodexAccountPaused(request),
  slashCommandDispatch: (request: Parameters<DesktopRpcRequests["slashCommandDispatch"]>[0]) =>
    rpc.request.slashCommandDispatch(request),
  slashCommandList: (request?: Parameters<DesktopRpcRequests["slashCommandList"]>[0]) =>
    rpc.request.slashCommandList(request),
  reset: () => {
    setShellMessages([])
    activeTurnIds.clear()
    resetComposerDraft()
    shellModel().pendingTurn = false
    shellModel().thinkingTurnId = null
    shellModel().lastTurnFailed = false
    render()
  },
  setComposerDraft: (value: string) => {
    composerInput.value = value
    shellModel().lastTurnFailed = false
    renderComposer()
  },
  setGymState: (state: GymPaneState) => gymPanel?.setState(state),
  simulateLargePaste: (value: string) => stageLargeTextPaste(value),
  stageAttachmentForSmoke: (input: {
    name: string
    type?: string
    size?: number
    source?: ComposerAttachmentSource
  }) => {
    const staged = stageComposerAttachmentFiles(
      [
        {
          name: input.name,
          type: input.type ?? "text/plain",
          size: input.size ?? 10,
        },
      ],
      { source: input.source ?? "manual" },
    )
    applyComposerStateTransaction(staged.transaction)
  },
  stopTurn: stopActiveTurn,
  submitComposer,
  tokenAccountingStatus: () => rpc.request.tokenAccountingStatus(),
  threadTokenSummary: (request?: Parameters<DesktopRpcRequests["threadTokenSummary"]>[0]) =>
    rpc.request.threadTokenSummary(request),
  threadSwitchPerformance: () => ({
    cachedThreadIds: [...threadMessageCache.keys()],
    latest: threadSwitchPerformanceSamples.at(-1) ?? null,
    pendingSelectionIds: [...pendingThreadSwitches.keys()],
    samples: threadSwitchPerformanceSamples.map(sample => ({ ...sample })),
  }),
  resetThreadSwitchPerformance: () => {
    threadSwitchPerformanceSamples.splice(0)
    pendingThreadSwitches.clear()
    qaMetricSamples.splice(0)
  },
  toolCatalog: () => rpc.request.toolCatalog(),
}

Object.assign(globalThis, {
  khalaCodeDesktop: controls,
})

void controls.appInfo().catch(() => undefined)
mountComposerHud()

const sidebarNavRoot = document.getElementById("sidebar-nav-root")
const threadSidebarEl = document.getElementById("thread-sidebar")
const fleetPanelEl = document.getElementById("fleet-panel")
const forumPanelEl = document.getElementById("forum-panel")
const inboxPanelEl = document.getElementById("inbox-panel")
const gymPanelEl = document.getElementById("gym-panel")
const settingsPanelEl = document.getElementById("settings-panel")
const threadShell = document.querySelector<HTMLElement>(".khala-code-thread-shell")
const composerDock = document.querySelector<HTMLElement>(".composer-dock")
const initialGymState = gymPaneStateFromLocation(globalThis.location)
const initialView = initialKhalaCodeViewFromLocation(globalThis.location)

const setActiveCodexThreadId = (threadId: string | null): void => {
  const changed = shellModel().activeCodexThreadId !== threadId
  shellModel().activeCodexThreadId = threadId
  if (threadId === null) {
    localStorage.removeItem(activeThreadIdStorageKey)
  } else {
    localStorage.setItem(activeThreadIdStorageKey, threadId)
  }
  if (changed) {
    shellModel().threadTokenSummary = emptyThreadTokenSummary(threadId)
    renderThreadTokenCounter()
    void refreshThreadTokenSummary()
  }
  threadSidebar?.setActiveThreadId(threadId)
}

const beginCodexThreadSwitch = (input: {
  readonly selectionId: number
  readonly source: CodexThreadSelectionSource
  readonly threadId: string
}): void => {
  cacheVisibleThreadMessages()
  const cached = cachedThreadMessages(input.threadId)
  setActiveCodexThreadId(input.threadId)
  if (cached !== null) {
    setShellMessages(cached)
  }
  activeTurnIds.clear()
  shellModel().pendingTurn = false
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  beginThreadSwitchPerformanceSample({
    cacheHit: cached !== null,
    optimisticMessageCount: cached?.length ?? 0,
    selectionId: input.selectionId,
    source: input.source,
    threadId: input.threadId,
  })
  requestAnimationFrame(focusComposerInput)
}

const activateCodexThread = (input: {
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly selectionId?: number
  readonly threadId: string
}): void => {
  setActiveCodexThreadId(input.threadId)
  cacheThreadMessages(input.threadId, input.messages)
  const visibleMessages = recentMessagesForInitialThreadRender(input.messages)
  setShellMessages(visibleMessages)
  const hydratedVisibleMessages = shellModel().messages
  activeTurnIds.clear()
  shellModel().pendingTurn = false
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  completeThreadSwitchPerformanceSample({
    fullMessageCount: input.messages.length,
    ...(input.selectionId === undefined ? {} : { selectionId: input.selectionId }),
  })
  scheduleFullThreadHydration({
    fullMessages: input.messages,
    ...(input.selectionId === undefined ? {} : { selectionId: input.selectionId }),
    threadId: input.threadId,
    visibleMessages: hydratedVisibleMessages,
  })
  requestAnimationFrame(focusComposerInput)
}

const beginNewCodexThread = (): void => {
  setActiveCodexThreadId(null)
  setShellMessages([])
  activeTurnIds.clear()
  shellModel().pendingTurn = false
  shellModel().thinkingTurnId = null
  shellModel().lastTurnFailed = false
  render()
  requestAnimationFrame(focusComposerInput)
}

const loadGymDemoOptimization = () => {
  const state = gymPaneStateFromBridgeProof({
    proof: khalaCodeGymDemoBridgeProof,
    generatedAt: new Date().toISOString(),
    sourceRef: "fixture.khala_code.gym.part2_demo",
  })
  gymPanel?.setState(state)
  showGymProofPane()
  return gymOptimizationRunFromBridgeProof(khalaCodeGymDemoBridgeProof)
}

const fleetPanel =
  fleetPanelEl === null
    ? null
    : mountFleetPanel(fleetPanelEl, {
        delegateRun: request => controls.codexFleetDelegateRun(request),
        fleetRunControl: request => controls.fleetRunControl(request),
        fleetRunList: request => controls.fleetRunList(request),
        fleetRunStart: request => controls.fleetRunStart(request),
        fleetWorkerControl: request => controls.fleetWorkerControl(request),
        lifecycleNdjson: fleetLifecycleLines.iterable,
        loadGymDemoProof: () => loadGymDemoOptimization(),
        startDelegationOptimization: async () => loadGymDemoOptimization(),
        fetch: async () => {
          const status = await controls.codexFleetStatus()
          sidebar?.setFleetCounts(projectKhalaCodeSidebarFleetCounts(status))
          return status
        },
        removeAccount: async accountRef => {
          const result = await controls.removeCodexAccount(accountRef)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        setAccountPaused: async request => {
          const result = await controls.setCodexAccountPaused(request)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        consumeResetCredit: async request => {
          const result = await controls.consumeCodexRateLimitResetCredit(request)
          return {
            ok: result.ok,
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
        connectAccount: accountRef => controls.connectCodexAccount(accountRef),
        openExternal: url => controls.openExternalUrl(url),
      })

const inboxPanel =
  inboxPanelEl === null
    ? null
    : mountUnifiedInboxPanel(inboxPanelEl, {
        fetch: async (): Promise<import("./inbox.js").UnifiedInboxSource> => ({
          codexHarness: await controls.codexHarnessStatus(),
          ecosystem: await controls.codexEcosystemRead({}),
          fleet: await controls.codexFleetStatus(),
          pylon: await controls.pylonStatus(),
          coding: await controls.codingStatus(),
          tokenAccounting: await controls.tokenAccountingStatus(),
        }),
        onOpenFleet: () => setActiveView("fleet"),
        onOpenSettings: () => setActiveView("settings"),
        onReconnectAccount: accountRef => {
          void controls.connectCodexAccount(accountRef).then(() => {
            void refreshFleetSidebarCounts()
            void inboxPanel?.refresh()
          })
        },
        onResumeRun: async runRef => {
          await controls.fleetRunControl({ runRef, verb: "resume" })
          await refreshFleetSidebarCounts()
          await inboxPanel?.refresh()
        },
      })

const forumPanel =
  forumPanelEl === null
    ? null
    : mountKhalaCodeForumPanel(forumPanelEl, {
        request: async request => {
          const result = await controls.forumRequest(request)
          if (!result.ok) {
            throw new Error(result.error ?? `Forum request failed with ${result.status}`)
          }
          return result.payload
        },
        openExternal: url => controls.openExternalUrl(url),
      })

const gymPanel =
  gymPanelEl === null ? null : mountGymPane(gymPanelEl, initialGymState)

let claudeSettingsSection: ReturnType<typeof mountClaudeSettingsSection> | null = null
let plansSection: ReturnType<typeof mountKhalaCodePlansPanel> | null = null

const settingsPanel =
  settingsPanelEl === null
    ? null
    : mountCodexSettingsPanel(settingsPanelEl, {
        fetch: () => controls.codexSettingsRead({ includeHiddenModels: true }),
        onRender: () => {
          void claudeSettingsSection?.refresh()
          void plansSection?.refresh()
        },
        write: async request => {
          const result = await controls.codexConfigValueWrite(request)
          return {
            ok: result.ok,
            ...(result.settings === undefined ? {} : { settings: result.settings }),
            ...(result.error === undefined ? {} : { error: result.error }),
          }
        },
      })
claudeSettingsSection = settingsPanelEl === null
  ? null
  : mountClaudeSettingsSection(settingsPanelEl, {
      fetch: () => controls.claudeSettingsRead(),
    })
plansSection = settingsPanelEl === null
  ? null
  : mountKhalaCodePlansPanel(settingsPanelEl, {
      catalog: () => controls.khalaCodePlanCatalog(),
      purchase: request => controls.khalaCodePlanPurchase(request),
      status: () => controls.khalaCodePlanStatus(),
    })

const threadListCacheKey = (
  request: {
    readonly limit?: number | undefined
    readonly searchTerm?: string | undefined
  },
): string => JSON.stringify({
  limit: request.limit ?? null,
  searchTerm: request.searchTerm?.trim() ?? "",
})

const clearThreadListCache = (): void => {
  threadListCache.clear()
  threadListRequests.clear()
}

const cachedSessionCatalog = async (
  request: Parameters<DesktopRpcRequests["sessionCatalog"]>[0],
): Promise<Awaited<ReturnType<DesktopRpcRequests["sessionCatalog"]>>> => {
  const key = threadListCacheKey(request ?? {})
  const cached = threadListCache.get(key)
  if (cached !== undefined && performance.now() - cached.cachedAt < THREAD_LIST_CACHE_TTL_MS) {
    return cached.result
  }

  const inFlight = threadListRequests.get(key)
  if (inFlight !== undefined) return inFlight

  const promise = controls.sessionCatalog(request).then(result => {
    threadListCache.set(key, { cachedAt: performance.now(), result })
    return result
  }).finally(() => {
    threadListRequests.delete(key)
  })
  threadListRequests.set(key, promise)
  return promise
}

const scheduleIdle = (task: () => void): void => {
  if (typeof requestIdleCallback === "function") {
    requestIdleCallback(task, { timeout: 450 })
    return
  }
  window.setTimeout(task, 80)
}

const prefetchRecentThreadMessages = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): void => {
  for (const thread of recentThreadsForHotkeys(threads).slice(0, THREAD_PREFETCH_LIMIT)) {
    if (threadMessageCache.has(thread.id) || threadPrefetches.has(thread.id)) continue
    const prefetch = controls.codexThreadRead({
      includeTurns: true,
      threadId: thread.id,
    }).then(result => {
      cacheThreadMessages(result.threadId, result.messages ?? [])
    }).catch(() => undefined).finally(() => {
      threadPrefetches.delete(thread.id)
    })
    threadPrefetches.set(thread.id, prefetch)
  }
}

const schedulePrefetchRecentThreadMessages = (
  threads: readonly KhalaCodeDesktopCodexThreadSummary[],
): void => {
  scheduleIdle(() => prefetchRecentThreadMessages(threads))
}

const threadSidebar =
  threadSidebarEl === null
    ? null
    : mountCodexThreadSidebar(threadSidebarEl, {
        activeThreadId: () => shellModel().activeCodexThreadId,
        archiveThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadArchive({ threadId })
        },
        deleteThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadDelete({ threadId })
        },
        forkThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadFork({ sessionId, threadId })
        },
        isThreadStreaming: threadId => shellModel().activeCodexThreadId === threadId && shellModel().pendingTurn,
        listThreads: async request => {
          const catalog = await cachedSessionCatalog({
            limit: 50,
            searchTerm: request.searchTerm,
          })
          const threads = catalog.entries.map(sessionCatalogEntryToThreadSummary)
          const result = {
            ok: true as const,
            data: catalog.entries,
            groups: [
              {
                key: "all-harnesses",
                label: "All sessions",
                threadIds: threads.map(thread => thread.id),
              },
            ],
            threads,
          }
          schedulePrefetchRecentThreadMessages(result.threads ?? [])
          return result
        },
        renameThread: async (threadId, name) => {
          clearThreadListCache()
          return controls.codexThreadRename({ name, threadId })
        },
        resumeThread: threadId => controls.codexThreadResume({ sessionId, threadId }),
        sessionId,
        unarchiveThread: async threadId => {
          clearThreadListCache()
          return controls.codexThreadUnarchive({ threadId })
        },
        onNewThreadRequested: beginNewCodexThread,
        onThreadSelectionStarted: beginCodexThreadSwitch,
        onThreadSelected: activateCodexThread,
      })

window.addEventListener("keydown", event => {
  const recentThreadIndex = recentThreadHotkeyIndexForEvent(event)
  const recentThreadCycleDirection = recentThreadCycleDirectionForEvent(event)
  if (recentThreadIndex === null && recentThreadCycleDirection === null) return
  if (threadSidebar === null) return

  event.preventDefault()
  const selection = recentThreadIndex !== null
    ? threadSidebar.selectRecentThread(recentThreadIndex)
    : recentThreadCycleDirection === null
      ? Promise.resolve(false)
      : threadSidebar.selectAdjacentRecentThread(recentThreadCycleDirection)
  void selection.then(selected => {
    if (selected) setActiveView("chat")
  })
})

const setActiveView = (value: string): void => {
  const panelOpenStartedAt = performance.now()
  const activeValue =
    value === "fleet" || value === "forum" || value === "inbox" || value === "settings"
      ? value
      : "chat"
  const showChat = activeValue === "chat"
  const showFleet = activeValue === "fleet"
  const showForum = activeValue === "forum"
  const showInbox = activeValue === "inbox"
  const showSettings = activeValue === "settings"
  if (threadSidebarEl !== null) threadSidebarEl.hidden = !showChat
  if (fleetPanelEl !== null) fleetPanelEl.hidden = !showFleet
  if (forumPanelEl !== null) forumPanelEl.hidden = !showForum
  if (inboxPanelEl !== null) inboxPanelEl.hidden = !showInbox
  if (settingsPanelEl !== null) settingsPanelEl.hidden = !showSettings
  gymPanel?.setVisible(false)
  if (threadShell !== null) threadShell.hidden = showFleet || showForum || showInbox || showSettings
  if (composerDock !== null) composerDock.hidden = showFleet || showForum || showInbox || showSettings
  fleetPanel?.setVisible(showFleet)
  forumPanel?.setVisible(showForum)
  inboxPanel?.setVisible(showInbox)
  settingsPanel?.setVisible(showSettings)
  if (showSettings) {
    void claudeSettingsSection?.refresh()
    void plansSection?.refresh()
  }
  threadSidebar?.setVisible(showChat)
  if (!showChat) markQaTimer("panel.open_ms", panelOpenStartedAt, { panel: activeValue })
  if (showChat) {
    requestAnimationFrame(focusComposerInput)
  }
}

function showGymProofPane(): void {
  if (threadSidebarEl !== null) threadSidebarEl.hidden = true
  if (fleetPanelEl !== null) fleetPanelEl.hidden = true
  if (forumPanelEl !== null) forumPanelEl.hidden = true
  if (inboxPanelEl !== null) inboxPanelEl.hidden = true
  if (settingsPanelEl !== null) settingsPanelEl.hidden = true
  if (threadShell !== null) threadShell.hidden = true
  if (composerDock !== null) composerDock.hidden = true
  gymPanel?.setVisible(true)
  fleetPanel?.setVisible(false)
  forumPanel?.setVisible(false)
  inboxPanel?.setVisible(false)
  settingsPanel?.setVisible(false)
  threadSidebar?.setVisible(false)
}

const sidebar =
  sidebarNavRoot === null
    ? null
    : mountKhalaCodeSidebar(sidebarNavRoot, {
        selectedValue: initialView,
        onActivate: value => setActiveView(value),
      })

const refreshFleetSidebarCounts = async (): Promise<void> => {
  if (sidebar === null) return
  try {
    sidebar.setFleetCounts(projectKhalaCodeSidebarFleetCounts(await controls.codexFleetStatus()))
  } catch {
    sidebar.setFleetCounts(null)
  }
}

if (sidebarNavRoot !== null) {
  void refreshFleetSidebarCounts()
}
setActiveView(initialView)
renderThreadTokenCounter()
void refreshThreadTokenSummary()
const firstRenderStartedAt = performance.now()
render()
markQaTimer("first_render.ms", firstRenderStartedAt, { view: initialView })
requestAnimationFrame(focusComposerInput)
