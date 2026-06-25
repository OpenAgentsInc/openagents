// Owner-gated inference cost / provider-lane analytics (#6232) — SQL behavior
// test. Runs `readInferenceAnalytics` against a REAL node:sqlite database loaded
// with the real token-usage migrations, so the GROUP BY / SUM / cost-coverage
// / demand-attribution SQL is exercised for real rather than against a
// hand-rolled query mock.
import { Effect } from 'effect'
import { readFileSync } from 'node:fs'
import { DatabaseSync } from 'node:sqlite'
import { describe, expect, test } from 'vitest'

import {
  TokenUsageLedger,
  type TokenUsageLedgerFilters,
  systemTokenUsageLedgerRuntime,
} from './token-usage-ledger'

type Row = Record<string, unknown>

class SqliteD1Statement {
  private bound: ReadonlyArray<unknown> = []
  constructor(
    private readonly db: DatabaseSync,
    private readonly sql: string,
  ) {}
  bind(...values: ReadonlyArray<unknown>): SqliteD1Statement {
    this.bound = values.map(value => (value === undefined ? null : value))
    return this
  }
  async first<T = Row>(): Promise<T | null> {
    return (this.db.prepare(this.sql).get(...(this.bound as never[])) ??
      null) as T | null
  }
  async all<T = Row>(): Promise<{ results: T[] }> {
    return {
      results: this.db.prepare(this.sql).all(...(this.bound as never[])) as T[],
    }
  }
  async run<T = Row>(): Promise<{ success: true; results: T[] }> {
    this.db.prepare(this.sql).run(...(this.bound as never[]))
    return { success: true, results: [] }
  }
}

class SqliteD1 {
  constructor(private readonly db: DatabaseSync) {}
  prepare(sql: string): SqliteD1Statement {
    return new SqliteD1Statement(this.db, sql)
  }
  async batch(
    statements: ReadonlyArray<SqliteD1Statement>,
  ): Promise<Array<{ success: true }>> {
    this.db.exec('BEGIN')
    try {
      for (const statement of statements) {
        await statement.run()
      }
      this.db.exec('COMMIT')
    } catch (error) {
      this.db.exec('ROLLBACK')
      throw error
    }
    return statements.map(() => ({ success: true as const }))
  }
}

const tokenUsageEventsMigration = readFileSync(
  new URL('../migrations/0137_token_usage_events.sql', import.meta.url),
  'utf8',
)
const demandAttributionMigration = readFileSync(
  new URL(
    '../migrations/0232_token_usage_demand_attribution.sql',
    import.meta.url,
  ),
  'utf8',
)

const makeDb = (): D1Database => {
  const raw = new DatabaseSync(':memory:')
  raw.exec(`${tokenUsageEventsMigration}\n${demandAttributionMigration}`)
  return new SqliteD1(raw) as unknown as D1Database
}

const NOW = '2026-06-25T12:00:00.000Z'

