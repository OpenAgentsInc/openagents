-- The OpenAuth owner-link domain now writes through the Cloud Run Postgres
-- adapter. Migration 0028 deliberately omitted D1's secondary unique while
-- Postgres was only a fail-soft mirror; the write cutover requires restoring
-- that exact conflict identity so owner-approved link upserts stay idempotent.
--
-- A nullable credential remains intentionally distinct under both SQLite and
-- Postgres uniqueness semantics. Credential-bearing Pylon links, including
-- Sarah's owner-scoped coding capacity, are exact triples.

CREATE UNIQUE INDEX IF NOT EXISTS openauth_agent_links_owner_agent_credential_uidx
  ON openauth_agent_links (
    openauth_user_id,
    agent_user_id,
    agent_credential_id
  );
