-- WorldBuilder's answer lines, again (0141_worldbuilder.sql; #13324):
--  - the line to show keeps its commas: only line breaks leave it (the page reads the whole "S" line);
--  - a ground edit's layer comes in its own field, `layer` (one of seven, by the schema in
--    coax/web/api/worldbuilder.c), and goes in the asset's place as the page reads it
--    (apps/openagents-web/worldbuilder-plan.coax wb_read); an answer that put the layer in `asset`
--    still works.
CREATE OR REPLACE FUNCTION worldbuilder.say(t text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(regexp_replace(coalesce(t, ''), '[\r\n|]', ' ', 'g'), 200) $$;
CREATE OR REPLACE FUNCTION worldbuilder.lines(answer text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE j jsonb; a jsonb; out text; asset text;
BEGIN
  BEGIN j := answer::jsonb; EXCEPTION WHEN others THEN RETURN NULL; END;
  IF jsonb_typeof(j) <> 'object' OR jsonb_typeof(j->'actions') IS DISTINCT FROM 'array' THEN RETURN NULL; END IF;
  out := 'S ' || worldbuilder.say(j->>'say') || E'\n';
  FOR a IN SELECT e FROM jsonb_array_elements(j->'actions') e LIMIT 24 LOOP
    IF jsonb_typeof(a) <> 'object' THEN CONTINUE; END IF;
    asset := worldbuilder.clean(a->>'asset');
    IF coalesce(a->>'op', '') IN ('paint', 'raise', 'lower', 'smooth', 'flatten') AND coalesce(a->>'layer', '') <> '' THEN
      asset := worldbuilder.clean(a->>'layer');
    END IF;
    out := out || 'A ' || worldbuilder.clean(a->>'op') || '|' || asset || '|' || worldbuilder.num(a, 'count')
      || '|' || worldbuilder.clean(a->>'targets') || '|' || worldbuilder.list(a, 'names') || '|' || worldbuilder.clean(a->>'anchor')
      || '|' || worldbuilder.list(a, 'anchor_names') || '|' || worldbuilder.clean(a->>'side') || '|' || worldbuilder.num(a, 'distance')
      || '|' || worldbuilder.num(a, 'spread') || '|' || worldbuilder.num(a, 'size') || '|' || worldbuilder.num(a, 'factor')
      || '|' || worldbuilder.num(a, 'degrees') || E'\n';
  END LOOP;
  RETURN out;
END $$;
