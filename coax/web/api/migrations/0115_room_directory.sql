-- The Everglade instance directory (#12717, epic #12718; spec sections 3.4 and 9): which oa-room
-- shard holds each instance, who sits in it, and where a new one goes. A shard is one Cloud Run
-- service of at most one container (oa-room is shard 1, oa-room-K shard K), so an instance's world
-- lives in one process; the load balancer sends /api/room/sK/ to shard K.
--   - room_shards: each shard, how many instances placement puts on it (rooms_cap: instances are
--     packed onto the lowest shard with room, so another container runs only when it is needed),
--     and the simulations it last said it had free.
--   - room_instances: written by the shards' reports (POST /api/playtest/room-directory, signed with
--     the seat-ticket key, coax/web/room/room-directory.coax) and by placement.
--   - room_holds: a seat held from the ticket until the shard reports the player sitting (20 s), so
--     sixteen players at once fill one instance and the seventeenth gets the next.
-- Times are wall-clock milliseconds. Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.room_shards (
  shard int PRIMARY KEY CHECK (shard BETWEEN 1 AND 99),
  rooms_cap int NOT NULL,
  free int, at bigint
);
-- Six shards of three instances (48 players) each: on Cloud Run's 1 vCPU one process's 30 Hz tick fits
-- about three full rooms (2026-11-03: four rooms took 78% of the vCPU and the tick fell to 28 Hz).
INSERT INTO verse.room_shards (shard, rooms_cap) VALUES (1, 3), (2, 3), (3, 3), (4, 3), (5, 3), (6, 3) ON CONFLICT (shard) DO NOTHING;

CREATE TABLE IF NOT EXISTS verse.room_instances (
  code text PRIMARY KEY,
  realm text NOT NULL,
  n int,                                   -- N of a public instance REALM-N, else NULL (a named one)
  shard int NOT NULL,
  players int NOT NULL DEFAULT 0,
  pids text[] NOT NULL DEFAULT '{}',
  made_at bigint NOT NULL,
  at bigint                                -- its shard's last report naming it
);
CREATE INDEX IF NOT EXISTS room_instances_realm ON verse.room_instances (realm, n);

CREATE TABLE IF NOT EXISTS verse.room_holds (
  code text NOT NULL, pid text NOT NULL, until_ms bigint NOT NULL,
  PRIMARY KEY (code, pid)
);

CREATE OR REPLACE FUNCTION verse.room_now() RETURNS bigint LANGUAGE sql VOLATILE AS $$
  SELECT (extract(epoch FROM clock_timestamp()) * 1000)::bigint $$;

-- Whether an instance is live: its shard named it within 15 s, it was placed within 25 s (its
-- first player is on the way), or a seat in it is held.
CREATE OR REPLACE FUNCTION verse.room_live(i verse.room_instances, now bigint) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce(i.at, 0) > now - 15000 OR i.made_at > now - 25000
      OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.until_ms > now) $$;

-- How many seats an instance has taken: its reported players, the held seats of players not yet
-- among them.
CREATE OR REPLACE FUNCTION verse.room_taken(i verse.room_instances, now bigint) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT i.players + (SELECT count(*)::int FROM verse.room_holds h
                      WHERE h.code = i.code AND h.until_ms > now AND NOT (h.pid = ANY (i.pids))) $$;

-- Drops what has gone: expired holds, instances no shard has named for 15 s.
CREATE OR REPLACE FUNCTION verse.room_sweep(now bigint) RETURNS void LANGUAGE sql AS $$
  DELETE FROM verse.room_holds WHERE until_ms <= now;
  DELETE FROM verse.room_instances i WHERE NOT verse.room_live(i, now);
$$;

