-- A realm's placed things: the rules leave plpgsql (#13514, coax/web/api/COAX.md "Where API rules go"). Whether a
-- spawn or a despawn may stand (only its owner and admins, a kind it places, its own size, 40 m on a made realm and
-- 400 m on a hand-built one, 200 things, a made realm's 64 changes, one at a time) is coax/web/api/realm-place-policy.coax's,
-- asked by metaverse/edit/realm-world.coax for each edit of a batch on the facts of the realm's spec. SQL keeps the
-- rows: worlds.c realm_post carries out what was taken with the two writes below, in the transaction that saves the
-- world's revision. 0190's verse.realm_spawn and verse.realm_despawn go.
--
-- The world reads one more fact: verse.realm_world_spec says `made 1` for a made realm (its reach and its change limit).

DROP FUNCTION verse.realm_spawn(text, int, text, int, int, int, text, boolean);
DROP FUNCTION verse.realm_despawn(text, int, text, boolean);

-- A thing added at the end of a realm's placed list, at (px, pz) centimetres turned pyaw whole degrees, in metres and
-- radians as the list keeps them. -> its ID, the list's new length (0: no realm has this slug).
CREATE FUNCTION verse.realm_thing_add(s text, pthing text, px int, pz int, pyaw int, k text) RETURNS int LANGUAGE sql AS $$
  WITH t AS (SELECT jsonb_build_object('thing', pthing, 'x', trim_scale(round(px / 100.0, 2)), 'z', trim_scale(round(pz / 100.0, 2)),
                                       'yaw', trim_scale(round(radians(pyaw)::numeric, 3))) AS v),
  m AS (UPDATE verse.made_realms SET placed = placed || jsonb_build_array((SELECT v FROM t)), settings_ms = verse.now_ms()
        WHERE slug = s RETURNING jsonb_array_length(placed) AS len),
  h AS (INSERT INTO verse.realm_edit (slug, placed, ms, by_player)
        SELECT s, jsonb_build_array(t.v), verse.now_ms(), (verse.builder_player(k)).id FROM t
        WHERE NOT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s) AND EXISTS (SELECT 1 FROM verse.realm_rights WHERE slug = s)
        ON CONFLICT (slug) DO UPDATE SET placed = verse.realm_edit.placed || excluded.placed, ms = excluded.ms, by_player = excluded.by_player
        RETURNING jsonb_array_length(placed) AS len)
  SELECT coalesce((SELECT len FROM m), (SELECT len FROM h), 0) $$;

-- Thing pid of a realm's placed list marked gone (away) or standing again (not away): a made realm's 0109 shaped entry
-- (its turn and size kept; the entry goes when it keeps neither), a hand-built realm's "gone" on the entry.
-- -> pid, or 0 when the list has no such thing.
CREATE FUNCTION verse.realm_thing_mark(s text, pid int, away boolean) RETURNS int LANGUAGE sql AS $$
  WITH e AS (SELECT 'placed:' || (pid - 1) AS ent),
  sh AS (SELECT (SELECT v FROM jsonb_array_elements(r.shaped) v WHERE v->>'entity' = (SELECT ent FROM e) LIMIT 1) AS v
         FROM verse.made_realms r WHERE r.slug = s),
  m AS (UPDATE verse.made_realms SET shaped = verse.realm_entry(shaped, (SELECT ent FROM e),
          CASE WHEN away THEN jsonb_build_object('entity', (SELECT ent FROM e), 'yaw', coalesce(((SELECT v FROM sh)->>'yaw')::int, 0),
                                                 'scale', coalesce(((SELECT v FROM sh)->>'scale')::int, 100), 'gone', true)
               WHEN coalesce(((SELECT v FROM sh)->>'yaw')::int, 0) = 0 AND coalesce(((SELECT v FROM sh)->>'scale')::int, 100) = 100 THEN NULL
               ELSE (SELECT v FROM sh) || '{"gone": false}' END), settings_ms = verse.now_ms()
        WHERE slug = s AND pid BETWEEN 1 AND jsonb_array_length(placed) RETURNING 1),
  h AS (UPDATE verse.realm_edit SET placed = jsonb_set(placed, ARRAY[(pid - 1)::text],
          CASE WHEN away THEN (placed -> (pid - 1)) || '{"gone": true}' ELSE (placed -> (pid - 1)) - 'gone' END), ms = verse.now_ms()
        WHERE slug = s AND pid BETWEEN 1 AND jsonb_array_length(placed) AND NOT EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s) RETURNING 1)
  SELECT CASE WHEN EXISTS (SELECT 1 FROM m) OR EXISTS (SELECT 1 FROM h) THEN pid ELSE 0 END $$;

-- The spec the world reads: 0190's, and a made realm says so.
CREATE OR REPLACE FUNCTION verse.realm_world_spec(s text, k text, site_admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN out IS NULL OR NOT verse.edit_made(s) THEN out
              ELSE out || E'made 1\n' || coalesce((SELECT string_agg('thing ' || t, E'\n' ORDER BY o) || E'\n' FROM unnest(verse.builder_things()) WITH ORDINALITY AS x(t, o)), '') END
  FROM (SELECT verse.edit_spec(s, k, site_admin) AS out) x $$;
