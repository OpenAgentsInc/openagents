// KS-8.5 (#8316): agent runtime repository CONTRACT suite.
//
// One behavioral spec, TWO implementations of `AgentRuntimeWriteStore`:
//   - D1: `makeD1AgentRuntimeWriteStore` over real SQLite (node:sqlite —
//     the engine D1 is built on), schema from the worker migrations
//     (condensed in test/sqlite-d1.ts).
//   - Postgres: `makePostgresAgentRuntimeStore` over a throwaway local
//     Postgres (initdb/pg_ctl), schema from khala-sync-server migration
//     0010. Skipped when no local Postgres binaries exist.
//
// Every case runs identically against both stores — the KS-8.5
// load-bearing properties:
//   * event-ledger dedupe keys port EXACTLY (run events: id PK +
//     (run_id, sequence) + (run_id, external_event_id); goal events: id
//     PK + partial (goal_id, external_event_id); NULL externals never
//     collide);
//   * state tables converge to the latest authoritative row;
//   * triggers upsert on the LIVE (owner, trigger_ref) arbiter and
//     REPLACE trigger_id; the replace-delete removes strays;
//   * agent_goals batches (archive + insert in one page) never trip the
//     one-active-goal-per-scope partial unique;
//   * agent_traces dedupe keys (owner+idempotency_key, owner+digest —
//     the training-consent / revenue-share keys) REJECT conflicting rows
//     on BOTH stores.

import {
  hasLocalPostgres,
  startLocalPostgres,
} from '@openagentsinc/khala-sync-server/test/local-postgres'
import { readFileSync } from 'node:fs'
import * as path from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'

import {
  makeD1AgentRuntimeWriteStore,
  makePostgresAgentRuntimeStore,
  type AgentRuntimeRow,
  type AgentRuntimeWriteStore,
} from './agent-runtime-store'
import { AGENT_RUNTIME_D1_SCHEMA, makeSqliteD1 } from './test/sqlite-d1'

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

let refCounter = 0
const nextRef = (prefix: string) => `${prefix}.contract.${++refCounter}`

const runRow = (
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  archived_at: null,
  assignment_json: '{}',
  assignment_kind: 'workroom_agent',
  auth_grant_ref: null,
  backend: 'shc_vm',
  canceled_at: null,
  completed_at: null,
  created_at: '2026-07-02T12:00:00.000Z',
  event_cursor: 0,
  external_run_id: null,
  failed_at: null,
  goal: 'contract goal',
  goal_id: null,
  id,
  project_id: null,
  provider_account_ref: null,
  repository_owner: 'OpenAgentsInc',
  repository_provider: 'github',
  repository_ref: 'main',
  repository_repo: 'openagents',
  runner_id: 'runner-contract',
  runtime: 'codex',
  started_at: null,
  status: 'queued',
  team_id: null,
  updated_at: '2026-07-02T12:00:00.000Z',
  user_id: 'user_contract',
  ...overrides,
})

const runEventRow = (
  runId: string,
  sequence: number,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  artifact_refs_json: '[]',
  created_at: `2026-07-02T12:00:0${Math.min(sequence, 9)}.000Z`,
  external_event_id: null,
  id: `${runId}:event:${sequence}`,
  payload_json: null,
  run_id: runId,
  sequence,
  source: 'runner',
  status: null,
  summary: `event ${sequence}`,
  type: 'runner.progress',
  ...overrides,
})

const goalRow = (
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  agent_id: 'agent_contract',
  archived_at: null,
  blocked_at: null,
  completed_at: null,
  created_at: '2026-07-02T12:00:00.000Z',
  current_run_id: null,
  id,
  objective: 'contract objective',
  paused_at: null,
  project_id: null,
  status: 'active',
  team_id: null,
  time_used_seconds: 0,
  token_budget: null,
  tokens_used: 0,
  updated_at: '2026-07-02T12:00:00.000Z',
  user_id: 'goal-user-contract',
  visibility: 'private',
  ...overrides,
})

const goalEventRow = (
  goalId: string,
  id: string,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  caller_type: 'runtime',
  created_at: '2026-07-02T12:00:00.000Z',
  event_type: 'UsageAccounted',
  expected_goal_id: null,
  external_event_id: null,
  goal_id: goalId,
  id,
  payload_json: null,
  run_id: null,
  status: 'active',
  time_delta_seconds: 0,
  token_delta: 10,
  ...overrides,
})

