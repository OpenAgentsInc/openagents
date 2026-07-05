-- KS-8.16 (#8327): Forge (git intake + coordination) domain — Postgres
-- twins of ALL SIXTEEN `forge_*` tables (worker migrations 0251/0252/0253/
-- 0254/0255/0256/0259/0260/0284): coordination issues/PRs/status, dispatch
-- leases, merge-queue ledger, packfile archives, tenants, git access
-- tokens (+scopes), verification receipts, promotion decisions,
-- receive-pack intakes, canonical refs, object tips, ref locks, GitHub
-- mirror receipts.
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.13 (universal porting rules
-- in §1); templates: 0014_forum_content.sql (KS-8.10),
-- 0016_treasury_domain.sql (KS-8.8), 0017_khala_code_product_state.sql
-- (KS-8.13).
--
-- R2 SPLIT (the §3.13 risk): raw git bytes NEVER live in a relational
-- row. `forge_git_packfile_archives` carries the R2 key + digest +
-- bounded metadata (exactly as in D1); `forge_git_objects` is the
-- object-tip dedupe ledger (object ids + provenance refs, no content).
-- The Postgres twins are column-for-column with D1 — no payload column
-- is added, none is widened.
--
-- SECRETS (SPEC invariant 9): `forge_git_access_tokens` stores token
-- HASHES and display prefixes only — raw tokens are never stored in D1
-- and therefore never stored here (the twin is column-for-column, no
-- widening). Custody columns (`token_hash`, `token_prefix`) never appear
-- in migration diagnostics or backfill/verify output — row KEYS
-- (tenant_ref/token_ref) and sha256 row hashes only.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's
-- byte representation — TEXT ISO-8601 timestamps, 0/1 booleans as
-- smallint, JSON payload columns as text (NOT jsonb: row-hash
-- reconciliation compares exact bytes). Counters/counts are bigint.
--
-- UNIQUE/PARTIAL-UNIQUE INDEXES ARE DELIBERATELY NOT PORTED
-- MID-MIGRATION (the KS-8.6/KS-8.8 rule): D1 stays the sole write
-- authority in this lane and enforces
--   * one ACTIVE dispatch lease per (tenant_ref, work_ref),
--   * one HELD ref lock per (tenant_ref, repository_ref, ref_name),
--   * unique (tenant_ref, github_issue_number), unique
--     (tenant_ref, change_ref) on PRs,
--   * unique token_hash, unique packfile digest / R2 key,
--   * unique (tenant, promotion, destination repo, destination ref)
--     mirror receipts.
-- The Postgres twin is a fail-soft read-back mirror converging on the PK;
-- a transiently stale twin (mirror lag, backfill catch-up) must NEVER be
-- able to reject a converge upsert, so these constraints are re-added at
-- the read/write cutover — the same moment the ref-lock protocol is
-- deliberately re-ported onto real `SELECT ... FOR UPDATE` row locks
-- instead of emulating the D1 held-lock dance (MIGRATION_PLAN §3.13).
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule),
-- from the five owning stores (`forge-coordination-store.ts`,
-- `forge-git-canonical-store.ts`, `forge-git-packfile-archive-store.ts`,
-- `forge-tenant-git-auth-store.ts`, `forge-github-mirror-store.ts` — the
-- CLOSED writer/reader set for these tables). Justifications inline. D1
-- artifacts with no live read behind them (`idx_forge_git_access_tokens_prefix`
-- — no store queries by prefix; `idx_forge_git_objects_packfile` — no
-- store read scans by packfile) are dropped until a read re-derives them.
--
-- NO FOREIGN KEYS (dual-write mirrors and the backfill land per-row;
-- integrity is verified by reconciliation — same as 0005/0008/0010/0014).

-- --------------------------------------------------------------------------
-- Coordination source of truth (worker 0251 + 0284)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_coordination_issues (
  tenant_ref           text NOT NULL,
  issue_ref            text NOT NULL,
  github_issue_number  bigint,
  title                text NOT NULL,
  state                text NOT NULL,
  priority_ref         text,
  source_refs_json     text NOT NULL DEFAULT '[]',
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  git_token_refs_json  text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, issue_ref)
);

