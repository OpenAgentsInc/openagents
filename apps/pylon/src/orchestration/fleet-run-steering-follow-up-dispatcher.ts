import { createHash } from "node:crypto"

import type {
  FleetRunSteeringFollowUpCompletion,
  FleetRunSteeringQueuedFollowUp,
  PylonOrchestrationStore,
} from "./store.js"

const DEFAULT_INTERVAL_MS = 1_000
const DEFAULT_LEASE_MS = 30_000
const MAX_BACKOFF_MS = 30_000

export type PylonFleetRunExactAttempt = {
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly workUnitRef: string
  readonly workClaimRef: string
  readonly assignmentRef: string
}

export type PylonFleetRunSteeringIntentIdentity = {
  readonly seq: number
  readonly intentId: string
  /** Stable before execution so an external control adapter can deduplicate. */
  readonly completionContractRef: string
}

export type PylonFleetRunAttemptControlResult =
  | { readonly state: "applied" }
  | { readonly state: "retry"; readonly failureRef: string }
  | { readonly state: "failed"; readonly failureRef: string }
  | { readonly state: "stale"; readonly failureRef: string }

/**
 * Production-owned exact-attempt control port. Implementations receive the
 * exact durable claim + assignment binding; there is no "latest attempt"
 * fallback and no prompt/tool argument crosses this boundary.
 */
export type PylonFleetRunAttemptControl = {
  readonly applyApproval: (input: PylonFleetRunExactAttempt & {
    readonly intent: PylonFleetRunSteeringIntentIdentity
    readonly approvalRef: string
    readonly decision: "allow" | "deny"
  }) => Promise<PylonFleetRunAttemptControlResult>
  readonly applySteer: (input: PylonFleetRunExactAttempt & {
    readonly intent: PylonFleetRunSteeringIntentIdentity
    /** Owner-private local material. Implementations must not project it. */
    readonly body: string | null
    readonly bodyRef: string | null
  }) => Promise<PylonFleetRunAttemptControlResult>
  readonly observeStop: (input: {
    readonly pylonRef: string
    readonly runRef: string
    readonly claimRef: string
    readonly intent: PylonFleetRunSteeringIntentIdentity
    readonly attempts: readonly PylonFleetRunExactAttempt[]
  }) => Promise<PylonFleetRunAttemptControlResult>
}

export type PylonFleetRunSteeringFollowUpCompletionSink = (
  completion: FleetRunSteeringFollowUpCompletion,
) => Promise<void>

export type PylonFleetRunSteeringFollowUpDispatcherOptions = {
  readonly store: PylonOrchestrationStore
  readonly control: PylonFleetRunAttemptControl
  readonly pylonRef: string
  readonly runRef: string
  readonly claimRef: string
  readonly onCompletion?: PylonFleetRunSteeringFollowUpCompletionSink | undefined
  readonly now?: (() => Date) | undefined
  readonly intervalMs?: number | undefined
  readonly leaseMs?: number | undefined
  readonly startImmediately?: boolean | undefined
}

export type PylonFleetRunSteeringFollowUpTickResult = {
  readonly ok: boolean
  readonly dispatched: number
  readonly completionsDelivered: number
  readonly pending: number
  readonly failure: "local_store_failed" | "control_failed" | "completion_delivery_failed" | null
}

const canonicalJson = (value: unknown): string => {
  if (value === null || typeof value !== "object") return JSON.stringify(value) ?? "null"
  if (Array.isArray(value)) return `[${value.map(canonicalJson).join(",")}]`
  const record = value as Record<string, unknown>
  return `{${Object.keys(record).sort().map(key =>
    `${JSON.stringify(key)}:${canonicalJson(record[key])}`
  ).join(",")}}`
}

const completionRefFor = (
  followUp: FleetRunSteeringQueuedFollowUp,
  state: FleetRunSteeringFollowUpCompletion["state"],
  completedAt: string,
): string => `completion.pylon.fleet_steering.${createHash("sha256")
  .update(canonicalJson({
    schema: "openagents.pylon.fleet_steering_follow_up_completion.v1",
    pylonRef: followUp.pylonRef,
    runRef: followUp.runRef,
    claimRef: followUp.claimRef,
    seq: followUp.seq,
    intentId: followUp.intentId,
    state,
    completedAt,
  }))
  .digest("hex")
  .slice(0, 24)}`

const completionContractRefFor = (
  followUp: FleetRunSteeringQueuedFollowUp,
): string => `contract.pylon.fleet_steering_completion.${createHash("sha256")
  .update(canonicalJson({
    schema: "openagents.pylon.fleet_steering_follow_up_completion_contract.v1",
    pylonRef: followUp.pylonRef,
    runRef: followUp.runRef,
    claimRef: followUp.claimRef,
    seq: followUp.seq,
    intentId: followUp.intentId,
  }))
  .digest("hex")
  .slice(0, 24)}`

