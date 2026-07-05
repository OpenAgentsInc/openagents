/**
 * KS-8.12 (#8323): sites content backfill + verification core —
 * D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-sites-content.ts`, following the
 * KS-8.10 template (`forum-content-backfill.ts`). Takes raw D1 rows
 * (snake_case objects, exactly as `wrangler d1 execute --json` returns
 * them) and converges them into the Postgres twins from migration
 * `0020_sites_core.sql` via the SHARED registry in
 * `./sites-content-tables.ts` (the same `upsertSitesContentRows` the
 * Worker's dual-write mirror uses — backfill and mirror can never fight
 * because they write identical converge upserts keyed on the PK).
 *
 * Verification (`verify*`): the 2026-06-29 after-action reconciliation
 * culture with the KS-8.12 acceptance specifics —
 *
 *   - exact row counts per table;
 *   - domain scalar tallies (project status tallies, deployment state
 *     tallies, builder sequence sums, snapshot byte totals);
 *   - PER-PROJECT VERSION CHAINS (count / distinct ids / min / max
 *     created_at over `site_versions` per site — the KS-8.12
 *     "per-project version-chain contiguity" acceptance is
 *     store-vs-store chain-map equality);
 *   - DEPLOYMENT STATE-MACHINE EQUALITY (per-site per-status deployment
 *     counts — the exact state-machine census on both stores);
 *   - BUILDER SEQUENCE CHAINS (count / distinct / min / max `sequence`
 *     per session over messages / events / phase runs — a builder
 *     transcript IS this chain);
 *   - newest-N full row hashes per table.
 *
 * Output references row KEYS and sha256 hashes only — never prompts,
 * message bodies, or snapshot preview text.
 */

import { createHash } from "node:crypto"
import {
  normalizeSitesContentValue,
  requireSitesContentUnsafe,
  SITES_CONTENT_TABLE_COLUMNS,
  SITES_CONTENT_TABLE_PK,
  SITES_CONTENT_TABLES,
  SITES_REMAINDER_TABLES,
  ALL_SITES_CONTENT_TABLES,
  upsertSitesContentRows,
  type SitesContentRow,
  type SitesContentTable,
} from "./sites-content-tables.js"
import type { SyncSql } from "./sql.js"

export {
  SITES_CONTENT_TABLE_COLUMNS,
  SITES_CONTENT_TABLE_PK,
  SITES_CONTENT_TABLES,
  SITES_REMAINDER_TABLES,
  ALL_SITES_CONTENT_TABLES,
  upsertSitesContentRows,
  type SitesContentRow,
  type SitesContentTable,
}

export type D1SourceRow = SitesContentRow

/** Newest-first ordering column per table (for the hash sample). */
export const SITES_CONTENT_TABLE_ORDER: Readonly<
  Record<SitesContentTable, string>
> = {
  site_access_grants: "created_at",
  site_builder_artifacts: "created_at",
  site_builder_events: "created_at",
  site_builder_file_snapshots: "updated_at",
  site_builder_messages: "created_at",
  site_builder_phase_runs: "created_at",
  site_builder_previews: "updated_at",
  site_builder_repair_attempts: "created_at",
  site_builder_saved_versions: "created_at",
  site_builder_sessions: "updated_at",
  site_deployment_attempts: "updated_at",
  site_deployments: "updated_at",
  site_events: "created_at",
  site_projects: "updated_at",
  site_versions: "created_at",
  site_build_validations: "created_at",
  site_revision_feedback: "updated_at",
  site_compatibility_checks: "created_at",
  site_provisioning_plans: "updated_at",
  site_storage_bindings: "updated_at",
  site_source_exports: "updated_at",
  site_referral_sources: "updated_at",
  referral_invites: "updated_at",
  site_referral_policy_events: "created_at",
  site_environment_values: "updated_at",
  site_commerce_products: "updated_at",
  site_commerce_paid_actions: "updated_at",
  site_commerce_payment_events: "created_at",
  site_commerce_revenue_share_links: "created_at",
  site_commerce_review_decisions: "updated_at",
  site_mdk_checkout_intents: "updated_at",
  site_mdk_account_bindings: "updated_at",
  site_payment_catalog_items: "updated_at",
  site_referral_payout_ledger_entries: "created_at",
  targeted_site_campaigns: "updated_at",
  targeted_site_prospects: "updated_at",
  targeted_site_capture_policy_events: "created_at",
  targeted_site_static_capture_runs: "created_at",
  targeted_site_rendered_capture_runs: "created_at",
  targeted_site_capture_provider_adapter_runs: "created_at",
  targeted_site_quality_audits: "created_at",
  targeted_site_remake_briefs: "created_at",
  targeted_site_remake_preview_generations: "created_at",
  targeted_site_operator_review_events: "created_at",
  targeted_site_remake_outreach_email_dispatches: "created_at",
  targeted_site_agent_toolkit_grants: "updated_at",
  targeted_site_agent_toolkit_actions: "created_at",
  targeted_site_sales_reward_policy_events: "created_at",
  tenant_custom_hostnames: "updated_at",
  deployments: "updated_at",
  deployment_events: "created_at",

}

