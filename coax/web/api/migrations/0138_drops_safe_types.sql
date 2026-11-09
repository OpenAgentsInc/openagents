-- Published files and model offsets (0016, 0038, 0040).
--
-- A kept file is served from the site's own origin, so its type must be one a
-- browser shows or saves and never opens as a page: an SVG can carry a script,
-- so it is kept and published as plain text (it can still be saved and read).
-- drops.c sets the type on every copy from its own list of these types. Files
-- already kept take the new type, so publishing them again is safe.
--
-- A model's offsets, lengths, counts and strides are whole numbers no bigger
-- than a binary glTF can hold; anything else leaves the accessor or the image
-- out, as drops.c reads only what lies inside the binary chunk.

CREATE OR REPLACE FUNCTION drops.content_type(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE ext
    WHEN 'txt' THEN 'text/plain; charset=utf-8' WHEN 'md' THEN 'text/markdown; charset=utf-8'
    WHEN 'markdown' THEN 'text/markdown; charset=utf-8' WHEN 'csv' THEN 'text/csv; charset=utf-8'
    WHEN 'json' THEN 'application/json' WHEN 'pdf' THEN 'application/pdf' WHEN 'rtf' THEN 'application/rtf'
    WHEN 'docx' THEN 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
    WHEN 'odt' THEN 'application/vnd.oasis.opendocument.text'
    WHEN 'png' THEN 'image/png' WHEN 'jpg' THEN 'image/jpeg' WHEN 'jpeg' THEN 'image/jpeg' WHEN 'gif' THEN 'image/gif'
    WHEN 'webp' THEN 'image/webp' WHEN 'avif' THEN 'image/avif' WHEN 'heic' THEN 'image/heic' WHEN 'svg' THEN 'text/plain; charset=utf-8'
    WHEN 'mp3' THEN 'audio/mpeg' WHEN 'wav' THEN 'audio/wav' WHEN 'm4a' THEN 'audio/mp4' WHEN 'aac' THEN 'audio/aac'
    WHEN 'ogg' THEN 'audio/ogg' WHEN 'opus' THEN 'audio/ogg' WHEN 'flac' THEN 'audio/flac' WHEN 'webm' THEN 'audio/webm'
    WHEN 'glb' THEN 'model/gltf-binary' WHEN 'gltf' THEN 'model/gltf+json' WHEN 'obj' THEN 'model/obj'
    WHEN 'mtl' THEN 'model/mtl' WHEN 'stl' THEN 'model/stl' WHEN 'usdz' THEN 'model/vnd.usdz+zip'
    WHEN 'mp4' THEN 'video/mp4' WHEN 'mov' THEN 'video/quicktime' WHEN 'zip' THEN 'application/zip'
    WHEN 'ktx2' THEN 'image/ktx2'
    ELSE 'application/octet-stream'
  END $$;

UPDATE drops.files SET content_type = drops.content_type(ext) WHERE ext = 'svg' OR content_type = 'image/svg+xml';

-- A whole number from 0 to top in a model's JSON, or NULL (a sign, a fraction, a string, too big).
CREATE FUNCTION drops.glb_whole(v jsonb, top bigint) RETURNS bigint LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v) = 'number' AND v::text ~ '^[0-9]{1,12}$'
              THEN CASE WHEN v::text::bigint <= top THEN v::text::bigint END END $$;
-- The same, with absent when the field isn't there at all.
CREATE FUNCTION drops.glb_whole_or(v jsonb, top bigint, absent bigint) RETURNS bigint LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN v IS NULL THEN absent ELSE drops.glb_whole(v, top) END $$;

-- Where an accessor's elements lie in the binary chunk: "AT COUNT COMPONENT_TYPE COMPONENTS STRIDE END NORMALIZED",
-- or NULL when they don't lie there plainly (no buffer view, another buffer, sparse) or a number isn't a whole one in range.
CREATE OR REPLACE FUNCTION drops.glb_acc(j jsonb, i int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN a IS NULL OR v IS NULL OR a ? 'sparse' OR coalesce(v->'buffer', '0') <> '0'
              OR vat IS NULL OR aat IS NULL OR cnt IS NULL OR comp IS NULL OR stride IS NULL OR vlen IS NULL THEN NULL
    ELSE concat_ws(' ', vat + aat, cnt, comp,
                   CASE a->>'type' WHEN 'SCALAR' THEN 1 WHEN 'VEC2' THEN 2 WHEN 'VEC3' THEN 3 WHEN 'VEC4' THEN 4 ELSE 0 END,
                   stride, vat + vlen, CASE WHEN a->'normalized' = 'true' THEN 1 ELSE 0 END) END
  FROM (SELECT CASE WHEN jsonb_typeof(j->'accessors'->i) = 'object' THEN j->'accessors'->i END AS a) x
  LEFT JOIN LATERAL (SELECT j->'bufferViews'->(drops.glb_whole(a->'bufferView', 2147483647)::int) AS v WHERE jsonb_typeof(a->'bufferView') = 'number') y ON true
  LEFT JOIN LATERAL (SELECT drops.glb_whole_or(v->'byteOffset', 4294967295, 0) AS vat, drops.glb_whole_or(a->'byteOffset', 4294967295, 0) AS aat,
                            drops.glb_whole(a->'count', 4294967295) AS cnt, drops.glb_whole(a->'componentType', 65535) AS comp,
                            drops.glb_whole_or(v->'byteStride', 255, 0) AS stride, drops.glb_whole_or(v->'byteLength', 4294967295, 0) AS vlen
                     WHERE jsonb_typeof(v) = 'object') z ON true $$;

-- The images a model carries in its binary chunk, PNG or JPEG: where each lies (whole numbers in range only).
CREATE OR REPLACE FUNCTION drops.glb_images(j jsonb) RETURNS TABLE (image int, at bigint, len bigint) LANGUAGE sql IMMUTABLE AS $$
  SELECT (k - 1)::int, vat, vlen
  FROM jsonb_array_elements(CASE WHEN jsonb_typeof(j->'images') = 'array' THEN j->'images' ELSE '[]' END) WITH ORDINALITY AS z(im, k)
  CROSS JOIN LATERAL (SELECT CASE WHEN jsonb_typeof(im) = 'object' THEN j->'bufferViews'->(drops.glb_whole(im->'bufferView', 2147483647)::int) END AS v) y
  CROSS JOIN LATERAL (SELECT drops.glb_whole_or(v->'byteOffset', 4294967295, 0) AS vat, drops.glb_whole(v->'byteLength', 4294967295) AS vlen) w
  WHERE jsonb_typeof(v) = 'object' AND coalesce(v->'buffer', '0') = '0'
    AND coalesce(im->>'mimeType', '') IN ('image/png', 'image/jpeg') AND vat IS NOT NULL AND vlen IS NOT NULL
  ORDER BY k $$;
