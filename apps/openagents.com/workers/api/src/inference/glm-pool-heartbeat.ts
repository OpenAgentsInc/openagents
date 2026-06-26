import { Effect } from 'effect'

import { recordFromUnknown } from '../json-boundary'
import {
  currentEpochMillis,
  epochMillisToIsoTimestamp,
} from '../runtime-primitives'
import {
  type TokenUsageLedgerShape,
  makeD1TokenUsageLedger,
} from '../token-usage-ledger'
import type { GlmReplicaRoutingStateOverride } from './hydralisk-adapter'
import { HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID } from './model-router'
import type {
  HydraliskGlm52Replica,
  SupplyLaneCredentialEnv,
} from './model-serving-policy'
import { resolveHydraliskGlm52Reap504bArming } from './model-serving-policy'
import { HYDRALISK_GLM_52_REAP_504B_MODEL_ID, priceRequest } from './pricing'
import type { InferenceUsage } from './provider-adapter'

export type GlmPoolHeartbeatHttpResponse = Readonly<{
  json: () => Promise<unknown>
  ok: boolean
}>

export type GlmPoolHeartbeatFetch = (
  input: string,
  init: RequestInit,
) => Promise<GlmPoolHeartbeatHttpResponse>

export type GlmPoolHeartbeatProbeStatus = 'ok' | 'failed' | 'skipped'

export type GlmPoolKeepWarmStatus =
  | 'completed'
  | 'control_plane_only'
  | 'disabled'
  | 'failed'
  | 'skipped_benchmark_reserved'
  | 'skipped_benchmark_window'
  | 'skipped_draining'

export type GlmPoolWatchdogStatus =
  | 'degraded'
  | 'healthy'
  | 'skipped'
  | 'unhealthy'

export type GlmPoolHeartbeatWarmState = 'cold' | 'unknown' | 'warm'

export type GlmPoolHeartbeatReplicaRecord = Readonly<{
  benchmarkReserved: boolean
  breakerConsecutiveFailures?: number | undefined
  breakerConsecutiveSuccesses?: number | undefined
  breakerFailureThreshold?: number | undefined
  breakerReadmitSuccessThreshold?: number | undefined
  draining: boolean
  healthStatus: GlmPoolHeartbeatProbeStatus
  keepWarmStatus: GlmPoolKeepWarmStatus
  modelsStatus: GlmPoolHeartbeatProbeStatus
  observedAt: string
  probeTimeoutMs: number
  replicaId: string
  replicaRef: string
  runRef: string
  totalWallClockMs: number
  usage: InferenceUsage
  warmCompletionStatus: GlmPoolHeartbeatProbeStatus
  warmState: GlmPoolHeartbeatWarmState
  watchdogStatus: GlmPoolWatchdogStatus
}>

export type GlmPoolHeartbeatPersistenceFailure = Readonly<{
  errorTag: string
  replicaId?: string | undefined
  runRef: string
  stage: 'replica_record' | 'scheduled_skip'
}>

export type GlmPoolHeartbeatRunReport = Readonly<{
  benchmarkOwnershipActive: boolean
  enabled: boolean
  observedAt: string
  persistenceFailures: ReadonlyArray<GlmPoolHeartbeatPersistenceFailure>
  records: ReadonlyArray<GlmPoolHeartbeatReplicaRecord>
  runRef: string
  skippedReason?: 'cadence' | 'disabled' | 'unarmed' | undefined
  warmCompletionEnabled: boolean
}>

type GlmPoolHeartbeatSkippedReason = NonNullable<
  GlmPoolHeartbeatRunReport['skippedReason']
>

export type HydraliskGlmPoolHeartbeatEnv = SupplyLaneCredentialEnv &
  Readonly<{
    HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE?: string | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES?: string | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED?: string | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_FAILURE_THRESHOLD?:
      | string
      | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_PROBE_TIMEOUT_MS?:
      | string
      | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_READMIT_SUCCESS_THRESHOLD?:
      | string
      | undefined
    HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED?:
      | string
      | undefined
  }>

export type GlmReplicaHeartbeatBreakerPolicy = Readonly<{
  failureThreshold: number
  readmitSuccessThreshold: number
}>