-- listIssues: WHERE tenant_ref ORDER BY updated_at DESC, issue_ref DESC.
CREATE INDEX IF NOT EXISTS idx_forge_coordination_issues_updated
  ON forge_coordination_issues (tenant_ref, updated_at DESC, issue_ref DESC);

CREATE TABLE IF NOT EXISTS forge_coordination_prs (
  tenant_ref        text NOT NULL,
  pr_ref            text NOT NULL,
  issue_ref         text NOT NULL,
  change_ref        text NOT NULL,
  state             text NOT NULL,
  base_head         text NOT NULL,
  patch_head        text NOT NULL,
  verification_ref  text,
  blocker_refs_json text NOT NULL DEFAULT '[]',
  source_refs_json  text NOT NULL DEFAULT '[]',
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  PRIMARY KEY (tenant_ref, pr_ref)
);

-- listChanges: WHERE tenant_ref [AND issue_ref] ORDER BY updated_at DESC.
CREATE INDEX IF NOT EXISTS idx_forge_coordination_prs_updated
  ON forge_coordination_prs (tenant_ref, updated_at DESC, pr_ref DESC);
CREATE INDEX IF NOT EXISTS idx_forge_coordination_prs_issue
  ON forge_coordination_prs (tenant_ref, issue_ref, updated_at DESC);

CREATE TABLE IF NOT EXISTS forge_coordination_status (
  tenant_ref       text NOT NULL,
  status_ref       text NOT NULL,
  subject_ref      text NOT NULL,
  nip34_kind       bigint NOT NULL,
  state            text NOT NULL,
  actor_ref        text NOT NULL,
  source_refs_json text NOT NULL DEFAULT '[]',
  created_at       text NOT NULL,
  PRIMARY KEY (tenant_ref, status_ref)
);

-- listStatuses: WHERE tenant_ref [AND subject_ref] ORDER BY created_at DESC.
CREATE INDEX IF NOT EXISTS idx_forge_coordination_status_subject
  ON forge_coordination_status (tenant_ref, subject_ref, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_forge_coordination_status_created
  ON forge_coordination_status (tenant_ref, created_at DESC, status_ref DESC);

CREATE TABLE IF NOT EXISTS forge_dispatch_leases (
  tenant_ref           text NOT NULL,
  lease_ref            text NOT NULL,
  work_ref             text NOT NULL,
  owner_agent_ref      text NOT NULL,
  state                text NOT NULL,
  idempotency_key_hash text,
  acquired_at          text NOT NULL,
  heartbeat_at         text NOT NULL,
  expires_at           text NOT NULL,
  released_at          text,
  source_refs_json     text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lease_ref)
);

-- acquire/readActive/listDispatchLeases key on (tenant_ref, work_ref);
-- the ACTIVE-per-work partial UNIQUE is deliberately NOT ported (header).
CREATE INDEX IF NOT EXISTS idx_forge_dispatch_leases_work
  ON forge_dispatch_leases (tenant_ref, work_ref, state);
CREATE INDEX IF NOT EXISTS idx_forge_dispatch_leases_acquired
  ON forge_dispatch_leases (tenant_ref, acquired_at DESC, lease_ref DESC);

CREATE TABLE IF NOT EXISTS forge_merge_queue_ledger (
  tenant_ref         text NOT NULL,
  queue_ref          text NOT NULL,
  base_head          text NOT NULL,
  actual_head        text NOT NULL,
  virtual_head       text NOT NULL,
  state              text NOT NULL,
  next_promotion_ref text,
  ready_json         text NOT NULL DEFAULT '[]',
  blocked_json       text NOT NULL DEFAULT '[]',
  source_refs_json   text NOT NULL DEFAULT '[]',
  created_at         text NOT NULL,
  updated_at         text NOT NULL,
  PRIMARY KEY (tenant_ref, queue_ref)
);

-- listMergeQueueLedgers/readLatestMergeQueueLedger: WHERE tenant_ref
-- ORDER BY updated_at DESC, queue_ref DESC.
CREATE INDEX IF NOT EXISTS idx_forge_merge_queue_ledger_updated
  ON forge_merge_queue_ledger (tenant_ref, updated_at DESC, queue_ref DESC);

