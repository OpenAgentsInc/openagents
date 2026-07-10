-- 0315_portal_engagements_and_content_items.sql
--
-- PORTAL-1 (#8652): client portal on openagents.com.
--
-- The Sell-in-Public revenue loop (docs/transcripts/247.md) makes Autopilot
-- Lead Gen the front-of-funnel product. Design partners onboard into an
-- ENGAGEMENT: a named client relationship whose agent-drafted content items
-- (A/B post pairs on a channel cadence) await client approve/reject in the
-- /portal surface. These two tables are the typed engagement model:
--
--   * `portal_engagements` — one row per client engagement. Client identity
--     binds through `client_user_id` (an OpenAuth user id, authoritative once
--     set) and/or `client_email` (pre-login binding so an operator can create
--     the engagement before the client's first sign-in; matched
--     case-insensitively against the verified session email). Owner-scoped
--     fail-closed: portal reads resolve ONLY through the caller's own session
--     identity — there is no engagement-id lookup route for clients.
--
--   * `portal_content_items` — agent-drafted content awaiting client
--     decision. `pair_ref` groups the two A/B variants of one planned post.
--     `state` walks draft -> approved|rejected (client decision, receipt
--     minted) -> published (follow-on publishing lane, not v1). Decisions are
--     immutable once made: `decided_at` + unique `decision_receipt_ref` are
--     set exactly once (`portal_content_decision:<opaque id>`), following the
--     0308 `admin_credit_grants.credit_receipt_ref` receipt precedent.
--
-- LIVE storage is Cloud SQL Postgres: post D1-evacuation (#8515) the Cloud
-- Run monolith serves OPENAGENTS_DB through the D1-shaped Postgres adapter,
-- and the account-capped Cloudflare D1 databases no longer accept DDL. The
-- Postgres twin of this schema is
-- packages/khala-sync-server/migrations/0058_portal_engagements_and_content_items.sql;
-- this SQLite file remains the schema the portal route/store tests load
-- in-memory (and the historical D1 record if the bridge ever revives).

CREATE TABLE IF NOT EXISTS portal_engagements (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  status TEXT NOT NULL DEFAULT 'preparing'
    CHECK (status IN ('preparing', 'active', 'paused', 'closed')),
  client_user_id TEXT,
  client_email TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS portal_engagements_client_user_idx
  ON portal_engagements(client_user_id);

CREATE INDEX IF NOT EXISTS portal_engagements_client_email_idx
  ON portal_engagements(client_email);

CREATE TABLE IF NOT EXISTS portal_content_items (
  id TEXT PRIMARY KEY NOT NULL,
  engagement_id TEXT NOT NULL
    REFERENCES portal_engagements(id) ON DELETE CASCADE,
  kind TEXT NOT NULL DEFAULT 'post'
    CHECK (kind IN ('post', 'email', 'ad')),
  channel TEXT NOT NULL,
  variant TEXT NOT NULL DEFAULT 'a'
    CHECK (variant IN ('a', 'b')),
  pair_ref TEXT,
  title TEXT NOT NULL,
  body TEXT NOT NULL,
  state TEXT NOT NULL DEFAULT 'draft'
    CHECK (state IN ('draft', 'approved', 'rejected', 'published')),
  decided_at TEXT,
  decision_receipt_ref TEXT UNIQUE,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS portal_content_items_engagement_idx
  ON portal_content_items(engagement_id, created_at DESC);

CREATE INDEX IF NOT EXISTS portal_content_items_pair_idx
  ON portal_content_items(pair_ref);
