-- FORGE SU-6 (#6796): GitHub mirror receipts for Forge-promoted commits.
--
-- GitHub is a downstream mirror only. These rows record attempts to mirror a
-- Forge promotion ref to a GitHub ref; they do not authorize promotion.

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'mirrored', 'refused', 'errored')),
  attempted_at TEXT NOT NULL,
  mirrored_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  retry_count INTEGER NOT NULL DEFAULT 0,
  idempotency_key TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_idempotency
  ON forge_github_mirror_receipts (tenant_ref, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_change
  ON forge_github_mirror_receipts (tenant_ref, change_ref, attempted_at);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_promotion
  ON forge_github_mirror_receipts (tenant_ref, promotion_ref, attempted_at);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, attempted_at);
