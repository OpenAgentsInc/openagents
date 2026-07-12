import type {
  ChatMessageImageAttachment,
  RuntimeInteractionDecision,
} from "@openagentsinc/khala-sync"
import type {
  ConfirmedAgentTimelineSnapshot,
  ConfirmedAgentRun,
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaConversationLiveSnapshot,
  KhalaSyncAgentTimeline,
  KhalaSyncConversation,
  KhalaSyncLiveAgentGraph,
  KhalaSyncRuntimeCommands,
  KhalaSyncRuntimeInteractions,
  RuntimeCommandTarget,
} from "@openagentsinc/khala-sync-client"
import {
  buildAppendUserMessageIntent,
  buildCloseTurnIntent,
  buildContinueTurnIntent,
  buildInterruptTurnIntent,
  buildRetryTurnIntent,
  buildStartTurnIntent,
  buildRuntimeInteractionDecisionCommand,
  openKhalaConversationLive,
} from "@openagentsinc/khala-sync-client"
import { Effect } from "effect"

import type { MobileCodingThreadLease } from "../coding/mobile-coding-navigation"

export type MobileConversationMessage = ConfirmedChatMessage
export type MobileConversationThreadSummary = ConfirmedChatThread

export type MobileConversationThread = MobileConversationThreadSummary & Readonly<{
  messages: ReadonlyArray<MobileConversationMessage>
  timeline?: ConfirmedAgentTimelineSnapshot | null
  /** Confirmed canonical `live_agent_graph` post-images for this exact thread scope. */
  graphs?: KhalaConversationLiveSnapshot["graphs"]
}>

export type MobileConversationMutationResult =
  | Readonly<{ ok: true; thread: MobileConversationThread }>
  | Readonly<{ ok: false; error: string }>

export type MobileRuntimeControlAction =
  | "cancel"
  | "close"
  | "resume"
  | "retry"

