-- Planetary Villa's pets (Hecate's Cemetery: hc-pet-upload, hc-pet-avatar-agent,
-- hc-pet-customize, hc-tombstones, hc-memorial-page, hc-memorial-share,
-- hc-safety; verse/planetary-villa/README.md). villa.c reads who is asking
-- (the playtest player key, X-Verse-Player) and does the bucket's and the
-- model's part; every rule is here.
--
-- A pet belongs to the player that made it. What its owner shares about it
-- (photos and short videos) goes to the private drops bucket under
-- <env>/villa/<pet>/ and is read only by the avatar agent: nobody else is ever
-- given a file, and its owner can delete any of them, or the whole pet. When
-- the owner presses PLAY the pet's tombstone takes a plot in the garden, and
-- its memorial page (name, dates, line, story, traits, achievements, avatar)
-- is open to anyone with its link; anyone with a player key may leave a
-- testimonial there, and the owner may hide one.
--
-- Limits: 6 pets a player, 12 files a pet, an image at most 16 MiB and a
-- video at most 64 MiB (jpg, png, webp, gif, heic; mp4, mov, webm); a file's
-- first bytes must be what its name says (drops.sniff). Text is cut to its
-- field's length and loses control characters. A testimonial is at most 600
-- characters, three a player a minute; a link in one is refused.
CREATE SCHEMA IF NOT EXISTS villa;

CREATE TABLE villa.pets (
  id text PRIMARY KEY,             -- q-<16 hex>
  slug text UNIQUE NOT NULL,       -- 8 letters and digits: the share link's
  player text NOT NULL REFERENCES verse.players (id),
  name text NOT NULL DEFAULT '',
  species text NOT NULL DEFAULT 'dog',
  born text NOT NULL DEFAULT '',
  died text NOT NULL DEFAULT '',   -- '' for a pet still with you
  line text NOT NULL DEFAULT '',   -- the tombstone's line
  story text NOT NULL DEFAULT '',
  traits text NOT NULL DEFAULT '',
  achievements text NOT NULL DEFAULT '',
  avatar jsonb NOT NULL DEFAULT '{}',
  choices jsonb NOT NULL DEFAULT '[]',   -- what the agent chose and why
  agent text NOT NULL DEFAULT 'none' CHECK (agent IN ('none', 'reading', 'ready', 'failed')),
  agent_note text NOT NULL DEFAULT '',
  agent_ms bigint,
  plot int UNIQUE CHECK (plot BETWEEN 0 AND 63),
  status text NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'placed', 'removed')),
  created_ms bigint NOT NULL,
  updated_ms bigint NOT NULL
);
CREATE INDEX pets_player ON villa.pets (player, created_ms);
CREATE TABLE villa.files (
  id text PRIMARY KEY,             -- g-<16 hex>
  pet text NOT NULL REFERENCES villa.pets (id),
  name text NOT NULL,
  ext text NOT NULL,
  kind text NOT NULL,              -- image, video
  bytes bigint NOT NULL,
  content_type text NOT NULL,
  object text NOT NULL,
  status text NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'uploading', 'kept', 'refused', 'deleted')),
  note text NOT NULL DEFAULT '',
  created_ms bigint NOT NULL
);
CREATE INDEX files_pet ON villa.files (pet, created_ms);
CREATE TABLE villa.notes (           -- testimonials on a memorial
  id bigserial PRIMARY KEY,
  pet text NOT NULL REFERENCES villa.pets (id),
  player text NOT NULL REFERENCES verse.players (id),
  name text NOT NULL,
  text text NOT NULL,
  hidden bool NOT NULL DEFAULT false,
  created_ms bigint NOT NULL
);
CREATE INDEX notes_pet ON villa.notes (pet, created_ms);

