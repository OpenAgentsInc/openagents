-- Realm builder, round 2 (byo-never-drop; docs/verse/realm-builder.md). Two things:
--
-- 1. A word about mood never makes a line still wanted when the thing is drawn: "friendly owls"
--    drew a flock of owls and also stood on the board as wanted. Each asked thing now has the
--    visitor's words as said ("said") and the thing alone without the words about mood or feeling
--    ("thing": "owls"); "same" is judged on the thing, and a wanted line is the thing.
-- 2. More of what is asked for most is drawn (eval-builder.sh and production's prompts, ranked):
--    buildings (a cabin, barn, windmill, lighthouse, castle, tower, market, shop, saloon, temple,
--    ruins), the land (hills, mountains, cliffs, a floating island, an island in the sea, a
--    volcano), water that is a river, frozen or lava, more animals (sheep, goats, cows, pigs,
--    wolves, foxes, dogs, camels, chickens, dinosaurs), more in the air (parrots, gulls, a dragon,
--    fairies, fireflies, butterflies), palm and blossom trees, flowers, mushrooms, cacti and
--    crystals, and more things standing about (a campfire, boats, a bridge, a signpost, a well,
--    treasure, gravestones, tumbleweeds, a statue). myrealm.coax draws all of them.
-- No words are matched here: the model reads them and only its typed answer is checked.

ALTER FUNCTION verse.builder_catalogue() RENAME TO builder_catalogue_0084;
CREATE FUNCTION verse.builder_catalogue() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_catalogue_0084() || '{
    "water":    {"says": "water: a pond or lake, a river running across the land, a frozen pond (ice), or a glowing pool of lava", "size": ["medium", "small", "large"], "variant": ["pond", "river", "frozen", "lava"], "most": 1},
    "land":     {"says": "the shape of the land: rolling hills round it, mountains (snow on their peaks) round it, cliffs round it, a floating island in the sky (its edge drops to open sky, small islands float beyond), an island in the sea (the sea all round), or a volcano smoking and glowing on the skyline", "variant": ["hills", "mountains", "cliffs", "floating", "island", "volcano"], "most": 1},
    "building": {"says": "one building of a kind: a log cabin, a red barn, a windmill with turning sails, a lighthouse with a lamp, a castle (walls, four towers and a gate), a stone tower, a market of stalls under awnings, a shop with an awning and a sign, a wild west saloon with a false front, a stepped temple, or ruins of broken columns", "variant": ["cabin", "barn", "windmill", "lighthouse", "castle", "tower", "market", "shop", "saloon", "temple", "ruins"], "most": 4},
    "herd":     {"says": "animals that roam and graze across the land, of one kind (two herds for two kinds): bison, deer, horses, sheep, goats, cows, pigs, wolves, foxes, dogs, camels, chickens or dinosaurs", "variant": ["bison", "deer", "horse", "sheep", "goat", "cow", "pig", "wolf", "fox", "dog", "camel", "chicken", "dinosaur"], "count": [1, 10, 6], "most": 2},
    "flock":    {"says": "what flies, circling whoever walks: crows, bats, owls, parrots, seagulls, a dragon (or a few), fairies (tiny glowing lights with wings), fireflies (glowing specks) or butterflies", "variant": ["crow", "bat", "owl", "parrot", "gull", "dragon", "fairy", "firefly", "butterfly"], "count": [1, 12, 7], "most": 1},
    "trees":    {"says": "a grove of trees: round, pine, dead, mixed, palm trees, or blossom trees (pink, like cherry blossom)", "variant": ["round", "pine", "dead", "mixed", "palm", "blossom"], "count": [3, 20, 9], "most": 1},
    "plants":   {"says": "low plants scattered about: ferns, flowers, mushrooms (glowing after dark), cacti, or crystals (glowing)", "variant": ["fern", "flower", "mushroom", "cactus", "crystal"], "count": [4, 24, 10], "most": 1},
    "props":    {"says": "things standing about: crates, barrels, a wagon, a fence, a camp (a chest and an anvil), a campfire, boats, a bridge, a signpost, a well, treasure (a chest and gold), gravestones, tumbleweeds, or a statue", "variant": ["crates", "barrels", "wagon", "fence", "camp", "campfire", "boat", "bridge", "signpost", "well", "treasure", "graves", "tumbleweeds", "statue"], "most": 8}}'::jsonb $$;
ALTER FUNCTION verse.builder_order() RENAME TO builder_order_0084;
CREATE FUNCTION verse.builder_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['land', 'building'] || verse.builder_order_0084() $$;

