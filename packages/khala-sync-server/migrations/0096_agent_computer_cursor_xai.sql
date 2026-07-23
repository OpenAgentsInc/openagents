-- #9193: document the platform-secret provider principals in Postgres.
-- Existing provider columns are unconstrained text. These comments make the
-- runtime contract explicit without changing stored credential material.
COMMENT ON COLUMN provider_accounts.provider IS
  'Provider identifier, including cursor and xai_grok Agent Computer principals.';
COMMENT ON COLUMN provider_account_auth_grants.provider IS
  'Provider identifier for one owner-scoped, one-use runtime grant.';
COMMENT ON COLUMN provider_account_leases.provider IS
  'Provider identifier for managed runtime capacity.';
