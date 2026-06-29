-- FORGE-1 (#6746): forge.openagents.com coordination source-of-truth.
--
-- D1 owns the coordination rows that GitHub mirrors from: work records, change
-- records, NIP-34-aligned status transitions, dispatch leases, and the virtual
-- merge-queue ledger. Rows carry refs, timestamps, and JSON-encoded ref arrays;
-- never raw prompts, private repository contents, local paths, provider
-- payloads, secrets, or wallet material.

CREATE TABLE IF NOT EXISTS forge_coordination_issues (
  tenant_ref TEXT NOT NULL,
  issue_ref TEXT NOT NULL,
  github_issue_number INTEGER,
  title TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('open', 'closed', 'draft')),
  priority_ref TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, issue_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_coordination_issues_github_number
  ON forge_coordination_issues (tenant_ref, github_issue_number)
  WHERE github_issue_number IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_forge_coordination_issues_state_priority
  ON forge_coordination_issues (tenant_ref, state, priority_ref, updated_at);

CREATE TABLE IF NOT EXISTS forge_coordination_prs (
  tenant_ref TEXT NOT NULL,
  pr_ref TEXT NOT NULL,
  issue_ref TEXT NOT NULL,
  change_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('draft', 'open', 'ready', 'blocked', 'applied', 'closed')),
  base_head TEXT NOT NULL,
  patch_head TEXT NOT NULL,
  verification_ref TEXT,
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, pr_ref),
  FOREIGN KEY (tenant_ref, issue_ref)
    REFERENCES forge_coordination_issues (tenant_ref, issue_ref)
    ON DELETE CASCADE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_coordination_prs_change_ref
  ON forge_coordination_prs (tenant_ref, change_ref);

CREATE INDEX IF NOT EXISTS idx_forge_coordination_prs_issue_state
  ON forge_coordination_prs (tenant_ref, issue_ref, state, updated_at);

CREATE TABLE IF NOT EXISTS forge_coordination_status (
  tenant_ref TEXT NOT NULL,
  status_ref TEXT NOT NULL,
  subject_ref TEXT NOT NULL,
  nip34_kind INTEGER NOT NULL CHECK (nip34_kind IN (1630, 1631, 1632, 1633)),
  state TEXT NOT NULL CHECK (state IN ('open', 'applied', 'closed', 'draft')),
  actor_ref TEXT NOT NULL,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, status_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_coordination_status_subject
  ON forge_coordination_status (tenant_ref, subject_ref, created_at);

CREATE TABLE IF NOT EXISTS forge_dispatch_leases (
  tenant_ref TEXT NOT NULL,
  lease_ref TEXT NOT NULL,
  work_ref TEXT NOT NULL,
  owner_agent_ref TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('active', 'released', 'expired', 'cancelled')),
  idempotency_key_hash TEXT,
  acquired_at TEXT NOT NULL,
  heartbeat_at TEXT NOT NULL,
  expires_at TEXT NOT NULL,
  released_at TEXT,
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lease_ref)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_dispatch_leases_active_work
  ON forge_dispatch_leases (tenant_ref, work_ref)
  WHERE state = 'active';

CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_dispatch_leases_idempotency
  ON forge_dispatch_leases (tenant_ref, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_forge_dispatch_leases_expiry
  ON forge_dispatch_leases (tenant_ref, state, expires_at);

CREATE TABLE IF NOT EXISTS forge_merge_queue_ledger (
  tenant_ref TEXT NOT NULL,
  queue_ref TEXT NOT NULL,
  base_head TEXT NOT NULL,
  actual_head TEXT NOT NULL,
  virtual_head TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('projected', 'blocked', 'promoting', 'promoted', 'superseded')),
  next_promotion_ref TEXT,
  ready_json TEXT NOT NULL DEFAULT '[]',
  blocked_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  PRIMARY KEY (tenant_ref, queue_ref)
);

CREATE INDEX IF NOT EXISTS idx_forge_merge_queue_ledger_state
  ON forge_merge_queue_ledger (tenant_ref, state, updated_at);