const traceRow = (
  traceUuid: string,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  agent_ref: 'agent-contract',
  blob_refs_json: '[]',
  content_digest: null,
  created_at: '2026-07-02T12:00:00.000Z',
  demand_kind: 'external',
  demand_source: null,
  idempotency_key: null,
  license: null,
  owner_user_id: 'trace-owner-contract',
  reward_amount_sats: null,
  reward_eligible: 0,
  schema_version: 'atif.v1.7',
  session_id: null,
  step_count: 3,
  trace_uuid: traceUuid,
  training_consent: 0,
  trajectory_id: `traj:${traceUuid}`,
  trajectory_json: '{"steps":3}',
  trajectory_r2_key: null,
  updated_at: '2026-07-02T12:00:00.000Z',
  upload_source: 'agent',
  visibility: 'owner_only',
  ...overrides,
})

const triggerRow = (
  ownerAgentUserId: string,
  triggerRef: string,
  definitionId: string,
  overrides: Partial<Record<string, unknown>> = {},
): AgentRuntimeRow => ({
  consecutive_failures: 0,
  created_at: '2026-07-02T12:00:00.000Z',
  definition_id: definitionId,
  next_run_at: '2026-07-03T12:00:00.000Z',
  owner_agent_user_id: ownerAgentUserId,
  owner_ref: `agent:${ownerAgentUserId}`,
  pause_reason: null,
  paused_at: null,
  state: 'enabled',
  trigger_id: `${definitionId}:${triggerRef}`,
  trigger_json: `{"kind":"cron","triggerRef":"${triggerRef}"}`,
  trigger_kind: 'cron',
  trigger_ref: triggerRef,
  updated_at: '2026-07-02T12:00:00.000Z',
  ...overrides,
})

type ContractHarness = Readonly<{
  store: AgentRuntimeWriteStore
  /** Portable read-only SQL (SELECT …) against the same store's tables. */
  query: (sql: string) => Promise<ReadonlyArray<Record<string, unknown>>>
}>

// ---------------------------------------------------------------------------
// The shared behavioral spec
// ---------------------------------------------------------------------------

