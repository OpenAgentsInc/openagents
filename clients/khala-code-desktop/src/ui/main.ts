import { Electroview } from "electrobun/view"

import {
  KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaCodeDesktopChatTurnRequest,
  type KhalaCodeDesktopMessage,
  type KhalaCodeDesktopMessageRole,
  type KhalaCodeDesktopRPCSchema,
} from "../shared/rpc"
import { renderMessageBody } from "./transcript-render"
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
})

const nativeRpc = Electroview.defineRPC<KhalaCodeDesktopRPCSchema>({
  maxRequestTime: KHALA_CODE_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {},
    messages: {},
  },
})

const electrobunGlobals = globalThis as typeof globalThis & {
  readonly __electrobun?: unknown
  readonly __electrobunRpcSocketPort?: unknown
  readonly __electrobunWebviewId?: unknown
}

const hasElectrobunBridge =
  electrobunGlobals.__electrobun !== undefined &&
  electrobunGlobals.__electrobunRpcSocketPort !== undefined &&
  electrobunGlobals.__electrobunWebviewId !== undefined

const rpc = hasElectrobunBridge ? nativeRpc : previewRpc()
if (hasElectrobunBridge) {
  new Electroview({ rpc: nativeRpc })
}

const initialMessages: readonly KhalaCodeDesktopMessage[] = [
  {
    id: "assistant-wake",
    role: "assistant",
    body:
      "Khala Code is awake. Point me at a repo, and I will keep the patch small enough to understand.",
  },
  {
    id: "user-fixture",
    role: "user",
    body: "Start with a tiny TypeScript helper and show me the shape before wiring the worker.",
  },
  {
    id: "assistant-code",
    role: "assistant",
    body:
      "Here is the helper I would land first:\n\n```ts\nexport type QueueItem = Readonly<{\n  id: string\n  title: string\n  priority: \"low\" | \"normal\" | \"high\"\n}>\n\nexport const describeItem = (item: QueueItem): string =>\n  `${item.priority}: ${item.title}`\n```\n\nThe important bit is that the queue item stays typed before it reaches any UI surface.",
  },
  {
    id: "assistant-diff",
    role: "assistant",
    body:
      "And the patch would stay this small:\n\n```diff\ndiff --git a/src/queue.ts b/src/queue.ts\nindex 91c5b8d..6df02d1 100644\n--- a/src/queue.ts\n+++ b/src/queue.ts\n@@ -1,5 +1,9 @@\n export type QueueItem = Readonly<{\n   id: string\n   title: string\n+  priority: \"low\" | \"normal\" | \"high\"\n }>\n+\n+export const describeItem = (item: QueueItem): string =>\n+  `${item.priority}: ${item.title}`\n```\n",
  },
]

const requireElement = <T extends Element>(selector: string): T => {
  const element = document.querySelector<T>(selector)
  if (element === null) throw new Error(`Missing ${selector}`)
  return element
}

const messageList = requireElement<HTMLElement>("#message-list")
const composerForm = requireElement<HTMLFormElement>("#composer-form")
const composerInput = requireElement<HTMLTextAreaElement>("#composer-input")
const sendButton = requireElement<HTMLButtonElement>("#send-button")

let messages: KhalaCodeDesktopMessage[] = [...initialMessages]
let pendingTurn = false
const sessionId = `khala-code-desktop-${Date.now().toString(36)}`

const roleLabel = (role: KhalaCodeDesktopMessageRole): string => {
  if (role === "user") return "You"
  if (role === "tool") return "Tool"
  if (role === "system") return "System"
  return "Khala Code"
}

const messageClass = (role: KhalaCodeDesktopMessageRole): string =>
  `message-bubble message-bubble--${role}`

const renderMessage = (message: KhalaCodeDesktopMessage): HTMLElement => {
  const article = document.createElement("article")
  article.className = messageClass(message.role)
  article.dataset.messageId = message.id

  const label = document.createElement("div")
  label.className = "message-label"
  label.textContent = roleLabel(message.role)

  const body = document.createElement("div")
  body.className = "message-body"
  body.append(...renderMessageBody(message.body))

  article.append(label, body)
  return article
}

const scrollToEnd = (): void => {
  messageList.scrollTo({
    top: messageList.scrollHeight,
    behavior: "smooth",
  })
}

const render = (): void => {
  messageList.replaceChildren(...messages.map(renderMessage))
  sendButton.disabled = pendingTurn || composerInput.value.trim() === ""
  composerInput.disabled = pendingTurn
  composerForm.dataset.pending = pendingTurn ? "true" : "false"
  requestAnimationFrame(scrollToEnd)
}

const focusComposerInput = (): void => {
  if (!pendingTurn) composerInput.focus({ preventScroll: true })
}

const nextMessageId = (role: KhalaCodeDesktopMessageRole): string =>
  `${role}-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`

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
  messages = [...messages, ...nextMessages]
  render()
}

const submitComposer = async (): Promise<KhalaCodeDesktopMessage | null> => {
  if (pendingTurn) return null
  const text = composerInput.value.trim()
  if (text === "") return null
  composerInput.value = ""
  const message = addMessage("user", text)
  pendingTurn = true
  render()
  try {
    const request: KhalaCodeDesktopChatTurnRequest = {
      messages,
      sessionId,
    }
    const response = await rpc.request.submitChatMessage(request)
    appendMessages(response.messages)
  } catch (error) {
    appendMessages([
      {
        body: `Khala Code turn failed: ${error instanceof Error ? error.message : String(error)}`,
        id: nextMessageId("system"),
        role: "system",
      },
    ])
  } finally {
    pendingTurn = false
    render()
  }
  return message
}

composerForm.addEventListener("submit", event => {
  event.preventDefault()
  void submitComposer().finally(() => composerInput.focus())
})

composerInput.addEventListener("input", () => {
  sendButton.disabled = pendingTurn || composerInput.value.trim() === ""
})

composerInput.addEventListener("keydown", event => {
  if (event.key === "Enter" && !event.shiftKey && composerInput.value.trim() !== "" && !pendingTurn) {
    event.preventDefault()
    void submitComposer()
  }
})

const controls = {
  addMessage,
  appInfo: () => rpc.request.appInfo(),
  codexAccountsStatus: () => rpc.request.codexAccountsStatus(),
  codingStatus: () => rpc.request.codingStatus(),
  consumeCodexRateLimitResetCredit: () =>
    rpc.request.consumeCodexRateLimitResetCredit(),
  isPending: () => pendingTurn,
  messages: () => messages.map(message => ({ ...message })),
  pylonStatus: () => rpc.request.pylonStatus(),
  reset: () => {
    messages = [...initialMessages]
    composerInput.value = ""
    pendingTurn = false
    render()
  },
  setComposerDraft: (value: string) => {
    composerInput.value = value
    sendButton.disabled = composerInput.value.trim() === ""
  },
  submitComposer,
  tokenAccountingStatus: () => rpc.request.tokenAccountingStatus(),
  toolCatalog: () => rpc.request.toolCatalog(),
}

Object.assign(globalThis, {
  khalaCodeDesktop: controls,
})

void controls.appInfo().catch(() => undefined)
render()
requestAnimationFrame(focusComposerInput)