const exactAttempt = (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
  followUp: FleetRunSteeringQueuedFollowUp,
): PylonFleetRunExactAttempt | "stale" | "invalid" => {
  if (
    followUp.workUnitRef === null ||
    followUp.workClaimRef === null ||
    followUp.assignmentRef === null
  ) return "invalid"
  const claim = options.store.getWorkClaim(followUp.workClaimRef)
  if (
    claim === null ||
    claim.runRef !== followUp.runRef ||
    claim.workUnitRef !== followUp.workUnitRef ||
    claim.assignmentRef !== followUp.assignmentRef
  ) return "invalid"
  if (claim.state === "closeout" || claim.state === "released" || claim.state === "expired") {
    return "stale"
  }
  return {
    pylonRef: followUp.pylonRef,
    runRef: followUp.runRef,
    claimRef: followUp.claimRef,
    workUnitRef: followUp.workUnitRef,
    workClaimRef: followUp.workClaimRef,
    assignmentRef: followUp.assignmentRef,
  }
}

const stopAttempts = (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
  followUp: FleetRunSteeringQueuedFollowUp,
): readonly PylonFleetRunExactAttempt[] | "invalid" => {
  if (followUp.residualRefs.length % 2 !== 0) return "invalid"
  const attempts: PylonFleetRunExactAttempt[] = []
  for (let index = 0; index < followUp.residualRefs.length; index += 2) {
    const workClaimRef = followUp.residualRefs[index]
    const assignmentRef = followUp.residualRefs[index + 1]
    if (workClaimRef === undefined || assignmentRef === undefined) return "invalid"
    const claim = options.store.getWorkClaim(workClaimRef)
    if (
      claim === null || claim.runRef !== followUp.runRef ||
      claim.assignmentRef !== assignmentRef
    ) return "invalid"
    if (claim.state === "claimed" || claim.state === "in_progress") {
      attempts.push({
        pylonRef: followUp.pylonRef,
        runRef: followUp.runRef,
        claimRef: followUp.claimRef,
        workUnitRef: claim.workUnitRef,
        workClaimRef,
        assignmentRef,
      })
    }
  }
  return attempts
}

const dispatchFollowUp = async (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
  followUp: FleetRunSteeringQueuedFollowUp,
): Promise<PylonFleetRunAttemptControlResult> => {
  const intent = {
    seq: followUp.seq,
    intentId: followUp.intentId,
    completionContractRef: completionContractRefFor(followUp),
  } satisfies PylonFleetRunSteeringIntentIdentity
  if (followUp.intentKind === "fleet_run_control") {
    const run = options.store.getFleetRun(followUp.runRef)
    if (run === null) {
      return { state: "stale", failureRef: "blocker.pylon.fleet_steering.run_missing" }
    }
    if (run.state !== "stopped") {
      return { state: "failed", failureRef: "blocker.pylon.fleet_steering.stop_state_lost" }
    }
    const attempts = stopAttempts(options, followUp)
    if (attempts === "invalid") {
      return { state: "failed", failureRef: "blocker.pylon.fleet_steering.stop_binding_invalid" }
    }
    // The stop mutation already prevents refill. Completion waits for every
    // exact residual attempt to become terminal; the production control port
    // may additionally observe/request safe cancellation where supported.
    if (attempts.length === 0) return { state: "applied" }
    return await options.control.observeStop({
      pylonRef: followUp.pylonRef,
      runRef: followUp.runRef,
      claimRef: followUp.claimRef,
      intent,
      attempts,
    })
  }
  const attempt = exactAttempt(options, followUp)
  if (attempt === "invalid") {
    return { state: "failed", failureRef: "blocker.pylon.fleet_steering.attempt_binding_invalid" }
  }
  if (attempt === "stale") {
    return { state: "stale", failureRef: "blocker.pylon.fleet_steering.attempt_terminal" }
  }
  if (followUp.intentKind === "approval_decision") {
    if (followUp.approvalRef === null || followUp.decision === null) {
      return { state: "failed", failureRef: "blocker.pylon.fleet_steering.approval_binding_invalid" }
    }
    const binding = options.store.getFleetRunSteeringApprovalBinding(followUp.approvalRef)
    if (
      binding === null || binding.state !== "pending" ||
      binding.pylonRef !== attempt.pylonRef || binding.runRef !== attempt.runRef ||
      binding.claimRef !== attempt.claimRef || binding.workUnitRef !== attempt.workUnitRef ||
      binding.workClaimRef !== attempt.workClaimRef || binding.assignmentRef !== attempt.assignmentRef
    ) return { state: "failed", failureRef: "blocker.pylon.fleet_steering.approval_binding_invalid" }
    return await options.control.applyApproval({
      ...attempt,
      intent,
      approvalRef: followUp.approvalRef,
      decision: followUp.decision,
    })
  }
  return await options.control.applySteer({
    ...attempt,
    intent,
    body: followUp.body,
    bodyRef: followUp.bodyRef,
  })
}

