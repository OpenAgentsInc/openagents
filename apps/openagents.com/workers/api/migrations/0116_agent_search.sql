CREATE TABLE IF NOT EXISTS agent_search_requests (
  id TEXT PRIMARY KEY,
  receipt_ref TEXT NOT NULL UNIQUE,
  actor_ref TEXT NOT NULL,
  agent_user_id TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  idempotency_key_hash TEXT NOT NULL UNIQUE,
  request_body_digest TEXT NOT NULL,
  query_hash TEXT NOT NULL,
  query_text TEXT,
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  provider TEXT NOT NULL CHECK (provider IN ('exa')),
  provider_request_id TEXT,
  status TEXT NOT NULL CHECK (status IN ('succeeded', 'failed')),
  cache_status TEXT NOT NULL CHECK (cache_status IN ('hit', 'miss')),
  charge_state TEXT NOT NULL CHECK (charge_state IN ('free_allowance')),
  product_id TEXT,
  entitlement_ref TEXT,
  provider_cost_dollars REAL,
  public_projection_json TEXT NOT NULL,
  created_at TEXT NOT NULL,
  completed_at TEXT,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS agent_search_requests_actor_created_idx
  ON agent_search_requests(actor_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS agent_search_requests_credential_created_idx
  ON agent_search_requests(credential_id, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_sources (
  id TEXT PRIMARY KEY,
  search_request_id TEXT NOT NULL REFERENCES agent_search_requests(id) ON DELETE CASCADE,
  source_ref TEXT NOT NULL UNIQUE,
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  domain TEXT NOT NULL,
  published_date TEXT,
  score REAL,
  highlight_text TEXT,
  selected_text_hash TEXT,
  public_safe INTEGER NOT NULL DEFAULT 1 CHECK (public_safe IN (0, 1)),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_search_sources_request_idx
  ON agent_search_sources(search_request_id);

CREATE TABLE IF NOT EXISTS agent_search_quota_events (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  credential_id TEXT NOT NULL,
  event_kind TEXT NOT NULL CHECK (event_kind IN ('search_request', 'provider_request')),
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  units INTEGER NOT NULL CHECK (units > 0),
  product_id TEXT,
  entitlement_ref TEXT,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_search_quota_actor_kind_created_idx
  ON agent_search_quota_events(actor_ref, event_kind, created_at DESC);

CREATE INDEX IF NOT EXISTS agent_search_quota_credential_kind_created_idx
  ON agent_search_quota_events(credential_id, event_kind, created_at DESC);

CREATE TABLE IF NOT EXISTS agent_search_cache_entries (
  id TEXT PRIMARY KEY,
  cache_key TEXT NOT NULL,
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  provider TEXT NOT NULL CHECK (provider IN ('exa')),
  results_json TEXT NOT NULL CHECK (length(results_json) <= 12000),
  result_count INTEGER NOT NULL DEFAULT 0 CHECK (result_count >= 0),
  cost_dollars REAL,
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_search_cache_key_active_idx
  ON agent_search_cache_entries(cache_key)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS agent_search_cache_fresh_idx
  ON agent_search_cache_entries(cache_key, expires_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS agent_search_metric_events (
  id TEXT PRIMARY KEY,
  actor_ref TEXT NOT NULL,
  credential_id TEXT,
  event_name TEXT NOT NULL,
  mode TEXT NOT NULL CHECK (mode IN ('basic')),
  cache_status TEXT CHECK (cache_status IS NULL OR cache_status IN ('hit', 'miss')),
  provider_status TEXT,
  provider_cost_dollars REAL,
  duration_ms INTEGER CHECK (duration_ms IS NULL OR duration_ms >= 0),
  result_count INTEGER CHECK (result_count IS NULL OR result_count >= 0),
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_search_metric_actor_created_idx
  ON agent_search_metric_events(actor_ref, created_at DESC);

CREATE INDEX IF NOT EXISTS agent_search_metric_event_created_idx
  ON agent_search_metric_events(event_name, created_at DESC);
