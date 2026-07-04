// KS-8.5 (#8316): agent runtime backfill core — idempotency + verify
// fidelity.
//
// Load-bearing properties: running the same event page twice yields an
// IDENTICAL Postgres state (second run inserts zero rows), the event
// ledgers NEVER clobber a dual-write-mirrored row, state-table converge
// upserts converge to the D1 snapshot value (re-runs are stable; triggers
// converge on the live (owner, trigger_ref) arbiter and REPLACE
// trigger_id; goal pages order archived-first past the scope unique), and
// the verify comparators catch count / tally / event-chain / hash drift
// exactly. Privacy: no assertion here ever prints trajectory content —
// hashes and keys only, same as the CLI.

import { SQL } from "bun"
import {
  afterAll,
  beforeAll,
  describe,
  expect,
  setDefaultTimeout,
  test,
} from "bun:test"
import { runMigrations } from "./migrate.js"
import {
  AGENT_RUNTIME_SCALAR_TALLIES,
  agentRuntimeRowHash,
  agentRuntimeVerifyReportClean,
  buildAgentRuntimeVerifyReport,
  compareEventChains,
  d1AgentRuntimeNewestHashes,
  eventChainTallyFromRows,
  postgresAgentRuntimeNewestHashes,
  postgresAgentRuntimeRowCount,
  postgresAgentRuntimeScalar,
  postgresEventChainTally,
  upsertAgentRuntimeRows,
  type D1SourceRow,
} from "./agent-runtime-backfill.js"
import type { SyncSql } from "./sql.js"
import { hasLocalPostgres, startLocalPostgres } from "./test/local-postgres.js"
import type { LocalPostgres } from "./test/local-postgres.js"

setDefaultTimeout(120_000)

const runRow = (n: number, overrides: Partial<Record<string, unknown>> = {}): D1SourceRow => ({
  archived_at: null,
  assignment_json: "{}",
  assignment_kind: "workroom_agent",
  auth_grant_ref: null,
  backend: "shc_vm",
  canceled_at: null,
  completed_at: null,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  event_cursor: 0,
  external_run_id: null,
  failed_at: null,
  goal: `backfill goal ${n}`,
  goal_id: null,
  id: `agent_run_backfill_${n}`,
  project_id: null,
  provider_account_ref: null,
  repository_owner: "OpenAgentsInc",
  repository_provider: "github",
  repository_ref: "main",
  repository_repo: "openagents",
  runner_id: `runner-${n}`,
  runtime: "codex",
  started_at: null,
  status: "queued",
  team_id: null,
  updated_at: `2026-07-01T0${n}:00:00.000Z`,
  user_id: `user-${n}`,
  ...overrides,
})

const runEventRow = (
  runId: string,
  sequence: number,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  artifact_refs_json: "[]",
  created_at: `2026-07-01T01:00:0${sequence}.000Z`,
  external_event_id: null,
  id: `${runId}_event_${sequence}`,
  payload_json: null,
  run_id: runId,
  sequence,
  source: "runner",
  status: null,
  summary: `event ${sequence}`,
  type: "runner.progress",
  ...overrides,
})

const traceRow = (n: number, overrides: Partial<Record<string, unknown>> = {}): D1SourceRow => ({
  agent_ref: `agent-${n}`,
  blob_refs_json: "[]",
  content_digest: `digest-${n}`,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  demand_kind: n === 2 ? "internal" : "external",
  demand_source: null,
  idempotency_key: `trace-idem-${n}`,
  license: null,
  owner_user_id: `owner-${n}`,
  reward_amount_sats: null,
  reward_eligible: 0,
  schema_version: "atif.v1.7",
  session_id: null,
  step_count: 3 * n,
  trace_uuid: `trace_backfill_${n}`,
  training_consent: n === 1 ? 1 : 0,
  trajectory_id: `traj-${n}`,
  trajectory_json: `{"steps":${n}}`,
  trajectory_r2_key: null,
  updated_at: `2026-07-01T0${n}:00:00.000Z`,
  upload_source: "agent",
  visibility: n === 2 ? "owner_only" : "unlisted",
  ...overrides,
})

