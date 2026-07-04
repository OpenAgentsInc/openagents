/**
 * KS-8.7 (#8318): billing/Stripe/pay-ins domain — the shared table
 * metadata for the D1 → Cloud SQL migration lane.
 *
 * MONEY-DOMAIN DISCIPLINE: this file is the single source of truth for the
 * domain's column orders and natural keys, shared by
 *
 *   - the Worker dual-write mirror
 *     (`apps/openagents.com/workers/api/src/billing-store.ts`), which
 *     read-backs the FRESH authoritative D1 row and converge-upserts the
 *     byte-identical copy into Postgres, and
 *   - the backfill + verify core (`./billing-backfill.ts`), whose row
 *     hashes must normalize identically on both sides.
 *
 * Every conflict target below is CONVERGE (`ON CONFLICT (key) DO UPDATE SET
 * col = EXCLUDED.col`): mirror and backfill both copy rows that D1 has
 * already accepted, so converging to the D1 snapshot value can never alter
 * an amount or re-make an idempotency decision — the write-side dedupe
 * (INSERT OR IGNORE on `idempotency_key` / `event_id` / PK) happens on D1,
 * the sole authority, and is never re-evaluated against Postgres. Converge
 * (rather than DO NOTHING) is required because several tables are
 * legitimately UPDATEd after insert (webhook processing_status, checkout
 * fulfillment, pay-in state transitions, auto-top-up policies).
 *
 * Column orders mirror the FINAL D1 schema (after every rewrite migration)
 * and khala-sync migration `0015_billing_pay_ins.sql` exactly; the
 * `billing_ledger_entries_next` rewrite artifact was renamed away by worker
 * migrations 0031/0170 and does not exist as a live table (nothing to
 * migrate — the decommission follow-up verifies absence).
 */

export type BillingDomainTable =
  | "billing_accounts"
  | "billing_ledger_entries"
  | "billing_usage_cursors"
  | "billing_coupon_redemptions"
  | "billing_credit_notifications"
  | "billing_auto_top_up_policies"
  | "billing_auto_top_up_events"
  | "stripe_customers"
  | "stripe_checkout_sessions"
  | "stripe_webhook_events"
  | "stripe_saved_payment_methods"
  | "pay_ins"
  | "pay_in_legs"
  | "buyer_payment_challenges"
  | "buyer_payment_receipts"
  | "buyer_payment_entitlements"
  | "buyer_payment_redemptions"
  | "buyer_payment_spend_limits"
  | "buyer_payment_credit_debits"
  | "buyer_payment_reconciliation_events"
  | "first_batch_payment_policies"
  | "khala_code_paid_plan_payment_intents"

export const BILLING_DOMAIN_TABLES: ReadonlyArray<BillingDomainTable> = [
  "billing_accounts",
  "billing_ledger_entries",
  "billing_usage_cursors",
  "billing_coupon_redemptions",
  "billing_credit_notifications",
  "billing_auto_top_up_policies",
  "billing_auto_top_up_events",
  "stripe_customers",
  "stripe_checkout_sessions",
  "stripe_webhook_events",
  "stripe_saved_payment_methods",
  "pay_ins",
  "pay_in_legs",
  "buyer_payment_challenges",
  "buyer_payment_receipts",
  "buyer_payment_entitlements",
  "buyer_payment_redemptions",
  "buyer_payment_spend_limits",
  "buyer_payment_credit_debits",
  "buyer_payment_reconciliation_events",
  "first_batch_payment_policies",
  "khala_code_paid_plan_payment_intents",
]

export type BillingDomainTableSpec = Readonly<{
  /** Full column list in FINAL D1 schema order (hash + upsert order). */
  columns: ReadonlyArray<string>
  /** Natural key = the converge conflict target (PK on both sides). */
  keyColumns: ReadonlyArray<string>
  /** Newest-first ordering column for the verify hash sample. */
  orderColumn: string
}>

export const BILLING_DOMAIN_TABLE_SPECS: Readonly<
  Record<BillingDomainTable, BillingDomainTableSpec>
