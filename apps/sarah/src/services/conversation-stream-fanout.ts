import { Schema } from "effect"

const PositiveInteger = Schema.Number.check(
  Schema.isFinite(),
  Schema.isInt(),
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
)

export const SarahConversationStreamFanoutConfig = Schema.Struct({
  maxOwnerRefCharacters: PositiveInteger,
  maxConversationRefCharacters: PositiveInteger,
  maxTurnRefCharacters: PositiveInteger,
  maxTurns: PositiveInteger,
  maxSubscribersPerTurn: PositiveInteger,
  maxEventsPerTurn: PositiveInteger,
  maxEventBytes: PositiveInteger,
  maxBytesPerTurn: PositiveInteger,
  maxSubscriberLagEvents: PositiveInteger,
  maxTurnAgeMs: PositiveInteger,
  recordTimeoutMs: PositiveInteger,
  maxReplayAgeMs: PositiveInteger,
})
export type SarahConversationStreamFanoutConfig =
  typeof SarahConversationStreamFanoutConfig.Type

export const DEFAULT_SARAH_CONVERSATION_STREAM_FANOUT_CONFIG =
  SarahConversationStreamFanoutConfig.make({
    maxOwnerRefCharacters: 256,
    maxConversationRefCharacters: 256,
    maxTurnRefCharacters: 256,
    maxTurns: 128,
    maxSubscribersPerTurn: 16,
    maxEventsPerTurn: 512,
    maxEventBytes: 16 * 1024,
    maxBytesPerTurn: 1024 * 1024,
    maxSubscriberLagEvents: 64,
    maxTurnAgeMs: 90_000,
    recordTimeoutMs: 5_000,
    maxReplayAgeMs: 30_000,
  })

export type SarahConversationStreamFanoutErrorReason =
  | "invalid_config"
  | "invalid_clock"
  | "invalid_scope"
  | "service_closed"
  | "turn_limit"
  | "turn_already_exists"
  | "stream_not_available"
  | "stream_not_active"
  | "subscriber_limit"
  | "invalid_cursor"
  | "replay_unavailable"
  | "replay_expired"
  | "subscriber_busy"
  | "subscriber_detached"
  | "slow_consumer"
  | "empty_event"
  | "stream_overflow"

const ERROR_MESSAGES = {
  invalid_config: "Conversation stream fanout configuration is invalid.",
  invalid_clock: "Conversation stream fanout clock is invalid.",
  invalid_scope: "Conversation stream scope is invalid.",
  service_closed: "Conversation stream fanout is closed.",
  turn_limit: "Conversation stream fanout is at its turn limit.",
  turn_already_exists: "Conversation stream turn already exists.",
  stream_not_available: "Conversation stream is not available for this scope.",
  stream_not_active: "Conversation stream is not active.",
  subscriber_limit: "Conversation stream is at its subscriber limit.",
  invalid_cursor: "Conversation stream cursor is invalid.",
  replay_unavailable: "Requested conversation stream replay is unavailable.",
  replay_expired: "Conversation stream replay has expired.",
  subscriber_busy: "Conversation stream subscriber already has a pending read.",
  subscriber_detached: "Conversation stream subscriber is detached.",
  slow_consumer: "Conversation stream subscriber exceeded its lag limit.",
  empty_event: "Conversation stream event is empty.",
  stream_overflow: "Conversation stream exceeded a configured bound.",
} as const satisfies Record<
  SarahConversationStreamFanoutErrorReason,
  string
>

/** Fixed public-safe failure; no scope, chunk, or provider payload is echoed. */
export class SarahConversationStreamFanoutError extends Error {
  readonly _tag = "SarahConversationStreamFanoutError"
  override readonly name = "SarahConversationStreamFanoutError"

  constructor(readonly reason: SarahConversationStreamFanoutErrorReason) {
    super(ERROR_MESSAGES[reason])
  }
}

export type SarahConversationStreamScope = Readonly<{
  ownerRef: string
  conversationRef: string
  turnRef: string
}>

export type SarahConversationStreamChunkFrame = Readonly<{
  sequence: number
  observedAtMs: number
  kind: "chunk"
  chunk: string
  byteLength: number
}>

