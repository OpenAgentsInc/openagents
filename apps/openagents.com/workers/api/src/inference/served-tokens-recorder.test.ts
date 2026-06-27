import { Effect } from 'effect'
import { describe, expect, test } from 'vitest'

import {
  type TokenUsageLedgerShape,
  TokenUsageLedgerStorageError,
  makeD1TokenUsageLedger,
} from '../token-usage-ledger'
import {
  type ServedTokensRecorder,
  type ServedTokensRecorderInput,
  buildServedTokensIngestBody,
  makeServedTokensRecorder,
  meterServedTokensFailSoft,
  servedTokensCostUsd,
  servedTokensEventId,
  servedTokensIdempotencyKey,
} from './served-tokens-recorder'

// ---------------------------------------------------------------------------
// Minimal in-memory D1 fake for the EXACT SQL the real `makeD1TokenUsageLedger`
// runs on the recorder write path + the public served-tokens counter read:
//   - findExisting: SELECT * FROM token_usage_events WHERE idempotency_key=? OR id=?
//   - insert:       INSERT INTO token_usage_events (...) VALUES (...)  (UNIQUE id/key)
//   - publicTokensServed: SELECT SUM(input)+SUM(output) AS tokens_served
// This exercises the REAL ledger (validation, idempotency, the public SUM) so
// the round trip is honest, not a stubbed projection.
// ---------------------------------------------------------------------------

type Row = Record<string, string | number | null>

const COLUMNS = [
  'id',
  'idempotency_key',
  'observed_at',
  'ingested_at',
  'producer_system',
  'source_route',
  'actor_user_id',
  'actor_team_id',
  'account_ref',
  'anonymized_source_ref',
  'run_ref',
  'session_ref',
  'task_ref',
  'repository_ref',
  'provider',
  'model',
  'backend_profile',
  'input_tokens',
  'output_tokens',
  'reasoning_tokens',
  'cache_read_tokens',
  'cache_write_5m_tokens',
  'cache_write_1h_tokens',
  'total_tokens',
  'usage_truth',
  'cost_amount',
  'currency',
  'demand_kind',
  'demand_source',
  'demand_client',
  'leaderboard_eligible',
  'privacy_opt_out',
  'safe_metadata_json',
] as const

const d1Meta = (): D1Meta & Record<string, unknown> => ({
  changed_db: true,
  changes: 1,
  duration: 0,
  last_row_id: 0,
  rows_read: 0,
  rows_written: 1,
  served_by: 'memory',
  served_by_primary: true,
  size_after: 0,
  timings: { sql_duration_ms: 0 },
})

const makeFakeDb = (rows: Array<Row> = []): D1Database => {
  const prepare = (sql: string): D1PreparedStatement => {
    let bound: ReadonlyArray<unknown> = []
    const stmt = {
      bind: (...values: Array<unknown>) => {
        bound = values
        return stmt
      },
      first: async <T>(): Promise<T | null> => {
        if (sql.includes('AS tokens_served')) {
          const tokensServed = rows.reduce(
            (sum, row) =>
              sum +
              Number(row.input_tokens ?? 0) +
              Number(row.output_tokens ?? 0),
            0,
          )
          return { tokens_served: tokensServed } as unknown as T
        }
        if (sql.includes('FROM token_usage_events') && sql.includes('WHERE')) {
          // findExisting: idempotency_key = ? OR id = ?
          const [idempotencyKey, id] = bound as [string, string]
          const found = rows.find(
            row => row.idempotency_key === idempotencyKey || row.id === id,
          )
          return (found ?? null) as unknown as T
        }
        return null as unknown as T
      },
      all: async <T>(): Promise<D1Result<T>> => ({
        meta: d1Meta(),
        results: [] as Array<T>,
        success: true,
      }),
      run: async <T>(): Promise<D1Result<T>> => {
        if (sql.trimStart().startsWith('INSERT INTO token_usage_events')) {
          const row: Row = {}
          COLUMNS.forEach((column, index) => {
            row[column] = bound[index] as string | number | null
          })
          const clash = rows.some(
            existing =>
              existing.id === row.id ||
              existing.idempotency_key === row.idempotency_key,
          )
          if (clash) {
            throw new Error('UNIQUE constraint failed: token_usage_events.id')
          }
          rows.push(row)
        }
        return { meta: d1Meta(), results: [] as Array<T>, success: true }
      },
      raw: async () => [],
    }
    return stmt as unknown as D1PreparedStatement
  }

  return {
    prepare,
    batch: async () => [],
    dump: async () => new ArrayBuffer(0),
    exec: async () => ({ count: 0, duration: 0 }),
    withSession: () => {
      throw new Error('not implemented')
    },
  } as unknown as D1Database
}

