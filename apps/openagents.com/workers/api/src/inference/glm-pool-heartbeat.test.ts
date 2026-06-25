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
})

describe('runScheduledGlmPoolHeartbeat', () => {
  test('reads replica config and respects the disabled default', async () => {
    const { ledger } = captureLedger()

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
  })
})