type GlmReplicaHeartbeatBreakerState = Readonly<{
  consecutiveFailures: number
  consecutiveSuccesses: number
  health: Exclude<GlmPoolWatchdogStatus, 'skipped'>
}>

const REPLICA_REF_PREFIX = 'replica.hydralisk.glm_52_reap_504b'
const HEARTBEAT_ACCOUNT_REF = 'account.openagents.owned_inference'
const HEARTBEAT_DEMAND_CLIENT = 'worker-cron'
const HEARTBEAT_DEMAND_SOURCE = 'glm-pool-heartbeat'

const latestRoutingStateByReplica = new Map<
  string,
  GlmReplicaRoutingStateOverride
>()
const latestHeartbeatRecordByReplica = new Map<
  string,
  GlmPoolHeartbeatReplicaRecord
>()
const breakerStateByReplica = new Map<string, GlmReplicaHeartbeatBreakerState>()

const DEFAULT_BREAKER_POLICY: GlmReplicaHeartbeatBreakerPolicy = {
  failureThreshold: 3,
  readmitSuccessThreshold: 2,
}
const MIN_BREAKER_THRESHOLD = 2
const MAX_BREAKER_THRESHOLD = 10
const DEFAULT_PROBE_TIMEOUT_MS = 2_000
const MIN_PROBE_TIMEOUT_MS = 10
const MAX_PROBE_TIMEOUT_MS = 30_000

const isEnabledFlag = (value: string | undefined): boolean => {
  const normalized = value?.trim().toLowerCase()
  return (
    normalized === 'true' ||
    normalized === '1' ||
    normalized === 'yes' ||
    normalized === 'on'
  )
}

const parsePositiveInteger = (
  value: string | undefined,
  fallback: number,
): number => {
  const parsed = Number(value?.trim())
  return Number.isInteger(parsed) && parsed > 0 ? parsed : fallback
}

const clampBreakerThreshold = (value: number): number =>
  Math.min(
    MAX_BREAKER_THRESHOLD,
    Math.max(MIN_BREAKER_THRESHOLD, Math.floor(value)),
  )

const clampProbeTimeoutMs = (value: number): number =>
  Math.min(
    MAX_PROBE_TIMEOUT_MS,
    Math.max(MIN_PROBE_TIMEOUT_MS, Math.floor(value)),
  )

const normalizeBreakerPolicy = (
  policy: Partial<GlmReplicaHeartbeatBreakerPolicy> | undefined,
): GlmReplicaHeartbeatBreakerPolicy => ({
  failureThreshold: clampBreakerThreshold(
    policy?.failureThreshold ?? DEFAULT_BREAKER_POLICY.failureThreshold,
  ),
  readmitSuccessThreshold: clampBreakerThreshold(
    policy?.readmitSuccessThreshold ??
      DEFAULT_BREAKER_POLICY.readmitSuccessThreshold,
  ),
})

const isoSlug = (iso: string): string =>
  iso
    .trim()
    .replace(/[^0-9a-z]/giu, '')
    .toLowerCase()

const replicaRefFor = (replicaId: string): string =>
  `${REPLICA_REF_PREFIX}.${replicaId}`

const runRefFor = (observedAt: string): string =>
  `heartbeat.hydralisk.glm_52_reap_504b.${isoSlug(observedAt)}`

const urlFor = (replica: HydraliskGlm52Replica, path: string): string =>
  `${replica.baseUrl.replace(/\/+$/u, '')}${path}`

const authorizedHeaders = (
  replica: HydraliskGlm52Replica,
): Record<string, string> => ({
  accept: 'application/json',
  authorization: `Bearer ${replica.bearerToken}`,
})

const promiseWithTimeout = async <A>(
  timeoutMs: number,
  run: (signal: AbortSignal) => Promise<A>,
): Promise<A | undefined> => {
  const controller = new AbortController()
  let settled = false
  return await new Promise<A | undefined>(resolve => {
    const timer = setTimeout(() => {
      if (!settled) {
        settled = true
        controller.abort()
        resolve(undefined)
      }
    }, timeoutMs)

    run(controller.signal).then(
      value => {
        if (!settled) {
          settled = true
          clearTimeout(timer)
          resolve(value)
        }
      },
      () => {
        if (!settled) {
          settled = true
          clearTimeout(timer)
          resolve(undefined)
        }
      },
    )
  })
}