-- The shard a new instance goes on: the lowest under its cap whose simulations aren't all in use
-- (a shard that hasn't reported for 15 s is idle: scaled to zero, all free; instances placed on it
-- and not yet reported take simulations its report doesn't count yet). NULL when every shard is full.
CREATE OR REPLACE FUNCTION verse.room_place(now bigint) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT s.shard FROM verse.room_shards s
  WHERE s.rooms_cap > (SELECT count(*) FROM verse.room_instances i WHERE i.shard = s.shard)
    AND (s.free IS NULL OR coalesce(s.at, 0) <= now - 15000
         OR s.free > (SELECT count(*) FROM verse.room_instances i WHERE i.shard = s.shard AND i.at IS NULL))
  ORDER BY s.shard LIMIT 1 $$;

-- Where a player sits: the instance ASKED of REALM (ASKED = REALM: the lowest public instance with a
-- free seat, else a new one), for player PID. A named instance that doesn't exist yet is made; one
-- that is full falls back to the pick (was_full true). The player already sitting or held in an instance of
-- the realm keeps it on a pick (a reconnect). The seat is held 20 s. NULL code: every shard is full.
CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats int,
                                           OUT code text, OUT shard int, OUT was_full boolean)
LANGUAGE plpgsql AS $$
DECLARE
  now bigint := verse.room_now();
  i verse.room_instances;
  k int;
BEGIN
  was_full := false;
  PERFORM pg_advisory_xact_lock(7731028);
  PERFORM verse.room_sweep(now);
  IF asked <> realm THEN
    SELECT * INTO i FROM verse.room_instances x WHERE x.code = asked;
    IF FOUND AND (pid = ANY (i.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = room_pick.pid)
                  OR verse.room_taken(i, now) < seats) THEN
      code := i.code; shard := i.shard;
    ELSIF FOUND THEN
      was_full := true;
    ELSE
      k := verse.room_place(now);
      IF k IS NULL THEN RETURN; END IF;
      INSERT INTO verse.room_instances (code, realm, n, shard, made_at)
      VALUES (asked, realm, CASE WHEN asked ~ ('^' || realm || '-[1-9][0-9]{0,2}$') THEN substr(asked, length(realm) + 2)::int END, k, now);
      code := asked; shard := k;
    END IF;
  END IF;
  IF code IS NULL THEN
    -- a reconnect keeps its instance
    SELECT x.code, x.shard INTO code, shard FROM verse.room_instances x
    WHERE x.realm = room_pick.realm AND x.n IS NOT NULL
      AND (pid = ANY (x.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = x.code AND h.pid = room_pick.pid AND h.until_ms > now))
    ORDER BY x.n LIMIT 1;
  END IF;
  IF code IS NULL THEN
    SELECT x.code, x.shard INTO code, shard FROM verse.room_instances x
    WHERE x.realm = room_pick.realm AND x.n IS NOT NULL AND verse.room_taken(x, now) < seats
    ORDER BY x.n LIMIT 1;
  END IF;
  IF code IS NULL THEN
    k := verse.room_place(now);
    IF k IS NULL THEN RETURN; END IF;
    SELECT min(g) INTO code FROM generate_series(1, 999) g
    WHERE NOT EXISTS (SELECT 1 FROM verse.room_instances x WHERE x.realm = room_pick.realm AND x.n = g)
      AND NOT EXISTS (SELECT 1 FROM verse.room_instances x WHERE x.code = room_pick.realm || '-' || g);
    IF code IS NULL THEN RETURN; END IF;
    INSERT INTO verse.room_instances (code, realm, n, shard, made_at) VALUES (realm || '-' || code, realm, code::int, k, now);
    code := realm || '-' || code; shard := k;
  END IF;
  INSERT INTO verse.room_holds (code, pid, until_ms) VALUES (code, pid, now + 20000)
  ON CONFLICT ON CONSTRAINT room_holds_pkey DO UPDATE SET until_ms = EXCLUDED.until_ms;
END $$;

