import type { GemmaContent, GemmaStreamEvent } from "./google-inference.ts"
import {
  makeSarahConversationStreamFanout,
  type SarahConversationStreamCanonicalRecord,
  type SarahConversationStreamFanout,
  type SarahConversationStreamFanoutConfig,
  type SarahConversationStreamFanoutDependencies,
  type SarahConversationStreamPublisher,
  type SarahConversationStreamSubscriber,
  type SarahConversationStreamTerminalFrame,
} from "./conversation-stream-fanout.ts"
import {
  makeSarahVoiceFragmentCoalescer,
  type SarahVoiceFragmentBatch,
  type SarahVoiceFragmentCoalescer,
  type SarahVoiceFragmentCoalescerConfig,
  type SarahVoiceFragmentCoalescerDependencies,
} from "./voice-fragment-coalescer.ts"

/**
 * The avatar-brain boundary currently has no distinct OpenAuth owner identity.
 * `prospectRef` is therefore the exact server-minted prospect/visitor scope,
 * while `conversationRef` is the exact renderer conversation inside it. Both
 * must come from authenticated request metadata, never from model messages.
 */
export type SarahTrustedVoiceStreamScope = Readonly<{
  prospectRef: string
  conversationRef: string
}>

export type SarahTrustedVoiceInference = Readonly<{
  system: string
  contents: ReadonlyArray<GemmaContent>
}>

export type SarahTrustedVoiceCompletion = Readonly<{
  scope: SarahTrustedVoiceStreamScope
  turnRef: string
  userText: string
  assistantText: string
  canonicalRecord: SarahConversationStreamCanonicalRecord
}>

export type SarahTrustedVoicePublishAndRecord = (
  completion: SarahTrustedVoiceCompletion,
  signal: AbortSignal,
) => Promise<void> | void

export type SarahVoiceStreamCoordinatorLease = Readonly<{
  turnRef: string
  subscriber: SarahConversationStreamSubscriber
}>

export type SarahVoiceStreamCoordinatorErrorReason =
  | "conversation_busy"
  | "fragment_rejected"
  | "group_unavailable"
  | "service_closed"

const COORDINATOR_ERROR_MESSAGES = {
  conversation_busy: "Conversation already has an executing voice stream.",
  fragment_rejected: "Voice stream fragment was rejected.",
  group_unavailable: "Voice stream group is unavailable.",
  service_closed: "Sarah voice stream coordinator is closed.",
} as const satisfies Record<
  SarahVoiceStreamCoordinatorErrorReason,
  string
>

/** Fixed public-safe integration failure; no scope or provider data is echoed. */
export class SarahVoiceStreamCoordinatorError extends Error {
  readonly _tag = "SarahVoiceStreamCoordinatorError"
  override readonly name = "SarahVoiceStreamCoordinatorError"

  constructor(readonly reason: SarahVoiceStreamCoordinatorErrorReason) {
    super(COORDINATOR_ERROR_MESSAGES[reason])
  }
}

export type SarahVoiceStreamCoordinatorSnapshot = Readonly<{
  closed: boolean
  pendingTurns: number
  executingTurns: number
  fanoutTurns: number
  subscribers: number
}>

export type SarahVoiceStreamCoordinator = Readonly<{
  /**
   * A normal open without `replayTurnRef` starts or joins the one pending
   * cumulative VAD group for this exact scope. Supplying `replayTurnRef`
   * performs delivery-only replay and can never launch or record a model turn.
   */
  open: (input: Readonly<{
    scope: SarahTrustedVoiceStreamScope
    fragment: string
    inference: SarahTrustedVoiceInference
    publishAndRecord: SarahTrustedVoicePublishAndRecord
    replayTurnRef?: string
    afterSequence?: number
  }>) => SarahVoiceStreamCoordinatorLease
  snapshot: () => SarahVoiceStreamCoordinatorSnapshot
  close: () => Promise<void>
}>

export type SarahVoiceStreamCoordinatorDependencies = Readonly<{
  streamReply: (
    inference: SarahTrustedVoiceInference,
    signal: AbortSignal,
  ) => AsyncIterable<GemmaStreamEvent>
  /** Must classify to fixed copy; raw provider errors must never be returned. */
  fallbackReply: (providerError: string | null) => string
  makeTurnRef?: () => string
  coalescerConfig?: SarahVoiceFragmentCoalescerConfig
  coalescerDependencies?: SarahVoiceFragmentCoalescerDependencies
  fanoutConfig?: SarahConversationStreamFanoutConfig
  fanoutDependencies?: SarahConversationStreamFanoutDependencies
}>

type PendingRequest = Readonly<{
  fragment: string
  inference: SarahTrustedVoiceInference
  publishAndRecord: SarahTrustedVoicePublishAndRecord
}>

type ActiveTurn = {
  readonly scope: SarahTrustedVoiceStreamScope
  readonly scopeKey: string
  readonly groupRef: string
  readonly turnRef: string
  readonly publisher: SarahConversationStreamPublisher
  latest: PendingRequest
  canonicalFragment: string | null
  phase: "pending" | "executing"
  joinSettled: boolean
  publisherSettled: boolean
  joinResult: Promise<SarahConversationStreamTerminalFrame> | null
}

