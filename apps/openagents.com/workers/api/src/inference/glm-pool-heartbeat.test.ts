import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { TokenUsageLedgerShape } from '../token-usage-ledger'
import {
  type GlmPoolHeartbeatFetch,
  glmPoolHeartbeatRoutingStateOracle,
  runGlmPoolHeartbeat,
  runScheduledGlmPoolHeartbeat,
} from './glm-pool-heartbeat'
import type { HydraliskGlm52Replica } from './model-serving-policy'

const OBSERVED_AT = '2026-06-25T16:00:00.000Z'

const replica = (
  replicaId: string,
  overrides: Partial<HydraliskGlm52Replica> = {},
): HydraliskGlm52Replica => ({
  baseUrl: `https://${replicaId}.glm.example.test`,
  baseUrlSecretRef: `HYDRALISK_GLM_52_REAP_504B_${replicaId.toUpperCase()}_BASE_URL`,
  bearerSecretRef: `HYDRALISK_GLM_52_REAP_504B_${replicaId.toUpperCase()}_BEARER_TOKEN`,
  bearerToken: `${replicaId}-token`,
  benchmarkReserved: false,
  costProfileRef: 'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.fixture',
  draining: false,
  evidenceRefs: [`receipt.hydralisk.glm.${replicaId}.fixture`],
  maxInflight: 1,
  profileRef: `profile.hydralisk.glm_52_reap_504b.${replicaId}.fixture`,
  replicaId,
  ...overrides,
})

const captureLedger = () => {
  const bodies: Array<Record<string, unknown>> = []
  const ledger = {
    ingestEvent: (body: unknown) =>
      Effect.sync(() => {
        bodies.push(body as Record<string, unknown>)
        return { event: {} as never, inserted: true }
      }),
  } as unknown as TokenUsageLedgerShape

  return { bodies, ledger }
}

const failingLedger = (errorTag: string): TokenUsageLedgerShape =>
  ({
    ingestEvent: () =>
      Effect.fail({
        _tag: errorTag,
      }),
  }) as unknown as TokenUsageLedgerShape

const okFetch =
  (
    calls: Array<Readonly<{ body?: string; method: string; url: string }>>,
  ): GlmPoolHeartbeatFetch =>
  async (input, init) => {
    calls.push({
      ...(typeof init.body === 'string' ? { body: init.body } : {}),
      method: init.method ?? 'GET',
      url: input,
    })
    if (String(input).endsWith('/v1/chat/completions')) {
      return Response.json({
        choices: [{ finish_reason: 'stop', message: { content: 'READY' } }],
        model: 'openagents/glm-5.2-reap-504b',
        usage: {
          completion_tokens: 1,
          prompt_tokens: 6,
          total_tokens: 7,
        },
      })
    }
    return Response.json({ ok: true })
  }

const failedFetch =
  (
    calls: Array<Readonly<{ body?: string; method: string; url: string }>>,
  ): GlmPoolHeartbeatFetch =>
  async (input, init) => {
    calls.push({
      ...(typeof init.body === 'string' ? { body: init.body } : {}),
      method: init.method ?? 'GET',
      url: input,
    })
    return Response.json({ ok: false }, { status: 503 })
  }

const warmCompletionFailureFetch =
  (
    calls: Array<Readonly<{ body?: string; method: string; url: string }>>,
  ): GlmPoolHeartbeatFetch =>
  async (input, init) => {
    calls.push({
      ...(typeof init.body === 'string' ? { body: init.body } : {}),
      method: init.method ?? 'GET',
      url: input,
    })
    if (String(input).endsWith('/v1/chat/completions')) {
      return Response.json({ error: 'temporarily unavailable' }, { status: 503 })
    }
    return Response.json({ ok: true })
  }

