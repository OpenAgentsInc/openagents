-- BF-8.4 honest referral attribution loop.
--
-- Extend the public-safe business funnel event ledger with the multiply-stage
-- receipt for a referred engagement. The table still stores only opaque refs
-- and coarse source buckets; payout/settlement truth remains in the existing
-- site_referral_payout_ledger_entries table.

CREATE TABLE IF NOT EXISTS business_funnel_events_0275 (
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
      'retained',
      'referred_engagement'
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

INSERT OR IGNORE INTO business_funnel_events_0275 (
  id,
  event_ref,
  stage,
  source_kind,
  source_ref,
  occurred_at,
  observed_at
)
SELECT
  id,
  event_ref,
  stage,
  source_kind,
  source_ref,
  occurred_at,
  observed_at
FROM business_funnel_events;

DROP TABLE business_funnel_events;

ALTER TABLE business_funnel_events_0275
  RENAME TO business_funnel_events;

CREATE INDEX IF NOT EXISTS business_funnel_events_stage_time_idx
  ON business_funnel_events(stage, occurred_at DESC);

CREATE INDEX IF NOT EXISTS business_funnel_events_source_time_idx
  ON business_funnel_events(source_kind, occurred_at DESC);
