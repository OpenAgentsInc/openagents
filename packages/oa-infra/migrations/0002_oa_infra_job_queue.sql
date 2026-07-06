-- oa-infra JobQueue backend (CFG-2, issue #8517).
--
-- Postgres-backed job queue leased with FOR UPDATE SKIP LOCKED
-- (job-queue-postgres.ts). Lifecycle:
--   pending --lease--> leased --ack--> deleted
--                        |--nack (attempts < max_attempts)--> pending (delayed)
--                        |--nack (attempts >= max_attempts)--> dead
--                        `--lease expiry--> re-leasable (or dead when exhausted)
-- `attempts` counts DELIVERIES (incremented on lease), so an expired lease
-- consumes an attempt exactly like an explicit nack.

CREATE TABLE IF NOT EXISTS oa_infra_jobs (
  id               uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
  topic            text        NOT NULL,
  payload          text        NOT NULL,
  status           text        NOT NULL DEFAULT 'pending'
                               CHECK (status IN ('pending', 'leased', 'dead')),
  attempts         integer     NOT NULL DEFAULT 0,
  max_attempts     integer     NOT NULL DEFAULT 5,
  run_at           timestamptz NOT NULL DEFAULT now(),
  lease_expires_at timestamptz,
  last_error       text,
  created_at       timestamptz NOT NULL DEFAULT now(),
  dead_at          timestamptz
);

CREATE INDEX IF NOT EXISTS oa_infra_jobs_pending_idx
  ON oa_infra_jobs (topic, run_at)
  WHERE status = 'pending';

CREATE INDEX IF NOT EXISTS oa_infra_jobs_leased_idx
  ON oa_infra_jobs (topic, lease_expires_at)
  WHERE status = 'leased';

CREATE INDEX IF NOT EXISTS oa_infra_jobs_dead_idx
  ON oa_infra_jobs (topic, dead_at)
  WHERE status = 'dead';
