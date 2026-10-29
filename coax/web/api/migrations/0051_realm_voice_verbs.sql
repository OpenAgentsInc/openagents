-- Voice edits v2 (eg-voice-edit, after 0045..0047; 0051 so it follows 0050_realm_edit): more things, more
-- verbs, and the fields a sentence carries. The production log showed what
-- the old catalogue could not say: "put pumpkins on the stall" came back a
-- stall, "put a building in front of me" a stall too (it never substitutes
-- now), "what else can I put down?" none (now list), "this barrel is
-- bigger" none (resize), "movies crates" none (a mishearing of "move these
-- crates": move), "place 12 benches all around" none (place, ring),
-- "delete this temple building" none (remove the city's building), "four big
-- crates" lost its size.
--
-- The edits (the page's contract; verse/everglade/edits.coax does them):
--   place   {edit, item, count, arrange?, onto?, colour?, size?}
--   remove  {edit, item?, who?, all?, building?}
--   move    {edit, item?, all?, direction, metres?}
--   resize  {edit, item?, all?, size}
--   rotate  {edit, item?, all?, turn}
--   restore {edit, building?, who?}
--   list    {edit}
--   undo    {edit}
--   missing {edit, name}   made here, never by the model: what they asked
--           for is not in the lists (the model's "name" says what it was)
--
-- As in 0047 the model fills every field it is given, so every field is
-- required, says what it is for, and has a "not said" value (item
-- something, name "", arrange front, onto ground, colour unsaid, size 1,
-- direction here, metres 0, turn 0, all false, who nobody, building
-- unnamed). voice_edit_hold keeps only each edit's own fields, drops the
-- "not said" values and clamps the numbers. No words of the sentence are
-- matched anywhere: the model reads it, only its typed answer is checked.

CREATE OR REPLACE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT CASE realm
  WHEN 'everglade' THEN '{
    "items": ["crate", "barrel", "bench", "fence", "stall", "pumpkin", "sack", "apple crate", "carrots", "pot", "bucket", "lantern", "cart", "vase", "table", "chair", "stool", "anvil", "cauldron", "training dummy", "weapon rack", "banner", "coins", "sign", "bush", "flowers", "rock", "rock pile", "hay pile", "stone wall", "crops", "house", "shed", "tower"],
    "sizes": ["0.8x0.9x0.9", "0.7x0.7x0.9", "2.8x0.5x0.5", "2.1x0.1x0.8", "2.9x1.5x2.6 (a market stall)", "0.6x0.6x0.5", "0.7x0.5x0.8", "0.7x0.4x0.2", "0.4x0.4x0.6", "0.5x0.5x0.2", "0.4x0.4x0.3", "0.4x0.4x2.4 (on a post)", "3.0x1.1x2.6", "0.7x0.7x0.5", "2.8x1.1x0.8", "0.6x0.5x1.1", "0.5x0.5x0.6", "1.1x0.4x0.6", "1.0x0.9x0.8", "0.8x0.6x1.9", "1.4x1.0x1.1", "1.6x0.2x2.4", "0.4x0.3x0.1", "1.2x0.2x1.8", "1.6x1.6x1.2", "a patch 6 m across", "1.8x1.8x0.8", "2.5x2.5x1.2", "2.0x2.0x1.2", "4.0x0.6x1.2", "rows, 4x4 m", "6x5x6 (a cottage: any building they ask for)", "3x3x3", "4x4x10"],
    "kinds": ["person", "cat", "building"],
    "onto": ["stall", "table", "crate", "barrel", "bench", "cart", "stool"],
    "colours": ["purple", "blue", "red", "pink", "yellow", "white", "orange"],
    "arrange": ["front", "row", "ring", "scatter", "fill"],
    "directions": ["here", "forward", "back", "left", "right", "away", "closer"],
    "buildings": ["inn", "general store", "smithy", "alchemist", "fighters'' hall", "rangers'' lodge", "mages'' tower", "temple", "observatory", "stables", "house"],
    "who": ["Bram", "Tilda", "Pip", "Oswin", "Pebble", "Marta", "Soot", "Hollis", "Fern", "Ada", "Quill", "Garrick", "Isolde", "Rook", "Sable", "Elowen", "Wisp", "Brother Anselm", "Vega", "Moss", "Old Tam", "Corin"],
    "edits": {
      "place": {"says": "put things down: count how many (1 to 40; a plural with no number is 3), arrange how (front: a row in front of the speaker, the usual; row; ring: all around, around me, in a circle; scatter: spread about; fill: cover the open ground in front, \"fill the field with\"), onto one of the things with a top when they said ON it, colour when they said one, size when they said big (1.5), huge (2.5), small (0.6) or tiny (0.4)",
                "examples": ["put two barrels here => place, item barrel, count 2", "add a bench => place, item bench, count 1",
                             "put pumpkins on the stall => place, item pumpkin, count 3, onto stall", "put six pumpkins on the stall => place, item pumpkin, count 6, onto stall",
                             "put a building in front of me => place, item house (any building is a house, never a stall)",
                             "place 12 benches all around => place, item bench, count 12, arrange ring", "place four big crates in front => place, item crate, count 4, size 1.5",
                             "fill the field with purple flowers => place, item flowers, colour purple, arrange fill", "a row of fences => place, item fence, count 4, arrange row",
                             "scatter some rocks about => place, item rock, count 3, arrange scatter",
                             "place a dragon => place, item something, name dragon (a dragon is not in the list: never put another thing instead)"]},
      "remove": {"says": "take away placed things, or the city''s people, cats or buildings, near the speaker or named: one (the nearest, \"that\", \"this crate\"), or many with all true (\"these\", \"those\", \"the crates\", plurals, \"all the\", \"clear this\")",
                 "examples": ["remove that => remove", "remove these crates => remove, item crate, all true", "get rid of those benches => remove, item bench, all true",
                              "remove all the barrels here => remove, item barrel, all true", "clear this => remove, all true", "take this crate away => remove, item crate",
                              "remove Marta => remove, item person, who Marta", "remove these cats => remove, item cat, all true", "get rid of that guard => remove, item person",
                              "delete this temple building => remove, item building, building temple", "remove the inn => remove, item building, building inn",
                              "remove that house => remove, item house", "remove Bob => remove, item person, name Bob (no one in the city is called Bob)"]},
      "move": {"says": "move placed things near the speaker (one, or many with all true) in a direction: here (to in front of the speaker, when no direction is said), forward, back, left, right, away, closer; metres when they said how far",
               "examples": ["move these crates => move, item crate, all true, direction here", "movies crates (misheard: move these crates) => move, item crate, all true, direction here",
                            "move it left two metres => move, direction left, metres 2", "push that barrel back => move, item barrel, direction back",
                            "bring the benches closer => move, item bench, all true, direction closer"]},
      "resize": {"says": "make placed things near the speaker bigger or smaller: size is the factor (bigger 1.5, smaller 0.67, twice as big 2, half 0.5)",
                 "examples": ["this barrel is bigger => resize, item barrel, size 1.5 (they want it bigger)", "make these benches smaller => resize, item bench, all true, size 0.67",
                              "make that twice as big => resize, size 2", "the stall should be half the size => resize, item stall, size 0.5"]},
      "rotate": {"says": "turn placed things near the speaker: turn in degrees, positive to the speaker''s right (turn it around: 180; a turn with no amount: 90)",
                 "examples": ["turn it around => rotate, turn 180", "turn that bench to the left => rotate, item bench, turn -90", "rotate these fences a bit right => rotate, item fence, all true, turn 30"]},
      "restore": {"says": "bring back a building of the city that was hidden, or a person or cat that was removed",
                  "examples": ["bring back the temple => restore, building temple", "bring Marta back => restore, who Marta"]},
      "list": {"says": "they ask what they can put down or what can be made",
               "examples": ["what else can I put down? => list", "what can you make? => list", "what can I place? => list"]},
      "undo": {"says": "take back the last change", "examples": ["undo that => undo", "put it back the way it was => undo"]}
    }
  }'::jsonb END $$;
CREATE OR REPLACE FUNCTION verse.voice_edit_order() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['place', 'remove', 'move', 'resize', 'rotate', 'restore', 'list', 'undo'] $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit below that does what they asked, '
    || 'and fill its fields from what they said; every field the edit does not use gets its "not said" value. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the edit it most likely means. '
    || 'Answer edit "none" only when it is not an edit at all (small talk, or a question other than what can be put down). '
    || 'Never answer a thing they did not ask for: when what they named is not in these lists, the item is something and name is what they said. '
    || E'\nThe things that can be put down (width x depth x height in metres at size 1):\n'
    || (SELECT string_agg(i || ' ' || (c.c->'sizes'->>(o - 1)::int), '; ' ORDER BY o) FROM jsonb_array_elements_text(c.c->'items') WITH ORDINALITY AS t(i, o))
    || E'\nThings with a top that others can go onto: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'onto') k)
    || E'\nColours: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'colours') k)
    || E'\nThe city''s buildings: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'buildings') k)
    || E'\nThe city''s people and cats: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c.c->'who') k)
    || E'\nThe edits:\n'
    || (SELECT string_agg('- ' || n || ': ' || (e->>'says')
         || E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(e->'examples') k), E'\n' ORDER BY o)
        FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT c.c->'edits'->n AS e) x WHERE e IS NOT NULL)
    || E'\n- none: not an edit. for example: what time is it => none; is this working? => none; hello => none'
  FROM c WHERE c.c IS NOT NULL $$;

