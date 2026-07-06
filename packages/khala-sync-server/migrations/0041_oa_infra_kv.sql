-- CFG-3 (issue #8518): AUTH_STORAGE Cloudflare KV evacuation target.
--
-- The owned KvStore table (packages/oa-infra migrations 0001 + 0004 define
-- the same objects for standalone oa-infra databases; both runners use
-- IF NOT EXISTS so either order is safe on a shared database). This is the
-- Postgres home for everything the Worker kept in the AUTH_STORAGE KV
-- namespace — OpenAuth issuer state (sessions/refresh/PKCE codes via the
-- `StorageAdapter` in apps/openagents.com/workers/api/src/auth/
-- openauth-storage.ts), mobile access-token revocation markers, account
-- deletion receipts, GitHub identity/write tokens, provider-account device
-- login state, and desktop/pylon auth attempts.
--
-- Semantics are owned by @openagentsinc/oa-infra's KvStore contract
-- (src/conformance/kv-store.ts): lazy expiry on read, literal-prefix
-- listPrefix scans (LIKE with escaped metacharacters — hence the
-- text_pattern_ops index; Cloud SQL databases use a non-C collation, so the
-- PK btree cannot serve LIKE prefix scans).

CREATE TABLE IF NOT EXISTS oa_infra_kv (
  key        text        PRIMARY KEY,
  value      text        NOT NULL,
  expires_at timestamptz,
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS oa_infra_kv_expires_at_idx
  ON oa_infra_kv (expires_at)
  WHERE expires_at IS NOT NULL;

CREATE INDEX IF NOT EXISTS oa_infra_kv_key_prefix_idx
  ON oa_infra_kv (key text_pattern_ops);
