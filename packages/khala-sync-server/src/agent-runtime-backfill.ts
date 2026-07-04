/**
 * KS-8.5 (#8316): agent runtime metadata backfill core — D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-agent-runtime.ts`, following the
 * KS-8.2 template (`token-ledger-backfill.ts`). Takes raw D1 rows
 * (snake_case objects, exactly as `wrangler d1 execute --json` returns
 * them) and copies them into the Postgres twins from migration
 * `0010_agent_runtime.sql`:
 *
 *   - `agent_run_events` / `agent_goal_events` — append-only event
 *     ledgers: bare `ON CONFLICT DO NOTHING` (covers the id PK plus the
 *     run/goal dedupe uniques — the exact D1 `INSERT OR IGNORE` key set).
 *     The dual-write mirror writes byte-identical rows, so backfill and
 *     mirror never fight; a re-run inserts zero rows (idempotency
 *     contract). Pages are batched into ONE multi-row INSERT.
 *
 *   - the six state tables (`agent_definitions`, `agent_definition_runs`,
 *     `agent_definition_triggers`, `agent_runs`, `agent_traces`,
 *     `agent_goals`) — CONVERGE upserts (`ON CONFLICT ... DO UPDATE SET`
 *     = the D1 snapshot value). State rows are last-writer records both
 *     sides update for the same authoritative D1 writes, so "converge to
 *     the D1 snapshot" is correct; the tiny write race against a
 *     concurrent mirror (rows updated mid-sweep) is exactly what the
 *     runbook's second catch-up sweep + `--verify` exist to close.
 *     `agent_definition_triggers` converges on the LIVE upsert arbiter
 *     (owner_agent_user_id, trigger_ref) — the live path REPLACES
 *     trigger_id under that key, so converging on the PK would strand
 *     superseded ids. `agent_goals` pages are ordered archived-first so
 *     the partial one-active-goal-per-scope unique never trips on a
 *     mid-page ordering artifact.
 *
 * PRIVACY: `agent_traces` rows are owner-private (`visibility` +
 * `owner_user_id` copied verbatim; trajectory_json is the already
 * public-safe tripwired projection). Verify output references trace_uuid
 * keys and sha256 row hashes ONLY — never trajectory content.
 *
 * Verification (`verify*`): the 2026-06-29 after-action reconciliation
 * culture with the KS-8.5 acceptance specifics — exact row counts,
 * per-run event-chain contiguity (count/min/max/distinct per run compared
 * across stores, plus gap tallies), trace row counts + content-hash
 * sampling + visibility/consent tallies, goal usage-counter sums, and
 * newest-N row-hash comparison. Nothing "close enough": exact or explain.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table shapes (column lists mirror migration 0010 exactly)
// ---------------------------------------------------------------------------

export type AgentRuntimeBackfillTable =
  | "agent_definitions"
  | "agent_definition_runs"
  | "agent_definition_triggers"
  | "agent_runs"
  | "agent_run_events"
  | "agent_traces"
  | "agent_goals"
  | "agent_goal_events"

/** Parent state tables first so event rows always trail their parents. */
export const AGENT_RUNTIME_TABLES: ReadonlyArray<AgentRuntimeBackfillTable> = [
  "agent_definitions",
  "agent_definition_runs",
  "agent_definition_triggers",
  "agent_runs",
  "agent_run_events",
  "agent_traces",
  "agent_goals",
  "agent_goal_events",
]

const DEFINITION_COLUMNS = [
  "id",
  "owner_agent_user_id",
  "owner_ref",
  "schema_literal",
  "name",
  "slug",
  "goal",
  "harness_json",
  "toolset_json",
  "triggers_json",
  "lane",
  "budget_json",
  "escalation_json",
  "source_refs_json",
  "definition_json",
  "created_at",
  "updated_at",
  "archived_at",
] as const

const DEFINITION_RUN_COLUMNS = [
  "run_id",
  "owner_agent_user_id",
  "definition_id",
  "definition_ref",
  "trigger_ref",
  "lane",
  "status",
  "pylon_ref",
  "assignment_ref",
  "durable_request_id",
  "durable_stream_url",
  "forge_tenant_ref",
  "forge_work_ref",
  "forge_repository_ref",
  "forge_git_token_refs_json",
  "refusal_error",
  "refusal_reason",
  "evidence_refs_json",
  "trigger_payload_json",
  "runtime_run_json",
  "initial_events_json",
  "budget_credits_reserved",
  "created_at",
  "updated_at",
] as const

