-- Public /stats aggregate reads filter by observed_at first, then group by
-- provider/model, channel, or day. The original indexes
-- used the group keys first, which leaves the hot 30d graph reads too exposed
-- to table scans while the token ledger is under write pressure.

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_tokens
  ON token_usage_events (observed_at, input_tokens, output_tokens);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_provider_model
  ON token_usage_events (
    observed_at,
    provider,
    model,
    input_tokens,
    output_tokens
  );

CREATE INDEX IF NOT EXISTS idx_token_usage_events_public_observed_channel
  ON token_usage_events (
    observed_at,
    demand_channel,
    input_tokens,
    output_tokens
  );
