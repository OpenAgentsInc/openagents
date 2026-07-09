import { Schema } from "effect"

const PositiveInteger = Schema.Number.check(
  Schema.isFinite(),
  Schema.isInt(),
  Schema.isGreaterThan(0),
)

/** Bounded process-local policy; every limit must be explicit and positive. */
export const SarahVoiceFragmentCoalescerConfig = Schema.Struct({
  quietWindowMs: PositiveInteger,
  maxWaitMs: PositiveInteger,
  maxConversationRefCharacters: PositiveInteger,
  maxTextCharacters: PositiveInteger,
  maxFragmentsPerGroup: PositiveInteger,
  maxActiveGroups: PositiveInteger,
  executionTimeoutMs: PositiveInteger,
})
export type SarahVoiceFragmentCoalescerConfig =
  typeof SarahVoiceFragmentCoalescerConfig.Type

export const DEFAULT_SARAH_VOICE_FRAGMENT_COALESCER_CONFIG =
  SarahVoiceFragmentCoalescerConfig.make({
    quietWindowMs: 250,
    maxWaitMs: 1_000,
    maxConversationRefCharacters: 256,
    maxTextCharacters: 4_000,
    maxFragmentsPerGroup: 8,
    maxActiveGroups: 128,
    executionTimeoutMs: 60_000,
  })

export type SarahVoiceFragmentFlushReason = "quiet_window" | "max_wait"

export type SarahVoiceFragmentBatch = Readonly<{
  conversationRef: string
  /** Latest cumulative VAD transcript; earlier partials are deliberately absent. */
  text: string
  fragmentCount: number
  flushReason: SarahVoiceFragmentFlushReason
  openedAtMs: number
  flushedAtMs: number
}>

export type SarahVoiceFragmentCoalescerErrorReason =
  | "invalid_conversation_ref"
  | "conversation_ref_too_large"
  | "conversation_busy"
  | "empty_fragment"
  | "fragment_too_large"
  | "too_many_fragments"
  | "too_many_active_groups"
  | "execution_timeout"
  | "execution_failed"
  | "service_closed"

const ERROR_MESSAGES = {
  invalid_conversation_ref: "Conversation reference is invalid.",
  conversation_ref_too_large:
    "Conversation reference exceeds the configured length limit.",
  conversation_busy:
    "Conversation already has an executing voice fragment group.",
  empty_fragment: "Voice fragment is empty.",
  fragment_too_large: "Voice fragment exceeds the configured text limit.",
  too_many_fragments:
    "Voice fragment group exceeds the configured fragment limit.",
  too_many_active_groups:
    "Voice fragment coalescer is at its active-group limit.",
  execution_timeout:
    "Voice fragment execution exceeded its configured deadline.",
  execution_failed: "Voice fragment execution failed.",
  service_closed: "Voice fragment coalescer is closed.",
} as const satisfies Record<SarahVoiceFragmentCoalescerErrorReason, string>

/**
 * Public-safe typed failure. It never carries fragment text, provider errors,
 * credentials, or conversation refs; the executor's raw failure stays local.
 */
export class SarahVoiceFragmentCoalescerError extends Error {
  readonly _tag = "SarahVoiceFragmentCoalescerError"
  override readonly name = "SarahVoiceFragmentCoalescerError"

  constructor(readonly reason: SarahVoiceFragmentCoalescerErrorReason) {
    super(ERROR_MESSAGES[reason])
  }
}

export type SarahVoiceFragmentScheduledTask = Readonly<{
  cancel: () => void
}>

export type SarahVoiceFragmentCoalescerDependencies = Readonly<{
  now: () => number
  schedule: (
    delayMs: number,
    task: () => void,
  ) => SarahVoiceFragmentScheduledTask
}>

export const systemSarahVoiceFragmentCoalescerDependencies: SarahVoiceFragmentCoalescerDependencies =
  {
    now: Date.now,
    schedule: (delayMs, task) => {
      const handle = setTimeout(task, delayMs)
      return { cancel: () => clearTimeout(handle) }
    },
  }

export type SarahVoiceFragmentExecutor<Result> = (
  batch: SarahVoiceFragmentBatch,
  signal: AbortSignal,
) => Promise<Result> | Result

export type SarahVoiceFragmentCoalescerSnapshot = Readonly<{
  activeGroups: number
  pendingGroups: number
  executingGroups: number
}>