-- --------------------------------------------------------------------------
-- Packfile archive ledger (worker 0252) — bytes stay in R2
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_git_packfile_archives (
  tenant_ref        text NOT NULL,
  packfile_ref      text NOT NULL,
  repository_ref    text NOT NULL,
  change_ref        text,
  receive_pack_ref  text,
  artifact_r2_key   text NOT NULL,
  packfile_sha256   text NOT NULL,
  packfile_bytes    bigint NOT NULL,
  object_format     text NOT NULL,
  command_count     bigint NOT NULL,
  capabilities_json text NOT NULL DEFAULT '[]',
  ref_updates_json  text NOT NULL DEFAULT '[]',
  source_refs_json  text NOT NULL DEFAULT '[]',
  content_type      text NOT NULL DEFAULT 'application/x-git-packed-objects',
  visibility        text NOT NULL,
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  PRIMARY KEY (tenant_ref, packfile_ref)
);

-- readPackfileByDigest dedupe lookup (plain, not UNIQUE — header) and
-- listPackfiles: WHERE tenant_ref [AND repository_ref] ORDER BY
-- created_at DESC.
CREATE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_digest
  ON forge_git_packfile_archives (tenant_ref, packfile_sha256);
CREATE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_repository
  ON forge_git_packfile_archives (tenant_ref, repository_ref, created_at DESC);

-- --------------------------------------------------------------------------
-- Tenant auth + token-scoped git access (worker 0253 + 0256 + 0284)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_tenants (
  tenant_ref                   text NOT NULL PRIMARY KEY,
  display_name                 text NOT NULL,
  state                        text NOT NULL,
  created_at                   text NOT NULL,
  updated_at                   text NOT NULL,
  confidential_workspace_mode  text,
  attestation_ref              text,
  encrypted_knowledge_pack_ref text,
  refusal_reason               text,
  retention_policy_ref         text
);

CREATE TABLE IF NOT EXISTS forge_git_access_tokens (
  tenant_ref           text NOT NULL,
  token_ref            text NOT NULL,
  subject_ref          text NOT NULL,
  repository_ref       text NOT NULL,
  token_hash           text NOT NULL,
  token_prefix         text NOT NULL,
  state                text NOT NULL,
  created_at           text NOT NULL,
  expires_at           text NOT NULL,
  last_used_at         text,
  revoked_at           text,
  source_refs_json     text NOT NULL DEFAULT '[]',
  ref_restrictions_json text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, token_ref)
);

-- authenticateGitAccessToken: WHERE token_hash = ? AND state = 'active'
-- (the hot auth lookup on every smart-Git request). Plain index, not
-- UNIQUE (header); the prefix index is NOT ported — no live read uses it.
CREATE INDEX IF NOT EXISTS idx_forge_git_access_tokens_hash
  ON forge_git_access_tokens (token_hash, state);
CREATE INDEX IF NOT EXISTS idx_forge_git_access_tokens_repository
  ON forge_git_access_tokens (tenant_ref, repository_ref, state, expires_at);

CREATE TABLE IF NOT EXISTS forge_git_access_token_scopes (
  tenant_ref text NOT NULL,
  token_ref  text NOT NULL,
  scope      text NOT NULL,
  created_at text NOT NULL,
  PRIMARY KEY (tenant_ref, token_ref, scope)
);
-- scope lookups key on (tenant_ref, token_ref[, scope IN (...)]) — the PK
-- prefix serves them; D1's (tenant_ref, scope) index has no live read.

