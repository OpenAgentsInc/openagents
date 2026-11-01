-- /realms lists every public realm (apps/openagents-web/realmlist.coax): the built-in realms
-- from its own table, then the public made realms from GET /api/playtest/realms/listed.
--
--   total N                               how many public made realms there are
--   SLUG|TITLE|LINE|YYYY-MM-DDTHH:MM|STYLE  one a line, newest first, at most n (the made time in Chicago)
--
-- The columns are /realms' own table's (realms/list.coax). Only visibility 'public' (0099) is
-- here, and a built-in realm's fork only once published, as on /og's rows: private and unlisted
-- realms never are. Names and lines have no '|' or control characters (verse.builder_line).
--
-- Additive: one new function; builder_reserved gains "listed".
CREATE FUNCTION verse.realm_list_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT 'total ' || (SELECT count(*) FROM verse.made_realms
                      WHERE visibility = 'public' AND (base IS NULL OR published_ms IS NOT NULL)) || E'\n'
      || coalesce(string_agg(m.slug || '|' || verse.builder_line(to_jsonb(m.name), 60)
                             || '|' || verse.builder_line(to_jsonb(verse.builder_share_line(m)), 120)
                             || '|' || to_char(to_timestamp(m.made_ms / 1000.0) AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD"T"HH24:MI')
                             || '|' || verse.builder_line(to_jsonb(m.style), 20), E'\n' ORDER BY m.made_ms DESC, m.slug) || E'\n', '')
  FROM (SELECT * FROM verse.made_realms WHERE visibility = 'public' AND (base IS NULL OR published_ms IS NOT NULL)
        ORDER BY made_ms DESC, slug LIMIT greatest(least(n, 200), 1)) m $$;

-- "listed" is a route, so never a made realm's address (0049's list and it).
CREATE OR REPLACE FUNCTION verse.builder_reserved() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['new', 'built', 'drop', 'fixture', 'sandbox', 'garden', 'characters', 'haunted-house', 'the-grid', 'everglade',
               'fairy-forest', 'dead-gears', 'ice-cream-man', 'hecates-cemetery', 'assets', 'made', 'mine', 'index', 'realm', 'plan', 'pick',
               'listed'] $$;