// ---------------------------------------------------------------------------
// Row hashes
// ---------------------------------------------------------------------------

/**
 * Canonical row hash: the registry-order column values joined with unit
 * separators, sha256'd. Normalization matches `upsertSitesContentRows`,
 * so the SAME D1 export row and its Postgres twin hash identically
 * (bigint sequences come back as strings from postgres.js; `String()`
 * canonicalizes both sides).
 */
export const sitesContentRowHash = (
  table: SitesContentTable,
  row: D1SourceRow,
): string => {
  const columns = SITES_CONTENT_TABLE_COLUMNS[table]
  const hash = createHash("sha256")
  for (const column of columns) {
    const value = normalizeSitesContentValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

const rowKey = (table: SitesContentTable, row: D1SourceRow): string =>
  String(row[SITES_CONTENT_TABLE_PK[table]] ?? "<null>")

export const d1SitesContentNewestHashes = (
  table: SitesContentTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> =>
  rows.map((row) => ({
    hash: sitesContentRowHash(table, row),
    key: rowKey(table, row),
  }))

export const postgresSitesContentNewestHashes = async (
  sql: SyncSql,
  table: SitesContentTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const unsafe = requireSitesContentUnsafe(sql)
  const orderColumn = SITES_CONTENT_TABLE_ORDER[table]
  const pk = SITES_CONTENT_TABLE_PK[table]
  const rows = await unsafe(
    `SELECT * FROM ${table} ORDER BY ${orderColumn} DESC, ${pk} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: sitesContentRowHash(table, row),
    key: rowKey(table, row),
  }))
}

// ---------------------------------------------------------------------------
// Counts and scalar tallies
// ---------------------------------------------------------------------------

export const postgresSitesContentRowCount = async (
  sql: SyncSql,
  table: SitesContentTable,
): Promise<number> => {
  const unsafe = requireSitesContentUnsafe(sql)
  const rows = await unsafe(`SELECT COUNT(*) AS total_rows FROM ${table}`, [])
  return Number(rows[0]?.["total_rows"] ?? 0)
}

/**
 * Domain scalar tallies per table (compared exactly across stores). The
 * SQL text is portable and runs verbatim on D1 AND Postgres so both
 * sides compute the same numbers over the same rows. Body/preview
 * tallies measure byte LENGTHS only — never content.
 */
export const SITES_CONTENT_SCALAR_TALLIES: Readonly<
  Record<
    SitesContentTable,
    ReadonlyArray<Readonly<{ metric: string; sql: string }>>
  >
> = {
  site_access_grants: [
    {
      metric: "active_grants",
      sql: `SELECT COUNT(*) AS value FROM site_access_grants WHERE revoked_at IS NULL`,
    },
  ],
  site_builder_artifacts: [
    {
      metric: "sum_byte_size",
      sql: `SELECT COALESCE(SUM(byte_size), 0) AS value FROM site_builder_artifacts`,
    },
  ],
  site_builder_events: [
    {
      metric: "sum_sequence",
      sql: `SELECT COALESCE(SUM(sequence), 0) AS value FROM site_builder_events`,
    },
    {
      metric: "phase_scoped_events",
      sql: `SELECT COUNT(*) AS value FROM site_builder_events WHERE phase_kind IS NOT NULL`,
    },
  ],
  site_builder_file_snapshots: [
    {
      metric: "sum_byte_size",
      sql: `SELECT COALESCE(SUM(byte_size), 0) AS value FROM site_builder_file_snapshots`,
    },
    {
      metric: "sum_preview_length",
      sql: `SELECT COALESCE(SUM(LENGTH(COALESCE(preview_text, ''))), 0) AS value FROM site_builder_file_snapshots`,
    },
    {
      metric: "distinct_paths",
      sql: `SELECT COUNT(DISTINCT path) AS value FROM site_builder_file_snapshots`,
    },
  ],
  site_builder_messages: [
    {
      metric: "sum_sequence",
      sql: `SELECT COALESCE(SUM(sequence), 0) AS value FROM site_builder_messages`,
    },
    {
      metric: "sum_body_length",
      sql: `SELECT COALESCE(SUM(LENGTH(body)), 0) AS value FROM site_builder_messages`,
    },
  ],
  site_builder_phase_runs: [
    {
      metric: "sum_sequence",
      sql: `SELECT COALESCE(SUM(sequence), 0) AS value FROM site_builder_phase_runs`,
    },
    {
      metric: "completed_runs",
      sql: `SELECT COUNT(*) AS value FROM site_builder_phase_runs WHERE completed_at IS NOT NULL`,
    },
  ],
  site_builder_previews: [
    {
      metric: "active_previews",
      sql: `SELECT COUNT(*) AS value FROM site_builder_previews WHERE archived_at IS NULL`,
    },
    {
      metric: "previews_with_url",
      sql: `SELECT COUNT(*) AS value FROM site_builder_previews WHERE preview_url IS NOT NULL`,
    },
  ],
  site_builder_repair_attempts: [
    {
      metric: "sum_attempt_number",
      sql: `SELECT COALESCE(SUM(attempt_number), 0) AS value FROM site_builder_repair_attempts`,
    },
    {
      metric: "completed_attempts",
      sql: `SELECT COUNT(*) AS value FROM site_builder_repair_attempts WHERE completed_at IS NOT NULL`,
    },
  ],
  site_builder_saved_versions: [
    {
      metric: "distinct_sites",
      sql: `SELECT COUNT(DISTINCT site_id) AS value FROM site_builder_saved_versions`,
    },
  ],
  site_builder_sessions: [
    {
      metric: "active_sessions",
      sql: `SELECT COUNT(*) AS value FROM site_builder_sessions WHERE archived_at IS NULL`,
    },
    {
      metric: "distinct_owners",
      sql: `SELECT COUNT(DISTINCT owner_user_id) AS value FROM site_builder_sessions`,
    },
  ],
  site_deployment_attempts: [
    {
      metric: "attempts_with_deployment",
      sql: `SELECT COUNT(*) AS value FROM site_deployment_attempts WHERE deployment_id IS NOT NULL`,
    },
  ],
  site_deployments: [
    {
      metric: "active_deployments",
      sql: `SELECT COUNT(*) AS value FROM site_deployments WHERE status = 'active'`,
    },
    {
      metric: "rolled_back_deployments",
      sql: `SELECT COUNT(*) AS value FROM site_deployments WHERE status = 'rolled_back'`,
    },
    {
      metric: "disabled_deployments",
      sql: `SELECT COUNT(*) AS value FROM site_deployments WHERE status = 'disabled'`,
    },
  ],
  site_events: [
    {
      metric: "distinct_event_types",
      sql: `SELECT COUNT(DISTINCT type) AS value FROM site_events`,
    },
    {
      metric: "sum_payload_length",
      sql: `SELECT COALESCE(SUM(LENGTH(COALESCE(payload_json, ''))), 0) AS value FROM site_events`,
    },
  ],
  site_projects: [
    {
      metric: "active_projects",
      sql: `SELECT COUNT(*) AS value FROM site_projects WHERE archived_at IS NULL`,
    },
    {
      metric: "approved_projects",
      sql: `SELECT COUNT(*) AS value FROM site_projects WHERE status = 'approved'`,
    },
    {
      metric: "projects_with_active_deployment",
      sql: `SELECT COUNT(*) AS value FROM site_projects WHERE active_deployment_id IS NOT NULL`,
    },
  ],
  site_versions: [
    {
      metric: "saved_versions",
      sql: `SELECT COUNT(*) AS value FROM site_versions WHERE build_status = 'saved'`,
    },
    {
      metric: "failed_versions",
      sql: `SELECT COUNT(*) AS value FROM site_versions WHERE build_status = 'build_failed'`,
    },
    {
      metric: "sum_manifest_length",
      sql: `SELECT COALESCE(SUM(LENGTH(static_assets_manifest_json)), 0) AS value FROM site_versions`,
    },
  ],
  site_build_validations: [],
  site_revision_feedback: [],
  site_compatibility_checks: [],
  site_provisioning_plans: [],
  site_storage_bindings: [],
  site_source_exports: [],
  site_referral_sources: [],
  referral_invites: [],
  site_referral_policy_events: [],
  site_environment_values: [],
  site_commerce_products: [
    {
      metric: "sum_amount_minor_units",
      sql: `SELECT COALESCE(SUM(amount), 0) AS value FROM site_commerce_products`,
    },
  ],
  site_commerce_paid_actions: [
    {
      metric: "sum_amount_minor_units",
      sql: `SELECT COALESCE(SUM(amount), 0) AS value FROM site_commerce_paid_actions`,
    },
  ],
  site_commerce_payment_events: [
    {
      metric: "sum_amount",
      sql: `SELECT COALESCE(SUM(amount), 0) AS value FROM site_commerce_payment_events`,
    },
    {
      metric: "sum_amount_credits",
      sql: `SELECT COALESCE(SUM(CASE WHEN asset = 'credits' THEN amount ELSE 0 END), 0) AS value FROM site_commerce_payment_events`,
    },
    {
      metric: "sum_amount_sats",
      sql: `SELECT COALESCE(SUM(CASE WHEN asset = 'sats' THEN amount ELSE 0 END), 0) AS value FROM site_commerce_payment_events`,
    },
    {
      metric: "sum_amount_usd",
      sql: `SELECT COALESCE(SUM(CASE WHEN asset = 'usd' THEN amount ELSE 0 END), 0) AS value FROM site_commerce_payment_events`,
    },
  ],
  site_commerce_revenue_share_links: [
    {
      metric: "claimed_payouts",
      sql: `SELECT COALESCE(SUM(provider_payout_claimed), 0) AS value FROM site_commerce_revenue_share_links`,
    },
  ],
  site_commerce_review_decisions: [],
  site_mdk_checkout_intents: [
    {
      metric: "sum_amount_minor_units",
      sql: `SELECT COALESCE(SUM(amount_minor_units), 0) AS value FROM site_mdk_checkout_intents`,
    },
  ],
  site_mdk_account_bindings: [],
  site_payment_catalog_items: [
    {
      metric: "sum_price_amount_minor_units",
      sql: `SELECT COALESCE(SUM(price_amount_minor_units), 0) AS value FROM site_payment_catalog_items`,
    },
  ],
  site_referral_payout_ledger_entries: [
    {
      metric: "sum_amount_sats",
      sql: `SELECT COALESCE(SUM(amount_sats), 0) AS value FROM site_referral_payout_ledger_entries`,
    },
    {
      metric: "sum_qualifying_amount_sats",
      sql: `SELECT COALESCE(SUM(qualifying_amount_sats), 0) AS value FROM site_referral_payout_ledger_entries`,
    },
  ],
  targeted_site_campaigns: [],
  targeted_site_prospects: [],
  targeted_site_capture_policy_events: [],
  targeted_site_static_capture_runs: [],
  targeted_site_rendered_capture_runs: [],
  targeted_site_capture_provider_adapter_runs: [],
  targeted_site_quality_audits: [],
  targeted_site_remake_briefs: [],
  targeted_site_remake_preview_generations: [],
  targeted_site_operator_review_events: [],
  targeted_site_remake_outreach_email_dispatches: [],
  targeted_site_agent_toolkit_grants: [],
  targeted_site_agent_toolkit_actions: [],
  targeted_site_sales_reward_policy_events: [
    {
      metric: "sum_reward_amount",
      sql: `SELECT COALESCE(SUM(reward_amount), 0) AS value FROM targeted_site_sales_reward_policy_events`,
    },
  ],
  tenant_custom_hostnames: [],
  deployments: [],
  deployment_events: [],

}

export const postgresSitesContentScalar = async (
  sql: SyncSql,
  tallySql: string,
): Promise<number> => {
  const unsafe = requireSitesContentUnsafe(sql)
  const rows = await unsafe(tallySql, [])
  return Number(rows[0]?.["value"] ?? 0)
}

// ---------------------------------------------------------------------------
// Grouped chains (version chains, deployment states, builder sequences)
// ---------------------------------------------------------------------------

/**
 * A grouped chain row: for one group key (site or session), the member
 * count, distinct-order-value count, and min/max order value. The
 * KS-8.12 acceptance objects are exactly these shapes:
 *
 *   - version chains: site_versions grouped by site_id over created_at;
 *   - builder transcripts: messages/events/phase runs grouped by
 *     session_id over sequence.
 *
 * Comparison is store-vs-store chain-map EQUALITY, not absolute
 * contiguity (rejected/superseded versions legitimately share the chain
 * on both sides).
 */
export type GroupChainRow = Readonly<{
  groupKey: string
  members: number
  distinctOrders: number
  minOrder: string
  maxOrder: string
}>

export const groupChainSql = (
  table: SitesContentTable,
  groupColumn: string,
  orderColumn: string,
): string =>
  `SELECT ${groupColumn} AS group_key,
       COUNT(*) AS members,
       COUNT(DISTINCT ${orderColumn}) AS distinct_orders,
       MIN(${orderColumn}) AS min_order,
       MAX(${orderColumn}) AS max_order
  FROM ${table}
 GROUP BY ${groupColumn}
 ORDER BY ${groupColumn}`

/** The chain queries the verify pass runs (same text on D1 + Postgres). */
export const SITES_CONTENT_CHAINS: ReadonlyArray<
  Readonly<{
    name: string
    table: SitesContentTable
    sql: string
  }>
> = [
  {
    name: "version_chain_per_site",
    sql: groupChainSql("site_versions", "site_id", "created_at"),
    table: "site_versions",
  },
  {
    name: "message_chain_per_session",
    sql: groupChainSql("site_builder_messages", "session_id", "sequence"),
    table: "site_builder_messages",
  },
  {
    name: "event_chain_per_session",
    sql: groupChainSql("site_builder_events", "session_id", "sequence"),
    table: "site_builder_events",
  },
  {
    name: "phase_run_chain_per_session",
    sql: groupChainSql("site_builder_phase_runs", "session_id", "sequence"),
    table: "site_builder_phase_runs",
  },
]

export const groupChainRowsFromRaw = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlyArray<GroupChainRow> =>
  rows.map((row) => ({
    distinctOrders: Number(row["distinct_orders"] ?? 0),
    groupKey: String(row["group_key"] ?? "<null>"),
    maxOrder: String(row["max_order"] ?? ""),
    members: Number(row["members"] ?? 0),
    minOrder: String(row["min_order"] ?? ""),
  }))

export const postgresGroupChains = async (
  sql: SyncSql,
  chainSql: string,
): Promise<ReadonlyArray<GroupChainRow>> => {
  const unsafe = requireSitesContentUnsafe(sql)
  return groupChainRowsFromRaw(await unsafe(chainSql, []))
}

export type GroupChainMismatch = Readonly<{
  groupKey: string
  d1: GroupChainRow | undefined
  postgres: GroupChainRow | undefined
}>

export const compareGroupChains = (
  d1: ReadonlyArray<GroupChainRow>,
  postgres: ReadonlyArray<GroupChainRow>,
): ReadonlyArray<GroupChainMismatch> => {
  const postgresByKey = new Map(
    postgres.map((chain) => [chain.groupKey, chain]),
  )
  const mismatches: Array<GroupChainMismatch> = []
  const seen = new Set<string>()
  for (const chain of d1) {
    seen.add(chain.groupKey)
    const twin = postgresByKey.get(chain.groupKey)
    if (
      twin === undefined ||
      twin.members !== chain.members ||
      twin.distinctOrders !== chain.distinctOrders ||
      twin.minOrder !== chain.minOrder ||
      twin.maxOrder !== chain.maxOrder
    ) {
      mismatches.push({ d1: chain, groupKey: chain.groupKey, postgres: twin })
    }
  }
  for (const chain of postgres) {
    if (!seen.has(chain.groupKey)) {
      mismatches.push({ d1: undefined, groupKey: chain.groupKey, postgres: chain })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Deployment state machine (per-site per-status census)
// ---------------------------------------------------------------------------

/**
 * The deployment state-machine census: one row per (site_id, status)
 * with its count. Equality of the full census across stores IS the
 * KS-8.12 "deployment state-machine equality" acceptance at the storage
 * layer. Same text runs on D1 + Postgres.
 */
export const deploymentStateSql = (): string =>
  `SELECT site_id, status, COUNT(*) AS deployments
  FROM site_deployments
 GROUP BY site_id, status
 ORDER BY site_id, status`

export type DeploymentStateRow = Readonly<{
  siteId: string
  status: string
  deployments: number
}>

export const deploymentStateRowsFromRaw = (
  rows: ReadonlyArray<Record<string, unknown>>,
): ReadonlyArray<DeploymentStateRow> =>
  rows.map((row) => ({
    deployments: Number(row["deployments"] ?? 0),
    siteId: String(row["site_id"] ?? "<null>"),
    status: String(row["status"] ?? "<null>"),
  }))

export const postgresDeploymentStates = async (
  sql: SyncSql,
): Promise<ReadonlyArray<DeploymentStateRow>> => {
  const unsafe = requireSitesContentUnsafe(sql)
  return deploymentStateRowsFromRaw(await unsafe(deploymentStateSql(), []))
}

export type DeploymentStateMismatch = Readonly<{
  siteId: string
  status: string
  d1: number | undefined
  postgres: number | undefined
}>

export const compareDeploymentStates = (
  d1: ReadonlyArray<DeploymentStateRow>,
  postgres: ReadonlyArray<DeploymentStateRow>,
): ReadonlyArray<DeploymentStateMismatch> => {
  const key = (row: DeploymentStateRow) => `${row.siteId}${row.status}`
  const postgresByKey = new Map(postgres.map((row) => [key(row), row]))
  const mismatches: Array<DeploymentStateMismatch> = []
  const seen = new Set<string>()
  for (const row of d1) {
    seen.add(key(row))
    const twin = postgresByKey.get(key(row))
    if (twin === undefined || twin.deployments !== row.deployments) {
      mismatches.push({
        d1: row.deployments,
        postgres: twin?.deployments,
        siteId: row.siteId,
        status: row.status,
      })
    }
  }
  for (const row of postgres) {
    if (!seen.has(key(row))) {
      mismatches.push({
        d1: undefined,
        postgres: row.deployments,
        siteId: row.siteId,
        status: row.status,
      })
    }
  }
  return mismatches
}

// ---------------------------------------------------------------------------
// Verify report
// ---------------------------------------------------------------------------

export type SitesContentVerifyReport = Readonly<{
  table: SitesContentTable
  countsMatch: boolean
  d1Total: number
  postgresTotal: number
  scalarMismatches: ReadonlyArray<{
    metric: string
    d1: number
    postgres: number
  }>
  chainMismatches: ReadonlyArray<GroupChainMismatch>
  stateMismatches: ReadonlyArray<DeploymentStateMismatch>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

export const compareNewestHashes = (
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): SitesContentVerifyReport["newestHashMismatches"] => {
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

export const buildSitesContentVerifyReport = (
  input: Readonly<{
    table: SitesContentTable
    d1Total: number
    postgresTotal: number
    scalars: ReadonlyArray<{ metric: string; d1: number; postgres: number }>
    d1Chains?: ReadonlyArray<GroupChainRow> | undefined
    postgresChains?: ReadonlyArray<GroupChainRow> | undefined
    d1States?: ReadonlyArray<DeploymentStateRow> | undefined
    postgresStates?: ReadonlyArray<DeploymentStateRow> | undefined
    d1Newest: ReadonlyArray<NewestRowHash>
    postgresNewest: ReadonlyArray<NewestRowHash>
  }>,
): SitesContentVerifyReport => ({
  chainMismatches:
    input.d1Chains === undefined || input.postgresChains === undefined
      ? []
      : compareGroupChains(input.d1Chains, input.postgresChains),
  countsMatch: input.d1Total === input.postgresTotal,
  d1Total: input.d1Total,
  newestHashMismatches: compareNewestHashes(
    input.d1Newest,
    input.postgresNewest,
  ),
  postgresTotal: input.postgresTotal,
  scalarMismatches: input.scalars.filter(
    (scalar) => scalar.d1 !== scalar.postgres,
  ),
  stateMismatches:
    input.d1States === undefined || input.postgresStates === undefined
      ? []
      : compareDeploymentStates(input.d1States, input.postgresStates),
  table: input.table,
})

export const sitesContentVerifyReportClean = (
  report: SitesContentVerifyReport,
): boolean =>
  report.countsMatch &&
  report.scalarMismatches.length === 0 &&
  report.chainMismatches.length === 0 &&
  report.stateMismatches.length === 0 &&
  report.newestHashMismatches.length === 0

// ---------------------------------------------------------------------------
// Set-membership referential checks (money/referral integrity, no cross-store
// joins — the KS-8.12 acceptance for the commerce tables)
// ---------------------------------------------------------------------------

/**
 * A child.column ⊆ parent.column referential check run WITHIN each store
 * (never a cross-store join): the commerce/referral remainder tables carry
 * KS-8.7/8.8-style refs and parent ids, and mirror-only fidelity means the
 * membership relation must hold on Postgres exactly as it holds on D1.
 */
export type SitesReferentialCheck = Readonly<{
  name: string
  childTable: SitesContentTable
  childColumn: string
  parentTable: SitesContentTable
  parentColumn: string
}>

export const SITES_REMAINDER_REFERENTIAL_CHECKS: ReadonlyArray<SitesReferentialCheck> =
  [
    {
      childColumn: "payment_event_id",
      childTable: "site_commerce_revenue_share_links",
      name: "revenue_share_links.payment_event_id in payment_events.id",
      parentColumn: "id",
      parentTable: "site_commerce_payment_events",
    },
    {
      childColumn: "referral_source_id",
      childTable: "site_referral_payout_ledger_entries",
      name: "payout_ledger.referral_source_id in referral_sources.id",
      parentColumn: "id",
      parentTable: "site_referral_sources",
    },
    {
      childColumn: "referral_source_id",
      childTable: "referral_invites",
      name: "referral_invites.referral_source_id in referral_sources.id",
      parentColumn: "id",
      parentTable: "site_referral_sources",
    },
  ]

/** Child key values (non-null) that are absent from the parent key set. */
export const missingReferences = (
  childValues: ReadonlyArray<unknown>,
  parentValues: ReadonlyArray<unknown>,
): ReadonlyArray<string> => {
  const parents = new Set(
    parentValues
      .map((value) => normalizeSitesContentValue(value))
      .filter((value): value is string | number => value !== null)
      .map((value) => String(value)),
  )
  const missing = new Set<string>()
  for (const raw of childValues) {
    const value = normalizeSitesContentValue(raw)
    if (value === null) continue
    if (!parents.has(String(value))) missing.add(String(value))
  }
  return [...missing]
}

/** Distinct non-null values of one column on the Postgres twin. */
export const postgresDistinctColumn = async (
  sql: SyncSql,
  table: SitesContentTable,
  column: string,
): Promise<ReadonlyArray<string>> => {
  const unsafe = requireSitesContentUnsafe(sql)
  const rows = await unsafe(
    `SELECT DISTINCT ${column} AS value FROM ${table} WHERE ${column} IS NOT NULL`,
    [],
  )
  return rows.map((row) => String(row["value"]))
}
