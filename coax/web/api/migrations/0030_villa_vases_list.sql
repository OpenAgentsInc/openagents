-- Planetary Villa's vase list again (0029_villa_play.sql): the list named its
-- rows x, which is also a vase's column, so GET /api/villa/vases failed where
-- 0029 went out first. The same function, its rows named w.
-- GET vases: the last 80; POST vases {x, z, yaw, flowers, plot}; POST vases/<n>/clean (its maker).
CREATE OR REPLACE FUNCTION villa.vases_api(method text, parts text[], body jsonb, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
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