> = {
  billing_accounts: {
    columns: ["user_id", "currency", "status", "created_at", "updated_at"],
    keyColumns: ["user_id"],
    orderColumn: "updated_at",
  },
  billing_auto_top_up_events: {
    columns: [
      "id",
      "user_id",
      "status",
      "amount_cents",
      "currency",
      "balance_before_cents",
      "balance_after_cents",
      "stripe_payment_intent_id",
      "ledger_entry_id",
      "reason",
      "idempotency_key",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  billing_auto_top_up_policies: {
    columns: [
      "user_id",
      "currency",
      "enabled",
      "threshold_cents",
      "amount_cents",
      "monthly_cap_cents",
      "spent_this_month_cents",
      "cap_period_yyyymm",
      "status",
      "pause_reason",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["user_id", "currency"],
    orderColumn: "updated_at",
  },
  billing_coupon_redemptions: {
    columns: ["user_id", "coupon_code", "ledger_entry_id", "redeemed_at"],
    keyColumns: ["user_id", "coupon_code"],
    orderColumn: "redeemed_at",
  },
  billing_credit_notifications: {
    columns: [
      "user_id",
      "kind",
      "email",
      "display_name",
      "balance_cents",
      "status",
      "resend_email_id",
      "error_message",
      "idempotency_key",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["user_id", "kind"],
    orderColumn: "updated_at",
  },
  billing_ledger_entries: {
    columns: [
      "id",
      "user_id",
      "team_id",
      "run_id",
      "source",
      "description",
      "amount_cents",
      "currency",
      "quantity",
      "unit",
      "unit_rate_cents",
      "metadata_json",
      "idempotency_key",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  billing_usage_cursors: {
    columns: [
      "run_id",
      "meter",
      "user_id",
      "team_id",
      "last_billed_at",
      "total_billed_quantity",
      "updated_at",
    ],
    keyColumns: ["run_id", "meter"],
    orderColumn: "updated_at",
  },
  buyer_payment_challenges: {
    columns: [
      "id",
      "challenge_ref",
      "idempotency_key_hash",
      "actor_ref",
      "owner_user_id",
      "product_id",
      "surface",
      "method",
      "path",
      "request_body_digest",
      "price_asset",
      "price_denomination",
      "price_amount_minor_units",
      "spend_cap_asset",
      "spend_cap_denomination",
      "spend_cap_amount_minor_units",
      "status",
      "expires_at",
      "metadata_refs_json",
      "public_projection_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_credit_debits: {
    columns: [
      "id",
      "debit_ref",
      "idempotency_key_hash",
      "actor_ref",
      "owner_user_id",
      "product_id",
      "amount_asset",
      "amount_denomination",
      "amount_minor_units",
      "billing_ledger_entry_ref",
      "receipt_ref",
      "status",
      "metadata_refs_json",
      "public_projection_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_entitlements: {
    columns: [
      "id",
      "entitlement_ref",
      "challenge_ref",
      "receipt_ref",
      "actor_ref",
      "owner_user_id",
      "product_id",
      "surface",
      "scope_refs_json",
      "status",
      "expires_at",
      "created_at",
      "consumed_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_receipts: {
    columns: [
      "id",
      "receipt_ref",
      "challenge_ref",
      "actor_ref",
      "owner_user_id",
      "product_id",
      "surface",
      "amount_asset",
      "amount_denomination",
      "amount_minor_units",
      "entitlement_ref",
      "redacted_payment_ref",
      "status",
      "metadata_refs_json",
      "public_projection_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_reconciliation_events: {
    columns: [
      "id",
      "event_ref",
      "idempotency_key_hash",
      "provider_ref",
      "external_event_ref",
      "challenge_ref",
      "receipt_ref",
      "product_id",
      "status",
      "result_ref",
      "metadata_refs_json",
      "public_projection_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_redemptions: {
    columns: [
      "id",
      "redemption_ref",
      "idempotency_key_hash",
      "challenge_ref",
      "actor_ref",
      "proof_ref",
      "entitlement_ref",
      "receipt_ref",
      "status",
      "replayed",
      "metadata_refs_json",
      "public_projection_json",
      "created_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  buyer_payment_spend_limits: {
    columns: [
      "id",
      "spend_limit_ref",
      "actor_ref",
      "owner_user_id",
      "product_id",
      "scope_ref",
      "window_ref",
      "amount_asset",
      "amount_denomination",
      "amount_minor_units",
      "status",
      "metadata_refs_json",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  first_batch_payment_policies: {
    columns: [
      "id",
      "software_order_id",
      "assignment_id",
      "site_id",
      "policy_mode",
      "applied_by_user_id",
      "reason",
      "customer_safe_summary",
      "created_at",
      "updated_at",
      "archived_at",
    ],
    keyColumns: ["id"],
    orderColumn: "updated_at",
  },
  khala_code_paid_plan_payment_intents: {
    columns: [
      "purchase_ref",
      "account_ref",
      "idempotency_key",
      "rail",
      "status",
      "plan_id",
      "amount_cents",
      "amount_sats",
      "stripe_checkout_session_id",
      "stripe_checkout_url",
      "lightning_payment_hash",
      "lightning_invoice",
      "lightning_network",
      "lightning_invoice_expires_at",
      "entitlement_receipt_ref",
      "failure_reason",
      "created_at",
      "updated_at",
      "fulfilled_at",
    ],
    keyColumns: ["purchase_ref"],
    orderColumn: "created_at",
  },
  pay_in_legs: {
    columns: [
      "id",
      "pay_in_id",
      "direction",
      "kind",
      "party_ref",
      "amount_msat",
      "resulting_balance_msat",
      "external_ref",
      "refund_of_leg_id",
      "created_at",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  pay_ins: {
    columns: [
      "id",
      "pay_in_type",
      "payer_ref",
      "cost_msat",
      "state",
      "failure_reason",
      "rung",
      "context_ref",
      "idempotency_key",
      "genesis_id",
      "successor_id",
      "created_at",
      "state_changed_at",
      "public_receipt_ref",
    ],
    keyColumns: ["id"],
    orderColumn: "created_at",
  },
  stripe_checkout_sessions: {
    columns: [
      "session_id",
      "user_id",
      "package_id",
      "amount_cents",
      "currency",
      "payment_status",
      "fulfillment_status",
      "ledger_entry_id",
      "stripe_customer_id",
      "checkout_url",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["session_id"],
    orderColumn: "created_at",
  },
  stripe_customers: {
    columns: [
      "user_id",
      "currency",
      "stripe_customer_id",
      "livemode",
      "email_snapshot",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["user_id", "currency", "livemode"],
    orderColumn: "updated_at",
  },
  stripe_saved_payment_methods: {
    columns: [
      "user_id",
      "currency",
      "livemode",
      "stripe_customer_id",
      "stripe_payment_method_id",
      "setup_intent_id",
      "brand",
      "last4",
      "exp_month",
      "exp_year",
      "status",
      "created_at",
      "updated_at",
    ],
    keyColumns: ["user_id", "currency", "livemode"],
    orderColumn: "updated_at",
  },
  stripe_webhook_events: {
    columns: [
      "event_id",
      "type",
      "processing_status",
      "checkout_session_id",
      "received_at",
      "processed_at",
    ],
    keyColumns: ["event_id"],
    orderColumn: "received_at",
  },
}

/**
 * Normalize a D1/driver value into the canonical scalar used for both the
 * Postgres bind parameter and the reconciliation row hash. Identical to the
 * KS-8.2 normalization so the SAME logical row hashes identically whether it
 * came from a wrangler D1 JSON export or a postgres.js read.
 */
export const normalizeBillingValue = (
  value: unknown,
): string | number | null => {
  if (value === undefined || value === null) return null
  if (typeof value === "number") return value
  if (typeof value === "bigint") return Number(value)
  if (typeof value === "boolean") return value ? 1 : 0
  return String(value)
}
