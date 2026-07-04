// KS-8.5 (#8316): agent runtime store — flags, fail-soft dual-write,
// read-back mirror, and read routing.
//
// Load-bearing properties:
//   * KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE defaults ON; unknown read modes
//     fall back to 'd1' (never fail open into an unproven read path);
//   * a Postgres mirror failure NEVER fails the D1-authoritative write —
//     it emits `khala_sync_agent_runtime_dual_write_failed`;
//   * diagnostics carry row KEYS only — never trajectory content, goal
//     objectives, or payload JSON (agent_traces are owner-private);
//   * with the flag off or no KHALA_SYNC_DB binding everything degrades
//     to plain D1 (zero Postgres calls);
//   * the trigger due-scan routes per KHALA_SYNC_AGENT_RUNTIME_READS:
//     compare serves D1 and logs divergence; postgres serves Postgres
//     with bounded retry and falls back to D1 on exhaustion.

import { Effect } from 'effect'
import { afterEach, beforeEach, describe, expect, test } from 'vitest'

import {
  agentRuntimeFlagsFromEnv,
  cancelActiveAgentRunsForBillingExhaustionForEnv,
  makeAgentDefinitionTriggerStoreForEnv,
  makeAgentGoalEventRepositoryForEnv,
  makeAgentRuntimeMirror,
  makeDualWriteAgentRuntimeWriteStore,
  makeTraceStoreForEnv,
  type AgentRuntimeDiagnostic,
  type AgentRuntimeDiagnosticEvent,
  type AgentRuntimeRow,
  type AgentRuntimeWriteStore,
  type PostgresAgentRuntimeStore,
} from './agent-runtime-store'
import { AGENT_RUNTIME_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fakes
// ---------------------------------------------------------------------------

type LogEntry = Readonly<{
  event: AgentRuntimeDiagnosticEvent
  fields: AgentRuntimeDiagnostic
}>

const makeLogRecorder = () => {
  const entries: Array<LogEntry> = []
  return {
    entries,
    log: (event: AgentRuntimeDiagnosticEvent, fields: AgentRuntimeDiagnostic) => {
      entries.push({ event, fields })
    },
  }
}

type UpsertCall = Readonly<{
  table: string
  rows: ReadonlyArray<AgentRuntimeRow>
}>

const makeFakePostgresStore = (
  behavior: 'ok' | 'throw' = 'ok',
): PostgresAgentRuntimeStore & {
  calls: Array<UpsertCall>
  deletes: Array<{ definitionId: string; keep: ReadonlyArray<string> }>
} => {
  const calls: Array<UpsertCall> = []
  const deletes: Array<{ definitionId: string; keep: ReadonlyArray<string> }> = []
  return {
    calls,
    deleteDefinitionTriggersNotIn: async (_owner, definitionId, keep) => {
      if (behavior === 'throw') throw new Error('pg down')
      deletes.push({ definitionId, keep })
    },
    deletes,
    listDueCronTriggerRows: async () => {
      throw new Error('not scripted')
    },
    listInboundWebhookTriggerRows: async () => {
      throw new Error('not scripted')
    },
    upsertRows: async (table, rows) => {
      if (behavior === 'throw') throw new Error('pg down')
      calls.push({ rows, table })
      return rows.length
    },
  }
}

/**
 * A scripted tagged-template SQL client for the ForEnv factories: each
 * statement pops the next scripted result (default []); every executed
 * statement's text is recorded for assertions. Throws when scripted to.
 */
const makeScriptedSqlClient = (options?: {
  results?: Array<ReadonlyArray<Record<string, unknown>>>
  throwOnEveryCall?: boolean
}) => {
  const executed: Array<string> = []
  const results = [...(options?.results ?? [])]
  const sql = (strings: TemplateStringsArray, ..._values: Array<unknown>) => {
    const text = strings.join('?').replaceAll(/\s+/g, ' ').trim()
    executed.push(text)
    if (options?.throwOnEveryCall === true) {
      return Promise.reject(new Error('pg down'))
    }
    return Promise.resolve(results.shift() ?? [])
  }
  return {
    executed,
    makeSqlClient: async (_connectionString: string) => ({
      end: () => Promise.resolve(),
      sql: sql as never,
    }),
  }
}

const TRACE_INPUT = {
  agentRef: 'agent-unit',
  blobRefs: [],
  contentDigest: null,
  demandKind: 'external' as const,
  demandSource: null,
  idempotencyKey: null,
  license: null,
  nowIso: '2026-07-04T12:00:00.000Z',
  ownerUserId: 'owner-unit',
  rewardAmountSats: null,
  rewardEligible: false,
  schemaVersion: 'atif.v1.7',
  sessionId: null,
  stepCount: 1,
  // Deliberately distinctive strings: the privacy assertions below check
  // these NEVER appear in diagnostics.
  trajectory: { secretDistinctiveTrajectoryMarker: true },
  trajectoryR2Key: null,
  traceUuid: 'trace-unit-1',
  trainingConsent: false,
  uploadSource: 'agent' as const,
  visibility: 'owner_only' as const,
  trajectoryId: 'traj-unit-1',
}

// ---------------------------------------------------------------------------
// Flags
// ---------------------------------------------------------------------------

describe('agentRuntimeFlagsFromEnv', () => {
  test('dual-write defaults ON; reads default d1', () => {
    expect(agentRuntimeFlagsFromEnv({})).toEqual({
      dualWrite: true,
      reads: 'd1',
    })
  })

  test('off values disable dual-write; read modes parse; typos fall back to d1', () => {
    for (const off of ['off', '0', 'false', 'disabled', 'no', 'OFF']) {
      expect(
        agentRuntimeFlagsFromEnv({
          KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE: off,
        }).dualWrite,
      ).toBe(false)
    }
    expect(
      agentRuntimeFlagsFromEnv({ KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE: 'on' })
        .dualWrite,
    ).toBe(true)
    expect(
      agentRuntimeFlagsFromEnv({ KHALA_SYNC_AGENT_RUNTIME_READS: 'postgres' })
        .reads,
    ).toBe('postgres')
    expect(
      agentRuntimeFlagsFromEnv({ KHALA_SYNC_AGENT_RUNTIME_READS: 'COMPARE' })
        .reads,
    ).toBe('compare')
    expect(
      agentRuntimeFlagsFromEnv({ KHALA_SYNC_AGENT_RUNTIME_READS: 'postgress' })
        .reads,
    ).toBe('d1')
  })
})

// ---------------------------------------------------------------------------
// Dual-write wrapper over the row seam
// ---------------------------------------------------------------------------

describe('makeDualWriteAgentRuntimeWriteStore', () => {
  const d1Calls: Array<UpsertCall> = []
  const d1: AgentRuntimeWriteStore = {
    deleteDefinitionTriggersNotIn: async () => {},
    upsertRows: async (table, rows) => {
      d1Calls.push({ rows, table })
      return rows.length
    },
  }

  beforeEach(() => {
    d1Calls.length = 0
  })

  test('mirrors after the D1 write; a Postgres failure never fails the write', async () => {
    const recorder = makeLogRecorder()
    const store = makeDualWriteAgentRuntimeWriteStore({
      d1,
      flags: { dualWrite: true, reads: 'd1' },
      log: recorder.log,
      postgres: makeFakePostgresStore('throw'),
    })
    const outcome = await store.upsertRows('agent_runs', [
      { id: 'run-x' } as AgentRuntimeRow,
    ])
    expect(outcome).toBe(1)
    expect(d1Calls).toHaveLength(1)
    expect(recorder.entries).toHaveLength(1)
    expect(recorder.entries[0]?.event).toBe(
      'khala_sync_agent_runtime_dual_write_failed',
    )
    expect(recorder.entries[0]?.fields.refs).toEqual(['run-x'])
  })

  test('flag off (or no postgres) bypasses the mirror entirely', async () => {
    const postgres = makeFakePostgresStore()
    const store = makeDualWriteAgentRuntimeWriteStore({
      d1,
      flags: { dualWrite: false, reads: 'd1' },
      postgres,
    })
    await store.upsertRows('agent_runs', [{ id: 'run-y' } as AgentRuntimeRow])
    expect(postgres.calls).toHaveLength(0)
  })
})

// ---------------------------------------------------------------------------
// Read-back mirror over real SQLite
// ---------------------------------------------------------------------------

describe('makeAgentRuntimeMirror', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>

  beforeEach(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
  })

  afterEach(() => {
    sqlite?.close()
  })

  test('mirrorRowsByPk reads the authoritative row back and upserts it', async () => {
    sqlite.exec(
      `INSERT INTO agent_traces (trace_uuid, owner_user_id, agent_ref, schema_version, trajectory_id, visibility, trajectory_json, created_at, updated_at)
       VALUES ('trace-a', 'owner-a', 'agent-a', 'atif.v1.7', 'traj-a', 'owner_only', '{"private":1}', '2026-07-04T00:00:00.000Z', '2026-07-04T00:00:00.000Z')`,
    )
    const postgres = makeFakePostgresStore()
    const recorder = makeLogRecorder()
    const mirror = makeAgentRuntimeMirror({
      db: sqlite.db,
      log: recorder.log,
      postgres,
    })
    await mirror.mirrorRowsByPk('agent_traces', ['trace-a'])
    expect(postgres.calls).toHaveLength(1)
    expect(postgres.calls[0]?.table).toBe('agent_traces')
    expect(postgres.calls[0]?.rows[0]?.['trace_uuid']).toBe('trace-a')
    expect(postgres.calls[0]?.rows[0]?.['visibility']).toBe('owner_only')
    expect(recorder.entries).toHaveLength(0)
  })

  test('a mirror failure is swallowed and logged with row KEYS only', async () => {
    sqlite.exec(
      `INSERT INTO agent_traces (trace_uuid, owner_user_id, agent_ref, schema_version, trajectory_id, visibility, trajectory_json, created_at, updated_at)
       VALUES ('trace-b', 'owner-b', 'agent-b', 'atif.v1.7', 'traj-b', 'owner_only', '{"secretDistinctiveTrajectoryMarker":true}', '2026-07-04T00:00:00.000Z', '2026-07-04T00:00:00.000Z')`,
    )
    const recorder = makeLogRecorder()
    const mirror = makeAgentRuntimeMirror({
      db: sqlite.db,
      log: recorder.log,
      postgres: makeFakePostgresStore('throw'),
    })
    await expect(
      mirror.mirrorRowsByPk('agent_traces', ['trace-b']),
    ).resolves.toBeUndefined()
    expect(recorder.entries).toHaveLength(1)
    const serialized = JSON.stringify(recorder.entries)
    expect(serialized).toContain('trace-b')
    // PRIVACY: no trajectory content ever reaches the diagnostic stream.
    expect(serialized).not.toContain('secretDistinctiveTrajectoryMarker')
  })

  test('mirrorAgentRunEventsSince mirrors only the appended tail', async () => {
    sqlite.exec(
      `INSERT INTO agent_run_events (id, run_id, sequence, type, summary, source, created_at) VALUES
        ('e1', 'run-a', 1, 't', 's1', 'runner', '2026-07-04T00:00:01.000Z'),
        ('e2', 'run-a', 2, 't', 's2', 'runner', '2026-07-04T00:00:02.000Z'),
        ('e3', 'run-a', 3, 't', 's3', 'runner', '2026-07-04T00:00:03.000Z')`,
    )
    const postgres = makeFakePostgresStore()
    const mirror = makeAgentRuntimeMirror({
      db: sqlite.db,
      log: makeLogRecorder().log,
      postgres,
    })
    await mirror.mirrorAgentRunEventsSince('run-a', 2)
    expect(postgres.calls[0]?.rows.map(row => row['id'])).toEqual(['e2', 'e3'])
  })

  test('mirrorTriggersForDefinition mirrors replace semantics (delete-not-in + full set)', async () => {
    sqlite.exec(
      `INSERT INTO agent_definition_triggers (trigger_id, owner_agent_user_id, owner_ref, definition_id, trigger_ref, trigger_kind, trigger_json, state, created_at, updated_at)
       VALUES ('def1:daily', 'owner-1', 'agent:owner-1', 'def1', 'daily', 'cron', '{}', 'enabled', '2026-07-04T00:00:00.000Z', '2026-07-04T00:00:00.000Z')`,
    )
    const postgres = makeFakePostgresStore()
    const mirror = makeAgentRuntimeMirror({
      db: sqlite.db,
      log: makeLogRecorder().log,
      postgres,
    })
    await mirror.mirrorTriggersForDefinition('owner-1', 'def1')
    expect(postgres.deletes).toEqual([{ definitionId: 'def1', keep: ['daily'] }])
    expect(postgres.calls[0]?.rows.map(row => row['trigger_ref'])).toEqual([
      'daily',
    ])
  })

  test('mirrorGoalScope mirrors the archive+insert batch for one scope', async () => {
    sqlite.exec(
      `INSERT INTO agent_goals (id, agent_id, user_id, objective, status, visibility, created_at, updated_at, archived_at) VALUES
        ('g-old', 'agent-1', 'u-1', 'old objective', 'active', 'private', '2026-07-04T00:00:00.000Z', '2026-07-04T01:00:00.000Z', '2026-07-04T01:00:00.000Z'),
        ('g-new', 'agent-1', 'u-1', 'new objective', 'active', 'private', '2026-07-04T01:00:00.000Z', '2026-07-04T01:00:00.000Z', NULL),
        ('g-other', 'agent-1', 'u-2', 'other scope', 'active', 'private', '2026-07-04T00:00:00.000Z', '2026-07-04T00:00:00.000Z', NULL)`,
    )
    const postgres = makeFakePostgresStore()
    const mirror = makeAgentRuntimeMirror({
      db: sqlite.db,
      log: makeLogRecorder().log,
      postgres,
    })
    await mirror.mirrorGoalScope({ agentId: 'agent-1', userId: 'u-1' })
    const ids = postgres.calls[0]?.rows.map(row => row['id'])
    expect(ids).toContain('g-old')
    expect(ids).toContain('g-new')
    expect(ids).not.toContain('g-other')
  })
})

