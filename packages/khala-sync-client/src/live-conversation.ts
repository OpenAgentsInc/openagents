import { Effect, Schema } from "effect"

import {
  ConfirmedChatMessageSchema,
  ConfirmedChatThreadSchema,
  KhalaSyncConversationStatusSchema,
  type KhalaSyncConversation,
  type KhalaSyncConversationStatus,
} from "./conversation.js"
import {
  ConfirmedAgentRunSchema,
  ConfirmedAgentTimelineEventSchema,
  type KhalaSyncAgentTimeline,
} from "./agent-timeline.js"
import {
  ConfirmedLiveAgentGraphsSchema,
  type KhalaSyncLiveAgentGraph,
} from "./live-agent-graph.js"

const LiveRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const LiveCursorSchema = Schema.NullOr(Schema.Number.check(
  Schema.isInt(),
  Schema.isGreaterThanOrEqualTo(0),
))
const LiveCorrelationRefsSchema = Schema.Array(LiveRefSchema).check(
  Schema.isMaxLength(500),
)

const KhalaConversationLiveEnvelopeBase = {
  kind: Schema.Literal("conversation.live"),
  subscriptionRef: LiveRefSchema,
  generation: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  sequence: Schema.Number.check(Schema.isInt(), Schema.isGreaterThan(0)),
  threadRef: LiveRefSchema,
  cursor: LiveCursorSchema,
  recovery: Schema.Literals(["initial", "resumed", "authoritative_refetch"]),
  runRef: Schema.optional(LiveRefSchema),
  messageRefs: LiveCorrelationRefsSchema,
  eventRefs: LiveCorrelationRefsSchema,
  graphRefs: LiveCorrelationRefsSchema,
} as const

export const KhalaConversationLiveEnvelopeSchema = Schema.Union([
  Schema.Struct({
    ...KhalaConversationLiveEnvelopeBase,
    delivery: Schema.Literals(["provisional", "confirmed"]),
  }),
  Schema.Struct({
    ...KhalaConversationLiveEnvelopeBase,
    delivery: Schema.Literal("interrupted"),
    reason: Schema.Literals([
    "bootstrapping",
    "catching_up",
    "must_refetch",
    "denied",
    "idle",
    "cursor_ahead",
    "read_failed",
    ]),
  }),
])
export type KhalaConversationLiveEnvelope =
  typeof KhalaConversationLiveEnvelopeSchema.Type

const KhalaConversationLiveTimelineSnapshotSchema = Schema.Struct({
  status: KhalaSyncConversationStatusSchema,
  run: Schema.NullOr(ConfirmedAgentRunSchema),
  events: Schema.Array(ConfirmedAgentTimelineEventSchema).check(
    Schema.isMaxLength(500),
  ),
})

export const KhalaConversationLiveSnapshotSchema = Schema.Struct({
  status: KhalaSyncConversationStatusSchema,
  thread: Schema.NullOr(ConfirmedChatThreadSchema),
  messages: Schema.Array(ConfirmedChatMessageSchema).check(
    Schema.isMaxLength(500),
  ),
  timeline: Schema.NullOr(KhalaConversationLiveTimelineSnapshotSchema),
  graphs: ConfirmedLiveAgentGraphsSchema,
})
export type KhalaConversationLiveSnapshot =
  typeof KhalaConversationLiveSnapshotSchema.Type

export const KhalaConversationLiveUpdateSchema = Schema.Struct({
  kind: Schema.Literal("conversation.live.update"),
  envelope: KhalaConversationLiveEnvelopeSchema,
  snapshot: Schema.NullOr(KhalaConversationLiveSnapshotSchema),
}).pipe(
  Schema.check(
    Schema.makeFilter(
      update => {
        if (update.envelope.delivery === "interrupted") {
          return update.snapshot === null && update.envelope.graphRefs.length === 0
        }
        if (update.snapshot === null) return false
        const graphRefs = update.snapshot.graphs.map(graph => graph.graphRef)
        return graphRefs.length === update.envelope.graphRefs.length &&
          graphRefs.every((graphRef, index) => graphRef === update.envelope.graphRefs[index])
      },
      { message: "live graph refs must exactly match the authoritative snapshot" },
    ),
  ),
)
export type KhalaConversationLiveUpdate =
  typeof KhalaConversationLiveUpdateSchema.Type