const fetchOk = (
  fetchImpl: GlmPoolHeartbeatFetch,
  replica: HydraliskGlm52Replica,
  path: string,
  probeTimeoutMs: number,
): Effect.Effect<boolean> =>
  Effect.tryPromise({
    catch: () => 'glm_pool_heartbeat_fetch_failed' as const,
    try: async () => {
      const responseOk = await promiseWithTimeout(
        probeTimeoutMs,
        async signal => {
          const response = await fetchImpl(urlFor(replica, path), {
            headers: authorizedHeaders(replica),
            method: 'GET',
            signal,
          })
          return response.ok
        },
      )
      return responseOk ?? false
    },
  }).pipe(Effect.catch(() => Effect.succeed(false)))

const warmRequestBody = (): Record<string, unknown> => ({
  max_tokens: 1,
  messages: [{ content: 'Reply READY.', role: 'user' }],
  model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
  stream: false,
  temperature: 0,
})

const usageFromWarmCompletion = (
  value: unknown,
): InferenceUsage | undefined => {
  const record = recordFromUnknown(value)
  const usage = recordFromUnknown(record?.['usage'])
  const promptTokens = Number(usage?.['prompt_tokens'])
  const completionTokens = Number(usage?.['completion_tokens'])
  const totalTokens = Number(usage?.['total_tokens'])
  if (
    !Number.isFinite(promptTokens) ||
    !Number.isFinite(completionTokens) ||
    !Number.isFinite(totalTokens)
  ) {
    return undefined
  }
  return {
    completionTokens,
    promptTokens,
    totalTokens,
  }
}

const warmCompletion = (
  fetchImpl: GlmPoolHeartbeatFetch,
  replica: HydraliskGlm52Replica,
  probeTimeoutMs: number,
): Effect.Effect<InferenceUsage | undefined> =>
  Effect.tryPromise({
    catch: () => 'glm_pool_heartbeat_warm_failed' as const,
    try: async () => {
      return await promiseWithTimeout(probeTimeoutMs, async signal => {
        const response = await fetchImpl(
          urlFor(replica, '/v1/chat/completions'),
          {
            body: JSON.stringify(warmRequestBody()),
            headers: {
              ...authorizedHeaders(replica),
              'content-type': 'application/json',
            },
            method: 'POST',
            signal,
          },
        )
        if (!response.ok) {
          return undefined
        }
        return usageFromWarmCompletion(await response.json())
      })
    },
  }).pipe(
    Effect.catch(() =>
      Effect.sync((): InferenceUsage | undefined => undefined),
    ),
  )

const emptyUsage: InferenceUsage = {
  completionTokens: 0,
  promptTokens: 0,
  totalTokens: 0,
}

const roundMs = (value: number): number =>
  Math.max(0, Math.round(value * 1000) / 1000)

const costUsdFor = (usage: InferenceUsage): number =>
  Math.round(
    Math.max(
      0,
      priceRequest({
        fundingKind: 'card',
        model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
        usage,
      }).costUsd,
    ) * 1_000_000,
  ) / 1_000_000

const recordRoutingState = (
  record: GlmPoolHeartbeatReplicaRecord,
): GlmReplicaRoutingStateOverride => ({
  benchmarkReserved: record.benchmarkReserved,
  draining: record.draining,
  health:
    record.watchdogStatus === 'healthy'
      ? 'healthy'
      : record.watchdogStatus === 'skipped'
        ? 'degraded'
        : record.watchdogStatus,
  warmState: record.warmState,
  ...(record.warmState === 'warm'
    ? { warmAtEpochMs: Date.parse(record.observedAt) }
    : {}),
})