export type SarahConversationStreamTerminalOutcome =
  | Readonly<{ kind: "terminal"; terminal: "complete" }>
  | Readonly<{
      kind: "error"
      reason:
        | "producer_failed"
        | "record_failed"
        | "record_timeout"
        | "invalid_clock"
    }>
  | Readonly<{
      kind: "overflow"
      limit: "event_count" | "event_bytes" | "stream_bytes" | "turn_age"
    }>
  | Readonly<{ kind: "aborted" }>
  | Readonly<{ kind: "closed" }>

export type SarahConversationStreamTerminalFrame = Readonly<{
  sequence: number
  observedAtMs: number
}> & SarahConversationStreamTerminalOutcome

export type SarahConversationStreamFrame =
  | SarahConversationStreamChunkFrame
  | SarahConversationStreamTerminalFrame

export type SarahConversationStreamCanonicalRecord = Readonly<{
  schema: "sarah.conversation_stream_record.v1"
  scope: SarahConversationStreamScope
  openedAtMs: number
  settledAtMs: number
  outcome: SarahConversationStreamTerminalOutcome
  finalSequence: number
  eventCount: number
  byteCount: number
  chunks: ReadonlyArray<SarahConversationStreamChunkFrame>
}>

export type SarahConversationStreamScheduledTask = Readonly<{
  cancel: () => void
}>

export type SarahConversationStreamFanoutDependencies = Readonly<{
  now: () => number
  schedule: (
    delayMs: number,
    task: () => void,
  ) => SarahConversationStreamScheduledTask
}>

export const systemSarahConversationStreamFanoutDependencies: SarahConversationStreamFanoutDependencies =
  {
    now: Date.now,
    schedule: (delayMs, task) => {
      const handle = setTimeout(task, delayMs)
      return { cancel: () => clearTimeout(handle) }
    },
  }

export type SarahConversationStreamTurnSnapshot = Readonly<{
  state:
    | "active"
    | "settling"
    | "terminal"
    | "error"
    | "overflow"
    | "aborted"
    | "closed"
  eventCount: number
  byteCount: number
  subscribers: number
  finalSequence: number | null
}>

export type SarahConversationStreamSubscriber = Readonly<{
  replayFromSequence: number
  next: () => Promise<SarahConversationStreamFrame>
  detach: () => void
}>

export type SarahConversationStreamPublisher = Readonly<{
  scope: SarahConversationStreamScope
  signal: AbortSignal
  settled: Promise<SarahConversationStreamTerminalFrame>
  publish: (chunk: string) => SarahConversationStreamChunkFrame
  complete: () => Promise<SarahConversationStreamTerminalFrame>
  fail: () => Promise<SarahConversationStreamTerminalFrame>
  abort: () => Promise<SarahConversationStreamTerminalFrame>
  snapshot: () => SarahConversationStreamTurnSnapshot
}>

export type SarahConversationStreamFanoutSnapshot = Readonly<{
  closed: boolean
  turns: number
  activeTurns: number
  retainedTurns: number
  subscribers: number
  events: number
  bytes: number
}>

export type SarahConversationStreamFanout = Readonly<{
  startTurn: (input: Readonly<{
    scope: SarahConversationStreamScope
    publishAndRecord: (
      record: SarahConversationStreamCanonicalRecord,
      signal: AbortSignal,
    ) => Promise<void> | void
  }>) => SarahConversationStreamPublisher
  subscribe: (input: SarahConversationStreamScope &
    Readonly<{ afterSequence?: number }>) => SarahConversationStreamSubscriber
  snapshot: () => SarahConversationStreamFanoutSnapshot
  close: () => Promise<void>
}>

type TurnState = SarahConversationStreamTurnSnapshot["state"]

type PendingRead = {
  readonly resolve: (frame: SarahConversationStreamFrame) => void
  readonly reject: (error: SarahConversationStreamFanoutError) => void
}

type SubscriberState = {
  cursor: number
  attached: boolean
  detachReason: SarahConversationStreamFanoutErrorReason
  pending: PendingRead | null
}