const goalRow = (n: number, overrides: Partial<Record<string, unknown>> = {}): D1SourceRow => ({
  agent_id: "agent_shared",
  archived_at: null,
  blocked_at: null,
  completed_at: null,
  created_at: `2026-07-01T0${n}:00:00.000Z`,
  current_run_id: null,
  id: `agent_goal_backfill_${n}`,
  objective: `objective ${n}`,
  paused_at: null,
  project_id: null,
  status: "active",
  team_id: null,
  time_used_seconds: 0,
  token_budget: null,
  tokens_used: 100 * n,
  updated_at: `2026-07-01T0${n}:00:00.000Z`,
  user_id: `goal-user-${n}`,
  visibility: "private",
  ...overrides,
})

const triggerRow = (
  triggerRef: string,
  overrides: Partial<Record<string, unknown>> = {},
): D1SourceRow => ({
  consecutive_failures: 0,
  created_at: "2026-07-01T01:00:00.000Z",
  definition_id: "agent_def_backfill_1",
  next_run_at: "2026-07-02T01:00:00.000Z",
  owner_agent_user_id: "owner-1",
  owner_ref: "agent:owner-1",
  pause_reason: null,
  paused_at: null,
  state: "enabled",
  trigger_id: `agent_def_backfill_1:${triggerRef}`,
  trigger_json: `{"kind":"cron","triggerRef":"${triggerRef}"}`,
  trigger_kind: "cron",
  trigger_ref: triggerRef,
  updated_at: "2026-07-01T01:00:00.000Z",
  ...overrides,
})

describe("agentRuntimeRowHash (pure)", () => {
  test("identical rows hash identically; any column change diverges", () => {
    const a = traceRow(1)
    const b = traceRow(1)
    expect(agentRuntimeRowHash("agent_traces", a)).toBe(
      agentRuntimeRowHash("agent_traces", b),
    )
    expect(
      agentRuntimeRowHash("agent_traces", { ...a, visibility: "public" }),
    ).not.toBe(agentRuntimeRowHash("agent_traces", a))
    expect(
      agentRuntimeRowHash("agent_traces", { ...a, trajectory_json: '{"steps":9}' }),
    ).not.toBe(agentRuntimeRowHash("agent_traces", a))
  })

  test("extra columns in the D1 export (e.g. d1_rowid) do not affect the hash", () => {
    const row = runRow(1)
    expect(agentRuntimeRowHash("agent_runs", { ...row, d1_rowid: 42 })).toBe(
      agentRuntimeRowHash("agent_runs", row),
    )
  })
})

describe("event chain comparison (pure)", () => {
  test("contiguous identical chains compare clean; a dropped event is caught", () => {
    const d1 = eventChainTallyFromRows([
      { distinct_sequences: 3, events: 3, max_sequence: 3, min_sequence: 1, parent_id: "run_a" },
    ])
    const samePg = eventChainTallyFromRows([
      { distinct_sequences: 3, events: 3, max_sequence: 3, min_sequence: 1, parent_id: "run_a" },
    ])
    expect(compareEventChains(d1, samePg)).toEqual([])
    expect(d1.gappedChains).toBe(0)

    const gappedPg = eventChainTallyFromRows([
      { distinct_sequences: 2, events: 2, max_sequence: 3, min_sequence: 1, parent_id: "run_a" },
    ])
    expect(gappedPg.gappedChains).toBe(1)
    expect(compareEventChains(d1, gappedPg)).toHaveLength(1)

    const missingParent = eventChainTallyFromRows([])
    expect(compareEventChains(d1, missingParent)).toHaveLength(1)
  })
})