// Fireworks DeepSeek V4 Flash row (the real prod Khala lane) WITH a stored cost.
const fireworksEvent = (
  overrides: Readonly<{
    eventId: string
    observedAt: string
    inputTokens: number
    outputTokens: number
    costUsd?: number | undefined
    demandClient?: string | undefined
    demandKind?:
      | 'external'
      | 'internal'
      | 'own_capacity'
      | 'unlabeled'
      | undefined
    demandSource?: string | undefined
    backendProfile?: string | undefined
    model?: string | undefined
    provider?: string | undefined
    safeMetadata?: Record<string, unknown> | undefined
  }>,
) => ({
  schemaVersion: 'openagents.token_usage_event.v1' as const,
  actor: { accountRef: 'agent:tester' },
  backendProfile: overrides.backendProfile ?? 'fireworks',
  ...(overrides.costUsd === undefined
    ? {}
    : { cost: { amount: overrides.costUsd, currency: 'USD' } }),
  ...(overrides.demandKind === undefined
    ? {}
    : {
        demand: {
          demandKind: overrides.demandKind,
          ...(overrides.demandSource === undefined
            ? {}
            : { demandSource: overrides.demandSource }),
          ...(overrides.demandClient === undefined
            ? {}
            : { demandClient: overrides.demandClient }),
        },
      }),
  eventId: overrides.eventId,
  idempotencyKey: `idem:${overrides.eventId}`,
  model: overrides.model ?? 'accounts/fireworks/models/deepseek-v4-flash',
  observedAt: overrides.observedAt,
  producerSystem: 'omega' as const,
  provider: overrides.provider ?? 'fireworks',
  ...(overrides.safeMetadata === undefined
    ? {}
    : { safeMetadata: overrides.safeMetadata }),
  sourceRoute: 'omega_hosted_gemini' as const,
  tokenCounts: {
    cacheReadTokens: 0,
    cacheWrite1hTokens: 0,
    cacheWrite5mTokens: 0,
    inputTokens: overrides.inputTokens,
    outputTokens: overrides.outputTokens,
    reasoningTokens: 0,
    totalTokens: overrides.inputTokens + overrides.outputTokens,
  },
  usageTruth: 'exact' as const,
})

const runLedger = <A>(
  db: D1Database,
  effect: Effect.Effect<A, unknown, TokenUsageLedger>,
): Promise<A> =>
  Effect.runPromise(
    effect.pipe(
      Effect.provide(TokenUsageLedger.live(db, systemTokenUsageLedgerRuntime)),
    ),
  )

const ingest = (body: unknown) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.ingestEvent(body).pipe(Effect.orDie),
  )

const analytics = (filters?: TokenUsageLedgerFilters & { window?: string }) =>
  Effect.flatMap(TokenUsageLedger, ledger =>
    ledger.readInferenceAnalytics({ now: NOW, ...filters }),
  )

const publicTokensServed = () =>
  Effect.flatMap(TokenUsageLedger, ledger => ledger.readPublicTokensServed())

