-- A made realm may be flat too (byo-painterly; docs/verse/art-styles.md "Flat"): the builder's styles
-- are lowpoly, painterly and flat, the setup question offers all three and the planner is told what
-- flat looks like. myrealm.coax draws flat with verse/style-preset.coax's flat garden preset.
CREATE OR REPLACE FUNCTION verse.builder_styles() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['lowpoly', 'painterly', 'flat'] $$;

ALTER FUNCTION verse.builder_questions() RENAME TO builder_questions_0061;
CREATE FUNCTION verse.builder_questions() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(verse.builder_questions_0061(), '{style,asks}',
    to_jsonb('Which art style: low poly (facets, flat colours), painterly (soft, painted) or flat (bold flat shapes, like a picture book)?'::text)) $$;

ALTER FUNCTION verse.builder_prompt() RENAME TO builder_prompt_0061;
CREATE FUNCTION verse.builder_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(verse.builder_prompt_0061(), E'\nStyles: lowpoly (facets and flat colours), painterly (soft, painted).',
    E'\nStyles: lowpoly (facets and flat colours), painterly (soft, painted), flat (bold flat shapes and few tones, like a picture book).') $$;
