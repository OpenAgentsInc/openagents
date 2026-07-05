/**
 * KS-8.8 (#8319): Treasury / payouts / tips settlement backfill core —
 * D1 → Postgres.
 *
 * Testable core behind `scripts/backfill-treasury.ts`, following the
 * KS-8.6 artanis lane (`artanis-backfill.ts`). Takes raw D1 rows
 * (snake_case objects, exactly as `wrangler d1 execute --json` returns
 * them) and upserts them into the Postgres twins from migration
 * `0016_treasury_domain.sql` with `ON CONFLICT ... DO NOTHING` — so the
 * backfill NEVER fights the live dual-write mirror (rows the mirror
 * already converged are fresher than any snapshot page; rows the mirror
 * never touched are filled here). Running a batch twice is a no-op by
 * construction (idempotency test: `treasury-backfill.test.ts`).
 *
 * MONEY RECONCILIATION IS THE ACCEPTANCE (MIGRATION_PLAN §3.5): verify
 * mode compares, per table, (1) exact row counts, (2) per-status/state row
 * tallies, (3) exact SUMs of every money column PER STATE and PER RAIL
 * (adapter kind / asset / direction where the table has one) — millisat
 * and minor-unit totals must match exactly, and (4) newest-N row-hash
 * comparison over a canonical column serialization. Nothing "close
 * enough": exact or explain.
 *
 * D1 and Postgres table names are IDENTICAL for this domain.
 *
 * `mpp_lightning_replay` / `mpp_spt_replay` (the x402/MPP chat-endpoint
 * replay guards) were retired from this registry in migration
 * `0036_drop_treasury_mpp_replay_tables.sql` (#8282 follow-up): D1 worker
 * migration `0303_drop_mpp_replay_tables.sql` already dropped both tables
 * (the `/mpp/v1/chat/completions` route was removed per #8387 — Khala Code
 * paid-plan purchases use their own payment-intent ledger and never read
 * either cache), the live Worker treasury store never wired a dual-write
 * mirror for them, and the Postgres twin held only a single stale
 * dual-write-converged row with no live D1 counterpart left to reconcile
 * against. Keeping them in this registry made `--verify` (with no
 * `--table` filter) hard-crash on "no such table" instead of completing —
 * a real gap found during the 2026-07-05 backup-verification follow-up.
 */

import { createHash } from "node:crypto"
import type { SyncSql } from "./sql.js"

// ---------------------------------------------------------------------------
// Table registry (column lists mirror migration 0016 exactly, which mirrors
// the live D1 schema: worker migrations 0101/0122/0128/0131/0143/0146/0147/
// 0149/0151/0153/0159..0167/0184/0196..0199/0203/0204/0206/0211/0214/0224/
// 0225/0261/0293)
// ---------------------------------------------------------------------------

export type TreasuryBackfillTable =
  | "treasury_transactions"
  | "nexus_payout_target_approvals"
  | "nexus_treasury_payout_intents"
  | "nexus_treasury_payout_attempts"
  | "nexus_treasury_payout_reconciliation_events"
  | "nexus_payment_authority_receipts"
  | "nexus_release_gates"
  | "forum_money_actions"
  | "forum_payment_events"
  | "forum_receipts"
  | "forum_l402_challenges"
  | "forum_l402_redemptions"
  | "forum_direct_tip_attempts"
  | "forum_direct_tip_webhook_events"
  | "forum_tip_recipient_wallets"
  | "forum_tip_settlement_claims"
  | "x_claim_reward_ledger"
  | "agent_claim_reward_ledger"
  | "agent_balances"
  | "labor_escrows"
  | "labor_escrow_receipts"
  | "partner_payout_ledger_entries"
  | "partner_agreements"
  | "site_referral_payout_ledger_entries"
  | "revenue_event_provenance"