-- A word for many of a kind: "wolves", "foxes", "fairies", "sheep".
CREATE FUNCTION verse.builder_many(v text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE v WHEN 'sheep' THEN 'sheep' WHEN 'bison' THEN 'bison' WHEN 'deer' THEN 'deer' WHEN 'wolf' THEN 'wolves'
                WHEN 'fox' THEN 'foxes' WHEN 'fairy' THEN 'fairies' WHEN 'firefly' THEN 'fireflies' WHEN 'butterfly' THEN 'butterflies'
                WHEN 'cactus' THEN 'cacti' WHEN 'gull' THEN 'seagulls' ELSE v || 's' END $$;

ALTER FUNCTION verse.builder_label(jsonb) RENAME TO builder_label_0084;
CREATE FUNCTION verse.builder_label(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f->>'feature'
    WHEN 'land' THEN CASE f->>'variant' WHEN 'hills' THEN 'Rolling hills' WHEN 'mountains' THEN 'Mountains round it'
                                        WHEN 'cliffs' THEN 'Cliffs round it' WHEN 'floating' THEN 'A floating island in the sky'
                                        WHEN 'island' THEN 'An island in the sea' ELSE 'A volcano' END
    WHEN 'building' THEN CASE f->>'variant' WHEN 'cabin' THEN 'A log cabin' WHEN 'barn' THEN 'A red barn' WHEN 'windmill' THEN 'A windmill'
                                            WHEN 'lighthouse' THEN 'A lighthouse' WHEN 'castle' THEN 'A castle' WHEN 'tower' THEN 'A stone tower'
                                            WHEN 'market' THEN 'A market' WHEN 'shop' THEN 'A shop' WHEN 'saloon' THEN 'A saloon'
                                            WHEN 'temple' THEN 'A temple' ELSE 'Ruins' END
    WHEN 'water' THEN CASE coalesce(f->>'variant', 'pond') WHEN 'river' THEN 'A river' WHEN 'frozen' THEN initcap(f->>'size') || ' frozen pond'
                                                         WHEN 'lava' THEN initcap(f->>'size') || ' pool of lava' ELSE initcap(f->>'size') || ' pond' END
    WHEN 'herd' THEN CASE WHEN (f->>'count')::int = 1 THEN 'A ' || CASE f->>'variant' WHEN 'horse' THEN 'horse' ELSE f->>'variant' END
                          ELSE 'A herd of ' || (f->>'count') || ' ' || verse.builder_many(f->>'variant') END
    WHEN 'flock' THEN CASE WHEN (f->>'count')::int = 1 THEN 'A ' || (f->>'variant')
                           WHEN f->>'variant' IN ('crow', 'bat', 'owl', 'parrot', 'gull') THEN 'A flock of ' || (f->>'count') || ' ' || verse.builder_many(f->>'variant')
                           WHEN f->>'variant' = 'fairy' THEN (f->>'count') || ' glowing fairies'
                           ELSE (f->>'count') || ' ' || verse.builder_many(f->>'variant') END
    WHEN 'trees' THEN CASE f->>'variant' WHEN 'palm' THEN (f->>'count') || ' palm trees' WHEN 'blossom' THEN (f->>'count') || ' blossom trees'
                                         ELSE verse.builder_label_0084(f) END
    WHEN 'plants' THEN (f->>'count') || ' ' || CASE coalesce(f->>'variant', 'fern') WHEN 'fern' THEN 'ferns' WHEN 'flower' THEN 'flowers'
                                                    WHEN 'mushroom' THEN 'mushrooms, glowing after dark' WHEN 'cactus' THEN 'cacti' ELSE 'glowing crystals' END
    WHEN 'props' THEN CASE f->>'variant' WHEN 'campfire' THEN 'A campfire' WHEN 'boat' THEN 'Boats' WHEN 'bridge' THEN 'A bridge'
                                         WHEN 'signpost' THEN 'A signpost' WHEN 'well' THEN 'A well' WHEN 'treasure' THEN 'Treasure: a chest and gold'
                                         WHEN 'graves' THEN 'Gravestones' WHEN 'tumbleweeds' THEN 'Tumbleweeds' WHEN 'statue' THEN 'A statue'
                                         ELSE verse.builder_label_0084(f) END
    ELSE verse.builder_label_0084(f) END $$;
ALTER FUNCTION verse.builder_phrase(jsonb) RENAME TO builder_phrase_0084;
CREATE FUNCTION verse.builder_phrase(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' IN ('land', 'building')
                OR (f->>'feature' = 'water' AND f->>'variant' IN ('river', 'frozen', 'lava'))
                OR (f->>'feature' = 'herd' AND f->>'variant' NOT IN ('bison', 'deer', 'horse'))
                OR (f->>'feature' = 'flock' AND f->>'variant' NOT IN ('crow', 'bat', 'owl'))
                OR (f->>'feature' = 'trees' AND f->>'variant' IN ('palm', 'blossom'))
                OR (f->>'feature' = 'plants' AND f->>'variant' <> 'fern')
                OR (f->>'feature' = 'props' AND f->>'variant' NOT IN ('crates', 'barrels', 'wagon', 'fence', 'camp'))
              THEN lower(left(verse.builder_label(f), 1)) || substr(verse.builder_label(f), 2)
              ELSE verse.builder_phrase_0084(f) END $$;

-- A board held: one building of each kind.
ALTER FUNCTION verse.builder_hold(jsonb) RENAME TO builder_hold_0084;
CREATE FUNCTION verse.builder_hold(d jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT h || jsonb_build_object('features', coalesce((
    SELECT jsonb_agg(f ORDER BY o) FROM (
      SELECT f, o, row_number() OVER (PARTITION BY CASE WHEN f->>'feature' = 'building' THEN f->>'variant' ELSE o::text END ORDER BY o) AS k
      FROM jsonb_array_elements(h->'features') WITH ORDINALITY AS t(f, o)) x WHERE k = 1), '[]'))
  FROM (SELECT verse.builder_hold_0084(d) AS h) y WHERE h IS NOT NULL $$;

-- Each asked thing: the words as said, then the thing alone (no words about mood); a whole place made
-- of several features is feature "place" (wanted only when they don't make it, as any other).
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0084;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(jsonb_set(s,
      '{properties,asked,items,properties,feature,enum}',
      coalesce(s #> '{properties,asked,items,properties,feature,enum}', '[]') || '["place"]'::jsonb),
      '{properties,asked,items,properties,said}', '{"type": "string"}'::jsonb),
      '{properties,asked,items,required}', '["said", "thing", "feature", "drawn", "same"]'::jsonb),
      '{properties,asked,items,propertyOrdering}', '["said", "thing", "feature", "drawn", "same"]'::jsonb)
  FROM (SELECT verse.builder_schema_0084() AS s) x $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0084;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0084()
    || E'\nFor each asked thing, said is their words as they said them; thing is those words without the ones about mood '
    || 'or feeling (friendly, cosy, happy, spooky, creepy, peaceful, lonely, cheerful, magical, quiet: they change nothing '
    || 'that is drawn), so "friendly owls" is the thing "owls" and "a cosy cabin" is "cabin". Judge same on the thing alone. '
    || 'Use the catalogue fully: a cabin, barn, lighthouse or castle is a building of that kind, not a house; a river, a '
    || 'frozen pond or lava is that water; hills, mountains, a floating island or an island in the sea is the land; sheep, '
    || 'wolves or chickens are a herd of that kind; a dragon, fairies or fireflies are what flies. A whole place (a village, '
    || 'a farm, a town, a garden, a glade, a cove, a campsite, a valley) is feature place: made of the features that make it '
    || 'and the ground; it is same true when they are on the board, and then never a line still wanted. Judged on the thing alone: said "friendly owls", '
    || 'thing "owls", drawn "A flock of 7 owls": same true. When part of a thing is drawn and part is not, same is false and '
    || 'the thing is only the part not drawn: "a japanese village" drawn as a village is the thing "japanese buildings"; a '
    || 'castle is medieval already. A quality is part of the thing: glowing, sleeping, hidden, a colour. Set count to how '
    || 'many they said: "a dragon" is 1, "two wolves" 2; with no number say the feature''s default. Two kinds of animal are two '
    || 'herds. Same is true whenever a visitor would say the board draws it: "a snowy mountain cabin" with snow ground, '
    || 'mountains and a log cabin; "a frozen lake" as a large frozen pond; "sunset" as dusk; "a graveyard" as gravestones. '
    || 'A thing never stands whole as a line still wanted when part of it is drawn: the line is the part that is not '
    || '("a misty pine valley" with pines wants "mist"; "a haunted swamp" with dead trees and a pond wants "swamp"; '
    || '"a cyberpunk city street" wants "city street" and "cyberpunk look"), and nothing is wanted twice.' $$;

-- What is still wanted: the thing (its words without mood), when no feature makes it or it is only
-- stood in for.
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

-- A made realm's row key: a building's and a herd's is its kind too (a castle and a windmill are two rows).
CREATE OR REPLACE FUNCTION verse.edit_key(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' = 'row' THEN f->>'variant'
              ELSE (f->>'feature') || coalesce(':' || CASE WHEN f->>'feature' = 'character' THEN regexp_replace(lower(f->>'name'), '[^a-z0-9]+', '-', 'g')
                                                          WHEN f->>'feature' IN ('props', 'building', 'herd') THEN f->>'variant' END, '') END $$;
