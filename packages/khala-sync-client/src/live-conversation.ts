import { Effect, Schema } from "effect"

import type {
  ConfirmedChatMessage,
  ConfirmedChatThread,
  KhalaSyncConversation,
  KhalaSyncConversationStatus,
} from "./conversation.js"
import type {
  ConfirmedAgentTimelineSnapshot,
  KhalaSyncAgentTimeline,
} from "./agent-timeline.js"

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

export type KhalaConversationLiveSnapshot = Readonly<{
  status: KhalaSyncConversationStatus
  thread: ConfirmedChatThread | null
  messages: ReadonlyArray<ConfirmedChatMessage>
  timeline: ConfirmedAgentTimelineSnapshot | null
}>

export type KhalaConversationLiveUpdate = Readonly<{
  envelope: KhalaConversationLiveEnvelope
  snapshot: KhalaConversationLiveSnapshot | null
}>

export type KhalaConversationLiveSubscription = Readonly<{
  close: () => Promise<void>
  closed: () => boolean
}>

export type KhalaConversationLiveOptions = Readonly<{
  conversation: KhalaSyncConversation
  timeline?: KhalaSyncAgentTimeline
  subscriptionRef: string
  generation: number
  threadRef: string
  afterCursor?: number | null
  signal?: AbortSignal
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
  let closed = false
  let draining = false
  let pending = false
  let sequence = 0
  let lastSignature = ""
  let unsubscribe = (): void => undefined

  const snapshot = async (): Promise<KhalaConversationLiveSnapshot> => {
    const status = options.conversation.threadStatus(options.threadRef)
    if (status.phase !== "live") return { status, thread: null, messages: [], timeline: null }
    const [threads, messages, timeline] = await Promise.all([
      Effect.runPromise(options.conversation.listConfirmedThreads()),
      Effect.runPromise(options.conversation.listConfirmedMessages(options.threadRef)),
      options.timeline === undefined
        ? Promise.resolve(null)
        : Effect.runPromise(options.timeline.snapshotForThread(options.threadRef)),
    ])
    return {
      status,
      thread: threads.find(thread => thread.threadRef === options.threadRef) ?? null,
      messages: messages.slice(-500),
      timeline,
    }
  }

  const nextUpdate = async (): Promise<KhalaConversationLiveUpdate | null> => {
    let current: KhalaConversationLiveSnapshot
    try {
      current = await snapshot()
    } catch (error) {
      options.onError?.(error)
      return {
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
        },
        snapshot: null,
      }
    }

    const { status } = current
    const cursor = status.cursor
    const runRef = current.timeline?.run?.runRef
    const messageRefs = current.messages.map(message => message.messageRef)
    const eventRefs = current.timeline?.events.map(event => event.eventRef) ?? []
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
    }
    if (reason !== undefined || delivery === "interrupted") {
      return {
        envelope: {
          ...baseEnvelope,
          delivery: "interrupted",
          reason: reason ?? "idle",
        },
        snapshot: null,
      }
    }
    return {
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
        const update = await nextUpdate()
        if (update !== null && !closed) await listener(update)
      }
    } catch (error) {
      options.onError?.(error)
    } finally {
      draining = false
      if (pending && !closed) void drain()
    }
  }

  const schedule = (): void => {
    if (closed) return
    pending = true
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
    options.signal?.removeEventListener("abort", abort)
    unsubscribe()
    await Effect.runPromise(options.conversation.closeThread(options.threadRef))
  }
  const abort = (): void => { void close() }
  options.signal?.addEventListener("abort", abort, { once: true })
  if (options.signal?.aborted === true) await close()

  schedule()
  return { close, closed: () => closed }
}
