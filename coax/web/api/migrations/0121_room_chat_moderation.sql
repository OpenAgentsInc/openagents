-- Everglade room chat (#12714 moderation, found in the chat bug hunt of 2026-11-04):
--   - a kicked player (verse.room_kicks, an admin's KICK or a report's HIDE) is out of the rooms' chat
--     too: their line is refused (403 {"error": "kicked", "until": MS}), as their seat ticket is;
--   - the words on the names' block list (verse.blocked_words, which verse.name_ok now reads) are
--     masked in a chat line, each letter a '*', before the line is kept;
--   - checks that share a room (a "pair-" automation mark, 0074) read each other's lines as they see
--     each other's presence: a headless line is still shown to no person and no other check.
-- room.c calls the 10-argument verse.room; 0110's is kept as verse.room_0110 and called from it.
-- Additive.

CREATE OR REPLACE FUNCTION verse.blocked_words() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['fuck', 'shit', 'cunt', 'nigg', 'fagg', 'retard', 'whore', 'slut', 'bitch', 'nazi', 'hitler', 'porn'] $$;

CREATE OR REPLACE FUNCTION verse.name_ok(n text) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT n IS NOT NULL AND length(n) BETWEEN 3 AND 20 AND n ~ '^[A-Za-z0-9][A-Za-z0-9 ''-]*$' AND n !~ '  '
    AND lower(n) !~ '(https?|www|\.com|\.net|\.org|discord|onlyfans)'
    AND NOT EXISTS (SELECT 1 FROM unnest(verse.blocked_words()) w
                    WHERE regexp_replace(lower(n), '[^a-z]', '', 'g') LIKE '%' || w || '%') $$;

-- A chat line with each blocked word masked, in any case ("Shit" -> "****").
CREATE OR REPLACE FUNCTION verse.chat_mask(t text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE w text;
BEGIN
  IF t IS NULL THEN RETURN t; END IF;
  FOREACH w IN ARRAY verse.blocked_words() LOOP
    t := regexp_replace(t, w, repeat('*', length(w)), 'gi');
  END LOOP;
  RETURN t;
END $$;

-- As 0064's, and a line from automation of the reader's own pair mark is shown too.
CREATE OR REPLACE FUNCTION verse.room_lines_paired(r text, after int, me text, pair text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(verse.room_line(c, me) ORDER BY c.n), '[]')
  FROM (SELECT * FROM verse.room_chat x WHERE x.room = r AND x.n > after AND x.removed_ms IS NULL
          AND (x.auto IS NULL OR (me IS NOT NULL AND x.player = me) OR (pair IS NOT NULL AND x.auto = pair))
          AND NOT EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked = x.player)
        ORDER BY x.n DESC LIMIT 50) c
$$;

DO $$ BEGIN
  IF NOT EXISTS (SELECT 1 FROM pg_proc p JOIN pg_namespace s ON s.oid = p.pronamespace WHERE s.nspname = 'verse' AND p.proname = 'room_0110') THEN
    ALTER FUNCTION verse.room(text, text, text, jsonb, text, int, text, text, boolean, boolean) RENAME TO room_0110;
  END IF;
END $$;

CREATE OR REPLACE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text,
                                      seat boolean, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb;
  me text := verse.player_of(k);
  pair_ text := verse.room_pair(mark);
  until_ bigint;
  read_after int;
BEGIN
  IF kind = 'chat' AND method = 'POST' AND me IS NOT NULL THEN
    SELECT until_ms INTO until_ FROM verse.room_kicks WHERE player = me AND until_ms > verse.now_ms();
    IF until_ IS NOT NULL THEN RETURN verse.r(403, jsonb_build_object('error', 'kicked', 'until', until_)); END IF;
    IF jsonb_typeof(body) = 'object' AND jsonb_typeof(body->'text') = 'string' THEN
      body := jsonb_set(body, '{text}', to_jsonb(verse.chat_mask(body->>'text')));
    END IF;
  END IF;
  out := verse.room_0110(method, kind, r, body, k, after, mark, ua, seat, site_admin);
  IF pair_ IS NULL OR (out->>'status')::int <> 200 OR jsonb_typeof(out->'body'->'messages') <> 'array' THEN RETURN out; END IF;
  IF kind = 'chat' AND method = 'GET' THEN read_after := greatest(after, 0);
  ELSIF kind = 'presence' AND method = 'POST' AND jsonb_typeof(body->'after') = 'number' THEN read_after := greatest((body->>'after')::numeric::int, 0);
  ELSE RETURN out; END IF;
  RETURN jsonb_set(out, '{body,messages}', verse.room_lines_paired(r, read_after, me, pair_));
END $$;
