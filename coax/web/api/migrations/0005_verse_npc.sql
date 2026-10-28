-- Character chat (pt-chat-endpoint; npc.c answers POST /api/playtest/npc/ask).
-- The demo's prompt and memory (aethyr-cpp tools/npc_chat.py at
-- demo/kiki-2026-10-28d): the character's name, role, greeting and what
-- players taught it, the world's name, the game's controls, the last ten
-- lines of the conversation and the question; answered in one to three
-- short sentences in character. Google first: Gemini on Vertex AI. A day's
-- answers are capped across the server (the demo's 2,000), and every
-- question and answer is kept.
CREATE TABLE verse.npc_day (day date PRIMARY KEY, n int NOT NULL);
CREATE TABLE verse.npc_log (
  id bigserial PRIMARY KEY,
  ms bigint NOT NULL,
  world text, npc text, question text NOT NULL,
  answer text,                   -- null when the model gave none
  error text,
  model text,
  ms_taken int
);

CREATE FUNCTION verse.npc_text(v jsonb, n int) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT left(btrim(CASE WHEN jsonb_typeof(v) IN ('string', 'number', 'boolean') THEN v #>> '{}' ELSE '' END), n) $$;

-- The Gemini request for a question, or {"status", "error"} when it can't be
-- asked: no question (400), or the day's answers used up (503).
CREATE FUNCTION verse.npc_request(body jsonb, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  npc jsonb := CASE WHEN jsonb_typeof(body->'npc') = 'object' THEN body->'npc' ELSE '{}' END;
  question text := verse.npc_text(body->'question', 500);
  name text := coalesce(nullif(verse.npc_text(npc->'name', 60), ''), 'Someone');
  role text := verse.npc_text(npc->'role', 120);
  knows text := verse.npc_text(npc->'knowledge', 4000);
  greet text := verse.npc_text(npc->'greet', 300);
  world text := coalesce(nullif(verse.npc_text(body->'world', 80), ''), 'this world');
  game text := 'Controls: W A S D walk, Shift runs, Space jumps, drag to look. B opens edit mode, '
    'where 1-9 pick a thing on the hotbar and a click places it, and the box takes typed '
    'or spoken changes ("add three tombstones in front of me", "build a cottage", '
    '"spawn a cat", "make it night"). X is delete mode: click a thing you placed to '
    'remove it; undo brings it back. Players teach characters in edit mode: '
    '"teach <name> that ...".';
  sys text;
  msgs jsonb := '[]';
  line jsonb;
  who text;
  said text;
  used int;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' OR question = '' THEN
    RETURN '{"status":400,"error":"ask something"}';
  END IF;
  INSERT INTO verse.npc_day AS d (day, n) VALUES (current_date, 1)
  ON CONFLICT (day) DO UPDATE SET n = d.n + 1 RETURNING n INTO used;
  IF used > cap THEN RETURN '{"status":503,"error":"the day''s answers are used up"}'; END IF;
  sys := 'You are ' || name || ', a character in ' || world || ', a small 3D world in the Verse where '
    || 'players walk around, build and talk to characters. '
    || CASE WHEN role <> '' THEN 'Your role: ' || role || '. ' ELSE '' END
    || CASE WHEN greet <> '' THEN 'You greet people with: ' || greet || ' ' ELSE '' END
    || 'Stay in character. Answer in one to three short, plain sentences, as spoken '
    || 'dialogue: no lists, no markdown, no stage directions. Be warm and a little '
    || 'playful. If asked something you could not know, say so in character. '
    || 'If a player asks how to do something in the game, use this: ' || game
    || CASE WHEN knows <> '' THEN E'\n\nWhat players have taught you (true in this world):\n' || knows ELSE '' END;
  -- The last ten lines, a speaker's lines run together, starting with the player's.
  FOR line IN SELECT e FROM (SELECT e, ord FROM jsonb_array_elements(CASE WHEN jsonb_typeof(body->'history') = 'array'
                THEN body->'history' ELSE '[]' END) WITH ORDINALITY AS h(e, ord) ORDER BY ord DESC LIMIT 10) t ORDER BY ord LOOP
    CONTINUE WHEN jsonb_typeof(line) <> 'object';
    who := CASE WHEN line->>'from' = 'npc' THEN 'model' ELSE 'user' END;
    said := verse.npc_text(line->'text', 500);
    CONTINUE WHEN said = '';
    IF jsonb_array_length(msgs) > 0 AND msgs->-1->>'role' = who THEN
      msgs := jsonb_set(msgs, '{-1,parts,0,text}', to_jsonb((msgs->-1->'parts'->0->>'text') || E'\n' || said));
    ELSE
      msgs := msgs || jsonb_build_array(jsonb_build_object('role', who, 'parts', jsonb_build_array(jsonb_build_object('text', said))));
    END IF;
  END LOOP;
  IF jsonb_array_length(msgs) > 0 AND msgs->-1->>'role' = 'user' THEN
    msgs := jsonb_set(msgs, '{-1,parts,0,text}', to_jsonb((msgs->-1->'parts'->0->>'text') || E'\n' || question));
  ELSE
    msgs := msgs || jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', question))));
  END IF;
  IF msgs->0->>'role' <> 'user' THEN msgs := msgs - 0; END IF;
  RETURN jsonb_build_object('status', 200, 'npc', name, 'world', world, 'question', question, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', sys))),
    'contents', msgs,
    'generationConfig', jsonb_build_object('maxOutputTokens', 2000, 'thinkingConfig', jsonb_build_object('thinkingLevel', 'LOW'))));
END $$;

-- The model's answer (Gemini's generateContent response) kept with the
-- question: {"status": 200, "text"} or {"status": 503, "error"}.
CREATE FUNCTION verse.npc_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
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
  INSERT INTO verse.npc_log (ms, world, npc, question, answer, error, model, ms_taken)
  VALUES (verse.now_ms(), asked->>'world', asked->>'npc', asked->>'question', left(t, 600), err, model, taken);
  IF t IS NULL THEN RETURN jsonb_build_object('status', 503, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'text', left(t, 600), 'model', model);
END $$;
