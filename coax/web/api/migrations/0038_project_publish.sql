-- Publishing a project made from a drop (rc-publish; docs/projects/new-project.md
-- Part 2, step 4). After PUBLISH froze the reviewed draft (0020), drops.c makes
-- the project's key, seals it (AES-256-GCM under OA_PROJECT_KEK; only the
-- sealed bytes are here), signs and sends its events to the relay, copies its
-- kept files into the site's bucket and writes its page, so it is live at
-- /projects/<slug> and listed on /projects without a code change or release.
--
-- Here: the slug the project takes, its items in the order they are signed,
-- each event as NIP-01 text (tags and content serialized by to_json, which
-- escapes exactly as NIP-01 does once control characters are gone), the page,
-- its index.md and /projects/published.txt (the list's rows for the page to add).
-- Every step is recorded, so a publish cut off half way goes on where it
-- stopped: no event is signed twice.

CREATE TABLE drops.projects (
  slug text PRIMARY KEY,
  drop_id text NOT NULL UNIQUE REFERENCES drops.drops (id),
  owner text NOT NULL,              -- who dropped it: the project's maintainer until their account takes the key
  pubkey text,                      -- the project's key (public half)
  key_sealed text,                  -- nonce || ciphertext || tag, hex; never the key itself
  name text NOT NULL,
  line text NOT NULL,
  visibility text NOT NULL DEFAULT 'public' CHECK (visibility IN ('public', 'private')),   -- staging's are private (deploy.sh)
  created_ms bigint NOT NULL,
  sent_ms bigint,                   -- every event is on the relay
  files_ms bigint,                  -- the files are in the site's bucket
  published_ms bigint               -- the page is up and listed
);
CREATE TABLE drops.project_items (
  slug text NOT NULL REFERENCES drops.projects (slug),
  seq int NOT NULL,                 -- the order they are signed in (parents first) and ranked in
  item text NOT NULL,               -- 'repo', 'project', an item's slug, 'order'
  kind int NOT NULL,
  title text NOT NULL DEFAULT '',
  body text NOT NULL DEFAULT '',
  item_kind text NOT NULL DEFAULT '',   -- initiative, milestone, task; '' for repo, project, order
  parent text,
  size text NOT NULL DEFAULT '',
  labels text[] NOT NULL DEFAULT '{}',
  event_id text,                    -- once sent
  event text,                       -- the signed event as sent
  PRIMARY KEY (slug, seq),
  UNIQUE (slug, item)
);

-- Text as it may go into an event, a page or a list row: no control characters but newlines and tabs.
CREATE FUNCTION drops.clean(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT regexp_replace(coalesce(s, ''), '[\x01-\x08\x0b\x0c\x0e-\x1f\x7f]', '', 'g') $$;
CREATE FUNCTION drops.one_line(s text, n int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT left(btrim(regexp_replace(drops.clean(s), '[\s|]+', ' ', 'g')), n) $$;
CREATE FUNCTION drops.j(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT to_json(drops.clean(s))::text $$;
CREATE FUNCTION drops.tag(VARIADIC v text[]) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT '[' || string_agg(drops.j(x), ',' ORDER BY k) || ']' FROM unnest(v) WITH ORDINALITY AS t(x, k) $$;
CREATE FUNCTION drops.html(s text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT replace(replace(replace(replace(drops.clean(s), '&', '&amp;'), '<', '&lt;'), '>', '&gt;'), '"', '&quot;') $$;

-- Where the work lives: the OpenAgents monorepo, as every Verse world's does (the project's own 30617).
CREATE FUNCTION drops.repo() RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$ SELECT '{
  "d": "openagents", "name": "OpenAgents", "description": "The OpenAgents monorepo: the apps, the protocol, Coax and the Grid.",
  "web": "https://github.com/OpenAgentsInc/openagents", "clone": "https://github.com/OpenAgentsInc/openagents.git",
  "euc": "0aa2d953dd48f0d402ea7c826bad90b2548524c2", "relay": "wss://relay.openagents.com"}'::jsonb $$;

-- Slugs a published project can't take: the site's own pages and projects, and private projects.
CREATE FUNCTION drops.slug_taken(s text) RETURNS bool LANGUAGE sql STABLE AS $$
  SELECT s IN ('coax', 'coder', 'dead-gears', 'openagents-desktop', 'planetary-villa', 'private', 'psionic', 'verse',
               'new', 'demo', 'api', 'published', 'list', 'index', 'mine', 'assets')
      OR EXISTS (SELECT 1 FROM projects.docs WHERE slug = s)
      OR EXISTS (SELECT 1 FROM drops.projects WHERE slug = s) $$;

-- Begins (or goes on with) publishing an approved drop: its slug and items. drops.c makes the key when
-- the answer's key is false, then signs the events of drops.publish_next in turn.
CREATE FUNCTION drops.publish_begin(pubkey text, drop_ text, vis text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE d drops.drops; x drops.drafts; p drops.projects; base text; s text; n int := 1; seq int := 0;
        m jsonb; t jsonb; ini text; ms text; ts text; seen text[] := ARRAY['repo', 'project', 'order']; body text;
BEGIN
  SELECT * INTO d FROM drops.drops WHERE id = drop_ AND owner = pubkey FOR UPDATE;
  IF d.id IS NULL THEN RETURN drops.r(404, '{"error":"not found"}'); END IF;
  SELECT * INTO p FROM drops.projects WHERE drop_id = d.id;
  IF d.status = 'published' THEN
    RETURN drops.r(200, jsonb_build_object('drop', 'published', 'slug', p.slug, 'page', '/projects/' || p.slug));
  END IF;
  IF d.status <> 'approved' THEN RETURN drops.r(409, '{"error":"PUBLISH the reviewed draft first"}'); END IF;
  IF p.slug IS NULL THEN
    SELECT * INTO x FROM drops.drafts WHERE drop_id = d.id;
    base := left(coalesce(nullif(drops.slug(x.draft#>>'{card,slug}'), ''), nullif(drops.slug(x.draft#>>'{card,name}'), ''), 'project'), 56);
    s := base;
    WHILE drops.slug_taken(s) LOOP n := n + 1; s := base || '-' || n; END LOOP;
    INSERT INTO drops.projects (slug, drop_id, owner, name, line, visibility, created_ms)
    VALUES (s, d.id, d.owner, drops.one_line(x.draft#>>'{card,name}', 120), drops.one_line(x.draft#>>'{card,line}', 400),
            CASE WHEN vis = 'private' THEN 'private' ELSE 'public' END, drops.now_ms())
    RETURNING * INTO p;
    INSERT INTO drops.project_items (slug, seq, item, kind) VALUES (s, 0, 'repo', 30617), (s, 1, 'project', 30621);
    seq := 2;
    -- one item slug each, unique in the project
    ini := drops.slug(coalesce(x.draft#>>'{roadmap,initiative,slug}', s));
    IF ini = '' OR ini = ANY (seen) THEN ini := s; END IF;
    seen := seen || ini;
    INSERT INTO drops.project_items (slug, seq, item, kind, item_kind, title, body)
    VALUES (s, seq, ini, 1621, 'initiative', drops.one_line(x.draft#>>'{roadmap,initiative,title}', 200), drops.clean(x.draft#>>'{roadmap,initiative,body}'));
    FOR m IN SELECT * FROM jsonb_array_elements(x.draft#>'{roadmap,milestones}') LOOP
      seq := seq + 1;
      ms := drops.slug(m->>'slug'); IF ms = '' THEN ms := 'milestone'; END IF;
      n := 1; WHILE (ms || CASE WHEN n > 1 THEN '-' || n ELSE '' END) = ANY (seen) LOOP n := n + 1; END LOOP;
      ms := ms || CASE WHEN n > 1 THEN '-' || n ELSE '' END; seen := seen || ms;
      INSERT INTO drops.project_items (slug, seq, item, kind, item_kind, parent, title, body)
      VALUES (s, seq, ms, 1621, 'milestone', ini, drops.one_line(m->>'title', 200), drops.clean(m->>'body'));
      FOR t IN SELECT * FROM jsonb_array_elements(m->'tasks') LOOP
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
        INSERT INTO drops.project_items (slug, seq, item, kind, item_kind, parent, title, body, size, labels)
        VALUES (s, seq, ts, 1621, 'task', ms, drops.one_line(t->>'title', 200), body,
                CASE WHEN t->>'size' IN ('S', 'M', 'L') THEN t->>'size' ELSE '' END,
                CASE WHEN jsonb_array_length(coalesce(t->'needs', '[]')) > 0 THEN ARRAY['needs-verse'] ELSE '{}' END);
      END LOOP;
    END LOOP;
    INSERT INTO drops.project_items (slug, seq, item, kind) VALUES (s, seq + 1, 'order', 30623);
  END IF;
  RETURN drops.r(200, jsonb_build_object('drop', 'publishing', 'slug', p.slug, 'key', p.key_sealed IS NOT NULL))
    || jsonb_build_object('slug', p.slug, 'owner', p.owner, 'has_key', p.key_sealed IS NOT NULL);
END $$;

-- The key, sealed by drops.c, kept once.
CREATE FUNCTION drops.publish_key(slug_ text, pub text, sealed text) RETURNS bool LANGUAGE sql VOLATILE AS $$
  UPDATE drops.projects SET pubkey = pub, key_sealed = sealed WHERE slug = slug_ AND key_sealed IS NULL
    AND pub ~ '^[0-9a-f]{64}$' AND sealed ~ '^[0-9a-f]+$' RETURNING true $$;
CREATE FUNCTION drops.publish_sealed(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT key_sealed FROM drops.projects WHERE slug = slug_ $$;

-- The next event to sign: {seq, kind, tags, content} as NIP-01 text, or {} when every one is sent.
CREATE FUNCTION drops.publish_next(slug_ text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE p drops.projects; i drops.project_items; r jsonb := drops.repo(); rc text; pc text; relay text := drops.repo()->>'relay';
        tags text[]; id_of jsonb; k int := 0;
BEGIN
  SELECT * INTO p FROM drops.projects WHERE slug = slug_;
  SELECT * INTO i FROM drops.project_items WHERE slug = slug_ AND event_id IS NULL ORDER BY seq LIMIT 1;
  IF i.slug IS NULL OR p.pubkey IS NULL THEN RETURN '{}'; END IF;
  rc := '30617:' || p.pubkey || ':' || (r->>'d');
  pc := '30621:' || p.pubkey || ':' || p.slug;
  SELECT coalesce(jsonb_object_agg(item, event_id), '{}') INTO id_of FROM drops.project_items WHERE slug = slug_ AND event_id IS NOT NULL;
  IF i.item = 'repo' THEN
    tags := ARRAY[drops.tag('d', r->>'d'), drops.tag('name', r->>'name'), drops.tag('description', r->>'description'),
      drops.tag('web', r->>'web'), drops.tag('clone', r->>'clone'), drops.tag('relays', relay), drops.tag('r', r->>'euc', 'euc')];
  ELSIF i.item = 'project' THEN
    tags := ARRAY[drops.tag('d', p.slug), drops.tag('name', p.name), drops.tag('description', p.line), drops.tag('a', rc, relay),
      drops.tag('path', rc, 'verse/' || p.slug || '/'), drops.tag('contributions', 'open'), drops.tag('license', 'Apache-2.0'),
      drops.tag('visibility', p.visibility), drops.tag('buzz-visibility', CASE WHEN p.visibility = 'public' THEN 'listed' ELSE 'unlisted' END), drops.tag('p', p.owner, '', 'maintainer')];
  ELSIF i.item = 'order' THEN
    tags := ARRAY[drops.tag('d', p.slug), drops.tag('a', pc, relay)];
    FOR i IN SELECT * FROM drops.project_items WHERE slug = slug_ AND kind = 1621 ORDER BY seq LOOP
      -- ranks as rank.ts reads them: base-62 digits, never ending in 0, in seq order
      tags := tags || drops.tag('item', id_of->>i.item,
        substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k / 61, 1)
        || substr('0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz', 2 + k % 61, 1));
      k := k + 1;
    END LOOP;
    tags := tags || drops.tag('alt', 'The ' || p.name || ' project''s task order');
    SELECT * INTO i FROM drops.project_items WHERE slug = slug_ AND item = 'order';
  ELSE
    tags := ARRAY[drops.tag('a', rc, relay), drops.tag('a', pc, relay), drops.tag('p', p.pubkey), drops.tag('slug', i.item),
      drops.tag('subject', i.title)];
    IF i.item_kind <> 'task' THEN tags := tags || drops.tag('t', i.item_kind); END IF;
    tags := tags || coalesce((SELECT array_agg(drops.tag('t', l)) FROM unnest(i.labels) l), '{}');
    IF i.size <> '' THEN tags := tags || drops.tag('size', i.size); END IF;
    IF i.parent IS NOT NULL THEN tags := tags || drops.tag('parent', id_of->>i.parent); END IF;
    tags := tags || drops.tag('alt', 'A ' || i.item_kind || ' in the ' || p.name || ' project: ' || i.title);
  END IF;
  RETURN jsonb_build_object('seq', i.seq, 'kind', i.kind, 'tags', '[' || array_to_string(tags, ',') || ']',
    'content', CASE WHEN i.kind = 1621 THEN drops.j(i.body) ELSE '""' END);
END $$;

-- An event the relay took (its id and the event as sent).
CREATE FUNCTION drops.publish_sent(slug_ text, seq_ int, id text, ev text) RETURNS bool LANGUAGE sql VOLATILE AS $$
  UPDATE drops.project_items SET event_id = id, event = ev WHERE slug = slug_ AND seq = seq_ AND event_id IS NULL
    AND id ~ '^[0-9a-f]{64}$' RETURNING true $$;

-- The drop's kept files: where each is in the drops bucket and where it goes on the site.
CREATE FUNCTION drops.publish_files(slug_ text) RETURNS TABLE (object text, dest text, content_type text) LANGUAGE sql STABLE AS $$
  SELECT f.object, 'projects/' || p.slug || '/files/' || coalesce(nullif(drops.slug(regexp_replace(f.name, '\.[^.]*$', '')), ''), 'file')
         || '-' || substr(f.id, 3, 6) || '.' || f.ext, f.content_type
  FROM drops.projects p JOIN drops.files f ON f.drop_id = p.drop_id AND f.status = 'scanned' WHERE p.slug = slug_ ORDER BY f.created_ms $$;

-- The page (the shared project app draws it live from the relay; the hidden HTML is the name and one link).
CREATE FUNCTION drops.publish_page(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>' || drops.html(p.name) || ' · OpenAgents project</title>
<meta name="description" content="' || drops.html(p.line) || '">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/projects/' || p.slug || '/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ff8c1f}
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>' || drops.html(p.name) || '</h1>
<p><a href="/projects/' || p.slug || '/index.md">Tasks</a></p>
</main>
<script type="module" src="/coaxial/host.js" data-app="project" data-text="' || p.pubkey || ' ' || p.slug || '"></script>
</body>
</html>
' FROM drops.projects p WHERE p.slug = slug_ $$;

-- The same for agents: the card, the design in the owner's words, the roadmap, the files and what's left to make.
CREATE FUNCTION drops.publish_md(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || p.name || E'\n\n' || p.line || E'\n\n'
    || 'An OpenAgents project made from a drop. Its tasks are on Nostr: ' || (drops.repo()->>'relay') || ', kind 30621 by '
    || p.pubkey || ', d ' || p.slug || E'.\n\n'
    || coalesce('Art direction: ' || drops.one_line(x.draft#>>'{card,art_direction}', 2000) || E'\n\n', '')
    || E'## Design\n\n' || drops.clean(x.draft#>>'{design,markdown}') || E'\n\n'
    || coalesce(E'### Open questions\n\n' || (SELECT string_agg('- ' || drops.one_line(q, 1000), E'\n') FROM jsonb_array_elements_text(x.draft#>'{design,open_questions}') q) || E'\n\n', '')
    || E'## Roadmap\n\n'
    || (SELECT string_agg(CASE i.item_kind WHEN 'initiative' THEN '**' || i.title || '**' WHEN 'milestone' THEN E'\n### ' || i.title
                              ELSE '- ' || i.title || coalesce(' (' || nullif(i.size, '') || ')', '') END, E'\n' ORDER BY i.seq)
        FROM drops.project_items i WHERE i.slug = p.slug AND i.kind = 1621) || E'\n\n'
    || coalesce(E'## Files\n\n' || (SELECT string_agg('- [' || f.dest || '](/' || f.dest || ')', E'\n') FROM drops.publish_files(p.slug) f) || E'\n\n', '')
    || coalesce(E'## Still to make\n\n' || (SELECT string_agg('- ' || drops.one_line(m, 500), E'\n') FROM jsonb_array_elements_text(x.draft->'to_make') m) || E'\n\n', '')
  FROM drops.projects p JOIN drops.drafts x ON x.drop_id = p.drop_id WHERE p.slug = slug_ $$;

-- /projects/published.txt: one PROJECTS_ROWS line (coax/web/project/list.ts) for each published project,
-- newest first, for /projects to add to the list its build baked.
CREATE FUNCTION drops.published_rows() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(concat_ws('|', p.slug, drops.one_line(p.name, 120), drops.one_line(p.line, 400), 'open', 'public',
      (SELECT count(*) FROM drops.project_items i WHERE i.slug = p.slug AND i.item_kind = 'task'),
      (SELECT count(*) FROM drops.project_items i WHERE i.slug = p.slug AND i.item_kind = 'task'), 0, '',
      lpad((p.published_ms / 1000)::text, 10, '0'),
      to_char(to_timestamp(p.published_ms / 1000.0) AT TIME ZONE 'America/Chicago', 'MM-DD HH24:MI'),
      '/projects/' || p.slug, p.pubkey, p.owner) || E'\n', '' ORDER BY p.published_ms DESC), '')
  FROM drops.projects p WHERE p.published_ms IS NOT NULL AND p.visibility = 'public' $$;

CREATE FUNCTION drops.publish_step(slug_ text, step text) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE p drops.projects;
BEGIN
  IF step = 'sent' THEN UPDATE drops.projects SET sent_ms = coalesce(sent_ms, drops.now_ms()) WHERE slug = slug_;
  ELSIF step = 'files' THEN UPDATE drops.projects SET files_ms = coalesce(files_ms, drops.now_ms()) WHERE slug = slug_;
  ELSIF step = 'listed' THEN
    UPDATE drops.projects SET published_ms = coalesce(published_ms, drops.now_ms()) WHERE slug = slug_;
  ELSIF step = 'done' THEN
    UPDATE drops.drops SET status = 'published' WHERE id = (SELECT drop_id FROM drops.projects WHERE slug = slug_);
  END IF;
  SELECT * INTO p FROM drops.projects WHERE slug = slug_;
  RETURN jsonb_build_object('sent', p.sent_ms IS NOT NULL, 'files', p.files_ms IS NOT NULL, 'listed', p.published_ms IS NOT NULL);
END $$;

-- /projects/published-slugs.txt: every slug this environment published, a line each, so a site deploy
-- keeps their pages (coax/web/upload.py) and a release leaves staging's on staging (coax/web/release.sh).
CREATE FUNCTION drops.published_slugs() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(slug || E'\n', '' ORDER BY slug), '') FROM drops.projects WHERE published_ms IS NOT NULL $$;

-- PUBLISH's answer: the draft (approved), and where the project is live, or why it isn't yet.
CREATE FUNCTION drops.publish_answer(pubkey text, drop_ text, slug_ text, why text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN why IS NULL
    THEN drops.r(200, (drops.draft_get(pubkey, drop_)->'body') || jsonb_build_object('published', jsonb_build_object('slug', slug_, 'page', '/projects/' || slug_)))
    ELSE drops.r(502, (drops.draft_get(pubkey, drop_)->'body') || jsonb_build_object('error', 'Not public yet: ' || why || ' stopped it. PUBLISH again goes on from there.'))
  END $$;
