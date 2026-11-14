-- A visitor's proposal is a pending world batch (#13515, after #13514): a visitor's spawns on a realm's world
-- (/api/worlds/<slug>) are kept for the owner instead of carried out, where the owner lets visitors build, and the
-- owner's ACCEPT places the thing as a world edit of their own (one step of the world's undo log). Whether one is
-- kept is coax/web/api/realm-place-policy.coax's (rpp_propose), asked by metaverse/edit/realm-world.coax; worlds.c
-- carries out what it says. SQL keeps the rows.
--
-- The world reads two more facts, as 0190 and 0193 made verse.realm_world_spec say what the realm places and that it is a
-- made realm: how many proposals wait (`proposals N`: the policy holds them to 50) and whether its owner lets visitors
-- build now (`building 0|1`), for anyone. A visitor's spec is a made realm's revision published (0094), but a proposal
-- is the owner's to decide now, as /place's was.

-- A proposal kept: a thing at (px, pz) centimetres turned pyaw whole degrees, in metres and radians as the cards read
-- them, by the player whose key asks. -> its ID (0: no player has that key).
CREATE FUNCTION verse.realm_propose(s text, pthing text, px int, pz int, pyaw int, k text) RETURNS bigint LANGUAGE sql AS $$
  WITH p AS (SELECT (verse.builder_player(k)).id AS who),
  kept AS (INSERT INTO verse.realm_proposals (slug, ms, by_player, change)
           SELECT s, verse.now_ms(), p.who, jsonb_build_object('kind', 'place', 'thing', pthing, 'x', trim_scale(round(px / 100.0, 2)),
                                                               'z', trim_scale(round(pz / 100.0, 2)), 'yaw', trim_scale(round(radians(pyaw)::numeric, 3)))
           FROM p WHERE p.who IS NOT NULL RETURNING id)
  SELECT coalesce((SELECT id FROM kept), 0) $$;

-- Proposal pid of realm s accepted, once: 1, or 0 when it is decided already or is none.
CREATE FUNCTION verse.realm_proposal_settle(s text, pid bigint) RETURNS int LANGUAGE sql AS $$
  WITH d AS (UPDATE verse.realm_proposals SET state = 'accepted', decided_ms = verse.now_ms() WHERE slug = s AND id = pid AND state = 'waiting' RETURNING 1)
  SELECT count(*)::int FROM d $$;

CREATE OR REPLACE FUNCTION verse.realm_world_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL THEN out
              ELSE out || CASE WHEN verse.edit_made(s) THEN E'made 1\n' || coalesce((SELECT string_agg('thing ' || t, E'\n' ORDER BY o) || E'\n'
                                                                                   FROM unnest(verse.builder_things()) WITH ORDINALITY AS x(t, o)), '')
                               ELSE '' END
                       || 'proposals ' || (SELECT count(*) FROM verse.realm_proposals WHERE slug = s AND state = 'waiting') || E'\n'
                       || 'building ' || CASE WHEN coalesce((SELECT coalesce((m.tools->>'build')::boolean, false) FROM verse.made_realms m WHERE m.slug = s),
                                                          (SELECT 'build' = ANY (h.tools) AND coalesce((SELECT (e.tools->>'build')::boolean FROM verse.realm_edit e WHERE e.slug = s), true)
                                                           FROM verse.realm_rights h WHERE h.slug = s), false) THEN '1' ELSE '0' END || E'\n' END
  FROM (SELECT verse.edit_spec(s, k, site_admin) AS out) x $$;
