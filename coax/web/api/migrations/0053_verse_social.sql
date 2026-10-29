-- Talking with other players (verse/fairy-forest/README.md, "Talking with
-- other players"; tasks ff-chat-room and ff-inbox): a room's chat lines carry
-- who said them, so a player can block someone; an admin can remove a line;
-- players who share a room have met; and players who have met can leave each
-- other messages that wait in an inbox until they are read. room.c answers:
--
--   POST /api/playtest/chat/<room>/remove   {n}: an admin (admin.c admin_who) takes line n out
--   GET  /api/playtest/inbox                your messages (the last 40, to or from you), how many
--                                           are unread, who you have met, who you block, admin
--   POST /api/playtest/inbox                {to, text}: a message to a player you have met -> {id}
--   POST /api/playtest/inbox/read           {upto, from}: every message to you up to that id (from
--                                           that player, if given) is read
--   POST /api/playtest/block                {player, on}: hide (or show again) their lines and
--                                           messages, for you only -> {blocked}
--
-- Abuse: a message is at most 500 characters; at most 5 a minute and 100 a
-- day a player; only to someone you have met; a blocked player's lines and
-- messages are never shown to who blocked them. Nothing here is analytics:
-- the page counts what it sends, never the words.
ALTER TABLE verse.room_chat ADD COLUMN removed_ms bigint, ADD COLUMN removed_by text;

CREATE TABLE verse.players_met (     -- two players who were in a room at once, a < b
  a text NOT NULL REFERENCES verse.players (id),
  b text NOT NULL REFERENCES verse.players (id),
  room text NOT NULL,                -- where they last were together
  first_ms bigint NOT NULL,
  last_ms bigint NOT NULL,
  PRIMARY KEY (a, b),
  CHECK (a < b)
);
CREATE INDEX players_met_b ON verse.players_met (b);

CREATE TABLE verse.player_blocks (
  player text NOT NULL REFERENCES verse.players (id),
  blocked text NOT NULL REFERENCES verse.players (id),
  at_ms bigint NOT NULL,
  PRIMARY KEY (player, blocked)
);

CREATE TABLE verse.inbox (
  id bigserial PRIMARY KEY,
  to_player text NOT NULL REFERENCES verse.players (id),
  from_player text NOT NULL REFERENCES verse.players (id),
  text text NOT NULL,
  at_ms bigint NOT NULL,
  read_ms bigint
);
CREATE INDEX inbox_to ON verse.inbox (to_player, id);
CREATE INDEX inbox_from ON verse.inbox (from_player, at_ms);

-- The name others see: the account's, else "Guest" and four letters of the id (as the rooms name you).
CREATE FUNCTION verse.player_name(pid text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT left(coalesce((SELECT nullif(btrim(a.name), '') FROM verse.players p JOIN verse.accounts a ON a.id = p.account WHERE p.id = pid),
                       'Guest ' || upper(substr(pid, 3, 4))), 40)
$$;
CREATE FUNCTION verse.player_of(k text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT id FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex')
$$;
CREATE FUNCTION verse.have_met(x text, y text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM verse.players_met WHERE a = least(x, y) AND b = greatest(x, y))
$$;
CREATE FUNCTION verse.clean_text(v jsonb, cap int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(coalesce(CASE WHEN jsonb_typeof(v) = 'string' THEN v #>> '{}' END, ''), '[\x00-\x1f\x7f]+', ' ', 'g')), cap)
$$;

-- A line now says who said it ("from", null for the demo's), and the lines
-- read leave out removed ones and those of anyone the reader blocks.
CREATE OR REPLACE FUNCTION verse.room_line(c verse.room_chat, me text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('n', c.n, 'name', c.name, 'text', c.text, 'at', c.at, 'mine', me IS NOT NULL AND c.player = me, 'from', c.player)
$$;
CREATE OR REPLACE FUNCTION verse.room_lines(r text, after int, me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(verse.room_line(c, me) ORDER BY c.n), '[]')
  FROM (SELECT * FROM verse.room_chat x WHERE x.room = r AND x.n > after AND x.removed_ms IS NULL
          AND NOT EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked = x.player)
        ORDER BY x.n DESC LIMIT 50) c
$$;

-- As 0014's, and: whoever is in the room with you, you have met (kept at
-- most once a minute a pair), and the answer names the recent lines an admin
-- removed, so a page that showed one takes it down.
CREATE OR REPLACE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  rec verse.players;
  who text;
  now_ms bigint := verse.now_ms();
  fresh bigint := verse.now_ms() - verse.room_present_ms();
  here int;
  was verse.room_here;
  said text;
  n_ int;
  since int;
  top int;
BEGIN
  IF r !~ '^[A-Za-z0-9_-]{1,64}$' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF kind = 'chat' AND method = 'GET' THEN
    RETURN verse.r(200, jsonb_build_object('messages', verse.room_lines(r, greatest(after, 0), rec.id),
      'last', coalesce((SELECT max(n) FROM verse.room_chat WHERE room = r), 0)));
  END IF;
  IF method <> 'POST' THEN RETURN verse.r(405, '{"error":"POST"}'); END IF;
  IF rec.id IS NULL THEN
    RETURN verse.r(403, '{"error":"send a player key (POST /api/playtest/player) to join a room"}');
  END IF;
  who := verse.player_name(rec.id);

  IF kind = 'chat' THEN
    said := verse.clean_text(body->'text', 500);
    IF said = '' THEN RETURN verse.r(400, '{"error":"say something"}'); END IF;
    SELECT count(*) INTO since FROM verse.room_chat
    WHERE player = rec.id AND at > extract(epoch FROM clock_timestamp()) - 10;
    IF since >= 5 THEN RETURN verse.r(429, '{"error":"a little slower"}'); END IF;
    PERFORM pg_advisory_xact_lock(7731013, hashtext(r));
    SELECT coalesce(max(n), 0) + 1 INTO n_ FROM verse.room_chat WHERE room = r;
    INSERT INTO verse.room_chat (room, n, player, name, text, at, sid)
    VALUES (r, n_, rec.id, who, said, extract(epoch FROM clock_timestamp()),
            CASE WHEN body->>'sid' ~ '^[A-Za-z0-9_:-]{1,80}$' THEN body->>'sid' END);
    RETURN verse.r(200, jsonb_build_object('n', n_));
  END IF;

  IF kind = 'presence' THEN
    SELECT * INTO was FROM verse.room_here WHERE room = r AND player = rec.id;
    IF was.player IS NULL OR was.seen_ms < fresh THEN
      SELECT count(*) INTO here FROM verse.room_here WHERE room = r AND seen_ms >= fresh AND player <> rec.id;
      IF here >= verse.room_max() THEN RETURN verse.r(429, '{"error":"the room is full"}'); END IF;
    END IF;
    INSERT INTO verse.room_here AS h (room, player, name, x, y, z, yaw, since_ms, seen_ms)
    VALUES (r, rec.id, who, verse.room_num(body->'x', 10000), verse.room_num(body->'y', 10000), verse.room_num(body->'z', 10000),
            verse.room_num(body->'yaw', 1000), now_ms, now_ms)
    ON CONFLICT (room, player) DO UPDATE SET name = EXCLUDED.name, x = EXCLUDED.x, y = EXCLUDED.y, z = EXCLUDED.z, yaw = EXCLUDED.yaw,
      seen_ms = EXCLUDED.seen_ms, since_ms = CASE WHEN h.seen_ms < fresh THEN EXCLUDED.since_ms ELSE h.since_ms END;
    INSERT INTO verse.players_met AS m (a, b, room, first_ms, last_ms)
    SELECT least(rec.id, h.player), greatest(rec.id, h.player), r, now_ms, now_ms
    FROM verse.room_here h WHERE h.room = r AND h.seen_ms >= fresh AND h.player <> rec.id
    ON CONFLICT (a, b) DO UPDATE SET room = EXCLUDED.room, last_ms = EXCLUDED.last_ms WHERE m.last_ms < EXCLUDED.last_ms - 60000;
    top := coalesce((SELECT max(n) FROM verse.room_chat WHERE room = r), 0);
    RETURN verse.r(200, jsonb_build_object(
      'you', jsonb_build_object('id', rec.id, 'name', who),
      'others', coalesce((SELECT jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                                           ORDER BY h.since_ms, h.player)
                          FROM verse.room_here h WHERE h.room = r AND h.seen_ms >= fresh AND h.player <> rec.id), '[]'),
      'messages', CASE WHEN jsonb_typeof(body->'after') = 'number' THEN verse.room_lines(r, greatest((body->>'after')::numeric::int, 0), rec.id) ELSE '[]' END,
      'removed', coalesce((SELECT jsonb_agg(c.n ORDER BY c.n) FROM verse.room_chat c WHERE c.room = r AND c.n > top - 50 AND c.removed_ms IS NOT NULL), '[]'),
      'last', top));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;

-- An admin takes a line out of a room (by: the admin's pubkey).
CREATE FUNCTION verse.room_remove(r text, body jsonb, by_ text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  n_ int;
BEGIN
  IF r !~ '^[A-Za-z0-9_-]{1,64}$' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  IF jsonb_typeof(body->'n') IS DISTINCT FROM 'number' THEN RETURN verse.r(400, '{"error":"which line: {n}"}'); END IF;
  n_ := (body->>'n')::numeric::int;
  UPDATE verse.room_chat SET removed_ms = verse.now_ms(), removed_by = by_ WHERE room = r AND n = n_ AND removed_ms IS NULL;
  IF NOT FOUND AND NOT EXISTS (SELECT 1 FROM verse.room_chat WHERE room = r AND n = n_) THEN
    RETURN verse.r(404, '{"error":"no such line"}');
  END IF;
  RETURN verse.r(200, jsonb_build_object('removed', n_));
END $$;

CREATE FUNCTION verse.blocked_by(me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(blocked ORDER BY at_ms), '[]') FROM verse.player_blocks WHERE player = me
$$;

-- The inbox and blocks. what: inbox, inbox/read or block; admin: the asker is a site admin.
CREATE FUNCTION verse.social(method text, what text, body jsonb, k text, admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  me text := verse.player_of(k);
  now_ms bigint := verse.now_ms();
  them text;
  said text;
  sent int;
  id_ bigint;
BEGIN
  IF me IS NULL THEN RETURN verse.r(403, '{"error":"send a player key (POST /api/playtest/player)"}'); END IF;
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;

  IF what = 'inbox' AND method = 'GET' THEN
    RETURN verse.r(200, jsonb_build_object(
      'you', jsonb_build_object('id', me, 'name', verse.player_name(me)),
      'admin', admin,
      'unread', (SELECT count(*) FROM verse.inbox i WHERE i.to_player = me AND i.read_ms IS NULL
                   AND NOT EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked = i.from_player)),
      'messages', coalesce((SELECT jsonb_agg(jsonb_build_object('id', x.id, 'from', x.from_player, 'name', verse.player_name(x.from_player),
                                                                'to', x.to_player, 'to_name', verse.player_name(x.to_player), 'text', x.text,
                                                                'at', x.at_ms, 'mine', x.from_player = me, 'read', x.read_ms IS NOT NULL) ORDER BY x.id)
                            FROM (SELECT * FROM verse.inbox i WHERE (i.to_player = me OR i.from_player = me)
                                    AND NOT EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked IN (i.from_player, i.to_player))
                                  ORDER BY i.id DESC LIMIT 40) x), '[]'),
      'met', coalesce((SELECT jsonb_agg(jsonb_build_object('id', y.other, 'name', verse.player_name(y.other),
                                                           'blocked', EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked = y.other))
                                        ORDER BY y.last_ms DESC)
                       FROM (SELECT CASE WHEN m.a = me THEN m.b ELSE m.a END AS other, m.last_ms FROM verse.players_met m
                             WHERE m.a = me OR m.b = me ORDER BY m.last_ms DESC LIMIT 30) y), '[]'),
      'blocked', verse.blocked_by(me)));
  END IF;
  IF method <> 'POST' THEN RETURN verse.r(405, '{"error":"POST"}'); END IF;

  IF what = 'inbox' THEN
    them := CASE WHEN jsonb_typeof(body->'to') = 'string' THEN body->>'to' END;
    IF them IS NULL OR them !~ '^[A-Za-z0-9_-]{1,64}$' OR NOT EXISTS (SELECT 1 FROM verse.players WHERE id = them) THEN
      RETURN verse.r(404, '{"error":"no such player"}');
    END IF;
    IF them = me THEN RETURN verse.r(400, '{"error":"that is you"}'); END IF;
    IF NOT verse.have_met(me, them) THEN RETURN verse.r(403, '{"error":"you can write to players you have met"}'); END IF;
    said := verse.clean_text(body->'text', 500);
    IF said = '' THEN RETURN verse.r(400, '{"error":"write something"}'); END IF;
    SELECT count(*) INTO sent FROM verse.inbox WHERE from_player = me AND at_ms > now_ms - 60000;
    IF sent >= 5 THEN RETURN verse.r(429, '{"error":"a little slower"}'); END IF;
    SELECT count(*) INTO sent FROM verse.inbox WHERE from_player = me AND at_ms > now_ms - 86400000;
    IF sent >= 100 THEN RETURN verse.r(429, '{"error":"that is enough for today"}'); END IF;
    INSERT INTO verse.inbox (to_player, from_player, text, at_ms) VALUES (them, me, said, now_ms) RETURNING id INTO id_;
    RETURN verse.r(200, jsonb_build_object('id', id_));
  END IF;

  IF what = 'inbox/read' THEN
    IF jsonb_typeof(body->'upto') IS DISTINCT FROM 'number' THEN RETURN verse.r(400, '{"error":"up to which: {upto}"}'); END IF;
    UPDATE verse.inbox SET read_ms = now_ms WHERE to_player = me AND read_ms IS NULL AND id <= (body->>'upto')::numeric::bigint
      AND (jsonb_typeof(body->'from') IS DISTINCT FROM 'string' OR from_player = body->>'from');
    RETURN verse.r(200, jsonb_build_object('unread', (SELECT count(*) FROM verse.inbox WHERE to_player = me AND read_ms IS NULL)));
  END IF;

  IF what = 'block' THEN
    them := CASE WHEN jsonb_typeof(body->'player') = 'string' THEN body->>'player' END;
    IF them IS NULL OR them = me OR NOT EXISTS (SELECT 1 FROM verse.players WHERE id = them) THEN
      RETURN verse.r(404, '{"error":"no such player"}');
    END IF;
    IF body->'on' = 'false'::jsonb THEN
      DELETE FROM verse.player_blocks WHERE player = me AND blocked = them;
    ELSE
      INSERT INTO verse.player_blocks (player, blocked, at_ms) VALUES (me, them, now_ms) ON CONFLICT DO NOTHING;
    END IF;
    RETURN verse.r(200, jsonb_build_object('blocked', verse.blocked_by(me)));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;
