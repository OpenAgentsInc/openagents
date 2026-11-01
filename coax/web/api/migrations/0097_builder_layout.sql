-- Realm builder: a place built round a middle (#12610; docs/verse/realm-builder.md "The site").
-- "a stone courtyard with a fountain" made an open plain: the board had a stone wall and a fountain
-- as props, set down anywhere. The planner now plans the place itself, with four features the page
-- lays out together (apps/openagents-web/myrealm-site.coax):
--   enclosure  walls all round the middle, a gate north and south: stone, wood (a rail fence),
--              palisade (sharpened logs) or hedge; small, medium or large
--   centre     the one thing in the middle: fountain, statue, well, campfire, tower, stones (a ring of
--              standing stones), tree (a giant tree) or pond
--   layout     how the rest stands round it: square (paved), garden (beds inside the walls), clearing
--              (the trees round open ground), ring, village (houses round a green)
--   path       stone (flagstones) or earth, from where you start through the middle
-- and a dock (props) from the shore over the water. A thing in the middle is the centre only: the
-- same thing as a prop, the tower as a building or the tree as a big tree is dropped when held.
-- The layout eval (coax/web/realm-layout-eval.coax, eval_layout.tsv) scores what is laid out.
-- The four take their kind in a field of their own, "kind" (held into the feature's variant, so the
-- spec reads "feature enclosure medium stone"): one enum of every variant past about 90 words is
-- refused by the model's API (400), and the shared variant enum stood at 81.
-- No words are matched here: the model reads them and only its typed answer is checked.

ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0096;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0096() || '{
    "enclosure": {"says": "walls all the way round the middle of the realm, a gateway in two sides; you start inside: stone walls (a courtyard, a cloister, a walled garden or yard, a castle courtyard), a wooden rail fence (a farmyard, a paddock, a fenced camp), a palisade of sharpened logs (a fort, a stockade), or a clipped hedge (a hedged garden); small, medium or large", "kind": ["stone", "wood", "palisade", "hedge"], "size": ["medium", "small", "large"], "most": 1},
    "centre":    {"says": "the one thing standing in the very middle, everything else round it: a fountain, a statue, a well, a campfire, a stone tower, a ring of standing stones, a giant tree or a round pond", "kind": ["fountain", "statue", "well", "campfire", "tower", "stones", "tree", "pond"], "most": 1},
    "layout":    {"says": "how the rest stands round the middle: square (paved in flagstones: a courtyard, a plaza, a town or market square), garden (flower beds and trees inside the walls), clearing (open ground with the trees all round it), ring (lanterns, stones, tents or houses in a circle round it), or village (houses round a green, facing it)", "kind": ["square", "garden", "clearing", "ring", "village"], "most": 1},
    "path":      {"says": "a path from where you start through the middle and on: stone (flagstones, cobbles) or earth (a trodden dirt track)", "kind": ["stone", "earth"], "most": 1},
    "props":     {"says": "things standing about: crates, barrels, wagons, a fence, a camp (a chest and an anvil), a campfire, boats, a bridge, a signpost, a well, treasure (a chest and gold), gravestones, tumbleweeds, a statue, a fountain (a stone basin, water rising in it), a stone wall, a shipwreck (a broken hull on its side), or a dock (a wooden jetty or pier out over the water, a boat tied to it; with water)", "variant": ["crates", "barrels", "wagon", "fence", "camp", "campfire", "boat", "bridge", "signpost", "well", "treasure", "graves", "tumbleweeds", "statue", "fountain", "wall", "wreck", "dock"], "most": 8}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0096;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT o[1:p] || ARRAY['enclosure', 'centre', 'layout', 'path'] || o[p + 1:]
  FROM (SELECT verse.builder_order_0096() AS o) x, LATERAL (SELECT array_position(x.o, 'mist') AS p) y $$;

-- A feature whose catalogue entry has kinds: its kind (or its variant, as a board sent back has it)
-- held to them, as its variant, and its words written again.
ALTER FUNCTION verse.builder_hold_feature(jsonb) RENAME TO builder_hold_feature_0096;
CREATE FUNCTION verse.builder_hold_feature(d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  h jsonb := verse.builder_hold_feature_0096(d);
  c jsonb := verse.builder_catalogue()->(h->>'feature');
  v jsonb;
BEGIN
  IF h IS NULL OR NOT (c ? 'kind') THEN RETURN h; END IF;
  v := CASE WHEN jsonb_typeof(d->'kind') = 'string' THEN d->'kind' ELSE d->'variant' END;
  h := h || jsonb_build_object('variant', verse.builder_pick(v, c->'kind'));
  RETURN h || jsonb_build_object('label', verse.builder_label(h));
END $$;
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0096;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(s, '{properties,features,items,properties,kind}', jsonb_build_object('type', 'string', 'enum',
           (SELECT jsonb_agg(DISTINCT k) FROM jsonb_each(verse.builder_catalogue()) e, jsonb_array_elements_text(coalesce(e.value->'kind', '[]')) k)))
  FROM (SELECT verse.builder_schema_0096() AS s) x $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0096;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN f->>'feature' = 'enclosure' THEN
      CASE f->>'variant' WHEN 'wood' THEN 'A wooden fence' WHEN 'palisade' THEN 'A palisade of logs' WHEN 'hedge' THEN 'A hedge' ELSE 'Stone walls' END
      || ' round a ' || CASE f->>'size' WHEN 'small' THEN 'small ' WHEN 'large' THEN 'large ' ELSE '' END
      || CASE f->>'variant' WHEN 'hedge' THEN 'garden' ELSE 'yard' END || ', a gate each end'
    WHEN f->>'feature' = 'centre' THEN
      CASE f->>'variant' WHEN 'statue' THEN 'A statue' WHEN 'well' THEN 'A well' WHEN 'campfire' THEN 'A campfire'
                         WHEN 'tower' THEN 'A stone tower' WHEN 'stones' THEN 'A ring of standing stones' WHEN 'tree' THEN 'A giant tree'
                         WHEN 'pond' THEN 'A round pond' ELSE 'A fountain' END || ' in the middle'
    WHEN f->>'feature' = 'layout' THEN
      CASE f->>'variant' WHEN 'garden' THEN 'A garden: beds and trees inside' WHEN 'clearing' THEN 'A clearing, the trees all round it'
                         WHEN 'ring' THEN 'Set in a ring round the middle' WHEN 'village' THEN 'Houses round a green, facing it'
                         ELSE 'A square paved in flagstones' END
    WHEN f->>'feature' = 'path' THEN CASE f->>'variant' WHEN 'earth' THEN 'An earth path' ELSE 'A flagstone path' END || ' through the middle'
    WHEN f->>'feature' = 'props' AND f->>'variant' = 'dock' THEN 'A wooden dock on the water'
    ELSE verse.builder_label_0096(f) END $$;
ALTER FUNCTION verse.builder_phrase(jsonb) RENAME TO builder_phrase_0096;
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' IN ('enclosure', 'centre', 'layout', 'path') OR (f->>'feature' = 'props' AND f->>'variant' = 'dock')
              THEN lower(left(verse.builder_label(f), 1)) || substr(verse.builder_label(f), 2)
              ELSE verse.builder_phrase_0096(f) END $$;

-- A board held: what stands in the middle is the centre only.
ALTER FUNCTION verse.builder_hold(jsonb) RENAME TO builder_hold_0096;
CREATE FUNCTION verse.builder_hold(d jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT h || jsonb_build_object('features', coalesce((
    SELECT jsonb_agg(f ORDER BY o) FROM jsonb_array_elements(h->'features') WITH ORDINALITY AS t(f, o)
    WHERE c IS NULL OR NOT (
         (f->>'feature' = 'props' AND f->>'variant' = c AND c IN ('fountain', 'statue', 'well', 'campfire'))
      OR (f->>'feature' = 'building' AND f->>'variant' = 'tower' AND c = 'tower')
      OR (f->>'feature' = 'bigtree' AND c = 'tree')
      OR (f->>'feature' = 'water' AND coalesce(f->>'variant', 'pond') = 'pond' AND c = 'pond'))), '[]'))
  FROM (SELECT verse.builder_hold_0096(d) AS h) y,
       LATERAL (SELECT (SELECT f->>'variant' FROM jsonb_array_elements(y.h->'features') f WHERE f->>'feature' = 'centre' LIMIT 1) AS c) z
  WHERE h IS NOT NULL $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0096;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0096()
    || E'\nA place built round a middle is planned as one: a courtyard, cloister, walled garden or yard, farmyard, '
    || 'fort or stockade, town or village square, plaza, clearing, a circle of stones, or anything "in the middle", '
    || '"at the centre" or "round" something. Its walls or fence are feature enclosure (stone walls for stone, a wooden '
    || 'rail fence, a palisade of logs for a fort, or a hedge); the thing in its very middle is feature centre; how the rest '
    || 'stands round it is feature layout (a courtyard, cloister or paved square is square; a garden inside walls or a hedge '
    || 'is garden; a clearing is clearing with trees; a ring or circle of lanterns, stones or tents is ring; houses or '
    || 'cottages round a green are village); a path, track or cobbled way to it is feature path. A thing in the middle is '
    || 'the centre only, never also a prop, a building or a big tree. A courtyard of stone is enclosure stone, layout square '
    || 'and ground stone. Each of enclosure, centre, layout and path says which in its field kind: '
    || (SELECT string_agg(n || ' kind ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(verse.builder_catalogue()->n->'kind') k), '; ')
        FROM unnest(ARRAY['enclosure', 'centre', 'layout', 'path']) n) || '. A dock, jetty or pier is props dock, with water: a lake is a large pond. '
    || 'Missing is "" (an empty string) when nothing of the thing is missing: never the word empty, and never "none".' $$;
