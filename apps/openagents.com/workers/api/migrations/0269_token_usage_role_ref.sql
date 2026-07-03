ALTER TABLE token_usage_events
  ADD COLUMN role_ref TEXT;

CREATE INDEX IF NOT EXISTS idx_token_usage_events_role_ref
  ON token_usage_events (role_ref, observed_at);