const exactScopeKey = (scope: SarahTrustedVoiceStreamScope): string =>
  `${scope.prospectRef.length}:${scope.prospectRef}${scope.conversationRef.length}:${scope.conversationRef}`

const latestUserContents = (
  contents: ReadonlyArray<GemmaContent>,
  fragment: string,
): GemmaContent[] => {
  const next = contents.map((content) => ({
    role: content.role,
    parts: content.parts.map((part) => ({ ...part })),
  }))
  for (let index = next.length - 1; index >= 0; index -= 1) {
    if (next[index]?.role !== "user") continue
    next[index] = { role: "user", parts: [{ text: fragment }] }
    return next
  }
  next.push({ role: "user", parts: [{ text: fragment }] })
  return next
}

const linkedAbortSignal = (
  signals: ReadonlyArray<AbortSignal>,
): Readonly<{ signal: AbortSignal; cleanup: () => void }> => {
  const controller = new AbortController()
  const abort = () => controller.abort()
  const listening: AbortSignal[] = []
  for (const signal of signals) {
    if (signal.aborted) {
      abort()
      break
    }
    signal.addEventListener("abort", abort, { once: true })
    listening.push(signal)
  }
  return {
    signal: controller.signal,
    cleanup: () => {
      for (const signal of listening) {
        signal.removeEventListener("abort", abort)
      }
    },
  }
}