type ConversationTurn = {
  readonly key: string
  readonly scope: SarahConversationStreamScope
  readonly openedAtMs: number
  readonly publishAndRecord: (
    record: SarahConversationStreamCanonicalRecord,
    signal: AbortSignal,
  ) => Promise<void> | void
  readonly frames: SarahConversationStreamFrame[]
  readonly subscribers: Set<SubscriberState>
  readonly abortController: AbortController
  readonly settled: Promise<SarahConversationStreamTerminalFrame>
  readonly resolveSettled: (
    frame: SarahConversationStreamTerminalFrame,
  ) => void
  state: TurnState
  lastObservedAtMs: number
  eventCount: number
  byteCount: number
  nextSequence: number
  finalFrame: SarahConversationStreamTerminalFrame | null
  terminalizing: Promise<SarahConversationStreamTerminalFrame> | null
  ageTask: SarahConversationStreamScheduledTask | null
  replayExpiryTask: SarahConversationStreamScheduledTask | null
}

const VALID_SCOPE_REF = /^[^\s\u0000-\u001f\u007f]+$/u
const characterCount = (value: string): number => Array.from(value).length
const encoder = new TextEncoder()
const byteLength = (value: string): number => encoder.encode(value).byteLength
const cappedSafeAdd = (left: number, right: number): number =>
  left > Number.MAX_SAFE_INTEGER - right
    ? Number.MAX_SAFE_INTEGER
    : left + right

const rejected = <Value>(
  reason: SarahConversationStreamFanoutErrorReason,
): Promise<Value> => Promise.reject(new SarahConversationStreamFanoutError(reason))

const throwError = (
  reason: SarahConversationStreamFanoutErrorReason,
): never => {
  throw new SarahConversationStreamFanoutError(reason)
}

const stateForOutcome = (
  outcome: SarahConversationStreamTerminalOutcome,
): TurnState => {
  if (outcome.kind === "terminal") return "terminal"
  if (outcome.kind === "error") return "error"
  if (outcome.kind === "overflow") return "overflow"
  if (outcome.kind === "aborted") return "aborted"
  return "closed"
}

const streamKey = (scope: SarahConversationStreamScope): string =>
  `${scope.ownerRef.length}:${scope.ownerRef}${scope.conversationRef.length}:${scope.conversationRef}${scope.turnRef.length}:${scope.turnRef}`

