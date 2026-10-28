-- A player's own realm, the demo's planets (pt-planets; /api/award and
-- /api/planets*, aethyr-cpp tools/serve.py at demo/kiki-2026-10-28d). A user
-- (an account, else the id the page sends) is awarded one planet id, made
-- once. Setting it up keeps the planet setup's own choices as the galaxy
-- draws them; each setup takes the next number, so a galaxy that asks for
-- what came after the last number it saw gets the new and changed planets.
-- A planet is a world too (verse.worlds): the player who set it up owns it,
-- and only its owner sets it up again or keeps its saved copy.
--
-- The demo's awards.json, planets.json and planet-save-<id>.json, and the
-- planets the Kiki flow published before it kept a registry (their
-- world-planet-<id>.json names the planet), wait in verse.demo_files (0009):
-- verse.planets_from_demo() copies them here, again after every import, and
-- adds only what is new.
CREATE TABLE verse.awards (
  user_id text PRIMARY KEY,
  planet text NOT NULL,
  at double precision NOT NULL,
  source text
);
CREATE INDEX awards_planet ON verse.awards (planet);
CREATE SEQUENCE verse.planet_n;
CREATE TABLE verse.planets (
  id text PRIMARY KEY,
  n bigint UNIQUE NOT NULL,
  name text NOT NULL,
  owner text NOT NULL,
  owner_id text NOT NULL,        -- never on a public route
  kind text NOT NULL,
  art text NOT NULL,
  cam text NOT NULL,
  cycle text NOT NULL,
  made double precision NOT NULL,
  at double precision NOT NULL,
  source text
);
CREATE TABLE verse.planet_saves (
  planet text PRIMARY KEY,
  saved jsonb,                   -- {"doc", "at"} or null
  invited double precision,
  source text
);

CREATE FUNCTION verse.planet_json(p verse.planets) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('id', p.id, 'n', p.n, 'name', p.name, 'owner', p.owner, 'kind', p.kind, 'art', p.art,
    'cam', p.cam, 'cycle', p.cycle, 'made', p.made, 'at', p.at)
$$;
CREATE FUNCTION verse.planet_pick(v text, options text[]) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN v = ANY (options) THEN v ELSE options[1] END
$$;

CREATE FUNCTION verse.planets_from_demo() RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  added int := 0;
  m int;
BEGIN
  WITH a AS (
    SELECT DISTINCT ON (x.key) x.key AS u, x.value #>> '{}' AS planet, f.path
    FROM verse.demo_files f, jsonb_each(CASE WHEN jsonb_typeof(f.doc) = 'object' THEN f.doc ELSE '{}' END) x
    WHERE f.path ~ '^aethyr-demo-data/(dev/)?awards\.json$' AND jsonb_typeof(x.value) = 'string'
      AND x.key ~ '^[A-Za-z0-9_-]{1,64}$' AND x.value #>> '{}' ~ '^[A-Za-z0-9_-]{1,64}$'
    ORDER BY x.key, f.space = 'kiki' DESC
  ), n AS (
    INSERT INTO verse.awards (user_id, planet, at, source)
    SELECT u, planet, extract(epoch FROM clock_timestamp()), path FROM a ORDER BY u
    ON CONFLICT DO NOTHING RETURNING 1)
  SELECT count(*) INTO m FROM n;
  added := added + m;

  WITH r AS (
    SELECT DISTINCT ON (p.key) p.value AS v, f.path
    FROM verse.demo_files f, jsonb_each(CASE WHEN jsonb_typeof(f.doc->'planets') = 'object' THEN f.doc->'planets' ELSE '{}' END) p
    WHERE f.path ~ '^aethyr-demo-data/(dev/)?planets\.json$' AND jsonb_typeof(p.value) = 'object' AND p.key ~ '^[A-Za-z0-9_-]{1,64}$'
    ORDER BY p.key, f.space = 'kiki' DESC
  ), w AS (
    SELECT DISTINCT ON (d.v->>'id') d.v, d.at, d.path FROM (
      SELECT f.doc->'doc'->'planet' AS v, (f.doc->>'publishedAt')::double precision AS at, f.path, f.space
      FROM verse.demo_files f
      WHERE f.path ~ '^aethyr-demo-data/(dev/)?world-planet-[A-Za-z0-9_-]{1,57}\.json$'
        AND jsonb_typeof(f.doc->'doc'->'planet') = 'object' AND jsonb_typeof(f.doc->'publishedAt') = 'number') d
    WHERE d.v->>'id' ~ '^[A-Za-z0-9_-]{1,64}$' AND d.path LIKE '%/world-' || (d.v->>'id') || '.json'
    ORDER BY d.v->>'id', d.space = 'kiki' DESC
  ), src AS (
    SELECT v->>'id' AS id, v, (v->>'made')::double precision AS made, (v->>'at')::double precision AS at, v->>'ownerId' AS owner_id, path
    FROM r WHERE v->>'id' ~ '^[A-Za-z0-9_-]{1,64}$' AND jsonb_typeof(v->'at') = 'number'
    UNION ALL
    SELECT v->>'id', v, at, at, (SELECT x.user_id FROM verse.awards x WHERE x.planet = v->>'id' ORDER BY x.user_id LIMIT 1), path
    FROM w WHERE NOT EXISTS (SELECT 1 FROM r WHERE r.v->>'id' = w.v->>'id')
  ), n AS (
    INSERT INTO verse.planets (id, n, name, owner, owner_id, kind, art, cam, cycle, made, at, source)
    SELECT id, nextval('verse.planet_n'), left(coalesce(nullif(btrim(v->>'name'), ''), 'My planet'), 40),
           left(coalesce(nullif(btrim(v->>'owner'), ''), 'Guest'), 40), coalesce(owner_id, ''),
           verse.planet_pick(v->>'kind', ARRAY['forest', 'village', 'cemetery']),
           verse.planet_pick(v->>'art', ARRAY['Low poly', 'Painted', 'Pixel', 'Ink']),
           verse.planet_pick(v->>'cam', ARRAY['Third person', 'First person', 'Top down']),
           verse.planet_pick(v->>'cycle', ARRAY['Day/night cycle', 'Always day', 'Always night']),
           coalesce(made, at), at, path
    FROM (SELECT * FROM src ORDER BY at, id) s
    ON CONFLICT DO NOTHING RETURNING 1)
  SELECT count(*) INTO m FROM n;
  added := added + m;

  WITH s AS (
    SELECT substring(f.path FROM '/planet-save-([A-Za-z0-9_-]{1,64})\.json$') AS planet, f.doc, f.path
    FROM verse.demo_files f WHERE f.path ~ '/planet-save-[A-Za-z0-9_-]{1,64}\.json$' AND jsonb_typeof(f.doc) = 'object'
  ), n AS (
    INSERT INTO verse.planet_saves (planet, saved, invited, source)
    SELECT planet, CASE WHEN jsonb_typeof(doc->'saved') = 'object' THEN doc->'saved' END,
           CASE WHEN jsonb_typeof(doc->'invited') = 'number' THEN (doc->>'invited')::double precision
                WHEN doc->'invited' = 'true'::jsonb THEN 0 END, path
    FROM s ON CONFLICT DO NOTHING RETURNING 1)
  SELECT count(*) INTO m FROM n;
  RETURN added + m;
