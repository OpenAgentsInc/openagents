-- Drops for a new project (rc-drop-zone; docs/projects/new-project.md Part 2):
-- the files someone drops on /projects (NEW PROJECT). They go to a private
-- bucket and stay private to the person who dropped them until a project made
-- from them is published. drops.c reads who is asking and does the bucket's
-- part; every rule is here.
--
-- Who may drop: a signed-in pubkey that is an admin (OA_ADMIN_PUBKEYS), a
-- member of a private project, or a Verse account (pt-accounts). Signing in is
-- a NIP-98 signature (a Nostr signer in the browser, or a tool), or the admin
-- sign-in link, whose session also opens /api/drops.
--
-- Limits: a file at most 64 MiB, a drop at most 40 files and 512 MiB, at most
-- 2 GiB and 20 open drops for one person. Only the kinds below, named by their
-- extension, and a file's first bytes must be what its extension says (the
-- scan); a program is refused whatever it's called.
CREATE SCHEMA IF NOT EXISTS drops;

CREATE TABLE drops.sessions (    -- a sign-in: only the token's SHA-256 is here
  token_sha256 text PRIMARY KEY,
  pubkey text NOT NULL,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL
);
CREATE TABLE drops.seen (        -- NIP-98 events, each accepted once
  id text PRIMARY KEY,
  ms bigint NOT NULL
);
CREATE TABLE drops.drops (
  id text PRIMARY KEY,           -- d-<16 hex>
  owner text NOT NULL,           -- the pubkey that dropped it
  status text NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'published', 'discarded')),
  created_ms bigint NOT NULL
);
CREATE INDEX drops_owner ON drops.drops (owner, created_ms);
CREATE TABLE drops.files (
  id text PRIMARY KEY,           -- f-<16 hex>
  drop_id text NOT NULL REFERENCES drops.drops (id),
  name text NOT NULL,            -- as dropped
  ext text NOT NULL,
  kind text NOT NULL,
  bytes bigint NOT NULL,
  content_type text NOT NULL,    -- ours, from the extension; never the browser's
  object text NOT NULL,          -- its name in the bucket
  session text,                  -- the upload's session URL, for its owner's browser only
  status text NOT NULL DEFAULT 'waiting' CHECK (status IN ('waiting', 'uploading', 'scanned', 'refused')),
  note text NOT NULL DEFAULT '',
  md5 text,
  created_ms bigint NOT NULL,
  done_ms bigint
);
CREATE INDEX files_drop ON drops.files (drop_id, created_ms);

CREATE FUNCTION drops.now_ms() RETURNS bigint LANGUAGE sql VOLATILE AS
  $$ SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint $$;