const flushCompletions = async (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
): Promise<number> => {
  if (options.onCompletion === undefined) return 0
  const pending = options.store.listFleetRunSteeringFollowUpCompletionOutbox({
    pylonRef: options.pylonRef,
    runRef: options.runRef,
    claimRef: options.claimRef,
    limit: 64,
  })
  let delivered = 0
  for (const completion of pending) {
    await options.onCompletion(completion)
    options.store.markFleetRunSteeringFollowUpCompletionsDelivered(
      [completion],
      options.now?.() ?? new Date(),
    )
    delivered += 1
  }
  return delivered
}

export const tickPylonFleetRunSteeringFollowUpDispatcher = async (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
): Promise<PylonFleetRunSteeringFollowUpTickResult> => {
  let completionsDelivered = 0
  try {
    completionsDelivered = await flushCompletions(options)
  } catch {
    return {
      ok: false,
      dispatched: 0,
      completionsDelivered,
      pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
      failure: "completion_delivery_failed",
    }
  }
  let followUp: FleetRunSteeringQueuedFollowUp | null
  const now = options.now?.() ?? new Date()
  try {
    followUp = options.store.acquireFleetRunSteeringFollowUp({
      pylonRef: options.pylonRef,
      runRef: options.runRef,
      claimRef: options.claimRef,
      now,
      leaseMs: options.leaseMs ?? DEFAULT_LEASE_MS,
    })
  } catch {
    return {
      ok: false,
      dispatched: 0,
      completionsDelivered,
      pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
      failure: "local_store_failed",
    }
  }
  if (followUp === null) {
    return {
      ok: true,
      dispatched: 0,
      completionsDelivered,
      pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
      failure: null,
    }
  }
  let result: PylonFleetRunAttemptControlResult
  try {
    result = await dispatchFollowUp(options, followUp)
  } catch {
    result = { state: "retry", failureRef: "blocker.pylon.fleet_steering.control_unavailable" }
  }
  try {
    if (result.state === "retry") {
      const exponent = Math.min(5, Math.max(0, followUp.attemptCount - 1))
      const delayMs = Math.min(MAX_BACKOFF_MS, 1_000 * (2 ** exponent))
      options.store.retryFleetRunSteeringFollowUp({
        followUp,
        nextAttemptAt: new Date(now.getTime() + delayMs),
        failureRef: result.failureRef,
      })
    } else {
      const completedAt = options.now?.() ?? new Date()
      const state = result.state
      options.store.completeFleetRunSteeringFollowUp({
        followUp,
        state,
        completionRef: completionRefFor(followUp, state, completedAt.toISOString()),
        ...(state === "applied" ? {} : { failureRef: result.failureRef }),
        completedAt,
      })
      try {
        completionsDelivered += await flushCompletions(options)
      } catch {
        return {
          ok: false,
          dispatched: 1,
          completionsDelivered,
          pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
          failure: "completion_delivery_failed",
        }
      }
    }
  } catch {
    return {
      ok: false,
      dispatched: 1,
      completionsDelivered,
      pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
      failure: "local_store_failed",
    }
  }
  return {
    ok: result.state !== "retry",
    dispatched: 1,
    completionsDelivered,
    pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
    failure: result.state === "retry" ? "control_failed" : null,
  }
}

export type PylonFleetRunSteeringFollowUpDispatcher = {
  readonly tick: () => Promise<PylonFleetRunSteeringFollowUpTickResult>
  readonly close: () => Promise<void>
}

export const openPylonFleetRunSteeringFollowUpDispatcher = (
  options: PylonFleetRunSteeringFollowUpDispatcherOptions,
): PylonFleetRunSteeringFollowUpDispatcher => {
  const intervalMs = options.intervalMs ?? DEFAULT_INTERVAL_MS
  if (!Number.isInteger(intervalMs) || intervalMs < 250 || intervalMs > 60_000) {
    throw new Error("Pylon FleetRun steering follow-up interval must be between 250 and 60000ms")
  }
  let closed = false
  let inFlight: Promise<PylonFleetRunSteeringFollowUpTickResult> | null = null
  const tick = (): Promise<PylonFleetRunSteeringFollowUpTickResult> => {
    if (closed) return Promise.resolve({
      ok: false,
      dispatched: 0,
      completionsDelivered: 0,
      pending: options.store.listFleetRunSteeringQueuedFollowUps(options).length,
      failure: "control_failed",
    })
    if (inFlight !== null) return inFlight
    const current = tickPylonFleetRunSteeringFollowUpDispatcher(options)
    inFlight = current
    void current.finally(() => {
      if (inFlight === current) inFlight = null
    })
    return current
  }
  const timer = setInterval(() => void tick(), intervalMs)
  timer.unref?.()
  if (options.startImmediately !== false) void tick()
  return {
    tick,
    close: async () => {
      closed = true
      clearInterval(timer)
      await inFlight?.catch(() => undefined)
    },
  }
}

export type PylonFleetRunSteeringFollowUpDispatcherFactory = (
  input: Readonly<{
    store: PylonOrchestrationStore
    pylonRef: string
    runRef: string
    claimRef: string
  }>,
) => PylonFleetRunSteeringFollowUpDispatcher | Promise<PylonFleetRunSteeringFollowUpDispatcher>
