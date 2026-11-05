-- An INVITE holds a seat in the inviter's room (#12795; epic #12718; docs/research/mmo.md sections
-- 15 and 16). The page's INVITE copies a link to the room with ?with=INVITER and asks
-- POST /api/playtest/room-invite/ROOM (player.c), which calls verse.room_invite: one seat in that
-- room is held for the inviter's friend for two minutes, as a row of verse.room_holds whose pid is
-- 'i-' || INVITER. It counts against the room's seats as any hold does (verse.room_taken), so a
-- sixteenth stranger is placed elsewhere while it is held. One invite a player: a press again moves
-- it to the room they are in now and starts its two minutes again.
-- A join that names the inviter (the ticket's ?with=, or the list's ?with=) takes the held seat:
-- verse.room_pick turns it into the joiner's own hold, so it is claimed once. When the inviter's room
-- is full and nothing is held, room_pick says was_full and the page says so before it joins another.
-- Additive, and safe to run twice.

CREATE OR REPLACE FUNCTION verse.room_invite_ms() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 120000::bigint $$;

-- The invite's seat in room ASKED for its friends, by player BY_PID, who must sit in it (reported
-- by its shard, or held for them on the way in). Its end (wall-clock ms), or why there is none:
-- 'not in it', 'full'.
CREATE OR REPLACE FUNCTION verse.room_invite(asked text, by_pid text, seats int, OUT until_ms bigint, OUT why text)
LANGUAGE plpgsql AS $$
DECLARE
  now bigint := verse.room_now();
  i verse.room_instances;
  tag text := 'i-' || by_pid;
BEGIN
  PERFORM pg_advisory_xact_lock(7731028);
  PERFORM verse.room_sweep(now);
  SELECT * INTO i FROM verse.room_instances x WHERE x.code = asked;
  IF NOT FOUND OR NOT (by_pid = ANY (i.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = asked AND h.pid = by_pid)) THEN
    why := 'not in it'; RETURN;
  END IF;
  DELETE FROM verse.room_holds h WHERE h.pid = tag AND h.code <> asked;
  IF NOT EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = asked AND h.pid = tag) AND verse.room_taken(i, now) >= seats THEN
    why := 'full'; RETURN;
  END IF;
  until_ms := now + verse.room_invite_ms();
  INSERT INTO verse.room_holds (code, pid, until_ms) VALUES (asked, tag, until_ms)
  ON CONFLICT ON CONSTRAINT room_holds_pkey DO UPDATE SET until_ms = EXCLUDED.until_ms;
END $$;

-- Where a player sits, as the four-argument room_pick (0115), and VIA: the player whose invite
-- brought them ('' none). A join of VIA's room takes the seat VIA's invite holds there, full or not.
CREATE OR REPLACE FUNCTION verse.room_pick(realm text, asked text, pid text, seats int, via text,
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
    IF FOUND AND (pid = ANY (i.pids) OR EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = room_pick.pid)) THEN
      code := i.code; shard := i.shard;
    ELSIF FOUND AND coalesce(via, '') <> '' AND via <> pid
          AND EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = 'i-' || via AND h.until_ms > now) THEN
      -- the invite's seat: this player's now (their own hold below), and no one else's
      DELETE FROM verse.room_holds h WHERE h.code = i.code AND h.pid = 'i-' || via;
      code := i.code; shard := i.shard;
    ELSIF FOUND AND verse.room_taken(i, now) < seats THEN
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

-- GET /api/playtest/rooms/REALM, as 0115's, but with PID the instance that player sits in is "with"
-- (and "join") while it has a free seat or a seat their invite holds; when it is full with none held,
-- "with_full" names it, so the page can say so rather than join another without a word.
CREATE OR REPLACE FUNCTION verse.room_list(realm text, seats int, pid text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH clk AS (SELECT verse.room_now() AS t),
  live AS (SELECT i.*, verse.room_taken(i, clk.t) AS taken,
                  EXISTS (SELECT 1 FROM verse.room_holds h WHERE h.code = i.code AND h.pid = 'i-' || room_list.pid AND h.until_ms > clk.t) AS invited
           FROM verse.room_instances i, clk
           WHERE i.realm = room_list.realm AND verse.room_live(i, clk.t)),
  pub AS (SELECT * FROM live WHERE n IS NOT NULL),
  j AS (SELECT coalesce((SELECT min(n) FROM pub WHERE taken < seats),
                        (SELECT min(g) FROM generate_series(1, 999) g WHERE NOT EXISTS (SELECT 1 FROM pub WHERE pub.n = g))) AS n),
  rooms AS (SELECT coalesce(string_agg(format('{"code":%s,"players":%s,"shard":%s}', to_json(code), taken, shard), ',' ORDER BY n), '') AS a FROM pub),
  w AS (SELECT (SELECT code FROM live WHERE pid = ANY (live.pids) AND (taken < seats OR invited) ORDER BY n NULLS LAST LIMIT 1) AS code,
               (SELECT code FROM live WHERE pid = ANY (live.pids) AND taken >= seats AND NOT invited ORDER BY n NULLS LAST LIMIT 1) AS full)
  SELECT format('{"realm":%s,"game":"everglade","seats":%s,"join":%s,"rooms":[%s]%s%s}', to_json(realm), seats,
                to_json(coalesce(w.code, realm || '-' || j.n)), rooms.a,
                CASE WHEN w.code IS NULL THEN '' ELSE ',"with":' || to_json(w.code)::text END,
                CASE WHEN w.code IS NULL AND w.full IS NOT NULL THEN ',"with_full":' || to_json(w.full)::text ELSE '' END)
  FROM j, rooms, w $$;