const specContractSuite = (getHarness: () => ContractHarness) => {
  test('run events: fresh inserts land; every dedupe key replays as a no-op', async () => {
    const { query, store } = getHarness()
    const runId = nextRef('run')
    await store.upsertRows('agent_runs', [runRow(runId)])

    const events = [runEventRow(runId, 1), runEventRow(runId, 2)]
    expect(await store.upsertRows('agent_run_events', events)).toBe(2)
    // Exact replay: id PK dedupe.
    expect(await store.upsertRows('agent_run_events', events)).toBe(0)
    // Different id, same (run_id, sequence): ignored.
    expect(
      await store.upsertRows('agent_run_events', [
        runEventRow(runId, 1, { id: `${runId}:event:dupe-seq` }),
      ]),
    ).toBe(0)

    // external_event_id dedupe within the run; NULLs never collide.
    expect(
      await store.upsertRows('agent_run_events', [
        runEventRow(runId, 3, { external_event_id: 'gh:evt:1' }),
      ]),
    ).toBe(1)
    expect(
      await store.upsertRows('agent_run_events', [
        runEventRow(runId, 4, {
          external_event_id: 'gh:evt:1',
          id: `${runId}:event:dupe-ext`,
        }),
      ]),
    ).toBe(0)
    expect(
      await store.upsertRows('agent_run_events', [runEventRow(runId, 5)]),
    ).toBe(1)

    const rows = await query(
      `SELECT sequence, summary FROM agent_run_events WHERE run_id = '${runId}' ORDER BY sequence`,
    )
    expect(rows.map(row => Number(row.sequence))).toEqual([1, 2, 3, 5])
    // The dupe-seq replay did not clobber the original row.
    expect(rows[0]?.summary).toBe('event 1')
  })

  test('goal events: id + (goal_id, external_event_id) dedupe; NULL externals never collide', async () => {
    const { store } = getHarness()
    const goalId = nextRef('goal')
    await store.upsertRows('agent_goals', [
      goalRow(goalId, { user_id: nextRef('goal-user') }),
    ])

    expect(
      await store.upsertRows('agent_goal_events', [
        goalEventRow(goalId, `${goalId}:e1`),
        goalEventRow(goalId, `${goalId}:e2`, { external_event_id: 'ext:1' }),
      ]),
    ).toBe(2)
    expect(
      await store.upsertRows('agent_goal_events', [
        goalEventRow(goalId, `${goalId}:e1`),
      ]),
    ).toBe(0)
    expect(
      await store.upsertRows('agent_goal_events', [
        goalEventRow(goalId, `${goalId}:e3`, { external_event_id: 'ext:1' }),
      ]),
    ).toBe(0)
    expect(
      await store.upsertRows('agent_goal_events', [
        goalEventRow(goalId, `${goalId}:e4`),
      ]),
    ).toBe(1)
  })

  test('state tables converge to the latest authoritative row', async () => {
    const { query, store } = getHarness()
    const runId = nextRef('run')
    await store.upsertRows('agent_runs', [runRow(runId)])
    await store.upsertRows('agent_runs', [
      runRow(runId, {
        completed_at: '2026-07-02T13:00:00.000Z',
        event_cursor: 7,
        status: 'completed',
        updated_at: '2026-07-02T13:00:00.000Z',
      }),
    ])
    const rows = await query(
      `SELECT status, event_cursor FROM agent_runs WHERE id = '${runId}'`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.status).toBe('completed')
    expect(Number(rows[0]?.event_cursor)).toBe(7)
  })

  test('triggers upsert on (owner, trigger_ref) and REPLACE trigger_id; delete-not-in removes strays', async () => {
    const { query, store } = getHarness()
    const owner = nextRef('owner')
    await store.upsertRows('agent_definition_triggers', [
      triggerRow(owner, 'daily', 'def_v1'),
      triggerRow(owner, 'hourly', 'def_v1'),
    ])
    // Definition re-created: same trigger_ref, NEW trigger_id.
    await store.upsertRows('agent_definition_triggers', [
      triggerRow(owner, 'daily', 'def_v1', {
        trigger_id: 'def_v1:daily:v2',
        updated_at: '2026-07-02T13:00:00.000Z',
      }),
    ])
    let rows = await query(
      `SELECT trigger_id, trigger_ref FROM agent_definition_triggers WHERE owner_agent_user_id = '${owner}' ORDER BY trigger_ref`,
    )
    expect(rows.map(row => row.trigger_id)).toEqual([
      'def_v1:daily:v2',
      'def_v1:hourly',
    ])

    // Replace-delete keeps only the surviving refs...
    await store.deleteDefinitionTriggersNotIn(owner, 'def_v1', ['daily'])
    rows = await query(
      `SELECT trigger_ref FROM agent_definition_triggers WHERE owner_agent_user_id = '${owner}'`,
    )
    expect(rows.map(row => row.trigger_ref)).toEqual(['daily'])

    // ...and an empty keep-set clears the definition.
    await store.deleteDefinitionTriggersNotIn(owner, 'def_v1', [])
    rows = await query(
      `SELECT trigger_ref FROM agent_definition_triggers WHERE owner_agent_user_id = '${owner}'`,
    )
    expect(rows).toEqual([])
  })

  test('goals: an archive+insert batch never trips the one-active-per-scope unique', async () => {
    const { query, store } = getHarness()
    const scopeUser = nextRef('scope-user')
    const oldGoal = goalRow(nextRef('goal-old'), { user_id: scopeUser })
    await store.upsertRows('agent_goals', [oldGoal])

    // The setGoal batch: old goal archived + new active goal, arriving in
    // insertion order (new row can precede the archived row).
    const newGoal = goalRow(nextRef('goal-new'), {
      updated_at: '2026-07-02T13:00:00.000Z',
      user_id: scopeUser,
    })
    const archivedOld = {
      ...oldGoal,
      archived_at: '2026-07-02T13:00:00.000Z',
      updated_at: '2026-07-02T13:00:00.000Z',
    }
    expect(
      await store.upsertRows('agent_goals', [newGoal, archivedOld]),
    ).toBe(2)

    const rows = await query(
      `SELECT id FROM agent_goals WHERE user_id = '${scopeUser}' AND archived_at IS NULL`,
    )
    expect(rows).toHaveLength(1)
    expect(rows[0]?.id).toBe(newGoal['id'])
  })

  test('traces: converge by trace_uuid; owner+idempotency and owner+digest keys REJECT conflicting rows', async () => {
    const { query, store } = getHarness()
    const owner = nextRef('trace-owner')
    const uuid = nextRef('trace')
    await store.upsertRows('agent_traces', [
      traceRow(uuid, {
        content_digest: `digest:${uuid}`,
        idempotency_key: `idem:${uuid}`,
        owner_user_id: owner,
      }),
    ])

    // Visibility change converges (the owner-private field round-trips).
    await store.upsertRows('agent_traces', [
      traceRow(uuid, {
        content_digest: `digest:${uuid}`,
        idempotency_key: `idem:${uuid}`,
        owner_user_id: owner,
        updated_at: '2026-07-02T13:00:00.000Z',
        visibility: 'public',
      }),
    ])
    const rows = await query(
      `SELECT visibility FROM agent_traces WHERE trace_uuid = '${uuid}'`,
    )
    expect(rows[0]?.visibility).toBe('public')

    // A DIFFERENT trace under the same (owner, idempotency_key) must be
    // rejected by the store — the exact D1 dedupe key (training-consent /
    // revenue-share correctness rides on this).
    await expect(
      store.upsertRows('agent_traces', [
        traceRow(nextRef('trace'), {
          idempotency_key: `idem:${uuid}`,
          owner_user_id: owner,
        }),
      ]),
    ).rejects.toThrow()

    // Same for (owner, content_digest).
    await expect(
      store.upsertRows('agent_traces', [
        traceRow(nextRef('trace'), {
          content_digest: `digest:${uuid}`,
          owner_user_id: owner,
        }),
      ]),
    ).rejects.toThrow()
  })
}

