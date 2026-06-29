-- FORGE SU-2 (#6770): control-plane verification and promotion receipts.
--
-- These rows are coordination receipts only. They carry refs, checksums, bounded
-- states, and redacted/public-safe source refs; they never contain raw prompts,
-- private repository contents, provider payloads, local paths, secrets, wallet
-- material, invoices, preimages, or bearer tokens.

CREATE TABLE IF NOT EXISTS forge_verification_receipts (
  tenant_ref TEXT NOT NULL,
  verification_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  repository_ref TEXT NOT NULL,
  base_ref TEXT NOT NULL,
  base_head TEXT NOT NULL,
  head_ref TEXT NOT NULL,
  head_head TEXT NOT NULL,
  packfile_ref TEXT NOT NULL,
  packfile_sha256 TEXT NOT NULL,
  executor_identity_ref TEXT NOT NULL,
  command_ref TEXT NOT NULL,
  command_args_json TEXT NOT NULL DEFAULT '[]',
  exit_code INTEGER,
  verdict TEXT NOT NULL CHECK (verdict IN ('passed', 'failed', 'timed_out', 'cancelled', 'errored')),
  started_at TEXT NOT NULL,
  completed_at TEXT NOT NULL,
  artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  log_sha256 TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, verification_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_verification_receipts_change
  ON forge_verification_receipts (tenant_ref, change_ref, completed_at);

CREATE INDEX IF NOT EXISTS idx_forge_verification_receipts_verdict
  ON forge_verification_receipts (tenant_ref, verdict, completed_at);

CREATE TABLE IF NOT EXISTS forge_promotion_decisions (
  tenant_ref TEXT NOT NULL,
  promotion_ref TEXT NOT NULL,
  queue_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  decision TEXT NOT NULL CHECK (decision IN ('approved', 'blocked', 'superseded')),
  base_head TEXT NOT NULL,
  candidate_head TEXT NOT NULL,
  promoted_head TEXT,
  verification_ref TEXT,
  gate_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  decided_by_ref TEXT NOT NULL,
  decided_at TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  redacted INTEGER NOT NULL DEFAULT 1 CHECK (redacted = 1),
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, promotion_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_promotion_decisions_queue
  ON forge_promotion_decisions (tenant_ref, queue_ref, decided_at);

CREATE INDEX IF NOT EXISTS idx_forge_promotion_decisions_change
  ON forge_promotion_decisions (tenant_ref, change_ref, decided_at);
