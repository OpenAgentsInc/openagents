-- Everglade's voice edits name every townsperson the page has (verse/everglade/people.coax
-- EG_PERSON): wave 1 (646995aa7c) added six for the new districts, and the catalogue (0051) still
-- listed the first 22, so "move Lark" or "Mistress Wren to the inn" could not be said.
-- Everglade Classic, Everglade 5E and forks of them share Everglade's catalogue (0113).
CREATE OR REPLACE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN m = 'everglade' THEN jsonb_set(c, '{who}', (c->'who') || '["Warden Hale", "Lark", "Mistress Wren", "Dunstan", "Tamsin", "Old Haddon"]'::jsonb)
              ELSE c END
  FROM (SELECT m, verse.voice_edit_catalogue_0051(m) AS c
        FROM (SELECT CASE WHEN b IN ('everglade-5e', 'everglade-classic') THEN 'everglade' ELSE b END AS m
              FROM (SELECT coalesce((SELECT base FROM verse.made_realms WHERE slug = realm AND base IS NOT NULL), realm) AS b) x) y) z $$;