// ---------------------------------------------------------------------------
// D1 (SQLite) harness
// ---------------------------------------------------------------------------

describe('agent runtime repository contract — D1 (SQLite)', () => {
  let sqlite: ReturnType<typeof makeSqliteD1>
  let harness: ContractHarness

  beforeAll(() => {
    sqlite = makeSqliteD1()
    sqlite.exec(AGENT_RUNTIME_D1_SCHEMA)
    harness = {
      query: async sql =>
        (await sqlite.db.prepare(sql).all<Record<string, unknown>>()).results ??
        [],
      store: makeD1AgentRuntimeWriteStore(sqlite.db),
    }
  })

  afterAll(() => {
    sqlite?.close()
  })

  specContractSuite(() => harness)
})

// ---------------------------------------------------------------------------
// Postgres harness (skipped without local Postgres binaries)
// ---------------------------------------------------------------------------

const MIGRATION_0010 = path.resolve(
  __dirname,
  '../../../../../packages/khala-sync-server/migrations/0010_agent_runtime.sql',
)

type PgClient = Readonly<{
  end: (options?: { timeout?: number }) => Promise<void>
  unsafe: (
    sql: string,
    params?: Array<unknown>,
  ) => Promise<Array<Record<string, unknown>>>
}>

describe.skipIf(!hasLocalPostgres())(
  'agent runtime repository contract — Postgres',
  () => {
    let pg: Awaited<ReturnType<typeof startLocalPostgres>>
    let client: PgClient | undefined
    let harness: ContractHarness

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const postgres = (await import('postgres')).default
      const admin = postgres(pg.url, { max: 1, prepare: false })
      await admin.unsafe('CREATE DATABASE agent_runtime_contract')
      await admin.end({ timeout: 5 })
      const raw = postgres(pg.urlFor('agent_runtime_contract'), {
        max: 4,
        prepare: false,
      })
      client = raw as unknown as PgClient
      await raw.unsafe(readFileSync(MIGRATION_0010, 'utf8'))
      harness = {
        query: async sql => (client as PgClient).unsafe(sql),
        store: makePostgresAgentRuntimeStore({
          acquireSql: () =>
            Promise.resolve({
              end: () => Promise.resolve(),
              sql: raw as never,
            }),
        }),
      }
    }, 120_000)

    afterAll(async () => {
      await client?.end({ timeout: 5 })
      await pg?.stop()
    }, 60_000)

    specContractSuite(() => harness)
  },
)
