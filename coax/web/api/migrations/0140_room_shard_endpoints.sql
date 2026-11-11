-- The shard directory's endpoints and health (#13179, docs/deployment/room-vm.md).
--
-- A room server on the room VM names its endpoint and its health in each directory report
-- (coax/web/room/room-directory.coax, with OA_ROOM_ENDPOINT set):
--   e wss://room.openagents.com/api/room/s1/     where its pages open their socket
--   h PLAYERS TICK_P95_US                        its seats and its tick's 95th percentile
-- and reports every 5 s while it runs, rooms or none. rooms.c takes those lines out before
-- room_report (0133) reads the rest, and once that report is taken, room_shard_seen keeps them here.
-- A report without an e line (a Cloud Run room) clears the endpoint: its pages use the load balancer's
-- /api/room/sK/ as before.
--
-- A shard is live, draining (0133) or down. Down: it has an endpoint and hasn't reported for 15 s, so it
-- died or its VM did. room_fits is false for a down shard: no new instance, reconnect or asked instance
-- goes there, and its players' pages, asking the directory again, are placed on a live shard; the
-- room restores them from the API's store. After 120 s without a report its endpoint is forgotten
-- (the shard counts as one whose host says nothing, as Cloud Run's idle shards are): a shard that is
-- gone for good, or rooms moved back to Cloud Run (vm-deploy.sh switch run), never strands placement.
-- Adding a shard is a row here (INSERT INTO verse.room_shards (shard, rooms_cap) VALUES (7, 3)) and a
-- unit on a VM that names its endpoint.
-- Additive, and safe to run twice.
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS endpoint text;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS players int;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS tick_us int;
ALTER TABLE verse.room_shards ADD COLUMN IF NOT EXISTS seen_at bigint;

-- What a shard's taken report said beside its rooms: its endpoint (NULL: none named) and health.
CREATE OR REPLACE FUNCTION verse.room_shard_seen(k int, ep text, p int, t int) RETURNS void LANGUAGE sql AS $$
  UPDATE verse.room_shards SET endpoint = ep, players = p, tick_us = t, seen_at = verse.room_now() WHERE shard = k $$;

-- live, draining or down (see above); a shard whose endpoint is forgotten is live.
CREATE OR REPLACE FUNCTION verse.room_down(k int, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.room_shards s
                 WHERE s.shard = k AND s.endpoint IS NOT NULL
                   AND coalesce(s.seen_at, 0) <= now - 15000 AND coalesce(s.seen_at, 0) > now - 120000) $$;

CREATE OR REPLACE FUNCTION verse.room_state(k int, now bigint) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN verse.room_down(k, now) THEN 'down' WHEN verse.room_draining(k, now) THEN 'draining' ELSE 'live' END $$;

-- The endpoint a page opens for shard k: a live shard's, while it reports (NULL: the load balancer's path).
CREATE OR REPLACE FUNCTION verse.room_endpoint(k int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.endpoint FROM verse.room_shards s
  WHERE s.shard = k AND s.endpoint IS NOT NULL AND coalesce(s.seen_at, 0) > verse.room_now() - 15000 $$;

-- room_fits (0133) and not down.
CREATE OR REPLACE FUNCTION verse.room_fits(k int, bld text, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT NOT verse.room_draining(k, now) AND NOT verse.room_down(k, now) AND (coalesce(bld, '') = '' OR NOT EXISTS (
    SELECT 1 FROM verse.room_shards s WHERE s.shard = k AND s.build IS NOT NULL AND s.build_at > now - 60000 AND s.build <> bld)) $$;

-- The directory's shards, for GET /api/playtest/room-shards: each one's state, endpoint and health.
CREATE OR REPLACE FUNCTION verse.room_shard_list() RETURNS text LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT verse.room_now() AS t)
  SELECT json_build_object('shards', coalesce(json_agg(json_build_object(
    'shard', s.shard, 'state', verse.room_state(s.shard, clk.t),
    'endpoint', CASE WHEN coalesce(s.seen_at, 0) > clk.t - 120000 THEN s.endpoint END,
    'players', s.players, 'tick_us', s.tick_us,
    'seen_ms_ago', CASE WHEN s.seen_at IS NULL THEN NULL ELSE clk.t - s.seen_at END,
    'instances', (SELECT count(*) FROM verse.room_instances i WHERE i.shard = s.shard)) ORDER BY s.shard), '[]'::json))::text
  FROM verse.room_shards s, clk $$;