export function makeSarahConversationStreamFanout(input: Readonly<{
  config?: SarahConversationStreamFanoutConfig
  dependencies?: SarahConversationStreamFanoutDependencies
}> = {}): SarahConversationStreamFanout {
  let config: SarahConversationStreamFanoutConfig
  try {
    config = Schema.decodeUnknownSync(SarahConversationStreamFanoutConfig)(
      input.config ?? DEFAULT_SARAH_CONVERSATION_STREAM_FANOUT_CONFIG,
    )
  } catch {
    return throwError("invalid_config")
  }
  if (
    config.maxEventBytes > config.maxBytesPerTurn ||
    config.maxSubscriberLagEvents > config.maxEventsPerTurn
  ) {
    return throwError("invalid_config")
  }

  const dependencies =
    input.dependencies ?? systemSarahConversationStreamFanoutDependencies
  const readNow = (): number => {
    let value: number
    try {
      value = dependencies.now()
    } catch {
      return throwError("invalid_clock")
    }
    if (!Number.isSafeInteger(value) || value < 0) {
      return throwError("invalid_clock")
    }
    return value
  }
  const turns = new Map<string, ConversationTurn>()
  let closed = false
  let closePromise: Promise<void> | null = null

  const validScope = (scope: SarahConversationStreamScope): boolean => {
    const refs = [
      [scope.ownerRef, config.maxOwnerRefCharacters],
      [scope.conversationRef, config.maxConversationRefCharacters],
      [scope.turnRef, config.maxTurnRefCharacters],
    ] as const
    return refs.every(
      ([value, limit]) =>
        value.length > 0 &&
        value === value.trim() &&
        characterCount(value) <= limit &&
        VALID_SCOPE_REF.test(value),
    )
  }

  const snapshotTurn = (
    turn: ConversationTurn,
  ): SarahConversationStreamTurnSnapshot => ({
    state: turn.state,
    eventCount: turn.eventCount,
    byteCount: turn.byteCount,
    subscribers: turn.subscribers.size,
    finalSequence: turn.finalFrame?.sequence ?? null,
  })

  const detachSubscriber = (
    turn: ConversationTurn,
    subscriber: SubscriberState,
    reason: SarahConversationStreamFanoutErrorReason,
  ) => {
    if (!subscriber.attached) return
    subscriber.attached = false
    subscriber.detachReason = reason
    turn.subscribers.delete(subscriber)
    const pending = subscriber.pending
    subscriber.pending = null
    pending?.reject(new SarahConversationStreamFanoutError(reason))
  }

  const nextFrameFor = (
    turn: ConversationTurn,
    subscriber: SubscriberState,
  ): SarahConversationStreamFrame | null =>
    turn.frames.find((frame) => frame.sequence > subscriber.cursor) ?? null

  const deliverPending = (
    turn: ConversationTurn,
    subscriber: SubscriberState,
  ) => {
    if (!subscriber.attached) return
    const latestSequence = turn.nextSequence - 1
    if (
      latestSequence - subscriber.cursor > config.maxSubscriberLagEvents
    ) {
      detachSubscriber(turn, subscriber, "slow_consumer")
      return
    }
    const pending = subscriber.pending
    if (pending === null) return
    const frame = nextFrameFor(turn, subscriber)
    if (frame === null) return
    subscriber.pending = null
    subscriber.cursor = frame.sequence
    if (frame.kind !== "chunk") {
      subscriber.attached = false
      subscriber.detachReason = "subscriber_detached"
      turn.subscribers.delete(subscriber)
    }
    pending.resolve(frame)
  }

  const notifySubscribers = (turn: ConversationTurn) => {
    for (const subscriber of [...turn.subscribers]) {
      deliverPending(turn, subscriber)
    }
  }

  const expireReplay = (turn: ConversationTurn) => {
    if (turns.get(turn.key) !== turn) return
    for (const subscriber of [...turn.subscribers]) {
      detachSubscriber(turn, subscriber, "replay_expired")
    }
    turn.ageTask?.cancel()
    turn.replayExpiryTask?.cancel()
    turn.ageTask = null
    turn.replayExpiryTask = null
    turns.delete(turn.key)
  }

  const appendChunk = (
    turn: ConversationTurn,
    chunk: string,
    eventBytes: number,
    observedAtMs: number,
  ): SarahConversationStreamChunkFrame => {
    const frame: SarahConversationStreamChunkFrame = {
      sequence: turn.nextSequence,
      observedAtMs,
      kind: "chunk",
      chunk,
      byteLength: eventBytes,
    }
    turn.nextSequence += 1
    turn.eventCount += 1
    turn.byteCount += eventBytes
    turn.frames.push(frame)
    notifySubscribers(turn)
    return frame
  }

  const terminalize = (
    turn: ConversationTurn,
    requestedOutcome: SarahConversationStreamTerminalOutcome,
    knownSettledAtMs?: number,
  ): Promise<SarahConversationStreamTerminalFrame> => {
    if (turn.terminalizing !== null) return turn.terminalizing
    if (turn.finalFrame !== null) return Promise.resolve(turn.finalFrame)
    let settledAtMs = knownSettledAtMs
    if (settledAtMs === undefined) {
      try {
        settledAtMs = readNow()
      } catch (error) {
        return Promise.reject(error)
      }
    }
    turn.state = "settling"
    turn.ageTask?.cancel()
    turn.ageTask = null
    if (requestedOutcome.kind !== "terminal") {
      turn.abortController.abort()
    }

    turn.terminalizing = (async () => {
      let outcome = requestedOutcome
      const record: SarahConversationStreamCanonicalRecord = {
        schema: "sarah.conversation_stream_record.v1",
        scope: turn.scope,
        openedAtMs: turn.openedAtMs,
        settledAtMs,
        outcome,
        finalSequence: turn.nextSequence,
        eventCount: turn.eventCount,
        byteCount: turn.byteCount,
        chunks: turn.frames.filter(
          (frame): frame is SarahConversationStreamChunkFrame =>
            frame.kind === "chunk",
        ),
      }
      const recordAbortController = new AbortController()
      const recordResult = Promise.resolve()
        .then(() => turn.publishAndRecord(record, recordAbortController.signal))
        .then(
          () => "recorded" as const,
          () => "failed" as const,
        )
      let recordTimeoutTriggered = false
      let resolveRecordTimeout!: (state: "timed_out") => void
      const recordTimeout = new Promise<"timed_out">((resolve) => {
        resolveRecordTimeout = resolve
      })
      const recordTimeoutTask = dependencies.schedule(
        config.recordTimeoutMs,
        () => {
          recordTimeoutTriggered = true
          resolveRecordTimeout("timed_out")
          recordAbortController.abort()
        },
      )
      const recordState = await Promise.race([
        recordResult,
        recordTimeout,
      ])
      recordTimeoutTask.cancel()
      if (recordTimeoutTriggered || recordState === "timed_out") {
        outcome = { kind: "error", reason: "record_timeout" }
        turn.abortController.abort()
      } else if (recordState === "failed") {
        outcome = { kind: "error", reason: "record_failed" }
        turn.abortController.abort()
      }

      const frame: SarahConversationStreamTerminalFrame = {
        sequence: turn.nextSequence,
        observedAtMs: settledAtMs,
        ...outcome,
      }
      turn.nextSequence += 1
      turn.frames.push(frame)
      turn.finalFrame = frame
      turn.state = stateForOutcome(outcome)
      notifySubscribers(turn)
      turn.replayExpiryTask = dependencies.schedule(
        config.maxReplayAgeMs,
        () => expireReplay(turn),
      )
      turn.resolveSettled(frame)
      return frame
    })()
    return turn.terminalizing
  }

  const overflow = (
    turn: ConversationTurn,
    limit: Extract<
      SarahConversationStreamTerminalOutcome,
      { kind: "overflow" }
    >["limit"],
    observedAtMs: number,
  ): never => {
    void terminalize(turn, { kind: "overflow", limit }, observedAtMs)
    return throwError("stream_overflow")
  }

  const startTurn: SarahConversationStreamFanout["startTurn"] = ({
    scope,
    publishAndRecord,
  }) => {
    if (closed) return throwError("service_closed")
    if (!validScope(scope)) return throwError("invalid_scope")
    const key = streamKey(scope)
    if (turns.has(key)) return throwError("turn_already_exists")
    if (turns.size >= config.maxTurns) return throwError("turn_limit")

    let resolveSettled!: (
      frame: SarahConversationStreamTerminalFrame,
    ) => void
    const settled = new Promise<SarahConversationStreamTerminalFrame>(
      (resolve) => {
        resolveSettled = resolve
      },
    )
    const openedAtMs = readNow()
    const turn: ConversationTurn = {
      key,
      scope: { ...scope },
      openedAtMs,
      publishAndRecord,
      frames: [],
      subscribers: new Set(),
      abortController: new AbortController(),
      settled,
      resolveSettled,
      state: "active",
      eventCount: 0,
      byteCount: 0,
      nextSequence: 1,
      finalFrame: null,
      terminalizing: null,
      ageTask: null,
      replayExpiryTask: null,
      lastObservedAtMs: openedAtMs,
    }
    turns.set(key, turn)
    turn.ageTask = dependencies.schedule(config.maxTurnAgeMs, () => {
      if (turn.state === "active") {
        let observedAtMs: number
        let outcome: SarahConversationStreamTerminalOutcome
        try {
          observedAtMs = readNow()
          outcome = { kind: "overflow", limit: "turn_age" }
        } catch {
          observedAtMs = cappedSafeAdd(
            turn.openedAtMs,
            config.maxTurnAgeMs,
          )
          outcome = { kind: "error", reason: "invalid_clock" }
        }
        void terminalize(turn, outcome, observedAtMs)
      }
    })

    const publish = (chunk: string): SarahConversationStreamChunkFrame => {
      if (closed || turn.state !== "active") {
        return throwError("stream_not_active")
      }
      if (chunk.length === 0) return throwError("empty_event")
      const observedAtMs = readNow()
      turn.lastObservedAtMs = Math.max(turn.lastObservedAtMs, observedAtMs)
      if (observedAtMs - turn.openedAtMs >= config.maxTurnAgeMs) {
        return overflow(turn, "turn_age", observedAtMs)
      }
      const eventBytes = byteLength(chunk)
      if (eventBytes > config.maxEventBytes) {
        return overflow(turn, "event_bytes", observedAtMs)
      }
      if (turn.eventCount >= config.maxEventsPerTurn) {
        return overflow(turn, "event_count", observedAtMs)
      }
      if (turn.byteCount + eventBytes > config.maxBytesPerTurn) {
        return overflow(turn, "stream_bytes", observedAtMs)
      }
      return appendChunk(turn, chunk, eventBytes, observedAtMs)
    }

    return {
      scope: turn.scope,
      signal: turn.abortController.signal,
      settled,
      publish,
      complete: () => terminalize(turn, { kind: "terminal", terminal: "complete" }),
      fail: () => terminalize(turn, { kind: "error", reason: "producer_failed" }),
      abort: () => terminalize(turn, { kind: "aborted" }),
      snapshot: () => snapshotTurn(turn),
    }
  }

  const subscribe: SarahConversationStreamFanout["subscribe"] = (request) => {
    if (closed) return throwError("service_closed")
    if (!validScope(request)) return throwError("invalid_scope")
    const turn = turns.get(streamKey(request))
    if (turn === undefined) return throwError("stream_not_available")
    if (turn.subscribers.size >= config.maxSubscribersPerTurn) {
      return throwError("subscriber_limit")
    }
    const latestSequence = turn.nextSequence - 1
    const replayFloor = Math.max(
      0,
      latestSequence - config.maxSubscriberLagEvents,
    )
    const afterSequence = request.afterSequence ?? replayFloor
    if (
      !Number.isSafeInteger(afterSequence) ||
      afterSequence < 0 ||
      afterSequence > latestSequence
    ) {
      return throwError("invalid_cursor")
    }
    if (afterSequence < replayFloor) {
      return throwError("replay_unavailable")
    }

    const subscriber: SubscriberState = {
      cursor: afterSequence,
      attached: true,
      detachReason: "subscriber_detached",
      pending: null,
    }
    turn.subscribers.add(subscriber)

    const next = (): Promise<SarahConversationStreamFrame> => {
      if (!subscriber.attached) return rejected(subscriber.detachReason)
      if (subscriber.pending !== null) return rejected("subscriber_busy")
      const latest = turn.nextSequence - 1
      if (latest - subscriber.cursor > config.maxSubscriberLagEvents) {
        detachSubscriber(turn, subscriber, "slow_consumer")
        return rejected("slow_consumer")
      }
      const frame = nextFrameFor(turn, subscriber)
      if (frame !== null) {
        subscriber.cursor = frame.sequence
        if (frame.kind !== "chunk") {
          subscriber.attached = false
          subscriber.detachReason = "subscriber_detached"
          turn.subscribers.delete(subscriber)
        }
        return Promise.resolve(frame)
      }
      return new Promise<SarahConversationStreamFrame>((resolve, reject) => {
        subscriber.pending = { resolve, reject }
      })
    }

    return {
      replayFromSequence: afterSequence + 1,
      next,
      detach: () =>
        detachSubscriber(turn, subscriber, "subscriber_detached"),
    }
  }

  const snapshot = (): SarahConversationStreamFanoutSnapshot => {
    const allTurns = [...turns.values()]
    return {
      closed,
      turns: allTurns.length,
      activeTurns: allTurns.filter(
        (turn) => turn.state === "active" || turn.state === "settling",
      ).length,
      retainedTurns: allTurns.filter(
        (turn) => turn.state !== "active" && turn.state !== "settling",
      ).length,
      subscribers: allTurns.reduce(
        (count, turn) => count + turn.subscribers.size,
        0,
      ),
      events: allTurns.reduce(
        (count, turn) => count + turn.eventCount,
        0,
      ),
      bytes: allTurns.reduce(
        (count, turn) => count + turn.byteCount,
        0,
      ),
    }
  }

  const close = (): Promise<void> => {
    if (closePromise !== null) return closePromise
    closed = true
    closePromise = (async () => {
      const activeTurns = [...turns.values()].filter(
        (turn) => turn.state === "active",
      )
      let closeTimestamp: number | null = null
      if (activeTurns.length > 0) {
        try {
          closeTimestamp = readNow()
        } catch {
          closeTimestamp = null
        }
      }
      await Promise.all(
        [...turns.values()].map((turn) =>
          turn.state === "active"
            ? terminalize(
                turn,
                closeTimestamp === null
                  ? { kind: "error", reason: "invalid_clock" }
                  : { kind: "closed" },
                closeTimestamp ?? turn.lastObservedAtMs,
              )
            : turn.terminalizing ?? Promise.resolve(turn.finalFrame!),
        ),
      )
      for (const turn of turns.values()) {
        turn.ageTask?.cancel()
        turn.replayExpiryTask?.cancel()
        turn.ageTask = null
        turn.replayExpiryTask = null
        for (const subscriber of [...turn.subscribers]) {
          detachSubscriber(turn, subscriber, "service_closed")
        }
      }
      turns.clear()
    })()
    return closePromise
  }

  return { startTurn, subscribe, snapshot, close }
}
