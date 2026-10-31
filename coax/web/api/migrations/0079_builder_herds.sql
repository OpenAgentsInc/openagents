-- Nothing asked for is dropped (byo-never-drop; docs/verse/realm-builder.md "What can't be drawn
-- yet"). "Only pastel colours, bison everywhere and tepees" came back as a flat stone ground and
-- nothing else: the catalogue had no animals or dwellings, no colour palette, and what it couldn't
-- make was left out without a word. Now:
--   - the catalogue has a herd (bison, deer or horses, which roam), dwellings (tepees, tents or round
--     huts) and a village (its people going about their day, plain figures from the characters' kit,
--     never a caricature); myrealm.coax draws them;
--   - a realm has a palette (natural, pastel, vivid, muted) that tints all of it;
--   - the planner already names what was asked for ("asked", 0065), each with the feature that
--     makes it or none: every one with none is kept on the board as wanted ("want THING" lines,
--     NOT APPLIED YET on the page; at most 24), carried through each revision, and becomes a waiting ask on the
--     realm's feature list (0078) when it is made. An ADD in edit mode keeps them the same way.
-- No words are matched here: the model reads them and only its typed answer is checked.

-- ---- the catalogue
ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0078;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0078() || '{
    "dwellings": {"says": "dwellings standing together: tepees (tall cones of hide on poles), tents, or round huts with thatched roofs", "variant": ["tepee", "tent", "hut"], "count": [1, 8, 4], "most": 1},
    "village":   {"says": "a village: its people going about their day, walking between the houses and dwellings (plain figures, never a caricature)", "count": [3, 8, 5], "most": 1},
    "herd":      {"says": "a herd of animals that roam and graze across the land: bison, deer or horses", "variant": ["bison", "deer", "horse"], "count": [3, 10, 6], "most": 1}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0078;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['house', 'dwellings', 'village', 'herd'] || array_remove(verse.builder_order_0078(), 'house') $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0078;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f->>'feature'
    WHEN 'herd' THEN 'A herd of ' || (f->>'count') || ' ' || CASE f->>'variant' WHEN 'horse' THEN 'horses' ELSE f->>'variant' END
    WHEN 'dwellings' THEN CASE WHEN (f->>'count')::int = 1 THEN 'A ' || (f->>'variant') ELSE (f->>'count') || ' ' || (f->>'variant') || 's' END
    WHEN 'village' THEN 'A village of ' || (f->>'count') || ' people'
    ELSE verse.builder_label_0078(f) END $$;
ALTER FUNCTION verse.builder_phrase(jsonb) RENAME TO builder_phrase_0078;
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' IN ('herd', 'dwellings', 'village') THEN lower(left(verse.builder_label(f), 1)) || substr(verse.builder_label(f), 2)
              ELSE verse.builder_phrase_0078(f) END $$;

-- ---- the palette: a tint over the whole realm
CREATE FUNCTION verse.builder_palettes() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['natural', 'pastel', 'vivid', 'muted'] $$;
ALTER TABLE verse.made_realms ADD COLUMN palette text NOT NULL DEFAULT 'natural';

-- What was wanted and can't be drawn yet: the earlier ones, then each asked thing with no feature,
-- a thing once (by its words, any case), at most 24, each at most 60 characters.
CREATE FUNCTION verse.builder_wanted(prev jsonb, asked jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY o), '[]') FROM (
    SELECT t, o FROM (
      SELECT DISTINCT ON (lower(t)) t, o FROM (
        SELECT verse.builder_line(v, 60) t, o FROM jsonb_array_elements(CASE WHEN jsonb_typeof(prev) = 'array' THEN prev ELSE '[]' END) WITH ORDINALITY x(v, o)
        UNION ALL
        SELECT verse.builder_line(a->'thing', 60), 100 + o FROM jsonb_array_elements(CASE WHEN jsonb_typeof(asked) = 'array' THEN asked ELSE '[]' END) WITH ORDINALITY y(a, o)
        WHERE a->>'feature' = 'none') u
      WHERE t <> '' ORDER BY lower(t), o) d
    ORDER BY o LIMIT 24) e $$;

-- A board held: the palette (natural unless named) and what is wanted ride along.
ALTER FUNCTION verse.builder_hold(jsonb) RENAME TO builder_hold_0078;
CREATE FUNCTION verse.builder_hold(d jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT h || jsonb_build_object('palette', verse.builder_pick(d->'palette', verse.builder_palettes()),
                                 'wanted', verse.builder_wanted(d->'wanted', NULL))
  FROM (SELECT verse.builder_hold_0078(d) AS h) x WHERE h IS NOT NULL $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0078;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0078()
    || E'\nPalettes: natural (true colours), pastel (soft, pale colours), vivid (bright, saturated), muted (greyed, quiet). '
    || 'Choose the one that fits the colours they asked for; natural when they named none. '
    || 'In "asked", list every thing they asked for, also each one no feature makes (feature none): none of them is ever left out. '
    || 'When they ask for a people or a culture, the village feature shows its people plainly and respectfully, as everyday people, never a caricature.' $$;

ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0078;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(s,
      '{properties,palette}', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.builder_palettes()))),
      '{required}', (s->'required') || '["palette"]'::jsonb),
      '{propertyOrdering}', coalesce(s->'propertyOrdering', s->'required') || '["palette"]'::jsonb)
  FROM (SELECT verse.builder_schema_0078() AS s) x $$;

