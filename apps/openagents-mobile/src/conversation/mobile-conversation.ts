import type {
  ConfirmedAgentTimelineSnapshot,
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncAgentTimeline,
  KhalaSyncConversation,
  KhalaSyncRuntimeCommands,
} from "@openagentsinc/khala-sync-client"
import {
  buildAppendUserMessageIntent,
  buildInterruptTurnIntent,
  buildStartTurnIntent,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

export type MobileConversationMessage = ConfirmedChatMessage
export type MobileConversationThreadSummary = ConfirmedChatThread

export type MobileConversationThread = MobileConversationThreadSummary & Readonly<{
  messages: ReadonlyArray<MobileConversationMessage>
  timeline?: ConfirmedAgentTimelineSnapshot | null
}>

export type MobileConversationMutationResult =
  | Readonly<{ ok: true; thread: MobileConversationThread }>
  | Readonly<{ ok: false; error: string }>

export type MobileConversationHost = Readonly<{
  listThreads: () => Promise<ReadonlyArray<MobileConversationThreadSummary>>
  newThread: () => Promise<MobileConversationMutationResult>
  openThread: (threadRef: string) => Promise<MobileConversationThread | null>
  sendMessage: (input: Readonly<{
    threadRef: string
    body: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>) => Promise<MobileConversationMutationResult>
  interrupt?: (input: Readonly<{
    threadRef: string
    runRef: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>) => Promise<MobileConversationMutationResult>
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
  timeline?: KhalaSyncAgentTimeline
  runtime?: KhalaSyncRuntimeCommands
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
          ) {
            const timeline = options.timeline === undefined
              ? null
              : await run(options.timeline.snapshotForThread(threadRef))
            return { ...summary, messages, timeline }
          }
        } catch {
          // A transient read is retried while the exact scope remains live.
        }
      }
      await sleep(100)
    }
    return null
  }

  const confirmedRuntimeOutcome = async (input: Readonly<{
    threadRef: string
    runRef: string
    afterSequence: number
    onUpdate?: (thread: MobileConversationThread) => void
  }>): Promise<MobileConversationThread | null> => {
    let lastSignature = ""
    for (let attempt = 0; attempt < Math.max(pollAttempts, 300); attempt += 1) {
      const thread = await confirmedThread(input.threadRef)
      const timeline = thread?.timeline
      const run = timeline?.run
      if (thread !== null) {
        const signature = [
          run?.runRef ?? "none",
          run?.status ?? "none",
          ...(timeline?.events ?? []).map(event => `${event.eventRef}:${event.version}`),
        ].join("|")
        if (signature !== lastSignature) {
          lastSignature = signature
          input.onUpdate?.(thread)
        }
      }
      const latestSequence = Math.max(
        0,
        ...(timeline?.events.map(event => event.sequence) ?? []),
      )
      if (
        thread !== null &&
        run?.runRef === input.runRef &&
        latestSequence > input.afterSequence &&
        (run.status === "completed" || run.status === "failed" || run.status === "canceled")
      ) return thread
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
      if (thread === null) {
        return { ok: false, error: "Message is still pending reconciliation." }
      }
      if (options.runtime === undefined) return { ok: true, thread }
      const turnRef = `turn.mobile.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`
      const active = thread.timeline?.run
      const continuingActiveRun =
        active !== null && active !== undefined && active.status === "running"
      const previousSequence = Math.max(
        0,
        ...(thread.timeline?.events.map(event => event.sequence) ?? []),
      )
      const context = {
        nowIso: new Date().toISOString(),
        surface: "mobile" as const,
        target: {
          lane: active?.runtime === "claude_code"
            ? "claude_pylon" as const
            : active?.runtime === "openagents_native"
              ? "hosted_khala" as const
              : "codex_app_server" as const,
        },
      }
      try {
        if (continuingActiveRun) {
          await run(options.runtime.appendUserMessage(buildAppendUserMessageIntent({
            context,
            messageRef,
            threadRef: input.threadRef,
            turnRef: active.runRef,
          })))
        } else {
          await run(options.runtime.startTurn(buildStartTurnIntent({
            context,
            messageRef,
            threadRef: input.threadRef,
            turnRef,
          })))
        }
      } catch {
        return { ok: false, error: "Message was admitted, but runtime dispatch is unavailable." }
      }
      const expectedRunRef = continuingActiveRun ? active.runRef : turnRef
      const settled = await confirmedRuntimeOutcome({
        afterSequence: continuingActiveRun ? previousSequence : 0,
        onUpdate: input.onUpdate,
        runRef: expectedRunRef,
        threadRef: input.threadRef,
      })
      return settled === null
        ? { ok: false, error: "Runtime outcome is still pending reconciliation." }
        : { ok: true, thread: settled }
    },
    interrupt: async input => {
      const thread = await confirmedThread(input.threadRef)
      if (thread === null || thread.timeline?.run?.runRef !== input.runRef) {
        return { ok: false, error: "The confirmed runtime turn is unavailable." }
      }
      if (options.runtime === undefined) {
        return { ok: false, error: "The runtime command service is unavailable." }
      }
      try {
        const previousSequence = Math.max(
          0,
          ...(thread.timeline.events.map(event => event.sequence) ?? []),
        )
        await run(options.runtime.interruptTurn(buildInterruptTurnIntent({
          commandRef: `mobile.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`,
          context: {
            nowIso: new Date().toISOString(),
            surface: "mobile",
            target: { lane: "codex_app_server" },
          },
          threadRef: input.threadRef,
          turnRef: input.runRef,
        })))
        const settled = await confirmedRuntimeOutcome({
          afterSequence: previousSequence,
          onUpdate: input.onUpdate,
          runRef: input.runRef,
          threadRef: input.threadRef,
        })
        return settled === null
          ? { ok: false, error: "Interrupt is still pending reconciliation." }
          : { ok: true, thread: settled }
      } catch {
        return { ok: false, error: "Interrupt is still pending reconciliation." }
      }
    },
  }
}

export const selectMobileConversation = async (input: Readonly<{
  conversation: () => KhalaSyncConversation | null
  timeline?: () => KhalaSyncAgentTimeline | null
  runtime?: () => KhalaSyncRuntimeCommands | null
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
  adapter?: Omit<MobileConversationAdapterOptions, "conversation" | "runtime" | "timeline">
}>): Promise<MobileConversationSelection> => {
  const sleep = input.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = input.pollAttempts ?? 30

  for (let attempt = 0; attempt < pollAttempts; attempt += 1) {
    const conversation = input.conversation()
    const timeline = input.timeline?.() ?? undefined
    const runtime = input.runtime?.() ?? undefined
    if (conversation !== null && conversation.personalStatus().phase === "live") {
      const host = makeMobileConversationHost({
        conversation,
        ...(runtime === undefined ? {} : { runtime }),
        ...(timeline === undefined ? {} : { timeline }),
        ...input.adapter,
      })
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
