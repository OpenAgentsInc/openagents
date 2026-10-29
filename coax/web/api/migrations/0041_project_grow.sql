-- Later drops grow a project (rc-later-drops; docs/projects/new-project.md
-- Part 2, step 5). A member drops files on a project's page; the agent reads
-- them against the project's work as it is on the relay now and proposes what
-- they add: tasks under milestones the project has or new ones, design notes,
-- assets. The member reviews the proposals with the drop's review (0020) and
-- ACCEPTs them: drops.c signs them with the project's key (new issues, a
-- design note, the task order extended) and copies the files next to the page,
-- whose live fold shows them. DECLINE is DISCARD.
--
-- A project made from a drop has its key sealed here already (0038). A project
-- made by hand gives its key to the service: an adopt call signed by that key
-- (drops.c checks the key is the signer's, seals it under OA_PROJECT_KEK and
-- keeps only the sealed bytes, with the members who may drop on it).

ALTER TABLE drops.drops ADD COLUMN project text, ADD COLUMN project_pubkey text;

CREATE TABLE drops.homes (           -- hand-made projects whose key the service holds
  slug text NOT NULL,
  pubkey text NOT NULL,
  key_sealed text NOT NULL,          -- nonce || ciphertext || tag, hex; never the key itself
  name text NOT NULL,
  members text[] NOT NULL,           -- who may drop on it: the owner and maintainers
  adopted_ms bigint NOT NULL,
  PRIMARY KEY (slug, pubkey)
);

CREATE TABLE drops.grow_items (      -- what ACCEPT signs, in order (as drops.project_items for a new project)
  drop_id text NOT NULL REFERENCES drops.drops (id),
  seq int NOT NULL,
  item text NOT NULL,                -- a new item's slug, 'design', 'order'
  kind int NOT NULL,
  item_kind text NOT NULL DEFAULT '',   -- milestone, task, note; '' for the order
  parent text,                       -- a new milestone this hangs from (its item)
  parent_id text,                    -- or an issue the project has (its event id)
  title text NOT NULL DEFAULT '',
  body text NOT NULL DEFAULT '',     -- for the order: the order's tags as they were, a JSON array
  size text NOT NULL DEFAULT '',
  labels text[] NOT NULL DEFAULT '{}',
  event_id text,
  event text,
  PRIMARY KEY (drop_id, seq),
  UNIQUE (drop_id, item)
);

-- The projects that take drops: made from a drop (their dropper is the member) or adopted.
CREATE FUNCTION drops.home(slug_ text, pk text) RETURNS TABLE (slug text, pubkey text, key_sealed text, name text, members text[])
LANGUAGE sql STABLE AS $$
  SELECT p.slug, p.pubkey, p.key_sealed, p.name, ARRAY[p.owner] FROM drops.projects p
   WHERE p.slug = slug_ AND p.pubkey = pk AND p.key_sealed IS NOT NULL
  UNION ALL
  SELECT h.slug, h.pubkey, h.key_sealed, h.name, h.members FROM drops.homes h WHERE h.slug = slug_ AND h.pubkey = pk $$;