-- A shard's report (coax/web/room/room-directory.coax), its MAC already checked:
--   t MS / s SHARD FREE / r CODE PLAYERS PID ... per room
-- Its rooms are kept as it says (a room another shard had is this one's now: the report is what runs);
-- this shard's instances it no longer names go, unless placed in the last 25 s or a seat in one is held.
-- The number of rooms it named, or -1 when the report is malformed or stale.
CREATE OR REPLACE FUNCTION verse.room_report(body text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  now bigint := verse.room_now();
  lines text[] := string_to_array(rtrim(body, E'\n'), E'\n');
  f text[]; at bigint; sh int; fr int; named text[] := '{}'; ln text; m int := 0;
BEGIN
  IF array_length(lines, 1) IS NULL OR array_length(lines, 1) < 2 THEN RETURN -1; END IF;
  IF lines[1] !~ '^t [0-9]{1,15}$' OR lines[2] !~ '^s [1-9][0-9]? [0-9]{1,3}$' THEN RETURN -1; END IF;
  at := split_part(lines[1], ' ', 2)::bigint;
  IF at < now - 120000 OR at > now + 120000 THEN RETURN -1; END IF;
  sh := split_part(lines[2], ' ', 2)::int; fr := split_part(lines[2], ' ', 3)::int;
  PERFORM pg_advisory_xact_lock(7731028);
  INSERT INTO verse.room_shards (shard, rooms_cap, free, at) VALUES (sh, 0, fr, at)
  ON CONFLICT (shard) DO UPDATE SET free = EXCLUDED.free, at = EXCLUDED.at
  WHERE verse.room_shards.at IS NULL OR verse.room_shards.at <= EXCLUDED.at;
  FOREACH ln IN ARRAY lines[3:] LOOP
    IF ln !~ '^r [a-z0-9-]{1,24} [0-9]{1,3}( [a-z0-9-]{1,40})*$' THEN RETURN -1; END IF;
    f := string_to_array(ln, ' ');
    named := named || f[2];
    INSERT INTO verse.room_instances AS x (code, realm, n, shard, players, pids, made_at, at)
    VALUES (f[2], coalesce(substring(f[2] FROM '^(.*)-[1-9][0-9]{0,2}$'), f[2]),
            substring(f[2] FROM '-([1-9][0-9]{0,2})$')::int, sh, f[3]::int, coalesce(f[4:], '{}'), now, at)
    ON CONFLICT (code) DO UPDATE SET shard = EXCLUDED.shard, players = EXCLUDED.players, pids = EXCLUDED.pids, at = EXCLUDED.at
    WHERE x.at IS NULL OR x.at <= EXCLUDED.at;
    DELETE FROM verse.room_holds h WHERE h.code = f[2] AND h.pid = ANY (f[4:]);
    m := m + 1;
  END LOOP;
  DELETE FROM verse.room_instances i WHERE i.shard = sh AND NOT (i.code = ANY (named))
    AND (i.at IS NOT NULL OR i.made_at <= now - 25000)
    AND NOT EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.until_ms > now);
  RETURN m;
END $$;

-- GET /api/playtest/rooms/REALM: its public instances, lowest first, and which a newcomer joins
-- (the lowest with a free seat, else the next number), as the room server used to answer it, each
-- instance's players its taken seats (the seated and the held):
--   {"realm":R,"game":"everglade","seats":16,"join":"R-K","rooms":[{"code":"R-1","players":3,"shard":1},...]}
-- With PID (?with=, spec 8.1, #12716): the instance that player sits in, while it has a free seat, is
-- "join", and "with" names it (no "with" when the player sits nowhere, or the instance is full).
CREATE OR REPLACE FUNCTION verse.room_list(realm text, seats int, pid text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT verse.room_now() AS t),
  live AS (SELECT i.*, verse.room_taken(i, clk.t) AS taken FROM verse.room_instances i, clk
           WHERE i.realm = room_list.realm AND verse.room_live(i, clk.t)),
  pub AS (SELECT * FROM live WHERE n IS NOT NULL),
  j AS (SELECT coalesce((SELECT min(n) FROM pub WHERE taken < seats),
                        (SELECT min(g) FROM generate_series(1, 999) g WHERE NOT EXISTS (SELECT 1 FROM pub WHERE pub.n = g))) AS n),
  rooms AS (SELECT coalesce(string_agg(format('{"code":%s,"players":%s,"shard":%s}', to_json(code), taken, shard), ',' ORDER BY n), '') AS a FROM pub),
  w AS (SELECT (SELECT code FROM live WHERE pid = ANY (live.pids) AND taken < seats ORDER BY n NULLS LAST LIMIT 1) AS code)
  SELECT format('{"realm":%s,"game":"everglade","seats":%s,"join":%s,"rooms":[%s]%s}', to_json(realm), seats,
                to_json(coalesce(w.code, realm || '-' || j.n)), rooms.a,
                CASE WHEN w.code IS NULL THEN '' ELSE ',"with":' || to_json(w.code)::text END)
  FROM j, rooms, w $$;
