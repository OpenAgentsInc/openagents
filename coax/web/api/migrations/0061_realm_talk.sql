-- A made realm's characters talk (byo-talk; apps/openagents-web/myrealm.coax): the page asks
-- POST /api/playtest/npc/ask as Everglade's people do, with "realm": the made realm's slug. Its
-- owner's TALK setting holds here too: with it off only the owner (and an admin) gets an answer,
-- and nothing is spent on the day's cap. A slug that isn't a made realm is a 404. Without "realm"
-- the question is asked as before.
ALTER FUNCTION verse.npc_ask(jsonb, int, text) RENAME TO npc_ask_0060;

CREATE FUNCTION verse.npc_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  s text := CASE WHEN jsonb_typeof(body) = 'object' AND jsonb_typeof(body->'realm') = 'string' THEN body->>'realm' END;
  talk boolean;
BEGIN
  IF s IS NOT NULL THEN
    SELECT coalesce((m.tools->>'talk')::boolean, false) INTO talk FROM verse.made_realms m WHERE m.slug = s;
    IF NOT FOUND THEN RETURN '{"status":404,"error":"no such realm"}'; END IF;
    IF NOT talk AND NOT verse.builder_owns(k, s) THEN RETURN '{"status":403,"error":"the owner hasn''t let visitors talk here"}'; END IF;
  END IF;
  RETURN verse.npc_ask_0060(body, cap, k);
END $$;