-- Adopt: signer signed the call with the project's own key (drops.c checked the key is theirs and sealed it).
-- req: {slug, name, members}.
CREATE FUNCTION drops.adopt(signer text, req jsonb, sealed text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE s text := req->>'slug'; m text[];
BEGIN
  IF s IS NULL OR s = '' OR s <> drops.slug(s) THEN RETURN drops.r(400, '{"error":"a slug: lowercase words joined by dashes"}'); END IF;
  IF coalesce(btrim(req->>'name'), '') = '' THEN RETURN drops.r(400, '{"error":"the project''s name"}'); END IF;
  IF jsonb_typeof(req->'members') <> 'array' THEN RETURN drops.r(400, '{"error":"members: their hex keys"}'); END IF;
  SELECT array_agg(DISTINCT x) INTO m FROM jsonb_array_elements_text(req->'members') x;
  IF m IS NULL OR EXISTS (SELECT 1 FROM unnest(m) x WHERE x !~ '^[0-9a-f]{64}$') THEN
    RETURN drops.r(400, '{"error":"members: their hex keys"}');
  END IF;
  IF sealed !~ '^[0-9a-f]+$' OR signer !~ '^[0-9a-f]{64}$' THEN RETURN drops.r(400, '{"error":"not sealed"}'); END IF;
  IF EXISTS (SELECT 1 FROM drops.projects WHERE slug = s AND pubkey = signer) THEN
    RETURN drops.r(409, '{"error":"this project''s key is here already"}');
  END IF;
  INSERT INTO drops.homes (slug, pubkey, key_sealed, name, members, adopted_ms)
  VALUES (s, signer, sealed, drops.one_line(req->>'name', 120), m, drops.now_ms())
  ON CONFLICT (slug, pubkey) DO UPDATE SET key_sealed = EXCLUDED.key_sealed, name = EXCLUDED.name, members = EXCLUDED.members,
    adopted_ms = EXCLUDED.adopted_ms;
  RETURN drops.r(200, jsonb_build_object('slug', s, 'pubkey', signer, 'members', array_length(m, 1)));
END $$;

-- A new drop: on a project (req {project, pubkey}) only its members and admins make one.
CREATE FUNCTION drops.new_drop(pubkey text, admin bool, req jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE id_ text := 'd-' || drops.hex16(); h record;
BEGIN
  IF coalesce(req->>'project', '') = '' THEN RETURN drops.new_drop(pubkey, admin); END IF;
  SELECT * INTO h FROM drops.home(req->>'project', coalesce(req->>'pubkey', '')) LIMIT 1;
  IF h.slug IS NULL THEN RETURN drops.r(404, '{"error":"this project doesn''t take drops yet"}'); END IF;
  IF NOT (admin OR pubkey = ANY (h.members)) THEN RETURN drops.r(403, '{"error":"only the project''s members add to it"}'); END IF;
  IF (SELECT count(*) FROM drops.drops WHERE owner = pubkey AND status = 'open') >= drops.open_limit() THEN
    RETURN drops.r(409, '{"error":"too many open drops"}');
  END IF;
  INSERT INTO drops.drops (id, owner, created_ms, project, project_pubkey) VALUES (id_, pubkey, drops.now_ms(), h.slug, h.pubkey);
  RETURN drops.r(200, jsonb_build_object('id', id_, 'project', h.slug, 'name', h.name));
END $$;

CREATE OR REPLACE FUNCTION drops.list(pubkey text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT drops.r(200, jsonb_build_object('drops', coalesce(jsonb_agg(jsonb_build_object('id', d.id, 'status', d.status,
      'created', d.created_ms, 'files', (SELECT count(*) FROM drops.files f WHERE f.drop_id = d.id AND f.status <> 'refused'),
      'bytes', (SELECT coalesce(sum(bytes), 0) FROM drops.files f WHERE f.drop_id = d.id AND f.status <> 'refused'),
      'project', d.project)
    ORDER BY d.created_ms DESC), '[]')))
  FROM drops.drops d WHERE d.owner = pubkey $$;

-- The project a drop grows, for drops.c: {slug, pubkey, sealed}, or NULL for a new project's drop.
CREATE FUNCTION drops.grow_target(pubkey text, drop_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('slug', h.slug, 'pubkey', h.pubkey, 'sealed', h.key_sealed)
  FROM drops.drops d, drops.home(d.project, d.project_pubkey) h
  WHERE d.id = drop_ AND d.owner = grow_target.pubkey AND d.project IS NOT NULL LIMIT 1 $$;

-- An event's first tag named n: its value.
CREATE FUNCTION drops.ev_tag(e jsonb, n text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT t->>1 FROM jsonb_array_elements(coalesce(e->'tags', '[]')) WITH ORDINALITY AS z(t, i) WHERE t->>0 = n ORDER BY i LIMIT 1 $$;

-- The project's work as the relay has it (evs: its key's events): every issue {id, slug, kind, title, status,
-- parent}, the initiative's id, an issue's 'a' tags to copy, and the latest order's tags.
CREATE FUNCTION drops.grow_work(evs jsonb, pk text, slug_ text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  WITH e AS (SELECT DISTINCT ON (x->>'id') x FROM jsonb_array_elements(coalesce(evs, '[]')) x
             WHERE jsonb_typeof(x) = 'object' AND x->>'pubkey' = pk ORDER BY x->>'id'),
  issues AS (SELECT x, CASE WHEN EXISTS (SELECT 1 FROM jsonb_array_elements(x->'tags') t WHERE t->>0 = 't' AND t->>1 = 'initiative') THEN 'initiative'
                            WHEN EXISTS (SELECT 1 FROM jsonb_array_elements(x->'tags') t WHERE t->>0 = 't' AND t->>1 = 'milestone') THEN 'milestone'
                            ELSE 'task' END AS k FROM e WHERE (x->>'kind')::int = 1621),
  st AS (SELECT DISTINCT ON (drops.ev_tag(x, 'e')) drops.ev_tag(x, 'e') AS of, (x->>'kind')::int AS kind FROM e
         WHERE (x->>'kind')::int BETWEEN 1630 AND 1633 ORDER BY drops.ev_tag(x, 'e'), (x->>'created_at')::bigint DESC),
  ord AS (SELECT x FROM e WHERE (x->>'kind')::int = 30623 AND drops.ev_tag(x, 'd') = slug_ ORDER BY (x->>'created_at')::bigint DESC LIMIT 1)
  SELECT jsonb_build_object(
    'items', coalesce((SELECT jsonb_agg(jsonb_build_object('id', i.x->>'id', 'slug', coalesce(drops.ev_tag(i.x, 'slug'), ''), 'kind', i.k,
               'title', coalesce(drops.ev_tag(i.x, 'subject'), ''), 'parent', drops.ev_tag(i.x, 'parent'),
               'status', CASE (SELECT kind FROM st WHERE of = i.x->>'id') WHEN 1631 THEN 'resolved' WHEN 1632 THEN 'closed' WHEN 1633 THEN 'draft' ELSE 'open' END)
               ORDER BY (i.x->>'created_at')::bigint, i.x->>'id') FROM issues i), '[]'),
    'initiative', (SELECT i.x->>'id' FROM issues i WHERE i.k = 'initiative' ORDER BY (i.x->>'created_at')::bigint LIMIT 1),
    'a', coalesce((SELECT jsonb_agg(t) FROM (SELECT i.x FROM issues i ORDER BY (i.x->>'created_at')::bigint LIMIT 1) one,
                   jsonb_array_elements(one.x->'tags') t WHERE t->>0 = 'a'), '[]'),
    'order', (SELECT x->'tags' FROM ord)) $$;

-- The project and its work as the model reads it.
CREATE FUNCTION drops.grow_context(w jsonb, name_ text, slug_ text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT 'The project: ' || name_ || ' (' || slug_ || '). Its work now, an item a line (kind slug: title [status]), '
    || 'each task under its milestone:' || E'\n'
    || coalesce((SELECT string_agg(l, E'\n' ORDER BY o) FROM (
         SELECT m.ord * 10000 AS o, m.i->>'kind' || ' ' || (m.i->>'slug') || ': ' || (m.i->>'title') || ' [' || (m.i->>'status') || ']' AS l
         FROM jsonb_array_elements(w->'items') WITH ORDINALITY AS m(i, ord) WHERE m.i->>'kind' <> 'task'
         UNION ALL
         SELECT coalesce((SELECT p.ord FROM jsonb_array_elements(w->'items') WITH ORDINALITY AS p(i, ord) WHERE p.i->>'id' = t.i->>'parent'), 0) * 10000 + t.ord,
                '  task ' || (t.i->>'slug') || ': ' || (t.i->>'title') || ' [' || (t.i->>'status') || ']'
         FROM jsonb_array_elements(w->'items') WITH ORDINALITY AS t(i, ord) WHERE t.i->>'kind' = 'task') z), '(nothing yet)') $$;

CREATE FUNCTION drops.grow_schema() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_set(jsonb_set(jsonb_set(drops.draft_schema(), '{required}', '["card", "mechanics", "design", "roadmap", "assets", "to_make", "engine_needs"]'),
    '{properties,card}', '{"type": "OBJECT", "required": ["line"], "properties": {"line": {"type": "STRING", "description": "what this drop adds, in a sentence or two"}}}'),
    '{properties,roadmap}', jsonb_build_object('type', 'OBJECT', 'required', '["milestones"]'::jsonb, 'properties', jsonb_build_object('milestones',
      jsonb_build_object('type', 'ARRAY', 'items', jsonb_build_object('type', 'OBJECT', 'required', '["slug", "existing", "title", "body", "tasks"]'::jsonb,
        'properties', jsonb_build_object(
          'slug', '{"type": "STRING"}'::jsonb,
          'existing', '{"type": "BOOLEAN", "description": "true when slug is a milestone the project has: only its new tasks follow"}'::jsonb,
          'title', '{"type": "STRING"}'::jsonb, 'body', '{"type": "STRING"}'::jsonb,
          'tasks', drops.draft_schema()#>'{properties,roadmap,properties,milestones,items,properties,tasks}')))))) $$;

CREATE FUNCTION drops.grow_prompt() RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT
'You grow a project on openagents.com from what one of its members dropped now: new design notes, voice memos, '
'concept art, models, textures, sounds and documents. The project and its work as they are now follow. Propose only '
'what this drop adds or changes; never repeat work the project has, and keep the owner''s words.

Write:
- card.line: what this drop adds, in a sentence or two.
- mechanics: every new mechanic, feature, rule and look the drop describes, one item each, in the owner''s words.
- design.markdown: the drop''s new design notes, in the owner''s words and order, as Markdown; empty when it has '
'none. design.open_questions: what the drop leaves unclear, as questions to the owner.
- roadmap.milestones: where the new work goes. To add tasks to a milestone the project has, give its slug with '
'existing true and only the new tasks; a new milestone has existing false, a new slug, a title, a body and its '
'tasks. Each task has a new slug, a short body, a size (S, M or L), acceptance checks a reviewer could run (concrete '
'steps and what they should see, never "works well") and the mechanics it builds. No milestones when the drop adds '
'no work.
- assets: each dropped file, its kind, its licence ("unknown: ask the owner" unless the drop says), and its use.
- to_make: the assets the new work needs that the project does not have yet.
- engine_needs: what the new work needs from the Verse engine that it does not have today, reusing an existing '
'engine task when it covers the need (reuses true, its slug), else a new verse- slug with a body and acceptance '
'checks. The tasks that wait on a need list it in needs as verse:<slug>.

' || drops.engine_has() || '

The Verse engine''s planned upgrades (slug: title. what):
' || (SELECT string_agg(slug || ': ' || title || '. ' || what, E'\n' ORDER BY slug) FROM drops.engine_tasks) $$;

-- Why proposals won't do, or NULL: every task checkable, a new milestone with tasks, and something to add.
CREATE FUNCTION drops.grow_invalid(d jsonb) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE m jsonb; t jsonb; tasks int := 0;
BEGIN
  IF jsonb_typeof(d#>'{roadmap,milestones}') <> 'array' THEN RETURN 'no roadmap'; END IF;
  FOR m IN SELECT * FROM jsonb_array_elements(d#>'{roadmap,milestones}') LOOP
    IF jsonb_typeof(m->'tasks') <> 'array' THEN RETURN 'a milestone without tasks'; END IF;
    IF NOT coalesce((m->>'existing')::bool, false) THEN
      IF coalesce(m->>'title', '') = '' THEN RETURN 'a milestone without a title'; END IF;
      IF jsonb_array_length(m->'tasks') = 0 THEN RETURN 'a new milestone without tasks'; END IF;
    END IF;
    FOR t IN SELECT * FROM jsonb_array_elements(m->'tasks') LOOP
      IF coalesce(t->>'title', '') = '' THEN RETURN 'a task without a title'; END IF;
      IF jsonb_typeof(t->'acceptance') <> 'array' OR jsonb_array_length(t->'acceptance') = 0 THEN RETURN 'a task without acceptance checks'; END IF;
      tasks := tasks + 1;
    END LOOP;
  END LOOP;
  IF tasks = 0 AND coalesce(btrim(d#>>'{design,markdown}'), '') = ''
     AND coalesce(jsonb_array_length(CASE WHEN jsonb_typeof(d->'assets') = 'array' THEN d->'assets' END), 0) = 0 THEN
    RETURN 'nothing to add';
  END IF;
  RETURN NULL;
END $$;

-- A draft that grows a project carries {"grow": ...} (set here, never by an edit: 0020 edits only the draft's own fields).
CREATE OR REPLACE FUNCTION drops.draft_invalid(d jsonb) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE m jsonb; t jsonb;
BEGIN
  IF jsonb_typeof(d) <> 'object' THEN RETURN 'not an object'; END IF;
  IF d ? 'grow' THEN RETURN drops.grow_invalid(d); END IF;
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

-- A drop on a project is read with its work (drops.grow_start); draft_start is for a new project's drop.
CREATE OR REPLACE FUNCTION drops.draft_start(pubkey text, drop_ text, bucket text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; cur drops.drafts;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.project IS NOT NULL THEN RETURN drops.r(409, '{"error":"a drop on a project is read with its work"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  IF NOT EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status = 'scanned') THEN
    RETURN drops.r(409, '{"error":"nothing kept to read yet"}');
  END IF;
  SELECT * INTO cur FROM drops.drafts WHERE drop_id = d.id;
  IF cur.status = 'reading' AND cur.started_ms > drops.now_ms() - 300000 THEN RETURN drops.r(409, '{"error":"already reading"}'); END IF;
  INSERT INTO drops.drafts (drop_id, status, started_ms) VALUES (d.id, 'reading', drops.now_ms())
  ON CONFLICT (drop_id) DO UPDATE SET status = 'reading', note = '', started_ms = EXCLUDED.started_ms, done_ms = NULL,
    original = NULL, edits = 0, edited_ms = NULL;
  RETURN drops.r(200, '{}') || jsonb_build_object('request', drops.draft_request(d.id, bucket));
END $$;

-- READ MY DROP on a project: the model reads the drop with the project's work (evs: its events from the relay).
CREATE FUNCTION drops.grow_start(pubkey text, drop_ text, bucket text, evs jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; cur drops.drafts; h record; w jsonb;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL OR d.project IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status <> 'open' THEN RETURN drops.r(409, '{"error":"this drop is closed"}'); END IF;
  IF NOT EXISTS (SELECT 1 FROM drops.files WHERE drop_id = d.id AND status = 'scanned') THEN
    RETURN drops.r(409, '{"error":"nothing kept to read yet"}');
  END IF;
  SELECT * INTO cur FROM drops.drafts WHERE drop_id = d.id;
  IF cur.status = 'reading' AND cur.started_ms > drops.now_ms() - 300000 THEN RETURN drops.r(409, '{"error":"already reading"}'); END IF;
  SELECT * INTO h FROM drops.home(d.project, d.project_pubkey) LIMIT 1;
  w := drops.grow_work(evs, d.project_pubkey, d.project);
  INSERT INTO drops.drafts (drop_id, status, started_ms) VALUES (d.id, 'reading', drops.now_ms())
  ON CONFLICT (drop_id) DO UPDATE SET status = 'reading', note = '', started_ms = EXCLUDED.started_ms, done_ms = NULL,
    original = NULL, edits = 0, edited_ms = NULL;
  RETURN drops.r(200, '{}') || jsonb_build_object('request',
    jsonb_set(jsonb_set(drops.draft_request(d.id, bucket), '{systemInstruction,parts,0,text}',
                        to_jsonb(drops.grow_prompt() || E'\n\n' || drops.grow_context(w, coalesce(h.name, d.project), d.project))),
              '{generationConfig,responseSchema}', drops.grow_schema()));
END $$;

-- The model's answer: a drop on a project is marked as proposals for it, named for it.
CREATE OR REPLACE FUNCTION drops.draft_done(drop_ text, http int, answer text, model_ text, took int) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE body jsonb; txt text; d jsonb; why text; x drops.drafts; dr drops.drops; h record;
BEGIN
  SELECT * INTO dr FROM drops.drops WHERE id = drop_;
  IF http = 200 THEN
    BEGIN
      body := answer::jsonb;
      SELECT string_agg(p->>'text', '' ORDER BY i) INTO txt
      FROM jsonb_array_elements(body#>'{candidates,0,content,parts}') WITH ORDINALITY AS z(p, i)
      WHERE coalesce((p->>'thought')::bool, false) = false AND p ? 'text';
      d := txt::jsonb;
    EXCEPTION WHEN others THEN d := NULL;
    END;
    IF jsonb_typeof(d) = 'object' AND dr.project IS NOT NULL THEN
      SELECT * INTO h FROM drops.home(dr.project, dr.project_pubkey) LIMIT 1;
      d := (d - 'grow') || jsonb_build_object('grow', jsonb_build_object('project', dr.project, 'pubkey', dr.project_pubkey));
      d := jsonb_set(d, '{card}', CASE WHEN jsonb_typeof(d->'card') = 'object' THEN d->'card' ELSE '{}' END
                     || jsonb_build_object('name', coalesce(h.name, dr.project), 'slug', dr.project));
    END IF;
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

-- ACCEPT, after draft_approve froze the proposals: what to sign, from the draft and the project's work now
-- (evs). Kept once, so an accept cut off half way goes on where it stopped.
CREATE FUNCTION drops.grow_begin(pubkey text, drop_ text, evs jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; x drops.drafts; w jsonb; m jsonb; t jsonb; seq int := 0; seen text[]; ms text; ts text; n int;
        found text; body text; issues int := 0;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL OR d.project IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  IF d.status = 'published' THEN RETURN drops.r(200, '{"drop":"published"}'); END IF;
  IF d.status <> 'approved' THEN RETURN drops.r(409, '{"error":"ACCEPT the reviewed proposals first"}'); END IF;
  IF NOT EXISTS (SELECT 1 FROM drops.grow_items WHERE drop_id = d.id) THEN
    SELECT * INTO x FROM drops.drafts WHERE drop_id = d.id;
    w := drops.grow_work(evs, d.project_pubkey, d.project);
    SELECT coalesce(array_agg(i->>'slug'), '{}') || ARRAY['design', 'order', 'a'] INTO seen FROM jsonb_array_elements(w->'items') i;
    FOR m IN SELECT * FROM jsonb_array_elements(coalesce(x.draft#>'{roadmap,milestones}', '[]')) LOOP
      found := NULL;
      IF coalesce((m->>'existing')::bool, false) THEN
        SELECT i->>'id' INTO found FROM jsonb_array_elements(w->'items') i WHERE i->>'kind' = 'milestone' AND i->>'slug' = m->>'slug' LIMIT 1;
      END IF;
      IF found IS NULL AND jsonb_array_length(coalesce(m->'tasks', '[]')) > 0 THEN
        seq := seq + 1;
        ms := drops.slug(m->>'slug'); IF ms = '' THEN ms := 'milestone'; END IF;
        n := 1; WHILE (ms || CASE WHEN n > 1 THEN '-' || n ELSE '' END) = ANY (seen) LOOP n := n + 1; END LOOP;
        ms := ms || CASE WHEN n > 1 THEN '-' || n ELSE '' END; seen := seen || ms;
        INSERT INTO drops.grow_items (drop_id, seq, item, kind, item_kind, parent_id, title, body)
        VALUES (d.id, seq, ms, 1621, 'milestone', w->>'initiative', drops.one_line(coalesce(nullif(m->>'title', ''), m->>'slug'), 200), drops.clean(m->>'body'));
        issues := issues + 1;
      END IF;
      FOR t IN SELECT * FROM jsonb_array_elements(coalesce(m->'tasks', '[]')) LOOP
        seq := seq + 1;
        ts := drops.slug(t->>'slug'); IF ts = '' THEN ts := 'task'; END IF;
        n := 1; WHILE (ts || CASE WHEN n > 1 THEN '-' || n ELSE '' END) = ANY (seen) LOOP n := n + 1; END LOOP;
        ts := ts || CASE WHEN n > 1 THEN '-' || n ELSE '' END; seen := seen || ts;
        body := drops.clean(t->>'body') || E'\n\n## Acceptance\n\n'
          || (SELECT string_agg('- ' || drops.one_line(a, 1000), E'\n') FROM jsonb_array_elements_text(t->'acceptance') a);
        IF jsonb_array_length(coalesce(t->'needs', '[]')) > 0 THEN
          body := body || E'\n\n## Needs from the Verse engine\n\n'
            || (SELECT string_agg('- ' || drops.one_line(a, 200), E'\n') FROM jsonb_array_elements_text(t->'needs') a);
        END IF;
        INSERT INTO drops.grow_items (drop_id, seq, item, kind, item_kind, parent, parent_id, title, body, size, labels)
        VALUES (d.id, seq, ts, 1621, 'task', CASE WHEN found IS NULL THEN ms END, found, drops.one_line(t->>'title', 200), body,
                CASE WHEN t->>'size' IN ('S', 'M', 'L') THEN t->>'size' ELSE '' END,
                CASE WHEN jsonb_array_length(coalesce(t->'needs', '[]')) > 0 THEN ARRAY['needs-verse'] ELSE '{}' END);
        issues := issues + 1;
      END LOOP;
    END LOOP;
    IF coalesce(btrim(x.draft#>>'{design,markdown}'), '') <> '' THEN
      seq := seq + 1;
      body := drops.clean(x.draft#>>'{design,markdown}');
      IF jsonb_array_length(coalesce(x.draft#>'{design,open_questions}', '[]')) > 0 THEN
        body := body || E'\n\n## Open questions\n\n'
          || (SELECT string_agg('- ' || drops.one_line(q, 1000), E'\n') FROM jsonb_array_elements_text(x.draft#>'{design,open_questions}') q);
      END IF;
      INSERT INTO drops.grow_items (drop_id, seq, item, kind, item_kind, title, body)
      VALUES (d.id, seq, 'design', 1111, 'note', 'Design notes', body);
    END IF;
    IF issues > 0 THEN
      INSERT INTO drops.grow_items (drop_id, seq, item, kind, title, body)
      VALUES (d.id, seq + 1, 'order', 30623, '', coalesce((w->'order')::text, ''));
    END IF;
    -- the issues' 'a' tags as the project's own issues carry them
    INSERT INTO drops.grow_items (drop_id, seq, item, kind, title, body)
    VALUES (d.id, -1, 'a', 0, '', coalesce((w->'a')::text, '[]'));
  END IF;
  RETURN drops.r(200, '{"drop":"accepting"}');
END $$;

-- The next event ACCEPT signs: {seq, kind, tags, content} as NIP-01 text, or {} when every one is sent.
CREATE FUNCTION drops.grow_next(drop_ text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE d drops.drops; i drops.grow_items; h record; relay text := drops.repo()->>'relay'; pc text; rc text; tags text[]; id_of jsonb;
        a jsonb; old jsonb; top text; k int := 0; name_ text;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_;
  SELECT * INTO i FROM drops.grow_items WHERE drop_id = drop_ AND kind > 0 AND event_id IS NULL ORDER BY seq LIMIT 1;
  IF i.drop_id IS NULL THEN RETURN '{}'; END IF;
  SELECT * INTO h FROM drops.home(d.project, d.project_pubkey) LIMIT 1;
  name_ := coalesce(h.name, d.project);
  pc := '30621:' || d.project_pubkey || ':' || d.project;
  rc := '30617:' || d.project_pubkey || ':' || (drops.repo()->>'d');
  SELECT coalesce(jsonb_object_agg(item, event_id), '{}') INTO id_of FROM drops.grow_items WHERE drop_id = drop_ AND event_id IS NOT NULL;
  SELECT body::jsonb INTO a FROM drops.grow_items WHERE drop_id = drop_ AND item = 'a';
  IF i.kind = 1621 THEN
    IF jsonb_array_length(coalesce(a, '[]')) > 0 THEN
      SELECT array_agg(drops.tag(VARIADIC ARRAY(SELECT jsonb_array_elements_text(t))) ORDER BY o) INTO tags FROM jsonb_array_elements(a) WITH ORDINALITY AS z(t, o);
    ELSE
      tags := ARRAY[drops.tag('a', rc, relay), drops.tag('a', pc, relay)];
    END IF;
    tags := tags || drops.tag('p', d.project_pubkey) || drops.tag('slug', i.item) || drops.tag('subject', i.title);
    IF i.item_kind <> 'task' THEN tags := tags || drops.tag('t', i.item_kind); END IF;
    tags := tags || coalesce((SELECT array_agg(drops.tag('t', l)) FROM unnest(i.labels) l), '{}');
    IF i.size <> '' THEN tags := tags || drops.tag('size', i.size); END IF;
    IF coalesce(i.parent_id, id_of->>i.parent) IS NOT NULL THEN tags := tags || drops.tag('parent', coalesce(i.parent_id, id_of->>i.parent)); END IF;
    tags := tags || drops.tag('alt', 'A ' || i.item_kind || ' in the ' || name_ || ' project: ' || i.title);
  ELSIF i.kind = 1111 THEN   -- NIP-22: a comment on the project itself
    tags := ARRAY[drops.tag('A', pc, relay), drops.tag('K', '30621'), drops.tag('P', d.project_pubkey),
                  drops.tag('a', pc, relay), drops.tag('k', '30621'), drops.tag('p', d.project_pubkey), drops.tag('t', 'design'),
                  drops.tag('alt', 'Design notes for the ' || name_ || ' project')];
  ELSE                        -- the order as it was, then the new issues after its last rank
    old := CASE WHEN i.body = '' THEN NULL ELSE i.body::jsonb END;
    IF old IS NULL THEN
      tags := ARRAY[drops.tag('d', d.project), drops.tag('a', pc, relay)];
    ELSE
      SELECT array_agg(drops.tag(VARIADIC ARRAY(SELECT jsonb_array_elements_text(t))) ORDER BY o) INTO tags
      FROM jsonb_array_elements(old) WITH ORDINALITY AS z(t, o) WHERE t->>0 <> 'alt';
      SELECT max((t->>2) COLLATE "C") INTO top FROM jsonb_array_elements(old) t WHERE t->>0 = 'item';
    END IF;
    FOR i IN SELECT * FROM drops.grow_items WHERE drop_id = drop_ AND kind = 1621 ORDER BY seq LOOP
      tags := tags || drops.tag('item', id_of->>i.item, coalesce(top, '1')
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k / 61, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k % 61, 1));
      k := k + 1;
    END LOOP;
    tags := tags || drops.tag('alt', 'The ' || name_ || ' project''s task order');
    SELECT * INTO i FROM drops.grow_items WHERE drop_id = drop_ AND item = 'order';
  END IF;
  RETURN jsonb_build_object('seq', i.seq, 'kind', i.kind, 'tags', '[' || array_to_string(tags, ',') || ']',
    'content', CASE WHEN i.kind = 30623 THEN '""' ELSE drops.j(i.body) END);
END $$;

CREATE FUNCTION drops.grow_sent(drop_ text, seq_ int, id text, ev text) RETURNS bool LANGUAGE sql VOLATILE AS $$
  UPDATE drops.grow_items SET event_id = id, event = ev WHERE drop_id = drop_ AND seq = seq_ AND event_id IS NULL
    AND id ~ '^[0-9a-f]{64}$' RETURNING true $$;

-- The drop's kept files: where each is in the drops bucket and where it goes next to the project's page.
CREATE FUNCTION drops.grow_files(drop_ text) RETURNS TABLE (object text, dest text) LANGUAGE sql STABLE AS $$
  SELECT f.object, 'projects/' || d.project || '/files/' || coalesce(nullif(drops.slug(regexp_replace(f.name, '\.[^.]*$', '')), ''), 'file')
         || '-' || substr(f.id, 3, 6) || '.' || f.ext
  FROM drops.drops d JOIN drops.files f ON f.drop_id = d.id AND f.status = 'scanned' WHERE d.id = drop_ ORDER BY f.created_ms $$;

CREATE FUNCTION drops.grow_done(drop_ text) RETURNS bool LANGUAGE sql VOLATILE AS $$
  UPDATE drops.drops SET status = 'published' WHERE id = drop_ AND status = 'approved' RETURNING true $$;

CREATE FUNCTION drops.grow_answer(pubkey text, drop_ text, why text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN why IS NULL
    THEN drops.r(200, (drops.draft_get(pubkey, drop_)->'body') || jsonb_build_object('published', jsonb_build_object('slug', d.project,
      'page', '/projects/' || d.project, 'added', (SELECT count(*) FROM drops.grow_items g WHERE g.drop_id = d.id AND g.kind = 1621))))
    ELSE drops.r(502, (drops.draft_get(pubkey, drop_)->'body') || jsonb_build_object('error', 'Not added yet: ' || why || ' stopped it. ACCEPT again goes on from there.'))
  END FROM drops.drops d WHERE d.id = drop_ $$;

-- The drop area on a project: its name and whether this viewer may drop on it.
CREATE FUNCTION drops.home_for(pubkey text, admin bool, slug_ text, pk text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT drops.r(200, jsonb_build_object('slug', h.slug, 'name', h.name, 'member', admin OR home_for.pubkey = ANY (h.members)))
                   FROM drops.home(slug_, pk) h LIMIT 1), drops.r(404, '{"error":"this project doesn''t take drops yet"}')) $$;
