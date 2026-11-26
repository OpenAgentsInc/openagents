-- Coax Web zero-infra Postgres queue, cache, and atomic lease locks (Issue #14079).
-- Mirrors Laravel 13's database-first defaults (jobs, job_batches, failed_jobs,
-- cache, cache_locks) using Postgres FOR UPDATE SKIP LOCKED.

CREATE SCHEMA IF NOT EXISTS queue;
CREATE SCHEMA IF NOT EXISTS sys;

CREATE TABLE IF NOT EXISTS queue.jobs (
  id              BIGSERIAL PRIMARY KEY,
  queue           TEXT NOT NULL DEFAULT 'default',
  kind            TEXT NOT NULL,
  payload         JSONB NOT NULL,
  idempotency_key TEXT UNIQUE,
  attempts        SMALLINT NOT NULL DEFAULT 0,
  max_attempts    SMALLINT NOT NULL DEFAULT 3,
  reserved_at     TIMESTAMPTZ,
  reserved_by     TEXT,
  available_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS queue_jobs_claim_idx
  ON queue.jobs (queue, available_at, id)
  WHERE reserved_at IS NULL;

CREATE TABLE IF NOT EXISTS queue.job_batches (
  id              TEXT PRIMARY KEY,
  name            TEXT NOT NULL,
  total_jobs      INTEGER NOT NULL DEFAULT 0,
  pending_jobs    INTEGER NOT NULL DEFAULT 0,
  failed_jobs     INTEGER NOT NULL DEFAULT 0,
  failed_job_ids  JSONB NOT NULL DEFAULT '[]'::jsonb,
  cancelled_at    TIMESTAMPTZ,
  created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
  finished_at     TIMESTAMPTZ
);

CREATE TABLE IF NOT EXISTS queue.failed_jobs (
  id              BIGSERIAL PRIMARY KEY,
  queue           TEXT NOT NULL,
  kind            TEXT NOT NULL,
  payload         JSONB NOT NULL,
  error_text      TEXT NOT NULL,
  failed_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS queue_failed_jobs_idx
  ON queue.failed_jobs (queue, failed_at DESC);

CREATE TABLE IF NOT EXISTS sys.cache (
  key             TEXT PRIMARY KEY,
  value           BYTEA NOT NULL,
  created_ms      BIGINT NOT NULL,
  expires_ms      BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS sys_cache_expires_idx
  ON sys.cache (expires_ms);

CREATE TABLE IF NOT EXISTS sys.locks (
  key             TEXT PRIMARY KEY,
  owner           TEXT NOT NULL,
  expires_ms      BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS sys_locks_expires_idx
  ON sys.locks (expires_ms);
