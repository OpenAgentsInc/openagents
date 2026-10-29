-- The real model fills every field it is given (edit_live.sh against
-- 0046: "remove these crates" came back with who "Marta", no all, and "put
-- two barrels here" with count 1). So each field says what it is for, all
-- and who are always answered (false, and "nobody" when no name was said),
-- and a name is kept only for one person: never with a placed thing's kind,
-- never with many.

-- Every edit's items in one enum (remove's: place's and the people), the people's names, and all.
-- The model fills every field it is given, so each says what it is for, all
-- and who are always answered (false, "nobody" when not said) and come after
-- edit and item: a name it wasn't told can't slip into "remove these crates".
CREATE OR REPLACE FUNCTION verse.voice_edit_schema(realm text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('type', 'object', 'required', jsonb_build_array('edit', 'all', 'who'),
    'propertyOrdering', jsonb_build_array('edit', 'item', 'count', 'all', 'who'), 'properties', jsonb_build_object(
    'edit', jsonb_build_object('type', 'string', 'enum', to_jsonb(verse.voice_edit_order() || ARRAY['none'])),
    'item', jsonb_build_object('type', 'string', 'description', 'the kind of thing they named, if they named one',
                               'enum', coalesce(verse.voice_edit_catalogue(realm)->'remove'->'item', verse.voice_edit_catalogue(realm)->'place'->'item')),
    'count', jsonb_build_object('type', 'integer', 'description', 'for place: the number they said ("two" is 2, "a" is 1)'),
    'all', jsonb_build_object('type', 'boolean', 'description', 'for remove: true when they mean more than one (these, those, a plural, all, clear), else false'),
    'who', jsonb_build_object('type', 'string', 'description', 'for remove: the person or cat they said by name; nobody when they said no name',
                              'enum', to_jsonb(ARRAY['nobody']) || (verse.voice_edit_catalogue(realm)->'remove'->'who')))) $$;

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
  IF c ? 'all' AND jsonb_typeof(d->'all') = 'boolean' AND (d->>'all')::boolean THEN out := out || '{"all": true}'::jsonb; END IF;
  -- a name only for one person: not with a placed thing's kind, nor with many
  IF c ? 'who' AND c->'who' ? coalesce(d->>'who', '') AND NOT out ? 'all'
     AND coalesce(out->>'item', 'person') IN ('person', 'cat') THEN out := out || jsonb_build_object('who', d->>'who'); END IF;
  RETURN out;
END $$;