export type SarahVoiceFragmentCoalescer<Result> = Readonly<{
  /**
   * Join one cumulative VAD fragment. Callers in the same pending group receive
   * the exact same promise and executor result; completion side effects belong
   * inside the injected executor so they run once, never once per waiter.
   * Once execution begins, new text for that exact conversation fails with
   * `conversation_busy`; it never receives a reply generated for older text.
   */
  join: (input: Readonly<{
    conversationRef: string
    fragment: string
  }>) => Promise<Result>
  snapshot: () => SarahVoiceFragmentCoalescerSnapshot
  close: () => void
}>

type GroupState = "pending" | "executing" | "settled"

type VoiceFragmentGroup<Result> = {
  readonly conversationRef: string
  readonly openedAtMs: number
  readonly result: Promise<Result>
  readonly resolve: (result: Result | PromiseLike<Result>) => void
  readonly reject: (error: SarahVoiceFragmentCoalescerError) => void
  latestText: string
  fragmentCount: number
  state: GroupState
  quietTask: SarahVoiceFragmentScheduledTask | null
  maxWaitTask: SarahVoiceFragmentScheduledTask | null
  executionTimeoutTask: SarahVoiceFragmentScheduledTask | null
  abortController: AbortController | null
}

const characterCount = (text: string): number => Array.from(text).length
const VALID_CONVERSATION_REF = /^[^\s\u0000-\u001f\u007f]+$/u

const makeRejected = <Result>(
  reason: SarahVoiceFragmentCoalescerErrorReason,
): Promise<Result> => Promise.reject(new SarahVoiceFragmentCoalescerError(reason))

/**
 * Pure FC-BRAIN grouping/single-flight seam.
 *
 * It intentionally is not wired directly into `liveStreamingResponse`: a
 * promise-only integration would buffer the model reply and violate Sarah's
 * immediate SSE role/keepalive first-byte law. The bounded, exact-scope
 * delivery primitive now lives in `conversation-stream-fanout.ts`; live route
 * wiring remains separate so this grouping policy cannot accidentally turn
 * subscriber replay into another `publishAndRecord` transcript write.
 */
