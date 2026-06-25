ALTER TABLE token_usage_events
  ADD COLUMN demand_kind TEXT NOT NULL DEFAULT 'unlabeled';

ALTER TABLE token_usage_events
  ADD COLUMN demand_source TEXT;

ALTER TABLE token_usage_events
  ADD COLUMN demand_client TEXT;

UPDATE token_usage_events
   SET demand_kind = CASE
       WHEN lower(COALESCE(json_extract(safe_metadata_json, '$.demandKind'), 'unlabeled')) = 'internal'
         THEN 'internal'
       WHEN lower(COALESCE(json_extract(safe_metadata_json, '$.demandKind'), 'unlabeled')) = 'external'
         THEN 'external'
       ELSE 'unlabeled'
     END,
     demand_source = CASE
       WHEN json_type(safe_metadata_json, '$.demandSource') = 'text'
         THEN json_extract(safe_metadata_json, '$.demandSource')
       ELSE NULL
     END,
     demand_client = CASE
       WHEN json_type(safe_metadata_json, '$.demandClient') = 'text'
         THEN json_extract(safe_metadata_json, '$.demandClient')
       ELSE NULL
     END;

CREATE INDEX IF NOT EXISTS idx_token_usage_events_demand_kind
  ON token_usage_events (demand_kind, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_demand_source
  ON token_usage_events (demand_kind, demand_source, observed_at);

CREATE INDEX IF NOT EXISTS idx_token_usage_events_demand_client
  ON token_usage_events (demand_kind, demand_client, observed_at);