// ---------------------------------------------------------------------------
// ForEnv factories (flag wiring end-to-end over SQLite + scripted PG)
// ---------------------------------------------------------------------------

describe('makeTraceStoreForEnv', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>

  beforeEach(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
  })

  afterEach(() => {
    sqlite?.close()
  })

  test('createTrace mirrors the fresh row to Postgres', async () => {
    const scripted = makeScriptedSqlClient()
    const store = makeTraceStoreForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: scripted.makeSqlClient },
    )
    const result = await store.createTrace(TRACE_INPUT)
    expect(result.created).toBe(true)
    expect(
      scripted.executed.some(text => text.startsWith('INSERT INTO agent_traces')),
    ).toBe(true)
  })

  test('dual-write off (or missing binding) → plain D1, zero Postgres calls', async () => {
    const scripted = makeScriptedSqlClient()
    const offStore = makeTraceStoreForEnv(
      {
        KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE: 'off',
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: scripted.makeSqlClient },
    )
    const result = await offStore.createTrace(TRACE_INPUT)
    expect(result.created).toBe(true)
    expect(scripted.executed).toHaveLength(0)

    const noBinding = makeTraceStoreForEnv(
      { OPENAGENTS_DB: sqlite.db },
      { makeSqlClient: scripted.makeSqlClient },
    )
    expect(
      (await noBinding.readTraceByUuid(TRACE_INPUT.traceUuid))?.ownerUserId,
    ).toBe(TRACE_INPUT.ownerUserId)
    expect(scripted.executed).toHaveLength(0)
  })

  test('a Postgres outage never fails the trace write; diagnostics stay key-only', async () => {
    const recorder = makeLogRecorder()
    const scripted = makeScriptedSqlClient({ throwOnEveryCall: true })
    const store = makeTraceStoreForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { log: recorder.log, makeSqlClient: scripted.makeSqlClient },
    )
    const result = await store.createTrace(TRACE_INPUT)
    expect(result.created).toBe(true)
    expect(recorder.entries).toHaveLength(1)
    expect(recorder.entries[0]?.event).toBe(
      'khala_sync_agent_runtime_dual_write_failed',
    )
    const serialized = JSON.stringify(recorder.entries)
    expect(serialized).toContain(TRACE_INPUT.traceUuid)
    expect(serialized).not.toContain('secretDistinctiveTrajectoryMarker')
  })
})

