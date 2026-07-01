-- Public /stats history should read daily buckets, not rebuild them from the
-- raw token ledger on every page load. The Worker maintains this table on each
-- successful token_usage_events insert; this migration backfills existing rows.

CREATE TABLE IF NOT EXISTS public_khala_tokens_served_daily_rollups (
  timezone TEXT NOT NULL,
  day TEXT NOT NULL,
  tokens_served INTEGER NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events INTEGER NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (timezone, day)
);

INSERT INTO public_khala_tokens_served_daily_rollups (
  timezone,
  day,
  tokens_served,
  usage_events,
  updated_at
)
SELECT
  'America/Chicago' AS timezone,
  date(observed_at, '-5 hours') AS day,
  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
    AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day
ON CONFLICT(timezone, day) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;
