-- CFG-4 Domain 2 (#8519, epic #8515): identity core HARD cutover —
-- `users` + `auth_identities` become Postgres-AUTHORITATIVE (D1 code path
-- deleted, including the auth-GATE reads). 0028 deliberately did NOT port
-- UNIQUE constraints while D1 was the sole write authority; now that the
-- Worker writes these two tables here directly, the D1-era uniqueness the
-- writers rely on must be enforced by Postgres itself.
--
-- Derived from the actual statements (the KS-8.2 "indexes from real reads"
-- rule):
--   * `index.ts` upsertGitHubUser/upsertEmailUser use
--     `ON CONFLICT(provider, provider_subject)` — needs a UNIQUE arbiter
--     (worker migration 0002 declares UNIQUE(provider, provider_subject)).
--     Replaces the non-unique 0028 accelerator of the same shape.
--   * `agent-registration.ts` registration relies on the same UNIQUE to
--     refuse duplicate agent externalIds (provider='agent_programmatic').
--   * users PK lookups (session upserts, gate reads, profile enrichment
--     IN-lists) ride the primary key — nothing to add.
--   * admin user listing (`admin-credits-routes.ts`) pages
--     `WHERE kind = 'human' AND deleted_at IS NULL ORDER BY created_at DESC`
--     — accelerator below.
--   * `auth_identities.user_id` lookups (github-username enrichment
--     subqueries) ride the existing 0028 `auth_identities_user_idx`.
--
-- APPLY BEFORE the cutover deploy. If the unique index creation fails on
-- duplicate (provider, provider_subject) mirror rows, reconcile against D1
-- first (`scripts/backfill-identity-auth.ts --table auth_identities` while
-- the old D1-writing Worker is still serving) — D1 enforced this unique,
-- so genuine duplicates can only be stale mirror artifacts.

-- ---------------------------------------------------------------------------
-- users: onboarding-state columns (worker migration 0025) — the 0028 twin
-- carried only the nine canonical identity columns, but the D1 `users`
-- table also owns the onboarding wizard state that
-- `onboarding/repository.ts` reads and updates. The hard cut moves that
-- module here, so the twin must carry the full worker-side column set
-- (same type-fidelity rules as 0028: TEXT ISO timestamps, D1 INTEGER 0/1
-- flags become bigint).
-- ---------------------------------------------------------------------------

ALTER TABLE users
  ADD COLUMN IF NOT EXISTS onboarding_step text NOT NULL DEFAULT 'repository',
  ADD COLUMN IF NOT EXISTS onboarding_completed_at text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_provider text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_id text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_owner text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_name text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_full_name text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_private bigint,
  ADD COLUMN IF NOT EXISTS onboarding_repository_default_branch text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_html_url text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_description text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_selected_at text,
  ADD COLUMN IF NOT EXISTS onboarding_repository_skipped_at text,
  ADD COLUMN IF NOT EXISTS onboarding_billing_skipped_at text,
  ADD COLUMN IF NOT EXISTS onboarding_goal text,
  ADD COLUMN IF NOT EXISTS onboarding_updated_at text;

DROP INDEX IF EXISTS auth_identities_provider_subject_idx;

CREATE UNIQUE INDEX IF NOT EXISTS auth_identities_provider_subject_key
  ON auth_identities (provider, provider_subject);

-- Non-unique twin of worker 0004's (provider, provider_username) index —
-- serves the github-login target resolution reads
-- (`admin-credits-routes.ts`, `operator-targets.ts`).
CREATE INDEX IF NOT EXISTS auth_identities_provider_username_idx
  ON auth_identities (provider, provider_username);

-- Admin signup listing: newest humans first.
CREATE INDEX IF NOT EXISTS users_kind_created_at_idx
  ON users (kind, created_at DESC);