describe('makeAgentGoalEventRepositoryForEnv', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>

  beforeEach(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
    sqlite.exec(
      `INSERT INTO agent_goals (id, agent_id, user_id, objective, status, visibility, created_at, updated_at)
       VALUES ('goal-1', 'agent-1', 'u-1', 'objective', 'active', 'private', '2026-07-04T00:00:00.000Z', '2026-07-04T00:00:00.000Z')`,
    )
  })

  afterEach(() => {
    sqlite?.close()
  })

  test('recordOnce mirrors a fresh event and skips the duplicate replay', async () => {
    const scripted = makeScriptedSqlClient()
    const repo = makeAgentGoalEventRepositoryForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      undefined,
      { makeSqlClient: scripted.makeSqlClient },
    )
    const input = {
      callerType: 'runtime' as const,
      eventType: 'WorkerResumed',
      externalEventId: 'goal:goal-1:continuation:1',
      goalId: 'goal-1',
    }
    const first = await Effect.runPromise(repo.recordOnce(input))
    expect(first).toBeDefined()
    const insertsAfterFirst = scripted.executed.filter(text =>
      text.startsWith('INSERT INTO agent_goal_events'),
    ).length
    expect(insertsAfterFirst).toBe(1)

    const second = await Effect.runPromise(repo.recordOnce(input))
    expect(second).toBeUndefined()
    const insertsAfterSecond = scripted.executed.filter(text =>
      text.startsWith('INSERT INTO agent_goal_events'),
    ).length
    expect(insertsAfterSecond).toBe(1)
  })
})

