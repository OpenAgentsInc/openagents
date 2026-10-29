-- Planetary Villa: a pet's skills, and the vases left in Hecate's Cemetery
-- (hc-skills, hc-vase, hc-vase-place; verse/planetary-villa/README.md).
--
-- Skills are chosen with the pet (fetch, obedience, digging) and kept in its
-- avatar. A vase is flowers picked in the field, arranged and left anywhere in
-- the garden or the field: everyone sees it, and it stays until its maker
-- cleans it up (who else may clean one up is still Kiki's open question); its
-- flowers fade over three days, which the page draws from its age. At most
-- three vases a player, seven flowers a vase, one vase a player a minute.

CREATE OR REPLACE FUNCTION villa.avatar_fix(a jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE out jsonb := '{}'; k text; lo real; hi real; d real; v real; c jsonb;
BEGIN
  IF jsonb_typeof(a) <> 'object' THEN a := '{}'; END IF;
  FOR k, lo, hi, d IN SELECT * FROM (VALUES ('size', 0.4, 1.8, 1), ('legs', 0.5, 1.6, 1), ('body', 0.6, 1.6, 1), ('girth', 0.6, 1.6, 1),
      ('head', 0.7, 1.5, 1), ('snout', 0.4, 1.6, 1), ('ears', 0, 1, 0), ('ear_size', 0.5, 1.8, 1), ('tail', 0, 1.8, 1), ('fluff', 0.5, 2, 1)) t LOOP
    v := CASE WHEN jsonb_typeof(a->k) = 'number' THEN (a->>k)::real ELSE d END;
    out := out || jsonb_build_object(k, round(greatest(lo, least(hi, v))::numeric, 3));
  END LOOP;
  out := out || jsonb_build_object('pattern', CASE WHEN a->>'pattern' IN ('plain', 'patches', 'points', 'stripes', 'socks') THEN a->>'pattern' ELSE 'plain' END);
  FOREACH k IN ARRAY ARRAY['coat', 'marks'] LOOP
    c := a->k;
    IF jsonb_typeof(c) = 'array' AND jsonb_array_length(c) = 3 AND (SELECT bool_and(jsonb_typeof(x) = 'number') FROM jsonb_array_elements(c) x) THEN
      out := out || jsonb_build_object(k, (SELECT jsonb_agg(round(greatest(0, least(1, (x #>> '{}')::real))::numeric, 3)) FROM jsonb_array_elements(c) x));
    ELSE
      out := out || jsonb_build_object(k, CASE k WHEN 'coat' THEN '[0.55, 0.36, 0.2]'::jsonb ELSE '[0.92, 0.88, 0.82]'::jsonb END);
    END IF;
  END LOOP;
  out := out || jsonb_build_object('skills', coalesce((SELECT jsonb_agg(DISTINCT x #>> '{}') FROM jsonb_array_elements(
    CASE WHEN jsonb_typeof(a->'skills') = 'array' THEN a->'skills' ELSE '["fetch", "obedience", "digging"]' END) x
    WHERE x #>> '{}' IN ('fetch', 'obedience', 'digging')), '[]'));
  RETURN out;
END $$;

CREATE TABLE villa.vases (
  id bigserial PRIMARY KEY,
  player text NOT NULL REFERENCES verse.players (id),
  x real NOT NULL, z real NOT NULL, yaw real NOT NULL DEFAULT 0,
  flowers jsonb NOT NULL,          -- [{kind 0..2, slot 0..6}]
  plot int,                        -- the tombstone's plot it was left at, if any
  placed_ms bigint NOT NULL
);
CREATE INDEX vases_placed ON villa.vases (placed_ms);

CREATE FUNCTION villa.vase_json(v villa.vases, me text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('n', v.id, 'x', v.x, 'z', v.z, 'yaw', v.yaw, 'flowers', v.flowers, 'plot', v.plot,
    'age', greatest(0, (verse.now_ms() - v.placed_ms) / 1000), 'mine', me IS NOT NULL AND v.player = me) $$;

-- GET vases: the last 80; POST vases {x, z, yaw, flowers, plot}; POST vases/<n>/clean (its maker).
CREATE FUNCTION villa.vases_api(method text, parts text[], body jsonb, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE me verse.players := villa.player(k); n int; fl jsonb; v villa.vases;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  IF cardinality(parts) = 1 AND method = 'GET' THEN
    RETURN villa.r(200, jsonb_build_object('vases', coalesce((SELECT jsonb_agg(villa.vase_json(w, me.id) ORDER BY w.placed_ms)
      FROM villa.vases w WHERE w.id IN (SELECT id FROM villa.vases ORDER BY placed_ms DESC LIMIT 80)), '[]')));
  END IF;
  IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;
  IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;
  IF cardinality(parts) = 1 THEN
    SELECT count(*) INTO n FROM villa.vases WHERE player = me.id;
    IF n >= 3 THEN RETURN villa.err(409, 'three vases is the most: clean one up first'); END IF;
    IF EXISTS (SELECT 1 FROM villa.vases WHERE player = me.id AND placed_ms > verse.now_ms() - 60000) THEN RETURN villa.err(429, 'a little slower'); END IF;
    SELECT coalesce(jsonb_agg(jsonb_build_object('kind', (f->>'kind')::int, 'slot', (f->>'slot')::int)), '[]') INTO fl
    FROM (SELECT f FROM jsonb_array_elements(CASE WHEN jsonb_typeof(body->'flowers') = 'array' THEN body->'flowers' ELSE '[]' END) f
          WHERE jsonb_typeof(f->'kind') = 'number' AND (f->>'kind')::numeric BETWEEN 0 AND 2
            AND jsonb_typeof(f->'slot') = 'number' AND (f->>'slot')::numeric BETWEEN 0 AND 6 LIMIT 7) z;
    IF jsonb_array_length(fl) = 0 THEN RETURN villa.err(400, 'put a flower in it first'); END IF;
    INSERT INTO villa.vases (player, x, z, yaw, flowers, plot, placed_ms)
    VALUES (me.id, verse.room_num(body->'x', 200), verse.room_num(body->'z', 200), verse.room_num(body->'yaw', 10), fl,
            CASE WHEN jsonb_typeof(body->'plot') = 'number' AND (body->>'plot')::numeric BETWEEN 0 AND 63 THEN (body->>'plot')::int END,
            verse.now_ms()) RETURNING * INTO v;
    RETURN villa.r(200, villa.vase_json(v, me.id));
  END IF;
  IF cardinality(parts) = 3 AND parts[3] = 'clean' AND parts[2] ~ '^[0-9]{1,18}$' THEN
    DELETE FROM villa.vases WHERE id = parts[2]::bigint AND player = me.id;
    RETURN villa.r(200, '{"cleaned":true}');
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;
