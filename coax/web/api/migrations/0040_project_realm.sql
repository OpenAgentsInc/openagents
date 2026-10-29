-- A first realm from the drop's assets (rc-first-realm; docs/projects/new-project.md
-- Part 2, step 2): when a drop is published, its binary glTF models (.glb) are
-- packed into one asset pack (the realm host's VPK1, docs/verse/realm-host.md)
-- and stood on an empty floor at /projects/<slug>/realm, so the project has
-- somewhere to walk on day one. The project's card links it: its project event
-- names the realm, its index.md links it, its page's link preview shows the
-- drop's cover.
--
-- Here: which models go in and under what licence, a model's glTF read into
-- what drops.c copies out of its binary chunk (every primitive's world
-- transform, where its positions, normals, uvs and indices lie, its colour and
-- its texture), the realm's page and its index.md. drops.c does the bytes.

ALTER TABLE drops.projects ADD COLUMN realm_ms bigint;     -- the realm's pack and page are in the site's bucket
ALTER TABLE drops.projects ADD COLUMN realm_note text;     -- what the realm couldn't place, for the owner

-- The drop's models that go in the realm, in the order they were dropped, each named once (the pack's
-- mesh name: letters, digits, - and _, at most 31) with the licence the draft gave it.
CREATE FUNCTION drops.realm_models(slug_ text) RETURNS TABLE (object text, file text, mesh text, licence text) LANGUAGE sql STABLE AS $$
  SELECT f.object, drops.one_line(f.name, 60),
         left(coalesce(nullif(trim(BOTH '-' FROM regexp_replace(regexp_replace(f.name, '\.[^.]*$', ''), '[^A-Za-z0-9_-]+', '-', 'g')), ''), 'model'), 31),
         coalesce(nullif(drops.one_line((SELECT a->>'licence' FROM jsonb_array_elements(coalesce(x.draft->'assets', '[]')) a
                                         WHERE a->>'file' = f.name LIMIT 1), 60), ''), 'licence unknown: ask the owner')
  FROM drops.projects p JOIN drops.files f ON f.drop_id = p.drop_id AND f.status = 'scanned' AND f.ext = 'glb'
  LEFT JOIN drops.drafts x ON x.drop_id = p.drop_id
  WHERE p.slug = slug_ ORDER BY f.created_ms, f.id LIMIT 24 $$;
CREATE FUNCTION drops.has_realm(slug_ text) RETURNS bool LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM drops.realm_models(slug_)) $$;

-- ---- a model's glTF (its JSON chunk, as jsonb): https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html

-- 4x4 matrices as glTF keeps them: 16 numbers, column by column.
CREATE FUNCTION drops.glb_m4(a float8[], b float8[]) RETURNS float8[] LANGUAGE sql IMMUTABLE AS $$
  SELECT array_agg(v ORDER BY c, r) FROM (
    SELECT c, r, sum(a[k * 4 + r + 1] * b[c * 4 + k + 1]) AS v
    FROM generate_series(0, 3) c, generate_series(0, 3) r, generate_series(0, 3) k GROUP BY c, r) x $$;
CREATE FUNCTION drops.glb_num(v jsonb, i int, fallback float8) RETURNS float8 LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN jsonb_typeof(v->i) = 'number' THEN (v->>i)::float8 ELSE fallback END $$;
-- A node's own transform: its matrix, or its translation, rotation (a quaternion) and scale.
CREATE FUNCTION drops.glb_local(n jsonb) RETURNS float8[] LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE t jsonb := n->'translation'; q jsonb := n->'rotation'; s jsonb := n->'scale';
        x float8; y float8; z float8; w float8; sx float8; sy float8; sz float8; m float8[];
BEGIN
  IF jsonb_typeof(n->'matrix') = 'array' AND jsonb_array_length(n->'matrix') = 16 THEN
    SELECT array_agg(CASE WHEN jsonb_typeof(e) = 'number' THEN e::text::float8 ELSE 0 END ORDER BY k) INTO m
    FROM jsonb_array_elements(n->'matrix') WITH ORDINALITY AS z(e, k);
    RETURN m;
  END IF;
  x := drops.glb_num(q, 0, 0); y := drops.glb_num(q, 1, 0); z := drops.glb_num(q, 2, 0); w := drops.glb_num(q, 3, 1);
  sx := drops.glb_num(s, 0, 1); sy := drops.glb_num(s, 1, 1); sz := drops.glb_num(s, 2, 1);
  RETURN ARRAY[(1 - 2 * (y * y + z * z)) * sx, 2 * (x * y + z * w) * sx, 2 * (x * z - y * w) * sx, 0,
               2 * (x * y - z * w) * sy, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z + x * w) * sy, 0,
               2 * (x * z + y * w) * sz, 2 * (y * z - x * w) * sz, (1 - 2 * (x * x + y * y)) * sz, 0,
               drops.glb_num(t, 0, 0), drops.glb_num(t, 1, 0), drops.glb_num(t, 2, 0), 1];
