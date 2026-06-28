import {
  GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
  GLM_STRESS_BACKOFF_STEP_FRACTION,
  GLM_STRESS_DEMAND_KIND,
  GLM_STRESS_DEMAND_SOURCE,
  GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD,
  type GlmStressFailureKind,
} from './stress-saturation-plan'
import { percentile } from './report'

export const GLM_LIVE_ADAPTIVE_STRESS_RUNNER_SCHEMA =
  'openagents.khala.glm_live_adaptive_stress_runner.v0_1' as const

export const GLM_LIVE_ADAPTIVE_STRESS_ARTIFACT_SCHEMA =
  'openagents.khala.glm_live_adaptive_stress_artifact.v0_1' as const

export const GLM_LIVE_ADAPTIVE_STRESS_SUPERVISOR_SCHEMA =
  'openagents.khala.glm_live_adaptive_stress_supervisor_tick.v0_1' as const

export const GLM_REAP_SERVED_MODEL = 'openagents/glm-5.2-reap-504b' as const

export const GLM_LIVE_ADAPTIVE_STRESS_WINDOW_BREAKER_MAX_OBSERVATIONS = 3

export type GlmLiveAdaptiveStressOutcomeStatus =
  | 'ok'
  | 'failed'
  | 'preempted_for_external'

export type GlmLiveAdaptiveStressDecisionAction =
  | 'hold'
  | 'increase'
  | 'decrease'
  | 'pause'

export type GlmLiveAdaptiveStressDecisionReason =
  | 'clean_window'
  | 'clean_windows_allow_probe'
  | 'empty_window'
  | 'error_rate_over_budget'
  | 'external_preemption_observed'
  | 'overload_failures_observed'

export type GlmLiveAdaptiveStressSupervisorAction =
  | 'dispatch_next_window'
  | 'yield_to_external'
  | 'complete'

export type GlmLiveAdaptiveStressSupervisorReason =
  | 'continuous_window_ready'
  | 'external_demand_active'
  | 'max_ticks_completed'
  | 'previous_backoff_carried_forward'
  | 'previous_window_clean'

export type GlmLiveAdaptiveStressObservation = Readonly<{
  requestRef: string
  status: GlmLiveAdaptiveStressOutcomeStatus
  httpStatus: number | null
  failureKind: GlmStressFailureKind | null
  servedModel: string | null
  provider: string | null
  worker: string | null
  inputTokens: number
  outputTokens: number
  totalTokens: number
  usageTruth: 'exact' | 'missing'
  ttftMs: number | null
  wallClockMs: number
}>

export type GlmLiveAdaptiveStressDecisionInput = Readonly<{
  currentConcurrency: number
  minConcurrency: number
  maxConcurrency: number
  consecutiveCleanWindows: number
  cleanWindowIncreaseThreshold: number
  observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>
}>

export type GlmLiveAdaptiveStressDecision = Readonly<{
  action: GlmLiveAdaptiveStressDecisionAction
  currentConcurrency: number
  nextConcurrency: number
  nextConsecutiveCleanWindows: number
  observedCount: number
  okCount: number
  failedCount: number
  preemptedCount: number
  overloadFailureCount: number
  observedErrorRate: number | null
  reasonRefs: ReadonlyArray<string>
}>

export type GlmLiveAdaptiveStressSummary = Readonly<{
  okCount: number
  failedCount: number
  preemptedCount: number
  nonGlmOkCount: number
  inputTokens: number
  outputTokens: number
  totalTokens: number
  nonGlmTokens: number
  failureByStatus: Readonly<Record<string, number>>
  failureByKind: Readonly<Record<string, number>>
  ttftMs: Readonly<{
    p50: number | null
    p90: number | null
    p99: number | null
  }>
}>

export type GlmLiveAdaptiveStressWindowBreakerDecision = Readonly<{
  tripped: boolean
  observedCount: number
  failedCount: number
  preemptedCount: number
  overloadFailureCount: number
  observedErrorRate: number | null
  reasonRefs: ReadonlyArray<string>
}>

export type GlmLiveAdaptiveStressWindow = Readonly<{
  startedAt: string
  completedAt: string
  decision: GlmLiveAdaptiveStressDecision
}>