-- Every field required, edit and item first, name next: what they named decides the rest.
CREATE OR REPLACE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  WITH c AS (SELECT verse.voice_edit_catalogue(realm) AS c)
  SELECT jsonb_build_object('type', 'object',
    'required', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building"]'::jsonb,
    'propertyOrdering', '["edit", "item", "name", "count", "arrange", "onto", "colour", "size", "direction", "metres", "turn", "all", "who", "building"]'::jsonb,
    'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'description', 'the kind of thing they named (a person, cat or building of the city by its kind); something when they named no kind ("that", "it", "this") or a thing not in the list',
                               'enum', (c.c->'items') || (c.c->'kinds') || '["something"]'::jsonb),
    'name', jsonb_build_object('type', 'string', 'description', 'only when they named a thing, a person or a building that is NOT in the lists: what they said, as said ("dragon", "Bob"); else ""'),
    'count', jsonb_build_object('type', 'integer', 'description', 'for place: how many (1 to 40; "two" is 2, "a" is 1, a plural with no number is 3); else 1'),
    'arrange', jsonb_build_object('type', 'string', 'description', 'for place: front (the usual), row, ring (all around, around me, in a circle), scatter (spread about), fill (fill the field with)',
                                  'enum', c.c->'arrange'),
    'onto', jsonb_build_object('type', 'string', 'description', 'for place: the thing they said to put it ON; ground when they did not say on',
                               'enum', '["ground"]'::jsonb || (c.c->'onto')),
    'colour', jsonb_build_object('type', 'string', 'description', 'for place: the colour they said; unsaid when none', 'enum', (c.c->'colours') || '["unsaid"]'::jsonb),
    'size', jsonb_build_object('type', 'number', 'description', 'for place: big 1.5, huge 2.5, small 0.6, tiny 0.4, else 1; for resize: the factor (bigger 1.5, smaller 0.67, twice 2, half 0.5)'),
    'direction', jsonb_build_object('type', 'string', 'description', 'for move: which way; here (to in front of the speaker) when not said', 'enum', c.c->'directions'),
    'metres', jsonb_build_object('type', 'number', 'description', 'for move: how far, when they said; else 0'),
    'turn', jsonb_build_object('type', 'number', 'description', 'for rotate: degrees, positive to the right, around is 180; else 0'),
    'all', jsonb_build_object('type', 'boolean', 'description', 'for remove, move, resize, rotate: true when they mean more than one (these, those, a plural, all, clear), else false'),
    'who', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the person or cat of the city they said by name; nobody when they said no such name',
                              'enum', '["nobody"]'::jsonb || (c.c->'who')),
    'building', jsonb_build_object('type', 'string', 'description', 'for remove or restore: the city''s building they named; unnamed when none',
                                   'enum', '["unnamed"]'::jsonb || (c.c->'buildings'))))
  FROM c WHERE c.c IS NOT NULL $$;