const DEFINITION_TRIGGER_COLUMNS = [
  "trigger_id",
  "owner_agent_user_id",
  "owner_ref",
  "definition_id",
  "trigger_ref",
  "trigger_kind",
  "trigger_json",
  "state",
  "consecutive_failures",
  "next_run_at",
  "paused_at",
  "pause_reason",
  "created_at",
  "updated_at",
] as const

const AGENT_RUN_COLUMNS = [
  "id",
  "user_id",
  "team_id",
  "project_id",
  "runtime",
  "backend",
  "runner_id",
  "assignment_kind",
  "repository_provider",
  "repository_owner",
  "repository_repo",
  "repository_ref",
  "goal",
  "goal_id",
  "provider_account_ref",
  "auth_grant_ref",
  "external_run_id",
  "status",
  "event_cursor",
  "assignment_json",
  "created_at",
  "updated_at",
  "started_at",
  "completed_at",
  "failed_at",
  "canceled_at",
  "archived_at",
] as const

const AGENT_RUN_EVENT_COLUMNS = [
  "id",
  "run_id",
  "sequence",
  "type",
  "summary",
  "status",
  "source",
  "payload_json",
  "artifact_refs_json",
  "external_event_id",
  "created_at",
] as const

const AGENT_TRACE_COLUMNS = [
  "trace_uuid",
  "owner_user_id",
  "agent_ref",
  "schema_version",
  "trajectory_id",
  "session_id",
  "visibility",
  "step_count",
  "trajectory_json",
  "trajectory_r2_key",
  "blob_refs_json",
  "idempotency_key",
  "training_consent",
  "license",
  "content_digest",
  "reward_eligible",
  "reward_amount_sats",
  "upload_source",
  "demand_kind",
  "demand_source",
  "created_at",
  "updated_at",
] as const

const AGENT_GOAL_COLUMNS = [
  "id",
  "agent_id",
  "user_id",
  "team_id",
  "project_id",
  "objective",
  "status",
  "visibility",
  "current_run_id",
  "token_budget",
  "tokens_used",
  "time_used_seconds",
  "created_at",
  "updated_at",
  "completed_at",
  "paused_at",
  "blocked_at",
  "archived_at",
] as const

const AGENT_GOAL_EVENT_COLUMNS = [
  "id",
  "goal_id",
  "run_id",
  "expected_goal_id",
  "caller_type",
  "event_type",
  "status",
  "token_delta",
  "time_delta_seconds",
  "payload_json",
  "external_event_id",
  "created_at",
] as const

export const AGENT_RUNTIME_TABLE_COLUMNS: Readonly<
  Record<AgentRuntimeBackfillTable, ReadonlyArray<string>>
> = {
  agent_definition_runs: DEFINITION_RUN_COLUMNS,
  agent_definition_triggers: DEFINITION_TRIGGER_COLUMNS,
  agent_definitions: DEFINITION_COLUMNS,
  agent_goal_events: AGENT_GOAL_EVENT_COLUMNS,
  agent_goals: AGENT_GOAL_COLUMNS,
  agent_run_events: AGENT_RUN_EVENT_COLUMNS,
  agent_runs: AGENT_RUN_COLUMNS,
  agent_traces: AGENT_TRACE_COLUMNS,
}

/**
 * Conflict handling per table: the event ledgers NEVER overwrite (mirror
 * rows win / identical bytes); state tables CONVERGE to the D1 snapshot.
 * Triggers converge on the LIVE upsert arbiter (owner, trigger_ref) —
 * the same key the D1 write path replaces trigger_id under.
 */
const TABLE_CONFLICT: Readonly<
  Record<
    AgentRuntimeBackfillTable,
    Readonly<{ keyColumns: ReadonlyArray<string>; mode: "nothing" | "converge" }>
  >
> = {
  agent_definition_runs: { keyColumns: ["run_id"], mode: "converge" },
  agent_definition_triggers: {
    keyColumns: ["owner_agent_user_id", "trigger_ref"],
    mode: "converge",
  },
  agent_definitions: { keyColumns: ["id"], mode: "converge" },
  agent_goal_events: { keyColumns: [], mode: "nothing" },
  agent_goals: { keyColumns: ["id"], mode: "converge" },
  agent_run_events: { keyColumns: [], mode: "nothing" },
  agent_runs: { keyColumns: ["id"], mode: "converge" },
  agent_traces: { keyColumns: ["trace_uuid"], mode: "converge" },
}

