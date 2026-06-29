CREATE TABLE IF NOT EXISTS exa_enrichment_budget_events (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  run_id TEXT REFERENCES exa_enrichment_runs(id) ON DELETE SET NULL,
  day_key TEXT NOT NULL CHECK (length(day_key) = 10),
  request_units INTEGER NOT NULL CHECK (request_units > 0),
  reason TEXT NOT NULL CHECK (length(reason) > 0 AND length(reason) <= 120),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS exa_enrichment_budget_assignment_day_idx
  ON exa_enrichment_budget_events(assignment_id, day_key, created_at DESC);

CREATE INDEX IF NOT EXISTS exa_enrichment_budget_day_idx
  ON exa_enrichment_budget_events(day_key, created_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_cache_entries (
  id TEXT PRIMARY KEY NOT NULL,
  cache_key TEXT NOT NULL CHECK (length(cache_key) > 0),
  source_category TEXT NOT NULL CHECK (length(source_category) > 0),
  search_type TEXT NOT NULL CHECK (length(search_type) > 0),
  freshness_max_age_hours INTEGER NOT NULL CHECK (freshness_max_age_hours >= 0),
  results_json TEXT NOT NULL CHECK (length(results_json) <= 12000),
  result_count INTEGER NOT NULL DEFAULT 0 CHECK (result_count >= 0),
  cost_dollars REAL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS exa_enrichment_cache_key_idx
  ON exa_enrichment_cache_entries(cache_key)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS exa_enrichment_cache_fresh_idx
  ON exa_enrichment_cache_entries(cache_key, expires_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS exa_enrichment_metric_events (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  run_id TEXT REFERENCES exa_enrichment_runs(id) ON DELETE SET NULL,
  query_id TEXT REFERENCES exa_enrichment_queries(id) ON DELETE SET NULL,
  event_name TEXT NOT NULL CHECK (length(event_name) > 0 AND length(event_name) <= 160),
  status TEXT NOT NULL CHECK (length(status) > 0 AND length(status) <= 80),
  error_code TEXT CHECK (error_code IS NULL OR length(error_code) <= 160),
  search_type TEXT CHECK (search_type IS NULL OR length(search_type) <= 80),
  source_category TEXT CHECK (source_category IS NULL OR length(source_category) <= 80),
  result_count INTEGER CHECK (result_count IS NULL OR result_count >= 0),
  source_card_count INTEGER CHECK (source_card_count IS NULL OR source_card_count >= 0),
  latency_ms INTEGER CHECK (latency_ms IS NULL OR latency_ms >= 0),
  cost_dollars REAL,
  cache_status TEXT CHECK (
    cache_status IS NULL OR cache_status IN ('hit', 'miss', 'stale', 'bypass')
  ),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS exa_enrichment_metric_assignment_created_idx
  ON exa_enrichment_metric_events(assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS exa_enrichment_metric_event_created_idx
  ON exa_enrichment_metric_events(event_name, created_at DESC);
