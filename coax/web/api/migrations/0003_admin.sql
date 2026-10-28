-- Admin sign-in for /admin/analytics (admin.c): until the phone sign-in
-- exists, an admin signs a NIP-98 request with their Nostr key (sign.c). A
-- signed event is good once; a sign-in keeps a session whose token only the
-- browser holds (its SHA-256 is here).
CREATE TABLE analytics.admin_events (
  id text PRIMARY KEY,         -- the signed event's id: never accepted twice
  pubkey text NOT NULL,
  ms bigint NOT NULL
);
CREATE TABLE analytics.admin_sessions (
  token_sha256 text PRIMARY KEY,
  pubkey text NOT NULL,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL
);
-- The reports read sessions by their events' time and space.
CREATE INDEX events_space_ms ON analytics.events (space, ms);
