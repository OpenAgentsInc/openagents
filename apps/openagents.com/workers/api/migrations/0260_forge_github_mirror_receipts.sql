-- FORGE SU-6 (#6796): GitHub mirror receipts for Forge-promoted commits.
--
-- GitHub is a downstream read-only mirror. These rows record the mirror worker's
-- one-way attempt from a Forge promotion/canonical ref to the configured GitHub
-- branch. They carry refs, bounded status, public commit ids, and redacted
-- refusal/error reasons only; they never contain GitHub tokens, raw packfiles,
-- raw source, private repository contents, local paths, provider payloads,
-- prompts, wallet material, or payment material.

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_github_repository TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('mirrored', 'refused', 'failed')),
  attempt_count INTEGER NOT NULL DEFAULT 1 CHECK (attempt_count >= 1),
  first_attempted_at TEXT NOT NULL,
  last_attempted_at TEXT NOT NULL,
  completed_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref),
  UNIQUE (
    tenant_ref,
    promotion_ref,
    destination_github_repository,
    destination_github_ref
  )
);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_promotion
  ON forge_github_mirror_receipts (tenant_ref, promotion_ref, updated_at);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, updated_at);