const breakerStateForProbe = (
  input: Readonly<{
    policy: GlmReplicaHeartbeatBreakerPolicy
    probeHealthy: boolean
    replicaId: string
  }>,
): GlmReplicaHeartbeatBreakerState => {
  const previous = breakerStateByReplica.get(input.replicaId)
  if (input.probeHealthy) {
    if (previous?.health === 'unhealthy') {
      const consecutiveSuccesses = previous.consecutiveSuccesses + 1
      return consecutiveSuccesses >= input.policy.readmitSuccessThreshold
        ? {
            consecutiveFailures: 0,
            consecutiveSuccesses: 0,
            health: 'healthy',
          }
        : {
            consecutiveFailures: 0,
            consecutiveSuccesses,
            health: 'unhealthy',
          }
    }
    return {
      consecutiveFailures: 0,
      consecutiveSuccesses: 0,
      health: 'healthy',
    }
  }

  const consecutiveFailures = (previous?.consecutiveFailures ?? 0) + 1
  return {
    consecutiveFailures,
    consecutiveSuccesses: 0,
    health:
      consecutiveFailures >= input.policy.failureThreshold
        ? 'unhealthy'
        : 'degraded',
  }
}

const applyBreakerProbe = (
  input: Readonly<{
    policy: GlmReplicaHeartbeatBreakerPolicy
    probeHealthy: boolean
    replicaId: string
  }>,
): GlmReplicaHeartbeatBreakerState => {
  const next = breakerStateForProbe(input)
  breakerStateByReplica.set(input.replicaId, next)
  return next
}

export const recordGlmPoolHeartbeatRoutingState = (
  records: ReadonlyArray<GlmPoolHeartbeatReplicaRecord>,
): void => {
  for (const record of records) {
    latestRoutingStateByReplica.set(
      record.replicaId,
      recordRoutingState(record),
    )
    latestHeartbeatRecordByReplica.set(record.replicaId, record)
  }
}

export const glmPoolHeartbeatRoutingStateOracle = (
  replicaId: string,
): GlmReplicaRoutingStateOverride | undefined =>
  latestRoutingStateByReplica.get(replicaId)

export const glmPoolHeartbeatLatestRecordOracle = (
  replicaId: string,
): GlmPoolHeartbeatReplicaRecord | undefined =>
  latestHeartbeatRecordByReplica.get(replicaId)

const ingestHeartbeatRecord = (
  ledger: TokenUsageLedgerShape,
  record: GlmPoolHeartbeatReplicaRecord,
): Effect.Effect<void, unknown> =>
  ledger
    .ingestEvent({
      schemaVersion: 'openagents.token_usage_event.v1',
      actor: { accountRef: HEARTBEAT_ACCOUNT_REF },
      backendProfile: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      cost:
        record.usage.totalTokens > 0
          ? { amount: costUsdFor(record.usage), currency: 'USD' }
          : undefined,
      demand: {
        demandClient: HEARTBEAT_DEMAND_CLIENT,
        demandKind: 'own_capacity',
        demandSource: HEARTBEAT_DEMAND_SOURCE,
      },
      eventId: `event.${record.runRef}.${record.replicaId}`,
      idempotencyKey: `inference:glm-pool-heartbeat:${record.runRef}:${record.replicaId}`,
      model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
      observedAt: record.observedAt,
      privacy: { leaderboardEligible: false, privacyOptOut: false },
      producerSystem: 'omega',
      provider: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      safeMetadata: {
        benchmarkReserved: record.benchmarkReserved,
        ...(record.breakerConsecutiveFailures === undefined
          ? {}
          : { breakerConsecutiveFailures: record.breakerConsecutiveFailures }),
        ...(record.breakerConsecutiveSuccesses === undefined
          ? {}
          : {
              breakerConsecutiveSuccesses:
                record.breakerConsecutiveSuccesses,
            }),
        ...(record.breakerFailureThreshold === undefined
          ? {}
          : { breakerFailureThreshold: record.breakerFailureThreshold }),
        ...(record.breakerReadmitSuccessThreshold === undefined
          ? {}
          : {
              breakerReadmitSuccessThreshold:
                record.breakerReadmitSuccessThreshold,
            }),
        demandClient: HEARTBEAT_DEMAND_CLIENT,
        demandKind: 'own_capacity',
        demandSource: HEARTBEAT_DEMAND_SOURCE,
        draining: record.draining,
        heartbeatKind: 'glm_pool_heartbeat',
        heartbeatRunRef: record.runRef,
        healthStatus: record.healthStatus,
        keepWarmStatus: record.keepWarmStatus,
        modelsStatus: record.modelsStatus,
        probeTimeoutMs: record.probeTimeoutMs,
        replicaWarmState: record.warmState,
        selectedReplicaId: record.replicaId,
        selectedReplicaRef: record.replicaRef,
        totalWallClockMs: record.totalWallClockMs,
        warmCompletionStatus: record.warmCompletionStatus,
        watchdogStatus: record.watchdogStatus,
      },
      sourceRefs: { runRef: record.runRef },
      sourceRoute: 'omega_hosted_gemini',
      tokenCounts: {
        cacheReadTokens: 0,
        cacheWrite1hTokens: 0,
        cacheWrite5mTokens: 0,
        inputTokens: Math.max(0, Math.trunc(record.usage.promptTokens)),
        outputTokens: Math.max(0, Math.trunc(record.usage.completionTokens)),
        reasoningTokens: 0,
        totalTokens: Math.max(0, Math.trunc(record.usage.totalTokens)),
      },
      usageTruth: 'exact',
    })
    .pipe(Effect.asVoid)

