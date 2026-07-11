import type { DesktopThread } from "../chat-contract.ts"
import type { DesktopRuntimeGatewayResponse } from "../runtime-gateway-contract.ts"
import type { ChatHost } from "./shell.ts"

export type RuntimeConversationRequest = (
  value: unknown,
) => Promise<DesktopRuntimeGatewayResponse>

export type RuntimeConversationOptions = Readonly<{
  request: RuntimeConversationRequest
  randomId?: () => string
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
}>

let requestSequence = 0

const timestamp = (value: string): string => {
  const date = new Date(value)
  return Number.isFinite(date.getTime())
    ? date.toISOString().slice(11, 16)
    : "--:--"
}

const threadSummary = (thread: Readonly<{
  threadRef: string
  title: string
  updatedAt: string
}>): DesktopThread => ({
  id: thread.threadRef,
  title: thread.title,
  updatedAt: thread.updatedAt,
  notes: [],
})

const nextId = (kind: "thread" | "message", randomId: () => string): string =>
  `${kind}.desktop.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`

export const makeRuntimeConversationChatHost = (
  options: RuntimeConversationOptions,
): ChatHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const sleep = options.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = options.pollAttempts ?? 30

  const catalog = () => options.request({
    kind: "query",
    requestId: `renderer-conversation-catalog-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })

  const confirmedThread = async (
    threadRef: string,
    requiredMessageRef?: string,
  ): Promise<DesktopThread | null> => {
    for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
      const [catalogResult, threadResult] = await Promise.all([
        catalog(),
        options.request({
          kind: "query",
          requestId: `renderer-conversation-thread-${++requestSequence}`,
          query: { id: "conversation.thread", threadRef },
        }),
      ])
      if (
        catalogResult.kind === "conversation_catalog" &&
        catalogResult.status.phase === "live" &&
        threadResult.kind === "conversation_thread" &&
        threadResult.status.phase === "live"
      ) {
        const summary = catalogResult.threads.find(thread => thread.threadRef === threadRef)
        if (summary !== undefined) {
          const projected: DesktopThread = {
            ...threadSummary(summary),
            notes: threadResult.messages.map(message => ({
              key: message.messageRef,
              role: "user" as const,
              text: message.body,
              timestamp: timestamp(message.createdAt),
            })),
          }
          if (
            requiredMessageRef === undefined ||
            projected.notes.some(note => note.key === requiredMessageRef)
          ) return projected
        }
      }
      await sleep(100)
    }
    return null
  }

  return {
    listThreads: async () => {
      const result = await catalog()
      return result.kind === "conversation_catalog" && result.status.phase === "live"
        ? result.threads.map(threadSummary)
        : []
    },
    newThread: async () => {
      const threadRef = nextId("thread", randomId)
      const outcome = await options.request({
        kind: "command",
        commandId: `renderer-conversation-create-${++requestSequence}`,
        command: { id: "conversation.create", threadRef, title: "New chat" },
      })
      if (
        outcome.kind !== "conversation_mutation_outcome" ||
        outcome.status !== "pending_reconcile"
      ) return null
      return confirmedThread(threadRef)
    },
    openThread: confirmedThread,
    hydrateThread: confirmedThread,
    sendMessage: async input => {
      const messageRef = nextId("message", randomId)
      const outcome = await options.request({
        kind: "command",
        commandId: `renderer-conversation-append-${++requestSequence}`,
        command: {
          id: "conversation.append",
          threadRef: input.id,
          messageRef,
          body: input.message,
        },
      })
      if (
        outcome.kind !== "conversation_mutation_outcome" ||
        outcome.status !== "pending_reconcile"
      ) return { ok: false, error: "Authoritative conversation Sync is unavailable." }

      const thread = await confirmedThread(input.id, messageRef)
      if (thread !== null) return { ok: true, thread }
      return {
        ok: false,
        error: "Message is still pending reconciliation.",
      }
    },
  }
}

export const selectDesktopChatHost = async (input: Readonly<{
  request: RuntimeConversationRequest | undefined
  local: ChatHost
  options?: Omit<RuntimeConversationOptions, "request">
}>): Promise<ChatHost> => {
  if (input.request === undefined) return input.local
  const result = await input.request({
    kind: "query",
    requestId: `renderer-conversation-mode-${++requestSequence}`,
    query: { id: "conversation.catalog" },
  })
  return result.kind === "conversation_catalog" && result.status.phase === "live"
    ? makeRuntimeConversationChatHost({ request: input.request, ...input.options })
    : input.local
}
