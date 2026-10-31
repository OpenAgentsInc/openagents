-- Hecate's Cemetery: more of a dog's look kept with its avatar (hc-dog-details): four more
-- marking layouts (spots, saddle, mask, blaze), the tail's shape, a fluffy chest, and a
-- collar with its colour and a tag. avatar_fix keeps them, each held to its range or words.
CREATE OR REPLACE FUNCTION villa.avatar_fix(a jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE out jsonb := '{}'; k text; lo real; hi real; d real; v real; c jsonb;
BEGIN
  IF jsonb_typeof(a) <> 'object' THEN a := '{}'; END IF;
  FOR k, lo, hi, d IN SELECT * FROM (VALUES ('size', 0.4, 1.8, 1), ('legs', 0.5, 1.6, 1), ('body', 0.6, 1.6, 1), ('girth', 0.6, 1.6, 1),
      ('head', 0.7, 1.5, 1), ('snout', 0.4, 1.6, 1), ('ears', 0, 1, 0), ('ear_size', 0.5, 1.8, 1), ('tail', 0, 1.8, 1), ('fluff', 0.5, 2, 1),
      ('chest', 0, 1, 0)) t LOOP
    v := CASE WHEN jsonb_typeof(a->k) = 'number' THEN (a->>k)::real ELSE d END;
    out := out || jsonb_build_object(k, round(greatest(lo, least(hi, v))::numeric, 3));
  END LOOP;
  out := out || jsonb_build_object('pattern', CASE WHEN a->>'pattern' IN ('plain', 'patches', 'points', 'stripes', 'socks',
      'spots', 'saddle', 'mask', 'blaze') THEN a->>'pattern' ELSE 'plain' END);
  out := out || jsonb_build_object('tail_shape', CASE WHEN a->>'tail_shape' IN ('straight', 'plume', 'curl') THEN a->>'tail_shape' ELSE 'straight' END);
  out := out || jsonb_build_object('collar', CASE WHEN a->>'collar' IN ('none', 'red', 'blue', 'green', 'purple', 'black', 'white') THEN a->>'collar' ELSE 'none' END);
  out := out || jsonb_build_object('tag', coalesce(jsonb_typeof(a->'tag') = 'boolean' AND (a->>'tag')::boolean, false));
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