const ingestScheduledSkipDiagnostic = (
  input: Readonly<{
    benchmarkOwnershipActive: boolean
    cadenceMinutes: number
    enabled: boolean
    ledger: TokenUsageLedgerShape
    observedAt: string
    replicaCount: number
    runRef: string
    skippedReason: GlmPoolHeartbeatSkippedReason
    warmCompletionEnabled: boolean
  }>,
): Effect.Effect<void, unknown> =>
  input.ledger
    .ingestEvent({
      schemaVersion: 'openagents.token_usage_event.v1',
      actor: { accountRef: HEARTBEAT_ACCOUNT_REF },
      backendProfile: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      demand: {
        demandClient: HEARTBEAT_DEMAND_CLIENT,
        demandKind: 'own_capacity',
        demandSource: HEARTBEAT_DEMAND_SOURCE,
      },
      eventId: `event.${input.runRef}.scheduled.${input.skippedReason}`,
      idempotencyKey: `inference:glm-pool-heartbeat:${input.runRef}:scheduled:${input.skippedReason}`,
      model: HYDRALISK_GLM_52_REAP_504B_MODEL_ID,
      observedAt: input.observedAt,
      privacy: { leaderboardEligible: false, privacyOptOut: false },
      producerSystem: 'omega',
      provider: HYDRALISK_GLM_52_REAP_504B_ADAPTER_ID,
      safeMetadata: {
        benchmarkOwnershipActive: input.benchmarkOwnershipActive,
        cadenceMinutes: input.cadenceMinutes,
        demandClient: HEARTBEAT_DEMAND_CLIENT,
        demandKind: 'own_capacity',
        demandSource: HEARTBEAT_DEMAND_SOURCE,
        enabled: input.enabled,
        heartbeatDiagnosticKind: 'scheduled_skip',
        heartbeatKind: 'glm_pool_heartbeat',
        heartbeatRunRef: input.runRef,
        replicaCount: input.replicaCount,
        scheduledSkipReason: input.skippedReason,
        warmCompletionEnabled: input.warmCompletionEnabled,
      },
      sourceRefs: { runRef: input.runRef },
      sourceRoute: 'omega_hosted_gemini',
      tokenCounts: {
        cacheReadTokens: 0,
        cacheWrite1hTokens: 0,
        cacheWrite5mTokens: 0,
        inputTokens: 0,
        outputTokens: 0,
        reasoningTokens: 0,
        totalTokens: 0,
      },
      usageTruth: 'exact',
    })
    .pipe(Effect.asVoid)

const errorTagFromUnknown = (error: unknown): string => {
  if (
    typeof error === 'object' &&
    error !== null &&
    '_tag' in error &&
    typeof error._tag === 'string'
  ) {
    return error._tag
  }

  return 'unknown_error'
}