/** Natural key used for newest-N hash comparison output. */
export const AGENT_RUNTIME_TABLE_KEY: Readonly<
  Record<AgentRuntimeBackfillTable, ReadonlyArray<string>>
> = {
  agent_definition_runs: ["run_id"],
  agent_definition_triggers: ["trigger_id"],
  agent_definitions: ["id"],
  agent_goal_events: ["id"],
  agent_goals: ["id"],
  agent_run_events: ["id"],
  agent_runs: ["id"],
  agent_traces: ["trace_uuid"],
}

/** Newest-first ordering column per table (for the hash sample). */
export const AGENT_RUNTIME_TABLE_ORDER: Readonly<
  Record<AgentRuntimeBackfillTable, string>
> = {
  agent_definition_runs: "updated_at",
  agent_definition_triggers: "updated_at",
  agent_definitions: "updated_at",
  agent_goal_events: "created_at",
  agent_goals: "updated_at",
  agent_run_events: "created_at",
  agent_runs: "updated_at",
  agent_traces: "created_at",
}

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)`; the
 * structural `SyncSql` seam deliberately does not, so this module widens
 * it locally (same note as pylon/token backfill: DIRECT connections only,
 * never Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "agent runtime backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * `agent_goals` upsert ordering: rows with archived_at set land BEFORE
 * still-active rows so the partial one-active-per-scope unique never
 * observes two active rows for one scope mid-page.
 */
const orderRowsForUpsert = (
  table: AgentRuntimeBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<D1SourceRow> =>
  table === "agent_goals"
    ? [...rows].sort((left, right) => {
        const leftArchived = left["archived_at"] === null ? 1 : 0
        const rightArchived = right["archived_at"] === null ? 1 : 0
        return leftArchived - rightArchived
      })
    : rows

/**
 * Upsert one page of D1 rows into `table`. Event ledgers: one multi-row
 * `INSERT ... ON CONFLICT DO NOTHING` per page. State tables: converge
 * upserts. Returns how many rows were actually inserted fresh (0 on an
 * events re-run — the idempotency contract; converge tables report page
 * size since DO UPDATE returns every row).
 */
export const upsertAgentRuntimeRows = async (
  sql: SyncSql,
  table: AgentRuntimeBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = AGENT_RUNTIME_TABLE_COLUMNS[table]
  const conflict = TABLE_CONFLICT[table]

  if (conflict.mode === "nothing") {
    const params: Array<unknown> = []
    const tuples = rows.map((row) => {
      const placeholders = columns.map((column) => {
        params.push(normalizeValue(row[column]))
        return `$${params.length}`
      })
      return `(${placeholders.join(", ")})`
    })
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES ${tuples.join(", ")} ON CONFLICT DO NOTHING RETURNING 1 AS inserted`,
      params,
    )
    return result.length
  }

  const setClauses = columns
    .filter((column) => !conflict.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")
  let touched = 0
  for (const row of orderRowsForUpsert(table, rows)) {
    const values = columns.map((column) => normalizeValue(row[column]))
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columns.join(", ")}) VALUES (${placeholders}) ON CONFLICT (${conflict.keyColumns.join(", ")}) DO UPDATE SET ${setClauses} RETURNING 1 AS touched`,
      values as Array<unknown>,
    )
    touched += result.length
  }
  return touched
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

/** Plain row count. */
export const postgresAgentRuntimeRowCount = async (
  sql: SyncSql,
  table: AgentRuntimeBackfillTable,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Per-run event-chain shape: for every run, the event count, min/max
 * sequence, and distinct-sequence count. Contiguity holds when
 * count === distinct === (max - min + 1). Comparing the full per-run map
 * across stores is the KS-8.5 event-chain acceptance evidence.
 */
export type EventChainRow = Readonly<{
  parentId: string
  events: number
  distinctSequences: number
  minSequence: number
  maxSequence: number
}>

export type EventChainTally = Readonly<{
  chains: ReadonlyArray<EventChainRow>
  totalEvents: number
  gappedChains: number
}>

const chainConfig: Readonly<
  Record<
    "agent_run_events" | "agent_goal_events",
    Readonly<{ parentColumn: string; sequenceExpr: string }>
  >
> = {
  // goal events have no integer sequence — chain identity is the ordered
  // (created_at, id) pair, so we tally count + distinct ids per goal.
  agent_goal_events: { parentColumn: "goal_id", sequenceExpr: "NULL" },
  agent_run_events: { parentColumn: "run_id", sequenceExpr: "sequence" },
}

export const eventChainTallyFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): EventChainTally => {
  const chains = rows.map((row) => ({
    distinctSequences: Number(row["distinct_sequences"] ?? 0),
    events: Number(row["events"] ?? 0),
    maxSequence: Number(row["max_sequence"] ?? 0),
    minSequence: Number(row["min_sequence"] ?? 0),
    parentId: String(row["parent_id"] ?? ""),
  }))
  const gappedChains = chains.filter(
    (chain) =>
      chain.events !== chain.distinctSequences ||
      (chain.maxSequence > 0 &&
        chain.maxSequence - chain.minSequence + 1 !== chain.distinctSequences),
  ).length
  return {
    chains,
    gappedChains,
    totalEvents: chains.reduce((sum, chain) => sum + chain.events, 0),
  }
}

/** The SQL producing per-parent chain rows (same text runs on D1 + PG). */
export const eventChainSql = (
  table: "agent_run_events" | "agent_goal_events",
): string => {
  const config = chainConfig[table]
  const sequence =
    config.sequenceExpr === "NULL"
      ? `COUNT(DISTINCT id) AS distinct_sequences,
         0 AS min_sequence,
         0 AS max_sequence`
      : `COUNT(DISTINCT ${config.sequenceExpr}) AS distinct_sequences,
         MIN(${config.sequenceExpr}) AS min_sequence,
         MAX(${config.sequenceExpr}) AS max_sequence`
  return `SELECT ${config.parentColumn} AS parent_id,
       COUNT(*) AS events,
       ${sequence}
  FROM ${table}
 GROUP BY ${config.parentColumn}
 ORDER BY ${config.parentColumn}`
}

export const postgresEventChainTally = async (
  sql: SyncSql,
  table: "agent_run_events" | "agent_goal_events",
): Promise<EventChainTally> => {
  const unsafe = requireUnsafe(sql)
  return eventChainTallyFromRows(await unsafe(eventChainSql(table), []))
}

export type EventChainMismatch = Readonly<{
  parentId: string
  d1: EventChainRow | undefined
  postgres: EventChainRow | undefined
}>

export const compareEventChains = (
  d1: EventChainTally,
  postgres: EventChainTally,
): ReadonlyArray<EventChainMismatch> => {
  const postgresByParent = new Map(
    postgres.chains.map((chain) => [chain.parentId, chain]),
  )
  const mismatches: Array<EventChainMismatch> = []
  const seen = new Set<string>()
  for (const chain of d1.chains) {
    seen.add(chain.parentId)
    const twin = postgresByParent.get(chain.parentId)
    if (
      twin === undefined ||
      twin.events !== chain.events ||
      twin.distinctSequences !== chain.distinctSequences ||
      twin.minSequence !== chain.minSequence ||
      twin.maxSequence !== chain.maxSequence
    ) {
      mismatches.push({ d1: chain, parentId: chain.parentId, postgres: twin })
    }
  }
  for (const chain of postgres.chains) {
    if (!seen.has(chain.parentId)) {
      mismatches.push({ d1: undefined, parentId: chain.parentId, postgres: chain })
    }
  }
  return mismatches
}

/**
 * Domain scalar tallies per table (compared exactly across stores). The
 * SQL text is portable and runs verbatim on D1 AND Postgres so both sides
 * compute the same numbers over the same rows. Trace tallies never touch
 * trajectory content — counts, visibility mix, and consent sums only.
 */
export const AGENT_RUNTIME_SCALAR_TALLIES: Readonly<
  Record<AgentRuntimeBackfillTable, ReadonlyArray<Readonly<{ metric: string; sql: string }>>>
> = {
  agent_definition_runs: [
    {
      metric: "sum_budget_credits_reserved_millis",
      sql: `SELECT COALESCE(SUM(CAST(budget_credits_reserved * 1000 AS bigint)), 0) AS value FROM agent_definition_runs`,
    },
  ],
  agent_definition_triggers: [
    {
      metric: "enabled_triggers",
      sql: `SELECT COUNT(*) AS value FROM agent_definition_triggers WHERE state = 'enabled'`,
    },
    {
      metric: "sum_consecutive_failures",
      sql: `SELECT COALESCE(SUM(consecutive_failures), 0) AS value FROM agent_definition_triggers`,
    },
  ],
  agent_definitions: [
    {
      metric: "active_definitions",
      sql: `SELECT COUNT(*) AS value FROM agent_definitions WHERE archived_at IS NULL`,
    },
  ],
  agent_goal_events: [
    {
      metric: "sum_token_delta",
      sql: `SELECT COALESCE(SUM(token_delta), 0) AS value FROM agent_goal_events`,
    },
  ],
  agent_goals: [
    {
      metric: "sum_tokens_used",
      sql: `SELECT COALESCE(SUM(tokens_used), 0) AS value FROM agent_goals`,
    },
    {
      metric: "active_goals",
      sql: `SELECT COUNT(*) AS value FROM agent_goals WHERE archived_at IS NULL`,
    },
  ],
  agent_run_events: [
    {
      metric: "sum_sequence",
      sql: `SELECT COALESCE(SUM(sequence), 0) AS value FROM agent_run_events`,
    },
  ],
  agent_runs: [
    {
      metric: "sum_event_cursor",
      sql: `SELECT COALESCE(SUM(event_cursor), 0) AS value FROM agent_runs`,
    },
    {
      metric: "completed_runs",
      sql: `SELECT COUNT(*) AS value FROM agent_runs WHERE status = 'completed'`,
    },
  ],
  agent_traces: [
    {
      metric: "public_traces",
      sql: `SELECT COUNT(*) AS value FROM agent_traces WHERE visibility = 'public'`,
    },
    {
      metric: "owner_only_traces",
      sql: `SELECT COUNT(*) AS value FROM agent_traces WHERE visibility = 'owner_only'`,
    },
    {
      metric: "training_consented_traces",
      sql: `SELECT COALESCE(SUM(training_consent), 0) AS value FROM agent_traces`,
    },
    {
      metric: "sum_step_count",
      sql: `SELECT COALESCE(SUM(step_count), 0) AS value FROM agent_traces`,
    },
    {
      metric: "distinct_content_digests",
      sql: `SELECT COUNT(DISTINCT content_digest) AS value FROM agent_traces WHERE content_digest IS NOT NULL`,
    },
  ],
}

export const postgresAgentRuntimeScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertAgentRuntimeRows`,
 * so the SAME D1 export row and its Postgres twin hash identically. For
 * agent_traces this IS the content-hash sample the KS-8.5 acceptance asks
 * for (trajectory_json bytes participate; the hash output itself is
 * public-safe).
 */