export type MobileConversationHost = Readonly<{
  listThreads: () => Promise<ReadonlyArray<MobileConversationThreadSummary>>
  newThread: () => Promise<MobileConversationMutationResult>
  openThread: (threadRef: string) => Promise<MobileConversationThread | null>
  watchThread?: (
    threadRef: string,
    onUpdate: (thread: MobileConversationThread) => void,
  ) => Promise<MobileCodingThreadLease | null>
  sendMessage: (input: Readonly<{
    threadRef: string
    body: string
    attachments?: ReadonlyArray<ChatMessageImageAttachment>
    /** Exact persisted composer target for a brand-new coding turn. Ignored
     * while steering an already-confirmed active run, whose lane is fixed. */
    runtimeTarget?: RuntimeCommandTarget
    onUpdate?: (thread: MobileConversationThread) => void
  }>) => Promise<MobileConversationMutationResult>
  interrupt?: (input: Readonly<{
    threadRef: string
    runRef: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>) => Promise<MobileConversationMutationResult>
  controlTurn?: (input: Readonly<{
    action: MobileRuntimeControlAction
    threadRef: string
    runRef: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>) => Promise<MobileConversationMutationResult>
  decideInteraction?: (input: Readonly<{
    interactionRef: string
    threadRef: string
    turnRef: string
    decision: RuntimeInteractionDecision
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
  agentGraph?: KhalaSyncLiveAgentGraph
  runtime?: KhalaSyncRuntimeCommands
  interactions?: KhalaSyncRuntimeInteractions
  randomId?: () => string
  now?: () => Date
  commandTtlMs?: number
  sleep?: (ms: number) => Promise<void>
  pollAttempts?: number
}>

const nextRef = (kind: "thread" | "message", randomId: () => string): string =>
  `${kind}.mobile.${randomId().replace(/[^A-Za-z0-9._:-]/g, "")}`

const run = <Value, Error>(effect: Effect.Effect<Value, Error>): Promise<Value> =>
  Effect.runPromise(effect)

const laneForConfirmedRuntime = (
  runtime: ConfirmedAgentRun["runtime"],
) => runtime === "claude_code"
  ? "claude_pylon" as const
  : runtime === "openagents_native"
    ? "hosted_khala" as const
    : runtime === "codex" || runtime === "opencode_codex"
      ? "codex_app_server" as const
      : null

export const makeMobileConversationHost = (
  options: MobileConversationAdapterOptions,
): MobileConversationHost => {
  const randomId = options.randomId ?? (() => globalThis.crypto.randomUUID())
  const now = options.now ?? (() => new Date())
  const commandTtlMs = options.commandTtlMs ?? 5 * 60_000
  const sleep = options.sleep ?? (ms => new Promise(resolve => setTimeout(resolve, ms)))
  const pollAttempts = options.pollAttempts ?? 30
  let subscriptionSequence = 0

  const listThreads = async (): Promise<ReadonlyArray<MobileConversationThreadSummary>> =>
    options.conversation.personalStatus().phase === "live"
      ? run(options.conversation.listConfirmedThreads())
      : []

  const threadFromSnapshot = (
    snapshot: KhalaConversationLiveSnapshot,
  ): MobileConversationThread | null => snapshot.thread === null
    ? null
    : {
        ...snapshot.thread,
        messages: snapshot.messages,
        timeline: snapshot.timeline,
        graphs: snapshot.graphs,
      }

  const readConfirmedThread = async (
    threadRef: string,
  ): Promise<MobileConversationThread | null> => {
    if (
      options.conversation.personalStatus().phase !== "live" ||
      options.conversation.threadStatus(threadRef).phase !== "live"
    ) return null
    try {
      const [threads, messages, timeline, graphSnapshot] = await Promise.all([
        run(options.conversation.listConfirmedThreads()),
        run(options.conversation.listConfirmedMessages(threadRef)),
        options.timeline === undefined
          ? Promise.resolve(null)
          : run(options.timeline.snapshotForThread(threadRef)),
        options.agentGraph === undefined
          ? Promise.resolve(null)
          : run(options.agentGraph.snapshotForThread(threadRef)),
      ])
      const summary = threads.find(thread => thread.threadRef === threadRef)
      return summary === undefined
        ? null
        : { ...summary, messages, timeline, graphs: graphSnapshot?.graphs ?? [] }
    } catch {
      return null
    }
  }

  const waitForThread = async (input: Readonly<{
    threadRef: string
    requiredMessageRef?: string
  }>): Promise<MobileConversationThread | null> => {
    const accepted = (thread: MobileConversationThread | null): thread is MobileConversationThread =>
      thread !== null && (
        input.requiredMessageRef === undefined ||
        thread.messages.some(message => message.messageRef === input.requiredMessageRef)
      )
    const initial = await readConfirmedThread(input.threadRef)
    if (accepted(initial)) return initial

    let settle!: (thread: MobileConversationThread | null) => void
    let settled = false
    const result = new Promise<MobileConversationThread | null>(resolve => {
      settle = thread => {
        if (settled) return
        settled = true
        resolve(thread)
      }
    })
    let subscription
    try {
      subscription = await openKhalaConversationLive({
        conversation: options.conversation,
        timeline: options.timeline,
        ...(options.agentGraph === undefined ? {} : { agentGraph: options.agentGraph }),
        subscriptionRef: `subscription.mobile.${++subscriptionSequence}`,
        generation: subscriptionSequence,
        threadRef: input.threadRef,
        afterCursor: options.conversation.threadStatus(input.threadRef).cursor,
      }, update => {
        const thread = update.snapshot === null ? null : threadFromSnapshot(update.snapshot)
        if (accepted(thread)) settle(thread)
      })
    } catch {
      return null
    }
    const deadline = sleep(Math.max(1, pollAttempts) * 100).then(() => null)
    const resolved = await Promise.race([result, deadline])
    settled = true
    await subscription.close()
    return resolved
  }

  const confirmedThread = (
    threadRef: string,
    requiredMessageRef?: string,
  ): Promise<MobileConversationThread | null> =>
    waitForThread({
      threadRef,
      ...(requiredMessageRef === undefined ? {} : { requiredMessageRef }),
    })

  const confirmedRuntimeOutcome = async (input: Readonly<{
    intentId: string
    threadRef: string
    runRef: string
    afterSequence: number
    onUpdate?: (thread: MobileConversationThread) => void
  }>): Promise<
    | Readonly<{ kind: "settled"; thread: MobileConversationThread }>
    | Readonly<{ kind: "expired" }>
    | null
  > => {
    let lastSignature = ""
    const evaluate = async (
      thread: MobileConversationThread | null,
    ): Promise<Readonly<{ kind: "settled"; thread: MobileConversationThread }> | Readonly<{ kind: "expired" }> | null> => {
      const command = await run(options.runtime!.outcome({
        intentId: input.intentId,
        threadRef: input.threadRef,
      }))
      if (command?.status === "expired") return { kind: "expired" }
      const timeline = thread?.timeline
      const activeRun = timeline?.run
      if (thread !== null) {
        const signature = [
          activeRun?.runRef ?? "none",
          activeRun?.status ?? "none",
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
        activeRun?.runRef === input.runRef &&
        latestSequence > input.afterSequence &&
        (activeRun.status === "completed" || activeRun.status === "failed" || activeRun.status === "canceled")
      ) return { kind: "settled", thread }
      return null
    }

    const initial = await evaluate(await readConfirmedThread(input.threadRef))
    if (initial !== null) return initial

    let settle!: (outcome: Readonly<{ kind: "settled"; thread: MobileConversationThread }> | Readonly<{ kind: "expired" }> | null) => void
    let settled = false
    const result = new Promise<Readonly<{ kind: "settled"; thread: MobileConversationThread }> | Readonly<{ kind: "expired" }> | null>(resolve => {
      settle = outcome => {
        if (settled || outcome === null) return
        settled = true
        resolve(outcome)
      }
    })
    let subscription
    try {
      subscription = await openKhalaConversationLive({
        conversation: options.conversation,
        timeline: options.timeline,
        ...(options.agentGraph === undefined ? {} : { agentGraph: options.agentGraph }),
        subscriptionRef: `subscription.mobile.${++subscriptionSequence}`,
        generation: subscriptionSequence,
        threadRef: input.threadRef,
        afterCursor: options.conversation.threadStatus(input.threadRef).cursor,
      }, async update => {
        const thread = update.snapshot === null ? null : threadFromSnapshot(update.snapshot)
        settle(await evaluate(thread))
      })
    } catch {
      return null
    }
    const deadline = sleep(Math.max(pollAttempts, 300) * 100).then(() => null)
    const resolved = await Promise.race([result, deadline])
    settled = true
    await subscription.close()
    return resolved
  }

  const confirmedControlOutcome = async (input: Readonly<{
    action: MobileRuntimeControlAction
    intentId: string
    threadRef: string
    runRef: string
    previousRunVersion: number
    onUpdate?: (thread: MobileConversationThread) => void
  }>): Promise<
    | Readonly<{ kind: "settled"; thread: MobileConversationThread }>
    | Readonly<{ kind: "expired" }>
    | Readonly<{ kind: "failed" }>
    | null
  > => {
    let lastSignature = ""
    const evaluate = async (
      thread: MobileConversationThread | null,
    ): Promise<
      | Readonly<{ kind: "settled"; thread: MobileConversationThread }>
      | Readonly<{ kind: "expired" }>
      | Readonly<{ kind: "failed" }>
      | null
    > => {
      const command = await run(options.runtime!.outcome({
        intentId: input.intentId,
        threadRef: input.threadRef,
      }))
      if (command?.status === "expired") return { kind: "expired" }
      if (command?.status === "failed") return { kind: "failed" }
      const activeRun = thread?.timeline?.run
      if (thread !== null) {
        const signature = `${activeRun?.runRef ?? "none"}:${activeRun?.status ?? "none"}:${activeRun?.version ?? 0}`
        if (signature !== lastSignature) {
          lastSignature = signature
          input.onUpdate?.(thread)
        }
      }
      if (
        command === null || command.status === "pending" || thread === null ||
        activeRun?.runRef !== input.runRef ||
        activeRun.version <= input.previousRunVersion
      ) return null
      const expected = input.action === "cancel" || input.action === "close"
        ? activeRun.status === "canceled"
        : activeRun.status === "queued" || activeRun.status === "running" ||
          activeRun.status === "waiting_for_input"
      return expected ? { kind: "settled", thread } : null
    }

    const initial = await evaluate(await readConfirmedThread(input.threadRef))
    if (initial !== null) return initial

    let settle!: (outcome:
      | Readonly<{ kind: "settled"; thread: MobileConversationThread }>
      | Readonly<{ kind: "expired" }>
      | Readonly<{ kind: "failed" }>
      | null
    ) => void
    let settled = false
    const result = new Promise<
      | Readonly<{ kind: "settled"; thread: MobileConversationThread }>
      | Readonly<{ kind: "expired" }>
      | Readonly<{ kind: "failed" }>
      | null
    >(resolve => {
      settle = outcome => {
        if (settled || outcome === null) return
        settled = true
        resolve(outcome)
      }
    })
    let subscription
    try {
      subscription = await openKhalaConversationLive({
        conversation: options.conversation,
        timeline: options.timeline,
        ...(options.agentGraph === undefined ? {} : { agentGraph: options.agentGraph }),
        subscriptionRef: `subscription.mobile.${++subscriptionSequence}`,
        generation: subscriptionSequence,
        threadRef: input.threadRef,
        afterCursor: options.conversation.threadStatus(input.threadRef).cursor,
      }, async update => {
        const thread = update.snapshot === null ? null : threadFromSnapshot(update.snapshot)
        settle(await evaluate(thread))
      })
    } catch {
      return null
    }
    const deadline = sleep(Math.max(pollAttempts, 300) * 100).then(() => null)
    const resolved = await Promise.race([result, deadline])
    settled = true
    await subscription.close()
    return resolved
  }

  const interactionIsTerminal = (
    thread: MobileConversationThread | null,
    interactionRef: string,
    turnRef: string,
  ): thread is MobileConversationThread => {
    if (thread?.timeline?.run?.runRef !== turnRef) return false
    const item = thread.timeline.events.find(event =>
      event.eventRef === interactionRef)?.item
    if (item === null || item === undefined) return false
    const status = item.kind === "question" && item.questionRef === interactionRef
      ? item.status
      : item.kind === "approval" && item.interactionRef === interactionRef
        ? item.status
        : item.kind === "plan" && item.interactionRef === interactionRef
          ? item.status
          : undefined
    return status === "resolved" || status === "expired" || status === "revoked"
  }

  const waitForInteractionResolution = async (input: Readonly<{
    interactionRef: string
    threadRef: string
    turnRef: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>): Promise<MobileConversationThread | null> => {
    const initial = await readConfirmedThread(input.threadRef)
    if (interactionIsTerminal(initial, input.interactionRef, input.turnRef)) {
      return initial
    }

    let settle!: (thread: MobileConversationThread | null) => void
    let settled = false
    const result = new Promise<MobileConversationThread | null>(resolve => {
      settle = thread => {
        if (settled) return
        settled = true
        resolve(thread)
      }
    })
    let subscription
    try {
      subscription = await openKhalaConversationLive({
        conversation: options.conversation,
        timeline: options.timeline,
        ...(options.agentGraph === undefined ? {} : { agentGraph: options.agentGraph }),
        subscriptionRef: `subscription.mobile.${++subscriptionSequence}`,
        generation: subscriptionSequence,
        threadRef: input.threadRef,
        afterCursor: options.conversation.threadStatus(input.threadRef).cursor,
      }, update => {
        const thread = update.snapshot === null ? null : threadFromSnapshot(update.snapshot)
        if (thread !== null) input.onUpdate?.(thread)
        if (interactionIsTerminal(thread, input.interactionRef, input.turnRef)) {
          settle(thread)
        }
      })
    } catch {
      return null
    }
    const deadline = sleep(Math.max(pollAttempts, 300) * 100).then(() => null)
    const resolved = await Promise.race([result, deadline])
    settled = true
    await subscription.close()
    return resolved
  }

  const controlTurn = async (input: Readonly<{
    action: MobileRuntimeControlAction
    threadRef: string
    runRef: string
    onUpdate?: (thread: MobileConversationThread) => void
  }>): Promise<MobileConversationMutationResult> => {
    const thread = await confirmedThread(input.threadRef)
    const activeRun = thread?.timeline?.run
    if (thread === null || activeRun?.runRef !== input.runRef) {
      return { ok: false, error: "The confirmed runtime turn is unavailable." }
    }
    if (options.runtime === undefined) {
      return { ok: false, error: "The runtime command service is unavailable." }
    }
    const lane = laneForConfirmedRuntime(activeRun.runtime)
    if (lane === null) {
      return { ok: false, error: "The confirmed runtime lane is unavailable." }
    }
    const allowed = input.action === "cancel"
      ? activeRun.status === "queued" || activeRun.status === "running" ||
        activeRun.status === "waiting_for_input"
      : input.action === "resume"
        ? activeRun.status === "canceled"
        : activeRun.status === "completed" || activeRun.status === "failed" ||
          activeRun.status === "canceled"
    if (!allowed) {
      return { ok: false, error: "This control is not valid for the confirmed runtime state." }
    }
    const suffix = randomId().replace(/[^A-Za-z0-9._:-]/g, "")
    const createdAt = now()
    const commandInput = {
      commandRef: `mobile.${suffix}`,
      context: {
        expiresAtIso: new Date(createdAt.getTime() + commandTtlMs).toISOString(),
        nowIso: createdAt.toISOString(),
        surface: "mobile" as const,
        target: { lane },
      },
      threadRef: input.threadRef,
      turnRef: input.runRef,
    }
    const intent = input.action === "cancel"
      ? buildInterruptTurnIntent(commandInput)
      : input.action === "resume"
        ? buildContinueTurnIntent(commandInput)
        : input.action === "retry"
          ? buildRetryTurnIntent(commandInput)
          : buildCloseTurnIntent(commandInput)
    try {
      if (input.action === "cancel") {
        await run(options.runtime.interruptTurn(intent))
      } else if (input.action === "resume") {
        await run(options.runtime.continueTurn(intent))
      } else if (input.action === "retry") {
        await run(options.runtime.retryTurn(intent))
      } else {
        await run(options.runtime.closeTurn(intent))
      }
      const outcome = await confirmedControlOutcome({
        action: input.action,
        intentId: intent.intentId,
        onUpdate: input.onUpdate,
        previousRunVersion: activeRun.version,
        runRef: input.runRef,
        threadRef: input.threadRef,
      })
      if (outcome === null) {
        return { ok: false, error: "Runtime control is still pending reconciliation." }
      }
      if (outcome.kind === "expired") {
        return { ok: false, error: "Runtime control expired while this device was offline." }
      }
      if (outcome.kind === "failed") {
        return { ok: false, error: "Runtime control was rejected by the confirmed authority." }
      }
      return { ok: true, thread: outcome.thread }
    } catch {
      return { ok: false, error: "Runtime control is still pending reconciliation." }
    }
  }

  return {
    listThreads,
    openThread: confirmedThread,
    watchThread: async (threadRef, onUpdate) => {
      const initial = await readConfirmedThread(threadRef)
      if (initial === null) return null
      let closed = false
      try {
        const subscription = await openKhalaConversationLive({
          conversation: options.conversation,
          timeline: options.timeline,
          ...(options.agentGraph === undefined ? {} : { agentGraph: options.agentGraph }),
          subscriptionRef: `subscription.mobile.${++subscriptionSequence}`,
          generation: subscriptionSequence,
          threadRef,
          afterCursor: options.conversation.threadStatus(threadRef).cursor,
        }, update => {
          if (closed || update.snapshot === null) return
          const thread = threadFromSnapshot(update.snapshot)
          if (thread !== null && thread.threadRef === threadRef) onUpdate(thread)
        })
        return {
          close: async () => {
            if (closed) return
            closed = true
            await subscription.close()
          },
        }
      } catch {
        return null
      }
    },
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
          ...(input.attachments === undefined ? {} : { attachments: input.attachments }),
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
      const createdAt = now()
      const context = {
        expiresAtIso: new Date(createdAt.getTime() + commandTtlMs).toISOString(),
        nowIso: createdAt.toISOString(),
        surface: "mobile" as const,
        target: continuingActiveRun
          ? {
              lane: active.runtime === "claude_code"
                ? "claude_pylon" as const
                : active.runtime === "openagents_native"
                  ? "hosted_khala" as const
                  : "codex_app_server" as const,
            }
          : input.runtimeTarget ?? { lane: "codex_app_server" as const },
      }
      const runtimeIntent = continuingActiveRun
        ? buildAppendUserMessageIntent({
            context,
            messageRef,
            threadRef: input.threadRef,
            turnRef: active.runRef,
          })
        : buildStartTurnIntent({
            context,
            messageRef,
            threadRef: input.threadRef,
            turnRef,
          })
      try {
        if (continuingActiveRun) {
          await run(options.runtime.appendUserMessage(runtimeIntent))
        } else {
          await run(options.runtime.startTurn(runtimeIntent))
        }
      } catch {
        return { ok: false, error: "Message was admitted, but runtime dispatch is unavailable." }
      }
      const expectedRunRef = continuingActiveRun ? active.runRef : turnRef
      const settled = await confirmedRuntimeOutcome({
        afterSequence: continuingActiveRun ? previousSequence : 0,
        intentId: runtimeIntent.intentId,
        onUpdate: input.onUpdate,
        runRef: expectedRunRef,
        threadRef: input.threadRef,
      })
      return settled === null
        ? { ok: false, error: "Runtime outcome is still pending reconciliation." }
        : settled.kind === "expired"
          ? { ok: false, error: "Runtime command expired while this device was offline." }
          : { ok: true, thread: settled.thread }
    },
    controlTurn,
    interrupt: input => controlTurn({ ...input, action: "cancel" }),
    decideInteraction: options.interactions === undefined
      ? undefined
      : async input => {
          const suffix = randomId().replace(/[^A-Za-z0-9._:-]/g, "")
          const command = buildRuntimeInteractionDecisionCommand({
            interactionRef: input.interactionRef,
            threadRef: input.threadRef,
            turnRef: input.turnRef,
            envelope: {
              decisionRef: `decision.mobile.${suffix}`,
              idempotencyKey: `idem.decision.mobile.${suffix}`,
              decidedAt: now().toISOString(),
              surface: "mobile",
              decision: input.decision,
            },
          })
          try {
            await run(options.interactions!.decide(command))
          } catch {
            return { ok: false, error: "This interaction is no longer actionable." }
          }
          const thread = await waitForInteractionResolution(input)
          return thread === null
            ? { ok: false, error: "Decision is still pending reconciliation." }
            : { ok: true, thread }
        },
  }
}

export const selectMobileConversation = async (input: Readonly<{
  conversation: () => KhalaSyncConversation | null
  timeline?: () => KhalaSyncAgentTimeline | null
  agentGraph?: () => KhalaSyncLiveAgentGraph | null
  runtime?: () => KhalaSyncRuntimeCommands | null
  interactions?: () => KhalaSyncRuntimeInteractions | null
  preferredThreadRef?: string
  adapter?: Omit<MobileConversationAdapterOptions, "agentGraph" | "conversation" | "interactions" | "runtime" | "timeline">
}>): Promise<MobileConversationSelection> => {
  const conversation = input.conversation()
  const timeline = input.timeline?.() ?? undefined
  const agentGraph = input.agentGraph?.() ?? undefined
  const runtime = input.runtime?.() ?? undefined
  const interactions = input.interactions?.() ?? undefined
  if (conversation === null || conversation.personalStatus().phase !== "live") {
    return { mode: "local" }
  }
  const host = makeMobileConversationHost({
    conversation,
    ...(runtime === undefined ? {} : { runtime }),
    ...(interactions === undefined ? {} : { interactions }),
    ...(timeline === undefined ? {} : { timeline }),
    ...(agentGraph === undefined ? {} : { agentGraph }),
    ...input.adapter,
  })
  try {
    const threads = await host.listThreads()
    const preferred = input.preferredThreadRef === undefined
      ? undefined
      : threads.find(thread => thread.threadRef === input.preferredThreadRef)
    const activeSummary = preferred ?? threads[0]
    const activeThread = activeSummary === undefined
      ? null
      : await host.openThread(activeSummary.threadRef)
    return threads.length === 0 || activeThread !== null
      ? { mode: "sync", host, threads, activeThread }
      : { mode: "local" }
  } catch {
    return { mode: "local" }
  }
}
