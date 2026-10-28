-- An agent reads a drop and drafts the project (rc-read-drop, rc-engine-links;
-- docs/projects/new-project.md Part 2): the card, the design document in the
-- owner's words with its open questions, the roadmap as milestones and tasks
-- with acceptance checks, the asset inventory with licences, and the engine
-- upgrades the design needs, linked to the Verse engine's tasks. The model is
-- Google's first (Gemini on Vertex AI, drops.c); it reads the dropped files
-- straight from the bucket. Nothing here is public: a draft is its owner's
-- until rc-review's PUBLISH.
CREATE TABLE drops.drafts (
  drop_id text PRIMARY KEY REFERENCES drops.drops (id),
  status text NOT NULL CHECK (status IN ('reading', 'ready', 'failed')),
  draft jsonb,
  note text NOT NULL DEFAULT '',
  model text,
  started_ms bigint NOT NULL,
  done_ms bigint,
  took_ms int
);

-- What the Verse engine has today, and its planned upgrades (the verse-engine
-- initiative, apps/openagents-web/projects/verse/work/engine.json), so a
-- draft reuses a task before it proposes a new one.
CREATE TABLE drops.engine_tasks (
  slug text PRIMARY KEY,
  title text NOT NULL,
  what text NOT NULL
);
INSERT INTO drops.engine_tasks (slug, title, what) VALUES
  ('verse-moonlight', 'Night lighting: moonlight as the key light, soft shadows', 'A night lighting model for realms: one moonlight key light, dim ambient, soft shadows, and tone mapping that keeps dark scenes readable.'),
  ('verse-outlines', 'Stylized outlines for shapes and paths', 'An outline pass (depth and normal edges) with per-material strength, for stylized low-poly realms.'),
  ('verse-glow', 'Glowing materials and bloom', 'Emissive materials (glowing white pets, glowing flowers) with a bloom pass that stays cheap on phones.'),
  ('verse-particles', 'A GPU particle system', 'Particles for trails, glows and drifting motes: emitters attached to characters and objects, bounded counts, drawn in one pass.'),
  ('verse-soft-silhouette', 'Soft-edged silhouette characters', 'A character material that renders as a shadow with fuzzy edges (an alpha fringe or dither), lit by nothing but its rim.'),
  ('verse-quadruped', 'Four-legged characters: rig, gaits and customizable shape', 'A quadruped rig with walk, trot, run, sit, lie, dig and pick-up motions, and shape parameters (size, proportions, ears, tail, coat colours and pattern) that an agent or a slider can set.'),
  ('verse-authored-motions', 'Motions and emotes as data an agent can write', 'Emotes as small keyframe documents (poses over time on the rig) that an agent can author from a description and the engine plays, blends and checks for bounds.'),
  ('verse-cinematics', 'Camera cinematics: authored flights, skippable', 'Camera paths with timing and easing, scene changes behind them, skippable, ending with control handed back.'),
  ('verse-terrain-paths', 'Terrain and walkable garden paths', 'A heightfield ground for open fields, path pieces that mark walkable routes, and navigation for followers over them.'),
  ('verse-orbit-camera', 'A free-orbit third-person camera', 'An orbiting over-the-shoulder camera (the Breath of the Wild view) with collision against scenery, on mouse, keys and touch; builds on player.coax.'),
  ('verse-throw-physics', 'Thrown objects and pick-up', 'Simple ballistic objects that bounce and roll, and a pick-up and carry behaviour for characters.'),
  ('verse-closeup-arrange', 'A close-up arranging view', 'A mode that frames one object up close and lets you point and click to place small items on it (flowers in a vase), then returns to the world.');
CREATE FUNCTION drops.engine_has() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT
  'The Verse engine (Coax, WebGPU in the browser and Metal on iOS) already has: realms on a galaxy map, a walking '
  'third-person player with collision, a follow camera, skinned characters with clip blending (humanoid robots), '
  'a Haunted House quest with an editor for objects, characters and zones, character chat (typed questions to '
  'NPCs answered by a model), presence and a chat box for other players in a room, saves, published worlds, a '
  'player''s own realm, and a HUD with a hotbar.' $$;