const heartbeatPersistenceFailure = (
  input: Readonly<{
    error: unknown
    replicaId?: string | undefined
    runRef: string
    stage: GlmPoolHeartbeatPersistenceFailure['stage']
  }>,
): GlmPoolHeartbeatPersistenceFailure => ({
  errorTag: errorTagFromUnknown(input.error),
  ...(input.replicaId === undefined ? {} : { replicaId: input.replicaId }),
  runRef: input.runRef,
  stage: input.stage,
})

const skippedRecord = (
  input: Readonly<{
    keepWarmStatus: Extract<
      GlmPoolKeepWarmStatus,
      'skipped_benchmark_reserved' | 'skipped_draining'
    >
    observedAt: string
    probeTimeoutMs: number
    replica: HydraliskGlm52Replica
    runRef: string
  }>,
): GlmPoolHeartbeatReplicaRecord => ({
  benchmarkReserved: input.replica.benchmarkReserved,
  draining: input.replica.draining,
  healthStatus: 'skipped',
  keepWarmStatus: input.keepWarmStatus,
  modelsStatus: 'skipped',
  observedAt: input.observedAt,
  probeTimeoutMs: input.probeTimeoutMs,
  replicaId: input.replica.replicaId,
  replicaRef: replicaRefFor(input.replica.replicaId),
  runRef: input.runRef,
  totalWallClockMs: 0,
  usage: emptyUsage,
  warmCompletionStatus: 'skipped',
  warmState: 'unknown',
  watchdogStatus: 'skipped',
})

const probeReplica = (
  input: Readonly<{
    benchmarkOwnershipActive: boolean
    fetchImpl: GlmPoolHeartbeatFetch
    ledger: TokenUsageLedgerShape
    nowMs: () => number
    observedAt: string
    breakerPolicy: GlmReplicaHeartbeatBreakerPolicy
    probeTimeoutMs: number
    replica: HydraliskGlm52Replica
    runRef: string
    warmCompletionEnabled: boolean
  }>,
): Effect.Effect<GlmPoolHeartbeatReplicaRecord> =>
  Effect.gen(function* () {
    if (input.replica.benchmarkReserved) {
      return skippedRecord({
        keepWarmStatus: 'skipped_benchmark_reserved',
        observedAt: input.observedAt,
        probeTimeoutMs: input.probeTimeoutMs,
        replica: input.replica,
        runRef: input.runRef,
      })
    }
    if (input.replica.draining) {
      return skippedRecord({
        keepWarmStatus: 'skipped_draining',
        observedAt: input.observedAt,
        probeTimeoutMs: input.probeTimeoutMs,
        replica: input.replica,
        runRef: input.runRef,
      })
    }

    const startedAt = input.nowMs()
    const healthOk = yield* fetchOk(
      input.fetchImpl,
      input.replica,
      '/health',
      input.probeTimeoutMs,
    )
    const modelsOk = yield* fetchOk(
      input.fetchImpl,
      input.replica,
      '/v1/models',
      input.probeTimeoutMs,
    )
    const shouldWarm =
      input.warmCompletionEnabled && !input.benchmarkOwnershipActive
    const usage = shouldWarm
      ? yield* warmCompletion(
          input.fetchImpl,
          input.replica,
          input.probeTimeoutMs,
        )
      : undefined
    const totalWallClockMs = roundMs(input.nowMs() - startedAt)
    const warmOk = usage !== undefined
    const breakerState = applyBreakerProbe({
      policy: input.breakerPolicy,
      probeHealthy: healthOk && modelsOk && (!shouldWarm || warmOk),
      replicaId: input.replica.replicaId,
    })
    const watchdogStatus: GlmPoolWatchdogStatus = breakerState.health
    const keepWarmStatus: GlmPoolKeepWarmStatus =
      shouldWarm && warmOk
        ? 'completed'
        : shouldWarm
          ? 'failed'
          : input.benchmarkOwnershipActive
            ? 'skipped_benchmark_window'
            : input.warmCompletionEnabled
              ? 'failed'
              : 'control_plane_only'
    const healthStatus: GlmPoolHeartbeatProbeStatus = healthOk ? 'ok' : 'failed'
    const modelsStatus: GlmPoolHeartbeatProbeStatus = modelsOk ? 'ok' : 'failed'
    const warmCompletionStatus: GlmPoolHeartbeatProbeStatus = shouldWarm
      ? warmOk
        ? 'ok'
        : 'failed'
      : 'skipped'
    const warmState: GlmPoolHeartbeatWarmState = warmOk
      ? 'warm'
      : watchdogStatus === 'healthy'
        ? 'unknown'
        : 'cold'

    return {
      benchmarkReserved: input.replica.benchmarkReserved,
      breakerConsecutiveFailures: breakerState.consecutiveFailures,
      breakerConsecutiveSuccesses: breakerState.consecutiveSuccesses,
      breakerFailureThreshold: input.breakerPolicy.failureThreshold,
      breakerReadmitSuccessThreshold:
        input.breakerPolicy.readmitSuccessThreshold,
      draining: input.replica.draining,
      healthStatus,
      keepWarmStatus,
      modelsStatus,
      observedAt: input.observedAt,
      probeTimeoutMs: input.probeTimeoutMs,
      replicaId: input.replica.replicaId,
      replicaRef: replicaRefFor(input.replica.replicaId),
      runRef: input.runRef,
      totalWallClockMs,
      usage: usage ?? emptyUsage,
      warmCompletionStatus,
      warmState,
      watchdogStatus,
    }
  })