CREATE FUNCTION villa.r(status int, body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT jsonb_build_object('status', status, 'body', body) $$;
CREATE FUNCTION villa.err(status int, why text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT villa.r(status, jsonb_build_object('error', why)) $$;

-- Text as kept: no control characters, trimmed, at most n characters.
CREATE FUNCTION villa.clean(v jsonb, n int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(CASE WHEN jsonb_typeof(v) = 'string' THEN v #>> '{}' ELSE '' END,
                                   '[\x00-\x09\x0b-\x1f\x7f]+', ' ', 'g')), n) $$;
CREATE FUNCTION villa.field_cap(f text) RETURNS int LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE f WHEN 'name' THEN 40 WHEN 'species' THEN 20 WHEN 'born' THEN 24 WHEN 'died' THEN 24 WHEN 'line' THEN 120
    WHEN 'story' THEN 4000 WHEN 'traits' THEN 600 WHEN 'achievements' THEN 600 END $$;
-- A link or an address in a testimonial, or NULL.
CREATE FUNCTION villa.linky(t text) RETURNS bool LANGUAGE sql IMMUTABLE AS $$
  SELECT t ~* '(https?://|www\.|[a-z0-9-]+\.(com|net|org|io|ru|xyz|top|link|ly|gg|me)\b)' $$;

CREATE FUNCTION villa.kind(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN ext IN ('jpg', 'jpeg', 'png', 'webp', 'gif', 'heic') THEN 'image'
              WHEN ext IN ('mp4', 'mov', 'webm') THEN 'video' END $$;
CREATE FUNCTION villa.type(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE ext WHEN 'webm' THEN 'video/webm' ELSE drops.content_type(ext) END $$;
CREATE FUNCTION villa.sniff(ext text, head bytea) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN ext = 'webm' THEN CASE WHEN drops.at(head, 0, '1a45dfa3') THEN NULL ELSE 'it is not a WebM video' END
              ELSE drops.sniff(ext, head) END $$;

-- The player for a key, or NULL.
CREATE FUNCTION villa.player(k text) RETURNS verse.players LANGUAGE sql STABLE AS $$
  SELECT * FROM verse.players WHERE k ~ '^[A-Za-z0-9_-]{20,128}$' AND key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex') $$;
CREATE FUNCTION villa.player_name(p verse.players) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT left(coalesce((SELECT nullif(btrim(name), '') FROM verse.accounts WHERE id = p.account), 'Guest ' || upper(substr(p.id, 3, 4))), 40) $$;

-- What anyone may see of a pet (the tombstone and the memorial), and what its owner sees.
CREATE FUNCTION villa.public(p villa.pets) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('slug', p.slug, 'name', p.name, 'species', p.species, 'born', p.born, 'died', p.died, 'line', p.line,
    'story', p.story, 'traits', p.traits, 'achievements', p.achievements, 'avatar', p.avatar, 'plot', p.plot) $$;
CREATE FUNCTION villa.owned(p villa.pets) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT villa.public(p) || jsonb_build_object('id', p.id, 'status', p.status, 'choices', p.choices, 'agent', p.agent,
    'agent_note', p.agent_note,
    'files', coalesce((SELECT jsonb_agg(jsonb_build_object('id', f.id, 'name', f.name, 'kind', f.kind, 'bytes', f.bytes,
                         'status', f.status, 'note', f.note) ORDER BY f.created_ms)
                       FROM villa.files f WHERE f.pet = p.id AND f.status <> 'deleted'), '[]')) $$;
CREATE FUNCTION villa.notes_of(pet_ text, owner bool) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('n', n.id, 'name', n.name, 'text', n.text, 'hidden', n.hidden) ORDER BY n.created_ms), '[]')
  FROM villa.notes n WHERE n.pet = pet_ AND (owner OR NOT n.hidden) $$;

-- The avatar as kept: every number held to its range, a pattern by name, two colours.
CREATE FUNCTION villa.avatar_fix(a jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
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
  RETURN out;
END $$;

-- ---- the avatar agent (hc-pet-avatar-agent): Gemini reads the pet's text,
-- photos and videos from the bucket and answers the avatar and its reasons.
CREATE FUNCTION villa.agent_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT '{"type":"object","properties":{
    "species":{"type":"string","enum":["dog","cat","rabbit","fox","other"]},
    "size":{"type":"number"},"legs":{"type":"number"},"body":{"type":"number"},"girth":{"type":"number"},"head":{"type":"number"},
    "snout":{"type":"number"},"ears":{"type":"number"},"ear_size":{"type":"number"},"tail":{"type":"number"},"fluff":{"type":"number"},
    "pattern":{"type":"string","enum":["plain","patches","points","stripes","socks"]},
    "coat":{"type":"array","items":{"type":"number"}},"marks":{"type":"array","items":{"type":"number"}},
    "choices":{"type":"array","items":{"type":"object","properties":{"part":{"type":"string"},"why":{"type":"string"}},"required":["part","why"]}}},
    "required":["species","size","legs","body","girth","head","snout","ears","ear_size","tail","fluff","pattern","coat","marks","choices"]}'::jsonb $$;
