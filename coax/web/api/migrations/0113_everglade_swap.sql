-- The swap (#12710, epic #12718): /worlds/everglade plays the SRD 5e module (everglade5e.coax),
-- under its own slug, rights, world, forks and saves. /worlds/everglade-5e answers 301 to it at the
-- load balancer (coax/web/api/lb.sh). The old module stays, unlisted, as Everglade Classic at
-- /worlds/everglade-classic, its own realm (the owner, 2026-11-03: "unlisted"):
--   - everglade-5e's edits since it split from Everglade (0112) come onto Everglade (the owner,
--     2026-11-03: "copying sure"): when its world was published again after the copy (a version
--     over 1), Everglade's world becomes it, as a publish, logged;
--   - Everglade Classic gets Everglade's rights, a fork base with its rule sets, its world as
--     Everglade's is published now (after the copy), its voice catalogue, and a reserved slug.
-- Additive, and safe to run twice.

DO $$
DECLARE five verse.published; ver int;
BEGIN
  SELECT * INTO five FROM verse.published WHERE world = 'everglade-5e';
  IF five.world IS NOT NULL AND five.version > 1
     AND NOT EXISTS (SELECT 1 FROM verse.log WHERE world = 'everglade' AND summary = 'everglade-5e copied on (#12710)') THEN
    PERFORM pg_advisory_xact_lock(7731004, hashtext('everglade'));
    SELECT coalesce(max(version), 0) + 1 INTO ver FROM verse.published WHERE world = 'everglade';
    INSERT INTO verse.published (world, doc, version, published_by, published_at)
    VALUES ('everglade', five.doc, ver, 'copied from everglade-5e', extract(epoch FROM clock_timestamp()))
    ON CONFLICT (world) DO UPDATE SET doc = EXCLUDED.doc, version = EXCLUDED.version, published_by = EXCLUDED.published_by, published_at = EXCLUDED.published_at;
    INSERT INTO verse.log (world, version, who, summary, at, state, doc)
    VALUES ('everglade', ver, 'copied from everglade-5e', 'everglade-5e copied on (#12710)', extract(epoch FROM clock_timestamp()), 'published', five.doc);
  END IF;
END $$;

INSERT INTO verse.realm_rights (slug, owner_account, admins, tools, things, rows)
SELECT 'everglade-classic', owner_account, admins, tools, things, rows FROM verse.realm_rights WHERE slug = 'everglade'
ON CONFLICT (slug) DO NOTHING;

INSERT INTO verse.fork_bases (slug, title, style, ground, rules)
SELECT 'everglade-classic', 'Everglade Classic', style, ground, rules FROM verse.fork_bases WHERE slug = 'everglade'
ON CONFLICT (slug) DO NOTHING;

INSERT INTO verse.published (world, doc, version, published_by, published_at)
SELECT 'everglade-classic', doc, 1, 'copied from everglade', extract(epoch FROM clock_timestamp()) FROM verse.published WHERE world = 'everglade'
ON CONFLICT (world) DO NOTHING;

-- Everglade Classic and Everglade 5E speak with Everglade's catalogue; a fork with its base's.
CREATE OR REPLACE FUNCTION verse.voice_edit_catalogue(realm text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT verse.voice_edit_catalogue_0051(CASE WHEN b IN ('everglade-5e', 'everglade-classic') THEN 'everglade' ELSE b END)
  FROM (SELECT coalesce((SELECT base FROM verse.made_realms WHERE slug = realm AND base IS NOT NULL), realm) AS b) x $$;

CREATE OR REPLACE FUNCTION verse.builder_reserved() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT ARRAY['new', 'built', 'drop', 'fixture', 'sandbox', 'garden', 'characters', 'haunted-house', 'the-grid', 'everglade',
               'fairy-forest', 'dead-gears', 'ice-cream-man', 'hecates-cemetery', 'assets', 'made', 'mine', 'index', 'realm', 'plan', 'pick',
               'listed', 'everglade-5e', 'everglade-classic', '5e-lab', '5e-mmo', 'one-shot'] $$;
