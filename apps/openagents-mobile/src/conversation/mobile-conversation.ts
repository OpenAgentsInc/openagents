import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

export type MobileConversationMessage = ConfirmedChatMessage
export type MobileConversationThreadSummary = ConfirmedChatThread

export type MobileConversationThread = MobileConversationThreadSummary & Readonly<{
  messages: ReadonlyArray<MobileConversationMessage>
}>

export type MobileConversationMutationResult =
  | Readonly<{ ok: true; thread: MobileConversationThread }>
  | Readonly<{ ok: false; error: string }>

export type MobileConversationHost = Readonly<{
  listThreads: () => Promise<ReadonlyArray<MobileConversationThreadSummary>>
  newThread: () => Promise<MobileConversationMutationResult>
  openThread: (threadRef: string) => Promise<MobileConversationThread | null>
  sendMessage: (input: Readonly<{ threadRef: string; body: string }>) => Promise<MobileConversationMutationResult>
}>

export type MobileConversationSelection =
  | Readonly<{ mode: "local" }>
  | Readonly<{
      mode: "sync"
      host: MobileConversationHost
      threads: ReadonlyArray<MobileConversationThreadSummary>
      activeThread: MobileConversationThread | null
    }>

export type MobileConversationAdapterOptions = Readonly<{
  conversation: KhalaSyncConversation
  randomId?: () => string
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
}>

const nextRef = (kind: "thread" | "message", randomId: () => string): string =>
  `${kind}.mobile.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`

const run = <Value, Error>(effect: Effect.Effect<Value, Error>): Promise<Value> =>
  Effect.runPromise(effect)

export const makeMobileConversationHost = (
  options: MobileConversationAdapterOptions,
): MobileConversationHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const sleep = options.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = options.pollAttempts ?? 30

  const listThreads = async (): Promise<ReadonlyArray<MobileConversationThreadSummary>> =>
    options.conversation.personalStatus().phase === "live"
      ? run(options.conversation.listConfirmedThreads())
      : []

  const confirmedThread = async (
    threadRef: string,
    requiredMessageRef?: string,
  ): Promise<MobileConversationThread | null> => {
    try {
      await run(options.conversation.openThread(threadRef))
    } catch {
      return null
    }
    for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
      if (
        options.conversation.personalStatus().phase === "live" &&
        options.conversation.threadStatus(threadRef).phase === "live"
      ) {
        try {
          const [threads, messages] = await Promise.all([
            run(options.conversation.listConfirmedThreads()),
            run(options.conversation.listConfirmedMessages(threadRef)),
          ])
          const summary = threads.find(thread => thread.threadRef === threadRef)
          if (
            summary !== undefined &&
            (requiredMessageRef === undefined || messages.some(message => message.messageRef === requiredMessageRef))
          ) return { ...summary, messages }
        } catch {
          // A transient read is retried while the exact scope remains live.
        }
      }
      await sleep(100)
    }
    return null
  }

  return {
    listThreads,
    openThread: confirmedThread,
    newThread: async () => {
      const threadRef = nextRef("thread", randomId)
      try {
        await run(options.conversation.createThread({
          threadId: threadRef,
          title: "New chat",
        }))
      } catch {
        return { ok: false, error: "Authoritative conversation Sync is unavailable." }
      }
      const thread = await confirmedThread(threadRef)
      return thread === null
        ? { ok: false, error: "New chat is still pending reconciliation." }
        : { ok: true, thread }
    },
    sendMessage: async input => {
      const messageRef = nextRef("message", randomId)
      try {
        await run(options.conversation.appendMessage({
          threadId: input.threadRef,
          messageId: messageRef,
          body: input.body,
        }))
      } catch {
        return { ok: false, error: "Authoritative conversation Sync is unavailable." }
      }
      const thread = await confirmedThread(input.threadRef, messageRef)
      return thread === null
        ? { ok: false, error: "Message is still pending reconciliation." }
        : { ok: true, thread }
    },
  }
}

export const selectMobileConversation = async (input: Readonly<{
  conversation: () => KhalaSyncConversation | null
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
  adapter?: Omit<MobileConversationAdapterOptions, "conversation">
}>): Promise<MobileConversationSelection> => {
  const sleep = input.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = input.pollAttempts ?? 30

  for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
    const conversation = input.conversation()
    if (conversation !== null && conversation.personalStatus().phase === "live") {
      const host = makeMobileConversationHost({ conversation, ...input.adapter })
      try {
        const threads = await host.listThreads()
        const activeThread = threads[0] === undefined
          ? null
          : await host.openThread(threads[0].threadRef)
        if (threads.length === 0 || activeThread !== null) {
          return { mode: "sync", host, threads, activeThread }
        }
      } catch {
        return { mode: "local" }
      }
      return { mode: "local" }
    }
    await sleep(100)
  }
  return { mode: "local" }
}
