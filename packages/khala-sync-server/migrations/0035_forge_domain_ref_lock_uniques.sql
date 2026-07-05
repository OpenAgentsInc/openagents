-- KS-8.16 follow-up (#8358, epic #8282/#8327): re-add the Forge domain's
-- deliberately-unported UNIQUE constraints ahead of wiring the ref-lock
-- write port (`forge-git-canonical-postgres-store.ts`,
-- `makePostgresForgeGitCanonicalStore`) as write authority.
--
-- 0021_forge_domain.sql's header named FIVE bullets of D1 uniques that were
-- deliberately not ported mid-migration (so a transiently stale read-back
-- mirror could never reject a converge upsert): active-lease-per-work,
-- held-lock-per-ref, issues' github_issue_number, PRs' change_ref,
-- token_hash, packfile digest/R2 key, and the mirror-receipt destination
-- tuple. Re-deriving the EXACT D1 definitions (worker migrations
-- 0251/0252/0253/0255/0260) for this pass turned up NINE distinct unique
-- indexes/constraints, not six — the prior pass's tracking comment bundled
-- two pairs together ("packfile digest / R2 key" is two separate D1
-- UNIQUE indexes on different columns; "github-issue-number/change_ref" is
-- one unique index per table, on two different tables). Verified against
-- a real backfilled copy of prod (`khala_sync_prod`) and staging
-- (`khala_sync_staging`) before writing this file: zero rows violate any
-- of the nine constraints below on either database (prod Forge traffic is
-- still tiny — single/low-digit rows per table).
--
-- The nine, one per bullet, each cross-referenced to its D1 source:
--
--   1. forge_coordination_issues: UNIQUE (tenant_ref, github_issue_number)
--      WHERE github_issue_number IS NOT NULL
--      — D1 `idx_forge_coordination_issues_github_number`, 0251 L22-24.
--   2. forge_coordination_prs: UNIQUE (tenant_ref, change_ref)
--      — D1 `idx_forge_coordination_prs_change_ref`, 0251 L48-49.
--   3. forge_dispatch_leases: UNIQUE (tenant_ref, work_ref)
--      WHERE state = 'active' (one ACTIVE dispatch lease per work item)
--      — D1 `idx_forge_dispatch_leases_active_work`, 0251 L84-86.
--   4. forge_dispatch_leases: UNIQUE (tenant_ref, idempotency_key_hash)
--      WHERE idempotency_key_hash IS NOT NULL
--      — D1 `idx_forge_dispatch_leases_idempotency`, 0251 L88-90. NOT
--      named in the prior pass's tracking comment; caught by this pass's
--      exhaustive re-diff against the D1 migration files.
--   5. forge_git_packfile_archives: UNIQUE (tenant_ref, packfile_sha256)
--      — D1 `idx_forge_git_packfile_archives_digest`, 0252 L29-30. The
--      existing Postgres index of the same name/columns
--      (0021_forge_domain.sql) is a plain (non-unique) index — dropped and
--      re-created UNIQUE below rather than left duplicated.
--   6. forge_git_packfile_archives: UNIQUE (artifact_r2_key)
--      — D1 `idx_forge_git_packfile_archives_r2_key`, 0252 L32-33. Also
--      not named individually in the prior pass's tracking comment.
--   7. forge_git_access_tokens: UNIQUE (token_hash)
--      — D1 `idx_forge_git_access_tokens_hash`, 0253 L35-36. The existing
--      Postgres index of a similar name (`idx_forge_git_access_tokens_hash`,
--      0021_forge_domain.sql) covers (token_hash, state) for the hot auth
--      lookup and is NOT the same index — left untouched; this migration
--      adds a separate token_hash-only unique index.
--   8. forge_git_ref_locks: UNIQUE (tenant_ref, repository_ref, ref_name)
--      WHERE state = 'held' (one HELD lock per ref)
--      — D1 `idx_forge_git_ref_locks_held_ref`, 0255 L93-95. MOOT for the
--      new Postgres write path: `makePostgresForgeGitCanonicalStore` never
--      writes `forge_git_ref_locks` rows at all (it uses
--      `pg_advisory_xact_lock` + `SELECT ... FOR UPDATE` instead of the
--      D1 held/applied/rejected row dance). Still added here for schema
--      parity and because the D1-authoritative dual-write mirror already
--      satisfies it today (D1 enforces the same constraint), so it is a
--      no-op integrity backstop rather than a functional requirement.
--   9. forge_github_mirror_receipts:
--      UNIQUE (tenant_ref, promotion_ref, destination_github_repository,
--              destination_github_ref)
--      — D1 table-level UNIQUE, 0260 L32-38. The existing Postgres index
--      of a similar name (`idx_forge_github_mirror_receipts_promotion`,
--      0021_forge_domain.sql) additionally orders by `updated_at DESC` for
--      its listing read and is NOT the same index — left untouched; this
--      migration adds a separate exact-tuple unique index.
--
-- These are schema-parity-only in this migration: no store code changes,
-- no write-path behavior changes. Wiring the ref-lock write port and any
-- domain-wide write-authority flip stay tracked on #8358 as separate,
-- explicitly-reviewed follow-up work.

-- 1. forge_coordination_issues — one github_issue_number per tenant.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_coordination_issues_github_number
  ON forge_coordination_issues (tenant_ref, github_issue_number)
  WHERE github_issue_number IS NOT NULL;

-- 2. forge_coordination_prs — one change_ref per tenant.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_coordination_prs_change_ref
  ON forge_coordination_prs (tenant_ref, change_ref);

-- 3. forge_dispatch_leases — one ACTIVE lease per (tenant_ref, work_ref).
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_dispatch_leases_active_work
  ON forge_dispatch_leases (tenant_ref, work_ref)
  WHERE state = 'active';

-- 4. forge_dispatch_leases — one lease per idempotency key hash.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_dispatch_leases_idempotency
  ON forge_dispatch_leases (tenant_ref, idempotency_key_hash)
  WHERE idempotency_key_hash IS NOT NULL;

-- 5. forge_git_packfile_archives — one packfile per digest per tenant.
-- Replaces the existing plain (non-unique) index of the same name.
DROP INDEX IF EXISTS idx_forge_git_packfile_archives_digest;
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_digest
  ON forge_git_packfile_archives (tenant_ref, packfile_sha256);

-- 6. forge_git_packfile_archives — one row per R2 artifact key globally.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_packfile_archives_r2_key
  ON forge_git_packfile_archives (artifact_r2_key);

-- 7. forge_git_access_tokens — one row per token hash globally. Distinct
-- from the existing (token_hash, state) auth-lookup index, left in place.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_access_tokens_hash_unique
  ON forge_git_access_tokens (token_hash);

-- 8. forge_git_ref_locks — one HELD lock per (tenant_ref, repository_ref,
-- ref_name). Moot for the new advisory-lock write path (header); kept for
-- schema parity and as a mirror-integrity backstop.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_git_ref_locks_held_ref
  ON forge_git_ref_locks (tenant_ref, repository_ref, ref_name)
  WHERE state = 'held';

-- 9. forge_github_mirror_receipts — one receipt per exact destination
-- tuple. Distinct from the existing listing index that also orders by
-- updated_at, left in place.
CREATE UNIQUE INDEX IF NOT EXISTS idx_forge_github_mirror_receipts_destination_unique
  ON forge_github_mirror_receipts (
    tenant_ref,
    promotion_ref,
    destination_github_repository,
    destination_github_ref
  );