export type GlmLiveAdaptiveStressArtifact = Readonly<{
  schema: typeof GLM_LIVE_ADAPTIVE_STRESS_ARTIFACT_SCHEMA
  telemetrySchema: typeof GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA
  publicSafe: true
  runId: string
  generatedAt: string
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: string
  model: string
  initialConcurrency: number
  finalConcurrency: number
  minConcurrency: number
  maxConcurrency: number
  maxTokens: number
  windowMs: number
  durationMs: number
  summary: GlmLiveAdaptiveStressSummary
  windows: ReadonlyArray<GlmLiveAdaptiveStressWindow>
  observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>
}>

export type GlmLiveAdaptiveStressSupervisorTick = Readonly<{
  schema: typeof GLM_LIVE_ADAPTIVE_STRESS_SUPERVISOR_SCHEMA
  telemetrySchema: typeof GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA
  publicSafe: true
  generatedAt: string
  runIdPrefix: string
  tickIndex: number
  maxTicks: number
  cadenceMs: number
  action: GlmLiveAdaptiveStressSupervisorAction
  nextRunId: string | null
  nextEarliestStartAt: string | null
  demandKind: typeof GLM_STRESS_DEMAND_KIND
  demandSource: typeof GLM_STRESS_DEMAND_SOURCE
  demandClient: string
  model: string
  minConcurrency: number
  maxConcurrency: number
  recommendedInitialConcurrency: number
  previousRunId: string | null
  previousFinalConcurrency: number | null
  previousSummary: GlmLiveAdaptiveStressSummary | null
  reasonRefs: ReadonlyArray<string>
}>

const positiveIntegerOr = (value: number, fallback: number): number =>
  Number.isFinite(value) && value > 0 ? Math.floor(value) : fallback

const boundedConcurrency = (
  value: number,
  minConcurrency: number,
  maxConcurrency: number,
): number =>
  Math.min(
    positiveIntegerOr(maxConcurrency, 1),
    Math.max(positiveIntegerOr(minConcurrency, 1), positiveIntegerOr(value, 1)),
  )

const reasonRefs = (
  reasons: ReadonlyArray<GlmLiveAdaptiveStressDecisionReason>,
): ReadonlyArray<string> =>
  reasons.map(reason => `runner.glm_live_adaptive_stress.${reason}`)

const supervisorReasonRefs = (
  reasons: ReadonlyArray<GlmLiveAdaptiveStressSupervisorReason>,
): ReadonlyArray<string> =>
  reasons.map(reason => `supervisor.glm_live_adaptive_stress.${reason}`)

const nextLowerConcurrency = (
  currentConcurrency: number,
  minConcurrency: number,
): number => {
  const step = Math.max(
    1,
    Math.ceil(currentConcurrency * GLM_STRESS_BACKOFF_STEP_FRACTION),
  )
  return Math.max(positiveIntegerOr(minConcurrency, 1), currentConcurrency - step)
}

export const glmLiveAdaptiveStressHeaders = (
  demandClient: string,
  requestRef: string,
): Readonly<Record<string, string>> => ({
  'x-openagents-demand-kind': GLM_STRESS_DEMAND_KIND,
  'x-openagents-demand-source': GLM_STRESS_DEMAND_SOURCE,
  'x-openagents-client': demandClient,
  'x-openagents-request-ref': requestRef,
})

export const isGlmReapServedModel = (servedModel: string | null): boolean =>
  servedModel === GLM_REAP_SERVED_MODEL

export const classifyGlmLiveAdaptiveStressFailure = (
  httpStatus: number | null,
  curlExitCode: number,
): GlmStressFailureKind => {
  if (curlExitCode === 28) {
    return 'timeout'
  }
  if (httpStatus === 429) {
    return 'rate_limited'
  }
  if (httpStatus === 502 || httpStatus === 503 || httpStatus === 504) {
    return 'gateway_overload'
  }
  if (httpStatus !== null && httpStatus >= 500 && httpStatus <= 599) {
    return 'provider_overload'
  }
  return 'unknown'
}

