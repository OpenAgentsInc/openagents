-- FORGE SU-6 (#6796): GitHub read-only mirror receipts.
--
-- GitHub is downstream visibility only. These rows record attempts to mirror
-- Forge-promoted canonical refs to GitHub refs, including refusals and upstream
-- errors. Promotion authority remains forge_promotion_decisions.

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  github_repository TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('mirrored', 'failed', 'refused')),
  attempted_at TEXT NOT NULL,
  mirrored_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref),
  CHECK (
    (status = 'mirrored' AND mirrored_at IS NOT NULL AND refusal_reason IS NULL AND error_reason IS NULL)
    OR (status = 'failed' AND mirrored_at IS NULL AND error_reason IS NOT NULL)
    OR (status = 'refused' AND mirrored_at IS NULL AND refusal_reason IS NOT NULL)
  )
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_destination
  ON forge_github_mirror_receipts (
    tenant_ref,
    promotion_ref,
    source_canonical_ref,
    destination_github_ref,
    commit_id
  );

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, attempted_at);
