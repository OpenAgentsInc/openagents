ALTER TABLE token_usage_events
  ADD COLUMN demand_channel TEXT NOT NULL DEFAULT 'khala_api'
  CHECK (demand_channel IN ('khala_api', 'direct_local'));

UPDATE token_usage_events
   SET demand_channel = 'khala_api'
 WHERE demand_channel IS NULL OR demand_channel = '';

CREATE INDEX IF NOT EXISTS idx_token_usage_events_demand_channel
  ON token_usage_events (demand_channel, observed_at);
