-- Build your own realm, fast (byo-plan-speed): the planner, SAY IT's pick and a realm's revision answer
-- with their thinking off (thinkingBudget 0), on gemini-2.5-flash (realms.c OA_BUILDER_MODEL's default
-- since this). Measured 2026-10-30 on the builder's own request: gemini-3.8-flash at thinking LOW took
-- 2.5 to 8.7 s with a 38.9 s outlier (staging's 20 s first board was that tail, not a cold start);
-- gemini-2.5-flash with thinking off took 1.1 to 1.9 s, with the same features picked. realms.c cuts a
-- try off at OA_BUILDER_TIMEOUT_MS (3500) and tries once more, as voice edits do.
ALTER FUNCTION verse.builder_ask(jsonb, int, text) RENAME TO builder_ask_0059;
ALTER FUNCTION verse.builder_pick_ask(jsonb, int, text) RENAME TO builder_pick_ask_0059;
ALTER FUNCTION verse.edit_revise_ask(text, jsonb, int, text, boolean) RENAME TO edit_revise_ask_0059;

CREATE FUNCTION verse.builder_fast(x jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(x->'request'->'generationConfig') = 'object'
              THEN jsonb_set(x, '{request,generationConfig,thinkingConfig}', '{"thinkingBudget": 0}')
              ELSE x END $$;

CREATE FUNCTION verse.builder_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.builder_fast(verse.builder_ask_0059(body, cap, k)) $$;
CREATE FUNCTION verse.builder_pick_ask(body jsonb, cap int, k text) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.builder_fast(verse.builder_pick_ask_0059(body, cap, k)) $$;
CREATE FUNCTION verse.edit_revise_ask(s text, body jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE sql AS $$
  SELECT verse.builder_fast(verse.edit_revise_ask_0059(s, body, cap, k, site_admin)) $$;