describe.skipIf(!hasLocalPostgres())(
  "agent runtime backfill against local Postgres",
  () => {
    let pg: LocalPostgres
    let rawSql: SQL
    let sql: SyncSql

    beforeAll(async () => {
      pg = await startLocalPostgres()
      const admin = new SQL({ url: pg.url, max: 1 })
      await admin.unsafe("CREATE DATABASE khala_agent_runtime_backfill")
      await admin.end()
      const url = pg.urlFor("khala_agent_runtime_backfill")
      const result = await runMigrations({ databaseUrl: url })
      expect(result.applied).toContain("0010_agent_runtime.sql")
      rawSql = new SQL({ url, max: 4 })
      sql = rawSql as unknown as SyncSql
    })

    afterAll(async () => {
      await rawSql?.end()
      await pg?.stop()
    })

    test("run events: run twice → identical state (idempotency), chains contiguous", async () => {
      await upsertAgentRuntimeRows(sql, "agent_runs", [runRow(1), runRow(2)])
      const page = [
        runEventRow("agent_run_backfill_1", 1),
        runEventRow("agent_run_backfill_1", 2),
        runEventRow("agent_run_backfill_1", 3),
        runEventRow("agent_run_backfill_2", 1),
      ]
      const first = await upsertAgentRuntimeRows(sql, "agent_run_events", page)
      expect(first).toBe(4)

      const chainsAfterFirst = await postgresEventChainTally(sql, "agent_run_events")
      const hashesAfterFirst = await postgresAgentRuntimeNewestHashes(
        sql,
        "agent_run_events",
        10,
      )

      const second = await upsertAgentRuntimeRows(sql, "agent_run_events", page)
      expect(second).toBe(0)

      expect(await postgresEventChainTally(sql, "agent_run_events")).toEqual(
        chainsAfterFirst,
      )
      expect(
        await postgresAgentRuntimeNewestHashes(sql, "agent_run_events", 10),
      ).toEqual(hashesAfterFirst)
      expect(chainsAfterFirst.gappedChains).toBe(0)
      expect(chainsAfterFirst.totalEvents).toBe(4)
    })

    test("run events: DO NOTHING never clobbers a dual-write-mirrored row", async () => {
      await upsertAgentRuntimeRows(sql, "agent_run_events", [
        runEventRow("agent_run_backfill_1", 1, { summary: "stale backfill copy" }),
      ])
      const rows = await rawSql`
        SELECT summary FROM agent_run_events
         WHERE id = 'agent_run_backfill_1_event_1'`
      expect(rows[0]?.summary).toBe("event 1")
    })

    test("run events: the dedupe key set ports exactly (run+sequence, run+external id)", async () => {
      // A different event id colliding on (run_id, sequence) must be ignored.
      const inserted = await upsertAgentRuntimeRows(sql, "agent_run_events", [
        runEventRow("agent_run_backfill_2", 1, { id: "other_id_same_sequence" }),
      ])
      expect(inserted).toBe(0)

      // external_event_id dedupe within a run; NULLs stay non-colliding.
      const withExternal = await upsertAgentRuntimeRows(sql, "agent_run_events", [
        runEventRow("agent_run_backfill_2", 2, { external_event_id: "gh:1" }),
      ])
      expect(withExternal).toBe(1)
      const replayed = await upsertAgentRuntimeRows(sql, "agent_run_events", [
        runEventRow("agent_run_backfill_2", 3, {
          external_event_id: "gh:1",
          id: "replay_same_external",
        }),
      ])
      expect(replayed).toBe(0)
    })

    test("state tables: converge upsert converges to the D1 snapshot and re-runs stably", async () => {
      await upsertAgentRuntimeRows(sql, "agent_runs", [
        runRow(1, { event_cursor: 3, status: "running", updated_at: "2026-07-01T02:00:00.000Z" }),
      ])
      let rows = await rawSql`
        SELECT status, event_cursor FROM agent_runs WHERE id = 'agent_run_backfill_1'`
      expect(rows[0]?.status).toBe("running")
      expect(Number(rows[0]?.event_cursor)).toBe(3)

      // Later D1 snapshot converges the same row forward.
      await upsertAgentRuntimeRows(sql, "agent_runs", [
        runRow(1, {
          completed_at: "2026-07-01T03:00:00.000Z",
          event_cursor: 5,
          status: "completed",
          updated_at: "2026-07-01T03:00:00.000Z",
        }),
      ])
      rows = await rawSql`
        SELECT status, event_cursor FROM agent_runs WHERE id = 'agent_run_backfill_1'`
      expect(rows[0]?.status).toBe("completed")
      expect(Number(rows[0]?.event_cursor)).toBe(5)
    })

    test("triggers: converge on the live (owner, trigger_ref) arbiter replaces trigger_id", async () => {
      await upsertAgentRuntimeRows(sql, "agent_definition_triggers", [
        triggerRow("daily"),
      ])
      // The live path re-registers the same trigger_ref under a new
      // trigger_id (definition re-created) — the converge must REPLACE.
      await upsertAgentRuntimeRows(sql, "agent_definition_triggers", [
        triggerRow("daily", {
          definition_id: "agent_def_backfill_2",
          trigger_id: "agent_def_backfill_2:daily",
          updated_at: "2026-07-01T02:00:00.000Z",
        }),
      ])
      const rows = await rawSql`
        SELECT trigger_id, definition_id FROM agent_definition_triggers
         WHERE owner_agent_user_id = 'owner-1' AND trigger_ref = 'daily'`
      expect(rows).toHaveLength(1)
      expect(rows[0]?.trigger_id).toBe("agent_def_backfill_2:daily")
    })

    test("goals: archived-first page ordering never trips the scope unique", async () => {
      // Same scope: the archived predecessor and its active successor land
      // in ONE page, active row listed first in D1 rowid order.
      const active = goalRow(1, {
        id: "goal_scope_active",
        user_id: "scope-user",
      })
      const archived = goalRow(1, {
        archived_at: "2026-07-01T02:00:00.000Z",
        id: "goal_scope_archived",
        user_id: "scope-user",
      })
      const touched = await upsertAgentRuntimeRows(sql, "agent_goals", [
        active,
        archived,
      ])
      expect(touched).toBe(2)
      const rows = await rawSql`
        SELECT id FROM agent_goals
         WHERE user_id = 'scope-user' AND archived_at IS NULL`
      expect(rows).toHaveLength(1)
      expect(rows[0]?.id).toBe("goal_scope_active")
    })

    test("traces: content-hash sample + visibility/consent tallies are exact", async () => {
      await upsertAgentRuntimeRows(sql, "agent_traces", [traceRow(1), traceRow(2)])

      // Idempotent re-run converges (no drift).
      await upsertAgentRuntimeRows(sql, "agent_traces", [traceRow(1)])
      expect(await postgresAgentRuntimeRowCount(sql, "agent_traces")).toBe(2)

      const tallies = AGENT_RUNTIME_SCALAR_TALLIES["agent_traces"]
      const byMetric: Record<string, number> = {}
      for (const tally of tallies) {
        byMetric[tally.metric] = await postgresAgentRuntimeScalar(sql, tally.sql)
      }
      expect(byMetric["public_traces"]).toBe(0)
      expect(byMetric["owner_only_traces"]).toBe(1)
      expect(byMetric["training_consented_traces"]).toBe(1)
      expect(byMetric["sum_step_count"]).toBe(9)
      expect(byMetric["distinct_content_digests"]).toBe(2)

      // The newest-N hash sample: the D1 export twin of the same rows
      // hashes identically (the KS-8.5 trace content-hash evidence).
      const postgresNewest = await postgresAgentRuntimeNewestHashes(
        sql,
        "agent_traces",
        10,
      )
      const d1Newest = d1AgentRuntimeNewestHashes("agent_traces", [
        traceRow(2),
        traceRow(1),
      ])
      const report = buildAgentRuntimeVerifyReport({
        d1Newest,
        d1Total: 2,
        postgresNewest,
        postgresTotal: 2,
        scalars: [],
        table: "agent_traces",
      })
      expect(report.newestHashMismatches).toEqual([])
      expect(agentRuntimeVerifyReportClean(report)).toBe(true)
    })

    test("verify report: count, scalar, and hash drift are caught exactly", async () => {
      const report = buildAgentRuntimeVerifyReport({
        d1Newest: d1AgentRuntimeNewestHashes("agent_goals", [goalRow(9)]),
        d1Total: 3,
        postgresNewest: [],
        postgresTotal: 2,
        scalars: [{ d1: 100, metric: "sum_tokens_used", postgres: 90 }],
        table: "agent_goals",
      })
      expect(report.countsMatch).toBe(false)
      expect(report.scalarMismatches).toHaveLength(1)
      expect(report.newestHashMismatches).toHaveLength(1)
      expect(agentRuntimeVerifyReportClean(report)).toBe(false)
    })
  },
)
