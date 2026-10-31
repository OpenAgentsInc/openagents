-- Checks that walk a room together (ev-ff-shared, 2026-10-30). 0069 shows automation's presence only
-- to its own player, so two headless sessions in Fairy Forest never see each other and a check of
-- "the others move" cannot run. A request whose automation mark starts "pair-" (header
-- X-Verse-Automation or cookie verse-automation, e.g. pair-ff-1a2b) is shown to the other players of
-- that same mark too: the sessions of one check see each other, and still no person in the room, no
-- homepage and no other check ever does. A headless User-Agent alone, or any other mark, stays as 0069.

-- The mark a request pairs on: its own mark when it is "pair-...", else NULL.
CREATE FUNCTION verse.room_pair(mark text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN m ~ '^pair-[A-Za-z0-9_.:-]{1,59}$' THEN m END
  FROM (SELECT regexp_replace(coalesce(mark, ''), '[^A-Za-z0-9_.:-]', '', 'g') AS m) x
$$;

-- As 0069's, and automation of the same pair mark sees each other.
CREATE FUNCTION verse.room_seen(r text, me text, pair text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                            ORDER BY h.since_ms, h.player), '[]')
  FROM verse.room_here h
  WHERE h.room = r AND h.seen_ms >= verse.now_ms() - verse.room_present_ms()
    AND (h.auto IS NULL OR (me IS NOT NULL AND h.player = me) OR (pair IS NOT NULL AND h.auto = pair))
$$;

-- As 0069's, the others answered through verse.room_seen with the request's pair.
CREATE OR REPLACE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  who verse.players;
  auto_ text;
  pair_ text := verse.room_pair(mark);
  seen jsonb;
BEGIN
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO who FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  auto_ := left(coalesce(nullif(regexp_replace(coalesce(mark, ''), '[^A-Za-z0-9_.:-]', '', 'g'), ''), verse.headless_agent(ua), who.auto), 64);
  IF kind = 'presence' AND method = 'GET' THEN
    IF r !~ '^[A-Za-z0-9_-]{1,64}$' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
    seen := verse.room_seen(r, who.id, pair_);
    RETURN verse.r(200, jsonb_build_object('others', seen, 'count', jsonb_array_length(seen)));
  END IF;
  out := verse.room(method, kind, r, body, k, after);
  IF (out->>'status')::int <> 200 OR who.id IS NULL THEN RETURN out; END IF;
  IF kind = 'presence' AND method = 'POST' THEN
    UPDATE verse.room_here SET auto = auto_ WHERE room = r AND player = who.id AND auto IS DISTINCT FROM auto_;
    seen := (SELECT coalesce(jsonb_agg(o ORDER BY i), '[]') FROM jsonb_array_elements(verse.room_seen(r, who.id, pair_)) WITH ORDINALITY e(o, i) WHERE o->>'id' <> who.id);
    RETURN jsonb_set(out, '{body,others}', seen);
  END IF;
  IF kind = 'chat' AND method = 'POST' AND jsonb_typeof(out->'body'->'n') = 'number' AND auto_ IS NOT NULL THEN
    UPDATE verse.room_chat SET auto = auto_ WHERE room = r AND n = (out->'body'->>'n')::int AND player = who.id;
  END IF;
  RETURN out;
END $$;