const observationStats = (
  observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>,
): Readonly<{
  observedCount: number
  okCount: number
  failedCount: number
  preemptedCount: number
  overloadFailureCount: number
  observedErrorRate: number | null
}> => {
  const observedCount = observations.length
  const okCount = observations.filter(
    observation => observation.status === 'ok',
  ).length
  const failedCount = observations.filter(
    observation => observation.status === 'failed',
  ).length
  const preemptedCount = observations.filter(
    observation => observation.status === 'preempted_for_external',
  ).length
  const overloadFailureCount = observations.filter(
    observation =>
      observation.status === 'failed' &&
      (observation.failureKind === 'gateway_overload' ||
        observation.failureKind === 'provider_overload' ||
        observation.failureKind === 'rate_limited' ||
        observation.failureKind === 'timeout'),
  ).length
  const observedErrorRate =
    observedCount === 0 ? null : failedCount / observedCount

  return {
    observedCount,
    okCount,
    failedCount,
    preemptedCount,
    overloadFailureCount,
    observedErrorRate,
  }
}

export const decideGlmLiveAdaptiveStressWindowBreaker = (input: {
  readonly currentConcurrency: number
  readonly observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>
}): GlmLiveAdaptiveStressWindowBreakerDecision => {
  const {
    observedCount,
    failedCount,
    preemptedCount,
    overloadFailureCount,
    observedErrorRate,
  } = observationStats(input.observations)
  const minObservedBeforeTrip = Math.max(
    1,
    Math.min(
      positiveIntegerOr(input.currentConcurrency, 1),
      GLM_LIVE_ADAPTIVE_STRESS_WINDOW_BREAKER_MAX_OBSERVATIONS,
    ),
  )
  const reasons: Array<GlmLiveAdaptiveStressDecisionReason> = []

  if (preemptedCount > 0) {
    reasons.push('external_preemption_observed')
  }
  if (overloadFailureCount >= minObservedBeforeTrip) {
    reasons.push('overload_failures_observed')
  }
  if (
    observedCount >= minObservedBeforeTrip &&
    observedErrorRate !== null &&
    observedErrorRate > GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD
  ) {
    reasons.push('error_rate_over_budget')
  }

  return {
    tripped: reasons.length > 0,
    observedCount,
    failedCount,
    preemptedCount,
    overloadFailureCount,
    observedErrorRate,
    reasonRefs: reasonRefs([...new Set(reasons)]),
  }
}

export const decideGlmLiveAdaptiveStressConcurrency = (
  input: GlmLiveAdaptiveStressDecisionInput,
): GlmLiveAdaptiveStressDecision => {
  const currentConcurrency = boundedConcurrency(
    input.currentConcurrency,
    input.minConcurrency,
    input.maxConcurrency,
  )
  const {
    observedCount,
    okCount,
    failedCount,
    preemptedCount,
    overloadFailureCount,
    observedErrorRate,
  } = observationStats(input.observations)

  if (preemptedCount > 0) {
    return {
      action: 'pause',
      currentConcurrency,
      nextConcurrency: positiveIntegerOr(input.minConcurrency, 1),
      nextConsecutiveCleanWindows: 0,
      observedCount,
      okCount,
      failedCount,
      preemptedCount,
      overloadFailureCount,
      observedErrorRate,
      reasonRefs: reasonRefs(['external_preemption_observed']),
    }
  }

  if (observedCount === 0) {
    return {
      action: 'hold',
      currentConcurrency,
      nextConcurrency: currentConcurrency,
      nextConsecutiveCleanWindows: 0,
      observedCount,
      okCount,
      failedCount,
      preemptedCount,
      overloadFailureCount,
      observedErrorRate,
      reasonRefs: reasonRefs(['empty_window']),
    }
  }

  if (
    overloadFailureCount > 0 ||
    (observedErrorRate !== null &&
      observedErrorRate > GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD)
  ) {
    const reasons: Array<GlmLiveAdaptiveStressDecisionReason> = [
      ...(observedErrorRate !== null &&
      observedErrorRate > GLM_STRESS_ERROR_RATE_BACKOFF_THRESHOLD
        ? (['error_rate_over_budget'] as const)
        : []),
      ...(overloadFailureCount > 0
        ? (['overload_failures_observed'] as const)
        : []),
    ]
    return {
      action: 'decrease',
      currentConcurrency,
      nextConcurrency: nextLowerConcurrency(
        currentConcurrency,
        input.minConcurrency,
      ),
      nextConsecutiveCleanWindows: 0,
      observedCount,
      okCount,
      failedCount,
      preemptedCount,
      overloadFailureCount,
      observedErrorRate,
      reasonRefs: reasonRefs([...new Set(reasons)]),
    }
  }

  const nextCleanWindows = input.consecutiveCleanWindows + 1
  if (
    okCount > 0 &&
    nextCleanWindows >= input.cleanWindowIncreaseThreshold &&
    currentConcurrency < input.maxConcurrency
  ) {
    return {
      action: 'increase',
      currentConcurrency,
      nextConcurrency: currentConcurrency + 1,
      nextConsecutiveCleanWindows: 0,
      observedCount,
      okCount,
      failedCount,
      preemptedCount,
      overloadFailureCount,
      observedErrorRate,
      reasonRefs: reasonRefs(['clean_windows_allow_probe']),
    }
  }

  return {
    action: 'hold',
    currentConcurrency,
    nextConcurrency: currentConcurrency,
    nextConsecutiveCleanWindows: nextCleanWindows,
    observedCount,
    okCount,
    failedCount,
    preemptedCount,
    overloadFailureCount,
    observedErrorRate,
    reasonRefs: reasonRefs(['clean_window']),
  }
}

