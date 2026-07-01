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
  type ComposerState,
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
import * as Three from "three"

import {
  KHALA_CODE_DESKTOP_DEFAULT_PREVIEW_PORT,
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaCodeDesktopChatTurnAttachment,
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopMessageRole,
  type KhalaCodeDesktopRPCSchema,
  type KhalaCodeDesktopThreadTokenSummary,
} from "../shared/rpc"
import { renderMessageBody } from "./transcript-render"
import { mountFleetPanel } from "./fleet-status"
import { mountCodexSettingsPanel } from "./codex-settings-panel"
import { mountCodexThreadSidebar } from "./codex-thread-sidebar"
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
import { mountKhalaCodeSidebar } from "./sidebar"
import { recentThreadIndexForDigitKey } from "./thread-hotkeys"
import "./styles.css"

type DesktopRpc = ReturnType<typeof Electroview.defineRPC<KhalaCodeDesktopRPCSchema>>
type DesktopRpcRequests = KhalaCodeDesktopRPCSchema["requests"]
type SlashCommandEntry =
  Awaited<ReturnType<DesktopRpcRequests["slashCommandList"]>>["commands"][number]

const postPreviewRpc = async <Result>(
  method: string,
  ...args: readonly unknown[]
): Promise<Result> => {
  const response = await fetch(`/rpc/${encodeURIComponent(method)}`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ args }),
  })
  if (!response.ok) throw new Error(`${method} failed with ${response.status}`)
  return await response.json() as Result
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
    openExternalUrl: url =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["openExternalUrl"]>>
      >("openExternalUrl", url),
    consumeCodexRateLimitResetCredit: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["consumeCodexRateLimitResetCredit"]>>
      >("consumeCodexRateLimitResetCredit"),
    onDeviceDeciderStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["onDeviceDeciderStatus"]>>
      >("onDeviceDeciderStatus"),
    pylonStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["pylonStatus"]>>
      >("pylonStatus"),
    removeCodexAccount: accountRef =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["removeCodexAccount"]>>
      >("removeCodexAccount", accountRef),
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
  },
})

