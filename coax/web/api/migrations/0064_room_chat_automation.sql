-- A room's chat lines from automation (2026-10-30: a headless check typed "evening all" twice into
-- Hecate's Cemetery and every visitor saw it). A line is automation's when the request carried the
-- automation mark (header X-Verse-Automation or cookie verse-automation, as analytics reads it), its
-- User-Agent is a headless browser (verse.headless_agent) or its player was minted by
-- automation (verse.players.auto). Such a line is kept with its mark and shown only to the player who
-- said it, so a check still sees its own line and no real visitor ever does. room.c calls the
-- 8-argument verse.room; 0053's 6-argument one stays for an older server still running.
ALTER TABLE verse.room_chat ADD COLUMN auto text;

CREATE OR REPLACE FUNCTION verse.room_lines(r text, after int, me text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(verse.room_line(c, me) ORDER BY c.n), '[]')
  FROM (SELECT * FROM verse.room_chat x WHERE x.room = r AND x.n > after AND x.removed_ms IS NULL
          AND (x.auto IS NULL OR (me IS NOT NULL AND x.player = me))
          AND NOT EXISTS (SELECT 1 FROM verse.player_blocks b WHERE b.player = me AND b.blocked = x.player)
        ORDER BY x.n DESC LIMIT 50) c
$$;

-- A headless browser's User-Agent (as analytics.bot_agent names it), else NULL. Only a browser that
-- runs the page: a script calling the API itself (curl, python) is the API's own tests and tools.
CREATE FUNCTION verse.headless_agent(ua text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN coalesce(ua, '') ~* '(headlesschrome|headless|phantomjs|puppeteer|playwright|selenium|webdriver|lighthouse)'
              THEN analytics.bot_agent(ua) END $$;

-- As 0053's, and a line said is marked when it is automation's. mark: the request's automation mark
-- ("" for none); ua: its User-Agent.
CREATE FUNCTION verse.room(method text, kind text, r text, body jsonb, k text, after int, mark text, ua text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb := verse.room(method, kind, r, body, k, after);
  who verse.players;
  auto_ text;
BEGIN
  IF kind <> 'chat' OR method <> 'POST' OR (out->>'status')::int <> 200 OR jsonb_typeof(out->'body'->'n') <> 'number' THEN RETURN out; END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO who FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  auto_ := left(coalesce(nullif(regexp_replace(coalesce(mark, ''), '[^A-Za-z0-9_.:-]', '', 'g'), ''), verse.headless_agent(ua), who.auto), 64);
  IF auto_ IS NOT NULL THEN
    UPDATE verse.room_chat SET auto = auto_ WHERE room = r AND n = (out->'body'->>'n')::int AND player = who.id;
  END IF;
  RETURN out;
END $$;