export const agentRuntimeRowHash = (
  table: AgentRuntimeBackfillTable,
  row: D1SourceRow,
): string => {
  const columns = AGENT_RUNTIME_TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (
  table: AgentRuntimeBackfillTable,
  row: D1SourceRow,
): string =>
  AGENT_RUNTIME_TABLE_KEY[table]
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const postgresAgentRuntimeNewestHashes = async (
  sql: SyncSql,
  table: AgentRuntimeBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const orderColumn = AGENT_RUNTIME_TABLE_ORDER[table]
  const keyColumns = AGENT_RUNTIME_TABLE_KEY[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: agentRuntimeRowHash(table, row),
    key: rowKey(table, row),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1AgentRuntimeNewestHashes = (
  table: AgentRuntimeBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: agentRuntimeRowHash(table, row),
    key: rowKey(table, row),
  }))

export type AgentRuntimeVerifyReport = Readonly<{
  table: AgentRuntimeBackfillTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
  }>
  chainMismatches: ReadonlyArray<EventChainMismatch>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): AgentRuntimeVerifyReport["newestHashMismatches"] => {
  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const mismatches: Array<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }> = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      mismatches.push({ d1Hash: entry.hash, key: entry.key, postgresHash })
    }
  }
  return mismatches
}

export const buildAgentRuntimeVerifyReport = (
  input: Readonly<{
    table: AgentRuntimeBackfillTable
    d1Total: number
    postgresTotal: number
    scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
    d1Chains?: EventChainTally | undefined
    postgresChains?: EventChainTally | undefined
    d1Newest: ReadonlyArray<NewestRowHash>
    postgresNewest: ReadonlyArray<NewestRowHash>
  }>,
): AgentRuntimeVerifyReport => ({
  chainMismatches:
    input.d1Chains === undefined || input.postgresChains === undefined
      ? []
      : compareEventChains(input.d1Chains, input.postgresChains),
  countsMatch: input.d1Total === input.postgresTotal,
  d1Total: input.d1Total,
  newestHashMismatches: compareNewestHashes(input.d1Newest, input.postgresNewest),
  postgresTotal: input.postgresTotal,
  scalarMismatches: input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  ),
  table: input.table,
})

export const agentRuntimeVerifyReportClean = (
  report: AgentRuntimeVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.chainMismatches.length === 0 &&
  report.newestHashMismatches.length === 0
