-- Public /stats mix panels should read compact daily rollups instead of
-- rescanning the raw token ledger on every page load. The Worker maintains
-- these tables on each successful token_usage_events insert; this migration
-- backfills existing rows.

CREATE TABLE IF NOT EXISTS public_khala_tokens_served_model_daily_rollups (
  day TEXT NOT NULL,
  provider TEXT NOT NULL DEFAULT '',
  model TEXT NOT NULL DEFAULT '',
  tokens_served INTEGER NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events INTEGER NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (day, provider, model)
);

CREATE TABLE IF NOT EXISTS public_khala_tokens_served_channel_daily_rollups (
  day TEXT NOT NULL,
  demand_channel TEXT NOT NULL DEFAULT 'khala_api',
  tokens_served INTEGER NOT NULL DEFAULT 0 CHECK (tokens_served >= 0),
  usage_events INTEGER NOT NULL DEFAULT 0 CHECK (usage_events >= 0),
  updated_at TEXT NOT NULL,
  PRIMARY KEY (day, demand_channel)
);

INSERT INTO public_khala_tokens_served_model_daily_rollups (
  day,
  provider,
  model,
  tokens_served,
  usage_events,
  updated_at
)
SELECT
  date(observed_at) AS day,
  COALESCE(provider, '') AS provider,
  COALESCE(model, '') AS model,
  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
    AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day, provider, model
ON CONFLICT(day, provider, model) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;

INSERT INTO public_khala_tokens_served_channel_daily_rollups (
  day,
  demand_channel,
  tokens_served,
  usage_events,
  updated_at
)
SELECT
  date(observed_at) AS day,
  CASE
    WHEN lower(COALESCE(demand_channel, '')) = 'direct_local'
      THEN 'direct_local'
    ELSE 'khala_api'
  END AS demand_channel,
  COALESCE(SUM(input_tokens), 0) + COALESCE(SUM(output_tokens), 0)
    AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day, demand_channel
ON CONFLICT(day, demand_channel) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;