export type TreasuryTableSpec = Readonly<{
  /** Column list in canonical (migration) order. */
  columns: ReadonlyArray<string>
  /** Conflict target for the DO NOTHING upsert (the table's natural key). */
  conflictKey: string
  /** Column newest-N verification orders by (text ISO timestamps sort). */
  orderColumn: string
  /** Low-cardinality column the per-status tally groups by (null = one group). */
  statusColumn: string | null
  /**
   * Money columns whose SUM must match EXACTLY per status group — the
   * millisat/minor-unit reconciliation the money domain hangs on.
   */
  amountColumns: ReadonlyArray<string>
  /** Optional second grouping dimension (rail: adapter kind / asset / direction). */
  railColumn: string | null
}>

export const TREASURY_TABLE_SPECS: Readonly<
  Record<TreasuryBackfillTable, TreasuryTableSpec>
> = {
  agent_balances: {
    amountColumns: ["balance_msat", "held_msat", "usd_credit_msat"],
    columns: [
      "actor_ref", "balance_msat", "sweep_enabled", "sweep_threshold_sat",
      "send_credits_below_sat", "receive_credits_below_sat", "created_at",
      "updated_at", "held_msat", "usd_credit_msat",
    ],
    conflictKey: "actor_ref",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "sweep_enabled",
  },
  agent_claim_reward_ledger: {
    amountColumns: ["amount_sats"],
    columns: [
      "id", "idempotency_key", "campaign_ref", "agent_claim_ref", "owner_ref",
      "x_account_ref", "tweet_ref", "state", "amount_sats", "destination_kind",
      "redacted_destination_ref", "payout_intent_ref", "dispatch_attempt_ref",
      "settlement_ref", "rejection_reason", "policy_refs_json",
      "caveat_refs_json", "created_at", "updated_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "state",
  },
  forum_direct_tip_attempts: {
    amountColumns: ["amount_sats"],
    columns: [
      "id", "idempotency_key", "payer_actor_ref", "recipient_actor_ref",
      "target_topic_id", "target_post_id", "target_post_permalink",
      "amount_sats", "provider_ref", "external_ref", "redacted_evidence_ref",
      "payment_mode", "payment_event_status", "status", "receipt_ref",
      "payment_event_id", "created_at", "updated_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    railColumn: "payment_mode",
    statusColumn: "status",
  },
  forum_direct_tip_webhook_events: {
    amountColumns: ["amount_sats"],
    columns: [
      "id", "provider_event_ref", "direct_tip_attempt_id", "provider_ref",
      "external_ref", "amount_sats", "payment_event_status",
      "redacted_evidence_ref", "event_body_digest_ref", "signature_binding_ref",
      "reconciliation_status", "reconciliation_result", "first_seen_at",
      "last_seen_at", "delivery_count", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "last_seen_at",
    railColumn: null,
    statusColumn: "reconciliation_status",
  },
  forum_l402_challenges: {
    amountColumns: ["price_value"],
    columns: [
      "id", "idempotency_key", "actor_ref", "action_kind", "method", "path",
      "route_params_json", "request_body_digest", "target_forum_id",
      "target_topic_id", "target_post_id", "price_asset", "price_value",
      "spend_cap_asset", "spend_cap_value", "expires_at",
      "public_projection_json", "created_at", "archived_at",
      "recipient_actor_ref", "recipient_readiness_ref", "mdk_provider_ref",
      "mdk_environment", "mdk_sandbox", "mdk_implementation_state",
      "mdk_checkout_ref", "mdk_checkout_url_ref", "mdk_checkout_launch_path",
      "mdk_invoice_ref", "mdk_payment_hash_ref", "l402_credential_ref",
      "l402_replay_nonce_ref", "l402_endpoint_ref",
      "l402_entitlement_scope_refs_json", "l402_www_authenticate",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "price_asset",
    statusColumn: "action_kind",
  },
  forum_l402_redemptions: {
    amountColumns: [],
    columns: [
      "id", "idempotency_key", "challenge_id", "actor_ref", "proof_ref",
      "entitlement_ref", "receipt_id", "replayed", "public_projection_json",
      "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: null,
    statusColumn: "replayed",
  },
  forum_money_actions: {
    amountColumns: ["amount_value"],
    columns: [
      "id", "idempotency_key", "actor_ref", "action_kind", "target_forum_id",
      "target_topic_id", "target_post_id", "amount_asset", "amount_value",
      "payment_event_id", "receipt_id", "earning_actor_ref",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "amount_asset",
    statusColumn: "action_kind",
  },
  forum_payment_events: {
    amountColumns: ["amount_value"],
    columns: [
      "id", "money_action_id", "provider_ref", "external_ref", "amount_asset",
      "amount_value", "redacted_evidence_ref", "public_projection_json",
      "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "provider_ref",
    statusColumn: "amount_asset",
  },
  forum_receipts: {
    amountColumns: ["amount_value"],
    columns: [
      "id", "receipt_ref", "action_kind", "target_forum_id", "target_topic_id",
      "target_post_id", "amount_asset", "amount_value", "recipient_actor_ref",
      "redacted_payment_ref", "public_projection_json", "created_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "amount_asset",
    statusColumn: "action_kind",
  },
  forum_tip_recipient_wallets: {
    amountColumns: [],
    columns: [
      "id", "actor_ref", "provider_class", "wallet_ref",
      "receive_capability_ref", "payout_target_approval_ref",
      "readiness_refs_json", "caveat_refs_json", "custody_policy_refs_json",
      "claim_policy_refs_json", "source_ref", "state",
      "public_projection_json", "created_at", "updated_at", "disabled_at",
      "archived_at", "bolt12_offer", "lightning_address", "spark_address",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "state",
  },
  forum_tip_settlement_claims: {
    amountColumns: [],
    columns: [
      "id", "idempotency_key", "receipt_id", "receipt_ref",
      "recipient_actor_ref", "settlement_ref",
      "settlement_evidence_refs_json", "source_ref", "public_projection_json",
      "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: null,
    statusColumn: "source_ref",
  },
  labor_escrow_receipts: {
    amountColumns: ["amount_msat"],
    columns: [
      "id", "escrow_id", "idempotency_key", "transition_kind",
      "work_request_id", "requester_actor_ref", "provider_actor_ref",
      "amount_msat", "receipt_ref", "evidence_ref", "state_after",
      "forfeit_destination", "forfeit_destination_actor_ref",
      "public_projection_json", "created_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: null,
    statusColumn: "transition_kind",
  },
  labor_escrows: {
    amountColumns: ["amount_msat"],
    columns: [
      "id", "idempotency_key", "work_request_id", "requester_actor_ref",
      "provider_actor_ref", "amount_msat", "state", "funding_source",
      "job_event_id", "acceptance_event_ref", "reserve_receipt_ref",
      "release_receipt_ref", "refund_receipt_ref", "forfeit_receipt_ref",
      "forfeit_destination", "forfeit_destination_actor_ref",
      "forfeit_condition_ref", "public_projection_json", "created_at",
      "updated_at", "released_at", "refunded_at", "forfeited_at",
      "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "state",
  },
  nexus_payment_authority_receipts: {
    amountColumns: [],
    columns: [
      "id", "receipt_ref", "payout_intent_ref", "payout_attempt_ref",
      "event_ref", "receipt_kind", "audience", "metadata_refs_json",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflictKey: "receipt_ref",
    orderColumn: "created_at",
    railColumn: "audience",
    statusColumn: "receipt_kind",
  },
  nexus_payout_target_approvals: {
    amountColumns: [],
    columns: [
      "id", "approval_ref", "idempotency_key_hash", "payout_target_ref",
      "redacted_destination_ref", "owner_user_id", "agent_ref", "pylon_ref",
      "status", "approved_by_ref", "approval_policy_ref", "scope_refs_json",
      "public_projection_json", "created_at", "updated_at", "expires_at",
      "archived_at",
    ],
    conflictKey: "approval_ref",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "status",
  },
  nexus_release_gates: {
    amountColumns: [],
    columns: [
      "id", "gate_ref", "idempotency_key_hash", "gate_kind", "status",
      "evidence_refs_json", "blocker_refs_json", "public_projection_json",
      "created_at", "updated_at", "archived_at",
    ],
    conflictKey: "gate_ref",
    orderColumn: "updated_at",
    railColumn: "gate_kind",
    statusColumn: "status",
  },
  nexus_treasury_payout_attempts: {
    amountColumns: ["amount_minor_units"],
    columns: [
      "id", "payout_attempt_ref", "payout_intent_ref", "idempotency_key_hash",
      "adapter_kind", "adapter_attempt_ref", "status", "redacted_payment_ref",
      "redacted_destination_ref", "amount_asset", "amount_denomination",
      "amount_minor_units", "metadata_refs_json", "public_projection_json",
      "created_at", "updated_at", "archived_at",
    ],
    conflictKey: "payout_attempt_ref",
    orderColumn: "updated_at",
    railColumn: "adapter_kind",
    statusColumn: "status",
  },
  nexus_treasury_payout_intents: {
    amountColumns: ["amount_minor_units", "spend_cap_amount_minor_units"],
    columns: [
      "id", "payout_intent_ref", "idempotency_key_hash", "actor_ref",
      "owner_user_id", "source_kind", "buyer_payment_ref",
      "accepted_work_refs_json", "assignment_ref", "artanis_dispatch_ref",
      "pylon_job_ref", "payout_target_ref", "payout_target_approval_ref",
      "adapter_kind", "amount_asset", "amount_denomination",
      "amount_minor_units", "spend_cap_asset", "spend_cap_denomination",
      "spend_cap_amount_minor_units", "policy_snapshot_ref", "status",
      "metadata_refs_json", "public_projection_json", "created_at",
      "updated_at", "archived_at",
    ],
    conflictKey: "payout_intent_ref",
    orderColumn: "updated_at",
    railColumn: "adapter_kind",
    statusColumn: "status",
  },
  nexus_treasury_payout_reconciliation_events: {
    amountColumns: [],
    columns: [
      "id", "event_ref", "idempotency_key_hash", "provider_ref",
      "external_event_ref", "adapter_kind", "payout_intent_ref",
      "payout_attempt_ref", "status", "result_ref", "metadata_refs_json",
      "public_projection_json", "created_at", "archived_at",
    ],
    conflictKey: "event_ref",
    orderColumn: "created_at",
    railColumn: "adapter_kind",
    statusColumn: "status",
  },
  partner_agreements: {
    amountColumns: [],
    columns: [
      "id", "agreement_ref", "partner_ref", "partner_user_id",
      "customer_user_id", "role", "effective_from", "effective_until",
      "policy_state", "created_at", "archived_at",
    ],
    conflictKey: "agreement_ref",
    orderColumn: "created_at",
    railColumn: "role",
    statusColumn: "policy_state",
  },
  partner_payout_ledger_entries: {
    amountColumns: ["amount", "qualifying_amount"],
    columns: [
      "id", "payout_ref", "idempotency_key", "partner_role",
      "partner_user_id", "partner_ref", "beneficiary_user_id", "asset",
      "qualifying_event_ref", "qualifying_event_kind", "qualifying_amount",
      "amount", "period_key", "state", "state_reason_ref",
      "previous_entry_id", "reversal_of_entry_id", "evidence_refs_json",
      "policy_refs_json", "caveat_refs_json", "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "asset",
    statusColumn: "state",
  },
  revenue_event_provenance: {
    amountColumns: ["amount_cents", "amount_sats"],
    columns: [
      "event_ref", "evidence_bundle_ref", "idempotency_key", "product_ref",
      "revenue_surface_ref", "receipt_ref", "ledger_table", "ledger_row_ref",
      "demand_provenance", "payment_state", "amount_cents", "amount_sats",
      "public_evidence_refs_json", "caveat_refs_json", "source_refs_json",
      "recorded_at", "created_at", "updated_at",
    ],
    conflictKey: "event_ref",
    orderColumn: "recorded_at",
    railColumn: "product_ref",
    statusColumn: "payment_state",
  },
  site_referral_payout_ledger_entries: {
    amountColumns: ["amount_sats", "qualifying_amount_sats"],
    columns: [
      "id", "payout_ref", "idempotency_key", "referral_attribution_id",
      "referral_source_id", "referral_invite_id", "referrer_user_id",
      "referred_user_id", "qualifying_event_ref", "qualifying_event_kind",
      "qualifying_amount_sats", "amount_sats", "period_key", "state",
      "state_reason_ref", "previous_entry_id", "reversal_of_entry_id",
      "evidence_refs_json", "policy_refs_json", "caveat_refs_json",
      "created_at", "archived_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: null,
    statusColumn: "state",
  },
  treasury_transactions: {
    amountColumns: ["amount_sat"],
    columns: [
      "id", "direction", "amount_sat", "state", "bolt11", "payment_ref",
      "created_at", "settled_at", "expires_at", "failure_reason_ref",
      "recipient_ref", "redacted_destination_ref", "owed_ref", "owed_sat",
      "recipient_confirmation_state", "recipient_confirmation_ref",
      "recipient_confirmed_at",
    ],
    conflictKey: "id",
    orderColumn: "created_at",
    railColumn: "direction",
    statusColumn: "state",
  },
  x_claim_reward_ledger: {
    amountColumns: ["amount_sats"],
    columns: [
      "id", "challenge_id", "claim_id", "owner_user_id", "agent_user_id",
      "x_account_ref", "amount_sats", "state", "state_reason_ref",
      "receipt_ref", "evidence_refs_json", "created_at", "updated_at",
      "treasury_payment_id",
    ],
    conflictKey: "id",
    orderColumn: "updated_at",
    railColumn: null,
    statusColumn: "state",
  },
}

export const TREASURY_BACKFILL_TABLES = Object.keys(
  TREASURY_TABLE_SPECS,
) as ReadonlyArray<TreasuryBackfillTable>

export type D1SourceRow = Readonly<Record<string, unknown>>

const normalizeValue = (value: unknown): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}

/**
 * Both Bun SQL and postgres.js expose `unsafe(text, params)` for
 * dynamic-text parameterized statements; the structural `SyncSql` seam
 * deliberately does not, so this module widens it locally. The backfill
 * runs over DIRECT connections (never Hyperdrive).
 */
type UnsafeQuery = (
  text: string,
  params: Array<unknown>,
) => Promise<Array<Record<string, unknown>>>

const requireUnsafe = (sql: SyncSql): UnsafeQuery => {
  const unsafe = (sql as SyncSql & { unsafe?: UnsafeQuery }).unsafe
  if (typeof unsafe !== "function") {
    throw new Error(
      "treasury backfill requires a driver exposing unsafe(text, params) (Bun SQL or postgres.js)",
    )
  }
  return unsafe
}

/**
 * Upsert one page of D1 rows into `table`. `ON CONFLICT (natural key) DO
 * NOTHING`: rows the dual-write mirror already owns win — the backfill
 * NEVER overwrites a settlement state or amount the live authority has
 * since advanced. Returns how many rows were actually inserted (0 on a
 * re-run — the idempotency contract).
 */
export const upsertTreasuryRows = async (
  sql: SyncSql,
  table: TreasuryBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): Promise<number> => {
  if (rows.length === 0) return 0
  const unsafe = requireUnsafe(sql)
  const spec = TREASURY_TABLE_SPECS[table]
  let inserted = 0
  for (const row of rows) {
    const values = spec.columns.map((column) => normalizeValue(row[column]))
    const columnsSql = spec.columns.join(", ")
    const placeholders = values.map((_, index) => `$${index + 1}`).join(", ")
    const result = await unsafe(
      `INSERT INTO ${table} (${columnsSql}) VALUES (${placeholders}) ON CONFLICT (${spec.conflictKey}) DO NOTHING RETURNING ${spec.conflictKey}`,
      values as Array<unknown>,
    )
    inserted += result.length
  }
  return inserted
}

// ---------------------------------------------------------------------------
// Verification
// ---------------------------------------------------------------------------

/** One (status, rail) group: row count + exact SUM per money column. */
export type TreasuryGroupTally = Readonly<{
  count: number
  /** amount column → exact SUM as a decimal string (bigint-safe). */
  sums: Readonly<Record<string, string>>
}>

export type TreasuryVerifyTally = Readonly<{
  total: number
  /** "status|rail" → group tally. */
  byGroup: Readonly<Record<string, TreasuryGroupTally>>
}>

const groupKey = (status: unknown, rail: unknown): string =>
  `${status === null || status === undefined ? "<null>" : String(status)}|${
    rail === null || rail === undefined ? "<null>" : String(rail)
  }`

/**
 * Count + per-(status, rail) tally with exact money SUMs over the Postgres
 * side of one table. SUMs are read back as text so bigint totals never
 * pass through float.
 */
export const postgresTreasuryTally = async (
  sql: SyncSql,
  table: TreasuryBackfillTable,
): Promise<TreasuryVerifyTally> => {
  const spec = TREASURY_TABLE_SPECS[table]
  const statusExpr =
    spec.statusColumn === null ? "'<all>'" : `${spec.statusColumn}::text`
  const railExpr =
    spec.railColumn === null ? "'<all>'" : `${spec.railColumn}::text`
  const sumSelects = spec.amountColumns
    .map(
      (column) =>
        `, COALESCE(SUM(${column}), 0)::text AS sum_${column}`,
    )
    .join("")
  const rows = await requireUnsafe(sql)(
    `SELECT ${statusExpr} AS status_value, ${railExpr} AS rail_value, count(*) AS row_count${sumSelects} FROM ${table} GROUP BY 1, 2 ORDER BY 1, 2`,
    [],
  )
  const byGroup: Record<string, TreasuryGroupTally> = {}
  let total = 0
  for (const row of rows) {
    const count = Number(row["row_count"])
    const sums: Record<string, string> = {}
    for (const column of spec.amountColumns) {
      sums[column] = String(row[`sum_${column}`] ?? "0")
    }
    byGroup[groupKey(row["status_value"], row["rail_value"])] = { count, sums }
    total += count
  }
  return { byGroup, total }
}

/**
 * The same tally over raw D1 group rows (the CLI runs the equivalent
 * GROUP BY through wrangler and hands the rows here).
 */
export const d1TreasuryTallyFromGroups = (
  table: TreasuryBackfillTable,
  groups: ReadonlyArray<Record<string, unknown>>,
): TreasuryVerifyTally => {
  const spec = TREASURY_TABLE_SPECS[table]
  const byGroup: Record<string, TreasuryGroupTally> = {}
  let total = 0
  for (const row of groups) {
    const count = Number(row["row_count"])
    const sums: Record<string, string> = {}
    for (const column of spec.amountColumns) {
      // D1 SUM comes back as a JS number (or null); the CLI casts with
      // CAST(... AS TEXT) so bigint totals survive. Normalize either way.
      const raw = row[`sum_${column}`]
      sums[column] = raw === null || raw === undefined ? "0" : String(raw)
    }
    byGroup[groupKey(row["status_value"], row["rail_value"])] = { count, sums }
    total += count
  }
  return { byGroup, total }
}

/** The D1-side GROUP BY the CLI must run for `d1TreasuryTallyFromGroups`. */
export const d1TreasuryTallySql = (table: TreasuryBackfillTable): string => {
  const spec = TREASURY_TABLE_SPECS[table]
  const statusExpr = spec.statusColumn ?? "'<all>'"
  const railExpr = spec.railColumn ?? "'<all>'"
  const sumSelects = spec.amountColumns
    .map(
      (column) =>
        `, CAST(COALESCE(SUM(${column}), 0) AS TEXT) AS sum_${column}`,
    )
    .join("")
  return `SELECT ${statusExpr} AS status_value, ${railExpr} AS rail_value, COUNT(*) AS row_count${sumSelects} FROM ${table} GROUP BY 1, 2 ORDER BY 1, 2`
}

/**
 * Canonical row hash: the migration-order column values joined with unit
 * separators, sha256'd. Column normalization matches `upsertTreasuryRows`,
 * so the SAME D1 export row and its Postgres twin hash identically.
 */
export const treasuryRowHash = (
  table: TreasuryBackfillTable,
  row: D1SourceRow,
): string => {
  const hash = createHash("sha256")
  for (const column of TREASURY_TABLE_SPECS[table].columns) {
    const value = normalizeValue(row[column])
    hash.update(value === null ? " " : String(value))
    hash.update("")
  }
  return hash.digest("hex")
}

export type NewestRowHash = Readonly<{ key: string; hash: string }>

/**
 * Newest-N row hashes on the Postgres side, keyed by the table's natural
 * key, newest-first by the table's order column.
 */
export const postgresTreasuryNewestRowHashes = async (
  sql: SyncSql,
  table: TreasuryBackfillTable,
  limit: number,
): Promise<ReadonlyArray<NewestRowHash>> => {
  const spec = TREASURY_TABLE_SPECS[table]
  const rows = await requireUnsafe(sql)(
    `SELECT * FROM ${table} ORDER BY ${spec.orderColumn} DESC, ${spec.conflictKey} DESC LIMIT $1`,
    [limit],
  )
  return rows.map((row) => ({
    hash: treasuryRowHash(table, row),
    key: String(row[spec.conflictKey]),
  }))
}

/** Same newest-N hashing over D1 export rows (already fetched by the CLI). */
export const d1TreasuryNewestRowHashes = (
  table: TreasuryBackfillTable,
  rows: ReadonlyArray<D1SourceRow>,
): ReadonlyArray<NewestRowHash> => {
  const keyColumn = TREASURY_TABLE_SPECS[table].conflictKey
  return rows.map((row) => ({
    hash: treasuryRowHash(table, row),
    key: String(row[keyColumn]),
  }))
}

export type TreasuryVerifyTableReport = Readonly<{
  table: TreasuryBackfillTable
  d1Total: number
  postgresTotal: number
  countsMatch: boolean
  groupMismatches: ReadonlyArray<{
    group: string
    d1: TreasuryGroupTally | undefined
    postgres: TreasuryGroupTally | undefined
  }>
  newestHashMismatches: ReadonlyArray<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }>
}>

const groupTalliesEqual = (
  left: TreasuryGroupTally | undefined,
  right: TreasuryGroupTally | undefined,
): boolean => {
  if (left === undefined || right === undefined) return false
  if (left.count !== right.count) return false
  const columns = new Set([...Object.keys(left.sums), ...Object.keys(right.sums)])
  for (const column of columns) {
    // Compare as bigint so "1000" and "1000.0"/whitespace variants cannot
    // sneak past a string comparison, and totals stay exact.
    const a = left.sums[column]
    const b = right.sums[column]
    if (a === undefined || b === undefined) return false
    // A driver may render an integer SUM as "1000.0"; strip ONLY a
    // trailing all-zero fraction — never round a real value.
    const canonical = (value: string): bigint =>
      BigInt(value.trim().replace(/\.0+$/, ""))
    if (canonical(a) !== canonical(b)) return false
  }
  return true
}

export const compareTreasuryTallies = (
  table: TreasuryBackfillTable,
  d1: TreasuryVerifyTally,
  postgres: TreasuryVerifyTally,
  d1Newest: ReadonlyArray<NewestRowHash>,
  postgresNewest: ReadonlyArray<NewestRowHash>,
): TreasuryVerifyTableReport => {
  const groups = new Set([
    ...Object.keys(d1.byGroup),
    ...Object.keys(postgres.byGroup),
  ])
  const groupMismatches: Array<{
    group: string
    d1: TreasuryGroupTally | undefined
    postgres: TreasuryGroupTally | undefined
  }> = []
  for (const group of [...groups].sort()) {
    const d1Group = d1.byGroup[group]
    const postgresGroup = postgres.byGroup[group]
    if (!groupTalliesEqual(d1Group, postgresGroup)) {
      groupMismatches.push({ d1: d1Group, group, postgres: postgresGroup })
    }
  }

  const postgresByKey = new Map(
    postgresNewest.map((entry) => [entry.key, entry.hash]),
  )
  const newestHashMismatches: Array<{
    key: string
    d1Hash: string | undefined
    postgresHash: string | undefined
  }> = []
  for (const entry of d1Newest) {
    const postgresHash = postgresByKey.get(entry.key)
    if (postgresHash !== entry.hash) {
      newestHashMismatches.push({
        d1Hash: entry.hash,
        key: entry.key,
        postgresHash,
      })
    }
  }

  return {
    countsMatch: d1.total === postgres.total,
    d1Total: d1.total,
    groupMismatches,
    newestHashMismatches,
    postgresTotal: postgres.total,
    table,
  }
}
