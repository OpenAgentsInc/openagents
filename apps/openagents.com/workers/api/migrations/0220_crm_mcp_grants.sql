-- 0220_crm_mcp_grants.sql
--
-- Scoped MCP grants for the CRM MCP server (epic #5991, sub-issue #5995).
--
-- A scoped MCP credential is server-authoritative: it binds a set of authority
-- classes (e.g. operator_read) AND a single tenant_ref to a hashed token. The
-- MCP transport authenticates a request to exactly one principal — the admin
-- token (full grant) or a scoped grant row here — and the catalog filters
-- tools/resources by the granted authorities and reads only the bound tenant.
-- The raw token is never stored (only its SHA-256), and is shown once at mint.

CREATE TABLE IF NOT EXISTS crm_mcp_grants (
  id TEXT PRIMARY KEY NOT NULL,
  grant_ref TEXT NOT NULL UNIQUE,
  token_hash TEXT NOT NULL UNIQUE,
  tenant_ref TEXT NOT NULL,
  authority_classes_json TEXT NOT NULL DEFAULT '[]',
  label TEXT,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'revoked')),
  created_at TEXT NOT NULL,
  expires_at TEXT
);

CREATE INDEX IF NOT EXISTS crm_mcp_grants_tenant_idx
  ON crm_mcp_grants(tenant_ref, created_at DESC);
