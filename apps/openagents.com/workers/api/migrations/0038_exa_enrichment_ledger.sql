CREATE TABLE IF NOT EXISTS exa_enrichment_runs (
  id TEXT PRIMARY KEY NOT NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  plan_id TEXT NOT NULL CHECK (length(plan_id) > 0),
  subject TEXT NOT NULL CHECK (length(subject) > 0 AND length(subject) <= 500),
  status TEXT NOT NULL CHECK (
    status IN (
      'planned',
      'queued',
      'running',
      'succeeded',
      'partial_failure',
      'failed',
      'needs_review',
      'approved',
      'rejected',
      'stale'
    )
  ),
  request_budget INTEGER NOT NULL DEFAULT 0 CHECK (request_budget >= 0),
  request_count INTEGER NOT NULL DEFAULT 0 CHECK (request_count >= 0),
  cache_hit_count INTEGER NOT NULL DEFAULT 0 CHECK (cache_hit_count >= 0),
  source_count INTEGER NOT NULL DEFAULT 0 CHECK (source_count >= 0),
  approved_source_count INTEGER NOT NULL DEFAULT 0 CHECK (approved_source_count >= 0),
  cost_dollars REAL,
  error_code TEXT,
  error_summary TEXT CHECK (error_summary IS NULL OR length(error_summary) <= 500),
  started_at TEXT,
  completed_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  archived_at TEXT
);

CREATE INDEX IF NOT EXISTS exa_enrichment_runs_assignment_created_idx
  ON exa_enrichment_runs(assignment_id, created_at DESC)
  WHERE archived_at IS NULL;

CREATE INDEX IF NOT EXISTS exa_enrichment_runs_status_updated_idx
  ON exa_enrichment_runs(status, updated_at DESC)
  WHERE archived_at IS NULL;

CREATE TABLE IF NOT EXISTS exa_enrichment_queries (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL REFERENCES exa_enrichment_runs(id) ON DELETE CASCADE,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  query_hash TEXT NOT NULL CHECK (length(query_hash) > 0),
  query_text TEXT NOT NULL CHECK (length(query_text) > 0 AND length(query_text) <= 500),
  source_category TEXT NOT NULL CHECK (length(source_category) > 0),
  search_type TEXT NOT NULL CHECK (length(search_type) > 0),
  freshness_max_age_hours INTEGER NOT NULL CHECK (freshness_max_age_hours >= 0),
  status TEXT NOT NULL CHECK (
    status IN ('planned', 'running', 'succeeded', 'failed', 'cached')
  ),
  result_count INTEGER NOT NULL DEFAULT 0 CHECK (result_count >= 0),
  latency_ms INTEGER CHECK (latency_ms IS NULL OR latency_ms >= 0),
  cost_dollars REAL,
  error_code TEXT,
  error_summary TEXT CHECK (error_summary IS NULL OR length(error_summary) <= 500),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS exa_enrichment_queries_run_created_idx
  ON exa_enrichment_queries(run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS exa_enrichment_queries_hash_created_idx
  ON exa_enrichment_queries(query_hash, created_at DESC);

CREATE TABLE IF NOT EXISTS exa_enrichment_sources (
  id TEXT PRIMARY KEY NOT NULL,
  run_id TEXT NOT NULL REFERENCES exa_enrichment_runs(id) ON DELETE CASCADE,
  query_id TEXT REFERENCES exa_enrichment_queries(id) ON DELETE SET NULL,
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  software_order_id TEXT REFERENCES software_orders(id) ON DELETE SET NULL,
  site_id TEXT REFERENCES site_projects(id) ON DELETE SET NULL,
  source_category TEXT NOT NULL CHECK (length(source_category) > 0),
  review_status TEXT NOT NULL CHECK (
    review_status IN (
      'proposed',
      'approved',
      'rejected',
      'internal_only',
      'public_safe'
    )
  ),
  title TEXT NOT NULL CHECK (length(title) > 0 AND length(title) <= 240),
  url TEXT NOT NULL CHECK (length(url) > 0 AND length(url) <= 2048),
  domain TEXT NOT NULL CHECK (length(domain) > 0 AND length(domain) <= 255),
  published_date TEXT,
  highlight_text TEXT CHECK (highlight_text IS NULL OR length(highlight_text) <= 1200),
  selected_text_hash TEXT,
  exa_request_id TEXT,
  search_type TEXT,
  public_safe INTEGER NOT NULL DEFAULT 0 CHECK (public_safe IN (0, 1)),
  rejected_reason TEXT CHECK (rejected_reason IS NULL OR length(rejected_reason) <= 500),
  approved_at TEXT,
  rejected_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS exa_enrichment_sources_assignment_created_idx
  ON exa_enrichment_sources(assignment_id, created_at DESC);

CREATE INDEX IF NOT EXISTS exa_enrichment_sources_run_created_idx
  ON exa_enrichment_sources(run_id, created_at DESC);

CREATE INDEX IF NOT EXISTS exa_enrichment_sources_public_safe_idx
  ON exa_enrichment_sources(assignment_id, public_safe, review_status, created_at DESC);

CREATE TABLE IF NOT EXISTS adjutant_assignment_enrichments (
  assignment_id TEXT NOT NULL REFERENCES adjutant_assignments(id) ON DELETE CASCADE,
  enrichment_run_id TEXT NOT NULL REFERENCES exa_enrichment_runs(id) ON DELETE CASCADE,
  research_brief_id TEXT,
  status TEXT NOT NULL CHECK (
    status IN (
      'planned',
      'running',
      'needs_review',
      'approved',
      'rejected',
      'stale',
      'failed'
    )
  ),
  required_for_launch INTEGER NOT NULL DEFAULT 0 CHECK (required_for_launch IN (0, 1)),
  approved_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (assignment_id, enrichment_run_id)
);

CREATE INDEX IF NOT EXISTS adjutant_assignment_enrichments_assignment_updated_idx
  ON adjutant_assignment_enrichments(assignment_id, updated_at DESC);