export type KhalaConversationLiveSubscription = Readonly<{
  close: () => Promise<void>
  closed: () => boolean
  metrics: () => KhalaConversationLiveMetrics
  /**
   * Drain-await for the delivery pump (openagents issue #8782): resolves when
   * no source signal is pending and no delivery is in flight (or the
   * subscription is closed). Deterministic test/orchestration synchronization
   * — the same contract as the shared `DrainableWorker.drain` in
   * `@openagentsinc/pipeline-signals` — so callers never sleep or poll.
   */
  settled: () => Promise<void>
}>

export type KhalaConversationLiveMetrics = Readonly<{
  sourceSignals: number
  deliveredUpdates: number
  coalescedSignals: number
  maxPendingSnapshots: 0 | 1
  lastDeliveryLatencyMs: number | null
}>

export type KhalaConversationLiveOptions = Readonly<{
  conversation: KhalaSyncConversation
  timeline?: KhalaSyncAgentTimeline
  agentGraph?: KhalaSyncLiveAgentGraph
  subscriptionRef: string
  generation: number
  threadRef: string
  afterCursor?: number | null
  signal?: AbortSignal
  now?: () => number
  onError?: (error: unknown) => void
}>

const recoveryFor = (
  afterCursor: number | null,
  cursor: number | null,
): KhalaConversationLiveEnvelope["recovery"] => {
  if (afterCursor === null || cursor === null) return "initial"
  return cursor > afterCursor + 1 ? "authoritative_refetch" : "resumed"
}

const interruptedReason = (
  phase: KhalaSyncConversationStatus["phase"],
): "bootstrapping" | "catching_up" | "must_refetch" | "denied" | "idle" | undefined =>
  phase === "live" ? undefined : phase

/**
 * Open one cursor-aware native conversation subscription.
 *
 * Delivery is serialized. While a consumer is awaiting one update, any
 * number of source notifications collapse into one newest authoritative
 * snapshot, so a slow renderer cannot create an unbounded event queue. The
 * durable Sync cursor classifies resume versus bounded refetch; socket health
 * and callback timing never manufacture confirmation.
 */