export const runGlmPoolHeartbeat = (
  input: Readonly<{
    benchmarkOwnershipActive: boolean
    fetchImpl?: GlmPoolHeartbeatFetch | undefined
    ledger: TokenUsageLedgerShape
    nowMs?: (() => number) | undefined
    observedAt: string
    breakerPolicy?: Partial<GlmReplicaHeartbeatBreakerPolicy> | undefined
    replicas: ReadonlyArray<HydraliskGlm52Replica>
    probeTimeoutMs?: number | undefined
    warmCompletionEnabled: boolean
  }>,
): Effect.Effect<GlmPoolHeartbeatRunReport> => {
  const runRef = runRefFor(input.observedAt)
  const fetchImpl = input.fetchImpl ?? globalThis.fetch
  const nowMs = input.nowMs ?? currentEpochMillis
  const breakerPolicy = normalizeBreakerPolicy(input.breakerPolicy)
  const probeTimeoutMs = clampProbeTimeoutMs(
    input.probeTimeoutMs ?? DEFAULT_PROBE_TIMEOUT_MS,
  )

  return Effect.gen(function* () {
    const records = yield* Effect.forEach(input.replicas, replica =>
      probeReplica({
        benchmarkOwnershipActive: input.benchmarkOwnershipActive,
        fetchImpl,
        ledger: input.ledger,
        nowMs,
        observedAt: input.observedAt,
        breakerPolicy,
        probeTimeoutMs,
        replica,
        runRef,
        warmCompletionEnabled: input.warmCompletionEnabled,
      }),
    )
    const persistenceFailures = yield* Effect.forEach(records, record =>
      ingestHeartbeatRecord(input.ledger, record).pipe(
        Effect.as(
          undefined as GlmPoolHeartbeatPersistenceFailure | undefined,
        ),
        Effect.catch(error =>
          Effect.succeed(
            heartbeatPersistenceFailure({
              error,
              replicaId: record.replicaId,
              runRef: record.runRef,
              stage: 'replica_record',
            }),
          ),
        ),
      ),
    ).pipe(
      Effect.map(failures =>
        failures.filter(
          (
            failure,
          ): failure is GlmPoolHeartbeatPersistenceFailure =>
            failure !== undefined,
        ),
      ),
    )
    recordGlmPoolHeartbeatRoutingState(records)

    return {
      benchmarkOwnershipActive: input.benchmarkOwnershipActive,
      enabled: true,
      observedAt: input.observedAt,
      persistenceFailures,
      records,
      runRef,
      warmCompletionEnabled: input.warmCompletionEnabled,
    }
  })
}

const cadenceAllows = (
  scheduledTimeMs: number,
  cadenceMinutes: number,
): boolean => {
  const minute = Math.floor(scheduledTimeMs / 60_000)
  return minute % cadenceMinutes === 0
}

