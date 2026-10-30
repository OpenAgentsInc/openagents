-- Voice edits, fast (eg-voice-fast): the planner answers with its thinking off (thinkingBudget 0), which on
-- gemini-2.5-flash (npc.c OA_EDIT_MODEL's default since this) takes the logged sentences from a p95 of 7.7 s
-- (gemini-3.8-flash, thinking LOW) to 1.6 s with the same answers. And one at a time means one with the model:
-- the page sends sentences said meanwhile in order (everglade-voice.coax), each the moment the last is answered,
-- so an answered one no longer holds the next back for 1.5 s (one never answered holds it 30 s at most).
CREATE OR REPLACE FUNCTION verse.voice_edit_ask(realm text, body jsonb, pubkey text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sentence text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'sentence', 300) ELSE '' END;
  row_id bigint;
BEGIN
  IF verse.voice_edit_catalogue(realm) IS NULL THEN RETURN '{"status":400,"error":"this realm takes no voice edits"}'; END IF;
  IF sentence = '' THEN RETURN '{"status":400,"error":"say what to change"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.voice_edits v WHERE v.pubkey = voice_edit_ask.pubkey AND v.ms > verse.now_ms() - 30000 AND v.ms_taken IS NULL) THEN
    RETURN '{"status":429,"error":"one edit at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.voice_edits WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s voice edits are used up"}';
  END IF;
  INSERT INTO verse.voice_edits (ms, realm, pubkey, sentence) VALUES (verse.now_ms(), realm, pubkey, sentence) RETURNING id INTO row_id;
  RETURN jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', verse.voice_edit_prompt(realm)))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text', sentence)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', verse.voice_edit_schema(realm),
      'temperature', 0, 'maxOutputTokens', 600, 'thinkingConfig', jsonb_build_object('thinkingBudget', 0))));
END $$;
