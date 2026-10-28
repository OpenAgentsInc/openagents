-- Rooms (pt-presence-chat; room.c answers /api/playtest/presence/<room> and
-- /api/playtest/chat/<room>, and the demo's /api/presence/<room> and
-- /api/chat/<room>): who else is in a world, and its chat. A room is a
-- world's id (haunted-house, or a player's world). Only a player key the
-- server minted is let in, a guest's too; the name others see is the
-- account's, else "Guest" and four letters of the player's id.
--
-- Presence: a page posts where it is about every second and is answered with
-- everyone else seen in the last six seconds. A room holds ROOM_MAX at once;
-- one more is told it is full. Chat: a line is at most 500 characters (cut,
-- as the demo did), at most five a player in ten seconds, and every line is
-- kept. The demo's room chat (chat-<room>.json, waiting in verse.demo_files)
-- comes in through verse.chat_from_demo(), keyed on the file and the line's
-- number, so it runs again after every import and adds only new lines.
CREATE TABLE verse.room_here (
  room text NOT NULL,
  player text NOT NULL REFERENCES verse.players (id),
  name text NOT NULL,
  x real NOT NULL DEFAULT 0, y real NOT NULL DEFAULT 0, z real NOT NULL DEFAULT 0, yaw real NOT NULL DEFAULT 0,
  since_ms bigint NOT NULL,      -- came in (again) then
  seen_ms bigint NOT NULL,
  PRIMARY KEY (room, player)
);
CREATE INDEX room_here_seen ON verse.room_here (room, seen_ms);
CREATE TABLE verse.room_chat (
  id bigserial PRIMARY KEY,
  room text NOT NULL,
  n int NOT NULL,                -- the line's number in its room, from 1
  player text,                   -- null for the demo's lines
  name text NOT NULL,
  text text NOT NULL,
  at double precision NOT NULL,  -- seconds, as the demo answers it
  sid text,
  space text NOT NULL DEFAULT 'web',
  source text,
  import_key text UNIQUE,
  UNIQUE (room, n)
);
CREATE INDEX room_chat_player ON verse.room_chat (player, at);

CREATE FUNCTION verse.room_max() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 16 $$;
CREATE FUNCTION verse.room_present_ms() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 6000 $$;

CREATE FUNCTION verse.room_num(v jsonb, lim real) RETURNS real LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'number' AND abs((v #>> '{}')::double precision) <= lim THEN (v #>> '{}')::real ELSE 0 END
$$;

CREATE FUNCTION verse.room_line(c verse.room_chat, me text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('n', c.n, 'name', c.name, 'text', c.text, 'at', c.at, 'mine', me IS NOT NULL AND c.player = me)
$$;

CREATE FUNCTION verse.room_lines(r text, after int, me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(verse.room_line(c, me) ORDER BY c.n), '[]')
  FROM (SELECT * FROM verse.room_chat WHERE room = r AND n > after ORDER BY n DESC LIMIT 50) c
$$;

-- One request. kind: presence or chat; key: the X-Verse-Player header;
-- after: the query's after (chat), or -1.
CREATE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  who := left(coalesce((SELECT nullif(btrim(name), '') FROM verse.accounts WHERE id = rec.account),
                       'Guest ' || upper(substr(rec.id, 3, 4))), 40);

  IF kind = 'chat' THEN
    said := left(btrim(regexp_replace(coalesce(CASE WHEN jsonb_typeof(body->'text') = 'string' THEN body->>'text' END, ''),
                                      '[\x00-\x1f\x7f]+', ' ', 'g')), 500);
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
    RETURN verse.r(200, jsonb_build_object(
      'you', jsonb_build_object('id', rec.id, 'name', who),
      'others', coalesce((SELECT jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                                           ORDER BY h.since_ms, h.player)
                          FROM verse.room_here h WHERE h.room = r AND h.seen_ms >= fresh AND h.player <> rec.id), '[]'),
      'messages', CASE WHEN jsonb_typeof(body->'after') = 'number' THEN verse.room_lines(r, greatest((body->>'after')::numeric::int, 0), rec.id) ELSE '[]' END,
      'last', coalesce((SELECT max(n) FROM verse.room_chat WHERE room = r), 0)));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;

CREATE FUNCTION verse.chat_from_demo() RETURNS int LANGUAGE sql AS $$
  WITH e AS (
    SELECT f.path, f.space, substring(f.path FROM '/chat-([A-Za-z0-9_-]{1,64})\.json$') AS room, x.v
    FROM verse.demo_files f, jsonb_array_elements(CASE WHEN jsonb_typeof(f.doc) = 'array' THEN f.doc ELSE '[]' END) AS x(v)
    WHERE f.path ~ '/chat-[A-Za-z0-9_-]{1,64}\.json$'
  ), todo AS (
    SELECT e.*, row_number() OVER (PARTITION BY room ORDER BY (v->>'at')::double precision, path, (v->>'n')::int) AS k
    FROM e
    WHERE room IS NOT NULL AND jsonb_typeof(v->'n') = 'number' AND jsonb_typeof(v->'at') = 'number'
      AND btrim(coalesce(v->>'text', '')) <> ''
      AND NOT EXISTS (SELECT 1 FROM verse.room_chat c WHERE c.import_key = e.path || '#' || (v->>'n'))
  ), n AS (
    INSERT INTO verse.room_chat (room, n, player, name, text, at, sid, space, source, import_key)
    SELECT room, coalesce((SELECT max(c.n) FROM verse.room_chat c WHERE c.room = todo.room), 0) + k::int, NULL,
           left(coalesce(nullif(btrim(v->>'name'), ''), 'Guest'), 40), left(btrim(v->>'text'), 500), (v->>'at')::double precision,
           CASE WHEN v->>'sid' ~ '^[A-Za-z0-9_:-]{1,80}$' THEN v->>'sid' END, space, path, path || '#' || (v->>'n')
    FROM todo ORDER BY room, k
    ON CONFLICT DO NOTHING RETURNING 1)
  SELECT count(*)::int FROM n
$$;
SELECT verse.chat_from_demo();