-- An answer held to the realm's catalogue: the edit with only its own fields, or NULL.
CREATE OR REPLACE FUNCTION verse.voice_edit_hold(realm text, d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  c jsonb := verse.voice_edit_catalogue(realm);
  verb text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'edit') = 'string' THEN d->>'edit' END;
  said text := CASE WHEN jsonb_typeof(d->'name') = 'string' THEN left(btrim(d->>'name'), 60) ELSE '' END;
  item text := CASE WHEN jsonb_typeof(d->'item') = 'string' THEN d->>'item' END;
  placed bool;
  out jsonb;
  x numeric;
BEGIN
  IF c IS NULL OR verb IS NULL OR NOT verb = ANY (verse.voice_edit_order()) THEN RETURN NULL; END IF;
  placed := coalesce(c->'items' ? item, false);
  out := jsonb_build_object('edit', verb);
  IF verb IN ('list', 'undo') THEN RETURN out; END IF;

  IF verb = 'place' THEN
    IF NOT placed THEN   -- never another thing instead: what they named is missing, or it's no edit
      RETURN CASE WHEN said <> '' THEN jsonb_build_object('edit', 'missing', 'name', said) END;
    END IF;
    x := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('item', item, 'count', CASE WHEN x IS NULL OR x < 1 THEN 1 ELSE least(x, 40)::int END);
    IF c->'arrange' ? coalesce(d->>'arrange', '') AND d->>'arrange' <> 'front' THEN out := out || jsonb_build_object('arrange', d->>'arrange'); END IF;
    IF c->'onto' ? coalesce(d->>'onto', '') THEN out := out || jsonb_build_object('onto', d->>'onto'); END IF;
    IF c->'colours' ? coalesce(d->>'colour', '') THEN out := out || jsonb_build_object('colour', d->>'colour'); END IF;
    x := CASE WHEN jsonb_typeof(d->'size') = 'number' THEN round((d->>'size')::numeric, 2) END;
    IF x > 0 AND x <> 1 THEN out := out || jsonb_build_object('size', trim_scale(greatest(0.25, least(4, x)))); END IF;
    RETURN out;
  END IF;

  IF verb = 'restore' THEN
    IF c->'buildings' ? coalesce(d->>'building', '') THEN out := out || jsonb_build_object('building', d->>'building');
    ELSIF c->'who' ? coalesce(d->>'who', '') THEN out := out || jsonb_build_object('who', d->>'who');
    ELSIF said <> '' THEN RETURN jsonb_build_object('edit', 'missing', 'name', said);
    END IF;
    RETURN out;
  END IF;

  -- remove, move, resize, rotate: what (a placed kind; remove also the city's people, cats and buildings), and how many
  IF placed OR (verb = 'remove' AND c->'kinds' ? coalesce(item, '')) THEN out := out || jsonb_build_object('item', item); END IF;
  IF jsonb_typeof(d->'all') = 'boolean' AND (d->>'all')::boolean THEN out := out || '{"all": true}'::jsonb; END IF;
  IF verb = 'remove' THEN
    -- a name only for one person: not with a placed thing's kind, nor with many
    IF c->'who' ? coalesce(d->>'who', '') AND NOT out ? 'all' AND coalesce(out->>'item', 'person') IN ('person', 'cat') THEN
      out := out || jsonb_build_object('who', d->>'who');
    END IF;
    -- a building of the city only as the building it is
    IF c->'buildings' ? coalesce(d->>'building', '') AND coalesce(out->>'item', 'building') = 'building' AND NOT out ? 'who' THEN
      out := out || jsonb_build_object('item', 'building', 'building', d->>'building');
    END IF;
  END IF;
  -- they named someone or something not here, and nothing that is
  IF said <> '' AND NOT placed AND NOT out ? 'who' AND NOT out ? 'building' THEN
    RETURN jsonb_build_object('edit', 'missing', 'name', said);
  END IF;
  IF verb = 'move' THEN
    out := out || jsonb_build_object('direction', CASE WHEN c->'directions' ? coalesce(d->>'direction', '') THEN d->>'direction' ELSE 'here' END);
    x := CASE WHEN jsonb_typeof(d->'metres') = 'number' THEN round((d->>'metres')::numeric, 1) END;
    IF x > 0 THEN out := out || jsonb_build_object('metres', trim_scale(greatest(0.5, least(30, x)))); END IF;
  ELSIF verb = 'resize' THEN
    x := CASE WHEN jsonb_typeof(d->'size') = 'number' THEN round((d->>'size')::numeric, 2) END;
    out := out || jsonb_build_object('size', CASE WHEN x IS NULL OR x <= 0 OR x = 1 THEN 1.5 ELSE trim_scale(greatest(0.25, least(4, x))) END);
  ELSIF verb = 'rotate' THEN
    x := CASE WHEN jsonb_typeof(d->'turn') = 'number' THEN round((d->>'turn')::numeric) END;
    out := out || jsonb_build_object('turn', CASE WHEN x IS NULL OR x = 0 THEN 90 ELSE greatest(-360, least(360, x))::int END);
  END IF;
  RETURN out;
END $$;
