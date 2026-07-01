-- Repair public Khala Tokens Served projections for usage rows that carry only
-- provider-reported total_tokens. Direct-local Khala Code / Codex events can be
-- exact but total-only; those rows were accepted into token_usage_events while
-- the public rollups used input_tokens + output_tokens only, so they contributed
-- zero to /stats. Recompute the public rollups from the canonical served-token
-- definition: split input+output when present, otherwise total_tokens.

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_tokens_total
  ON token_usage_events (observed_at, input_tokens, output_tokens, total_tokens);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_provider_model_total
  ON token_usage_events (
    observed_at,
    provider,
    model,
    input_tokens,
    output_tokens,
    total_tokens
  );

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_channel_total
  ON token_usage_events (
    observed_at,
    demand_channel,
    input_tokens,
    output_tokens,
    total_tokens
  );

DELETE FROM public_khala_tokens_served_daily_rollups;

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
  COALESCE(SUM(
    CASE
      WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
        THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
      ELSE COALESCE(total_tokens, 0)
    END
  ), 0) AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day
ON CONFLICT(timezone, day) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;

DELETE FROM public_khala_tokens_served_model_daily_rollups;

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
  COALESCE(SUM(
    CASE
      WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
        THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
      ELSE COALESCE(total_tokens, 0)
    END
  ), 0) AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day, provider, model
ON CONFLICT(day, provider, model) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;

DELETE FROM public_khala_tokens_served_channel_daily_rollups;

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
  COALESCE(SUM(
    CASE
      WHEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0) > 0
        THEN COALESCE(input_tokens, 0) + COALESCE(output_tokens, 0)
      ELSE COALESCE(total_tokens, 0)
    END
  ), 0) AS tokens_served,
  COUNT(*) AS usage_events,
  strftime('%Y-%m-%dT%H:%M:%fZ', 'now') AS updated_at
FROM token_usage_events
WHERE 1 = 1
GROUP BY day, demand_channel
ON CONFLICT(day, demand_channel) DO UPDATE SET
  tokens_served = excluded.tokens_served,
  usage_events = excluded.usage_events,
  updated_at = excluded.updated_at;