describe('makeAgentDefinitionTriggerStoreForEnv read routing', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>

  const seedTrigger = () => {
    sqlite.exec(
      `INSERT INTO agent_definition_triggers (trigger_id, owner_agent_user_id, owner_ref, definition_id, trigger_ref, trigger_kind, trigger_json, state, consecutive_failures, next_run_at, created_at, updated_at)
       VALUES ('def1:daily', 'owner-1', 'agent:owner-1', 'def1', 'daily', 'cron', '{"kind":"cron","triggerRef":"daily","expr":"0 12 * * *","tz":"UTC"}', 'enabled', 0, '2026-07-04T00:00:00.000Z', '2026-07-03T00:00:00.000Z', '2026-07-03T00:00:00.000Z')`,
    )
  }

  const pgTriggerRow = (overrides: Partial<Record<string, unknown>> = {}) => ({
    consecutive_failures: 0,
    created_at: '2026-07-03T00:00:00.000Z',
    definition_id: 'def1',
    next_run_at: '2026-07-04T00:00:00.000Z',
    owner_agent_user_id: 'owner-1',
    owner_ref: 'agent:owner-1',
    pause_reason: null,
    paused_at: null,
    state: 'enabled',
    trigger_id: 'def1:daily',
    trigger_json:
      '{"kind":"cron","triggerRef":"daily","expr":"0 12 * * *","tz":"UTC"}',
    trigger_ref: 'daily',
    updated_at: '2026-07-03T00:00:00.000Z',
    ...overrides,
  })

  beforeEach(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
    seedTrigger()
  })

  afterEach(() => {
    sqlite?.close()
  })

  test("default reads ('d1') never touch Postgres", async () => {
    const scripted = makeScriptedSqlClient()
    const store = makeAgentDefinitionTriggerStoreForEnv(
      {
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: scripted.makeSqlClient },
    )
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    expect(due).toHaveLength(1)
    expect(
      scripted.executed.filter(text => text.startsWith('SELECT')),
    ).toHaveLength(0)
  })

  test('postgres mode serves Postgres rows decoded through the same mapper', async () => {
    const scripted = makeScriptedSqlClient({ results: [[pgTriggerRow()]] })
    const store = makeAgentDefinitionTriggerStoreForEnv(
      {
        KHALA_SYNC_AGENT_RUNTIME_READS: 'postgres',
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { makeSqlClient: scripted.makeSqlClient },
    )
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    expect(due).toHaveLength(1)
    expect(due[0]?.ownerAgentUserId).toBe('owner-1')
    expect(due[0]?.triggerRef).toBe('daily')
  })

  test('postgres mode retries then falls back to D1 on exhaustion (never throws)', async () => {
    const recorder = makeLogRecorder()
    const scripted = makeScriptedSqlClient({ throwOnEveryCall: true })
    const store = makeAgentDefinitionTriggerStoreForEnv(
      {
        KHALA_SYNC_AGENT_RUNTIME_READS: 'postgres',
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      {
        log: recorder.log,
        makeSqlClient: scripted.makeSqlClient,
        wait: () => Promise.resolve(),
      },
    )
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    expect(due).toHaveLength(1)
    expect(due[0]?.triggerId).toBe('def1:daily')
    const events = recorder.entries.map(entry => entry.event)
    expect(
      events.filter(e => e === 'khala_sync_agent_runtime_postgres_read_failed'),
    ).toHaveLength(2)
    expect(events).toContain('khala_sync_agent_runtime_postgres_read_fallback')
  })

  test('compare mode serves D1 and logs divergence', async () => {
    const recorder = makeLogRecorder()
    const scripted = makeScriptedSqlClient({
      results: [[pgTriggerRow({ next_run_at: '2026-07-09T00:00:00.000Z' })]],
    })
    const store = makeAgentDefinitionTriggerStoreForEnv(
      {
        KHALA_SYNC_AGENT_RUNTIME_READS: 'compare',
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { log: recorder.log, makeSqlClient: scripted.makeSqlClient },
    )
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    // Served from D1 authority.
    expect(due[0]?.nextRunAt).toBe('2026-07-04T00:00:00.000Z')
    expect(recorder.entries.map(entry => entry.event)).toContain(
      'khala_sync_agent_runtime_read_compare_mismatch',
    )
  })

  test('compare mode with matching rows stays silent', async () => {
    const recorder = makeLogRecorder()
    const scripted = makeScriptedSqlClient({ results: [[pgTriggerRow()]] })
    const store = makeAgentDefinitionTriggerStoreForEnv(
      {
        KHALA_SYNC_AGENT_RUNTIME_READS: 'compare',
        KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
        OPENAGENTS_DB: sqlite.db,
      },
      { log: recorder.log, makeSqlClient: scripted.makeSqlClient },
    )
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    expect(due).toHaveLength(1)
    expect(recorder.entries).toHaveLength(0)
  })

  test('no binding → plain D1 even when reads=postgres', async () => {
    const store = makeAgentDefinitionTriggerStoreForEnv({
      KHALA_SYNC_AGENT_RUNTIME_READS: 'postgres',
      OPENAGENTS_DB: sqlite.db,
    })
    const due = await store.listDueCronTriggers('2026-07-05T00:00:00.000Z', 10)
    expect(due).toHaveLength(1)
  })
})

// ---------------------------------------------------------------------------
// Billing cancel sweep wrapper (flag-off path)
// ---------------------------------------------------------------------------

describe('cancelActiveAgentRunsForBillingExhaustionForEnv', () => {
  test('with dual-write off it degrades to the plain D1 sweep', async () => {
    const sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
    try {
      const scripted = makeScriptedSqlClient()
      const canceled = await cancelActiveAgentRunsForBillingExhaustionForEnv(
        {
          KHALA_SYNC_AGENT_RUNTIME_DUAL_WRITE: 'off',
          KHALA_SYNC_DB: { connectionString: 'postgres://fake' },
          OPENAGENTS_DB: sqlite.db,
        },
        'user-none',
        { balanceCents: 0, balanceFormatted: '$0.00' },
        { makeSqlClient: scripted.makeSqlClient },
      )
      expect(canceled).toEqual([])
      expect(scripted.executed).toHaveLength(0)
    } finally {
      sqlite.close()
    }
  })
})
