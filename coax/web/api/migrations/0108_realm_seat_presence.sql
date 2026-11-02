-- Presence for made realms, and an agent's seat in them (docs/verse/realm-seat.md slice 4, #12655).
--
-- A made realm's room is "made-<slug>" (myrealm.coax posts to /api/playtest/presence/made-<slug>).
-- A private realm's room is as hidden as the realm (0099): to anyone but its owner and site admins,
-- presence and chat there answer 404, so nobody learns who is in a realm they can't open.
--
-- A seat (`openagents realm play`) is an agent in a realm. Its holder posts the seat's presence
-- itself, signed with the CLI's key (NIP-98): room.c verifies the signature (caller-policy.coax),
-- acts with the account's player key (auth.caller_key, 0098) and says seat. Only that marks a
-- presence row `agent`: an X-Verse-Automation header saying "seat" without a signature is plain
-- automation and stays hidden (0069). An agent's row is shown to everyone in the room, with
-- {"agent": true} and its account's name, because a person should see when an agent is with them;
-- every other automation stays seen only by itself and its pair (0074).
--
-- A seat posts between its actions, and one action (a walk, an answer) takes up to 30 s, so an
-- agent's row stays fresh for room_seat_ms; its holder posts {"gone": true} when it leaves, which
-- takes the row out at once. The seat's own page (a guest player, automation) is in the room too, to
-- see the others; the seat names it ("page": its player id) and that page isn't shown its own agent.
-- room.c calls the 10-argument verse.room below.
ALTER TABLE verse.room_here ADD COLUMN agent boolean NOT NULL DEFAULT false;
ALTER TABLE verse.room_here ADD COLUMN seat_page text;

CREATE FUNCTION verse.room_seat_ms() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 40000 $$;

-- As 0077's, with an agent's row: seen by everyone while fresh for an agent, and flagged.
CREATE OR REPLACE FUNCTION verse.room_seen(r text, me text, pair text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.player, 'name', h.name, 'x', h.x, 'y', h.y, 'z', h.z, 'yaw', h.yaw)
                            || CASE WHEN l.look IS NOT NULL THEN jsonb_build_object('look', l.look) ELSE '{}'::jsonb END
                            || CASE WHEN h.agent THEN '{"agent": true}'::jsonb ELSE '{}'::jsonb END
                            ORDER BY h.since_ms, h.player), '[]')
  FROM verse.room_here h
  LEFT JOIN makers.wearing w ON w.player = h.player
  LEFT JOIN makers.listings l ON l.id = w.listing AND l.state = 'approved'
  WHERE h.room = r
    AND h.seen_ms >= verse.now_ms() - CASE WHEN h.agent THEN verse.room_seat_ms() ELSE verse.room_present_ms() END
    AND (h.agent OR h.auto IS NULL OR (me IS NOT NULL AND h.player = me) OR (pair IS NOT NULL AND h.auto = pair))
    AND NOT (h.agent AND me IS NOT NULL AND h.seat_page IS NOT DISTINCT FROM me)
$$;

-- A made realm's room that this caller may not see: its realm is private and they are neither
-- its owner nor a site admin (verse.realm_hidden, 0099). Every other room is open as before.
CREATE FUNCTION verse.room_hidden(r text, k text, site_admin boolean) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT r ~ '^made-[a-z0-9-]{1,59}$' AND verse.realm_hidden(substr(r, 6), k, site_admin)
$$;

-- One request, as 0074's, and: a hidden made realm's room is 404; seat (a verified NIP-98 caller,
-- k its account's player key) posts presence only, marks its row agent, and {"gone": true} leaves.
CREATE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text,
                           seat boolean, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  me text;
BEGIN
  IF verse.room_hidden(r, k, site_admin) THEN RETURN verse.r(404, '{"error":"unknown"}'); END IF;
  IF NOT coalesce(seat, false) THEN
    out := verse.room(method, kind, r, body, k, after, mark, ua);
    -- a page's own post is never an agent's, whatever it was before
    IF kind = 'presence' AND method = 'POST' AND (out->>'status')::int = 200 THEN
      UPDATE verse.room_here SET agent = false WHERE room = r AND player = out->'body'->'you'->>'id' AND agent;
    END IF;
    RETURN out;
  END IF;
  IF kind <> 'presence' OR method <> 'POST' THEN RETURN verse.r(405, '{"error":"a seat posts its presence"}'); END IF;
  IF jsonb_typeof(body) = 'object' AND body->'gone' = 'true'::jsonb THEN
    SELECT id INTO me FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
    IF me IS NULL THEN RETURN verse.r(403, '{"error":"no player"}'); END IF;
    DELETE FROM verse.room_here WHERE room = r AND player = me;
    RETURN verse.r(200, '{"gone": true}');
  END IF;
  out := verse.room(method, kind, r, body, k, after, 'seat', ua);
  IF (out->>'status')::int <> 200 THEN RETURN out; END IF;
  UPDATE verse.room_here SET agent = true,
    seat_page = CASE WHEN jsonb_typeof(body) = 'object' AND body->>'page' ~ '^p-[a-z0-9]{1,40}$' THEN body->>'page' END
  WHERE room = r AND player = out->'body'->'you'->>'id';
  RETURN jsonb_set(out, '{body,you,agent}', 'true'::jsonb);
END $$;
