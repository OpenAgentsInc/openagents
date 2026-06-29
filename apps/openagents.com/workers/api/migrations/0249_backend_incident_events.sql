-- 0249_backend_incident_events.sql (#6396)
--
-- Cloudflare-native backend incident sink for the trace-review triage loop.
-- Rows are public-safe summaries of failures observed by Worker catch paths,
-- Tail Worker / Logpush-style ingestion, queue consumers, Durable Objects, or
-- local Pylon crash reporters. Do not store raw stack traces, request bodies,
-- query strings, headers, prompts, provider payloads, credentials, or local
-- paths here.

CREATE TABLE IF NOT EXISTS backend_incident_events (
  id TEXT PRIMARY KEY,
  incident_ref TEXT NOT NULL UNIQUE,
  observed_at TEXT NOT NULL,
  source TEXT NOT NULL
    CHECK (source IN (
      'worker_fetch',
      'tail_worker',
      'workers_logs',
      'logpush',
      'queue_consumer',
      'durable_object',
      'pylon_local_runner'
    )),
  kind TEXT NOT NULL
    CHECK (kind IN (
      'unhandled_exception',
      'gateway_timeout',
      'silent_agent_crash'
    )),
  severity TEXT NOT NULL
    CHECK (severity IN ('warning', 'critical')),
  route_pattern TEXT NOT NULL DEFAULT 'unknown',
  method TEXT NOT NULL DEFAULT 'UNKNOWN',
  status_code INTEGER,
  error_name TEXT NOT NULL DEFAULT 'unknown',
  runtime_name TEXT NOT NULL DEFAULT 'cloudflare_workers',
  occurrence_count INTEGER NOT NULL DEFAULT 1,
  safe_metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_backend_incident_events_observed
  ON backend_incident_events(observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_backend_incident_events_kind_observed
  ON backend_incident_events(kind, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_backend_incident_events_source_observed
  ON backend_incident_events(source, observed_at DESC);
