import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import type { TokenUsageLedgerShape } from '../token-usage-ledger'
import {
  type GlmPoolHeartbeatFetch,
  glmPoolHeartbeatRoutingStateOracle,
  runGlmPoolHeartbeat,
  runScheduledGlmPoolHeartbeat,
  runScheduledGlmPoolHeartbeatForD1,
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

type TokenUsageRow = Readonly<{
  account_ref: string | null
  actor_team_id: string | null
  actor_user_id: string | null
  anonymized_source_ref: string | null
  backend_profile: string | null
  cache_read_tokens: number
  cache_write_1h_tokens: number
  cache_write_5m_tokens: number
  cost_amount: number | null
  currency: string | null
  demand_client: string | null
  demand_kind: string
  demand_source: string | null
  id: string
  idempotency_key: string
  ingested_at: string
  input_tokens: number
  leaderboard_eligible: number
  model: string | null
  observed_at: string
  output_tokens: number
  privacy_opt_out: number
  producer_system: string
  provider: string | null
  reasoning_tokens: number
  repository_ref: string | null
  run_ref: string | null
  safe_metadata_json: string
  session_ref: string | null
  source_route: string
  task_ref: string | null
  total_tokens: number
  usage_truth: string
}>

const d1Result = <T>(results: Array<T> = []): D1Result<T> =>
  ({
    meta: {},
    results,
    success: true,
  }) as D1Result<T>

const recordingTokenUsageD1 = (): D1Database & {
  rows: Array<TokenUsageRow>
} => {
  const rows: Array<TokenUsageRow> = []
  const prepare = (query: string): D1PreparedStatement => {
    let values: ReadonlyArray<unknown> = []
    function raw<T = unknown[]>(options: {
      columnNames: true
    }): Promise<[Array<string>, ...Array<T>]>
    function raw<T = unknown[]>(options?: {
      columnNames?: false
    }): Promise<Array<T>>
    function raw<T = unknown[]>(options?: {
      columnNames?: boolean
    }): Promise<Array<T> | [Array<string>, ...Array<T>]> {
      return options?.columnNames === true
        ? Promise.resolve([[]])
        : Promise.resolve([])
    }
    const statement: D1PreparedStatement = {
      all: <T = unknown>() => Promise.resolve(d1Result<T>()),
      bind: (...nextValues: ReadonlyArray<unknown>) => {
        values = nextValues

        return statement
      },
      first: <T = unknown>() => {
        if (query.includes('WHERE idempotency_key = ? OR id = ?')) {
          const row = rows.find(
            candidate =>
              candidate.idempotency_key === values[0] ||
              candidate.id === values[1],
          )

          return Promise.resolve(row === undefined ? null : (row as T))
        }

        return Promise.resolve(null)
      },
      raw,
      run: <T = unknown>() => {
        if (query.includes('INSERT INTO token_usage_events')) {
          rows.push({
            account_ref: values[8] as string | null,
            actor_team_id: values[7] as string | null,
            actor_user_id: values[6] as string | null,
            anonymized_source_ref: values[9] as string | null,
            backend_profile: values[16] as string | null,
            cache_read_tokens: values[20] as number,
            cache_write_1h_tokens: values[22] as number,
            cache_write_5m_tokens: values[21] as number,
            cost_amount: values[25] as number | null,
            currency: values[26] as string | null,
            demand_client: values[30] as string | null,
            demand_kind: values[28] as string,
            demand_source: values[29] as string | null,
            id: String(values[0]),
            idempotency_key: String(values[1]),
            ingested_at: values[3] as string,
            input_tokens: values[17] as number,
            leaderboard_eligible: values[31] as number,
            model: values[15] as string | null,
            observed_at: values[2] as string,
            output_tokens: values[18] as number,
            privacy_opt_out: values[32] as number,
            producer_system: values[4] as string,
            provider: values[14] as string | null,
            reasoning_tokens: values[19] as number,
            repository_ref: values[13] as string | null,
            run_ref: values[10] as string | null,
            safe_metadata_json: values[33] as string,
            session_ref: values[11] as string | null,
            source_route: values[5] as string,
            task_ref: values[12] as string | null,
            total_tokens: values[23] as number,
            usage_truth: values[24] as string,
          })
        }

        return Promise.resolve(d1Result<T>())
      },
    }

    return statement
  }

  return {
    batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
      Promise.all(statements.map(statement => statement.run<T>())),
    dump: () => Promise.resolve(new ArrayBuffer(0)),
    exec: () => Promise.resolve({ count: 0, duration: 0 }),
    prepare,
    rows,
    withSession: () =>
      ({
        batch: <T = unknown>(statements: Array<D1PreparedStatement>) =>
          Promise.all(statements.map(statement => statement.run<T>())),
        getBookmark: () => null,
        prepare,
      }) satisfies D1DatabaseSession,
  }
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
  test('scheduled D1 wrapper emits canonical GLM pool rows for completed probes', async () => {
    const calls: Array<
      Readonly<{ body?: string; method: string; url: string }>
    > = []
    const db = recordingTokenUsageD1()

    const report = await Effect.runPromise(
      runScheduledGlmPoolHeartbeatForD1({
        db,
        env: {
          HYDRALISK_GLM_52_REAP_504B_BASE_URL: 'https://primary.example.test',
          HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret',
          HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED: 'true',
          HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
            'preflight.hydralisk.glm.primary.fixture',
          HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
            'receipt.hydralisk.glm.primary.fixture',
        },
        fetchImpl: okFetch(calls),
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(report.persistenceFailures).toEqual([])
    expect(report.records).toHaveLength(1)
    expect(db.rows).toHaveLength(1)
    expect(db.rows[0]).toMatchObject({
      demand_client: 'worker-cron',
      demand_kind: 'own_capacity',
      demand_source: 'glm-pool-heartbeat',
      model: 'openagents/glm-5.2-reap-504b',
      provider: 'hydralisk-vllm-glm-5p2-reap-504b',
      total_tokens: 0,
      usage_truth: 'exact',
    })
    expect(JSON.parse(db.rows[0]!.safe_metadata_json)).toMatchObject({
      demandSource: 'glm-pool-heartbeat',
      heartbeatKind: 'glm_pool_heartbeat',
      selectedReplicaId: 'primary',
      watchdogStatus: 'healthy',
    })

    await Effect.runPromise(
      runScheduledGlmPoolHeartbeatForD1({
        db,
        env: {
          HYDRALISK_GLM_52_REAP_504B_BASE_URL: 'https://primary.example.test',
          HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN: 'secret',
          HYDRALISK_GLM_52_REAP_504B_ENABLED: 'ready',
          HYDRALISK_GLM_52_REAP_504B_HEARTBEAT_ENABLED: 'true',
          HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF:
            'preflight.hydralisk.glm.primary.fixture',
          HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF:
            'receipt.hydralisk.glm.primary.fixture',
        },
        fetchImpl: okFetch(calls),
        scheduledTimeMs: Date.parse(OBSERVED_AT),
      }),
    )

    expect(db.rows).toHaveLength(1)
    expect(String(JSON.stringify(db.rows))).not.toContain('secret')
    expect(String(JSON.stringify(db.rows))).not.toContain(
      'primary.example.test',
    )
  })

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
