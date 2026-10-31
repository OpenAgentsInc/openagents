-- Realm builder, round 3 (byo-never-drop; docs/verse/realm-builder.md). The 40-prompt eval
-- (eval-builder.sh) on 0088 drew 72.1% of the things asked for as asked, and 17 lines still wanted
-- named something the board drew. What it most often could not draw, and this round draws
-- (myrealm.coax, myrealm-things.coax, myrealm-shapes.coax):
--   - animals: elephants, giraffes, monkeys and squirrels (herd);
--   - trees: autumn trees (orange and red leaves) and acacias (flat tops, a savanna's);
--   - buildings: a pagoda (tiers of red roofs) and a longhouse (long, low, a steep turf roof);
--   - things standing about: a fountain, a stone wall and a shipwreck;
--   - mist lying over the land (feature mist): a foggy moor, a misty valley.
-- Words that said less than what is drawn: a market is a market of stalls, a small river is a
-- stream, a small house is a cottage, and the wagon prop is wagons (two are drawn).
-- Each asked thing a feature draws says "missing", the part of it the board does not draw as asked: its
-- line still wanted is that part, and none when the board draws all of it. "same" alone kept whole
-- things wanted that the board drew ("a giant tree" beside A giant tree, "glowing mushrooms" beside
-- mushrooms glowing after dark, "a pirate cove" as an island with treasure and parrots): the first
-- after-run of this round, with missing for places only, had 29 such lines.
-- A character is given a person's name, never its role again ("Keeper, lighthouse keeper" was a board line).
-- No words are matched here: the model reads them and only its typed answer is checked.

ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0091;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0091() || '{
    "mist":     {"says": "mist lying low over the land, thick in the distance: fog, a foggy moor, a misty valley", "most": 1},
    "building": {"says": "one building of a kind: a log cabin, a red barn, a windmill with turning sails, a lighthouse with a lamp, a castle (walls, four towers and a gate), a stone tower, a market of stalls under awnings, a shop with an awning and a sign, a wild west saloon with a false front, a stepped temple, ruins of broken columns, a pagoda (tiers of red roofs curling up at their corners), or a longhouse (long and low, a steep turf roof, a viking hall)", "variant": ["cabin", "barn", "windmill", "lighthouse", "castle", "tower", "market", "shop", "saloon", "temple", "ruins", "pagoda", "longhouse"], "most": 4},
    "herd":     {"says": "animals that roam and graze across the land, of one kind (two herds for two kinds): bison, deer, horses, sheep, goats, cows, pigs, wolves, foxes, dogs, camels, chickens, dinosaurs (long-necked, on four legs), elephants (trunks, tusks, big ears), giraffes (long necks, patched), monkeys or squirrels (bushy tails)", "variant": ["bison", "deer", "horse", "sheep", "goat", "cow", "pig", "wolf", "fox", "dog", "camel", "chicken", "dinosaur", "elephant", "giraffe", "monkey", "squirrel"], "count": [1, 10, 6], "most": 2},
    "trees":    {"says": "a grove of trees: round, pine, dead, mixed, palm trees, blossom trees (pink, like cherry blossom), autumn trees (orange and red leaves) or acacias (flat-topped, of a savanna)", "variant": ["round", "pine", "dead", "mixed", "palm", "blossom", "autumn", "acacia"], "count": [3, 20, 9], "most": 1},
    "props":    {"says": "things standing about: crates, barrels, wagons, a fence, a camp (a chest and an anvil), a campfire, boats, a bridge, a signpost, a well, treasure (a chest and gold), gravestones, tumbleweeds, a statue, a fountain (a stone basin, water rising in it), a stone wall, or a shipwreck (a broken hull on its side)", "variant": ["crates", "barrels", "wagon", "fence", "camp", "campfire", "boat", "bridge", "signpost", "well", "treasure", "graves", "tumbleweeds", "statue", "fountain", "wall", "wreck"], "most": 8}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0091;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT o[1:1] || ARRAY['mist'] || o[2:] FROM (SELECT verse.builder_order_0091() AS o) x $$;

ALTER FUNCTION verse.builder_many(text) RENAME TO builder_many_0091;
CREATE FUNCTION verse.builder_many(v text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE v WHEN 'monkey' THEN 'monkeys' ELSE verse.builder_many_0091(v) END $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0091;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN f->>'feature' = 'mist' THEN 'Mist over the land'
    WHEN f->>'feature' = 'building' AND f->>'variant' = 'pagoda' THEN 'A pagoda'
    WHEN f->>'feature' = 'building' AND f->>'variant' = 'longhouse' THEN 'A longhouse'
    WHEN f->>'feature' = 'building' AND f->>'variant' = 'market' THEN 'A market of stalls'
    WHEN f->>'feature' = 'water' AND f->>'variant' = 'river' AND f->>'size' = 'small' THEN 'A stream'
    WHEN f->>'feature' = 'trees' AND f->>'variant' = 'autumn' THEN (f->>'count') || ' autumn trees, orange and red'
    WHEN f->>'feature' = 'trees' AND f->>'variant' = 'acacia' THEN (f->>'count') || ' acacia trees'
    WHEN f->>'feature' = 'props' AND f->>'variant' = 'fountain' THEN 'A fountain'
    WHEN f->>'feature' = 'props' AND f->>'variant' = 'wall' THEN 'A stone wall'
    WHEN f->>'feature' = 'props' AND f->>'variant' = 'wreck' THEN 'A shipwreck'
    WHEN f->>'feature' = 'props' AND f->>'variant' = 'wagon' THEN 'Wagons'
    WHEN f->>'feature' = 'house' AND f->>'size' = 'small' THEN regexp_replace(verse.builder_label_0091(f), 'houses?$',
                                                                CASE WHEN (f->>'count')::int > 1 THEN 'cottages' ELSE 'cottage' END)
    ELSE verse.builder_label_0091(f) END $$;
ALTER FUNCTION verse.builder_phrase(jsonb) RENAME TO builder_phrase_0091;
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' = 'mist'
                OR (f->>'feature' = 'building' AND f->>'variant' IN ('pagoda', 'longhouse', 'market'))
                OR (f->>'feature' = 'water' AND f->>'variant' = 'river' AND f->>'size' = 'small')
                OR (f->>'feature' = 'trees' AND f->>'variant' IN ('autumn', 'acacia'))
                OR (f->>'feature' = 'herd' AND f->>'variant' IN ('elephant', 'giraffe', 'monkey', 'squirrel'))
                OR (f->>'feature' = 'props' AND f->>'variant' IN ('fountain', 'wall', 'wreck', 'wagon'))
              THEN lower(left(verse.builder_label(f), 1)) || substr(verse.builder_label(f), 2)
              ELSE verse.builder_phrase_0091(f) END $$;

-- Each asked thing that is a whole place says what of it the board does not draw.
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0091;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(s,
      '{properties,asked,items,properties,missing}', '{"type": "string"}'::jsonb),
      '{properties,asked,items,required}', '["said", "thing", "feature", "drawn", "same", "missing"]'::jsonb),
      '{properties,asked,items,propertyOrdering}', '["said", "thing", "feature", "drawn", "same", "missing"]'::jsonb)
  FROM (SELECT verse.builder_schema_0091() AS s) x $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0091;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0091()
    || E'\nFor each asked thing, missing is the part of it the board does not draw as asked, in a few words, and empty '
    || 'when a visitor would say the board draws all of it: it is what stays on the board as still wanted. "a giant tree" '
    || 'as a giant tree, "glowing mushrooms" as mushrooms glowing after dark, "a wooden cabin" as a log cabin, "sunset" as '
    || 'dusk and "snowy mountains" as mountains with snow ground are missing nothing; "a koi pond" drawn as a pond is '
    || 'missing "koi"; owls stood in for by crows are missing "owls"; a thing no feature draws (feature none) is '
    || 'missing all of it. A whole place (feature place) is never wanted whole: "a pirate cove" with an island, '
    || 'treasure and parrots is missing nothing; "a haunted swamp" with dead trees and a pond is missing "swamp"; '
    || '"a fishing village" with a village, boats and the sea is missing nothing; "a cyberpunk city street" is missing '
    || '"cyberpunk city street". A cottage is a small house; a stream is a small river; stalls are a market; '
    || 'fog and mist are feature mist; autumn or orange leaves are autumn trees; a t-rex is not the dinosaurs drawn (they '
    || 'are long-necked, on four legs), so it is same false. A character''s name is a name a person has (Old Tam, Mira, '
    || 'Captain Brae), never their role again: a lighthouse keeper is named, and "keeps the lighthouse" is the role.' $$;

-- What is still wanted: of a thing a feature draws, its missing part only; a thing no feature draws (none),
-- or an answer without missing, as before (0088).
ALTER FUNCTION verse.builder_wanted(jsonb, jsonb) RENAME TO builder_wanted_0091;
CREATE FUNCTION verse.builder_wanted(prev jsonb, asked jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_wanted_0091(prev, coalesce((
    SELECT jsonb_agg(CASE WHEN a->>'feature' IS DISTINCT FROM 'none' AND jsonb_typeof(a->'missing') = 'string'
                          THEN a || jsonb_build_object('thing', a->'missing', 'same', verse.builder_line(a->'missing', 60) = '')
                          ELSE a END ORDER BY o)
    FROM jsonb_array_elements(CASE WHEN jsonb_typeof(asked) = 'array' THEN asked ELSE '[]' END) WITH ORDINALITY y(a, o)), asked)) $$;