const readServed = (ledger: TokenUsageLedgerShape): Promise<number> =>
  Effect.runPromise(
    ledger
      .readPublicTokensServed()
      .pipe(Effect.map(aggregate => aggregate.tokensServed)),
  )

const fixedNow = () => '2026-06-24T00:00:00.000Z'

const runRecorder = (
  recorder: ReturnType<typeof makeServedTokensRecorder>,
  input: Parameters<ReturnType<typeof makeServedTokensRecorder>>[0],
): Promise<void> => Effect.runPromise(recorder(input))

describe('served-tokens-recorder', () => {
  test('a paid completion writes a ledger row and increases the served counter', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    const before = await readServed(ledger)
    expect(before).toBe(0)

    await runRecorder(recorder, {
      accountRef: 'agent:paid-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-paid-1',
      requestAttribution: {
        demandClient: 'qa-runner',
        demandKind: 'internal',
        demandSource: 'qa-dogfood',
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 12, totalTokens: 42 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]!.id).toBe(servedTokensEventId('chatcmpl-paid-1'))
    expect(rows[0]!.idempotency_key).toBe(
      servedTokensIdempotencyKey('chatcmpl-paid-1'),
    )
    expect(JSON.parse(String(rows[0]!.safe_metadata_json))).toMatchObject({
      demandClient: 'qa-runner',
      demandKind: 'internal',
      demandSource: 'qa-dogfood',
      requestedModel: 'openagents/khala',
    })
    expect(rows[0]!.demand_kind).toBe('internal')
    expect(rows[0]!.demand_source).toBe('qa-dogfood')
    expect(rows[0]!.demand_client).toBe('qa-runner')

    const after = await readServed(ledger)
    // The public counter sums input + output (12 + 30).
    expect(after).toBe(42)
  })

  test('own-capacity delegated coding tokens count in the public counter (#6280)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    await runRecorder(recorder, {
      accountRef: 'agent:owner-capacity-1',
      adapterId: 'pylon-codex-own-capacity',
      requestId: 'chatcmpl-own-capacity-1',
      requestAttribution: {
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/pylon-codex',
      streamed: true,
      usage: { completionTokens: 32, promptTokens: 68, totalTokens: 100 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]!.demand_kind).toBe('own_capacity')
    expect(JSON.parse(String(rows[0]!.safe_metadata_json))).toMatchObject({
      demandKind: 'own_capacity',
      demandSource: 'khala_coding_delegation',
    })
    expect(await readServed(ledger)).toBe(100)
  })

  test('MCP-issued own-capacity coding tokens count on the same public scalar (#6285)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    await runRecorder(recorder, {
      accountRef: 'agent:mcp-owner-capacity',
      adapterId: 'pylon-codex-own-capacity',
      requestId: 'chatcmpl-mcp-own-capacity',
      requestAttribution: {
        demandKind: 'own_capacity',
        demandSource: 'khala_mcp_request',
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/pylon-codex',
      streamed: true,
      usage: { completionTokens: 25, promptTokens: 75, totalTokens: 100 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]!.demand_kind).toBe('own_capacity')
    expect(rows[0]!.demand_source).toBe('khala_mcp_request')
    expect(JSON.parse(String(rows[0]!.safe_metadata_json))).toMatchObject({
      demandKind: 'own_capacity',
      demandSource: 'khala_mcp_request',
    })
    expect(await readServed(ledger)).toBe(100)
  })

  test('internal-stress Khala tokens persist distinctly and still count publicly (#6318 slice)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    await runRecorder(recorder, {
      accountRef: 'agent:stress-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-internal-stress-1',
      requestAttribution: {
        demandClient: 'stress-harness',
        demandKind: 'internal_stress',
        demandSource: 'glm-saturation',
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: true,
      usage: { completionTokens: 80, promptTokens: 20, totalTokens: 100 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]!.demand_kind).toBe('internal_stress')
    expect(rows[0]!.demand_source).toBe('glm-saturation')
    expect(rows[0]!.demand_client).toBe('stress-harness')
    expect(JSON.parse(String(rows[0]!.safe_metadata_json))).toMatchObject({
      demandClient: 'stress-harness',
      demandKind: 'internal_stress',
      demandSource: 'glm-saturation',
    })
    expect(await readServed(ledger)).toBe(100)
  })

  test('a free-tier (zero-debit) completion still counts its served tokens', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    // Free-tier calls debit no credit, but the tokens are served and must count.
    await runRecorder(recorder, {
      accountRef: 'agent:free-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-free-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: true,
      usage: { completionTokens: 100, promptTokens: 50, totalTokens: 150 },
    })

    expect(rows).toHaveLength(1)
    const after = await readServed(ledger)
    expect(after).toBe(150)
  })

  test('the same request id is idempotent (one served completion = one row)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    const call = () =>
      runRecorder(recorder, {
        accountRef: 'agent:dup-1',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-dup-1',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
      })

    await call()
    await call() // retry/replay for the SAME request id

    expect(rows).toHaveLength(1)
    const after = await readServed(ledger)
    // Counted exactly once, never double-counted.
    expect(after).toBe(30)
  })

  test('a zero-token completion is not recorded (nothing was served)', async () => {
    const rows: Array<Row> = []
    const db = makeFakeDb(rows)
    const ledger = makeD1TokenUsageLedger(db)
    const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

    await runRecorder(recorder, {
      accountRef: 'agent:empty-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-empty-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 0, promptTokens: 0, totalTokens: 0 },
    })

    expect(rows).toHaveLength(0)
    const after = await readServed(ledger)
    expect(after).toBe(0)
  })

  test('a recorder persistence failure never throws (the completion is unaffected)', async () => {
    // A ledger whose ingest always fails: the recorder must swallow it.
    const failingLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(makeFakeDb()),
      ingestEvent: () =>
        Effect.fail(
          new TokenUsageLedgerStorageError({
            error: new Error('boom'),
            operation: 'tokenUsageEvents.insert',
          }),
        ),
    }
    const recorder = makeServedTokensRecorder({
      ledger: failingLedger,
      nowIso: fixedNow,
    })

    // The recorder Effect swallows the ledger failure (it never fails), so
    // running it resolves cleanly — the completion is never affected.
    await expect(
      runRecorder(recorder, {
        accountRef: 'agent:fail-1',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-fail-1',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
      }),
    ).resolves.toBeUndefined()
  })

  // Live-counter PUSH (#6231): the recorder publishes ONE public-safe delta per
  // REAL new ledger row, and only then. ----------------------------------------

  type PublishedDelta = Readonly<{
    eventRef: string
    observedAt: string
    tokensServedDelta: number
  }>

  const recordWithPublisher = (
    rows: Array<Row>,
    published: Array<PublishedDelta>,
  ) => {
    const ledger = makeD1TokenUsageLedger(makeFakeDb(rows))
    const recorder = makeServedTokensRecorder({
      ledger,
      nowIso: fixedNow,
      publishDelta: delta =>
        Effect.sync(() => {
          published.push(delta)
        }),
    })
    return { ledger, recorder }
  }

  test('a real served completion publishes exactly one public-safe delta', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    await runRecorder(recorder, {
      accountRef: 'agent:push-1',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-push-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 12, totalTokens: 42 },
    })

    expect(published).toHaveLength(1)
    // The delta is the served input + output (12 + 30), the stable per-request
    // event ref, and a timestamp — nothing else.
    expect(published[0]).toStrictEqual({
      eventRef: servedTokensEventId('chatcmpl-push-1'),
      observedAt: fixedNow(),
      tokensServedDelta: 42,
    })
    // Public-safe: no account ref, model id, or provider in the pushed payload.
    const serialized = JSON.stringify(published[0])
    expect(serialized).not.toContain('agent:push-1')
    expect(serialized).not.toContain('khala')
    expect(serialized).not.toContain('hydralisk')
  })

  test('scheduler preemption metadata does not widen the public counter delta', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    await runRecorder(recorder, {
      accountRef: 'agent:push-preemption',
      adapterId: 'vertex-gemini',
      requestId: 'chatcmpl-push-aggregate',
      requestAttribution: {
        demandKind: 'external',
        demandSource: 'public-api',
      },
      requestMetrics: {
        schedulerPreemptionEvidenceRef:
          'scheduler.preemption.internal_stress.yield.fixture',
        schedulerPreemptionReason: 'external_reserved_headroom_unavailable',
        schedulerPreemptionTargetDemandClass: 'internal_stress',
        schedulerPreemptionTargetOutcome: 'preempted_yielded',
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 20, totalTokens: 50 },
    })

    expect(rows).toHaveLength(1)
    expect(JSON.parse(String(rows[0]!.safe_metadata_json))).toMatchObject({
      schedulerPreemptionTargetDemandClass: 'internal_stress',
      schedulerPreemptionTargetOutcome: 'preempted_yielded',
    })
    expect(published).toStrictEqual([
      {
        eventRef: servedTokensEventId('chatcmpl-push-aggregate'),
        observedAt: fixedNow(),
        tokensServedDelta: 50,
      },
    ])
    const serialized = JSON.stringify(published[0])
    expect(serialized).not.toContain('internal_stress')
    expect(serialized).not.toContain('preempt')
    expect(serialized).not.toContain('yield')
    expect(serialized).not.toContain('public-api')
  })

  test('internal dogfood records exact ledger rows and publishes a public counter delta (#6358 regression)', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    await runRecorder(recorder, {
      accountRef: 'agent:push-internal',
      adapterId: 'hydralisk',
      requestAttribution: {
        demandKind: 'internal',
        demandSource: 'heartbeat',
      },
      requestId: 'chatcmpl-push-internal',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 20, totalTokens: 50 },
    })

    expect(rows).toHaveLength(1)
    expect(rows[0]).toMatchObject({
      demand_kind: 'internal',
      demand_source: 'heartbeat',
      input_tokens: 20,
      output_tokens: 30,
    })
    expect(published).toStrictEqual([
      {
        eventRef: servedTokensEventId('chatcmpl-push-internal'),
        observedAt: fixedNow(),
        tokensServedDelta: 50,
      },
    ])
    expect(Object.keys(published[0] ?? {}).sort()).toEqual([
      'eventRef',
      'observedAt',
      'tokensServedDelta',
    ])
  })

  test('own-capacity closeouts remain counted in the all-demand public scalar (#6358)', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    await runRecorder(recorder, {
      accountRef: 'agent:push-own-capacity',
      adapterId: 'pylon-codex-own-capacity',
      requestAttribution: {
        demandKind: 'own_capacity',
        demandSource: 'khala_coding_delegation',
      },
      requestId: 'chatcmpl-push-own-capacity',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/pylon-codex',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 20, totalTokens: 50 },
    })

    expect(rows).toHaveLength(1)
    expect(published).toStrictEqual([
      {
        eventRef: servedTokensEventId('chatcmpl-push-own-capacity'),
        observedAt: fixedNow(),
        tokensServedDelta: 50,
      },
    ])
  })

  test('a duplicate (no-op) insert does NOT publish a second delta', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    const call = () =>
      runRecorder(recorder, {
        accountRef: 'agent:push-dup',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-push-dup',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 20, promptTokens: 10, totalTokens: 30 },
      })

    await call()
    await call() // replay for the SAME request id — a no-op insert.

    // Counted once, and pushed once — a replay must never double the counter.
    expect(rows).toHaveLength(1)
    expect(published).toHaveLength(1)
    expect(published[0]!.tokensServedDelta).toBe(30)
  })

  test('a zero-token completion publishes no delta (nothing was served)', async () => {
    const rows: Array<Row> = []
    const published: Array<PublishedDelta> = []
    const { recorder } = recordWithPublisher(rows, published)

    await runRecorder(recorder, {
      accountRef: 'agent:push-zero',
      adapterId: 'hydralisk',
      requestId: 'chatcmpl-push-zero',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 0, promptTokens: 0, totalTokens: 0 },
    })

    expect(rows).toHaveLength(0)
    expect(published).toHaveLength(0)
  })

  test('a failed completion (ledger ingest fails) publishes no delta', async () => {
    const published: Array<PublishedDelta> = []
    const failingLedger: TokenUsageLedgerShape = {
      ...makeD1TokenUsageLedger(makeFakeDb()),
      ingestEvent: () =>
        Effect.fail(
          new TokenUsageLedgerStorageError({
            error: new Error('boom'),
            operation: 'tokenUsageEvents.insert',
          }),
        ),
    }
    const recorder = makeServedTokensRecorder({
      ledger: failingLedger,
      nowIso: fixedNow,
      publishDelta: delta =>
        Effect.sync(() => {
          published.push(delta)
        }),
    })

    await expect(
      runRecorder(recorder, {
        accountRef: 'agent:push-fail',
        adapterId: 'hydralisk',
        requestId: 'chatcmpl-push-fail',
        requestedModel: 'openagents/khala',
        servedModel: 'openagents/khala',
        streamed: false,
        usage: { completionTokens: 5, promptTokens: 5, totalTokens: 10 },
      }),
    ).resolves.toBeUndefined()

    expect(published).toHaveLength(0)
  })

  test('the ingest body is public-safe (exact truth, no leaderboard, no secrets)', () => {
    const body = buildServedTokensIngestBody({
      accountRef: 'agent:body-1',
      adapterId: 'vertex-gemini',
      observedAt: fixedNow(),
      requestId: 'chatcmpl-body-1',
      requestAttribution: {
        demandClient: 'qa-runner',
        demandKind: 'internal',
        demandSource: 'qa-dogfood',
      },
      requestMetrics: {
        fallbackReason: 'glm_pool_saturated',
        generationWallClockMs: 1000,
        glmSaturationPolicy: 'queue_then_overflow',
        queueWaitMs: 125,
        replicaCapacityClass: 'spot',
        replicaCostProfileRef: 'cost.hydralisk.glm_52_reap_504b.g4_spot.tp4.v1',
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
        totalWallClockMs: 1500,
        ttftMs: 250,
      },
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      usage: {
        cachedPromptTokens: 7,
        completionTokens: 40,
        promptTokens: 20,
        totalTokens: 60,
      },
    })

    expect(body.usageTruth).toBe('exact')
    expect(body.privacy.leaderboardEligible).toBe(false)
    expect(body.tokenCounts.inputTokens).toBe(20)
    expect(body.tokenCounts.outputTokens).toBe(40)
    expect(body.tokenCounts.cacheReadTokens).toBe(7)
    expect(body.producerSystem).toBe('omega')
    expect(body.sourceRoute).toBe('omega_hosted_gemini')
    expect(body.safeMetadata).toMatchObject({
      demandClient: 'qa-runner',
      demandKind: 'internal',
      demandSource: 'qa-dogfood',
      fallbackReason: 'glm_pool_saturated',
      glmSaturationPolicy: 'queue_then_overflow',
      perceivedTokensPerSecond: 40,
      queueWaitMs: 125,
      requestedModel: 'openagents/khala',
      requestClass: 'interactive_stream',
      selectedReplicaRef: 'replica.hydralisk.glm_52_reap_504b.second',
      supplyLane: 'hydralisk',
      totalWallClockMs: 1500,
      ttftMs: 250,
    })
    expect(body.demand).toEqual({
      demandClient: 'qa-runner',
      demandKind: 'internal',
      demandSource: 'qa-dogfood',
    })
    expect(JSON.stringify(body)).not.toContain('prompt_text')
    expect(JSON.stringify(body)).not.toContain('https://')
    expect(JSON.stringify(body)).not.toContain('Bearer ')
  })

  test('records our marginal cost (USD) against the SERVED provider lane (#6232)', () => {
    // The real prod Khala lane is Fireworks DeepSeek V4 Flash ($0.14 in /
    // $0.28 out per Mtok). 1,000,000 in + 1,000,000 out => $0.14 + $0.28 = $0.42.
    const cost = servedTokensCostUsd(
      'accounts/fireworks/models/deepseek-v4-flash',
      {
        completionTokens: 1_000_000,
        promptTokens: 1_000_000,
        totalTokens: 2_000_000,
      },
    )

    expect(cost).toBeCloseTo(0.42, 6)
  })

  test('the ingest body carries cost_amount priced on the served model (#6232)', () => {
    const body = buildServedTokensIngestBody({
      accountRef: 'agent:cost-1',
      adapterId: 'fireworks',
      observedAt: fixedNow(),
      requestId: 'chatcmpl-cost-1',
      requestedModel: 'openagents/khala',
      servedModel: 'accounts/fireworks/models/deepseek-v4-flash',
      usage: {
        completionTokens: 1_000_000,
        promptTokens: 1_000_000,
        totalTokens: 2_000_000,
      },
    })

    expect(body.cost.currency).toBe('USD')
    expect(body.cost.amount).toBeCloseTo(0.42, 6)
  })

  // FAIL-SOFT METERING REGRESSION (#6363). The Artanis operator turn returns
  // the served reply even when the served-tokens write fails, because the
  // Khala-backed client meters through `meterServedTokensFailSoft`, which
  // swallows BOTH a recorder failure AND a recorder defect. Before this fix the
  // metering write rode inside the client Effect that the operator core converts
  // with `Effect.exit`, so any write error became a 503
  // `artanis_operator_mind_unavailable` despite Khala serving the answer.
  describe('meterServedTokensFailSoft (#6363)', () => {
    const exampleInput: ServedTokensRecorderInput = {
      accountRef: 'agent:artanis',
      adapterId: 'hydralisk',
      requestAttribution: {
        demandClient: 'artanis_operator_chat',
        demandKind: 'internal',
        demandSource: 'artanis',
      },
      requestId: 'chatcmpl-artanis-1',
      requestedModel: 'openagents/khala',
      servedModel: 'openagents/khala',
      streamed: false,
      usage: { completionTokens: 30, promptTokens: 12, totalTokens: 42 },
    }

    test('a metering WRITE FAILURE never fails the turn (succeeds with void)', async () => {
      // The recorder type pins the error channel to `never`, but at runtime a
      // ledger error can still escape as a typed failure (e.g. a recorder that
      // was composed without the swallowing matchEffect). Inject one via an
      // explicit `unknown` cast to prove the helper still succeeds.
      const failingRecorder: ServedTokensRecorder = () =>
        Effect.fail(
          new TokenUsageLedgerStorageError({
            error: 'd1 write failed',
            operation: 'ingestEvent',
          }),
        ) as unknown as ReturnType<ServedTokensRecorder>

      const outcome = await Effect.runPromiseExit(
        meterServedTokensFailSoft(failingRecorder, exampleInput),
      )

      expect(outcome._tag).toBe('Success')
    })

    test('a metering DEFECT never fails the turn (succeeds with void)', async () => {
      // The realistic production failure mode: the recorder dies with a DEFECT
      // (a synchronous throw in ingest-body build, a D1 binding throw, or a
      // sync-push throw). `Effect.exit` in the operator core captures defects,
      // so the helper MUST swallow them too.
      const dyingRecorder: ServedTokensRecorder = () =>
        Effect.die(new Error('synchronous metering defect'))

      const outcome = await Effect.runPromiseExit(
        meterServedTokensFailSoft(dyingRecorder, exampleInput),
      )

      expect(outcome._tag).toBe('Success')
    })

    test('the happy path still delegates the served-token row to the recorder', async () => {
      const rows: Array<Row> = []
      const db = makeFakeDb(rows)
      const ledger = makeD1TokenUsageLedger(db)
      const recorder = makeServedTokensRecorder({ ledger, nowIso: fixedNow })

      await Effect.runPromise(meterServedTokensFailSoft(recorder, exampleInput))

      expect(rows).toHaveLength(1)
      expect(rows[0]!.id).toBe(servedTokensEventId('chatcmpl-artanis-1'))
      expect(await readServed(ledger)).toBe(42)
    })
  })
})
