-- oa-infra KvStore backend (CFG-2, issue #8517).
--
-- Single-table key/value store with optional TTL. Expiry is LAZY: an expired
-- row is treated as absent and physically deleted on the read path
-- (kv-store-postgres.ts); there is no background sweeper requirement. A
-- partial index on expires_at keeps opportunistic sweeps cheap if a backend
-- operator ever wants to run `DELETE ... WHERE expires_at <= now()`.

CREATE TABLE IF NOT EXISTS oa_infra_kv (
  key        text        PRIMARY KEY,
  value      text        NOT NULL,
  expires_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS oa_infra_kv_expires_at_idx
  ON oa_infra_kv (expires_at)
  WHERE expires_at IS NOT NULL;
