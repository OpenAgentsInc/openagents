-- 0248_backend_exception_signals.sql (#6396)
--
-- Cloudflare-native backend failure signal sink for Tail Workers, Workers Logs /
-- Logpush transforms, Queue consumers, and explicit Worker catch paths. Rows are
-- public-safe classifications plus refs only; raw stack traces, request bodies,
-- provider payloads, and credentials do not belong here. The Khala trace-review
-- pipeline aggregates these rows into failure modes and backlog triage items.

CREATE TABLE IF NOT EXISTS backend_exception_signals (
  signal_ref TEXT PRIMARY KEY,
  observed_at TEXT NOT NULL,
  signal_kind TEXT NOT NULL CHECK (
    signal_kind IN (
      'unhandled_exception',
      'gateway_timeout',
      'agent_crash'
    )
  ),
  source TEXT NOT NULL CHECK (
    source IN (
      'worker_tail',
      'workers_logs',
      'logpush',
      'queue',
      'worker_catch'
    )
  ),
  surface TEXT NOT NULL,
  outcome TEXT NOT NULL,
  status_code INTEGER,
  trace_ref TEXT,
  request_ref TEXT,
  agent_ref TEXT,
  assignment_ref TEXT,
  error_class TEXT,
  sanitized_message TEXT,
  fingerprint TEXT NOT NULL,
  created_at TEXT NOT NULL,
  UNIQUE(fingerprint, observed_at)
);

CREATE INDEX IF NOT EXISTS idx_backend_exception_signals_observed
  ON backend_exception_signals(observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_backend_exception_signals_kind_observed
  ON backend_exception_signals(signal_kind, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_backend_exception_signals_surface_observed
  ON backend_exception_signals(surface, observed_at DESC);