export function makeSarahVoiceFragmentCoalescer<Result>(input: Readonly<{
  execute: SarahVoiceFragmentExecutor<Result>
  config?: SarahVoiceFragmentCoalescerConfig
  dependencies?: SarahVoiceFragmentCoalescerDependencies
}>): SarahVoiceFragmentCoalescer<Result> {
  const config = Schema.decodeUnknownSync(SarahVoiceFragmentCoalescerConfig)(
    input.config ?? DEFAULT_SARAH_VOICE_FRAGMENT_COALESCER_CONFIG,
  )
  const dependencies =
    input.dependencies ?? systemSarahVoiceFragmentCoalescerDependencies
  const pendingByConversation = new Map<string, VoiceFragmentGroup<Result>>()
  const activeByConversation = new Map<string, VoiceFragmentGroup<Result>>()
  const activeGroups = new Set<VoiceFragmentGroup<Result>>()
  let closed = false

  const removePending = (group: VoiceFragmentGroup<Result>) => {
    if (pendingByConversation.get(group.conversationRef) === group) {
      pendingByConversation.delete(group.conversationRef)
    }
  }

  const removeActive = (group: VoiceFragmentGroup<Result>) => {
    if (activeByConversation.get(group.conversationRef) === group) {
      activeByConversation.delete(group.conversationRef)
    }
  }

  const cancelGatheringTasks = (group: VoiceFragmentGroup<Result>) => {
    group.quietTask?.cancel()
    group.maxWaitTask?.cancel()
    group.quietTask = null
    group.maxWaitTask = null
  }

  const settleFailure = (
    group: VoiceFragmentGroup<Result>,
    reason: SarahVoiceFragmentCoalescerErrorReason,
  ) => {
    if (group.state === "settled") return
    group.state = "settled"
    removePending(group)
    cancelGatheringTasks(group)
    group.executionTimeoutTask?.cancel()
    group.executionTimeoutTask = null
    group.abortController?.abort()
    removeActive(group)
    activeGroups.delete(group)
    group.reject(new SarahVoiceFragmentCoalescerError(reason))
  }

  const settleSuccess = (group: VoiceFragmentGroup<Result>, result: Result) => {
    if (group.state === "settled") return
    group.state = "settled"
    group.executionTimeoutTask?.cancel()
    group.executionTimeoutTask = null
    removeActive(group)
    activeGroups.delete(group)
    group.resolve(result)
  }

  const flush = (
    group: VoiceFragmentGroup<Result>,
    flushReason: SarahVoiceFragmentFlushReason,
  ) => {
    if (group.state !== "pending") return
    group.state = "executing"
    removePending(group)
    cancelGatheringTasks(group)
    const abortController = new AbortController()
    group.abortController = abortController
    group.executionTimeoutTask = dependencies.schedule(
      config.executionTimeoutMs,
      () => settleFailure(group, "execution_timeout"),
    )
    const batch: SarahVoiceFragmentBatch = {
      conversationRef: group.conversationRef,
      text: group.latestText,
      fragmentCount: group.fragmentCount,
      flushReason,
      openedAtMs: group.openedAtMs,
      flushedAtMs: dependencies.now(),
    }

    let execution: Promise<Result>
    try {
      execution = Promise.resolve(input.execute(batch, abortController.signal))
    } catch {
      settleFailure(group, "execution_failed")
      return
    }
    void execution.then(
      (result) => settleSuccess(group, result),
      () => settleFailure(group, "execution_failed"),
    )
  }

  const scheduleQuietFlush = (group: VoiceFragmentGroup<Result>) => {
    group.quietTask?.cancel()
    group.quietTask = dependencies.schedule(config.quietWindowMs, () =>
      flush(group, "quiet_window"),
    )
  }

  const makeGroup = (
    conversationRef: string,
    fragment: string,
  ): VoiceFragmentGroup<Result> => {
    let resolve!: (result: Result | PromiseLike<Result>) => void
    let reject!: (error: SarahVoiceFragmentCoalescerError) => void
    const result = new Promise<Result>((onResolve, onReject) => {
      resolve = onResolve
      reject = onReject
    })
    const group: VoiceFragmentGroup<Result> = {
      conversationRef,
      openedAtMs: dependencies.now(),
      result,
      resolve,
      reject,
      latestText: fragment,
      fragmentCount: 1,
      state: "pending",
      quietTask: null,
      maxWaitTask: null,
      executionTimeoutTask: null,
      abortController: null,
    }
    pendingByConversation.set(conversationRef, group)
    activeByConversation.set(conversationRef, group)
    activeGroups.add(group)
    scheduleQuietFlush(group)
    group.maxWaitTask = dependencies.schedule(config.maxWaitMs, () =>
      flush(group, "max_wait"),
    )
    return group
  }

  const join: SarahVoiceFragmentCoalescer<Result>["join"] = ({
    conversationRef,
    fragment,
  }) => {
    if (closed) return makeRejected("service_closed")
    if (
      conversationRef.length === 0 ||
      conversationRef !== conversationRef.trim() ||
      !VALID_CONVERSATION_REF.test(conversationRef)
    ) {
      return makeRejected("invalid_conversation_ref")
    }
    if (
      characterCount(conversationRef) > config.maxConversationRefCharacters
    ) {
      return makeRejected("conversation_ref_too_large")
    }
    const boundedFragment = fragment.trim()
    if (boundedFragment.length === 0) return makeRejected("empty_fragment")
    if (characterCount(boundedFragment) > config.maxTextCharacters) {
      const active = activeByConversation.get(conversationRef)
      if (active?.state === "pending") {
        settleFailure(active, "fragment_too_large")
        return active.result
      }
      return makeRejected("fragment_too_large")
    }

    const active = activeByConversation.get(conversationRef)
    if (active?.state === "executing") {
      return makeRejected("conversation_busy")
    }
    if (active?.state === "pending") {
      if (active.fragmentCount >= config.maxFragmentsPerGroup) {
        settleFailure(active, "too_many_fragments")
        return active.result
      }
      active.fragmentCount += 1
      active.latestText = boundedFragment
      scheduleQuietFlush(active)
      return active.result
    }

    if (activeGroups.size >= config.maxActiveGroups) {
      return makeRejected("too_many_active_groups")
    }
    return makeGroup(conversationRef, boundedFragment).result
  }

  return {
    join,
    snapshot: () => {
      let executingGroups = 0
      for (const group of activeGroups) {
        if (group.state === "executing") executingGroups += 1
      }
      return {
        activeGroups: activeGroups.size,
        pendingGroups: pendingByConversation.size,
        executingGroups,
      }
    },
    close: () => {
      if (closed) return
      closed = true
      for (const group of [...activeGroups]) {
        settleFailure(group, "service_closed")
      }
    },
  }
}
