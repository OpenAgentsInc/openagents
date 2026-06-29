-- FORGE SU-6 (#6796): GitHub mirror receipts for Forge-promoted commits.
--
-- GitHub is downstream visibility only. These rows record attempts to move a
-- GitHub ref after Forge has already accepted a promotion decision.

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_repository TEXT NOT NULL,
  destination_ref TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('mirrored', 'already_mirrored', 'failed', 'refused')),
  attempted_at TEXT NOT NULL,
  mirrored_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_idempotent
  ON forge_github_mirror_receipts (
    tenant_ref,
    promotion_ref,
    destination_repository,
    destination_ref,
    commit_id
  );

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_promotion
  ON forge_github_mirror_receipts (tenant_ref, promotion_ref, attempted_at);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, attempted_at);
