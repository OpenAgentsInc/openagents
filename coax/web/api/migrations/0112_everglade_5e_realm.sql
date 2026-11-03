-- Everglade 5E (/worlds/everglade-5e, unlisted) is its own realm: Everglade's code on the SRD 5e
-- rules (apps/openagents-web/everglade5e.coax), until now edited, saved and forked as Everglade
-- itself. Its EDIT (Aethyrforge's panels, everglade-forge.coax) now edits everglade-5e:
--   - its rights: Everglade's owner account, admins, tools, things and rows, as realms.json says
--     (test_realm_rights.py checks the two agree);
--   - FORK: a fork base with Everglade's rule sets, which its code plays as Everglade's does;
--   - its world starts as Everglade's is published now, so the city looks the same the day it
--     splits off; from here its edits are its own and Everglade never changes with them;
--   - talking edits it with Everglade's catalogue (place, move, ...), the same code;
--   - its slug is never a made realm's.
-- Additive: rows where none are, and two functions replaced with supersets.

INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows)
SELECT 'everglade-5e', owner_account, admins, tools, things, rows FROM verse.realm_rights WHERE slug = 'everglade'
ON CONFLICT (slug) DO NOTHING;

INSERT INTO verse.fork_bases (slug, title, style, ground, rules)
SELECT 'everglade-5e', 'Everglade 5E', style, ground, rules FROM verse.fork_bases WHERE slug = 'everglade'
ON CONFLICT (slug) DO NOTHING;

INSERT INTO verse.published (world, doc, version, published_by, published_at)
SELECT 'everglade-5e', doc, 1, 'copied from everglade', extract(epoch FROM clock_timestamp()) FROM verse.published WHERE world = 'everglade'
ON CONFLICT (world) DO NOTHING;

-- Everglade 5E speaks with Everglade's catalogue; a fork of it with its base's (0106).
CREATE OR REPLACE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT verse.voice_edit_catalogue_0051(CASE b WHEN 'everglade-5e' THEN 'everglade' ELSE b END)
  FROM (SELECT coalesce((SELECT base FROM verse.made_realms WHERE slug = realm AND base IS NOT NULL), realm) AS b) x $$;

CREATE OR REPLACE FUNCTION verse.builder_reserved() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['new', 'built', 'drop', 'fixture', 'sandbox', 'garden', 'characters', 'haunted-house', 'the-grid', 'everglade',
               'fairy-forest', 'dead-gears', 'ice-cream-man', 'hecates-cemetery', 'assets', 'made', 'mine', 'index', 'realm', 'plan', 'pick',
               'listed', 'everglade-5e', '5e-lab', '5e-mmo', 'one-shot'] $$;