-- The board's lines: "palette P" and a "want THING" line for each thing wanted.
CREATE OR REPLACE FUNCTION verse.builder_lines(plan bigint, b jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'plan ' || plan || E'\nname ' || (b->>'name') || E'\nstyle ' || (b->>'style') || E'\nground ' || (b->>'ground') || E'\ntime ' || (b->>'time')
      || E'\ncamera ' || coalesce(b->>'camera', 'third') || E'\npalette ' || coalesce(b->>'palette', 'natural') || E'\n'
      || coalesce((SELECT string_agg('f ' || (f->>'feature') || ' ' || CASE WHEN (f->>'on')::boolean THEN '1' ELSE '0' END || ' ' || (f->>'label'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(b->'features') WITH ORDINALITY AS t(f, o)) || E'\n', '')
      || coalesce((SELECT string_agg('want ' || (w #>> '{}'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(coalesce(b->'wanted', '[]')) WITH ORDINALITY AS t(w, o)) || E'\n', '') $$;

-- The planner's answer: what it named with no feature is kept as wanted, with what was wanted before.
ALTER FUNCTION verse.builder_answer(jsonb, int, text, text, int, jsonb) RENAME TO builder_answer_0078;
CREATE FUNCTION verse.builder_answer(asked jsonb, http int, response text, model text, taken int, request jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pid bigint := (asked->>'id')::bigint;
  out jsonb := verse.builder_answer_0078(asked, http, response, model, taken, request);
  prev jsonb;
  nb jsonb;
  d jsonb;
BEGIN
  IF (out->>'status')::int <> 200 OR out ? 'retry' OR NOT out ? 'board' THEN RETURN out; END IF;
  SELECT board_in->'wanted' INTO prev FROM verse.realm_plans WHERE id = pid;
  -- every thing the planner named (not only the first six it checks the board against)
  BEGIN d := verse.builder_said(response)::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
  nb := (out->'board') || jsonb_build_object('wanted', verse.builder_wanted(verse.builder_wanted(prev, NULL) || coalesce(out->'board'->'wanted', '[]'),
                                                                            CASE WHEN jsonb_typeof(d) = 'object' THEN d->'asked' ELSE out->'asked' END));
  UPDATE verse.realm_plans SET board = nb WHERE id = pid;
  RETURN out || jsonb_build_object('board', nb, 'lines', verse.builder_lines(pid, nb));
END $$;

-- MAKE IT: the palette goes in, and each thing wanted waits on the realm's feature list (0078).
ALTER FUNCTION verse.builder_make(jsonb, text, int) RENAME TO builder_make_0078;
CREATE FUNCTION verse.builder_make(body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb := verse.builder_make_0078(body, k, per_day);
  rec verse.players := verse.builder_player(k);
  b jsonb;
BEGIN
  IF (out->>'status')::int <> 200 THEN RETURN out; END IF;
  b := verse.builder_board_of(body, rec);
  UPDATE verse.made_realms SET palette = verse.builder_pick(b->'palette', verse.builder_palettes()) WHERE slug = out->>'slug';
  INSERT INTO verse.realm_asks (slug, ms, player, words, state)
  SELECT out->>'slug', verse.now_ms(), rec.id, w #>> '{}', 'waiting'
  FROM jsonb_array_elements(verse.builder_wanted(b->'wanted', NULL)) WITH ORDINALITY AS t(w, o) ORDER BY o;
  RETURN out;
END $$;

-- The spec: "palette P" too (myrealm.coax tints the realm with it).
ALTER FUNCTION verse.builder_spec(text, text) RENAME TO builder_spec_0078;
CREATE FUNCTION verse.builder_spec(s text, k text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT verse.builder_spec_0078(s, k) || 'palette ' || m.palette || E'\n' FROM verse.made_realms m WHERE m.slug = s $$;

CREATE OR REPLACE FUNCTION verse.made_realms_edited() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF (NEW.name, NEW.style, NEW.ground, NEW.time, NEW.camera, NEW.features, NEW.placed, NEW.off, NEW.description, NEW.palette)
     IS DISTINCT FROM (OLD.name, OLD.style, OLD.ground, OLD.time, OLD.camera, OLD.features, OLD.placed, OLD.off, OLD.description, OLD.palette) THEN
    NEW.edited_ms := verse.now_ms();
  END IF;
  RETURN NEW;
END $$;

-- Edit mode's revision of a made realm: the model sees the palette, and its answer sets it.
ALTER FUNCTION verse.edit_revise_ask(text, jsonb, int, text, boolean) RENAME TO edit_revise_ask_0078;
CREATE FUNCTION verse.edit_revise_ask(s text, body jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  x jsonb := verse.edit_revise_ask_0078(s, body, cap, k, site_admin);
  p text := (SELECT palette FROM verse.made_realms WHERE slug = s AND (verse.fork_of(s)).slug IS NULL);
  t text;
BEGIN
  IF p IS NULL OR (x->>'status')::int <> 200 THEN RETURN x; END IF;
  t := x #>> '{request,contents,0,parts,0,text}';
  RETURN jsonb_set(x, '{request,contents,0,parts,0,text}', to_jsonb(replace(t, 'The current board: {', 'The current board: {"palette": "' || p || '", ')));
END $$;
-- A revision's answer sets the palette: the board it keeps (0050's edit_revise_answer holds it
-- with builder_hold, which now carries the palette) is the realm's. A trigger, since that function
-- names itself inside (a rename would break it).
CREATE FUNCTION verse.realm_plans_palette() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.slug IS NOT NULL AND jsonb_typeof(NEW.board) = 'object' AND jsonb_typeof(NEW.board->'palette') = 'string' AND (verse.fork_of(NEW.slug)).slug IS NULL THEN
    UPDATE verse.made_realms SET palette = verse.builder_pick(NEW.board->'palette', verse.builder_palettes())
    WHERE slug = NEW.slug AND palette IS DISTINCT FROM verse.builder_pick(NEW.board->'palette', verse.builder_palettes());
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER realm_plans_palette AFTER UPDATE OF board ON verse.realm_plans FOR EACH ROW EXECUTE FUNCTION verse.realm_plans_palette();

-- An ask's state holds a made realm's palette too, so DELETE puts it back.
CREATE OR REPLACE FUNCTION verse.ask_state(s text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  m verse.made_realms;
  e verse.realm_edit;
BEGIN
  SELECT * INTO m FROM verse.made_realms WHERE slug = s;
  IF m.slug IS NOT NULL THEN
    RETURN jsonb_build_object('made', true, 'ground', m.ground, 'time', m.time, 'palette', m.palette,
      'rows', coalesce((SELECT jsonb_object_agg(verse.edit_key(f), f) FROM jsonb_array_elements(verse.edit_made_board(m)) f), '{}'));
  END IF;
  SELECT * INTO e FROM verse.realm_edit WHERE slug = s;
  RETURN jsonb_build_object('made', false,
    'rows', coalesce((SELECT jsonb_object_agg(b->>'id', jsonb_build_object('on', (b->>'on')::boolean, 'raw', e.switched->(b->>'id')))
                      FROM jsonb_array_elements(verse.edit_hand_board(s)) b), '{}'));
END $$;
ALTER FUNCTION verse.ask_diff(jsonb, jsonb) RENAME TO ask_diff_0078;
CREATE FUNCTION verse.ask_diff(b jsonb, a jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.ask_diff_0078(b, a)
    || CASE WHEN b->>'palette' IS DISTINCT FROM a->>'palette' THEN jsonb_build_object('palette', jsonb_build_array(b->'palette', a->'palette')) ELSE '{}' END $$;
CREATE OR REPLACE FUNCTION verse.ask_changed(d jsonb) RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(d->'after', '{}') <> '{}' OR d ? 'ground' OR d ? 'time' OR d ? 'palette' $$;
ALTER FUNCTION verse.ask_revert(text, jsonb) RENAME TO ask_revert_0078;
CREATE FUNCTION verse.ask_revert(s text, did jsonb) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  n int := verse.ask_revert_0078(s, did);
BEGIN
  IF did ? 'palette' THEN
    UPDATE verse.made_realms SET palette = coalesce(did->'palette'->>0, palette) WHERE slug = s AND palette IS NOT DISTINCT FROM did->'palette'->>1;
  END IF;
  RETURN n;
END $$;

-- An ADD on a made realm: each thing it named that no feature makes waits on the list as its own ask
-- (when the rest of it was applied; one that changed nothing is that ask already).
ALTER FUNCTION verse.ask_answer(jsonb, int, text, text, int) RENAME TO ask_answer_0078;
CREATE FUNCTION verse.ask_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  out jsonb := verse.ask_answer_0078(asked, http, response, model, taken);
  a verse.realm_asks;
  d jsonb;
  extra text;
BEGIN
  SELECT * INTO a FROM verse.realm_asks WHERE id = (out->>'ask')::bigint;
  IF a.id IS NULL OR a.state <> 'applied' OR NOT verse.edit_made(a.slug) THEN RETURN out; END IF;
  BEGIN d := verse.builder_said(response)::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
  IF jsonb_typeof(d) <> 'object' THEN RETURN out; END IF;
  INSERT INTO verse.realm_asks (slug, ms, player, words, state)
  SELECT a.slug, verse.now_ms(), a.player, w #>> '{}', 'waiting' FROM jsonb_array_elements(verse.builder_wanted(NULL, d->'asked')) WITH ORDINALITY AS t(w, o) ORDER BY o;
  RETURN out;
END $$;