-- Gemini reads these from the bucket (fileData); other kinds are listed by name only.
CREATE FUNCTION drops.readable_type(ext text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE
    WHEN ext IN ('txt', 'md', 'markdown', 'csv', 'json', 'svg', 'gltf', 'obj', 'mtl') THEN 'text/plain'
    WHEN ext = 'pdf' THEN 'application/pdf'
    WHEN ext = 'png' THEN 'image/png' WHEN ext IN ('jpg', 'jpeg') THEN 'image/jpeg' WHEN ext = 'webp' THEN 'image/webp'
    WHEN ext = 'heic' THEN 'image/heic'
    WHEN ext = 'mp3' THEN 'audio/mp3' WHEN ext = 'wav' THEN 'audio/wav' WHEN ext = 'aac' THEN 'audio/aac'
    WHEN ext = 'ogg' THEN 'audio/ogg' WHEN ext = 'flac' THEN 'audio/flac' WHEN ext = 'm4a' THEN 'audio/mp4'
    WHEN ext = 'mp4' THEN 'video/mp4' WHEN ext = 'mov' THEN 'video/quicktime'
  END $$;

CREATE FUNCTION drops.draft_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT '{
  "type": "OBJECT",
  "required": ["card", "mechanics", "design", "roadmap", "assets", "to_make", "engine_needs"],
  "properties": {
    "card": {"type": "OBJECT", "required": ["name", "slug", "line", "owner", "cover", "art_direction"], "properties": {
      "name": {"type": "STRING"}, "slug": {"type": "STRING"}, "line": {"type": "STRING"}, "owner": {"type": "STRING"},
      "cover": {"type": "STRING", "description": "the dropped file to use as the cover image, by its name, or empty"},
      "art_direction": {"type": "STRING"}}},
    "design": {"type": "OBJECT", "required": ["markdown", "open_questions"], "properties": {
      "markdown": {"type": "STRING"}, "open_questions": {"type": "ARRAY", "items": {"type": "STRING"}}}},
    "roadmap": {"type": "OBJECT", "required": ["initiative", "milestones"], "properties": {
      "initiative": {"type": "OBJECT", "required": ["slug", "title", "body"], "properties": {
        "slug": {"type": "STRING"}, "title": {"type": "STRING"}, "body": {"type": "STRING"}}},
      "milestones": {"type": "ARRAY", "items": {"type": "OBJECT", "required": ["slug", "title", "body", "tasks"], "properties": {
        "slug": {"type": "STRING"}, "title": {"type": "STRING"}, "body": {"type": "STRING"},
        "tasks": {"type": "ARRAY", "items": {"type": "OBJECT", "required": ["slug", "title", "body", "size", "acceptance", "mechanics", "needs"], "properties": {
          "slug": {"type": "STRING"}, "title": {"type": "STRING"}, "body": {"type": "STRING"},
          "size": {"type": "STRING", "enum": ["S", "M", "L"]},
          "acceptance": {"type": "ARRAY", "items": {"type": "STRING"}},
          "mechanics": {"type": "ARRAY", "items": {"type": "STRING"}, "description": "the design mechanics this task builds, in the owner''s words"},
          "needs": {"type": "ARRAY", "items": {"type": "STRING"}, "description": "engine needs this task waits on, as verse:<slug>"}}}}}}}}},
    "mechanics": {"type": "ARRAY", "items": {"type": "STRING"}, "description": "every mechanic the drop describes, one each, in the owner''s words"},
    "assets": {"type": "ARRAY", "items": {"type": "OBJECT", "required": ["file", "kind", "licence", "use"], "properties": {
      "file": {"type": "STRING"}, "kind": {"type": "STRING"}, "licence": {"type": "STRING"}, "use": {"type": "STRING"}}}},
    "to_make": {"type": "ARRAY", "items": {"type": "STRING"}},
    "engine_needs": {"type": "ARRAY", "items": {"type": "OBJECT", "required": ["slug", "title", "why", "reuses"], "properties": {
      "slug": {"type": "STRING"}, "title": {"type": "STRING"}, "why": {"type": "STRING"},
      "reuses": {"type": "BOOLEAN", "description": "true when slug is an existing Verse engine task"},
      "body": {"type": "STRING"}, "acceptance": {"type": "ARRAY", "items": {"type": "STRING"}}}}}
  }
}'::jsonb $$;

