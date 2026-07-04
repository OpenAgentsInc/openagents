/**
 * KS-8.5 follow-up (#8334): agent runtime remainder backfill core.
 *
 * Copies the D1 remainder tables into Postgres migration
 * `0012_agent_runtime_remainder.sql`, preserving D1 byte shapes for
 * reconciliation. The verifier is deliberately key/hash based: it never
 * prints credential token hashes, job payloads, proposal bodies, or event
 * payload summaries.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

export type AgentRuntimeRemainderTable =
  | "agent_profiles"
  | "agent_credentials"
  | "agent_owner_claims"
  | "agent_owner_x_claim_challenges"
  | "agent_proposals"
  | "event_ledger_entries"
  | "khala_acceptance_jobs"
  | "khala_acceptance_verdicts"

export const AGENT_RUNTIME_REMAINDER_TABLES: ReadonlyArray<AgentRuntimeRemainderTable> = [
  "agent_profiles",
  "agent_credentials",
  "agent_owner_claims",
  "agent_owner_x_claim_challenges",
  "agent_proposals",
  "event_ledger_entries",
  "khala_acceptance_jobs",
  "khala_acceptance_verdicts",
]

const PROFILE_COLUMNS = [
  "user_id",
  "slug",
  "metadata_json",
  "created_at",
  "updated_at",
] as const

const CREDENTIAL_COLUMNS = [
  "id",
  "user_id",
  "openauth_user_id",
  "token_hash",
  "token_prefix",
  "name",
  "status",
  "created_at",
  "last_used_at",
  "revoked_at",
  "expires_at",
] as const

const OWNER_CLAIM_COLUMNS = [
  "id",
  "claim_token_hash",
  "claim_token_prefix",
  "status",
  "display_name",
  "slug",
  "external_id",
  "primary_email",
  "metadata_json",
  "owner_user_id",
  "agent_user_id",
  "credential_id",
  "token_prefix",
  "receipt_ref",
  "requested_at",
  "expires_at",
  "decided_at",
  "token_issued_at",
  "rejected_reason",
  "created_at",
  "updated_at",
] as const

const X_CLAIM_COLUMNS = [
  "id",
  "agent_claim_id",
  "owner_user_id",
  "agent_user_id",
  "x_account_ref",
  "x_handle",
  "nonce",
  "required_text",
  "required_url",
  "state",
  "receipt_ref",
  "tweet_ref",
  "tweet_url",
  "policy_refs_json",
  "caveat_refs_json",
  "rejected_reason",
  "created_at",
  "expires_at",
  "verified_at",
  "updated_at",
] as const

const PROPOSAL_COLUMNS = [
  "id",
  "receipt_ref",
  "status",
  "kind",
  "title",
  "summary",
  "body_text",
  "source_urls_json",
  "target_json",
  "author_json",
  "client_fingerprint_hash",
  "idempotency_key_hash",
  "promotion_kind",
  "promoted_target_ref",
  "operator_note",
  "operator_user_id",
  "decided_at",
  "created_at",
  "updated_at",
] as const

const EVENT_LEDGER_COLUMNS = [
  "entry_id",
  "owner_agent_user_id",
  "owner_ref",
  "source",
  "external_ref",
  "actor_ref",
  "content_ref",
  "subject_ref",
  "event_type",
  "source_refs_json",
  "payload_summary_json",
  "occurred_at",
  "received_at",
  "ordering_key",
  "ordering_sequence",
  "handled_state",
  "handled_by_run_id",
  "handled_by_definition_id",
  "handled_at",
  "handled_reason_ref",
  "training_consent",
  "created_at",
  "updated_at",
] as const

const ACCEPTANCE_JOB_COLUMNS = [
  "request_id",
  "status",
  "job_payload",
  "lease_id",
  "lease_expires_at",
  "attempts",
  "created_at",
  "updated_at",
] as const

const ACCEPTANCE_VERDICT_COLUMNS = [
  "request_id",
  "verification",
  "verified",
  "executed",
  "scalar_reward",
  "rubric_ref",
  "passed_checks",
  "failed_checks",
  "verification_receipt_ref",
  "version",
  "updated_at",
] as const

export const AGENT_RUNTIME_REMAINDER_TABLE_COLUMNS: Readonly<
  Record<AgentRuntimeRemainderTable, ReadonlyArray<string>>
> = {
  agent_credentials: CREDENTIAL_COLUMNS,
  agent_owner_claims: OWNER_CLAIM_COLUMNS,
  agent_owner_x_claim_challenges: X_CLAIM_COLUMNS,
  agent_profiles: PROFILE_COLUMNS,
  agent_proposals: PROPOSAL_COLUMNS,
  event_ledger_entries: EVENT_LEDGER_COLUMNS,
  khala_acceptance_jobs: ACCEPTANCE_JOB_COLUMNS,
  khala_acceptance_verdicts: ACCEPTANCE_VERDICT_COLUMNS,
}

const TABLE_CONFLICT: Readonly<
  Record<AgentRuntimeRemainderTable, Readonly<{ keyColumns: ReadonlyArray<string> }>>
> = {
  agent_credentials: { keyColumns: ["id"] },
  agent_owner_claims: { keyColumns: ["id"] },
  agent_owner_x_claim_challenges: { keyColumns: ["id"] },
  agent_profiles: { keyColumns: ["user_id"] },
  agent_proposals: { keyColumns: ["id"] },
  event_ledger_entries: { keyColumns: ["entry_id"] },
  khala_acceptance_jobs: { keyColumns: ["request_id"] },
  khala_acceptance_verdicts: { keyColumns: ["request_id"] },
}

export const AGENT_RUNTIME_REMAINDER_TABLE_KEY: Readonly<
  Record<AgentRuntimeRemainderTable, ReadonlyArray<string>>
> = {
  agent_credentials: ["id"],
  agent_owner_claims: ["id"],
  agent_owner_x_claim_challenges: ["id"],
  agent_profiles: ["user_id"],
  agent_proposals: ["id"],
  event_ledger_entries: ["entry_id"],
  khala_acceptance_jobs: ["request_id"],
  khala_acceptance_verdicts: ["request_id"],
}

export const AGENT_RUNTIME_REMAINDER_TABLE_ORDER: Readonly<
  Record<AgentRuntimeRemainderTable, string>
> = {
  agent_credentials: "created_at",
  agent_owner_claims: "updated_at",
  agent_owner_x_claim_challenges: "updated_at",
  agent_profiles: "updated_at",
  agent_proposals: "updated_at",
  event_ledger_entries: "ordering_sequence",
  khala_acceptance_jobs: "updated_at",
  khala_acceptance_verdicts: "updated_at",
}

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "agent runtime remainder backfill requires a driver exposing unsafe(text, params)",
    )
  }
  return unsafe
}

export const upsertAgentRuntimeRemainderRows = async (
  sql: SyncSql,
  table: AgentRuntimeRemainderTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const columns = AGENT_RUNTIME_REMAINDER_TABLE_COLUMNS[table]
  const conflict = TABLE_CONFLICT[table]
  const setClauses = columns
    .filter((column) => !conflict.keyColumns.includes(column))
    .map((column) => `${column} = EXCLUDED.${column}`)
    .join(", ")

  let touched = 0
  for (const row of rows) {
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

const SEPARATOR = String.fromCharCode(31)

export const agentRuntimeRemainderRowHash = (
  table: AgentRuntimeRemainderTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of AGENT_RUNTIME_REMAINDER_TABLE_COLUMNS[table]) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? "<null>" : String(value))
    hash.update(SEPARATOR)
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (
  table: AgentRuntimeRemainderTable,
  row: D1SourceRow,
): string =>
  AGENT_RUNTIME_REMAINDER_TABLE_KEY[table]
    .map((column) => String(row[column] ?? "<null>"))
    .join(":")

export const d1AgentRuntimeRemainderNewestHashes = (
  table: AgentRuntimeRemainderTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: agentRuntimeRemainderRowHash(table, row),
    key: rowKey(table, row),
  }))

export const postgresAgentRuntimeRemainderNewestHashes = async (
  sql: SyncSql,
  table: AgentRuntimeRemainderTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireUnsafe(sql)
  const orderColumn = AGENT_RUNTIME_REMAINDER_TABLE_ORDER[table]
  const keyColumns = AGENT_RUNTIME_REMAINDER_TABLE_KEY[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${keyColumns.join(" DESC, ")} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: agentRuntimeRemainderRowHash(table, row),
    key: rowKey(table, row),
  }))
}

export const postgresAgentRuntimeRemainderRowCount = async (
  sql: SyncSql,
  table: AgentRuntimeRemainderTable,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

export const AGENT_RUNTIME_REMAINDER_SCALAR_TALLIES: Readonly<
  Record<AgentRuntimeRemainderTable, ReadonlyArray<Readonly<{ metric: string; sql: string }>>>
> = {
  agent_credentials: [
    {
      metric: "active_credentials",
      sql: "SELECT COUNT(*) AS value FROM agent_credentials WHERE status = 'active'",
    },
    {
      metric: "revoked_credentials",
      sql: "SELECT COUNT(*) AS value FROM agent_credentials WHERE status = 'revoked'",
    },
    {
      metric: "credentials_with_openauth_user",
      sql: "SELECT COUNT(*) AS value FROM agent_credentials WHERE openauth_user_id IS NOT NULL",
    },
  ],
  agent_owner_claims: [
    {
      metric: "approved_claims",
      sql: "SELECT COUNT(*) AS value FROM agent_owner_claims WHERE status = 'approved'",
    },
    {
      metric: "pending_claims",
      sql: "SELECT COUNT(*) AS value FROM agent_owner_claims WHERE status = 'pending'",
    },
  ],
  agent_owner_x_claim_challenges: [
    {
      metric: "verified_or_approved_x_claims",
      sql: "SELECT COUNT(*) AS value FROM agent_owner_x_claim_challenges WHERE state IN ('verified', 'approved')",
    },
  ],
  agent_profiles: [
    {
      metric: "profiles_with_slug",
      sql: "SELECT COUNT(*) AS value FROM agent_profiles WHERE slug IS NOT NULL",
    },
  ],
  agent_proposals: [
    {
      metric: "pending_proposals",
      sql: "SELECT COUNT(*) AS value FROM agent_proposals WHERE status = 'pending'",
    },
    {
      metric: "promoted_proposals",
      sql: "SELECT COUNT(*) AS value FROM agent_proposals WHERE status = 'promoted'",
    },
  ],
  event_ledger_entries: [
    {
      metric: "open_event_ledger_entries",
      sql: "SELECT COUNT(*) AS value FROM event_ledger_entries WHERE handled_state = 'open'",
    },
    {
      metric: "sum_training_consent",
      sql: "SELECT COALESCE(SUM(training_consent), 0) AS value FROM event_ledger_entries",
    },
    {
      metric: "sum_ordering_sequence",
      sql: "SELECT COALESCE(SUM(ordering_sequence), 0) AS value FROM event_ledger_entries",
    },
  ],
  khala_acceptance_jobs: [
    {
      metric: "pending_acceptance_jobs",
      sql: "SELECT COUNT(*) AS value FROM khala_acceptance_jobs WHERE status = 'pending'",
    },
    {
      metric: "leased_acceptance_jobs",
      sql: "SELECT COUNT(*) AS value FROM khala_acceptance_jobs WHERE status = 'leased'",
    },
    {
      metric: "sum_acceptance_job_attempts",
      sql: "SELECT COALESCE(SUM(attempts), 0) AS value FROM khala_acceptance_jobs",
    },
  ],
  khala_acceptance_verdicts: [
    {
      metric: "verified_acceptance_verdicts",
      sql: "SELECT COALESCE(SUM(verified), 0) AS value FROM khala_acceptance_verdicts",
    },
    {
      metric: "executed_acceptance_verdicts",
      sql: "SELECT COALESCE(SUM(executed), 0) AS value FROM khala_acceptance_verdicts",
    },
    {
      metric: "sum_scalar_reward_micros",
      sql: "SELECT COALESCE(SUM(CAST(scalar_reward * 1000000 AS bigint)), 0) AS value FROM khala_acceptance_verdicts",
    },
  ],
}

export const postgresAgentRuntimeRemainderScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

export type OrderingDensityRow = Readonly<{
  ownerAgentUserId: string
  entries: number
  distinctSequences: number
  minSequence: number
  maxSequence: number
}>

export type OrderingDensityTally = Readonly<{
  owners: ReadonlyArray<OrderingDensityRow>
  totalEntries: number
  gappedOwners: number
}>

export const eventLedgerOrderingDensitySql = `SELECT owner_agent_user_id,
       COUNT(*) AS entries,
       COUNT(DISTINCT ordering_sequence) AS distinct_sequences,
       MIN(ordering_sequence) AS min_sequence,
       MAX(ordering_sequence) AS max_sequence
  FROM event_ledger_entries
 GROUP BY owner_agent_user_id
 ORDER BY owner_agent_user_id`

export const orderingDensityFromRows = (
  rows: ReadonlyArray<Record<string, unknown>>,
): OrderingDensityTally => {
  const owners = rows.map((row) => ({
    distinctSequences: Number(row["distinct_sequences"] ?? 0),
    entries: Number(row["entries"] ?? 0),
    maxSequence: Number(row["max_sequence"] ?? 0),
    minSequence: Number(row["min_sequence"] ?? 0),
    ownerAgentUserId: String(row["owner_agent_user_id"] ?? ""),
  }))
  return {
    gappedOwners: owners.filter(
      (owner) =>
        owner.entries !== owner.distinctSequences ||
        (owner.maxSequence > 0 &&
          owner.maxSequence - owner.minSequence + 1 !== owner.distinctSequences),
    ).length,
    owners,
    totalEntries: owners.reduce((sum, owner) => sum + owner.entries, 0),
  }
}

export const postgresEventLedgerOrderingDensity = async (
  sql: SyncSql,
): Promise<OrderingDensityTally> => {
  const unsafe = requireUnsafe(sql)
  return orderingDensityFromRows(await unsafe(eventLedgerOrderingDensitySql, []))
}

export type OrderingDensityMismatch = Readonly<{
  ownerAgentUserId: string
  d1: OrderingDensityRow | undefined
  postgres: OrderingDensityRow | undefined
}>

export const compareOrderingDensity = (
  d1: OrderingDensityTally,
  postgres: OrderingDensityTally,
): ReadonlyArray<OrderingDensityMismatch> => {
  const postgresByOwner = new Map(
    postgres.owners.map((owner) => [owner.ownerAgentUserId, owner]),
  )
  const mismatches: Array<OrderingDensityMismatch> = []
  const seen = new Set<string>()
  for (const owner of d1.owners) {
    seen.add(owner.ownerAgentUserId)
    const twin = postgresByOwner.get(owner.ownerAgentUserId)
    if (
      twin === undefined ||
      twin.entries !== owner.entries ||
      twin.distinctSequences !== owner.distinctSequences ||
      twin.minSequence !== owner.minSequence ||
      twin.maxSequence !== owner.maxSequence
    ) {
      mismatches.push({
        d1: owner,
        ownerAgentUserId: owner.ownerAgentUserId,
        postgres: twin,
      })
    }
  }
  for (const owner of postgres.owners) {
    if (!seen.has(owner.ownerAgentUserId)) {
      mismatches.push({
        d1: undefined,
        ownerAgentUserId: owner.ownerAgentUserId,
        postgres: owner,
      })
    }
  }
  return mismatches
}

export type AgentRuntimeRemainderVerifyReport = Readonly<{
  table: AgentRuntimeRemainderTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
  }>
  orderingDensityMismatches: ReadonlyArray<OrderingDensityMismatch>
  d1OrderingGappedOwners: number
  postgresOrderingGappedOwners: number
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): AgentRuntimeRemainderVerifyReport["newestHashMismatches"] => {
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

export const buildAgentRuntimeRemainderVerifyReport = (
  input: Readonly<{
    table: AgentRuntimeRemainderTable
    d1Total: number
    postgresTotal: number
    scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
    d1OrderingDensity?: OrderingDensityTally | undefined
    postgresOrderingDensity?: OrderingDensityTally | undefined
    d1Newest: ReadonlyArray<NewestRowHash>
    postgresNewest: ReadonlyArray<NewestRowHash>
  }>,
): AgentRuntimeRemainderVerifyReport => ({
  countsMatch: input.d1Total === input.postgresTotal,
  d1OrderingGappedOwners: input.d1OrderingDensity?.gappedOwners ?? 0,
  d1Total: input.d1Total,
  newestHashMismatches: compareNewestHashes(input.d1Newest, input.postgresNewest),
  orderingDensityMismatches:
    input.d1OrderingDensity === undefined ||
    input.postgresOrderingDensity === undefined
      ? []
      : compareOrderingDensity(input.d1OrderingDensity, input.postgresOrderingDensity),
  postgresOrderingGappedOwners: input.postgresOrderingDensity?.gappedOwners ?? 0,
  postgresTotal: input.postgresTotal,
  scalarMismatches: input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  ),
  table: input.table,
})

export const agentRuntimeRemainderVerifyReportClean = (
  report: AgentRuntimeRemainderVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.orderingDensityMismatches.length === 0 &&
  report.d1OrderingGappedOwners === 0 &&
  report.postgresOrderingGappedOwners === 0 &&
  report.newestHashMismatches.length === 0