export const runScheduledGlmPoolHeartbeat = (
  input: Readonly<{
    env: HydraliskGlmPoolHeartbeatEnv
    fetchImpl?: GlmPoolHeartbeatFetch | undefined
    ledger: TokenUsageLedgerShape
    scheduledTimeMs: number
  }>,
): Effect.Effect<GlmPoolHeartbeatRunReport> => {
  const observedAt = epochMillisToIsoTimestamp(input.scheduledTimeMs)
  const runRef = runRefFor(observedAt)
  const enabled = isEnabledFlag(
    input.env.HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED,
  )
  const warmCompletionEnabled = isEnabledFlag(
    input.env.HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_WARM_COMPLETION_ENABLED,
  )
  const benchmarkOwnershipActive = isEnabledFlag(
    input.env.HYDRALISK_GLM_52_REAP_504B_BENCHMARK_OWNERSHIP_ACTIVE,
  )
  const cadenceMinutes = parsePositiveInteger(
    input.env.HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES,
    4,
  )
  const breakerPolicy = normalizeBreakerPolicy({
    failureThreshold: parsePositiveInteger(
      input.env.HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_FAILURE_THRESHOLD,
      DEFAULT_BREAKER_POLICY.failureThreshold,
    ),
    readmitSuccessThreshold: parsePositiveInteger(
      input.env
        .HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_READMIT_SUCCESS_THRESHOLD,
      DEFAULT_BREAKER_POLICY.readmitSuccessThreshold,
    ),
  })
  const probeTimeoutMs = clampProbeTimeoutMs(
    parsePositiveInteger(
      input.env.HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_PROBE_TIMEOUT_MS,
      DEFAULT_PROBE_TIMEOUT_MS,
    ),
  )
  const arming = resolveHydraliskGlm52Reap504bArming(input.env)

  const skippedReport = (
    skippedReason: GlmPoolHeartbeatSkippedReason,
    reportEnabled: boolean,
    persistenceFailures: ReadonlyArray<GlmPoolHeartbeatPersistenceFailure> = [],
  ): GlmPoolHeartbeatRunReport => ({
    benchmarkOwnershipActive,
    enabled: reportEnabled,
    observedAt,
    persistenceFailures,
    records: [],
    runRef,
    skippedReason,
    warmCompletionEnabled,
  })

  const persistSkippedReport = (
    skippedReason: GlmPoolHeartbeatSkippedReason,
    reportEnabled: boolean,
  ): Effect.Effect<GlmPoolHeartbeatRunReport> =>
    ingestScheduledSkipDiagnostic({
      benchmarkOwnershipActive,
      cadenceMinutes,
      enabled: reportEnabled,
      ledger: input.ledger,
      observedAt,
      replicaCount: arming.replicas.length,
      runRef,
      skippedReason,
      warmCompletionEnabled,
    }).pipe(
      Effect.as(skippedReport(skippedReason, reportEnabled)),
      Effect.catch(error =>
        Effect.succeed(
          skippedReport(skippedReason, reportEnabled, [
            heartbeatPersistenceFailure({
              error,
              runRef,
              stage: 'scheduled_skip',
            }),
          ]),
        ),
      ),
    )

  if (!enabled) {
    return persistSkippedReport('disabled', false)
  }

  if (!cadenceAllows(input.scheduledTimeMs, cadenceMinutes)) {
    return persistSkippedReport('cadence', true)
  }

  if (arming.replicas.length === 0) {
    return persistSkippedReport('unarmed', true)
  }

  return runGlmPoolHeartbeat({
    benchmarkOwnershipActive,
    ...(input.fetchImpl === undefined ? {} : { fetchImpl: input.fetchImpl }),
    ledger: input.ledger,
    observedAt,
    breakerPolicy,
    replicas: arming.replicas,
    probeTimeoutMs,
    warmCompletionEnabled,
  })
}

export const runScheduledGlmPoolHeartbeatForD1 = (
  input: Readonly<{
    db: D1Database
    env: HydraliskGlmPoolHeartbeatEnv
    scheduledTimeMs: number
  }>,
): Effect.Effect<GlmPoolHeartbeatRunReport> =>
  runScheduledGlmPoolHeartbeat({
    env: input.env,
    ledger: makeD1TokenUsageLedger(input.db),
    scheduledTimeMs: input.scheduledTimeMs,
  })