const incrementRecord = (
  record: Record<string, number>,
  key: string,
): Record<string, number> => ({
  ...record,
  [key]: (record[key] ?? 0) + 1,
})

const sum = (
  observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>,
  pick: (observation: GlmLiveAdaptiveStressObservation) => number,
): number =>
  observations.reduce((total, observation) => total + pick(observation), 0)

export const summarizeGlmLiveAdaptiveStress = (
  observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>,
): GlmLiveAdaptiveStressSummary => {
  const okObservations = observations.filter(
    observation => observation.status === 'ok',
  )
  const glmOkObservations = okObservations.filter(observation =>
    isGlmReapServedModel(observation.servedModel),
  )
  const nonGlmOkObservations = okObservations.filter(
    observation => !isGlmReapServedModel(observation.servedModel),
  )
  const failedObservations = observations.filter(
    observation => observation.status === 'failed',
  )
  const ttftValues = glmOkObservations.flatMap(observation =>
    observation.ttftMs === null ? [] : [observation.ttftMs],
  )

  return {
    okCount: okObservations.length,
    failedCount: failedObservations.length,
    preemptedCount: observations.filter(
      observation => observation.status === 'preempted_for_external',
    ).length,
    nonGlmOkCount: nonGlmOkObservations.length,
    inputTokens: sum(glmOkObservations, observation => observation.inputTokens),
    outputTokens: sum(glmOkObservations, observation => observation.outputTokens),
    totalTokens: sum(glmOkObservations, observation => observation.totalTokens),
    nonGlmTokens: sum(
      nonGlmOkObservations,
      observation => observation.totalTokens,
    ),
    failureByStatus: failedObservations.reduce(
      (record, observation) =>
        incrementRecord(record, String(observation.httpStatus ?? 'none')),
      {} as Record<string, number>,
    ),
    failureByKind: failedObservations.reduce(
      (record, observation) =>
        incrementRecord(record, observation.failureKind ?? 'unknown'),
      {} as Record<string, number>,
    ),
    ttftMs: {
      p50: percentile(ttftValues, 50),
      p90: percentile(ttftValues, 90),
      p99: percentile(ttftValues, 99),
    },
  }
}

export const buildGlmLiveAdaptiveStressArtifact = (input: {
  readonly runId: string
  readonly generatedAt: string
  readonly demandClient: string
  readonly model: string
  readonly initialConcurrency: number
  readonly finalConcurrency: number
  readonly minConcurrency: number
  readonly maxConcurrency: number
  readonly maxTokens: number
  readonly windowMs: number
  readonly durationMs: number
  readonly observations: ReadonlyArray<GlmLiveAdaptiveStressObservation>
  readonly windows: ReadonlyArray<GlmLiveAdaptiveStressWindow>
}): GlmLiveAdaptiveStressArtifact => ({
  schema: GLM_LIVE_ADAPTIVE_STRESS_ARTIFACT_SCHEMA,
  telemetrySchema: GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
  publicSafe: true,
  runId: input.runId,
  generatedAt: input.generatedAt,
  demandKind: GLM_STRESS_DEMAND_KIND,
  demandSource: GLM_STRESS_DEMAND_SOURCE,
  demandClient: input.demandClient,
  model: input.model,
  initialConcurrency: input.initialConcurrency,
  finalConcurrency: input.finalConcurrency,
  minConcurrency: input.minConcurrency,
  maxConcurrency: input.maxConcurrency,
  maxTokens: input.maxTokens,
  windowMs: input.windowMs,
  durationMs: input.durationMs,
  summary: summarizeGlmLiveAdaptiveStress(input.observations),
  windows: input.windows,
  observations: input.observations,
})

