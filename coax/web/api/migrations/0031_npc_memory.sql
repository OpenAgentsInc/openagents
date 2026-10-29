-- Characters who remember you (eg-people): a character's memory of a player
-- across visits. npc.c now calls verse.npc_ask with the browser's player key
-- (X-Verse-Player). Every question and answer is already kept in
-- verse.npc_log; it now keeps who asked and on which visit (the page's own
-- stamp, body.visit), and a character is told what this player said to it
-- and what it answered on their earlier visits, so it can recall them. A
-- world may also name its own controls (body.controls), so a character in
-- Everglade doesn't explain the Haunted House's edit mode. Everything else
-- is 0005's and 0012's verse.npc_request.
ALTER TABLE verse.npc_log ADD COLUMN player text, ADD COLUMN visit text;
CREATE INDEX npc_log_memory ON verse.npc_log (player, world, npc, id) WHERE player IS NOT NULL;

CREATE FUNCTION verse.npc_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  res jsonb := verse.npc_request(body, cap);
  sys text;
  controls text := verse.npc_text(body->'controls', 1200);
  this_visit text := verse.npc_text(body->'visit', 64);
  who text;
  told text;
BEGIN
  IF (res->>'status')::int <> 200 THEN RETURN res; END IF;
  sys := res->'request'->'systemInstruction'->'parts'->0->>'text';
  IF controls <> '' THEN
    sys := regexp_replace(sys, 'If a player asks how to do something in the game, use this: [^\n]*',
                          'If a player asks how to do something in the game, use this: '
                            || replace(replace(controls, E'\\', E'\\\\'), E'\n', ' '));
  END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT id INTO who FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF who IS NOT NULL THEN
    -- Their last twelve exchanges with this character here, before this visit, oldest first.
    SELECT string_agg('They said: ' || question || E'\nYou answered: ' || answer, E'\n' ORDER BY id) INTO told
    FROM (SELECT l.id, l.question, l.answer FROM verse.npc_log l
          WHERE l.player = who AND l.world = res->>'world' AND l.npc = res->>'npc' AND l.answer IS NOT NULL
            AND (this_visit = '' OR l.visit IS DISTINCT FROM this_visit)
          ORDER BY l.id DESC LIMIT 12) t;
    IF told IS NOT NULL THEN
      sys := sys || E'\n\nThis player has talked with you before, on earlier visits. What they said and what you '
        || 'answered, oldest first (remember them, and bring up what they told you when it fits):' || E'\n' || told;
    END IF;
  END IF;
  RETURN jsonb_set(res, '{request,systemInstruction,parts,0,text}', to_jsonb(sys))
    || jsonb_build_object('player', who, 'visit', nullif(this_visit, ''));
END $$;

-- 0005's verse.npc_answer, keeping who asked and on which visit.
CREATE OR REPLACE FUNCTION verse.npc_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  r jsonb;
  t text;
  err text;
BEGIN
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', ' ') INTO t
    FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    t := nullif(btrim(t), '');
    IF t IS NULL THEN err := CASE WHEN r->'candidates'->0->>'finishReason' IN ('SAFETY', 'PROHIBITED_CONTENT', 'BLOCKLIST')
                                  THEN 'declined' ELSE 'no answer' END; END IF;
  ELSE
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  INSERT INTO verse.npc_log (ms, world, npc, question, answer, error, model, ms_taken, player, visit)
  VALUES (verse.now_ms(), asked->>'world', asked->>'npc', asked->>'question', left(t, 600), err, model, taken,
          asked->>'player', asked->>'visit');
  IF t IS NULL THEN RETURN jsonb_build_object('status', 503, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'text', left(t, 600), 'model', model);
END $$;
