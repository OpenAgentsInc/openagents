-- FORGE SU-6 (#6796): GitHub mirror receipts for Forge-promoted commits.
--
-- GitHub is a downstream mirror only. These rows record attempts to mirror a
-- Forge promotion receipt to the configured GitHub repository/branch, including
-- refusals and failures. They never authorize promotion decisions and never
-- store GitHub tokens, private repo contents, raw logs, prompts, local paths,
-- provider payloads, wallet material, invoices, preimages, or bearer tokens.

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref TEXT NOT NULL,
  mirror_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  source_canonical_ref TEXT NOT NULL,
  destination_github_ref TEXT NOT NULL,
  destination_repository TEXT NOT NULL,
  commit_id TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('pending', 'mirrored', 'refused', 'failed')),
  attempted_at TEXT NOT NULL,
  mirrored_at TEXT,
  refusal_reason TEXT,
  error_reason TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  attention_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_promotion
  ON forge_github_mirror_receipts (tenant_ref, promotion_ref, updated_at);

CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, updated_at);