describe('runGlmPoolHeartbeat', () => {
  test('records reserved and draining replicas without calling their endpoints', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        fetchImpl: okFetch(calls),
        ledger,
        observedAt: OBSERVED_AT,
        replicas: [
          replica('reserved', { benchmarkReserved: true }),
          replica('draining', { draining: true }),
        ],
        warmCompletionEnabled: true,
      }),
    )

    expect(calls).toEqual([])
    expect(report.records).toEqual([
      expect.objectContaining({
        keepWarmStatus: 'skipped_benchmark_reserved',
        replicaId: 'reserved',
        warmCompletionStatus: 'skipped',
        watchdogStatus: 'skipped',
      }),
      expect.objectContaining({
        keepWarmStatus: 'skipped_draining',
        replicaId: 'draining',
        warmCompletionStatus: 'skipped',
        watchdogStatus: 'skipped',
      }),
    ])
    expect(bodies).toHaveLength(2)
    expect(bodies[0]?.safeMetadata).toMatchObject({
      keepWarmStatus: 'skipped_benchmark_reserved',
      selectedReplicaId: 'reserved',
    })
    expect(bodies[1]?.safeMetadata).toMatchObject({
      keepWarmStatus: 'skipped_draining',
      selectedReplicaId: 'draining',
    })
  })

  test('warms an eligible replica only under the explicit completion flag', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        fetchImpl: okFetch(calls),
        ledger,
        nowMs: (() => {
          let now = 1_000
          return () => {
            now += 25
            return now
          }
        })(),
        observedAt: OBSERVED_AT,
        replicas: [replica('warm-path')],
        warmCompletionEnabled: true,
      }),
    )

    expect(
      calls.map(call => `${call.method} ${new URL(call.url).pathname}`),
    ).toEqual(['GET /health', 'GET /v1/models', 'POST /v1/chat/completions'])
    expect(report.records[0]).toMatchObject({
      keepWarmStatus: 'completed',
      replicaId: 'warm-path',
      usage: { completionTokens: 1, promptTokens: 6, totalTokens: 7 },
      warmCompletionStatus: 'ok',
      warmState: 'warm',
      watchdogStatus: 'healthy',
    })
    expect(bodies[0]).toMatchObject({
      model: 'openagents/glm-5.2-reap-504b',
      tokenCounts: {
        inputTokens: 6,
        outputTokens: 1,
        totalTokens: 7,
      },
    })
    expect(bodies[0]?.safeMetadata).toMatchObject({
      heartbeatKind: 'glm_pool_heartbeat',
      keepWarmStatus: 'completed',
      selectedReplicaId: 'warm-path',
      warmCompletionStatus: 'ok',
      watchdogStatus: 'healthy',
    })
    expect(glmPoolHeartbeatRoutingStateOracle('warm-path')).toMatchObject({
      health: 'healthy',
      warmState: 'warm',
    })
  })

  test('keeps probe results public-safe when token ledger persistence fails', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []

    const report = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        fetchImpl: okFetch(calls),
        ledger: failingLedger('TokenUsageLedgerStorageError'),
        observedAt: OBSERVED_AT,
        replicas: [replica('persistence-failure')],
        warmCompletionEnabled: false,
      }),
    )

    expect(report.records).toHaveLength(1)
    expect(report.persistenceFailures).toEqual([
      {
        errorTag: 'TokenUsageLedgerStorageError',
        replicaId: 'persistence-failure',
        runRef:
          'heartbeat.hydralisk.glm_52_reap_504b.20260625t160000000z',
        stage: 'replica_record',
      },
    ])
    expect(String(JSON.stringify(report.persistenceFailures))).not.toContain(
      'persistence-failure-token',
    )
  })

  test('persists failed rows and continues when a replica probe hangs', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()
    const fetchImpl: GlmPoolHeartbeatFetch = async (input, init) => {
      calls.push({
        method: init.method ?? 'GET',
        url: input,
      })
      if (String(input).includes('hung.glm.example.test')) {
        return await new Promise(() => undefined)
      }
      return Response.json({ ok: true })
    }

    const report = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        fetchImpl,
        ledger,
        observedAt: OBSERVED_AT,
        probeTimeoutMs: 50,
        replicas: [replica('hung'), replica('healthy-after-timeout')],
        warmCompletionEnabled: false,
      }),
    )

    expect(
      calls.map(call => `${call.method} ${new URL(call.url).host}`),
    ).toEqual([
      'GET hung.glm.example.test',
      'GET hung.glm.example.test',
      'GET healthy-after-timeout.glm.example.test',
      'GET healthy-after-timeout.glm.example.test',
    ])
    expect(report.records).toEqual([
      expect.objectContaining({
        healthStatus: 'failed',
        keepWarmStatus: 'control_plane_only',
        modelsStatus: 'failed',
        probeTimeoutMs: 50,
        replicaId: 'hung',
        watchdogStatus: 'degraded',
      }),
      expect.objectContaining({
        healthStatus: 'ok',
        modelsStatus: 'ok',
        probeTimeoutMs: 50,
        replicaId: 'healthy-after-timeout',
        watchdogStatus: 'healthy',
      }),
    ])
    expect(bodies).toHaveLength(2)
    expect(bodies[0]?.safeMetadata).toMatchObject({
      heartbeatKind: 'glm_pool_heartbeat',
      healthStatus: 'failed',
      modelsStatus: 'failed',
      probeTimeoutMs: 50,
      selectedReplicaId: 'hung',
      watchdogStatus: 'degraded',
    })
    expect(bodies[1]?.safeMetadata).toMatchObject({
      healthStatus: 'ok',
      modelsStatus: 'ok',
      selectedReplicaId: 'healthy-after-timeout',
      watchdogStatus: 'healthy',
    })
    expect(String(JSON.stringify(bodies))).not.toContain('hung-token')
  })

  test('benchmark ownership windows block warm completions but keep control-plane health', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: true,
        fetchImpl: okFetch(calls),
        ledger,
        observedAt: OBSERVED_AT,
        replicas: [replica('benchmark-window')],
        warmCompletionEnabled: true,
      }),
    )

    expect(
      calls.map(call => `${call.method} ${new URL(call.url).pathname}`),
    ).toEqual(['GET /health', 'GET /v1/models'])
    expect(report.records[0]).toMatchObject({
      keepWarmStatus: 'skipped_benchmark_window',
      replicaId: 'benchmark-window',
      warmCompletionStatus: 'skipped',
      warmState: 'unknown',
      watchdogStatus: 'healthy',
    })
    expect(bodies[0]?.safeMetadata).toMatchObject({
      keepWarmStatus: 'skipped_benchmark_window',
      selectedReplicaId: 'benchmark-window',
    })
  })

  test('marks failed probes degraded before the bounded unhealthy threshold', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()
    const target = replica('breaker-failure-threshold')

    const first = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy: { failureThreshold: 3, readmitSuccessThreshold: 2 },
        fetchImpl: failedFetch(calls),
        ledger,
        observedAt: '2026-06-25T16:01:00.000Z',
        replicas: [target],
        warmCompletionEnabled: false,
      }),
    )
    const second = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy: { failureThreshold: 3, readmitSuccessThreshold: 2 },
        fetchImpl: failedFetch(calls),
        ledger,
        observedAt: '2026-06-25T16:02:00.000Z',
        replicas: [target],
        warmCompletionEnabled: false,
      }),
    )
    const third = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy: { failureThreshold: 3, readmitSuccessThreshold: 2 },
        fetchImpl: failedFetch(calls),
        ledger,
        observedAt: '2026-06-25T16:03:00.000Z',
        replicas: [target],
        warmCompletionEnabled: false,
      }),
    )

    expect(first.records[0]).toMatchObject({
      breakerConsecutiveFailures: 1,
      healthStatus: 'failed',
      modelsStatus: 'failed',
      watchdogStatus: 'degraded',
    })
    expect(second.records[0]).toMatchObject({
      breakerConsecutiveFailures: 2,
      watchdogStatus: 'degraded',
    })
    expect(third.records[0]).toMatchObject({
      breakerConsecutiveFailures: 3,
      watchdogStatus: 'unhealthy',
    })
    expect(glmPoolHeartbeatRoutingStateOracle(target.replicaId)).toMatchObject({
      health: 'unhealthy',
    })
    expect(bodies.at(-1)?.safeMetadata).toMatchObject({
      breakerConsecutiveFailures: 3,
      breakerFailureThreshold: 3,
      selectedReplicaId: target.replicaId,
      watchdogStatus: 'unhealthy',
    })
  })

  test('counts warm completion failures as breaker failures', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { bodies, ledger } = captureLedger()
    const target = replica('breaker-warm-failure')
    const breakerPolicy = { failureThreshold: 2, readmitSuccessThreshold: 2 }

    const first = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy,
        fetchImpl: warmCompletionFailureFetch(calls),
        ledger,
        observedAt: '2026-06-25T16:03:30.000Z',
        replicas: [target],
        warmCompletionEnabled: true,
      }),
    )
    const second = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy,
        fetchImpl: warmCompletionFailureFetch(calls),
        ledger,
        observedAt: '2026-06-25T16:03:45.000Z',
        replicas: [target],
        warmCompletionEnabled: true,
      }),
    )

    expect(first.records[0]).toMatchObject({
      breakerConsecutiveFailures: 1,
      healthStatus: 'ok',
      modelsStatus: 'ok',
      warmCompletionStatus: 'failed',
      watchdogStatus: 'degraded',
    })
    expect(second.records[0]).toMatchObject({
      breakerConsecutiveFailures: 2,
      healthStatus: 'ok',
      modelsStatus: 'ok',
      warmCompletionStatus: 'failed',
      watchdogStatus: 'unhealthy',
    })
    expect(bodies.at(-1)?.safeMetadata).toMatchObject({
      breakerConsecutiveFailures: 2,
      selectedReplicaId: target.replicaId,
      warmCompletionStatus: 'failed',
      watchdogStatus: 'unhealthy',
    })
  })

  test('readmits unhealthy replicas only after the bounded healthy threshold', async () => {
    const failedCalls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const okCalls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const { ledger } = captureLedger()
    const target = replica('breaker-readmit-threshold')
    const breakerPolicy = { failureThreshold: 3, readmitSuccessThreshold: 2 }

    for (const observedAt of [
      '2026-06-25T16:04:00.000Z',
      '2026-06-25T16:05:00.000Z',
      '2026-06-25T16:06:00.000Z',
    ]) {
      await Effect.runPromise(
        runGlmPoolHeartbeat({
          benchmarkOwnershipActive: false,
          breakerPolicy,
          fetchImpl: failedFetch(failedCalls),
          ledger,
          observedAt,
          replicas: [target],
          warmCompletionEnabled: false,
        }),
      )
    }

    const firstHealthy = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy,
        fetchImpl: okFetch(okCalls),
        ledger,
        observedAt: '2026-06-25T16:07:00.000Z',
        replicas: [target],
        warmCompletionEnabled: false,
      }),
    )
    const secondHealthy = await Effect.runPromise(
      runGlmPoolHeartbeat({
        benchmarkOwnershipActive: false,
        breakerPolicy,
        fetchImpl: okFetch(okCalls),
        ledger,
        observedAt: '2026-06-25T16:08:00.000Z',
        replicas: [target],
        warmCompletionEnabled: false,
      }),
    )

    expect(firstHealthy.records[0]).toMatchObject({
      breakerConsecutiveFailures: 0,
      breakerConsecutiveSuccesses: 1,
      healthStatus: 'ok',
      modelsStatus: 'ok',
      watchdogStatus: 'unhealthy',
    })
    expect(secondHealthy.records[0]).toMatchObject({
      breakerConsecutiveFailures: 0,
      breakerConsecutiveSuccesses: 0,
      watchdogStatus: 'healthy',
    })
    expect(glmPoolHeartbeatRoutingStateOracle(target.replicaId)).toMatchObject({
      health: 'healthy',
    })
  })
})