-- --------------------------------------------------------------------------
-- Control-plane receipts (worker 0254 + 0259)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_verification_receipts (
  tenant_ref            text NOT NULL,
  verification_ref      text NOT NULL,
  change_ref            text NOT NULL,
  repository_ref        text NOT NULL,
  base_ref              text NOT NULL,
  base_head             text NOT NULL,
  head_ref              text NOT NULL,
  head_head             text NOT NULL,
  packfile_ref          text NOT NULL,
  packfile_sha256       text NOT NULL,
  executor_identity_ref text NOT NULL,
  command_ref           text NOT NULL,
  command_args_json     text NOT NULL DEFAULT '[]',
  exit_code             bigint,
  verdict               text NOT NULL,
  started_at            text NOT NULL,
  completed_at          text NOT NULL,
  artifact_refs_json    text NOT NULL DEFAULT '[]',
  log_sha256            text NOT NULL,
  source_refs_json      text NOT NULL DEFAULT '[]',
  redacted              smallint NOT NULL DEFAULT 1,
  created_at            text NOT NULL,
  PRIMARY KEY (tenant_ref, verification_ref)
);

-- listVerificationReceipts: WHERE tenant_ref [AND change_ref] ORDER BY
-- completed_at DESC.
CREATE INDEX IF NOT EXISTS idx_forge_verification_receipts_change
  ON forge_verification_receipts (tenant_ref, change_ref, completed_at DESC);
CREATE INDEX IF NOT EXISTS idx_forge_verification_receipts_completed
  ON forge_verification_receipts (tenant_ref, completed_at DESC, verification_ref DESC);

CREATE TABLE IF NOT EXISTS forge_promotion_decisions (
  tenant_ref        text NOT NULL,
  promotion_ref     text NOT NULL,
  queue_ref         text NOT NULL,
  change_ref        text NOT NULL,
  decision          text NOT NULL,
  base_head         text NOT NULL,
  candidate_head    text NOT NULL,
  promoted_head     text,
  verification_ref  text,
  gate_refs_json    text NOT NULL DEFAULT '[]',
  blocker_refs_json text NOT NULL DEFAULT '[]',
  decided_by_ref    text NOT NULL,
  decided_at        text NOT NULL,
  source_refs_json  text NOT NULL DEFAULT '[]',
  redacted          smallint NOT NULL DEFAULT 1,
  created_at        text NOT NULL,
  target_ref        text NOT NULL DEFAULT '',
  queue_position    bigint NOT NULL DEFAULT 0,
  gate_results_json text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, promotion_ref)
);

-- listPromotionDecisionReceipts: WHERE tenant_ref [AND change_ref] ORDER
-- BY decided_at DESC; queue replay reads by (tenant_ref, queue_ref).
CREATE INDEX IF NOT EXISTS idx_forge_promotion_decisions_queue
  ON forge_promotion_decisions (tenant_ref, queue_ref, decided_at DESC);
CREATE INDEX IF NOT EXISTS idx_forge_promotion_decisions_change
  ON forge_promotion_decisions (tenant_ref, change_ref, decided_at DESC);

-- --------------------------------------------------------------------------
-- Smart-Git receive-pack canonical intake (worker 0255)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_git_receive_pack_intakes (
  tenant_ref       text NOT NULL,
  receive_pack_ref text NOT NULL,
  repository_ref   text NOT NULL,
  token_ref        text NOT NULL,
  subject_ref      text NOT NULL,
  change_ref       text,
  packfile_ref     text,
  packfile_sha256  text,
  packfile_bytes   bigint NOT NULL,
  object_format    text NOT NULL,
  state            text NOT NULL,
  command_count    bigint NOT NULL,
  ref_updates_json text NOT NULL DEFAULT '[]',
  source_refs_json text NOT NULL DEFAULT '[]',
  rejection_code   text,
  rejection_reason text,
  created_at       text NOT NULL,
  updated_at       text NOT NULL,
  PRIMARY KEY (tenant_ref, receive_pack_ref)
);