describe('readInferenceAnalytics (#6232)', () => {
  test('aggregates tokens + cost by provider, model, route, and day', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.18,
          demandClient: 'gym-opencode-runner',
          demandKind: 'internal',
          demandSource: 'openagents-gym',
          eventId: 'e1',
          inputTokens: 200_000,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 500_000,
          safeMetadata: {
            requestClass: 'async_job',
            supplyLane: 'fireworks',
            totalWallClockMs: 1200,
          },
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.09,
          demandClient: 'sdk',
          demandKind: 'external',
          demandSource: 'public-api',
          eventId: 'e2',
          inputTokens: 121_065,
          observedAt: '2026-06-25T05:00:00.000Z',
          outputTokens: 310_787,
          safeMetadata: {
            fallbackReason: 'glm_pool_saturated',
            glmSaturationPolicy: 'queue_then_overflow',
            queueWaitMs: 125,
            requestClass: 'async_job',
            supplyLane: 'fireworks',
            totalWallClockMs: 800,
          },
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          backendProfile: 'hydralisk-vllm-glm-5p2-reap-504b',
          costUsd: 0.42,
          demandClient: 'gym-opencode-runner',
          demandKind: 'internal',
          demandSource: 'openagents-gym',
          eventId: 'e3-glm',
          inputTokens: 10_000,
          model: 'openagents/glm-5.2-reap-504b',
          observedAt: '2026-06-25T06:00:00.000Z',
          outputTokens: 20_000,
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          safeMetadata: {
            generationWallClockMs: 500,
            perceivedTokensPerSecond: 40_000,
            queueWaitMs: 0,
            replicaCapacityClass: 'spot',
            replicaCostProfileRef:
              'cost.hydralisk.glm_52_reap_504b.g4_spot.tp4.v1',
            replicaHealthScore: 1,
            replicaInflightCount: 1,
            replicaMaxInflight: 1,
            replicaQueueDepth: 0,
            replicaRegion: 'us-central1-a',
            replicaWarmState: 'warm',
            requestClass: 'interactive_stream',
            selectedReplicaId: 'second',
            selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.second',
            supplyLane: 'hydralisk',
            totalWallClockMs: 2400,
            ttftMs: 320,
          },
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: '7d' }))

    expect(result.schemaVersion).toBe('openagents.inference_analytics.v1')
    expect(result.window).toBe('7d')

    // byProvider collapses the two Fireworks rows and keeps the GLM adapter
    // visible as its own serving lane.
    expect(result.byProvider).toHaveLength(2)
    expect(result.byProvider[0]).toMatchObject({
      key: 'fireworks',
      inputTokens: 321_065,
      outputTokens: 810_787,
      totalTokens: 1_131_852,
      usageEvents: 2,
    })
    expect(result.byProvider[0]?.costUsd).toBeCloseTo(0.27, 6)
    expect(result.byProvider[0]?.costCoverage).toBe(1)

    expect(result.byModel[0]?.key).toBe(
      'accounts/fireworks/models/deepseek-v4-flash',
    )
    expect(result.byRoute[0]?.key).toBe('omega:omega_hosted_gemini')
    expect(result.bySupplyLane).toEqual([
      expect.objectContaining({
        key: 'fireworks',
        totalTokens: 1_131_852,
      }),
      expect.objectContaining({
        key: 'hydralisk',
        totalTokens: 30_000,
      }),
    ])
    expect(result.byAdapter).toEqual([
      expect.objectContaining({
        key: 'fireworks',
        totalTokens: 1_131_852,
      }),
      expect.objectContaining({
        key: 'hydralisk-vllm-glm-5p2-reap-504b',
        totalTokens: 30_000,
      }),
    ])
    expect(result.byGlmReplica).toEqual([
      expect.objectContaining({
        key: 'replica.hydralisk.glm_52_reap_504b.second',
        label: 'second',
        totalTokens: 30_000,
      }),
    ])
    expect(result.byRequestClass).toEqual([
      expect.objectContaining({
        key: 'async_job',
        totalTokens: 1_131_852,
        usageEvents: 2,
      }),
      expect.objectContaining({
        key: 'interactive_stream',
        totalTokens: 30_000,
        usageEvents: 1,
      }),
    ])
    expect(result.byDemandKind).toEqual([
      expect.objectContaining({
        key: 'internal',
        totalTokens: 730_000,
        usageEvents: 2,
      }),
      expect.objectContaining({
        key: 'external',
        totalTokens: 431_852,
        usageEvents: 1,
      }),
    ])
    expect(result.byDemandSource).toEqual([
      expect.objectContaining({
        key: 'internal:openagents-gym',
        totalTokens: 730_000,
      }),
      expect.objectContaining({
        key: 'external:public-api',
        totalTokens: 431_852,
      }),
    ])
    expect(result.byDemandClient).toEqual([
      expect.objectContaining({
        key: 'internal:gym-opencode-runner',
        totalTokens: 730_000,
      }),
      expect.objectContaining({
        key: 'external:sdk',
        totalTokens: 431_852,
      }),
    ])
    expect(result.byDemandClientDay).toEqual([
      expect.objectContaining({
        day: '2026-06-25',
        key: 'internal:gym-opencode-runner',
        totalTokens: 730_000,
      }),
      expect.objectContaining({
        day: '2026-06-25',
        key: 'external:sdk',
        totalTokens: 431_852,
      }),
    ])

    // byDay collapses both same-day rows into one ascending point.
    expect(result.byDay).toHaveLength(1)
    expect(result.byDay[0]).toMatchObject({
      day: '2026-06-25',
      totalTokens: 1_161_852,
      usageEvents: 3,
    })

    expect(result.operational).toMatchObject({
      busyEvents: 0,
      fallbackEvents: 1,
      fallbackRate: 0.333333,
      saturationEvents: 1,
      queueWaitMs: {
        p50Ms: 0,
        p90Ms: 125,
        sampleCount: 2,
      },
      ttftMs: {
        p50Ms: 320,
        sampleCount: 1,
      },
      perceivedTokensPerSecond: {
        p50TokensPerSecond: 40_000,
        sampleCount: 1,
      },
    })
    expect(result.glmReplicas).toEqual([
      expect.objectContaining({
        busyEvents: 0,
        capacityClass: 'spot',
        costCoverage: 1,
        costUsd: 0.42,
        effectiveCostPerServedTokenUsd: expect.any(Number),
        fallbackEvents: 0,
        idleHours: expect.any(Number),
        keepWarmStatus: 'not_measured',
        key: 'replica.hydralisk.glm_52_reap_504b.second',
        label: 'second',
        latestInflight: 1,
        latestQueueDepth: 0,
        maxInflight: 1,
        saturationEvents: 0,
        totalTokens: 30_000,
        uptimeHours: 168,
        usageEvents: 1,
        warmState: 'warm',
        watchdogStatus: 'not_measured',
      }),
    ])
    expect(result.glmReplicas[0]?.ttftMs).toMatchObject({
      p50Ms: 320,
      sampleCount: 1,
    })
    expect(result.glmReplicas[0]?.perceivedTokensPerSecond).toMatchObject({
      p50TokensPerSecond: 40_000,
      sampleCount: 1,
    })
    expect(result.glmReplicas[0]?.idleHours).toBeCloseTo(167.999333, 6)
    expect(result.glmReplicas[0]?.effectiveCostPerServedTokenUsd).toBeCloseTo(
      0.020682,
      6,
    )
    expect(result.ownedHourly).toMatchObject({
      activeDemandBurnUsd: 0.002462,
      activeServingHours: 0.000667,
      costCoverage: 'partial',
      effectiveCostPerServedTokenUsd: 0.020682,
      externalDemandBurnUsd: 0,
      hourlyBurnUsd: 3.693151,
      idleBurnUsd: 620.446906,
      idleHours: 167.999333,
      internalDemandBurnUsd: 0.002462,
      monthlyBurnUsd: 2696,
      windowBurnUsd: 620.449368,
      blockerRefs: [
        'blocker.inference_analytics.accepted_outcomes_not_measured',
        'blocker.inference_analytics.glm_benchmark_reserved_burn_not_measured',
        'blocker.inference_analytics.glm_keepwarm_burn_not_measured',
        'blocker.inference_analytics.glm_storage_overhead_not_measured',
        'blocker.inference_analytics.owned_hourly_host_lifecycle_derived_window_assumption',
      ],
    })
    expect(result.ownedHourly.profiles).toEqual([
      expect.objectContaining({
        gpuCount: 4,
        machineShape: 'g4-standard-192',
        monthlyComputeUsd: 2696,
        profileRef:
          'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25',
        provisioningModel: 'spot',
      }),
    ])
    expect(result.ownedHourly.scenarios).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          provisioningModel: 'spot',
          replicaCount: 1,
          windowBurnUsd: 620.449368,
        }),
        expect.objectContaining({
          provisioningModel: 'dws_flex',
          replicaCount: 1,
          windowBurnUsd: 1512,
        }),
        expect.objectContaining({
          provisioningModel: 'on_demand',
          replicaCount: 1,
          windowBurnUsd: 3024,
        }),
      ]),
    )
    expect(result.ownedHourly.demand).toEqual([
      expect.objectContaining({
        activeDemandBurnUsd: 0.002462,
        demandClient: 'gym-opencode-runner',
        demandKind: 'internal',
        demandSource: 'openagents-gym',
        key: 'internal:openagents-gym:gym-opencode-runner',
        totalTokens: 30_000,
      }),
    ])
    expect(result.totals.totalTokens).toBe(1_161_852)
    expect(result.totals.usageEvents).toBe(3)
    expect(result.totals.costUsd).toBeCloseTo(0.69, 6)
    // Every row carried a stored cost.
    expect(result.totals.costCoverage).toBe(1)
  })

  test('projects GLM heartbeat metadata into owner replica warmth columns (#6269)', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          backendProfile: 'hydralisk-vllm-glm-5p2-reap-504b',
          demandClient: 'worker-cron',
          demandKind: 'own_capacity',
          demandSource: 'glm-pool-heartbeat',
          eventId: 'glm-heartbeat-second',
          inputTokens: 0,
          model: 'openagents/glm-5.2-reap-504b',
          observedAt: '2026-06-25T11:56:00.000Z',
          outputTokens: 0,
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          safeMetadata: {
            heartbeatKind: 'glm_pool_heartbeat',
            heartbeatRunRef: 'heartbeat.hydralisk.glm_52_reap_504b.fixture',
            keepWarmStatus: 'skipped_benchmark_window',
            replicaWarmState: 'unknown',
            selectedReplicaId: 'second',
            selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.second',
            totalWallClockMs: 240,
            warmCompletionStatus: 'skipped',
            watchdogStatus: 'healthy',
          },
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: 'today' }))

    expect(result.glmReplicas).toEqual([
      expect.objectContaining({
        keepWarmStatus: 'skipped_benchmark_window',
        key: 'replica.hydralisk.glm_52_reap_504b.second',
        label: 'second',
        totalTokens: 0,
        usageEvents: 1,
        warmState: 'unknown',
        watchdogStatus: 'healthy',
      }),
    ])
    expect(result.byDemandKind).toEqual([
      expect.objectContaining({
        key: 'own_capacity',
        totalTokens: 0,
        usageEvents: 1,
      }),
    ])
  })

  test('amortizes owned GLM hourly burn across idle, served-token, and accepted-outcome math (#6267)', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          backendProfile: 'hydralisk-vllm-glm-5p2-reap-504b',
          costUsd: 0.42,
          demandClient: 'gym-opencode-runner',
          demandKind: 'internal',
          demandSource: 'openagents-gym',
          eventId: 'glm-owned-cost',
          inputTokens: 10_000,
          model: 'openagents/glm-5.2-reap-504b',
          observedAt: '2026-06-25T06:00:00.000Z',
          outputTokens: 20_000,
          provider: 'hydralisk-vllm-glm-5p2-reap-504b',
          safeMetadata: {
            acceptedOutcomes: 2,
            replicaCapacityClass: 'spot',
            replicaCostProfileRef:
              'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25',
            replicaMaxInflight: 1,
            replicaWarmState: 'warm',
            requestClass: 'interactive_stream',
            selectedReplicaId: 'primary',
            selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.primary',
            supplyLane: 'hydralisk',
            totalWallClockMs: 3_600_000,
          },
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: 'today' }))

    expect(result.ownedHourly).toMatchObject({
      acceptedOutcomes: 2,
      activeDemandBurnUsd: 3.693151,
      activeServingHours: 1,
      costCoverage: 'partial',
      costPerAcceptedOutcomeUsd: 22.158906,
      effectiveCostPerServedTokenUsd: 0.001477,
      hourlyBurnUsd: 3.693151,
      idleBurnUsd: 40.624661,
      idleHours: 11,
      internalDemandBurnUsd: 3.693151,
      monthlyBurnUsd: 2696,
      uptimeHours: 12,
      windowBurnUsd: 44.317812,
    })
    expect(result.ownedHourly.blockerRefs).not.toContain(
      'blocker.inference_analytics.accepted_outcomes_not_measured',
    )
    expect(result.ownedHourly.scenarios).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          costPerAcceptedOutcomeUsd: 22.158906,
          effectiveCostPerServedTokenUsd: 0.001477,
          provisioningModel: 'spot',
          windowBurnUsd: 44.317812,
        }),
        expect.objectContaining({
          costPerAcceptedOutcomeUsd: 54,
          provisioningModel: 'dws_flex',
          windowBurnUsd: 108,
        }),
        expect.objectContaining({
          costPerAcceptedOutcomeUsd: 108,
          provisioningModel: 'on_demand',
          windowBurnUsd: 216,
        }),
      ]),
    )
    expect(result.glmReplicas[0]).toMatchObject({
      effectiveCostPerServedTokenUsd: 0.001477,
      idleHours: 11,
      uptimeHours: 12,
    })
  })

  test('reports owned GLM idle burn even when no token rows were served (#6267)', async () => {
    const db = makeDb()

    const result = await runLedger(db, analytics({ window: 'today' }))

    expect(result.totals.usageEvents).toBe(0)
    expect(result.ownedHourly).toMatchObject({
      activeDemandBurnUsd: 0,
      activeServingHours: 0,
      costCoverage: 'partial',
      effectiveCostPerServedTokenUsd: 'not_measured',
      hourlyBurnUsd: 3.693151,
      idleBurnUsd: 44.317812,
      idleHours: 12,
      monthlyBurnUsd: 2696,
      uptimeHours: 12,
      windowBurnUsd: 44.317812,
    })
    expect(result.ownedHourly.demand).toEqual([])
    expect(result.ownedHourly.profiles[0]).toMatchObject({
      gpuCount: 4,
      machineShape: 'g4-standard-192',
      profileRef:
        'cost_profile.hydralisk.glm_52_reap_504b.g4_4g.spot.2026_06_25',
      provisioningModel: 'spot',
    })
  })

  test('reports cost coverage < 1 when rows predate cost recording', async () => {
    const db = makeDb()
    // One row WITH cost, one row WITHOUT (NULL cost, the historical shape).
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.1,
          eventId: 'with-cost',
          inputTokens: 1_000,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 1_000,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          eventId: 'no-cost',
          inputTokens: 1_000,
          observedAt: '2026-06-25T02:00:00.000Z',
          outputTokens: 1_000,
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: '7d' }))

    expect(result.totals.usageEvents).toBe(2)
    expect(result.totals.costUsd).toBeCloseTo(0.1, 6)
    // Half the rows carry a stored cost.
    expect(result.totals.costCoverage).toBe(0.5)
  })

  test('keeps the public token counter total-only across internal and external demand', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          demandClient: 'qa-runner',
          demandKind: 'internal',
          demandSource: 'qa-dogfood',
          eventId: 'internal',
          inputTokens: 100,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 50,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          demandClient: 'sdk',
          demandKind: 'external',
          demandSource: 'public-api',
          eventId: 'external',
          inputTokens: 20,
          observedAt: '2026-06-25T02:00:00.000Z',
          outputTokens: 10,
        }),
      ),
    )

    const result = await runLedger(db, publicTokensServed())

    expect(result).toEqual({ tokensServed: 180 })
    expect(JSON.stringify(result)).not.toContain('internal')
    expect(JSON.stringify(result)).not.toContain('external')
    expect(JSON.stringify(result)).not.toContain('qa-dogfood')
  })

  test('window=today excludes rows before UTC start of day', async () => {
    const db = makeDb()
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.05,
          eventId: 'yesterday',
          inputTokens: 10,
          observedAt: '2026-06-24T23:00:00.000Z',
          outputTokens: 10,
        }),
      ),
    )
    await runLedger(
      db,
      ingest(
        fireworksEvent({
          costUsd: 0.05,
          eventId: 'today',
          inputTokens: 10,
          observedAt: '2026-06-25T01:00:00.000Z',
          outputTokens: 10,
        }),
      ),
    )

    const result = await runLedger(db, analytics({ window: 'today' }))

    expect(result.window).toBe('today')
    expect(result.totals.usageEvents).toBe(1)
    expect(result.byDay).toHaveLength(1)
    expect(result.byDay[0]?.day).toBe('2026-06-25')
  })

  test('rejects an invalid window with a typed validation error', async () => {
    const db = makeDb()
    const outcome = await Effect.runPromise(
      analytics({ window: 'bogus' }).pipe(
        Effect.match({
          onFailure: error => error._tag,
          onSuccess: () => 'success',
        }),
        Effect.provide(
          TokenUsageLedger.live(db, systemTokenUsageLedgerRuntime),
        ),
      ),
    )

    expect(outcome).toBe('TokenUsageLedgerValidationError')
  })
})
