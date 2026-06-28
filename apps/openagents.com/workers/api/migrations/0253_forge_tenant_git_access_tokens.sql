-- FORGE-4 (#6750): tenant auth + token-scoped git access.
--
-- Raw git access tokens are never stored. D1 stores tenant records, token
-- hashes/prefixes, repository scope, lifecycle timestamps, and one bounded git
-- scope row per granted operation. Route/protocol layers authenticate by hash
-- and exact repository/scope, then update last_used_at.

CREATE TABLE IF NOT EXISTS forge_tenants (
  tenant_ref TEXT PRIMARY KEY,
  display_name TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'suspended')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS forge_git_access_tokens (
  tenant_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  token_hash TEXT NOT NULL,
  token_prefix TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'revoked', 'expired')),
  created_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  last_used_at TEXT,
  revoked_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, token_ref),
  FOREIGN KEY (tenant_ref)
    REFERENCES forge_tenants (tenant_ref)
    ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_access_tokens_hash
  ON forge_git_access_tokens (token_hash);

CREATE INDEX IF NOT EXISTS idx_forge_git_access_tokens_prefix
  ON forge_git_access_tokens (token_prefix);

CREATE INDEX IF NOT EXISTS idx_forge_git_access_tokens_repository
  ON forge_git_access_tokens (tenant_ref, repository_ref, state, expires_at);

CREATE TABLE IF NOT EXISTS forge_git_access_token_scopes (
  tenant_ref TEXT NOT NULL,
  token_ref TEXT NOT NULL,
  scope TEXT NOT NULL CHECK (scope IN ('git:upload-pack', 'git:receive-pack', 'git:admin')),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, token_ref, scope),
  FOREIGN KEY (tenant_ref, token_ref)
    REFERENCES forge_git_access_tokens (tenant_ref, token_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_forge_git_access_token_scopes_scope
  ON forge_git_access_token_scopes (tenant_ref, scope);