export const buildGlmLiveAdaptiveStressSupervisorTick = (input: {
  readonly generatedAt: string
  readonly runIdPrefix: string
  readonly tickIndex: number
  readonly maxTicks: number
  readonly cadenceMs: number
  readonly model: string
  readonly initialConcurrency: number
  readonly minConcurrency: number
  readonly maxConcurrency: number
  readonly externalDemandActive: boolean
  readonly previousArtifact?: GlmLiveAdaptiveStressArtifact | undefined
}): GlmLiveAdaptiveStressSupervisorTick => {
  const tickIndex = Math.max(0, Math.floor(input.tickIndex))
  const maxTicks = Math.max(0, Math.floor(input.maxTicks))
  const cadenceMs = positiveIntegerOr(input.cadenceMs, 60_000)
  const minConcurrency = positiveIntegerOr(input.minConcurrency, 1)
  const maxConcurrency = Math.max(
    minConcurrency,
    positiveIntegerOr(input.maxConcurrency, minConcurrency),
  )
  const previousFinalConcurrency =
    input.previousArtifact === undefined
      ? null
      : boundedConcurrency(
          input.previousArtifact.finalConcurrency,
          minConcurrency,
          maxConcurrency,
        )
  const carriedConcurrency =
    previousFinalConcurrency ??
    boundedConcurrency(input.initialConcurrency, minConcurrency, maxConcurrency)
  const previousSummary =
    input.previousArtifact === undefined ? null : input.previousArtifact.summary
  const previousHadBackoff =
    previousSummary !== null &&
    (previousSummary.failedCount > 0 || previousSummary.preemptedCount > 0)
  const previousWasClean =
    previousSummary !== null &&
    previousSummary.failedCount === 0 &&
    previousSummary.preemptedCount === 0 &&
    previousSummary.okCount > 0
  const action: GlmLiveAdaptiveStressSupervisorAction =
    tickIndex >= maxTicks
      ? 'complete'
      : input.externalDemandActive
        ? 'yield_to_external'
        : 'dispatch_next_window'
  const recommendedInitialConcurrency =
    action === 'yield_to_external' || action === 'complete'
      ? minConcurrency
      : carriedConcurrency
  const nextEarliestStartAt =
    action === 'complete'
      ? null
      : new Date(Date.parse(input.generatedAt) + cadenceMs).toISOString()
  const reasons: Array<GlmLiveAdaptiveStressSupervisorReason> = [
    ...(action === 'complete' ? (['max_ticks_completed'] as const) : []),
    ...(action === 'yield_to_external'
      ? (['external_demand_active'] as const)
      : []),
    ...(action === 'dispatch_next_window'
      ? (['continuous_window_ready'] as const)
      : []),
    ...(previousHadBackoff
      ? (['previous_backoff_carried_forward'] as const)
      : []),
    ...(previousWasClean ? (['previous_window_clean'] as const) : []),
  ]

  return {
    schema: GLM_LIVE_ADAPTIVE_STRESS_SUPERVISOR_SCHEMA,
    telemetrySchema: GLM_CONTINUOUS_STRESS_TELEMETRY_SCHEMA,
    publicSafe: true,
    generatedAt: input.generatedAt,
    runIdPrefix: input.runIdPrefix,
    tickIndex,
    maxTicks,
    cadenceMs,
    action,
    nextRunId:
      action === 'dispatch_next_window'
        ? `${input.runIdPrefix}-${String(tickIndex + 1).padStart(4, '0')}`
        : null,
    nextEarliestStartAt,
    demandKind: GLM_STRESS_DEMAND_KIND,
    demandSource: GLM_STRESS_DEMAND_SOURCE,
    demandClient: input.runIdPrefix,
    model: input.model,
    minConcurrency,
    maxConcurrency,
    recommendedInitialConcurrency,
    previousRunId: input.previousArtifact?.runId ?? null,
    previousFinalConcurrency,
    previousSummary,
    reasonRefs: supervisorReasonRefs([...new Set(reasons)]),
  }
}