END $$;
-- Every mesh the scene draws and its world transform, walking down from the node (at most 32 deep).
CREATE FUNCTION drops.glb_walk(j jsonb, node int, parent float8[], depth int) RETURNS TABLE (o_mesh int, o_m float8[]) LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE n jsonb := j->'nodes'->node; w float8[]; ch int;
BEGIN
  IF n IS NULL OR depth > 32 THEN RETURN; END IF;
  w := drops.glb_m4(parent, drops.glb_local(n));
  IF jsonb_typeof(n->'mesh') = 'number' THEN o_mesh := (n->>'mesh')::int; o_m := w; RETURN NEXT; END IF;
  IF jsonb_typeof(n->'children') = 'array' THEN
    FOR ch IN SELECT e::text::int FROM jsonb_array_elements(n->'children') e WHERE jsonb_typeof(e) = 'number' LOOP
      RETURN QUERY SELECT * FROM drops.glb_walk(j, ch, w, depth + 1);
    END LOOP;
  END IF;
END $$;
-- Where an accessor's elements lie in the binary chunk: "AT COUNT COMPONENT_TYPE COMPONENTS STRIDE END NORMALIZED",
-- or NULL when they don't lie there plainly (no buffer view, another buffer, sparse).
CREATE FUNCTION drops.glb_acc(j jsonb, i int) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN a IS NULL OR v IS NULL OR a ? 'sparse' OR coalesce((v->>'buffer')::int, 0) <> 0
              OR jsonb_typeof(a->'count') <> 'number' OR jsonb_typeof(a->'componentType') <> 'number' THEN NULL
    ELSE concat_ws(' ', coalesce((v->>'byteOffset')::bigint, 0) + coalesce((a->>'byteOffset')::bigint, 0), (a->>'count')::bigint,
                   (a->>'componentType')::int,
                   CASE a->>'type' WHEN 'SCALAR' THEN 1 WHEN 'VEC2' THEN 2 WHEN 'VEC3' THEN 3 WHEN 'VEC4' THEN 4 ELSE 0 END,
                   coalesce((v->>'byteStride')::int, 0),
                   coalesce((v->>'byteOffset')::bigint, 0) + coalesce((v->>'byteLength')::bigint, 0),
                   CASE WHEN coalesce((a->>'normalized')::bool, false) THEN 1 ELSE 0 END) END
  FROM (SELECT j->'accessors'->i AS a) x
  LEFT JOIN LATERAL (SELECT j->'bufferViews'->((a->>'bufferView')::int) AS v WHERE jsonb_typeof(a->'bufferView') = 'number') y ON true $$;
-- The images a model carries in its binary chunk, PNG or JPEG: where each lies.
CREATE FUNCTION drops.glb_images(j jsonb) RETURNS TABLE (image int, at bigint, len bigint) LANGUAGE sql IMMUTABLE AS $$
  SELECT (k - 1)::int, coalesce((v->>'byteOffset')::bigint, 0), (v->>'byteLength')::bigint
  FROM jsonb_array_elements(coalesce(j->'images', '[]')) WITH ORDINALITY AS z(im, k)
  CROSS JOIN LATERAL (SELECT j->'bufferViews'->((im->>'bufferView')::int) AS v) y
  WHERE jsonb_typeof(im->'bufferView') = 'number' AND v IS NOT NULL AND coalesce((v->>'buffer')::int, 0) = 0
    AND coalesce(im->>'mimeType', '') IN ('image/png', 'image/jpeg') AND jsonb_typeof(v->'byteLength') = 'number'
  ORDER BY k $$;