-- intake listings by repository; change-scoped listings (partial: most
-- intakes carry no change_ref).
CREATE INDEX IF NOT EXISTS idx_forge_git_receive_pack_intakes_repository
  ON forge_git_receive_pack_intakes (tenant_ref, repository_ref, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_forge_git_receive_pack_intakes_change
  ON forge_git_receive_pack_intakes (tenant_ref, change_ref, created_at DESC)
  WHERE change_ref IS NOT NULL;

CREATE TABLE IF NOT EXISTS forge_git_refs (
  tenant_ref                  text NOT NULL,
  repository_ref              text NOT NULL,
  ref_name                    text NOT NULL,
  object_id                   text,
  previous_object_id          text,
  object_format               text NOT NULL,
  state                       text NOT NULL,
  updated_by_change_ref       text NOT NULL,
  updated_by_packfile_ref     text NOT NULL,
  updated_by_receive_pack_ref text NOT NULL,
  source_refs_json            text NOT NULL DEFAULT '[]',
  created_at                  text NOT NULL,
  updated_at                  text NOT NULL,
  PRIMARY KEY (tenant_ref, repository_ref, ref_name)
);

-- listRefs(state): WHERE tenant/repo/state ORDER BY ref_name — the ref
-- advertisement scan (D1's (…, state, updated_at) artifact re-derived to
-- the actual ORDER BY). readRef/unfiltered listRefs ride the PK.
CREATE INDEX IF NOT EXISTS idx_forge_git_refs_state
  ON forge_git_refs (tenant_ref, repository_ref, state, ref_name);

CREATE TABLE IF NOT EXISTS forge_git_objects (
  tenant_ref       text NOT NULL,
  repository_ref   text NOT NULL,
  object_id        text NOT NULL,
  object_format    text NOT NULL,
  packfile_ref     text NOT NULL,
  packfile_sha256  text NOT NULL,
  first_seen_at    text NOT NULL,
  latest_seen_at   text NOT NULL,
  source_refs_json text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, repository_ref, object_id)
);
-- readObject rides the PK; D1's packfile index has no live read — dropped.

CREATE TABLE IF NOT EXISTS forge_git_ref_locks (
  tenant_ref             text NOT NULL,
  lock_ref               text NOT NULL,
  repository_ref         text NOT NULL,
  ref_name               text NOT NULL,
  receive_pack_ref       text NOT NULL,
  expected_old_object_id text NOT NULL,
  new_object_id          text NOT NULL,
  action                 text NOT NULL,
  state                  text NOT NULL,
  acquired_at            text NOT NULL,
  released_at            text,
  source_refs_json       text NOT NULL DEFAULT '[]',
  PRIMARY KEY (tenant_ref, lock_ref)
);

-- lock rows are read back per receive-pack (mirror + inspection); the
-- HELD-per-ref partial UNIQUE is deliberately NOT ported (header — the
-- lock protocol re-ports onto SELECT ... FOR UPDATE at cutover).
CREATE INDEX IF NOT EXISTS idx_forge_git_ref_locks_receive_pack
  ON forge_git_ref_locks (tenant_ref, receive_pack_ref);
CREATE INDEX IF NOT EXISTS idx_forge_git_ref_locks_ref_state
  ON forge_git_ref_locks (tenant_ref, repository_ref, ref_name, state);

-- --------------------------------------------------------------------------
-- GitHub mirror receipts (worker 0260)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS forge_github_mirror_receipts (
  tenant_ref                    text NOT NULL,
  mirror_ref                    text NOT NULL,
  promotion_ref                 text NOT NULL,
  change_ref                    text NOT NULL,
  repository_ref                text NOT NULL,
  source_canonical_ref          text NOT NULL,
  destination_github_repository text NOT NULL,
  destination_github_ref        text NOT NULL,
  commit_id                     text NOT NULL,
  status                        text NOT NULL,
  attempt_count                 bigint NOT NULL DEFAULT 1,
  first_attempted_at            text NOT NULL,
  last_attempted_at             text NOT NULL,
  completed_at                  text,
  refusal_reason                text,
  error_reason                  text,
  source_refs_json              text NOT NULL DEFAULT '[]',
  redacted                      smallint NOT NULL DEFAULT 1,
  created_at                    text NOT NULL,
  updated_at                    text NOT NULL,
  PRIMARY KEY (tenant_ref, mirror_ref)
);

-- listReceipts by promotion/status; readReceiptForPromotion keys on the
-- full destination tuple (plain index — the D1 UNIQUE is not ported
-- mid-migration, header).
CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_promotion
  ON forge_github_mirror_receipts (
    tenant_ref, promotion_ref,
    destination_github_repository, destination_github_ref,
    updated_at DESC
  );
CREATE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_status
  ON forge_github_mirror_receipts (tenant_ref, status, updated_at DESC);