const nativeRpc = Electroview.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {
      chatTurnEvent(event) {
        applyChatTurnEvent(event)
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

let messages: KhalaCodeDesktopMessage[] = []
let composerState: ComposerState = emptyComposerState()
let pendingTurn = false
let thinkingTurnId: string | null = null
let lastTurnFailed = false
let lastSubmittedDraft = ""
let dragActive = false
let transcriptPinnedToEnd = true
let slashCommands: SlashCommandEntry[] = []
let slashCommandLoadInFlight: Promise<void> | null = null
const activeTurnIds = new Set<string>()
const objectUrls = new Set<string>()
const localTextAttachments = new Map<string, string>()
const localAttachmentFiles = new Map<string, File>()
let composerAttachmentReceipts: ComposerAttachmentUploadReceipt[] = []
const sessionIdStorageKey = "khala-code-desktop.session-id.v1"
const activeThreadIdStorageKey = "khala-code-desktop.active-thread-id.v1"
const storedSessionId = localStorage.getItem(sessionIdStorageKey)
const sessionId =
  storedSessionId?.startsWith("khala-code-desktop-") === true
    ? storedSessionId
    : `khala-code-desktop-${Date.now().toString(36)}`
localStorage.setItem(sessionIdStorageKey, sessionId)
localStorage.removeItem(activeThreadIdStorageKey)
let activeCodexThreadId: string | null = null

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

let threadTokenSummary = emptyThreadTokenSummary(null)
let threadTokenPopoverOpen = false
let threadTokenRefreshInFlight = false
let threadTokenRefreshQueued = false

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

const renderThinkingIndicator = (): HTMLElement | null => {
  if (thinkingTurnId === null) return null

  const article = document.createElement("article")
  article.className = `${messageClass("assistant")} message-bubble--thinking`
  article.dataset.messageId = `thinking-${thinkingTurnId}`
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
  transcriptPinnedToEnd = true
  messageList.scrollTo({
    top: messageList.scrollHeight,
    behavior,
  })
}

const setTranscriptScrollTop = (top: number): void => {
  messageList.scrollTop = Math.max(0, Math.min(top, maxTranscriptScrollTop()))
  transcriptPinnedToEnd = isNearTranscriptEnd()
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

const statusForComposer = (): CommandComposerStatus => {
  if (pendingTurn) return "streaming"
  if (lastTurnFailed) return "error"
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
  const summary = threadTokenSummary
  threadTokenCounterValue.textContent = formatCompactTokens(summary.totalTokens)
  threadTokenCounter.setAttribute(
    "aria-label",
    `${formatExactTokens(summary.totalTokens)} thread tokens, ${formatExactTokens(summary.leaderboardSyncedTokens)} synced to leaderboard`,
  )
  threadTokenCounter.setAttribute("aria-expanded", String(threadTokenPopoverOpen))
  threadTokenPopover.hidden = !threadTokenPopoverOpen
  if (!threadTokenPopoverOpen) return

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
      "Codex state",
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
  meta.textContent = summary.remoteDisabled
    ? "Sync disabled"
    : summary.remoteConfigured
      ? `Updated ${formatThreadTokenUpdatedAt(summary.updatedAt)}`
      : "Remote sync not configured"

  threadTokenPopover.replaceChildren(title, rows, meta)
}

const refreshThreadTokenSummary = async (): Promise<void> => {
  const threadId = activeCodexThreadId
  if (threadId === null) {
    threadTokenSummary = emptyThreadTokenSummary(null)
    renderThreadTokenCounter()
    return
  }
  if (threadTokenRefreshInFlight) {
    threadTokenRefreshQueued = true
    return
  }

  threadTokenRefreshInFlight = true
  try {
    const summary = await rpc.request.threadTokenSummary({ threadId })
    if (activeCodexThreadId === threadId) {
      threadTokenSummary = summary
      renderThreadTokenCounter()
    }
  } catch {
    if (activeCodexThreadId === threadId) {
      threadTokenSummary = {
        ...emptyThreadTokenSummary(threadId),
        totalTokens: threadTokenSummary.threadId === threadId
          ? threadTokenSummary.totalTokens
          : 0,
      }
      renderThreadTokenCounter()
    }
  } finally {
    threadTokenRefreshInFlight = false
    if (threadTokenRefreshQueued) {
      threadTokenRefreshQueued = false
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
  composerState.doc.attachments.length > 0 ||
  (lastTurnFailed && lastSubmittedDraft.trim() !== "")

const renderMessages = (): void => {
  const stickToEnd = transcriptPinnedToEnd && isNearTranscriptEnd()
  const previousScrollTop = messageList.scrollTop
  const thinking = renderThinkingIndicator()
  messageList.replaceChildren(
    ...messages.map(renderMessage),
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
    composerState.doc.attachments.map((attachment) => ({
      id: attachment.id,
      kind:
        attachment.kind === "snippet"
          ? "code"
          : attachment.kind === "file"
            ? "file"
            : attachment.kind,
      status: attachment.status,
      selected:
        composerState.selection.selectedAttachmentId === attachment.id,
    }))
  composerHudRuntime.handle.setProjection({
    focused: document.activeElement === composerInput,
    dragActive,
    reducedMotion: prefersReducedMotion?.matches === true,
    attachments,
    dropcursor: {
      visible: dragActive,
      x: 0,
      intensity: dragActive ? 0.9 : 0,
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
  const result = applyComposerTransaction(composerState, transaction)
  if (!result.ok) {
    lastTurnFailed = true
    renderComposer()
    return false
  }
  composerState = result.state
  lastTurnFailed = false
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
  composerAttachmentReceipts = [...composerAttachmentReceipts, receipt]
}

const attachmentById = (attachmentId: string): ComposerAttachment | undefined =>
  composerState.doc.attachments.find(attachment => attachment.id === attachmentId)

const failLocalAttachmentUpload = (
  attachmentId: string,
  errorText: string,
): void => {
  const transaction = setComposerAttachmentStatusTransaction(
    composerState,
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
    composerState,
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
      composerState,
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
  const attachment = composerState.doc.attachments.find(
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
    composerState,
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
    composerState,
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
  const attachments = composerState.doc.attachments.map(attachmentPropsFor)
  composerRail.hidden = attachments.length === 0 && !dragActive
  composerRail.dataset.oaCommandComposerDragActive = dragActive ? "true" : "false"
  composerRail.replaceChildren(
    ...attachments.map(renderAttachment),
    ...(dragActive
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
  `${slashCommandPlatform()}:${pendingTurn ? "active" : "idle"}`

let loadedSlashCommandKey = ""

const ensureSlashCommandsLoaded = (): void => {
  const key = slashCommandLoadKey()
  if (loadedSlashCommandKey === key && slashCommands.length > 0) return
  if (slashCommandLoadInFlight !== null) return
  slashCommandLoadInFlight = rpc.request.slashCommandList({
    activeTurn: pendingTurn,
    platform: slashCommandPlatform(),
    sideConversation: false,
  }).then(response => {
    slashCommands = [...response.commands]
    loadedSlashCommandKey = key
  }).catch(() => {
    slashCommands = []
    loadedSlashCommandKey = key
  }).finally(() => {
    slashCommandLoadInFlight = null
    renderSlashCommandPalette()
  })
}

const slashCommandQuery = (): string | null => {
  const value = composerInput.value.trimStart()
  if (!value.startsWith("/")) return null
  return value.slice(1).split(/\s+/)[0]?.toLowerCase() ?? ""
}

const matchingSlashCommands = (): readonly SlashCommandEntry[] => {
  const query = slashCommandQuery()
  if (query === null) return []
  return slashCommands.filter(command =>
    command.command.includes(query) ||
    command.aliases.some(alias => alias.includes(query))
  ).slice(0, 8)
}

const selectSlashCommand = (command: SlashCommandEntry): void => {
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
  const attachmentCount = composerState.doc.attachments.length

  composerForm.dataset.oaCommandComposerStatus = status
  composerFrame.dataset.oaCommandComposerFrame = ""
  sendButton.disabled = !pendingTurn && !canSubmitComposer()
  sendButton.type = pendingTurn ? "button" : "submit"
  sendButton.title = sendLabel
  sendButton.setAttribute("aria-label", `${sendLabel} message`)
  sendButton.dataset.oaCommandComposerSubmit = pendingTurn ? "stop" : "send"
  sendButton.dataset.status = status
  setButtonIcon(sendButton, pendingTurn ? "stop" : "send")
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
  messages = [...messages, message]
  render()
  return message
}

const appendMessages = (nextMessages: readonly KhalaCodeDesktopMessage[]): void => {
  if (nextMessages.length === 0) return
  const merged = [...messages]
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
  messages = merged
  render()
}

function applyChatTurnEvent(event: KhalaCodeDesktopChatTurnEvent): void {
  if (!activeTurnIds.has(event.turnId)) return
  if (event.type === "thread_ready") {
    setActiveCodexThreadId(event.threadId)
    void refreshThreadTokenSummary()
    return
  }
  if (
    event.type === "message_start" ||
    event.type === "message_delta" ||
    event.type === "message_replace"
  ) {
    thinkingTurnId = null
  }
  switch (event.type) {
    case "message_start":
      pendingTurn = true
      appendMessages([event.message])
      break
    case "message_delta":
      messages = messages.map(message =>
        message.id === event.messageId
          ? { ...message, body: `${message.body}${event.delta}` }
          : message
      )
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
  composerState = emptyComposerState()
  renderComposer()
}

const stopActiveTurn = (): void => {
  if (!pendingTurn) return
  const stoppedTurnIds = [...activeTurnIds]
  for (const turnId of stoppedTurnIds) {
    void rpc.request.codexTurnInterrupt({ sessionId, turnId }).catch(() => undefined)
  }
  activeTurnIds.clear()
  pendingTurn = false
  thinkingTurnId = null
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
      messages = []
      activeTurnIds.clear()
      render()
      return "Cleared the visible transcript."
    case "copy_last_assistant_message": {
      const assistant = [...messages].reverse().find(message => message.role === "assistant")
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
  lastSubmittedDraft = draftText
  lastTurnFailed = false
  resetComposerDraft()
  const message = addMessage("user", body)
  try {
    const result = await rpc.request.slashCommandDispatch({
      activeTurn: pendingTurn,
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
  if (pendingTurn) {
    stopActiveTurn()
    return null
  }
  const draftText =
    composerInput.value.trim() === "" && lastTurnFailed
      ? lastSubmittedDraft
      : composerInput.value.trim()
  if (draftText === "" && composerState.doc.attachments.length === 0) return null

  const attachments = [...composerState.doc.attachments]
  const body = submittedBody(draftText, attachments)
  if (draftText.startsWith("/")) {
    return submitSlashCommand(draftText, body)
  }
  const imageAttachments = await imageAttachmentsForSubmit(attachments)
  lastSubmittedDraft = draftText
  lastTurnFailed = false
  resetComposerDraft()
  const message = addMessage("user", body)
  const turnId = nextTurnId()
  activeTurnIds.add(turnId)
  pendingTurn = true
  thinkingTurnId = turnId
  render()
  requestAnimationFrame(focusComposerInput)
  try {
    const request: KhalaCodeDesktopChatTurnRequest = {
      ...(imageAttachments.length === 0 ? {} : { attachments: imageAttachments }),
      messages,
      sessionId,
      ...(activeCodexThreadId === null ? { startNewThread: true } : { threadId: activeCodexThreadId }),
      turnId,
    }
    const response = await rpc.request.submitChatMessage(request)
    if (activeTurnIds.has(turnId)) {
      if (response.backend.threadId !== undefined) {
        setActiveCodexThreadId(response.backend.threadId)
        void threadSidebar?.refresh()
      }
      if (thinkingTurnId === turnId) thinkingTurnId = null
      appendMessages(response.messages)
    }
  } catch (error) {
    if (activeTurnIds.has(turnId)) {
      lastTurnFailed = true
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
    if (thinkingTurnId === turnId) thinkingTurnId = null
    pendingTurn = activeTurnIds.size > 0
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
  dragActive = active
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
  if (!pendingTurn) return
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
  lastTurnFailed = false
  renderComposer()
})

composerInput.addEventListener("focus", updateComposerHudProjection)
composerInput.addEventListener("blur", updateComposerHudProjection)

composerInput.addEventListener("keydown", event => {
  if (event.metaKey || event.ctrlKey || event.altKey) return
  if (
    event.key === "Enter" &&
    !event.shiftKey &&
    !pendingTurn &&
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
  transcriptPinnedToEnd = isNearTranscriptEnd()
}, { passive: true })
threadTokenCounter.addEventListener("click", () => {
  threadTokenPopoverOpen = !threadTokenPopoverOpen
  renderThreadTokenCounter()
  if (threadTokenPopoverOpen) void refreshThreadTokenSummary()
})
document.addEventListener("click", event => {
  if (!threadTokenPopoverOpen) return
  const target = event.target
  if (target instanceof Node && threadTokenMeter.contains(target)) return
  threadTokenPopoverOpen = false
  renderThreadTokenCounter()
})
window.addEventListener("wheel", proxyTranscriptWheel, { passive: false })
window.addEventListener("keydown", event => {
  if (event.key === "Escape" && threadTokenPopoverOpen) {
    threadTokenPopoverOpen = false
    renderThreadTokenCounter()
    return
  }
  proxyTranscriptKeyScroll(event)
})
window.setInterval(() => {
  if (document.hidden || activeCodexThreadId === null) return
  void refreshThreadTokenSummary()
}, 2_000)
window.addEventListener("beforeunload", () => {
  for (const url of objectUrls) URL.revokeObjectURL(url)
  composerHudRuntime?.dispose()
})

const controls = {
  addMessage,
  appInfo: () => rpc.request.appInfo(),
  attachments: () => composerState.doc.attachments.map(attachment => ({ ...attachment })),
  attachmentReceipts: () => composerAttachmentReceipts.map(receipt => ({ ...receipt })),
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
  consumeCodexRateLimitResetCredit: () =>
    rpc.request.consumeCodexRateLimitResetCredit(),
  focusComposer: focusComposerInput,
  isComposerFocused: () => document.activeElement === composerInput,
  isPending: () => pendingTurn,
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
  messages: () => messages.map(message => ({ ...message })),
  pylonStatus: () => rpc.request.pylonStatus(),
  removeCodexAccount: (accountRef: string) =>
    rpc.request.removeCodexAccount(accountRef),
  slashCommandDispatch: (request: Parameters<DesktopRpcRequests["slashCommandDispatch"]>[0]) =>
    rpc.request.slashCommandDispatch(request),
  slashCommandList: (request?: Parameters<DesktopRpcRequests["slashCommandList"]>[0]) =>
    rpc.request.slashCommandList(request),
  reset: () => {
    messages = []
    activeTurnIds.clear()
    resetComposerDraft()
    pendingTurn = false
    thinkingTurnId = null
    lastTurnFailed = false
    render()
  },
  setComposerDraft: (value: string) => {
    composerInput.value = value
    lastTurnFailed = false
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
const gymPanelEl = document.getElementById("gym-panel")
const settingsPanelEl = document.getElementById("settings-panel")
const threadShell = document.querySelector<HTMLElement>(".khala-code-thread-shell")
const composerDock = document.querySelector<HTMLElement>(".composer-dock")
const initialGymState = gymPaneStateFromLocation(globalThis.location)
const initialView = initialKhalaCodeViewFromLocation(globalThis.location)

const setActiveCodexThreadId = (threadId: string | null): void => {
  const changed = activeCodexThreadId !== threadId
  activeCodexThreadId = threadId
  if (threadId === null) {
    localStorage.removeItem(activeThreadIdStorageKey)
  } else {
    localStorage.setItem(activeThreadIdStorageKey, threadId)
  }
  if (changed) {
    threadTokenSummary = emptyThreadTokenSummary(threadId)
    renderThreadTokenCounter()
    void refreshThreadTokenSummary()
  }
  threadSidebar?.setActiveThreadId(threadId)
}

const activateCodexThread = (input: {
  readonly messages: readonly KhalaCodeDesktopMessage[]
  readonly threadId: string
}): void => {
  setActiveCodexThreadId(input.threadId)
  messages = [...input.messages]
  activeTurnIds.clear()
  pendingTurn = false
  thinkingTurnId = null
  lastTurnFailed = false
  render()
  requestAnimationFrame(focusComposerInput)
}

const beginNewCodexThread = (): void => {
  setActiveCodexThreadId(null)
  messages = []
  activeTurnIds.clear()
  pendingTurn = false
  thinkingTurnId = null
  lastTurnFailed = false
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
        loadGymDemoProof: () => loadGymDemoOptimization(),
        startDelegationOptimization: async () => loadGymDemoOptimization(),
        fetch: () => controls.codexFleetStatus(),
        removeAccount: accountRef => controls.removeCodexAccount(accountRef),
        connectAccount: accountRef => controls.connectCodexAccount(accountRef),
        openExternal: url => controls.openExternalUrl(url),
      })

const gymPanel =
  gymPanelEl === null ? null : mountGymPane(gymPanelEl, initialGymState)

const settingsPanel =
  settingsPanelEl === null
    ? null
    : mountCodexSettingsPanel(settingsPanelEl, {
        fetch: () => controls.codexSettingsRead({ includeHiddenModels: true }),
        write: request => controls.codexConfigValueWrite(request),
      })

const threadSidebar =
  threadSidebarEl === null
    ? null
    : mountCodexThreadSidebar(threadSidebarEl, {
        activeThreadId: () => activeCodexThreadId,
        archiveThread: threadId => controls.codexThreadArchive({ threadId }),
        deleteThread: threadId => controls.codexThreadDelete({ threadId }),
        forkThread: threadId => controls.codexThreadFork({ sessionId, threadId }),
        listThreads: request => controls.codexThreadList({
          archived: request.archived,
          limit: 50,
          searchTerm: request.searchTerm,
          sessionId,
          useStateDbOnly: true,
        }),
        renameThread: (threadId, name) => controls.codexThreadRename({ name, threadId }),
        resumeThread: threadId => controls.codexThreadResume({ sessionId, threadId }),
        sessionId,
        unarchiveThread: threadId => controls.codexThreadUnarchive({ threadId }),
        onNewThreadRequested: beginNewCodexThread,
        onThreadSelected: activateCodexThread,
      })

window.addEventListener("keydown", event => {
  const recentThreadIndex = recentThreadHotkeyIndexForEvent(event)
  if (recentThreadIndex === null) return
  if (threadSidebar === null) return

  event.preventDefault()
  void threadSidebar.selectRecentThread(recentThreadIndex).then(selected => {
    if (selected) setActiveView("chat")
  })
})

const setActiveView = (value: string): void => {
  const activeValue = value === "fleet" || value === "settings" ? value : "chat"
  const showChat = activeValue === "chat"
  const showFleet = activeValue === "fleet"
  const showSettings = activeValue === "settings"
  if (threadSidebarEl !== null) threadSidebarEl.hidden = !showChat
  if (fleetPanelEl !== null) fleetPanelEl.hidden = !showFleet
  if (settingsPanelEl !== null) settingsPanelEl.hidden = !showSettings
  gymPanel?.setVisible(false)
  if (threadShell !== null) threadShell.hidden = showFleet || showSettings
  if (composerDock !== null) composerDock.hidden = showFleet || showSettings
  fleetPanel?.setVisible(showFleet)
  settingsPanel?.setVisible(showSettings)
  threadSidebar?.setVisible(showChat)
  if (showChat) {
    requestAnimationFrame(focusComposerInput)
  }
}

function showGymProofPane(): void {
  if (threadSidebarEl !== null) threadSidebarEl.hidden = true
  if (fleetPanelEl !== null) fleetPanelEl.hidden = true
  if (settingsPanelEl !== null) settingsPanelEl.hidden = true
  if (threadShell !== null) threadShell.hidden = true
  if (composerDock !== null) composerDock.hidden = true
  gymPanel?.setVisible(true)
  fleetPanel?.setVisible(false)
  settingsPanel?.setVisible(false)
  threadSidebar?.setVisible(false)
}

if (sidebarNavRoot !== null) {
  mountKhalaCodeSidebar(sidebarNavRoot, {
    selectedValue: initialView,
    onActivate: value => setActiveView(value),
  })
}
setActiveView(initialView)
renderThreadTokenCounter()
void refreshThreadTokenSummary()
render()
requestAnimationFrame(focusComposerInput)
