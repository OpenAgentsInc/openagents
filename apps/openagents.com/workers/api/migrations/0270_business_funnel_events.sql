-- Business funnel stage receipt ledger (BF-1.4 / issue #8077).
--
-- This table records aggregate-countable stage receipts for the /business
-- engine. Rows intentionally carry opaque refs and coarse source attribution
-- only; they do not store contact details, user ids, payment payloads, raw
-- provider data, or per-user journey state.

CREATE TABLE IF NOT EXISTS business_funnel_events (
  id TEXT PRIMARY KEY NOT NULL,
  event_ref TEXT NOT NULL UNIQUE,
  stage TEXT NOT NULL CHECK (
    stage IN (
      'visit',
      'signup',
      'intake_spec',
      'payment',
      'provisioned',
      'first_outcome',
      'retained'
    )
  ),
  source_kind TEXT NOT NULL CHECK (
    source_kind IN (
      'content',
      'outbound',
      'ai_search',
      'referral',
      'direct',
      'unknown'
    )
  ),
  source_ref TEXT,
  occurred_at TEXT NOT NULL,
  observed_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS business_funnel_events_stage_time_idx
  ON business_funnel_events(stage, occurred_at DESC);

CREATE INDEX IF NOT EXISTS business_funnel_events_source_time_idx
  ON business_funnel_events(source_kind, occurred_at DESC);

