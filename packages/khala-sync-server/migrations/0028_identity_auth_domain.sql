-- KS-8.18 (#8329): Identity and auth core domain — Postgres twins of the
-- SEVENTEEN canonical identity/auth tables (worker migrations
-- 0002/0003/0004/0009/0011/0044-0050/0173/0234/0237/0283): users, auth
-- identities, OpenAuth storage + agent links, GitHub write connections /
-- attempts / grants, and the provider (BYOK) account custody family
-- (accounts, connection attempts, auth grants, events, sanity checks,
-- parallel-probe receipts, leases, failover receipts, token custody +
-- audit).
-- Plan: docs/khala-sync/MIGRATION_PLAN.md §3.15 (Wave E — the LAST domain;
-- universal porting rules in §1). Templates: 0021_forge_domain.sql
-- (KS-8.16 store-factory read-back mirror), 0015_billing_pay_ins.sql
-- (KS-8.7 money-domain discipline), 0011_artanis_domain.sql (KS-8.6).
--
-- WHY LAST: this is the hottest READ family in the system (auth runs on
-- every request) and the maximum blast radius — a bad cutover breaks
-- everything. It goes after the recipe has been proven ~14 times. This
-- lane lands MACHINERY ONLY: D1 stays the SOLE authority, the Postgres
-- twin is a fail-soft dual-write mirror + backfill target. There is NO
-- read cutover here — auth read serving from Postgres is the highest-risk,
-- owner-gated, done-last step (docs/khala-sync/RUNBOOK.md).
--
-- SECRETS (SPEC invariant 9 — the invariant this domain motivated). The
-- twin stores EXACTLY what D1 stores, with NO widening and the SAME
-- at-rest encryption posture as today. Raw tokens live on NEITHER engine.
-- Secret-bearing columns are twinned byte-for-byte but NEVER appear in
-- migration diagnostics or backfill/verify output — row KEYS (ids / refs /
-- owner_user_id) and sha256 row hashes only:
--   * provider_account_token_custody.{refresh,access,id_token}_ciphertext_b64
--     + their IVs + KMS key ids — AES-GCM ciphertext keyed by key id;
--   * openauth_storage.value_json — the OpenAuth session/refresh payload;
--   * provider_account_connection_attempts.user_code — device-code nonce;
--   * github_write_connection_attempts.state — OAuth CSRF / owner-claim
--     challenge nonce.
-- These are declared as `custodyColumns` in the shared registry
-- (packages/khala-sync-server/src/identity-auth-domain-tables.ts) so the
-- mirror and the backfill keep them out of every log line and tally.
--
-- TYPE FIDELITY (v1, reconciliation-bearing): every column keeps D1's byte
-- representation — TEXT ISO-8601 timestamps, JSON payload columns as text
-- (NOT jsonb: row-hash reconciliation compares exact bytes). D1 INTEGER
-- columns (openauth_storage.expires_at epoch-ms, operator_priority,
-- low_credit_flag 0/1, lease_limit, attempt_number, max_attempts) become
-- bigint. Ciphertext/base64 columns stay text.
--
-- UNIQUE / FOREIGN-KEY CONSTRAINTS ARE DELIBERATELY NOT PORTED MID-
-- MIGRATION (the KS-8.6/8.8/8.16 rule): D1 stays the sole write authority
-- and enforces its uniques (auth_identities(provider, provider_subject);
-- users→auth_identities→github/provider FKs; connection_ref / grant_ref /
-- provider_account_ref / lease_ref UNIQUE; github_write_connection_attempts
-- .state UNIQUE; openauth_agent_links owner/agent/credential tuple). The
-- Postgres twin is a fail-soft read-back mirror converging on the PK; a
-- transiently stale twin (mirror lag, backfill catch-up) must NEVER be
-- able to reject a converge upsert, so these constraints are re-added at
-- the read/write cutover follow-up, not here.
--
-- INDEXES ARE RE-DERIVED FROM ACTUAL QUERY PATTERNS (the KS-8.2 rule),
-- from the owning stores (auth/openauth-storage.ts,
-- github-write-connections.ts, provider-account-repository.ts,
-- provider-account-token-custody.ts, agent-registration.ts, and the
-- operator/pool route readers). Justifications inline. Indexes with no
-- live read behind them are dropped until a read re-derives them.
--
-- NO FOREIGN KEYS (dual-write mirrors + backfill land per-row; integrity
-- is verified by reconciliation — same as 0005/0008/0014/0021).

-- --------------------------------------------------------------------------
-- Core identity (worker 0002 + 0004)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS users (
  id            text NOT NULL,
  kind          text NOT NULL,
  display_name  text NOT NULL,
  primary_email text,
  avatar_url    text,
  status        text NOT NULL,
  created_at    text NOT NULL,
  updated_at    text NOT NULL,
  deleted_at    text,
  PRIMARY KEY (id)
);

-- Directory scans by kind+status (agent vs human listings).
CREATE INDEX IF NOT EXISTS users_kind_status_idx
  ON users (kind, status);

CREATE TABLE IF NOT EXISTS auth_identities (
  id                text NOT NULL,
  user_id           text NOT NULL,
  provider          text NOT NULL,
  provider_subject  text NOT NULL,
  email             text,
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  deleted_at        text,
  provider_username text,
  PRIMARY KEY (id)
);

-- The auth lookup: resolve identity by (provider, provider_subject) on
-- every OAuth callback. D1's UNIQUE on this pair is intentionally a plain
-- index here (see header) so a stale twin never rejects a converge.
CREATE INDEX IF NOT EXISTS auth_identities_provider_subject_idx
  ON auth_identities (provider, provider_subject);
-- List identities per user (account settings).
CREATE INDEX IF NOT EXISTS auth_identities_user_idx
  ON auth_identities (user_id);

-- --------------------------------------------------------------------------
-- OpenAuth storage (worker 0003) + agent links (worker 0234)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS openauth_storage (
  key        text NOT NULL,
  value_json text NOT NULL, -- CUSTODY: OpenAuth session/refresh payload.
  expires_at bigint,
  updated_at text NOT NULL,
  PRIMARY KEY (key)
);

-- Sweep expired session rows (the OpenAuth TTL scan).
CREATE INDEX IF NOT EXISTS openauth_storage_expires_at_idx
  ON openauth_storage (expires_at);

CREATE TABLE IF NOT EXISTS openauth_agent_links (
  id                  text NOT NULL,
  openauth_user_id    text NOT NULL,
  agent_user_id       text NOT NULL,
  agent_credential_id text,
  link_kind           text NOT NULL,
  status              text NOT NULL,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  revoked_at          text,
  PRIMARY KEY (id)
);

-- Owner → linked agents, and agent → owners (both active-status scoped).
CREATE INDEX IF NOT EXISTS openauth_agent_links_owner_status_idx
  ON openauth_agent_links (openauth_user_id, status, updated_at);
CREATE INDEX IF NOT EXISTS openauth_agent_links_agent_status_idx
  ON openauth_agent_links (agent_user_id, status, updated_at);

-- --------------------------------------------------------------------------
-- GitHub write connections / attempts / grants (worker 0011)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS github_write_connections (
  id              text NOT NULL,
  user_id         text NOT NULL,
  github_id       text NOT NULL,
  github_login    text NOT NULL,
  connection_ref  text NOT NULL,
  secret_ref      text,
  scopes_json     text NOT NULL,
  status          text NOT NULL,
  health          text NOT NULL,
  connected_at    text,
  disconnected_at text,
  last_status_at  text NOT NULL,
  metadata_json   text,
  created_at      text NOT NULL,
  updated_at      text NOT NULL,
  deleted_at      text,
  PRIMARY KEY (id)
);

-- Resolve a user's live connection (status+health scoped).
CREATE INDEX IF NOT EXISTS github_write_connections_user_idx
  ON github_write_connections (user_id, status, health);
-- Resolve by connection_ref (grant issuance path).
CREATE INDEX IF NOT EXISTS github_write_connections_ref_idx
  ON github_write_connections (connection_ref);

CREATE TABLE IF NOT EXISTS github_write_connection_attempts (
  id                    text NOT NULL,
  user_id               text NOT NULL,
  state                 text NOT NULL, -- CUSTODY: OAuth CSRF / claim nonce.
  expected_github_id    text NOT NULL,
  expected_github_login text NOT NULL,
  redirect_after        text,
  scopes_json           text NOT NULL,
  status                text NOT NULL,
  expires_at            text NOT NULL,
  completed_at          text,
  failed_at             text,
  failure_reason        text,
  created_at            text NOT NULL,
  updated_at            text NOT NULL,
  PRIMARY KEY (id)
);

-- Resolve a pending attempt by its callback state (the OAuth return).
-- D1's UNIQUE(state) is a plain index here; the value is a custody nonce.
CREATE INDEX IF NOT EXISTS github_write_attempts_state_idx
  ON github_write_connection_attempts (state);
-- Recent attempts per user, and expiry sweeps.
CREATE INDEX IF NOT EXISTS github_write_attempts_user_created_idx
  ON github_write_connection_attempts (user_id, created_at);
CREATE INDEX IF NOT EXISTS github_write_attempts_status_expiry_idx
  ON github_write_connection_attempts (status, expires_at);

CREATE TABLE IF NOT EXISTS github_write_auth_grants (
  id                text NOT NULL,
  connection_id     text NOT NULL,
  user_id           text NOT NULL,
  runner_session_id text,
  connection_ref    text NOT NULL,
  secret_ref        text NOT NULL,
  grant_ref         text NOT NULL,
  status            text NOT NULL,
  requested_action  text,
  metadata_json     text,
  created_at        text NOT NULL,
  updated_at        text NOT NULL,
  expires_at        text NOT NULL,
  used_at           text,
  revoked_at        text,
  failed_at         text,
  PRIMARY KEY (id)
);

-- Grant lookups: by user (recent), by runner session (resolution), and by
-- status+expiry (revocation/expiry sweeps). grant_ref UNIQUE → plain here.
CREATE INDEX IF NOT EXISTS github_write_grants_user_created_idx
  ON github_write_auth_grants (user_id, created_at);
CREATE INDEX IF NOT EXISTS github_write_grants_runner_session_idx
  ON github_write_auth_grants (runner_session_id);
CREATE INDEX IF NOT EXISTS github_write_grants_status_expiry_idx
  ON github_write_auth_grants (status, expires_at);
CREATE INDEX IF NOT EXISTS github_write_grants_grant_ref_idx
  ON github_write_auth_grants (grant_ref);

-- --------------------------------------------------------------------------
-- Provider (BYOK) accounts (worker 0009 → rebuilt 0173, + 0044/0046/0048)
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS provider_accounts (
  id                         text NOT NULL,
  user_id                    text NOT NULL,
  team_id                    text,
  provider                   text NOT NULL,
  auth_mode                  text NOT NULL,
  status                     text NOT NULL,
  health                     text NOT NULL,
  provider_account_ref       text NOT NULL,
  secret_ref                 text,
  account_label              text,
  plan_type                  text,
  connected_at               text,
  disconnected_at            text,
  denied_at                  text,
  last_status_at             text NOT NULL,
  metadata_json              text,
  created_at                 text NOT NULL,
  updated_at                 text NOT NULL,
  deleted_at                 text,
  last_sanity_check_at       text,
  last_sanity_check_result   text,
  operator_priority          bigint NOT NULL DEFAULT 100,
  cooldown_until             text,
  low_credit_flag            bigint NOT NULL DEFAULT 0,
  recent_failure_class       text,
  last_selected_at           text,
  operator_label             text,
  lease_limit                bigint NOT NULL DEFAULT 1,
  last_parallel_probe_at     text,
  last_parallel_probe_result text,
  last_successful_launch_at  text,
  last_failed_launch_at      text,
  reauth_required_reason     text,
  operator_note              text,
  refill_note                text,
  PRIMARY KEY (id)
);

-- Account selection scans: per (user, provider), per (team, provider), and
-- pool health scans (status, health). provider_account_ref UNIQUE → plain.
CREATE INDEX IF NOT EXISTS provider_accounts_user_provider_idx
  ON provider_accounts (user_id, provider);
CREATE INDEX IF NOT EXISTS provider_accounts_team_provider_idx
  ON provider_accounts (team_id, provider);
CREATE INDEX IF NOT EXISTS provider_accounts_status_health_idx
  ON provider_accounts (status, health);
CREATE INDEX IF NOT EXISTS provider_accounts_ref_idx
  ON provider_accounts (provider_account_ref);

CREATE TABLE IF NOT EXISTS provider_account_connection_attempts (
  id                  text NOT NULL,
  provider_account_id text NOT NULL,
  user_id             text NOT NULL,
  team_id             text,
  provider            text NOT NULL,
  method              text NOT NULL,
  source              text NOT NULL,
  login_ref           text,
  verification_url    text,
  user_code           text, -- CUSTODY: device-code one-time challenge.
  status              text NOT NULL,
  expires_at          text NOT NULL,
  completed_at        text,
  failed_at           text,
  metadata_json       text,
  created_at          text NOT NULL,
  updated_at          text NOT NULL,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_connection_attempts_user_created_idx
  ON provider_account_connection_attempts (user_id, created_at);
CREATE INDEX IF NOT EXISTS provider_connection_attempts_provider_account_idx
  ON provider_account_connection_attempts (provider_account_id, created_at);
CREATE INDEX IF NOT EXISTS provider_connection_attempts_status_expiry_idx
  ON provider_account_connection_attempts (status, expires_at);

CREATE TABLE IF NOT EXISTS provider_account_auth_grants (
  id                   text NOT NULL,
  provider_account_id  text NOT NULL,
  user_id              text NOT NULL,
  team_id              text,
  thread_id            text,
  workroom_id          text,
  runner_session_id    text,
  provider             text NOT NULL,
  provider_account_ref text NOT NULL,
  provider_secret_ref  text NOT NULL,
  grant_ref            text NOT NULL,
  status               text NOT NULL,
  requested_action     text,
  metadata_json        text,
  created_at           text NOT NULL,
  updated_at           text NOT NULL,
  expires_at           text NOT NULL,
  used_at              text,
  revoked_at           text,
  failed_at            text,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_grants_user_created_idx
  ON provider_account_auth_grants (user_id, created_at);
CREATE INDEX IF NOT EXISTS provider_grants_runner_session_idx
  ON provider_account_auth_grants (runner_session_id);
CREATE INDEX IF NOT EXISTS provider_grants_status_expiry_idx
  ON provider_account_auth_grants (status, expires_at);
CREATE INDEX IF NOT EXISTS provider_grants_grant_ref_idx
  ON provider_account_auth_grants (grant_ref);

CREATE TABLE IF NOT EXISTS provider_account_events (
  id                  text NOT NULL,
  provider_account_id text,
  auth_grant_id       text,
  user_id             text NOT NULL,
  team_id             text,
  thread_id           text,
  workroom_id         text,
  runner_session_id   text,
  kind                text NOT NULL,
  summary             text NOT NULL,
  source_refs_json    text NOT NULL,
  evidence_refs_json  text NOT NULL,
  target_ref          text,
  metadata_json       text,
  actor_id            text,
  created_at          text NOT NULL,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_account_events_user_created_idx
  ON provider_account_events (user_id, created_at);
CREATE INDEX IF NOT EXISTS provider_account_events_target_idx
  ON provider_account_events (target_ref);
CREATE INDEX IF NOT EXISTS provider_account_events_account_created_idx
  ON provider_account_events (provider_account_id, created_at);

CREATE TABLE IF NOT EXISTS provider_account_sanity_checks (
  id                   text NOT NULL,
  provider_account_id  text NOT NULL,
  user_id              text NOT NULL,
  team_id              text,
  provider             text NOT NULL,
  provider_account_ref text NOT NULL,
  classification       text NOT NULL,
  summary              text NOT NULL,
  grant_ref            text,
  created_at           text NOT NULL,
  metadata_json        text,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_account_sanity_checks_account_created_idx
  ON provider_account_sanity_checks (provider_account_id, created_at DESC);
CREATE INDEX IF NOT EXISTS provider_account_sanity_checks_result_created_idx
  ON provider_account_sanity_checks (classification, created_at DESC);

CREATE TABLE IF NOT EXISTS provider_account_parallel_probe_receipts (
  id                   text NOT NULL,
  probe_run_id         text NOT NULL,
  probe_id             text NOT NULL,
  lease_id             text NOT NULL,
  provider_account_id  text NOT NULL,
  user_id              text NOT NULL,
  team_id              text,
  provider_account_ref text NOT NULL,
  started_at           text NOT NULL,
  finished_at          text NOT NULL,
  terminal_status      text NOT NULL,
  classification       text NOT NULL,
  collision_class      text NOT NULL,
  metadata_json        text,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_account_parallel_probe_receipts_run_idx
  ON provider_account_parallel_probe_receipts (probe_run_id, started_at);
CREATE INDEX IF NOT EXISTS provider_account_parallel_probe_receipts_account_idx
  ON provider_account_parallel_probe_receipts (provider_account_id, started_at DESC);

CREATE TABLE IF NOT EXISTS provider_account_leases (
  id                        text NOT NULL,
  lease_ref                 text NOT NULL,
  provider_account_id       text NOT NULL,
  user_id                   text NOT NULL,
  team_id                   text,
  provider                  text NOT NULL,
  provider_account_ref      text NOT NULL,
  requested_action          text NOT NULL,
  run_id                    text,
  assignment_id             text,
  selected_by_policy_version text NOT NULL,
  selection_reason          text NOT NULL,
  status                    text NOT NULL,
  started_at                text NOT NULL,
  expires_at                text NOT NULL,
  released_at               text,
  terminal_outcome          text,
  metadata_json             text,
  order_id                  text,
  selected_by_actor         text,
  last_touched_at           text,
  failure_class             text,
  PRIMARY KEY (id)
);

-- Active-lease scans per account, per-user history, per-order history.
-- lease_ref UNIQUE → plain index here.
CREATE INDEX IF NOT EXISTS provider_account_leases_active_idx
  ON provider_account_leases (provider_account_id, status, expires_at);
CREATE INDEX IF NOT EXISTS provider_account_leases_user_idx
  ON provider_account_leases (user_id, started_at DESC);
CREATE INDEX IF NOT EXISTS provider_account_leases_order_idx
  ON provider_account_leases (order_id, started_at DESC);
CREATE INDEX IF NOT EXISTS provider_account_leases_ref_idx
  ON provider_account_leases (lease_ref);

CREATE TABLE IF NOT EXISTS provider_account_failover_receipts (
  id                            text NOT NULL,
  run_id                        text,
  assignment_id                 text,
  requested_action              text NOT NULL,
  previous_lease_ref            text,
  previous_provider_account_ref text,
  next_lease_ref                text,
  next_provider_account_ref     text,
  failure_class                 text NOT NULL,
  account_state_action          text NOT NULL,
  outcome                       text NOT NULL,
  attempt_number                bigint NOT NULL,
  max_attempts                  bigint NOT NULL,
  customer_safe_status          text NOT NULL,
  created_at                    text NOT NULL,
  metadata_json                 text,
  order_id                      text,
  policy_version                text NOT NULL DEFAULT 'provider-account-lease-policy:v1',
  cooldown_until                text,
  operator_summary              text NOT NULL DEFAULT 'Provider account failover was recorded.',
  customer_safe_summary         text,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_order_idx
  ON provider_account_failover_receipts (order_id, created_at DESC);
CREATE INDEX IF NOT EXISTS provider_account_failover_receipts_created_idx
  ON provider_account_failover_receipts (created_at DESC);

-- --------------------------------------------------------------------------
-- Provider account token custody (worker 0283) — the encrypted vault.
-- Ciphertext + IV + key-id columns are custody-bearing (see header): the
-- twin holds the SAME AES-GCM ciphertext D1 holds, keyed by KMS key id.
-- --------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS provider_account_token_custody (
  provider_account_ref    text NOT NULL,
  owner_user_id           text NOT NULL,
  provider                text NOT NULL,
  secret_ref              text NOT NULL,
  refresh_ciphertext_b64  text NOT NULL, -- CUSTODY
  refresh_iv_b64          text NOT NULL, -- CUSTODY
  refresh_key_id          text NOT NULL, -- CUSTODY
  access_ciphertext_b64   text NOT NULL, -- CUSTODY
  access_iv_b64           text NOT NULL, -- CUSTODY
  access_key_id           text NOT NULL, -- CUSTODY
  access_expires_at       text NOT NULL,
  account_id              text,
  id_token_ciphertext_b64 text, -- CUSTODY
  id_token_iv_b64         text, -- CUSTODY
  id_token_key_id         text, -- CUSTODY
  created_at              text NOT NULL,
  updated_at              text NOT NULL,
  last_refreshed_at       text,
  PRIMARY KEY (provider_account_ref)
);

-- Owner-scoped custody lookup (never selects ciphertext for the scan key).
CREATE INDEX IF NOT EXISTS provider_account_token_custody_owner_idx
  ON provider_account_token_custody (owner_user_id, provider_account_ref);

CREATE TABLE IF NOT EXISTS provider_account_token_custody_audit (
  id                   text NOT NULL,
  provider_account_ref text NOT NULL,
  owner_user_id        text NOT NULL,
  provider             text NOT NULL,
  event_kind           text NOT NULL,
  status               text NOT NULL,
  actor_ref            text,
  source_ref           text,
  error_tag            text,
  error_message        text,
  metadata_json        text,
  created_at           text NOT NULL,
  PRIMARY KEY (id)
);

CREATE INDEX IF NOT EXISTS provider_account_token_custody_audit_owner_idx
  ON provider_account_token_custody_audit (owner_user_id, created_at);
CREATE INDEX IF NOT EXISTS provider_account_token_custody_audit_account_idx
  ON provider_account_token_custody_audit (provider_account_ref, created_at);
