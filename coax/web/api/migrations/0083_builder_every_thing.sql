-- Every thing asked for is drawn as asked or listed as still wanted (byo-never-drop). "A floating
-- island village with a giant tree and friendly owls" came back as crows and round trees, and
-- neither the owls nor the giant tree nor the floating island was on the list of things wanted: the
-- planner named only the main things, and a thing it could draw something like (crows for owls) it
-- counted as drawn. Now:
--   - the flock has owls, and a giant tree is a feature of its own (bigtree: one tree, alone, four
--     times the others; round, pine or dead); myrealm.coax draws both. A floating island isn't drawn
--     yet, so it is listed;
--   - the planner names every thing asked for, each with its own words, and says of each whether its
--     feature draws it as they said it ("same"); one it only stands in for (crows for owls, a pond for
--     a sea, a house for a barn) waits on the board as wanted, as one with no feature does.
-- eval-builder.sh scores twenty prompts with a judge model: the share of things asked for that the
-- board shows or lists. No words are matched here: the model reads them and only its typed answer is
-- checked.

ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0082;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0082() || '{
    "flock":   {"says": "a flock of birds that circles whoever walks: crows, bats, or owls (tawny, broad winged)", "variant": ["crow", "bat", "owl"], "count": [3, 12, 7], "most": 1},
    "bigtree": {"says": "one giant tree standing alone, four times as tall as any other: a great old tree, a world tree, the tree a realm is built round", "variant": ["round", "pine", "dead"], "most": 1}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0082;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT o[1:p] || ARRAY['bigtree'] || o[p + 1:] FROM (SELECT verse.builder_order_0082() AS o) x, LATERAL (SELECT array_position(x.o, 'trees') AS p) y $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0082;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f->>'feature'
    WHEN 'bigtree' THEN CASE f->>'variant' WHEN 'pine' THEN 'A giant pine' WHEN 'dead' THEN 'A giant dead tree' ELSE 'A giant tree' END
    ELSE verse.builder_label_0082(f) END $$;
ALTER FUNCTION verse.builder_phrase(jsonb) RENAME TO builder_phrase_0082;
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' = 'bigtree' THEN lower(left(verse.builder_label(f), 1)) || substr(verse.builder_label(f), 2)
              ELSE verse.builder_phrase_0082(f) END $$;

-- Each asked thing says what its feature draws for it ("drawn", the words its board line will
-- have), then whether that is the thing as asked ("same"): writing what is drawn first is what lets
-- the model see a stand-in (asked "cherry blossom trees", drawn "round trees").
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0082;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(jsonb_set(s,
      '{properties,asked,items,properties,drawn}', '{"type": "string"}'::jsonb),
      '{properties,asked,items,properties,same}', '{"type": "boolean"}'::jsonb),
      '{properties,asked,items,required}', '["thing", "feature", "drawn", "same"]'::jsonb),
      '{properties,asked,items,propertyOrdering}', '["thing", "feature", "drawn", "same"]'::jsonb)
  FROM (SELECT verse.builder_schema_0082() AS s) x $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0082;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0082()
    || E'\nIn "asked", name every thing they asked for, never only the main ones: each creature, person, building, plant, '
    || 'place, landform and terrain they named, in their own few words with what makes it what it is ("giant tree", '
    || '"floating island", "palm trees", "red barn", "frozen lake", "koi pond", "stone lanterns"). Give each the feature that '
    || 'draws it (or setup, or none); in drawn, what that feature will show for it, in the few words its board line has '
    || '("9 round trees", "a flock of 7 crows", "crates", "a medium pond"; nothing for none); '
    || 'and same: true only when what is drawn is the thing they said, with every word of it (its kind, its colour, its size, '
    || 'what it is made of); '
    || 'false when it only stands in for it (crows for owls, round trees for palm trees, a pond for a sea or a river, a house '
    || 'for a barn or a castle, a meadow for a floating island, fewer than they asked for). Put the stand-in on the board '
    || 'too when it fits. Every thing with same false or feature none stays on the board as still wanted, in those words, '
    || 'so an agent can build it later: name each exactly, once. Words about mood (friendly, cosy, quiet, spooky) are not things '
    || 'and change nothing of what is drawn. How many they asked for sets the count: a few is near the least, many, lots, '
    || 'everywhere or a forest of them is the most; when that is more than the most, the thing (with its words: "bison '
    || 'everywhere") is same false.' $$;

-- What is still wanted: what no feature makes, and now what a feature only stands in for.
CREATE OR REPLACE FUNCTION verse.builder_wanted(prev jsonb, asked jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY o), '[]') FROM (
    SELECT t, o FROM (
      SELECT DISTINCT ON (lower(t)) t, o FROM (
        SELECT verse.builder_line(v, 60) t, o FROM jsonb_array_elements(CASE WHEN jsonb_typeof(prev) = 'array' THEN prev ELSE '[]' END) WITH ORDINALITY x(v, o)
        UNION ALL
        SELECT verse.builder_line(a->'thing', 60), 100 + o FROM jsonb_array_elements(CASE WHEN jsonb_typeof(asked) = 'array' THEN asked ELSE '[]' END) WITH ORDINALITY y(a, o)
        WHERE a->>'feature' = 'none' OR (jsonb_typeof(a->'same') = 'boolean' AND NOT (a->>'same')::boolean)) u
      WHERE t <> '' ORDER BY lower(t), o) d
    ORDER BY o LIMIT 24) e $$;
