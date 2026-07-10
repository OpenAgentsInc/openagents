-- PORTAL-1 (#8652): Postgres twin of the client-portal engagement tables
-- (worker D1 migration 0315_portal_engagements_and_content_items.sql).
--
-- Post D1-evacuation (#8515) the Cloud Run monolith serves OPENAGENTS_DB off
-- Cloud SQL Postgres through the D1-shaped adapter (makePostgresD1Database),
-- so this twin is the LIVE storage for portal-store.ts; the worker D1 file
-- remains the SQLite schema the route/store tests load in-memory.
--
-- TYPE FIDELITY (mirrors worker migration 0315): text ISO-8601 timestamps,
-- opaque text ids, CHECK-constrained status/kind/variant/state enums, the
-- UNIQUE immutable decision receipt ref, and the client identity binding
-- (authoritative client_user_id, or pre-login client_email matched
-- case-insensitively after normalization at the store boundary). No payment
-- material, no publishing authority.

CREATE TABLE IF NOT EXISTS portal_engagements (
  id             text PRIMARY KEY NOT NULL,
  name           text NOT NULL,
  status         text NOT NULL DEFAULT 'preparing'
    CHECK (status IN ('preparing', 'active', 'paused', 'closed')),
  client_user_id text,
  client_email   text,
  created_at     text NOT NULL,
  updated_at     text NOT NULL
);

CREATE INDEX IF NOT EXISTS portal_engagements_client_user_idx
  ON portal_engagements(client_user_id);

CREATE INDEX IF NOT EXISTS portal_engagements_client_email_idx
  ON portal_engagements(client_email);

CREATE TABLE IF NOT EXISTS portal_content_items (
  id                   text PRIMARY KEY NOT NULL,
  engagement_id        text NOT NULL
    REFERENCES portal_engagements(id) ON DELETE CASCADE,
  kind                 text NOT NULL DEFAULT 'post'
    CHECK (kind IN ('post', 'email', 'ad')),
  channel              text NOT NULL,
  variant              text NOT NULL DEFAULT 'a'
    CHECK (variant IN ('a', 'b')),
  pair_ref             text,
  title                text NOT NULL,
  body                 text NOT NULL,
  state                text NOT NULL DEFAULT 'draft'
    CHECK (state IN ('draft', 'approved', 'rejected', 'published')),
  decided_at           text,
  decision_receipt_ref text UNIQUE,
  created_at           text NOT NULL,
  updated_at           text NOT NULL
);

CREATE INDEX IF NOT EXISTS portal_content_items_engagement_idx
  ON portal_content_items(engagement_id, created_at DESC);

CREATE INDEX IF NOT EXISTS portal_content_items_pair_idx
  ON portal_content_items(pair_ref);
