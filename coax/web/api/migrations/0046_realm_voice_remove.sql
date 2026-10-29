-- Voice edits remove groups and the city's people (eg-voice-edit, after
-- 0045_realm_voice_edit.sql). An admin walked up to four crates, said
-- "remove these crates" and got told what the selector can do: remove had
-- no way to name what or how many. Now remove takes, all optional:
--   item  what kind: a placed thing (crate ... stall), a person or a cat
--   who   one of the city's people or cats by name (verse/everglade/people.coax EG_PERSON)
--   all   true for many ("these crates", "those benches", "all the barrels
--         here", "clear this"): every one near, and the whole row the
--         nearest was placed in
-- The page (verse/everglade/edits.coax eg_remove_pick) does the taking.
-- The prompt carries examples, and an unclear sentence gets the best typed
-- choice; "none" only for talk that is no edit at all.
--
--   -> 200 {edit, item?, count?, who?, all?, model}

CREATE OR REPLACE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT CASE realm
  WHEN 'everglade' THEN '{
    "place": {"says": "put one or more things on the ground in front of the speaker", "item": ["crate", "barrel", "bench", "fence", "stall"], "needs": "item", "count": [1, 6, 1],
              "examples": ["put two barrels here => place, item barrel, count 2", "add a bench => place, item bench", "a row of fences => place, item fence, count 4"]},
    "remove": {"says": "take away placed things, or people or cats of the city, near the speaker or named: one (the nearest, \"that\", \"this crate\"), or many with all true (\"these\", \"those\", \"the crates\", plurals, \"all the\", \"clear this\")",
               "item": ["crate", "barrel", "bench", "fence", "stall", "person", "cat"],
               "who": ["Bram", "Tilda", "Pip", "Oswin", "Pebble", "Marta", "Soot", "Hollis", "Fern", "Ada", "Quill", "Garrick", "Isolde", "Rook", "Sable", "Elowen", "Wisp", "Brother Anselm", "Vega", "Moss", "Old Tam", "Corin"],
               "all": true,
               "examples": ["remove that => remove", "remove these crates => remove, item crate, all true", "remove the crates => remove, item crate, all true",
                            "get rid of those benches => remove, item bench, all true", "remove all the barrels here => remove, item barrel, all true",
                            "clear this => remove, all true", "take this crate away => remove, item crate", "remove Marta => remove, who Marta",
                            "remove these cats => remove, item cat, all true", "get rid of that guard => remove, item person"]},
    "undo": {"says": "take back the last change", "examples": ["undo that => undo", "put it back => undo"]}
  }'::jsonb END $$;

CREATE OR REPLACE FUNCTION verse.voice_edit_prompt(realm text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You turn what an admin said out loud into one edit of ' || initcap(realm) || ', a realm in the Verse (a small 3D world). '
    || 'They stand in the realm and speak one sentence to change it. Choose the one edit from this list that does what they asked, '
    || 'and its fields from what they said (which thing, how many, whom). Leave out a field the edit does not have, or that they did not say. '
    || 'Speech recognition may mishear a word; pick what they most likely meant. When the sentence is unclear, still pick the edit it most likely means. '
    || E'Answer edit "none" only when it is not an edit at all (a question, small talk).\nThe edits:\n'
    || string_agg('- ' || n || ': ' || (c->>'says')
         || CASE WHEN c ? 'item' THEN '. item: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'item') k) ELSE '' END
         || CASE WHEN c ? 'count' THEN '. count ' || (c->'count'->>0) || ' to ' || (c->'count'->>1) || ' (usually ' || (c->'count'->>2) || ')' ELSE '' END
         || CASE WHEN c ? 'who' THEN '. who: ' || (SELECT string_agg(k, ', ') FROM jsonb_array_elements_text(c->'who') k) ELSE '' END
         || CASE WHEN c ? 'all' THEN '. all: true for more than one' ELSE '' END
         || CASE WHEN c ? 'examples' THEN E'\n  for example: ' || (SELECT string_agg(k, '; ') FROM jsonb_array_elements_text(c->'examples') k) ELSE '' END, E'\n' ORDER BY o)
  FROM unnest(verse.voice_edit_order()) WITH ORDINALITY AS t(n, o), LATERAL (SELECT verse.voice_edit_catalogue(realm)->n AS c) x
  WHERE c IS NOT NULL $$;

-- Every edit's items in one enum (remove's: place's and the people), the people's names, and all.
CREATE OR REPLACE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('edit'), 'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'enum', coalesce(verse.voice_edit_catalogue(realm)->'remove'->'item', verse.voice_edit_catalogue(realm)->'place'->'item')),
    'count', jsonb_build_object('type', 'integer'),
    'who', jsonb_build_object('type', 'string', 'enum', verse.voice_edit_catalogue(realm)->'remove'->'who'),
    'all', jsonb_build_object('type', 'boolean'))) $$;

-- An answer held to the realm's catalogue: the edit with only its own fields, or NULL.
CREATE OR REPLACE FUNCTION verse.voice_edit_hold(realm text, d jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  name text := CASE WHEN jsonb_typeof(d) = 'object' AND jsonb_typeof(d->'edit') = 'string' THEN d->>'edit' END;
  c jsonb := verse.voice_edit_catalogue(realm)->name;
  out jsonb;
  n numeric;
BEGIN
  IF c IS NULL OR name IS NULL THEN RETURN NULL; END IF;
  out := jsonb_build_object('edit', name);
  IF c ? 'item' THEN
    IF c->'item' ? coalesce(d->>'item', '') THEN out := out || jsonb_build_object('item', d->>'item');
    ELSIF c->>'needs' = 'item' THEN RETURN NULL;   -- a place names what it places
    END IF;
  END IF;
  IF c ? 'count' THEN
    n := CASE WHEN jsonb_typeof(d->'count') = 'number' THEN round((d->>'count')::numeric) END;
    out := out || jsonb_build_object('count', CASE WHEN n IS NULL OR n <= 0 THEN (c->'count'->>2)::int
                                                   ELSE greatest((c->'count'->>0)::int, least((c->'count'->>1)::int, n))::int END);
  END IF;
  IF c ? 'who' AND c->'who' ? coalesce(d->>'who', '') THEN out := out || jsonb_build_object('who', d->>'who'); END IF;
  IF c ? 'all' AND jsonb_typeof(d->'all') = 'boolean' AND (d->>'all')::boolean THEN out := out || '{"all": true}'::jsonb; END IF;
  RETURN out;
END $$;
