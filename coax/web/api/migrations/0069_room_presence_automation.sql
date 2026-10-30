-- Who is in a room, for everyone, and automation there seen only by itself (grid-walk, 2026-10-30).
-- The homepage draws the people walking the Grid on its own grid: it asks GET
-- /api/playtest/presence/<room> (no key needed, nothing joins) for everyone seen there in the last
-- six seconds. A presence from automation (the request's automation mark, a headless browser's
-- User-Agent, or a player minted by automation, as 0064 marks a chat line) is kept with its mark,
-- and a mark is seen only by the player it belongs to: no other player in the room and no homepage
-- ever draws a check walking the Grid. room.c already calls this 8-argument verse.room.
ALTER TABLE verse.room_here ADD COLUMN auto text;

-- Everyone fresh in room r, as presence answers them, but automation's only to its own player (me).
CREATE FUNCTION verse.room_seen(r text, me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                            ORDER BY h.since_ms, h.player), '[]')
  FROM verse.room_here h
  WHERE h.room = r AND h.seen_ms >= verse.now_ms() - verse.room_present_ms()
    AND (h.auto IS NULL OR (me IS NOT NULL AND h.player = me))
$$;

-- As 0064's, and presence: GET answers who is here {others, count}; a POST keeps whether this
-- player is automation and leaves automation out of the others it answers.
CREATE OR REPLACE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  who verse.players;
  auto_ text;
  seen jsonb;
BEGIN
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO who FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  auto_ := left(coalesce(nullif(regexp_replace(coalesce(mark, ''), '[^A-Za-z0-9_.:-]', '', 'g'), ''), verse.headless_agent(ua), who.auto), 64);
  IF kind = 'presence' AND method = 'GET' THEN
    IF r !~ '^[A-Za-z0-9_-]{1,64}$' THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
    seen := verse.room_seen(r, who.id);
    RETURN verse.r(200, jsonb_build_object('others', seen, 'count', jsonb_array_length(seen)));
  END IF;
  out := verse.room(method, kind, r, body, k, after);
  IF (out->>'status')::int <> 200 OR who.id IS NULL THEN RETURN out; END IF;
  IF kind = 'presence' AND method = 'POST' THEN
    UPDATE verse.room_here SET auto = auto_ WHERE room = r AND player = who.id AND auto IS DISTINCT FROM auto_;
    seen := (SELECT coalesce(jsonb_agg(o ORDER BY i), '[]') FROM jsonb_array_elements(verse.room_seen(r, who.id)) WITH ORDINALITY e(o, i) WHERE o->>'id' <> who.id);
    RETURN jsonb_set(out, '{body,others}', seen);
  END IF;
  IF kind = 'chat' AND method = 'POST' AND jsonb_typeof(out->'body'->'n') = 'number' AND auto_ IS NOT NULL THEN
    UPDATE verse.room_chat SET auto = auto_ WHERE room = r AND n = (out->'body'->>'n')::int AND player = who.id;
  END IF;
  RETURN out;
END $$;