-- One row per triangle list the scene draws (at most 4096): its world transform (16 numbers), its positions,
-- normals, uvs and indices (drops.glb_acc; normals, uvs and indices may be NULL), its colour (the material's
-- base colour, 4 numbers) and the image its base colour texture shows (-1 none).
CREATE FUNCTION drops.glb_plan(j jsonb) RETURNS TABLE (m text, pos text, nrm text, uv text, idx text, colour text, image int)
LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE roots int[]; r int; d record; p jsonb; mat jsonb; tex jsonb; img int; c jsonb; n int := 0;
        id float8[] := ARRAY[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
BEGIN
  IF jsonb_typeof(j) <> 'object' THEN RETURN; END IF;
  SELECT array_agg(e::text::int) INTO roots
  FROM jsonb_array_elements(coalesce(j->'scenes'->(coalesce((j->>'scene')::int, 0))->'nodes', '[]')) e WHERE jsonb_typeof(e) = 'number';
  IF roots IS NULL THEN   -- no scene: every node no other node holds
    SELECT array_agg(k - 1) INTO roots FROM jsonb_array_elements(coalesce(j->'nodes', '[]')) WITH ORDINALITY AS z(nd, k)
    WHERE NOT EXISTS (SELECT 1 FROM jsonb_array_elements(j->'nodes') o, jsonb_array_elements(coalesce(o->'children', '[]')) ch
                      WHERE jsonb_typeof(ch) = 'number' AND ch::text::int = k - 1);
  END IF;
  FOREACH r IN ARRAY coalesce(roots, '{}') LOOP
    FOR d IN SELECT * FROM drops.glb_walk(j, r, id, 0) LOOP
      FOR p IN SELECT * FROM jsonb_array_elements(coalesce(j->'meshes'->d.o_mesh->'primitives', '[]')) LOOP
        IF coalesce((p->>'mode')::int, 4) <> 4 OR jsonb_typeof(p#>'{attributes,POSITION}') <> 'number' THEN CONTINUE; END IF;
        mat := CASE WHEN jsonb_typeof(p->'material') = 'number' THEN j->'materials'->((p->>'material')::int) END;
        c := coalesce(mat#>'{pbrMetallicRoughness,baseColorFactor}', '[1, 1, 1, 1]');
        tex := mat#>'{pbrMetallicRoughness,baseColorTexture,index}';
        img := CASE WHEN jsonb_typeof(tex) = 'number' AND jsonb_typeof(j->'textures'->(tex::text::int)->'source') = 'number'
                    THEN (j->'textures'->(tex::text::int)->>'source')::int ELSE -1 END;
        m := array_to_string(d.o_m, ' ');
        pos := drops.glb_acc(j, (p#>>'{attributes,POSITION}')::int);
        nrm := CASE WHEN jsonb_typeof(p#>'{attributes,NORMAL}') = 'number' THEN drops.glb_acc(j, (p#>>'{attributes,NORMAL}')::int) END;
        uv := CASE WHEN jsonb_typeof(p#>'{attributes,TEXCOORD_0}') = 'number' THEN drops.glb_acc(j, (p#>>'{attributes,TEXCOORD_0}')::int) END;
        idx := CASE WHEN jsonb_typeof(p->'indices') = 'number' THEN drops.glb_acc(j, (p->>'indices')::int) END;
        colour := concat_ws(' ', drops.glb_num(c, 0, 1), drops.glb_num(c, 1, 1), drops.glb_num(c, 2, 1), drops.glb_num(c, 3, 1));
        image := img;
        IF pos IS NULL OR (p ? 'indices' AND idx IS NULL) THEN CONTINUE; END IF;
        RETURN NEXT;
        n := n + 1;
        IF n >= 4096 THEN RETURN; END IF;
      END LOOP;
    END LOOP;
  END LOOP;
END $$;

-- The realm's pack and page are up (and what it couldn't place).
CREATE FUNCTION drops.realm_done(slug_ text, note text) RETURNS bool LANGUAGE sql VOLATILE AS $$
  UPDATE drops.projects SET realm_ms = coalesce(realm_ms, drops.now_ms()), realm_note = nullif(drops.one_line(note, 1000), '')
  WHERE slug = slug_ RETURNING true $$;

-- The cover the draft picked from the drop, where it is on the site (or NULL).
CREATE FUNCTION drops.cover(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '/' || f.dest FROM drops.projects p JOIN drops.drafts x ON x.drop_id = p.drop_id
  JOIN drops.files df ON df.drop_id = p.drop_id AND df.status = 'scanned' AND df.kind = 'image' AND df.name = x.draft#>>'{card,cover}'
  JOIN drops.publish_files(p.slug) f ON f.object = df.object
  WHERE p.slug = slug_ LIMIT 1 $$;
-- A page's link preview: its title, line and the drop's cover (as og:image) when it has one.
CREATE FUNCTION drops.preview(slug_ text, title text, path text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="' || drops.html(title) || '">
<meta property="og:description" content="' || drops.html(p.line) || '">
<meta property="og:url" content="' || drops.html(origin || path) || '">
' || coalesce('<meta property="og:image" content="' || drops.html(origin || drops.cover(slug_)) || '">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:image" content="' || drops.html(origin || drops.cover(slug_)) || '">
', '<meta name="twitter:card" content="summary">
') FROM drops.projects p WHERE p.slug = slug_ $$;

-- The realm's page: the first-realm module's files, named by their content anew each release, are taken
-- from /realm/drop (a page every build makes), with this realm's pack, slot and name in their place.
CREATE FUNCTION drops.realm_page(slug_ text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || drops.html(p.name) || ' · a first realm · OpenAgents</title>
<meta name="description" content="' || drops.html(p.line) || '">
' || drops.preview(slug_, p.name || ': a first realm', '/projects/' || p.slug || '/realm', origin) || '<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/projects/' || p.slug || '/realm/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
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
<p><a href="/projects/' || p.slug || '/realm/index.md">A first realm</a></p>
</main>
<script type="module" data-realm-of="' || p.slug || '" data-name="' || drops.html(p.name) || '">
const me = document.querySelector("script[data-realm-of]"), slug = me.dataset.realmOf;
const t = await (await fetch("/realm/drop", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.realm = slug; s.dataset.slot = "project-" + slug;
s.dataset.assets = "pack:/projects/" + slug + "/realm/realm.pack";
s.dataset.text = me.dataset.name + "\n/projects/" + slug;
document.body.append(s);
</script>
</body>
</html>
' FROM drops.projects p WHERE p.slug = slug_ $$;

CREATE FUNCTION drops.realm_md(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || p.name || E': a first realm\n\n'
    || 'The 3D models dropped for [' || p.name || '](/projects/' || p.slug || E'), stood on an empty floor so the project has somewhere to walk on day one. '
    || E'You carry a lamp among them: W and S walk, A and D turn, a drag turns; on a phone a thumb drags to walk and turn.\n\n'
    || coalesce(E'## Standing here\n\n' || (SELECT string_agg('- ' || m.file || ' (' || m.licence || ')', E'\n') FROM drops.realm_models(p.slug) m) || E'\n\n', '')
    || coalesce(E'## Not placed\n\n' || p.realm_note || E'\n\n', '')
    || E'Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.\n'
  FROM drops.projects p WHERE p.slug = slug_ $$;

-- The project's events name the realm when the drop has a model: ["realm", "/projects/<slug>/realm"] on its 30621.
CREATE OR REPLACE FUNCTION drops.publish_next(slug_ text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
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
    IF drops.has_realm(p.slug) THEN tags := tags || drops.tag('realm', '/projects/' || p.slug || '/realm'); END IF;
  ELSIF i.item = 'order' THEN
    tags := ARRAY[drops.tag('d', p.slug), drops.tag('a', pc, relay)];
    FOR i IN SELECT * FROM drops.project_items WHERE slug = slug_ AND kind = 1621 ORDER BY seq LOOP
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

-- The project's index.md links its realm.
CREATE OR REPLACE FUNCTION drops.publish_md(slug_ text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || p.name || E'\n\n' || p.line || E'\n\n'
    || 'An OpenAgents project made from a drop. Its tasks are on Nostr: ' || (drops.repo()->>'relay') || ', kind 30621 by '
    || p.pubkey || ', d ' || p.slug || E'.\n\n'
    || CASE WHEN drops.has_realm(p.slug) THEN 'Walk its first realm, the dropped models on an empty floor: [/projects/' || p.slug || '/realm](/projects/' || p.slug || E'/realm).\n\n' ELSE '' END
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

-- The project's page shows the drop's cover in its link preview (the hidden HTML stays the name and one link);
-- origin is this environment's (https://openagents.com), as a preview's links are whole.
CREATE FUNCTION drops.publish_page(slug_ text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1">
<title>' || drops.html(p.name) || ' · OpenAgents project</title>
<meta name="description" content="' || drops.html(p.line) || '">
' || drops.preview(slug_, p.name, '/projects/' || p.slug, origin) || '<meta name="theme-color" content="#000000">
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
<script type="module" data-project="' || p.pubkey || ' ' || p.slug || '">
// The engine''s files are named by their content (each release names them anew): this page takes the
// project app''s script from a page every build makes, /projects/coax, and names its own project.
const t = await (await fetch("/projects/coax", { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.text = document.currentScript?.dataset.project || document.querySelector("script[data-project]").dataset.project;
document.body.append(s);
</script>
</body>
</html>
' FROM drops.projects p WHERE p.slug = slug_ $$;