export const openKhalaConversationLive = async (
  options: KhalaConversationLiveOptions,
  listener: (update: KhalaConversationLiveUpdate) => void | Promise<void>,
): Promise<KhalaConversationLiveSubscription> => {
  const afterCursor = options.afterCursor ?? null
  const now = options.now ?? Date.now
  let closed = false
  let draining = false
  let pending = false
  let sequence = 0
  let lastSignature = ""
  let pendingSince: number | null = null
  let sourceSignals = 0
  let deliveredUpdates = 0
  let coalescedSignals = 0
  let maxPendingSnapshots: 0 | 1 = 0
  let lastDeliveryLatencyMs: number | null = null
  let unsubscribe = (): void => undefined
  let settledResolvers: Array<() => void> = []

  const isSettled = (): boolean => closed || (!draining && !pending)
  const notifySettled = (): void => {
    if (!isSettled() || settledResolvers.length === 0) return
    const resolvers = settledResolvers
    settledResolvers = []
    for (const resolve of resolvers) resolve()
  }

  const snapshot = async (): Promise<KhalaConversationLiveSnapshot> => {
    const status = options.conversation.threadStatus(options.threadRef)
    if (status.phase !== "live") {
      return { status, thread: null, messages: [], timeline: null, graphs: [] }
    }
    const [threads, messages, timeline, agentGraph] = await Promise.all([
      Effect.runPromise(options.conversation.listConfirmedThreads()),
      Effect.runPromise(options.conversation.listConfirmedMessages(options.threadRef)),
      options.timeline === undefined
        ? Promise.resolve(null)
        : Effect.runPromise(options.timeline.snapshotForThread(options.threadRef)),
      options.agentGraph === undefined
        ? Promise.resolve(null)
        : Effect.runPromise(options.agentGraph.snapshotForThread(options.threadRef)),
    ])
    return {
      status,
      thread: threads.find(thread => thread.threadRef === options.threadRef) ?? null,
      messages: messages.slice(-500),
      timeline,
      graphs: agentGraph?.graphs ?? [],
    }
  }

  const nextUpdate = async (): Promise<KhalaConversationLiveUpdate | null> => {
    let current: KhalaConversationLiveSnapshot
    try {
      current = await snapshot()
    } catch (error) {
      options.onError?.(error)
      return {
        kind: "conversation.live.update",
        envelope: {
          kind: "conversation.live",
          delivery: "interrupted",
          subscriptionRef: options.subscriptionRef,
          generation: options.generation,
          sequence: ++sequence,
          threadRef: options.threadRef,
          cursor: null,
          recovery: "initial",
          reason: "read_failed",
          messageRefs: [],
          eventRefs: [],
          graphRefs: [],
        },
        snapshot: null,
      }
    }

    const { status } = current
    const cursor = status.cursor
    const runRef = current.timeline?.run?.runRef
    const messageRefs = current.messages.map(message => message.messageRef)
    const eventRefs = current.timeline?.events.map(event => event.eventRef) ?? []
    const graphRefs = current.graphs.map(graph => graph.graphRef)
    const delivery = status.phase === "live"
      ? status.pendingMutationCount > 0 ? "provisional" as const : "confirmed" as const
      : "interrupted" as const
    const reason = afterCursor !== null && cursor !== null && cursor < afterCursor
      ? "cursor_ahead" as const
      : delivery === "interrupted"
        ? interruptedReason(status.phase)
        : undefined
    const signature = JSON.stringify([
      delivery,
      cursor,
      status.pendingMutationCount,
      reason,
      runRef,
      ...current.messages.map(message => [message.messageRef, message.version]),
      current.thread?.version ?? null,
      ...((current.timeline?.events ?? []).map(event => [event.eventRef, event.version])),
      ...current.graphs.map(graph => [
        graph.graphRef,
        graph.attachmentGeneration,
        graph.cursor,
        graph.updatedAt,
      ]),
    ])
    if (signature === lastSignature) return null
    lastSignature = signature
    const baseEnvelope = {
      kind: "conversation.live" as const,
      subscriptionRef: options.subscriptionRef,
      generation: options.generation,
      sequence: ++sequence,
      threadRef: options.threadRef,
      cursor,
      recovery: recoveryFor(afterCursor, cursor),
      ...(runRef === undefined ? {} : { runRef }),
      messageRefs,
      eventRefs,
      graphRefs,
    }
    if (reason !== undefined || delivery === "interrupted") {
      return {
        kind: "conversation.live.update",
        envelope: {
          ...baseEnvelope,
          delivery: "interrupted",
          reason: reason ?? "idle",
          graphRefs: [],
        },
        snapshot: null,
      }
    }
    return {
      kind: "conversation.live.update",
      envelope: {
        ...baseEnvelope,
        delivery,
      },
      snapshot: current,
    }
  }

  const drain = async (): Promise<void> => {
    if (draining || closed) return
    draining = true
    try {
      while (pending && !closed) {
        pending = false
        const scheduledAt = pendingSince
        pendingSince = null
        const update = await nextUpdate()
        if (update !== null && !closed) {
          await listener(update)
          deliveredUpdates += 1
          lastDeliveryLatencyMs = scheduledAt === null
            ? 0
            : Math.max(0, now() - scheduledAt)
        }
      }
    } catch (error) {
      options.onError?.(error)
    } finally {
      draining = false
      if (pending && !closed) void drain()
      else notifySettled()
    }
  }

  const schedule = (): void => {
    if (closed) return
    sourceSignals += 1
    if (pending) coalescedSignals += 1
    else pendingSince = now()
    pending = true
    maxPendingSnapshots = 1
    void drain()
  }

  unsubscribe = options.conversation.subscribeThread(options.threadRef, schedule)
  try {
    await Effect.runPromise(options.conversation.openThread(options.threadRef))
  } catch (error) {
    unsubscribe()
    throw error
  }
  const close = async (): Promise<void> => {
    if (closed) return
    closed = true
    pending = false
    notifySettled()
    options.signal?.removeEventListener("abort", abort)
    unsubscribe()
    await Effect.runPromise(options.conversation.closeThread(options.threadRef))
  }
  const abort = (): void => { void close() }
  options.signal?.addEventListener("abort", abort, { once: true })
  if (options.signal?.aborted === true) await close()

  schedule()
  return {
    close,
    closed: () => closed,
    settled: () =>
      isSettled()
        ? Promise.resolve()
        : new Promise<void>(resolve => {
            settledResolvers.push(resolve)
          }),
    metrics: () => ({
      sourceSignals,
      deliveredUpdates,
      coalescedSignals,
      maxPendingSnapshots,
      lastDeliveryLatencyMs,
    }),
  }
}
