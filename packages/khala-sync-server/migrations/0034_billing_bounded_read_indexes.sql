-- KS-8.7 follow-up (#8337): the billing domain's bounded Postgres read
-- serving (KHALA_SYNC_BILLING_READS=postgres serves ONLY the four
-- allowlisted display surfaces named in billing-store.ts's
-- BILLING_DOMAIN_POSTGRES_SERVED_READ_TABLES) re-derives the D1 read
-- accelerators for the exact live read patterns it now actually answers
-- (the KS-8.2 re-derivation rule — same discipline as 0033's business
-- funnel dashboard index).
--
-- 0015_billing_pay_ins.sql intentionally dropped every D1 index beyond the
-- balance-SUM accelerator (billing_ledger_entries_user_created_idx) because
-- this lane routed exactly one read (the balance) to Postgres. This
-- migration re-derives TWO more accelerators for the two newly-served
-- surfaces that need one (the other two — billing_auto_top_up_policies via
-- its PK (user_id, currency), stripe_saved_payment_methods via its PK
-- (user_id, currency, livemode), and stripe_checkout_sessions via its PK
-- session_id / billing_ledger_entries via its UNIQUE idempotency_key — all
-- already hit an existing PK/UNIQUE index and need nothing new):
--
--   1. billing_auto_top_up_events had NO user_id index at all. The
--      auto-top-up display-state read (`readBillingAutoTopUpState`,
--      billing.ts / billing-store.ts's `readAutoTopUpStateRows`) runs
--      `WHERE user_id = ? ORDER BY created_at DESC LIMIT 6`.
--
--   2. pay_ins had its `_public_receipt_ref` index dropped by 0015 (no
--      Postgres read used it at the time). The inference/pay-in receipt
--      display read (`inference-receipts.ts`'s
--      `readInferenceReceiptByRef` / `listRecentInferenceReceipts`) runs
--      `WHERE public_receipt_ref = ?` and
--      `WHERE public_receipt_ref LIKE 'receipt.inference.charge.%' AND
--      pay_in_type = 'adjustment' AND state = 'paid' ORDER BY created_at
--      DESC LIMIT ?` — a plain btree index on public_receipt_ref serves the
--      exact-match lookup and the LIKE-prefix scan; the listing query's
--      remaining filter (pay_in_type, state) plus its ORDER BY gets its own
--      composite covering index.
--
-- No other billing-domain table gains an index here: widening either read
-- surface (or adding a new one) is a separate, individually reviewed
-- follow-up per this lane's money discipline.

CREATE INDEX IF NOT EXISTS billing_auto_top_up_events_user_created_idx
  ON billing_auto_top_up_events (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS pay_ins_public_receipt_ref_idx
  ON pay_ins (public_receipt_ref);

CREATE INDEX IF NOT EXISTS pay_ins_receipt_listing_covering_idx
  ON pay_ins (pay_in_type, state, created_at DESC);
