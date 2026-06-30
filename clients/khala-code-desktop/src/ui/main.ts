import {
  applyComposerTransaction,
  composerAttachmentId,
  emptyComposerState,
  offerComposerLargeTextPaste,
  retryComposerAttachmentTransaction,
  setComposerAttachmentStatusTransaction,
  stageComposerAttachmentFiles,
  stageComposerDroppedFiles,
  stageComposerPastedFiles,
  type ComposerAttachment,
  type ComposerAttachmentSource,
  type ComposerFileLike,
  type ComposerState,
  type ComposerTransaction,
} from "@openagentsinc/composer-state"
import {
  commandComposerClassName,
  type CommandComposerAttachmentProps,
  type CommandComposerStatus,
} from "@openagentsinc/ui/ai-elements/command-composer"
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
  type KhalaCodeDesktopChatTurnEvent,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopMessageRole,
  type KhalaCodeDesktopRPCSchema,
} from "../shared/rpc"
import { renderMessageBody } from "./transcript-render"
import { mountKhalaCodeSidebar } from "./sidebar"
import "./styles.css"

type DesktopRpc = ReturnType<typeof Electroview.defineRPC<KhalaCodeDesktopRPCSchema>>
type DesktopRpcRequests = KhalaCodeDesktopRPCSchema["requests"]

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
    codingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["codingStatus"]>>
      >("codingStatus"),
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
    submitChatMessage: request =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["submitChatMessage"]>>
      >("submitChatMessage", request),
    tokenAccountingStatus: () =>
      postPreviewRpc<
        Awaited<ReturnType<DesktopRpcRequests["tokenAccountingStatus"]>>
      >("tokenAccountingStatus"),
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

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector)
  if (element === null) throw new Error(`Missing ${selector}`)
  return element
}

const messageList = requireElement<HTMLElement>("#message-list")
const composerForm = requireElement<HTMLFormElement>("#composer-form")
const composerFrame = requireElement<HTMLElement>("#composer-frame")
const composerHudMount = requireElement<HTMLElement>("#composer-hud")
const composerInput = requireElement<HTMLTextAreaElement>("#composer-input")
const composerRail = requireElement<HTMLElement>("#composer-rail")
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
const activeTurnIds = new Set<string>()
const objectUrls = new Set<string>()
const localTextAttachments = new Map<string, string>()
const sessionId = `khala-code-desktop-${Date.now().toString(36)}`

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

  const body = document.createElement("div")
  body.className = "message-body"
  body.append(...renderMessageBody(message.body, message.role))

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
  button.textContent = label
  button.title = label
  button.setAttribute("aria-label", `${label} attachment: ${attachment.name}`)
  button.dataset.oaCommandComposerAttachmentAction = action
  button.dataset.attachmentId = attachment.id
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
  const transaction = retryComposerAttachmentTransaction(
    composerState,
    composerAttachmentId(attachmentId),
  )
  if (transaction !== null) {
    applyComposerStateTransaction(transaction)
    return
  }
  const fallback = setComposerAttachmentStatusTransaction(
    composerState,
    composerAttachmentId(attachmentId),
    { status: "staged", errorText: null },
  )
  if (fallback !== null) applyComposerStateTransaction(fallback)
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
  icon.textContent =
    attachment.kind === "image"
      ? "[]"
      : attachment.kind === "text"
        ? "T"
        : attachment.kind === "snippet"
          ? "{}"
          : "#"

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
  sendButton.querySelector(".oa-ai-command-composer-icon")!.textContent =
    pendingTurn ? "x" : "^"
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

const resetComposerDraft = (): void => {
  composerInput.value = ""
  composerState = emptyComposerState()
  renderComposer()
}

const stopActiveTurn = (): void => {
  if (!pendingTurn) return
  activeTurnIds.clear()
  pendingTurn = false
  thinkingTurnId = null
  appendMessages([
    {
      body: "Stopped the active turn locally. You can keep typing.",
      id: nextMessageId("system"),
      role: "system",
    },
  ])
  requestAnimationFrame(focusComposerInput)
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
      messages,
      sessionId,
      turnId,
    }
    const response = await rpc.request.submitChatMessage(request)
    if (activeTurnIds.has(turnId)) {
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
  applyComposerStateTransaction(staged.transaction)
}

const stageLargeTextPaste = (text: string): boolean => {
  const offer = offerComposerLargeTextPaste(text)
  if (!offer.offered || offer.transaction === null || offer.attachment === undefined) {
    return false
  }
  if (offer.attachment.contentRef !== undefined) {
    localTextAttachments.set(offer.attachment.contentRef, text)
  }
  return applyComposerStateTransaction(offer.transaction)
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
messageList.addEventListener("scroll", () => {
  transcriptPinnedToEnd = isNearTranscriptEnd()
}, { passive: true })
window.addEventListener("wheel", proxyTranscriptWheel, { passive: false })
window.addEventListener("keydown", proxyTranscriptKeyScroll)
window.addEventListener("beforeunload", () => {
  for (const url of objectUrls) URL.revokeObjectURL(url)
  composerHudRuntime?.dispose()
})

const controls = {
  addMessage,
  appInfo: () => rpc.request.appInfo(),
  attachments: () => composerState.doc.attachments.map(attachment => ({ ...attachment })),
  codexAccountsStatus: () => rpc.request.codexAccountsStatus(),
  codingStatus: () => rpc.request.codingStatus(),
  composerStatus: statusForComposer,
  consumeCodexRateLimitResetCredit: () =>
    rpc.request.consumeCodexRateLimitResetCredit(),
  focusComposer: focusComposerInput,
  isComposerFocused: () => document.activeElement === composerInput,
  isPending: () => pendingTurn,
  messages: () => messages.map(message => ({ ...message })),
  pylonStatus: () => rpc.request.pylonStatus(),
  reset: () => {
    messages = []
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
  toolCatalog: () => rpc.request.toolCatalog(),
}

Object.assign(globalThis, {
  khalaCodeDesktop: controls,
})

void controls.appInfo().catch(() => undefined)
mountComposerHud()

const sidebarRoot = document.getElementById("sidebar-root")
if (sidebarRoot !== null) {
  mountKhalaCodeSidebar(sidebarRoot, {
    selectedValue: "chat",
    onActivate: () => {},
  })
}
render()
requestAnimationFrame(focusComposerInput)
