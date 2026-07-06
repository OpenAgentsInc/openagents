-- Khala Sync per-user credit-balance projections (issue #8505, Part 2).
-- Mirrors khala_sync_public_counters (0006_khala_sync_public_counters.sql)
-- EXACTLY, keyed by user_id instead of a global counter_id.
--
-- AUTHORITY STAYS ON D1 (see docs/khala-code/2026-07-06-credits-ledger-vs-khala-sync-architecture-audit.md):
-- this table is a projection of the D1 `agent_balances` ledger, never a
-- second source of truth. The D1 write (grant, charge, clawback) always
-- happens first and always wins; this row only carries a live-synced COPY
-- of the resulting balance into scope.user.<userId> so the mobile balance
-- chip can update without a REST poll.
--
-- khala_sync_user_credit_balances: one row per user with a synced balance.
-- `balance_usd_cents` mirrors D1 agent_balances.balance_msat, converted at
-- the SAME shared BTC/USD rate every other credits surface uses
-- (usd-msat-conversion.ts's msatToUsdCentsRound), so the projected cents
-- value here is always the authoritative D1 msat balance re-expressed, never
-- independently computed.
--
-- BRING-UP ORDER: the row is created ONLY by the admin backfill action
-- (projection = exact current D1 balance at that instant), never by the
-- increment path — an increment against a missing row is refused (and its
-- idempotency guard rolled back), so a fresh deploy can never serve a
-- fabricated zero balance in place of the user's real one.
--
-- khala_sync_user_credit_balance_applied: exact-once guard. One row per
-- applied source ledger event (its idempotency key, reused verbatim from the
-- D1 write that already carries one — inference:charge:<requestId>,
-- <primitive>:charge:<chargeId>, signup:github:<githubUserId>, an admin
-- grant/clawback ref, etc.); the guard insert shares the increment
-- transaction, so a replayed event (ON CONFLICT DO NOTHING) skips the
-- increment.
--
-- khala_sync_user_credit_balance_repairs: audit trail for backfill/repair. A
-- repair NEVER happens silently: every projection overwrite records the
-- previous balance, the exact new balance, its source, and an audit note.

CREATE TABLE IF NOT EXISTS khala_sync_user_credit_balances (
  user_id            text        PRIMARY KEY,
  balance_usd_cents  bigint      NOT NULL DEFAULT 0
    CHECK (balance_usd_cents >= 0),
  last_event_at      timestamptz,
  updated_at         timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT khala_sync_user_credit_balances_user_id_nonempty
    CHECK (length(user_id) > 0)
);

CREATE TABLE IF NOT EXISTS khala_sync_user_credit_balance_applied (
  user_id         text        NOT NULL,
  idempotency_key text        NOT NULL,
  applied_at      timestamptz NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, idempotency_key),
  CONSTRAINT khala_sync_user_credit_balance_applied_key_nonempty
    CHECK (length(idempotency_key) > 0)
);

CREATE TABLE IF NOT EXISTS khala_sync_user_credit_balance_repairs (
  id               bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  user_id          text        NOT NULL,
  previous_balance bigint,
  new_balance      bigint      NOT NULL
    CHECK (new_balance >= 0),
  source           text        NOT NULL
    CHECK (source IN ('backfill', 'reconcile_repair')),
  audit_note       text        NOT NULL
    CHECK (length(audit_note) > 0),
  created_at       timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS khala_sync_user_credit_balance_repairs_user_idx
  ON khala_sync_user_credit_balance_repairs (user_id, created_at);