export function makeSarahVoiceStreamCoordinator(
  dependencies: SarahVoiceStreamCoordinatorDependencies,
): SarahVoiceStreamCoordinator {
  const fanout: SarahConversationStreamFanout =
    makeSarahConversationStreamFanout({
      ...(dependencies.fanoutConfig
        ? { config: dependencies.fanoutConfig }
        : {}),
      ...(dependencies.fanoutDependencies
        ? { dependencies: dependencies.fanoutDependencies }
        : {}),
    })
  const activeByScope = new Map<string, ActiveTurn>()
  const activeByGroup = new Map<string, ActiveTurn>()
  let closed = false

  const cleanupIfSettled = (turn: ActiveTurn) => {
    if (!turn.joinSettled || !turn.publisherSettled) return
    if (activeByScope.get(turn.scopeKey) === turn) {
      activeByScope.delete(turn.scopeKey)
    }
    if (activeByGroup.get(turn.groupRef) === turn) {
      activeByGroup.delete(turn.groupRef)
    }
  }

  const execute = async (
    batch: SarahVoiceFragmentBatch,
    coalescerSignal: AbortSignal,
  ): Promise<SarahConversationStreamTerminalFrame> => {
    const turn = activeByGroup.get(batch.conversationRef)
    if (turn === undefined) {
      throw new SarahVoiceStreamCoordinatorError("group_unavailable")
    }
    turn.phase = "executing"
    turn.canonicalFragment = batch.text
    const request = turn.latest
    const inference: SarahTrustedVoiceInference = {
      system: request.inference.system,
      contents: latestUserContents(request.inference.contents, batch.text),
    }
    const linked = linkedAbortSignal([
      coalescerSignal,
      turn.publisher.signal,
    ])

    try {
      if (linked.signal.aborted) {
        return await turn.publisher.abort()
      }
      let sawDelta = false
      let providerError: string | null = null
      for await (const event of dependencies.streamReply(
        inference,
        linked.signal,
      )) {
        if (linked.signal.aborted) break
        if (event.type === "delta" && event.text.length > 0) {
          sawDelta = true
          turn.publisher.publish(event.text)
        } else if (event.type === "error") {
          providerError = event.error
        }
      }
      if (linked.signal.aborted) {
        if (turn.publisher.signal.aborted) return await turn.publisher.settled
        return await turn.publisher.abort()
      }
      if (!sawDelta) {
        turn.publisher.publish(dependencies.fallbackReply(providerError))
      }
      return await turn.publisher.complete()
    } catch {
      if (turn.publisher.signal.aborted) return await turn.publisher.settled
      if (linked.signal.aborted) return await turn.publisher.abort()
      return await turn.publisher.fail()
    } finally {
      linked.cleanup()
    }
  }

  const coalescer: SarahVoiceFragmentCoalescer<SarahConversationStreamTerminalFrame> =
    makeSarahVoiceFragmentCoalescer({
      execute,
      ...(dependencies.coalescerConfig
        ? { config: dependencies.coalescerConfig }
        : {}),
      ...(dependencies.coalescerDependencies
        ? { dependencies: dependencies.coalescerDependencies }
        : {}),
    })

  const trackTurn = (
    turn: ActiveTurn,
    joined: Promise<SarahConversationStreamTerminalFrame>,
  ) => {
    void joined.then(
      () => {
        turn.joinSettled = true
        cleanupIfSettled(turn)
      },
      () => {
        turn.joinSettled = true
        if (!turn.publisher.signal.aborted) {
          void turn.publisher.fail().catch(() => {})
        }
        cleanupIfSettled(turn)
      },
    )
    void turn.publisher.settled.then(() => {
      turn.publisherSettled = true
      cleanupIfSettled(turn)
    })
  }

  const startTurn = (
    scope: SarahTrustedVoiceStreamScope,
    request: PendingRequest,
  ): ActiveTurn => {
    const scopeKey = exactScopeKey(scope)
    const groupRef = `sarah.group.${crypto.randomUUID()}`
    if (
      !coalescer.preflight({
        conversationRef: groupRef,
        fragment: request.fragment,
      }).accepted
    ) {
      throw new SarahVoiceStreamCoordinatorError("fragment_rejected")
    }
    const turnRef =
      dependencies.makeTurnRef?.() ?? `sarah.turn.${crypto.randomUUID()}`
    let turn!: ActiveTurn
    const publisher = fanout.startTurn({
      scope: {
        ownerRef: scope.prospectRef,
        conversationRef: scope.conversationRef,
        turnRef,
      },
      publishAndRecord: async (record, signal) => {
        if (
          record.outcome.kind !== "terminal" ||
          record.outcome.terminal !== "complete"
        ) {
          return
        }
        const current = turn.latest
        // This is the sole transcript/avatar-event authority. SSE subscribers
        // consume delivery frames only and can never replay this callback.
        await current.publishAndRecord(
          {
            scope: turn.scope,
            turnRef: turn.turnRef,
            userText: turn.canonicalFragment ?? current.fragment,
            assistantText: record.chunks.map((chunk) => chunk.chunk).join(""),
            canonicalRecord: record,
          },
          signal,
        )
      },
    })
    turn = {
      scope: { ...scope },
      scopeKey,
      groupRef,
      turnRef,
      publisher,
      latest: request,
      canonicalFragment: null,
      phase: "pending",
      joinSettled: false,
      publisherSettled: false,
      joinResult: null,
    }
    activeByScope.set(scopeKey, turn)
    activeByGroup.set(groupRef, turn)
    const joined = coalescer.joinWithAcceptance({
      conversationRef: groupRef,
      fragment: request.fragment,
    })
    turn.joinResult = joined.result
    if (!joined.accepted) {
      // No controller can escape for a rejected first fragment. Remove the
      // coordinator lease immediately; the fanout receives one non-success
      // terminal solely to cancel its age timer and bounded retained state.
      activeByScope.delete(scopeKey)
      activeByGroup.delete(groupRef)
      void joined.result.catch(() => {})
      void publisher.fail().catch(() => {})
      throw new SarahVoiceStreamCoordinatorError("fragment_rejected")
    }
    trackTurn(turn, joined.result)
    return turn
  }

  const open: SarahVoiceStreamCoordinator["open"] = (input) => {
    if (closed) {
      throw new SarahVoiceStreamCoordinatorError("service_closed")
    }
    if (input.replayTurnRef !== undefined) {
      return {
        turnRef: input.replayTurnRef,
        subscriber: fanout.subscribe({
          ownerRef: input.scope.prospectRef,
          conversationRef: input.scope.conversationRef,
          turnRef: input.replayTurnRef,
          ...(input.afterSequence !== undefined
            ? { afterSequence: input.afterSequence }
            : {}),
        }),
      }
    }

    const scopeKey = exactScopeKey(input.scope)
    const request: PendingRequest = {
      fragment: input.fragment,
      inference: input.inference,
      publishAndRecord: input.publishAndRecord,
    }
    let turn = activeByScope.get(scopeKey)
    if (turn === undefined) {
      turn = startTurn(input.scope, request)
    } else if (turn.phase === "pending") {
      // The coalescer owns cumulative-fragment validation and preserves the
      // same group promise; the coordinator's publisher remains active until
      // both that group and the canonical fanout terminal have settled.
      const joined = coalescer.joinWithAcceptance({
        conversationRef: turn.groupRef,
        fragment: request.fragment,
      })
      // Consume every result even when the acceptance receipt says no. The
      // original group's tracker still owns canonical failure/cleanup.
      void joined.result.catch(() => {})
      if (!joined.accepted || joined.result !== turn.joinResult) {
        throw new SarahVoiceStreamCoordinatorError("fragment_rejected")
      }
      turn.latest = request
    } else {
      throw new SarahVoiceStreamCoordinatorError("conversation_busy")
    }

    return {
      turnRef: turn.turnRef,
      subscriber: fanout.subscribe({
        ownerRef: input.scope.prospectRef,
        conversationRef: input.scope.conversationRef,
        turnRef: turn.turnRef,
      }),
    }
  }

  return {
    open,
    snapshot: () => {
      const fanoutSnapshot = fanout.snapshot()
      const active = [...activeByScope.values()]
      return {
        closed,
        pendingTurns: active.filter((turn) => turn.phase === "pending").length,
        executingTurns: active.filter((turn) => turn.phase === "executing")
          .length,
        fanoutTurns: fanoutSnapshot.turns,
        subscribers: fanoutSnapshot.subscribers,
      }
    },
    close: async () => {
      if (closed) return
      closed = true
      coalescer.close()
      await fanout.close()
      activeByScope.clear()
      activeByGroup.clear()
    },
  }
}