END $$;
SELECT verse.planets_from_demo();

-- The routes before this one, kept as they were.
ALTER FUNCTION verse.api(text, text[], jsonb, jsonb) RENAME TO api_0010;

CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  k text := who->>'key';
  npub text := nullif(who->>'npub', '');
  site boolean := coalesce((who->>'site')::boolean, false);
  now_s double precision := extract(epoch FROM clock_timestamp());
  n int := coalesce(array_length(parts, 1), 0);
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  rec verse.players;
  acct text;
  u text;
  uname text;
  pid text;
  w verse.worlds;
  p verse.planets;
  s jsonb;
  ps verse.planet_saves;
  after_n bigint;
BEGIN
  IF parts[1] NOT IN ('award', 'planets') OR n > 2 THEN RETURN verse.api_0010(method, parts, body, who); END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF npub IS NOT NULL THEN
    INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-' || npub, npub, verse.now_ms()) ON CONFLICT DO NOTHING;
    acct := 'n-' || npub;
  ELSE
    acct := rec.account;
  END IF;

  IF parts = ARRAY['award'] AND method = 'POST' THEN
    u := left(coalesce(b->>'user', ''), 64);
    INSERT INTO verse.awards (user_id, planet, at)
    VALUES (u, 'planet-' || right(regexp_replace(u, '[^A-Za-z0-9]', '', 'g'), 12) || right(to_hex(floor(now_s)::bigint), 4), now_s)
    ON CONFLICT DO NOTHING;
    RETURN verse.r(200, jsonb_build_object('planet', (SELECT planet FROM verse.awards WHERE user_id = u)));
  END IF;

  IF parts = ARRAY['planets'] AND method = 'GET' THEN
    after_n := CASE WHEN who->>'after' ~ '^[0-9]{1,15}$' THEN (who->>'after')::bigint ELSE 0 END;
    RETURN verse.r(200, jsonb_build_object(
      'planets', coalesce((SELECT jsonb_agg(verse.planet_json(x) ORDER BY x.n) FROM verse.planets x WHERE x.n > after_n), '[]'),
      'n', coalesce((SELECT max(x.n) FROM verse.planets x), 0)));
  END IF;

  IF parts = ARRAY['planets'] AND method = 'POST' THEN
    IF rec.id IS NULL AND acct IS NULL THEN
      RETURN verse.r(403, '{"error":"send a player key (POST /api/playtest/player) to make a planet"}');
    END IF;
    IF acct IS NOT NULL THEN
      u := acct;
      uname := coalesce((SELECT a.name FROM verse.accounts a WHERE a.id = acct),
                        CASE WHEN jsonb_typeof(b->'user') = 'object' THEN b->'user'->>'name' END);
    ELSE
      u := CASE WHEN jsonb_typeof(b->'user') = 'object' THEN left(b->'user'->>'id', 64) END;
      uname := CASE WHEN jsonb_typeof(b->'user') = 'object' THEN b->'user'->>'name' END;
    END IF;
    s := b->'settings';
    IF coalesce(u, '') !~ '^[A-Za-z0-9_-]{1,64}$' OR jsonb_typeof(s) IS DISTINCT FROM 'object' THEN
      RETURN verse.r(400, '{"error":"expected the user and the planet''s settings"}');
    END IF;
    INSERT INTO verse.awards (user_id, planet, at)
    VALUES (u, 'planet-' || right(regexp_replace(u, '[^A-Za-z0-9]', '', 'g'), 12) || right(to_hex(floor(now_s)::bigint), 4), now_s)
    ON CONFLICT DO NOTHING;
    SELECT planet INTO pid FROM verse.awards WHERE user_id = u;
    PERFORM pg_advisory_xact_lock(7731004, hashtext(pid));
    SELECT * INTO w FROM verse.worlds WHERE id = pid;
    IF w.id IS NOT NULL AND (w.owner_player IS NOT NULL OR w.owner_account IS NOT NULL) AND NOT (site OR verse.owns(rec, acct, pid)) THEN
      RETURN verse.r(403, '{"error":"this planet belongs to another player"}');
    END IF;
    IF w.id IS NULL THEN
      INSERT INTO verse.worlds (id, owner_player, owner_account, created_ms) VALUES (pid, rec.id, acct, verse.now_ms());
    ELSIF w.owner_player IS NULL AND w.owner_account IS NULL THEN
      UPDATE verse.worlds SET owner_player = rec.id, owner_account = acct WHERE id = pid;
    END IF;
    INSERT INTO verse.planets AS x (id, n, name, owner, owner_id, kind, art, cam, cycle, made, at)
    VALUES (pid, nextval('verse.planet_n'), left(coalesce(nullif(btrim(s->>'name'), ''), 'My planet'), 40),
            left(coalesce(nullif(btrim(uname), ''), 'Guest'), 40), u,
            verse.planet_pick(s->>'kind', ARRAY['forest', 'village', 'cemetery']),
            verse.planet_pick(s->>'art', ARRAY['Low poly', 'Painted', 'Pixel', 'Ink']),
            verse.planet_pick(s->>'cam', ARRAY['Third person', 'First person', 'Top down']),
            verse.planet_pick(s->>'cycle', ARRAY['Day/night cycle', 'Always day', 'Always night']), now_s, now_s)
    ON CONFLICT (id) DO UPDATE SET n = EXCLUDED.n, name = EXCLUDED.name, owner = EXCLUDED.owner, owner_id = EXCLUDED.owner_id,
      kind = EXCLUDED.kind, art = EXCLUDED.art, cam = EXCLUDED.cam, cycle = EXCLUDED.cycle, at = EXCLUDED.at, source = NULL
    RETURNING * INTO p;
    RETURN verse.r(200, verse.planet_json(p));
  END IF;

  IF parts = ARRAY['planets', 'mine'] THEN
    IF rec.id IS NOT NULL OR acct IS NOT NULL THEN
      SELECT * INTO p FROM verse.planets x WHERE verse.owns(rec, acct, x.id) ORDER BY x.n DESC LIMIT 1;
    END IF;
    IF method = 'GET' THEN
      IF p.id IS NULL THEN RETURN verse.r(200, '{"planet":null,"invited":false,"saved":null}'); END IF;
      SELECT * INTO ps FROM verse.planet_saves x WHERE x.planet = p.id;
      RETURN verse.r(200, jsonb_build_object('planet', verse.planet_json(p), 'invited', ps.invited IS NOT NULL, 'saved', ps.saved));
    END IF;
    IF p.id IS NULL THEN RETURN verse.r(404, '{"error":"make a planet first"}'); END IF;
    IF b ? 'doc' AND jsonb_typeof(b->'doc') NOT IN ('object', 'null') THEN
      RETURN verse.r(400, '{"error":"the planet must be a world record"}');
    END IF;
    INSERT INTO verse.planet_saves AS x (planet) VALUES (p.id) ON CONFLICT DO NOTHING;
    UPDATE verse.planet_saves x SET
      saved = CASE WHEN NOT b ? 'doc' THEN x.saved WHEN jsonb_typeof(b->'doc') = 'object' AND b->'doc' <> '{}'
                   THEN jsonb_build_object('doc', b->'doc', 'at', now_s) END,
      invited = CASE WHEN coalesce(b->'invited' = 'true'::jsonb, false) THEN coalesce(x.invited, now_s) ELSE x.invited END,
      source = NULL
    WHERE x.planet = p.id RETURNING * INTO ps;
    RETURN verse.r(200, jsonb_build_object('id', p.id, 'invited', ps.invited IS NOT NULL, 'at', ps.saved->'at'));
  END IF;

  IF parts[1] = 'planets' AND n = 2 AND method = 'GET' THEN
    SELECT * INTO p FROM verse.planets x WHERE x.id = parts[2];
    IF p.id IS NULL THEN RETURN verse.r(404, '{"error":"no such planet"}'); END IF;
    RETURN verse.r(200, verse.planet_json(p));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;