CREATE FUNCTION drops.r(status int, body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT jsonb_build_object('status', status, 'body', body) $$;
CREATE FUNCTION drops.hex16() RETURNS text LANGUAGE sql VOLATILE AS
  $$ SELECT left(replace(gen_random_uuid()::text, '-', ''), 16) $$;

CREATE FUNCTION drops.file_limit() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 67108864::bigint $$;
CREATE FUNCTION drops.drop_limit() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 536870912::bigint $$;
CREATE FUNCTION drops.files_limit() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 40 $$;
CREATE FUNCTION drops.owner_limit() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 2147483648::bigint $$;
CREATE FUNCTION drops.open_limit() RETURNS int LANGUAGE sql IMMUTABLE AS $$ SELECT 20 $$;

-- What kind of file an extension is, or NULL: not one we take.
CREATE FUNCTION drops.kind(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN ext IN ('txt', 'md', 'markdown', 'pdf', 'docx', 'odt', 'rtf', 'csv', 'json') THEN 'document'
    WHEN ext IN ('png', 'jpg', 'jpeg', 'gif', 'webp', 'avif', 'heic', 'svg') THEN 'image'
    WHEN ext IN ('mp3', 'wav', 'm4a', 'aac', 'ogg', 'opus', 'flac', 'webm') THEN 'audio'
    WHEN ext IN ('glb', 'gltf', 'bin', 'obj', 'mtl', 'fbx', 'stl', 'ply', 'blend', 'usdz') THEN 'model'
    WHEN ext IN ('ktx2', 'exr', 'hdr', 'tga') THEN 'texture'
    WHEN ext IN ('mp4', 'mov') THEN 'video'
    WHEN ext = 'zip' THEN 'archive'
  END $$;

CREATE FUNCTION drops.content_type(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE ext
    WHEN 'txt' THEN 'text/plain; charset=utf-8' WHEN 'md' THEN 'text/markdown; charset=utf-8'
    WHEN 'markdown' THEN 'text/markdown; charset=utf-8' WHEN 'csv' THEN 'text/csv; charset=utf-8'
    WHEN 'json' THEN 'application/json' WHEN 'pdf' THEN 'application/pdf' WHEN 'rtf' THEN 'application/rtf'
    WHEN 'docx' THEN 'application/vnd.openxmlformats-officedocument.wordprocessingml.document'
    WHEN 'odt' THEN 'application/vnd.oasis.opendocument.text'
    WHEN 'png' THEN 'image/png' WHEN 'jpg' THEN 'image/jpeg' WHEN 'jpeg' THEN 'image/jpeg' WHEN 'gif' THEN 'image/gif'
    WHEN 'webp' THEN 'image/webp' WHEN 'avif' THEN 'image/avif' WHEN 'heic' THEN 'image/heic' WHEN 'svg' THEN 'image/svg+xml'
    WHEN 'mp3' THEN 'audio/mpeg' WHEN 'wav' THEN 'audio/wav' WHEN 'm4a' THEN 'audio/mp4' WHEN 'aac' THEN 'audio/aac'
    WHEN 'ogg' THEN 'audio/ogg' WHEN 'opus' THEN 'audio/ogg' WHEN 'flac' THEN 'audio/flac' WHEN 'webm' THEN 'audio/webm'
    WHEN 'glb' THEN 'model/gltf-binary' WHEN 'gltf' THEN 'model/gltf+json' WHEN 'obj' THEN 'model/obj'
    WHEN 'mtl' THEN 'model/mtl' WHEN 'stl' THEN 'model/stl' WHEN 'usdz' THEN 'model/vnd.usdz+zip'
    WHEN 'mp4' THEN 'video/mp4' WHEN 'mov' THEN 'video/quicktime' WHEN 'zip' THEN 'application/zip'
    WHEN 'ktx2' THEN 'image/ktx2'
    ELSE 'application/octet-stream'
  END $$;

-- A file's extension, lowercase, from its name ('' without one).
CREATE FUNCTION drops.ext(name text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN name ~ '\.[A-Za-z0-9]{1,8}$' THEN lower(substring(name FROM '\.([A-Za-z0-9]{1,8})$')) ELSE '' END $$;

-- Whether b starts with the bytes p (hex) at offset at.
CREATE FUNCTION drops.at(b bytea, at int, p text) RETURNS bool LANGUAGE sql IMMUTABLE AS $$
  SELECT length(b) >= at + length(p) / 2 AND substring(b FROM at + 1 FOR length(p) / 2) = decode(p, 'hex') $$;

-- Text: no NUL byte and good UTF-8 (the head may end inside a character).
CREATE FUNCTION drops.texty(b bytea) RETURNS bool LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE n int := length(b); k int := 0;
BEGIN
  IF position('\x00'::bytea IN b) > 0 THEN RETURN false; END IF;
  WHILE k < 4 AND n - k > 0 LOOP   -- a cut character at the end is no fault
    BEGIN
      PERFORM convert_from(substring(b FROM 1 FOR n - k), 'UTF8');
      RETURN true;
    EXCEPTION WHEN others THEN k := k + 1;
    END;
  END LOOP;
  RETURN n = 0;
END $$;

-- The scan: why a file's first bytes (up to 4 KiB) aren't what its extension
-- says, or NULL when they are.
CREATE FUNCTION drops.sniff(ext text, head bytea) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE ftyp bool := drops.at(head, 4, '66747970');   -- "ftyp" at 4: MP4, MOV, M4A, AVIF, HEIC
BEGIN
  IF length(head) = 0 THEN RETURN 'it is empty'; END IF;
  IF drops.at(head, 0, '4d5a') OR drops.at(head, 0, '7f454c46') OR drops.at(head, 0, '2321')
     OR drops.at(head, 0, 'feedface') OR drops.at(head, 0, 'feedfacf') OR drops.at(head, 0, 'cefaedfe')
     OR drops.at(head, 0, 'cffaedfe') OR drops.at(head, 0, 'cafebabe') OR drops.at(head, 0, '0061736d') THEN
    RETURN 'it is a program';
  END IF;
  RETURN CASE
    WHEN ext IN ('txt', 'md', 'markdown', 'csv', 'json', 'obj', 'mtl', 'gltf') THEN
      CASE WHEN drops.texty(head) THEN NULL ELSE 'it is not text' END
    WHEN ext = 'svg' THEN
      CASE WHEN drops.texty(head) AND position('\x3c737667'::bytea IN head) > 0 THEN NULL ELSE 'it is not an SVG' END
    WHEN ext = 'rtf' THEN CASE WHEN drops.at(head, 0, '7b5c727466') THEN NULL ELSE 'it is not RTF' END
    WHEN ext = 'pdf' THEN CASE WHEN drops.at(head, 0, '255044462d') THEN NULL ELSE 'it is not a PDF' END
    WHEN ext IN ('docx', 'odt', 'zip', 'usdz') THEN CASE WHEN drops.at(head, 0, '504b0304') THEN NULL ELSE 'it is not a zip' END
    WHEN ext = 'png' THEN CASE WHEN drops.at(head, 0, '89504e470d0a1a0a') THEN NULL ELSE 'it is not a PNG' END
    WHEN ext IN ('jpg', 'jpeg') THEN CASE WHEN drops.at(head, 0, 'ffd8ff') THEN NULL ELSE 'it is not a JPEG' END
    WHEN ext = 'gif' THEN CASE WHEN drops.at(head, 0, '474946383761') OR drops.at(head, 0, '474946383961') THEN NULL ELSE 'it is not a GIF' END
    WHEN ext = 'webp' THEN CASE WHEN drops.at(head, 0, '52494646') AND drops.at(head, 8, '57454250') THEN NULL ELSE 'it is not a WebP' END
    WHEN ext IN ('avif', 'heic', 'mp4', 'm4a') THEN CASE WHEN ftyp THEN NULL ELSE 'it is not an MP4 family file' END
    WHEN ext = 'mov' THEN CASE WHEN ftyp OR drops.at(head, 4, '6d6f6f76') OR drops.at(head, 4, '77696465') OR drops.at(head, 4, '6d646174')
                               THEN NULL ELSE 'it is not a QuickTime movie' END
    WHEN ext = 'wav' THEN CASE WHEN drops.at(head, 0, '52494646') AND drops.at(head, 8, '57415645') THEN NULL ELSE 'it is not a WAV' END
    WHEN ext = 'mp3' THEN CASE WHEN drops.at(head, 0, '494433') OR (get_byte(head, 0) = 255 AND length(head) > 1 AND get_byte(head, 1) >= 224)
                               THEN NULL ELSE 'it is not an MP3' END
    WHEN ext = 'aac' THEN CASE WHEN ftyp OR drops.at(head, 0, 'fff1') OR drops.at(head, 0, 'fff9') THEN NULL ELSE 'it is not AAC' END
    WHEN ext IN ('ogg', 'opus') THEN CASE WHEN drops.at(head, 0, '4f676753') THEN NULL ELSE 'it is not Ogg' END
    WHEN ext = 'flac' THEN CASE WHEN drops.at(head, 0, '664c6143') THEN NULL ELSE 'it is not FLAC' END
    WHEN ext = 'webm' THEN CASE WHEN drops.at(head, 0, '1a45dfa3') THEN NULL ELSE 'it is not WebM' END
    WHEN ext = 'glb' THEN CASE WHEN drops.at(head, 0, '676c5446') THEN NULL ELSE 'it is not a binary glTF' END
    WHEN ext = 'fbx' THEN CASE WHEN drops.at(head, 0, '4b617964617261204642582042696e617279') OR drops.texty(head) THEN NULL ELSE 'it is not FBX' END
    WHEN ext = 'ply' THEN CASE WHEN drops.at(head, 0, '706c79') THEN NULL ELSE 'it is not PLY' END
    WHEN ext = 'blend' THEN CASE WHEN drops.at(head, 0, '424c454e444552') THEN NULL ELSE 'it is not a Blender file' END
    WHEN ext = 'ktx2' THEN CASE WHEN drops.at(head, 0, 'ab4b5458203230bb0d0a1a0a') THEN NULL ELSE 'it is not KTX2' END
    WHEN ext = 'exr' THEN CASE WHEN drops.at(head, 0, '762f3101') THEN NULL ELSE 'it is not OpenEXR' END
    WHEN ext = 'hdr' THEN CASE WHEN drops.at(head, 0, '233f') THEN NULL ELSE 'it is not Radiance HDR' END
    WHEN ext IN ('stl', 'bin', 'tga') THEN NULL   -- no magic of their own; a program was refused above
    ELSE 'we don''t take this kind of file'
  END;
END $$;

-- Whether pubkey may drop files.
CREATE FUNCTION drops.may(pubkey text, admin bool) RETURNS bool LANGUAGE sql STABLE AS $$
  SELECT pubkey ~ '^[0-9a-f]{64}$' AND (admin
    OR EXISTS (SELECT 1 FROM projects.docs d, jsonb_array_elements(d.doc->'members') m WHERE m->>'pubkey' = pubkey)
    OR EXISTS (SELECT 1 FROM verse.accounts a WHERE a.npub = pubkey)) $$;

-- Bytes an owner holds in open drops (files not refused).
CREATE FUNCTION drops.used(pubkey text) RETURNS bigint LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(f.bytes), 0)::bigint FROM drops.files f JOIN drops.drops d ON d.id = f.drop_id
  WHERE d.owner = pubkey AND d.status = 'open' AND f.status <> 'refused' $$;

CREATE FUNCTION drops.file_json(f drops.files) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('id', f.id, 'name', f.name, 'kind', f.kind, 'bytes', f.bytes, 'status', f.status, 'note', f.note) $$;

-- Who is signed in, and what they may still drop.
CREATE FUNCTION drops.me(pubkey text, admin bool) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT drops.r(200, jsonb_build_object('pubkey', pubkey, 'may', drops.may(pubkey, admin), 'used', drops.used(pubkey),
    'limits', jsonb_build_object('file', drops.file_limit(), 'drop', drops.drop_limit(), 'files', drops.files_limit(),
                                 'owner', drops.owner_limit()))) $$;

CREATE FUNCTION drops.new_drop(pubkey text, admin bool) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE id_ text := 'd-' || drops.hex16();
BEGIN
  IF NOT drops.may(pubkey, admin) THEN RETURN drops.r(403, '{"error":"this key may not drop files yet"}'); END IF;
  IF (SELECT count(*) FROM drops.drops WHERE owner = pubkey AND status = 'open') >= drops.open_limit() THEN
    RETURN drops.r(409, '{"error":"too many open drops"}');
  END IF;
  INSERT INTO drops.drops (id, owner, created_ms) VALUES (id_, pubkey, drops.now_ms());
  RETURN drops.r(200, jsonb_build_object('id', id_));
END $$;

-- An owner's drops, newest first. Anyone else's never show.
CREATE FUNCTION drops.list(pubkey text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT drops.r(200, jsonb_build_object('drops', coalesce(jsonb_agg(jsonb_build_object('id', d.id, 'status', d.status,
      'created', d.created_ms, 'files', (SELECT count(*) FROM drops.files f WHERE f.drop_id = d.id AND f.status <> 'refused'),
      'bytes', (SELECT coalesce(sum(bytes), 0) FROM drops.files f WHERE f.drop_id = d.id AND f.status <> 'refused'))
    ORDER BY d.created_ms DESC), '[]')))
  FROM drops.drops d WHERE d.owner = pubkey $$;

CREATE FUNCTION drops.get(pubkey text, drop_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN d.id IS NULL THEN drops.r(404, '{"error":"not found"}')
    ELSE drops.r(200, jsonb_build_object('id', d.id, 'status', d.status, 'files',
      (SELECT coalesce(jsonb_agg(drops.file_json(f) ORDER BY f.created_ms), '[]') FROM drops.files f WHERE f.drop_id = d.id))) END
  FROM (SELECT 1) one LEFT JOIN drops.drops d ON d.id = drop_ AND d.owner = pubkey $$;

-- A file the owner is about to upload: {name, bytes}. Admitted, it gets its
-- row and the object name drops.c opens an upload for (prefix: the
-- environment's folder in the bucket).
CREATE FUNCTION drops.admit(pubkey text, admin bool, drop_ text, req jsonb, prefix text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE
  d drops.drops;
  name_ text; ext_ text; kind_ text; bytes_ bigint; id_ text; obj text;
  in_drop int; drop_bytes bigint;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF NOT drops.may(pubkey, admin) THEN RETURN drops.r(403, '{"error":"this key may not drop files yet"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  name_ := req->>'name';
  IF jsonb_typeof(req->'name') <> 'string' OR length(name_) < 1 OR octet_length(name_) > 200 OR name_ ~ '[\x00-\x1f/\\]' THEN
    RETURN drops.r(400, '{"error":"a file needs a name"}');
  END IF;
  IF jsonb_typeof(req->'bytes') <> 'number' OR (req->>'bytes') !~ '^[0-9]{1,12}$' THEN RETURN drops.r(400, '{"error":"how big is it?"}'); END IF;
  bytes_ := (req->>'bytes')::bigint;
  ext_ := drops.ext(name_);
  kind_ := drops.kind(ext_);
  IF kind_ IS NULL THEN RETURN drops.r(415, jsonb_build_object('error', 'we don''t take ' || CASE WHEN ext_ = '' THEN 'files without an extension' ELSE '.' || ext_ || ' files' END)); END IF;
  IF bytes_ = 0 THEN RETURN drops.r(400, '{"error":"it is empty"}'); END IF;
  IF bytes_ > drops.file_limit() THEN RETURN drops.r(413, '{"error":"over 64 MB"}'); END IF;
  SELECT count(*), coalesce(sum(bytes), 0) INTO in_drop, drop_bytes FROM drops.files WHERE drop_id = d.id AND status <> 'refused';
  IF in_drop >= drops.files_limit() THEN RETURN drops.r(413, '{"error":"a drop holds 40 files"}'); END IF;
  IF drop_bytes + bytes_ > drops.drop_limit() THEN RETURN drops.r(413, '{"error":"a drop holds 512 MB"}'); END IF;
  IF drops.used(pubkey) + bytes_ > drops.owner_limit() THEN RETURN drops.r(413, '{"error":"you have 2 GB of drops already"}'); END IF;
  id_ := 'f-' || drops.hex16();
  obj := prefix || '/' || pubkey || '/' || d.id || '/' || id_ || '.' || ext_;
  INSERT INTO drops.files (id, drop_id, name, ext, kind, bytes, content_type, object, created_ms)
  VALUES (id_, d.id, name_, ext_, kind_, bytes_, drops.content_type(ext_), obj, drops.now_ms());
  RETURN drops.r(200, jsonb_build_object('file', id_)) || jsonb_build_object('object', obj, 'type', drops.content_type(ext_), 'bytes', bytes_);
END $$;

-- The upload is open (session: its URL) or couldn't be (session NULL).
CREATE FUNCTION drops.started(file_ text, session_ text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE f drops.files;
BEGIN
  IF session_ IS NULL OR session_ = '' THEN
    UPDATE drops.files SET status = 'refused', note = 'storage was unavailable; drop it again', done_ms = drops.now_ms() WHERE id = file_;
    RETURN drops.r(503, '{"error":"storage was unavailable; drop it again"}');
  END IF;
  UPDATE drops.files SET status = 'uploading', session = session_ WHERE id = file_ RETURNING * INTO f;
  RETURN drops.r(200, jsonb_build_object('file', drops.file_json(f), 'upload', session_));
END $$;

-- The owner says an upload finished: what drops.c needs to scan it.
CREATE FUNCTION drops.finishing(pubkey text, drop_ text, file_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN f.id IS NULL THEN drops.r(404, '{"error":"not found"}')
    WHEN f.status <> 'uploading' THEN drops.r(200, jsonb_build_object('file', drops.file_json(f))) || '{"settled":true}'
    ELSE drops.r(200, '{}') || jsonb_build_object('object', f.object) END
  FROM (SELECT 1) one
  LEFT JOIN drops.files f ON f.id = file_ AND f.drop_id = drop_
    AND EXISTS (SELECT 1 FROM drops.drops d WHERE d.id = drop_ AND d.owner = pubkey) $$;

-- The scan's verdict, from what the bucket holds: its size (-1: nothing
-- there yet), its MD5 and its first bytes. A refused file's object is deleted.
CREATE FUNCTION drops.scan(file_ text, size bigint, md5_ text, head bytea) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE f drops.files; why text;
BEGIN
  SELECT * INTO f FROM drops.files WHERE id = file_ FOR UPDATE;
  IF f.status <> 'uploading' THEN RETURN drops.r(200, jsonb_build_object('file', drops.file_json(f))); END IF;
  IF size < 0 THEN RETURN drops.r(409, '{"error":"the upload isn''t finished"}'); END IF;
  why := CASE WHEN size <> f.bytes THEN 'it arrived at a different size' ELSE drops.sniff(f.ext, head) END;
  UPDATE drops.files SET status = CASE WHEN why IS NULL THEN 'scanned' ELSE 'refused' END, note = coalesce(why, ''),
    md5 = md5_, session = NULL, done_ms = drops.now_ms() WHERE id = file_ RETURNING * INTO f;
  RETURN drops.r(200, jsonb_build_object('file', drops.file_json(f))) || jsonb_build_object('delete', why IS NOT NULL);
END $$;