describe('runScheduledGlmPoolHeartbeat', () => {
  test('reads replica config and respects the disabled default', async () => {
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runScheduledGlmPoolHeartbeat({
        env: {
          HYDRALISK_GLM_52_REAP_504B_BASE_URL: 'https://primary.example.test',
          HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret',
          HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
          HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
            'preflight.hydralisk.glm.primary.fixture',
          HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
            'receipt.hydralisk.glm.primary.fixture',
        },
        ledger,
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(report).toMatchObject({
      enabled: false,
      records: [],
      skippedReason: 'disabled',
    })
    expect(bodies).toHaveLength(1)
    expect(bodies[0]).toMatchObject({
      idempotencyKey:
        'inference:glm-pool-heartbeat:heartbeat.hydralisk.glm_52_reap_504b.20260625t160000000z:scheduled:disabled',
      model: 'openagents/glm-5.2-reap-504b',
      tokenCounts: {
        inputTokens: 0,
        outputTokens: 0,
        totalTokens: 0,
      },
    })
    expect(bodies[0]?.safeMetadata).toMatchObject({
      demandSource: 'glm-pool-heartbeat',
      enabled: false,
      heartbeatDiagnosticKind: 'scheduled_skip',
      heartbeatKind: 'glm_pool_heartbeat',
      replicaCount: 1,
      scheduledSkipReason: 'disabled',
    })
    expect(String(JSON.stringify(bodies[0]))).not.toContain('secret')
    expect(String(JSON.stringify(bodies[0]))).not.toContain(
      'primary.example.test',
    )
  })

  test('persists a canonical diagnostic row when cadence skips a scheduled heartbeat', async () => {
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runScheduledGlmPoolHeartbeat({
        env: {
          HYDRALISK_GLM_52_REAP_504B_BASE_URL: 'https://primary.example.test',
          HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret',
          HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_CADENCE_MINUTES: '7',
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED: 'true',
          HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
            'preflight.hydralisk.glm.primary.fixture',
          HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
            'receipt.hydralisk.glm.primary.fixture',
        },
        ledger,
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(report).toMatchObject({
      enabled: true,
      records: [],
      skippedReason: 'cadence',
    })
    expect(bodies).toHaveLength(1)
    expect(bodies[0]?.safeMetadata).toMatchObject({
      cadenceMinutes: 7,
      enabled: true,
      heartbeatDiagnosticKind: 'scheduled_skip',
      heartbeatKind: 'glm_pool_heartbeat',
      replicaCount: 1,
      scheduledSkipReason: 'cadence',
    })
    expect(String(JSON.stringify(bodies[0]))).not.toContain('secret')
    expect(String(JSON.stringify(bodies[0]))).not.toContain(
      'primary.example.test',
    )
  })

  test('persists a canonical diagnostic row when scheduled heartbeat is unarmed', async () => {
    const { bodies, ledger } = captureLedger()

    const report = await Effect.runPromise(
      runScheduledGlmPoolHeartbeat({
        env: {
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED: 'true',
        },
        ledger,
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(report).toMatchObject({
      enabled: true,
      records: [],
      skippedReason: 'unarmed',
    })
    expect(bodies).toHaveLength(1)
    expect(bodies[0]?.safeMetadata).toMatchObject({
      enabled: true,
      heartbeatDiagnosticKind: 'scheduled_skip',
      heartbeatKind: 'glm_pool_heartbeat',
      replicaCount: 0,
      scheduledSkipReason: 'unarmed',
    })
  })

  test('reports a typed scheduled blocker when diagnostic persistence fails', async () => {
    const report = await Effect.runPromise(
      runScheduledGlmPoolHeartbeat({
        env: {
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED: 'true',
        },
        ledger: failingLedger('TokenUsageLedgerUnsafePayload'),
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(report).toMatchObject({
      enabled: true,
      records: [],
      skippedReason: 'unarmed',
    })
    expect(report.persistenceFailures).toEqual([
      {
        errorTag: 'TokenUsageLedgerUnsafePayload',
        runRef:
          'heartbeat.hydralisk.glm_52_reap_504b.20260625t160000000z',
        stage: 'scheduled_skip',
      },
    ])
  })
})