CREATE FUNCTION drops.draft_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT
'You set up a new project on openagents.com from what its owner dropped: design notes, voice memos, concept art, '
'models, textures, sounds and documents. People and agents will build the project from what you write, so it must '
'be complete and faithful.

Write:
- card: the project''s name, a slug (lowercase words joined by dashes), one line about it, the owner as the drop '
'names them (else empty), the dropped image that best serves as the cover (its file name, else empty), and the art '
'direction in a few sentences.
- mechanics: every mechanic, feature, rule and look the drop describes, one item each, in the owner''s own words. '
'Miss none: a reader should find every idea from the drop here.
- design.markdown: the design document, in the owner''s words and order, as Markdown. Keep their phrasing; add '
'nothing they did not say. design.open_questions: what the drop leaves unclear or undecided, as questions to the owner.
- roadmap: one initiative and its milestones, in the order to build them, each with tasks. Every mechanic is built '
'by at least one task, and each task lists the mechanics it builds. Each task has a short body, a size (S, M or L) '
'and acceptance checks a reviewer could run: concrete steps and what they should see ("Open the realm on a phone, '
'press play: you stand in the cemetery at night, lit by moonlight"), never "works well".
- assets: each dropped file, its kind, its licence ("unknown: ask the owner" unless the drop says), and its use.
- to_make: the assets the design needs that the drop does not have yet.
- engine_needs: what the design needs from the Verse engine that it does not have today. Reuse an existing engine '
'task when it covers the need (reuses true, its slug); otherwise propose a new one (reuses false, a new verse- slug, '
'a body and acceptance checks). The tasks that wait on a need list it in needs as verse:<slug>.

' || drops.engine_has() || '

The Verse engine''s planned upgrades (slug: title. what):
' || (SELECT string_agg(slug || ': ' || title || '. ' || what, E'\n' ORDER BY slug) FROM drops.engine_tasks) $$;

-- The model request for a drop's scanned files, read from bucket.
CREATE FUNCTION drops.draft_request(drop_ text, bucket text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', drops.draft_prompt()))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts',
      jsonb_build_array(jsonb_build_object('text', 'The drop: ' || (SELECT count(*) FROM drops.files WHERE drop_id = drop_ AND status = 'scanned')
        || ' files. Each readable one follows its name. Files listed without content are kept as assets.'))
      || coalesce((SELECT jsonb_agg(p ORDER BY at, k) FROM (
           SELECT f.created_ms AS at, 0 AS k, jsonb_build_object('text', 'File: ' || f.name || ' (' || f.kind || ', ' || f.bytes || ' bytes)'
             || CASE WHEN drops.readable_type(f.ext) IS NULL THEN ', not readable here' ELSE '' END) AS p
           FROM drops.files f WHERE f.drop_id = drop_ AND f.status = 'scanned'
           UNION ALL
           SELECT f.created_ms, 1, jsonb_build_object('fileData', jsonb_build_object('mimeType', drops.readable_type(f.ext),
             'fileUri', 'gs://' || bucket || '/' || f.object))
           FROM drops.files f WHERE f.drop_id = drop_ AND f.status = 'scanned' AND drops.readable_type(f.ext) IS NOT NULL) x), '[]'))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'responseSchema', drops.draft_schema(),
      'temperature', 0.3, 'maxOutputTokens', 32768)) $$;

