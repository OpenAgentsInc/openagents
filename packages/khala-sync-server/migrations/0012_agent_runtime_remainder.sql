-- KS-8.5 follow-up (#8334): agent runtime remainder tables — Postgres
-- twins for the tables deliberately left out of the first core metadata
-- migration (0010_agent_runtime.sql):
--   agent_profiles, agent_credentials, agent_owner_claims,
--   agent_owner_x_claim_challenges, agent_proposals, event_ledger_entries,
--   khala_acceptance_jobs, khala_acceptance_verdicts.
--
-- Scope note: event_ledger_entries_next was a D1 rewrite artifact from
-- worker migration 0287. The live canonical table is event_ledger_entries;
-- the verifier treats a still-present event_ledger_entries_next table as an
-- artifact that must be empty or absent. There is no Postgres twin for it.
--
-- Type fidelity: keep D1's byte-compatible values for reconciliation.
-- Timestamps remain text, JSON columns remain text, booleans remain
-- smallint/integer-shaped values where D1 stored 0/1, and payload-bearing
-- columns are not converted to jsonb mid-migration.
--
-- Privacy: agent_credentials is secret-bearing. token_hash is a one-way
-- hash but still treated as private diagnostic material. Backfill/verify
-- tooling may hash rows and print row ids/prefixes only; never print raw
-- token_hash values or credential payloads.
--
-- Event ledger invariant: per-owner ordering_sequence remains dense and
-- unique. During dual-write/backfill this migration copies D1's sequence
-- values exactly; the future Postgres write-authority cutover must allocate
-- the next sequence inside the Postgres transaction, never by read-then-
-- insert outside the transaction.
--
-- No foreign keys: dual-write mirrors and backfill can land rows in table
-- order while parent domains are still D1-authoritative. Reconciliation
-- proves integrity before read/write authority moves.

CREATE TABLE IF NOT EXISTS agent_profiles (
  user_id       text NOT NULL PRIMARY KEY,
  slug          text UNIQUE,
  metadata_json text NOT NULL,
  created_at    text NOT NULL,
  updated_at    text NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_profiles_updated_idx
  ON agent_profiles (updated_at DESC);

CREATE TABLE IF NOT EXISTS agent_credentials (
  id               text NOT NULL PRIMARY KEY,
  user_id          text NOT NULL,
  openauth_user_id text,
  token_hash       text NOT NULL UNIQUE,
  token_prefix     text NOT NULL,
  name             text NOT NULL,
  status           text NOT NULL CHECK (status IN ('active', 'revoked')),
  created_at       text NOT NULL,
  last_used_at     text,
  revoked_at       text,
  expires_at       text
);

CREATE INDEX IF NOT EXISTS agent_credentials_user_status_idx
  ON agent_credentials (user_id, status);
CREATE INDEX IF NOT EXISTS agent_credentials_active_expiry_idx
  ON agent_credentials (status, expires_at);
CREATE INDEX IF NOT EXISTS agent_credentials_openauth_user_idx
  ON agent_credentials (openauth_user_id, status, revoked_at);

CREATE TABLE IF NOT EXISTS agent_owner_claims (
  id                 text NOT NULL PRIMARY KEY,
  claim_token_hash   text NOT NULL UNIQUE,
  claim_token_prefix text NOT NULL,
  status             text NOT NULL CHECK (
    status IN ('pending', 'approved', 'rejected', 'expired', 'revoked')
  ),
  display_name       text NOT NULL,
  slug               text,
  external_id        text,
  primary_email      text,
  metadata_json      text NOT NULL DEFAULT '{}',
  owner_user_id      text,
  agent_user_id      text,
  credential_id      text,
  token_prefix       text,
  receipt_ref        text NOT NULL UNIQUE,
  requested_at       text NOT NULL,
  expires_at         text NOT NULL,
  decided_at         text,
  token_issued_at    text,
  rejected_reason    text,
  created_at         text NOT NULL,
  updated_at         text NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_owner_claims_status_expires_idx
  ON agent_owner_claims (status, expires_at);
CREATE INDEX IF NOT EXISTS agent_owner_claims_owner_status_idx
  ON agent_owner_claims (owner_user_id, status);
CREATE INDEX IF NOT EXISTS agent_owner_claims_agent_user_idx
  ON agent_owner_claims (agent_user_id);

CREATE TABLE IF NOT EXISTS agent_owner_x_claim_challenges (
  id                 text NOT NULL PRIMARY KEY,
  agent_claim_id     text NOT NULL,
  owner_user_id      text NOT NULL,
  agent_user_id      text,
  x_account_ref      text NOT NULL,
  x_handle           text NOT NULL,
  nonce              text NOT NULL UNIQUE,
  required_text      text NOT NULL,
  required_url       text NOT NULL,
  state              text NOT NULL CHECK (
    state IN (
      'pending_owner_session',
      'pending_x_connection',
      'pending_tweet',
      'verified',
      'approved',
      'rejected',
      'expired',
      'revoked'
    )
  ),
  receipt_ref        text NOT NULL UNIQUE,
  tweet_ref          text,
  tweet_url          text,
  policy_refs_json   text NOT NULL DEFAULT '[]',
  caveat_refs_json   text NOT NULL DEFAULT '[]',
  rejected_reason    text,
  created_at         text NOT NULL,
  expires_at         text NOT NULL,
  verified_at        text,
  updated_at         text NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_active_claim_idx
  ON agent_owner_x_claim_challenges (agent_claim_id)
  WHERE state IN (
    'pending_owner_session',
    'pending_x_connection',
    'pending_tweet',
    'verified',
    'approved'
  );
CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_verified_account_idx
  ON agent_owner_x_claim_challenges (x_account_ref)
  WHERE state IN ('verified', 'approved');
CREATE UNIQUE INDEX IF NOT EXISTS agent_owner_x_claim_verified_tweet_idx
  ON agent_owner_x_claim_challenges (tweet_ref)
  WHERE state IN ('verified', 'approved') AND tweet_ref IS NOT NULL;
CREATE INDEX IF NOT EXISTS agent_owner_x_claim_owner_state_idx
  ON agent_owner_x_claim_challenges (owner_user_id, state, updated_at DESC);

CREATE TABLE IF NOT EXISTS agent_proposals (
  id                      text NOT NULL PRIMARY KEY,
  receipt_ref             text NOT NULL UNIQUE,
  status                  text NOT NULL CHECK (status IN ('pending', 'rejected', 'promoted')),
  kind                    text NOT NULL CHECK (
    kind IN (
      'site_improvement',
      'public_proof_note',
      'forum_topic_draft',
      'order_request_draft',
      'workroom_artifact_draft',
      'other'
    )
  ),
  title                   text NOT NULL,
  summary                 text NOT NULL,
  body_text               text NOT NULL,
  source_urls_json        text NOT NULL,
  target_json             text NOT NULL,
  author_json             text NOT NULL,
  client_fingerprint_hash text NOT NULL,
  idempotency_key_hash    text NOT NULL UNIQUE,
  promotion_kind          text,
  promoted_target_ref     text,
  operator_note           text,
  operator_user_id        text,
  decided_at              text,
  created_at              text NOT NULL,
  updated_at              text NOT NULL
);

CREATE INDEX IF NOT EXISTS agent_proposals_status_created_idx
  ON agent_proposals (status, created_at DESC);
CREATE INDEX IF NOT EXISTS agent_proposals_client_created_idx
  ON agent_proposals (client_fingerprint_hash, created_at DESC);

CREATE TABLE IF NOT EXISTS event_ledger_entries (
  entry_id                 text NOT NULL PRIMARY KEY,
  owner_agent_user_id      text NOT NULL,
  owner_ref                text NOT NULL,
  source                   text NOT NULL CHECK (source IN ('github', 'slack')),
  external_ref             text NOT NULL,
  actor_ref                text NOT NULL,
  content_ref              text NOT NULL,
  subject_ref              text NOT NULL,
  event_type               text NOT NULL,
  source_refs_json         text NOT NULL,
  payload_summary_json     text NOT NULL,
  occurred_at              text NOT NULL,
  received_at              text NOT NULL,
  ordering_key             text NOT NULL,
  ordering_sequence        bigint NOT NULL CHECK (ordering_sequence >= 1),
  handled_state            text NOT NULL DEFAULT 'open'
    CHECK (handled_state IN ('open', 'handled', 'responded', 'ignored')),
  handled_by_run_id        text,
  handled_by_definition_id text,
  handled_at               text,
  handled_reason_ref       text,
  training_consent         smallint NOT NULL DEFAULT 0 CHECK (training_consent = 0),
  created_at               text NOT NULL,
  updated_at               text NOT NULL,
  UNIQUE (owner_agent_user_id, source, external_ref),
  UNIQUE (owner_agent_user_id, ordering_sequence)
);

CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_received
  ON event_ledger_entries (owner_agent_user_id, received_at DESC);
CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_source_subject
  ON event_ledger_entries (owner_agent_user_id, source, subject_ref);
CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_owner_state_sequence
  ON event_ledger_entries (owner_agent_user_id, handled_state, ordering_sequence);
CREATE INDEX IF NOT EXISTS idx_event_ledger_entries_handled_run
  ON event_ledger_entries (owner_agent_user_id, handled_by_run_id);

CREATE TABLE IF NOT EXISTS khala_acceptance_jobs (
  request_id       text NOT NULL PRIMARY KEY,
  status           text NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending', 'leased')),
  job_payload      text NOT NULL,
  lease_id         text,
  lease_expires_at text,
  attempts         bigint NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  created_at       text NOT NULL,
  updated_at       text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_acceptance_jobs_lease
  ON khala_acceptance_jobs (status, lease_expires_at, created_at);

CREATE TABLE IF NOT EXISTS khala_acceptance_verdicts (
  request_id               text NOT NULL PRIMARY KEY,
  verification             text NOT NULL,
  verified                 smallint NOT NULL DEFAULT 0 CHECK (verified IN (0, 1)),
  executed                 smallint NOT NULL DEFAULT 0 CHECK (executed IN (0, 1)),
  scalar_reward            double precision NOT NULL DEFAULT 0,
  rubric_ref               text NOT NULL,
  passed_checks            text NOT NULL DEFAULT '[]',
  failed_checks            text NOT NULL DEFAULT '[]',
  verification_receipt_ref text NOT NULL,
  version                  bigint NOT NULL DEFAULT 1,
  updated_at               text NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_acceptance_verdicts_state
  ON khala_acceptance_verdicts (verification, updated_at DESC);
