-- Character chat's how-to (pt-chat-endpoint): the controls a character
-- names are the web's, not the old demo's. In a conversation ASK (T) types a
-- question, TEACH types something for them to remember, HINT (E) asks for a
-- hint and BYE (Esc) ends it. Everything else in the request is 0005's.
CREATE OR REPLACE FUNCTION verse.npc_request(body jsonb, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  npc jsonb := CASE WHEN jsonb_typeof(body->'npc') = 'object' THEN body->'npc' ELSE '{}' END;
  question text := verse.npc_text(body->'question', 500);
  name text := coalesce(nullif(verse.npc_text(npc->'name', 60), ''), 'Someone');
  role text := verse.npc_text(npc->'role', 120);
  knows text := verse.npc_text(npc->'knowledge', 4000);
  greet text := verse.npc_text(npc->'greet', 300);
  world text := coalesce(nullif(verse.npc_text(body->'world', 80), ''), 'this world');
  game text := 'Controls: W A S D or the arrows walk, Shift runs, Space jumps, drag to look; on a phone '
    'the left thumb walks and the right looks. E talks to whoever is near or opens a door. '
    'In a conversation the buttons are ASK (or T) to type a question, TEACH to type something '
    'for the character to remember, HINT (or E) for a hint and BYE (or Esc) to end it. '
    'V changes the camera and M shows the map. B opens edit mode: 1 to 5 place a jack-o''-lantern, '
    'candle, ghost, spider or tombstone in front of you, a click picks a thing, the arrows move it, '
    'R turns it, + and - resize it and Delete removes it; Z undoes, Y redoes, and B or Esc goes back '
    'to playing. The orbs lead through the house''s quest.';
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