CREATE FUNCTION villa.agent_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'You make a pet''s avatar for a gentle memorial garden. From what its owner wrote and the photos and video they shared, '
    || 'set the avatar''s build relative to a typical dog of medium size (1 is usual): size 0.4-1.8 (a chihuahua 0.45, a labrador 1.1, '
    || 'a great dane 1.6), legs 0.5-1.6, body 0.6-1.6 (a dachshund 1.5), girth 0.6-1.6, head 0.7-1.5, snout 0.4-1.6 (a pug 0.45, a '
    || 'greyhound 1.5), ears 0-1 (0 pricked up, 1 hanging down), ear_size 0.5-1.8, tail 0-1.8 (0 none), fluff 0.5-2 (the tail''s thickness). '
    || 'Coat and marks are linear RGB from 0 to 1; pattern is the marks'' layout. In choices, name each part you set and say in a short '
    || 'warm sentence what you saw that made you choose it, for its owner to read. Never guess a breed you cannot see; keep usual values then.' $$;
CREATE FUNCTION villa.agent_request(p villa.pets, bucket text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', villa.agent_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts',
      jsonb_build_array(jsonb_build_object('text', 'Name: ' || p.name || E'\nSpecies: ' || p.species || E'\nAbout them: ' || p.story
        || E'\nTraits: ' || p.traits))
      || coalesce((SELECT jsonb_agg(jsonb_build_object('fileData', jsonb_build_object('mimeType', f.content_type,
                     'fileUri', 'gs://' || bucket || '/' || f.object)) ORDER BY f.created_ms)
                   FROM (SELECT * FROM villa.files WHERE pet = p.id AND status = 'kept' ORDER BY created_ms LIMIT 8) f), '[]'))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', villa.agent_schema(),
      'temperature', 0.2, 'maxOutputTokens', 4096)) $$;

