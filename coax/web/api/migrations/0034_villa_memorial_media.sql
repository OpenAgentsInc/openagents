-- Planetary Villa: the photos and videos you shared, on the memorial
-- (hc-memorial-media). The upload screen promised that only you see them, so
-- a file goes on the memorial only when its owner shows it (none are shown
-- until then), and only while the memorial is placed, the file is kept and no
-- admin has hidden it. Anyone may then fetch it through villa.c, which reads
-- it from the private bucket; nobody can fetch any other file. A shown file
-- can be reported like the memorial (kind 'file', its id's hex as ref).
--
--   POST /api/villa/pets/<id>/files/<f>/show|unshow   its owner     -> the pet
--   GET  /api/villa/memorial/<slug>/media/<hex>       anyone        -> the bytes (Range too)
--   a memorial's JSON gains media: [{id: <hex>, kind: image|video}], at most six

ALTER TABLE villa.files ADD COLUMN shown bool NOT NULL DEFAULT false;
ALTER TABLE villa.files ADD COLUMN held bool NOT NULL DEFAULT false;   -- an admin hid it (0033)
CREATE INDEX files_shown ON villa.files (pet, created_ms) WHERE shown AND status = 'kept';

CREATE FUNCTION villa.media_of(pet_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', substr(f.id, 3), 'kind', f.kind) ORDER BY f.created_ms), '[]')
  FROM (SELECT * FROM villa.files WHERE pet = pet_ AND status = 'kept' AND shown AND NOT held ORDER BY created_ms LIMIT 6) f $$;

CREATE OR REPLACE FUNCTION villa.public(p villa.pets) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('slug', p.slug, 'name', p.name, 'species', p.species, 'born', p.born, 'died', p.died, 'line', p.line,
    'story', p.story, 'traits', p.traits, 'achievements', p.achievements, 'avatar', p.avatar, 'plot', p.plot,
    'media', villa.media_of(p.id)) $$;
CREATE OR REPLACE FUNCTION villa.owned(p villa.pets) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT villa.public(p) || jsonb_build_object('id', p.id, 'status', p.status, 'choices', p.choices, 'agent', p.agent,
    'agent_note', p.agent_note, 'emotes', p.emotes,
    'files', coalesce((SELECT jsonb_agg(jsonb_build_object('id', f.id, 'name', f.name, 'kind', f.kind, 'bytes', f.bytes,
                         'status', f.status, 'note', f.note, 'shown', f.shown, 'held', f.held) ORDER BY f.created_ms)
                       FROM villa.files f WHERE f.pet = p.id AND f.status <> 'deleted'), '[]')) $$;

-- The owner's SHOW / UNSHOW, and anyone's fetch of a shown file. A fetch's result carries
-- stream {object, type, bytes} for villa.c.
CREATE FUNCTION villa.media_api(method text, parts text[], k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE me verse.players := villa.player(k); p villa.pets; f villa.files; n int;
BEGIN
  IF parts[1] = 'memorial' AND cardinality(parts) = 4 AND parts[3] = 'media' THEN
    IF method <> 'GET' THEN RETURN villa.err(405, 'GET'); END IF;
    SELECT f2.* INTO f FROM villa.files f2 JOIN villa.pets p2 ON p2.id = f2.pet
    WHERE p2.slug = parts[2] AND p2.status = 'placed' AND parts[4] ~ '^[0-9a-f]{16}$' AND f2.id = 'g-' || parts[4]
      AND f2.status = 'kept' AND f2.shown AND NOT f2.held;
    IF f.id IS NULL THEN RETURN villa.err(404, 'not here'); END IF;
    RETURN villa.r(200, '{}') || jsonb_build_object('stream', jsonb_build_object('object', f.object, 'type', f.content_type, 'bytes', f.bytes));
  END IF;
  IF parts[1] = 'pets' AND cardinality(parts) = 5 AND parts[3] = 'files' AND parts[5] IN ('show', 'unshow') THEN
    IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;
    IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;
    SELECT * INTO p FROM villa.pets WHERE id = parts[2] AND player = me.id AND status <> 'removed';
    IF p.id IS NULL THEN RETURN villa.err(404, 'not found'); END IF;
    UPDATE villa.files SET shown = parts[5] = 'show' WHERE id = parts[4] AND pet = p.id AND status = 'kept';
    GET DIAGNOSTICS n = ROW_COUNT;
    IF n = 0 THEN RETURN villa.err(404, 'that file isn''t kept'); END IF;
    RETURN villa.r(200, villa.owned(p));
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;

-- Reports of a shown file (0031, 0033): what it is, hiding it, bringing it back.
ALTER TABLE villa.reports DROP CONSTRAINT reports_kind_check;
ALTER TABLE villa.reports ADD CONSTRAINT reports_kind_check CHECK (kind IN ('memorial', 'note', 'chat', 'file'));
CREATE OR REPLACE FUNCTION villa.report_kinds() RETURNS text[] LANGUAGE sql IMMUTABLE AS $$ SELECT ARRAY['memorial', 'note', 'chat', 'file'] $$;

CREATE OR REPLACE FUNCTION villa.report_shown(kind_ text, ref_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(CASE kind_
    WHEN 'memorial' THEN (SELECT left(concat_ws(' · ', name, line, story, traits, achievements), 2000) FROM villa.pets WHERE slug = ref_ AND status = 'placed')
    WHEN 'note' THEN (SELECT name || ': ' || text FROM villa.notes WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint)
    WHEN 'chat' THEN (SELECT name || ': ' || text FROM verse.room_chat WHERE room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND n = ref_::int)
    WHEN 'file' THEN (SELECT 'A ' || CASE f.kind WHEN 'video' THEN 'video' ELSE 'photo' END || ' on ' || p.name || '''s memorial (/api/villa/memorial/'
                             || p.slug || '/media/' || ref_ || '): ' || f.name
                      FROM villa.files f JOIN villa.pets p ON p.id = f.pet
                      WHERE ref_ ~ '^[0-9a-f]{16}$' AND f.id = 'g-' || ref_ AND f.status = 'kept' AND f.shown AND NOT f.held AND p.status = 'placed')
  END, '') $$;

CREATE OR REPLACE FUNCTION villa.report_hide(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ = 'memorial' THEN
    UPDATE villa.pets SET status = 'held', updated_ms = verse.now_ms() WHERE slug = ref_ AND status IN ('placed', 'held');
  ELSIF kind_ = 'note' THEN
    UPDATE villa.notes SET held = true WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint;
  ELSIF kind_ = 'chat' THEN
    UPDATE verse.room_chat SET text = villa.removed_text() WHERE room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND room_chat.n = ref_::int;
  ELSIF kind_ = 'file' THEN
    UPDATE villa.files SET held = true WHERE ref_ ~ '^[0-9a-f]{16}$' AND id = 'g-' || ref_ AND status <> 'deleted';
  ELSE RETURN false;
  END IF;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n > 0;
END $$;

CREATE OR REPLACE FUNCTION villa.report_restore(kind_ text, ref_ text) RETURNS bool LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ = 'memorial' THEN
    PERFORM set_config('villa.restoring', 'on', true);
    UPDATE villa.pets SET status = 'placed', updated_ms = verse.now_ms() WHERE slug = ref_ AND status = 'held';
    GET DIAGNOSTICS n = ROW_COUNT;
    PERFORM set_config('villa.restoring', '', true);
  ELSIF kind_ = 'note' THEN
    UPDATE villa.notes SET held = false, hidden = false WHERE ref_ ~ '^[0-9]{1,18}$' AND id = ref_::bigint;
    GET DIAGNOSTICS n = ROW_COUNT;
  ELSIF kind_ = 'chat' THEN
    UPDATE verse.room_chat c SET text = substr(r.shown, length(c.name) + 3)
    FROM (SELECT shown FROM villa.reports WHERE kind = 'chat' AND ref = ref_ ORDER BY at_ms LIMIT 1) r
    WHERE c.room = villa.chat_room() AND ref_ ~ '^[0-9]{1,9}$' AND c.n = ref_::int AND c.text IN (villa.removed_text(), villa.held_text());
    GET DIAGNOSTICS n = ROW_COUNT;
  ELSIF kind_ = 'file' THEN
    UPDATE villa.files SET held = false WHERE ref_ ~ '^[0-9a-f]{16}$' AND id = 'g-' || ref_ AND status = 'kept';
    GET DIAGNOSTICS n = ROW_COUNT;
  ELSE RETURN false;
  END IF;
  RETURN n > 0;
END $$;
