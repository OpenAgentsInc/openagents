-- SARAH-AUTONOMOUS-1: durable per-interval claim ledger for Sarah's scheduled
-- autonomous tick. Today Sarah only acts when the owner messages her thread
-- (owner-triggered hosted dispatch). The autonomous tick fires on the existing
-- per-minute cron drive, but must run AT MOST ONCE per owner per interval even
-- across concurrent Cloud Run instances. Each tick derives a deterministic
-- `tick_ref` from the opaque owner thread ref plus the interval bucket and
-- claims it with `INSERT ... ON CONFLICT (tick_ref) DO NOTHING RETURNING`: the
-- winner runs the tick, every other instance for that same bucket finds the row
-- already present and cleanly no-ops. No raw owner id ever enters `tick_ref`.
--
-- The row also records the tick's own outcome and the authority receipt ref so
-- the autonomous trigger has an audit trail alongside the tools' own target
-- receipts. This table is bookkeeping only: it grants no authority and gates
-- nothing but the interval cadence.

CREATE TABLE IF NOT EXISTS sarah_autonomous_tick_runs (
  tick_ref        text PRIMARY KEY,
  owner_user_id   text NOT NULL,
  thread_ref      text NOT NULL,
  interval_bucket bigint NOT NULL,
  started_at      text NOT NULL,
  outcome         text,
  receipt_ref     text,
  settled_at      text,
  CONSTRAINT sarah_autonomous_tick_runs_tick_ref_shape
    CHECK (tick_ref ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{0,255}$'),
  CONSTRAINT sarah_autonomous_tick_runs_owner_shape
    CHECK (owner_user_id ~ '^[A-Za-z0-9][A-Za-z0-9._:-]{2,159}$'),
  CONSTRAINT sarah_autonomous_tick_runs_thread_shape
    CHECK (thread_ref ~ '^thread\.sarah\.[0-9a-f]{24}$'),
  CONSTRAINT sarah_autonomous_tick_runs_bucket_nonneg
    CHECK (interval_bucket >= 0)
);

CREATE UNIQUE INDEX IF NOT EXISTS sarah_autonomous_tick_runs_owner_bucket_idx
  ON sarah_autonomous_tick_runs(owner_user_id, interval_bucket);

CREATE INDEX IF NOT EXISTS sarah_autonomous_tick_runs_owner_started_idx
  ON sarah_autonomous_tick_runs(owner_user_id, started_at DESC);