CREATE FUNCTION villa.agent_done(pet_ text, http int, answer text, took int) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE body jsonb; txt text; d jsonb; why text; p villa.pets;
BEGIN
  IF http = 200 THEN
    BEGIN
      body := answer::jsonb;
      SELECT string_agg(x->>'text', '' ORDER BY i) INTO txt
      FROM jsonb_array_elements(body#>'{candidates,0,content,parts}') WITH ORDINALITY AS z(x, i)
      WHERE coalesce((x->>'thought')::bool, false) = false AND x ? 'text';
      d := txt::jsonb;
    EXCEPTION WHEN others THEN d := NULL;
    END;
    why := CASE WHEN jsonb_typeof(d) = 'object' THEN NULL ELSE 'we could not read the answer: try again' END;
  ELSE
    why := 'we could not reach the agent: try again';
  END IF;
  UPDATE villa.pets SET agent = CASE WHEN why IS NULL THEN 'ready' ELSE 'failed' END, agent_note = coalesce(why, ''),
    avatar = CASE WHEN why IS NULL THEN villa.avatar_fix(d) ELSE avatar END,
    species = CASE WHEN why IS NULL AND d->>'species' IN ('dog', 'cat', 'rabbit', 'fox', 'other') THEN d->>'species' ELSE species END,
    choices = CASE WHEN why IS NULL AND jsonb_typeof(d->'choices') = 'array' THEN
      (SELECT coalesce(jsonb_agg(jsonb_build_object('part', villa.clean(c->'part', 20), 'why', villa.clean(c->'why', 200))), '[]')
       FROM (SELECT c FROM jsonb_array_elements(d->'choices') c LIMIT 16) z) ELSE choices END,
    updated_ms = verse.now_ms()
  WHERE id = pet_ RETURNING * INTO p;
  RETURN villa.r(CASE WHEN why IS NULL THEN 200 ELSE 502 END, villa.owned(p));
END $$;

-- A new slug: 8 of [a-z0-9] not yet taken.
CREATE FUNCTION villa.new_slug() RETURNS text LANGUAGE plpgsql VOLATILE AS $$
DECLARE s text;
BEGIN
  LOOP
    s := left(replace(gen_random_uuid()::text, '-', ''), 8);
    IF NOT EXISTS (SELECT 1 FROM villa.pets WHERE slug = s) THEN RETURN s; END IF;
  END LOOP;
END $$;

-- One request. parts: the path after /api/villa/; k: the player key; prefix: this environment's
-- folder in the bucket; bucket: its name. Besides status and body, a result may carry what villa.c
-- does next: open (an upload: object, type, bytes, file), scan (object, file), delete (objects),
-- request (the agent's).
CREATE FUNCTION villa.api(method text, parts text[], body jsonb, k text, prefix text, bucket text) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  me verse.players := villa.player(k);
  p villa.pets;
  f villa.files;
  n int;
  name_ text; ext_ text; kind_ text; bytes_ bigint; said text;
  free int;
  head0 text := parts[1];
  objs jsonb;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;

  -- Anyone: the garden's tombstones, and a memorial by its link.
  IF head0 = 'garden' AND method = 'GET' AND cardinality(parts) = 1 THEN
    RETURN villa.r(200, jsonb_build_object('pets', coalesce((SELECT jsonb_agg(jsonb_build_object('slug', slug, 'name', name, 'born', born,
      'died', died, 'line', line, 'plot', plot, 'avatar', avatar, 'mine', me.id IS NOT NULL AND player = me.id) ORDER BY plot)
      FROM villa.pets WHERE status = 'placed'), '[]')));
  END IF;
  IF head0 = 'memorial' AND cardinality(parts) BETWEEN 2 AND 3 THEN
    SELECT * INTO p FROM villa.pets WHERE slug = parts[2] AND status = 'placed';
    IF p.id IS NULL THEN RETURN villa.err(404, 'no memorial here'); END IF;
    IF cardinality(parts) = 2 AND method = 'GET' THEN
      RETURN villa.r(200, villa.public(p) || jsonb_build_object('mine', me.id IS NOT NULL AND p.player = me.id,
        'notes', villa.notes_of(p.id, me.id IS NOT NULL AND p.player = me.id)));
    END IF;
    IF parts[3] = 'notes' AND method = 'POST' THEN
      IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key to leave a note'); END IF;
      said := villa.clean(body->'text', 600);
      IF said = '' THEN RETURN villa.err(400, 'write something first'); END IF;
      IF villa.linky(said) THEN RETURN villa.err(400, 'notes can''t carry links'); END IF;
      SELECT count(*) INTO n FROM villa.notes WHERE player = me.id AND created_ms > verse.now_ms() - 60000;
      IF n >= 3 THEN RETURN villa.err(429, 'a little slower'); END IF;
      INSERT INTO villa.notes (pet, player, name, text, created_ms) VALUES (p.id, me.id, villa.player_name(me), said, verse.now_ms());
      RETURN villa.r(200, jsonb_build_object('notes', villa.notes_of(p.id, p.player = me.id)));
    END IF;
    RETURN villa.err(404, 'unknown');
  END IF;

  IF head0 <> 'pets' THEN RETURN villa.err(404, 'unknown'); END IF;
  IF me.id IS NULL THEN RETURN villa.err(403, 'send a player key (POST /api/playtest/player)'); END IF;

  IF cardinality(parts) = 1 THEN
    IF method = 'GET' THEN
      RETURN villa.r(200, jsonb_build_object('pets', coalesce((SELECT jsonb_agg(villa.owned(x) ORDER BY x.created_ms)
        FROM villa.pets x WHERE x.player = me.id AND x.status <> 'removed'), '[]')));
    END IF;
    SELECT count(*) INTO n FROM villa.pets WHERE player = me.id AND status <> 'removed';
    IF n >= 6 THEN RETURN villa.err(409, 'six pets is the most for now'); END IF;
    INSERT INTO villa.pets (id, slug, player, name, species, created_ms, updated_ms, avatar)
    VALUES ('q-' || drops.hex16(), villa.new_slug(), me.id, villa.clean(body->'name', 40),
            CASE WHEN body->>'species' IN ('dog', 'cat', 'rabbit', 'fox', 'other') THEN body->>'species' ELSE 'dog' END,
            verse.now_ms(), verse.now_ms(), villa.avatar_fix('{}'))
    RETURNING * INTO p;
    RETURN villa.r(200, villa.owned(p));
  END IF;

  SELECT * INTO p FROM villa.pets WHERE id = parts[2] AND player = me.id AND status <> 'removed' FOR UPDATE;
  IF p.id IS NULL THEN RETURN villa.err(404, 'not found'); END IF;

  IF cardinality(parts) = 2 THEN
    IF method = 'GET' THEN RETURN villa.r(200, villa.owned(p) || jsonb_build_object('notes', villa.notes_of(p.id, true))); END IF;
    -- an edit: any of the text fields
    UPDATE villa.pets SET
      name = CASE WHEN body ? 'name' THEN villa.clean(body->'name', 40) ELSE name END,
      species = CASE WHEN body->>'species' IN ('dog', 'cat', 'rabbit', 'fox', 'other') THEN body->>'species' ELSE species END,
      born = CASE WHEN body ? 'born' THEN villa.clean(body->'born', 24) ELSE born END,
      died = CASE WHEN body ? 'died' THEN villa.clean(body->'died', 24) ELSE died END,
      line = CASE WHEN body ? 'line' THEN villa.clean(body->'line', 120) ELSE line END,
      story = CASE WHEN body ? 'story' THEN villa.clean(body->'story', 4000) ELSE story END,
      traits = CASE WHEN body ? 'traits' THEN villa.clean(body->'traits', 600) ELSE traits END,
      achievements = CASE WHEN body ? 'achievements' THEN villa.clean(body->'achievements', 600) ELSE achievements END,
      avatar = CASE WHEN jsonb_typeof(body->'avatar') = 'object' THEN villa.avatar_fix(avatar || body->'avatar') ELSE avatar END,
      updated_ms = verse.now_ms()
    WHERE id = p.id RETURNING * INTO p;
    RETURN villa.r(200, villa.owned(p));
  END IF;
  IF method <> 'POST' THEN RETURN villa.err(405, 'POST'); END IF;

  IF parts[3] = 'files' AND cardinality(parts) = 3 THEN
    SELECT count(*) INTO n FROM villa.files WHERE pet = p.id AND status IN ('waiting', 'uploading', 'kept');
    IF n >= 12 THEN RETURN villa.err(409, 'twelve files is the most for one pet'); END IF;
    name_ := villa.clean(body->'name', 120);
    ext_ := drops.ext(name_); kind_ := villa.kind(ext_);
    IF kind_ IS NULL THEN RETURN villa.err(415, 'photos (jpg, png, webp, gif, heic) or short videos (mp4, mov, webm) only'); END IF;
    bytes_ := CASE WHEN jsonb_typeof(body->'bytes') = 'number' THEN (body->>'bytes')::numeric::bigint ELSE 0 END;
    IF bytes_ <= 0 THEN RETURN villa.err(400, 'it is empty'); END IF;
    IF bytes_ > (CASE kind_ WHEN 'image' THEN 16777216 ELSE 67108864 END) THEN
      RETURN villa.err(413, CASE kind_ WHEN 'image' THEN 'a photo can be at most 16 MB' ELSE 'a video can be at most 64 MB' END);
    END IF;
    INSERT INTO villa.files (id, pet, name, ext, kind, bytes, content_type, object, created_ms)
    VALUES ('g-' || drops.hex16(), p.id, name_, ext_, kind_, bytes_, villa.type(ext_), '', verse.now_ms()) RETURNING * INTO f;
    UPDATE villa.files SET object = prefix || '/villa/' || p.id || '/' || f.id || '.' || ext_, status = 'uploading'
    WHERE id = f.id RETURNING * INTO f;
    RETURN villa.r(200, jsonb_build_object('file', f.id)) || jsonb_build_object('open', jsonb_build_object('object', f.object,
      'type', f.content_type, 'bytes', f.bytes, 'file', f.id));
  END IF;
  IF parts[3] = 'files' AND cardinality(parts) = 5 THEN
    SELECT * INTO f FROM villa.files WHERE id = parts[4] AND pet = p.id AND status <> 'deleted';
    IF f.id IS NULL THEN RETURN villa.err(404, 'not found'); END IF;
    IF parts[5] = 'done' THEN
      RETURN villa.r(200, '{}') || jsonb_build_object('scan', jsonb_build_object('object', f.object, 'file', f.id));
    END IF;
    IF parts[5] = 'delete' THEN
      UPDATE villa.files SET status = 'deleted' WHERE id = f.id;
      RETURN villa.r(200, villa.owned(p)) || jsonb_build_object('delete', jsonb_build_array(f.object));
    END IF;
    RETURN villa.err(404, 'unknown');
  END IF;
  IF parts[3] = 'agent' THEN
    IF p.agent = 'reading' AND p.agent_ms > verse.now_ms() - 180000 THEN RETURN villa.err(409, 'already reading'); END IF;
    IF p.story = '' AND p.traits = '' AND NOT EXISTS (SELECT 1 FROM villa.files WHERE pet = p.id AND status = 'kept') THEN
      RETURN villa.err(409, 'tell us about them, or add a photo, first');
    END IF;
    UPDATE villa.pets SET agent = 'reading', agent_note = '', agent_ms = verse.now_ms() WHERE id = p.id;
    RETURN villa.r(200, '{}') || jsonb_build_object('request', villa.agent_request(p, bucket), 'pet', p.id);
  END IF;
  IF parts[3] = 'place' THEN   -- PLAY: the tombstone takes a plot
    IF p.name = '' THEN RETURN villa.err(409, 'give them a name first'); END IF;
    IF p.plot IS NULL THEN
      PERFORM pg_advisory_xact_lock(7731029);
      -- the plots in the garden's order, skipping the four old stones (villa.coax OLD_STONES)
      SELECT g INTO free FROM generate_series(0, 63) g
      WHERE g NOT IN (0, 7, 59, 60) AND NOT EXISTS (SELECT 1 FROM villa.pets WHERE plot = g)
      ORDER BY (CASE WHEN jsonb_typeof(body->'near') = 'number' THEN abs(g - (body->>'near')::int) ELSE 0 END), g LIMIT 1;
      IF free IS NULL THEN RETURN villa.err(409, 'the garden is full for now'); END IF;
    ELSE free := p.plot;
    END IF;
    UPDATE villa.pets SET plot = free, status = 'placed', updated_ms = verse.now_ms() WHERE id = p.id RETURNING * INTO p;
    RETURN villa.r(200, villa.owned(p));
  END IF;
  IF parts[3] = 'remove' THEN   -- the pet, its tombstone and its files, gone
    SELECT coalesce(jsonb_agg(object), '[]') INTO objs FROM villa.files WHERE pet = p.id AND status IN ('uploading', 'kept');
    UPDATE villa.files SET status = 'deleted' WHERE pet = p.id;
    UPDATE villa.pets SET status = 'removed', plot = NULL, story = '', traits = '', achievements = '', updated_ms = verse.now_ms() WHERE id = p.id;
    DELETE FROM villa.notes WHERE pet = p.id;
    RETURN villa.r(200, '{"removed":true}') || jsonb_build_object('delete', objs);
  END IF;
  IF parts[3] = 'notes' AND cardinality(parts) = 5 AND parts[5] IN ('hide', 'show') AND parts[4] ~ '^[0-9]{1,18}$' THEN
    UPDATE villa.notes SET hidden = parts[5] = 'hide' WHERE id = parts[4]::bigint AND pet = p.id;
    RETURN villa.r(200, jsonb_build_object('notes', villa.notes_of(p.id, true)));
  END IF;
  RETURN villa.err(404, 'unknown');
END $$;

-- The scan's answer (villa.c read the object's size and first 4 KiB): kept, or refused and deleted.
CREATE FUNCTION villa.scanned(file_ text, size bigint, head bytea) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE f villa.files; why text; p villa.pets;
BEGIN
  SELECT * INTO f FROM villa.files WHERE id = file_ FOR UPDATE;
  IF f.id IS NULL OR f.status = 'deleted' THEN RETURN villa.err(404, 'not found'); END IF;
  why := CASE WHEN size < 0 THEN 'it did not arrive' WHEN size <> f.bytes THEN 'it arrived cut short' ELSE villa.sniff(f.ext, head) END;
  UPDATE villa.files SET status = CASE WHEN why IS NULL THEN 'kept' ELSE 'refused' END, note = coalesce(why, '') WHERE id = f.id;
  SELECT * INTO p FROM villa.pets WHERE id = f.pet;
  RETURN villa.r(200, villa.owned(p)) || CASE WHEN why IS NOT NULL AND size >= 0 THEN jsonb_build_object('delete', jsonb_build_array(f.object)) ELSE '{}' END;
END $$;
