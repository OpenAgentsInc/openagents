-- A colour, ground, time of day or look asked for is the board's setup, not a thing still wanted
-- (byo-never-drop): "only pastel colours" set the palette pastel and also stood on the board as
-- "pastel colors, NOT APPLIED YET". The planner now names such an ask with feature setup, and only
-- the ones it names none wait (0079's builder_wanted keeps feature none only).
ALTER FUNCTION verse.builder_schema() RENAME TO builder_schema_0080;
CREATE FUNCTION verse.builder_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(s, '{properties,asked,items,properties,feature,enum}',
                   coalesce(s #> '{properties,asked,items,properties,feature,enum}', '[]') || '["setup"]'::jsonb)
  FROM (SELECT verse.builder_schema_0080() AS s) x $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0080;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.builder_prompt_0080()
    || E'\nAn ask about its colours, ground, time of day, art style or camera is the setup: name it with feature setup, '
    || 'and set the palette, ground, time, style or camera it asks for.' $$;