-- Why a draft won't do, or NULL.
CREATE FUNCTION drops.draft_invalid(d jsonb) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE m jsonb; t jsonb;
BEGIN
  IF jsonb_typeof(d) <> 'object' THEN RETURN 'not an object'; END IF;
  IF coalesce(d#>>'{card,name}', '') = '' THEN RETURN 'no name'; END IF;
  IF jsonb_typeof(d#>'{roadmap,milestones}') <> 'array' OR jsonb_array_length(d#>'{roadmap,milestones}') = 0 THEN RETURN 'no milestones'; END IF;
  FOR m IN SELECT * FROM jsonb_array_elements(d#>'{roadmap,milestones}') LOOP
    IF jsonb_typeof(m->'tasks') <> 'array' OR jsonb_array_length(m->'tasks') = 0 THEN RETURN 'a milestone without tasks'; END IF;
    FOR t IN SELECT * FROM jsonb_array_elements(m->'tasks') LOOP
      IF coalesce(t->>'title', '') = '' THEN RETURN 'a task without a title'; END IF;
      IF jsonb_typeof(t->'acceptance') <> 'array' OR jsonb_array_length(t->'acceptance') = 0 THEN RETURN 'a task without acceptance checks'; END IF;
    END LOOP;
  END LOOP;
  RETURN NULL;
END $$;

-- A slug as the site takes it: lowercase words joined by dashes.
CREATE FUNCTION drops.slug(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(trim(BOTH '-' FROM regexp_replace(lower(coalesce(s, '')), '[^a-z0-9]+', '-', 'g')), 48) $$;

-- The owner asks for a draft: the request to send, unless one is being read.
CREATE FUNCTION drops.draft_start(pubkey text, drop_ text, bucket text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; cur drops.drafts;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  IF NOT EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status = 'scanned') THEN
    RETURN drops.r(409, '{"error":"nothing kept to read yet"}');
  END IF;
  SELECT * INTO cur FROM drops.drafts WHERE drop_id = d.id;
  IF cur.status = 'reading' AND cur.started_ms > drops.now_ms() - 300000 THEN RETURN drops.r(409, '{"error":"already reading"}'); END IF;
  INSERT INTO drops.drafts (drop_id, status, started_ms) VALUES (d.id, 'reading', drops.now_ms())
  ON CONFLICT (drop_id) DO UPDATE SET status = 'reading', note = '', started_ms = EXCLUDED.started_ms, done_ms = NULL;
  RETURN drops.r(200, '{}') || jsonb_build_object('request', drops.draft_request(d.id, bucket));
END $$;

CREATE FUNCTION drops.draft_json(x drops.drafts) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('status', x.status, 'note', x.note, 'model', x.model, 'took_ms', x.took_ms, 'draft', x.draft) $$;

-- The model's answer (http status, body): a draft kept, or why not.
CREATE FUNCTION drops.draft_done(drop_ text, http int, answer text, model_ text, took int) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE body jsonb; txt text; d jsonb; why text; x drops.drafts;
BEGIN
  IF http = 200 THEN
    BEGIN
      body := answer::jsonb;
      SELECT string_agg(p->>'text', '' ORDER BY i) INTO txt
      FROM jsonb_array_elements(body#>'{candidates,0,content,parts}') WITH ORDINALITY AS z(p, i)
      WHERE coalesce((p->>'thought')::bool, false) = false AND p ? 'text';
      d := txt::jsonb;
    EXCEPTION WHEN others THEN d := NULL;
    END;
    why := CASE WHEN d IS NULL THEN 'the model''s answer was not a draft' ELSE drops.draft_invalid(d) END;
  ELSE
    why := 'the model did not answer (' || http || ')';
  END IF;
  IF why IS NULL THEN
    d := jsonb_set(d, '{card,slug}', to_jsonb(drops.slug(coalesce(nullif(d#>>'{card,slug}', ''), d#>>'{card,name}'))));
  END IF;
  UPDATE drops.drafts SET status = CASE WHEN why IS NULL THEN 'ready' ELSE 'failed' END, draft = CASE WHEN why IS NULL THEN d ELSE draft END,
    note = coalesce(why, ''), model = model_, done_ms = drops.now_ms(), took_ms = took
  WHERE drop_id = drop_ RETURNING * INTO x;
  RETURN drops.r(CASE WHEN why IS NULL THEN 200 ELSE 502 END, drops.draft_json(x));
END $$;

CREATE FUNCTION drops.draft_get(pubkey text, drop_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN d.id IS NULL THEN drops.r(404, '{"error":"not found"}')
    WHEN x.drop_id IS NULL THEN drops.r(200, '{"status":"none"}')
    ELSE drops.r(200, drops.draft_json(x)) END
  FROM (SELECT 1) one LEFT JOIN drops.drops d ON d.id = drop_ AND d.owner = pubkey
  LEFT JOIN drops.drafts x ON x.drop_id = d.id $$;
