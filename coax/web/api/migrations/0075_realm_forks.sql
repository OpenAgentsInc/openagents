-- FORK on a realm (ev-fork-realm; docs/verse/realm-fork.md): anyone takes a copy of a built-in realm
-- with their own rules. A fork is a made realm (verse.made_realms, world made-<slug>, owned by the
-- player key or the account, its page in the site's bucket, its card and line as 0071 made them)
-- whose `base` is the built-in realm whose module plays it. The original never changes: a fork
-- writes only its own row.
--
-- Its board is the base's rows (realm_rights.rows: what the base's code switches) and the rule
-- sets the base's code implements (verse.fork_bases.rules), each a row feature
-- {"feature": "row", "variant": ID, "label", "says", "rule"}. The shared edit layer switches them
-- (/switch) and talking revises them (/revise, under a schema whose enum is the fork's row ids);
-- the base's module reads its rules from the page facts (rules=...) and then from the board.
--
--   GET  /api/playtest/realms/<slug>/fork            text, a line each: base, title, fork 0|1, from
--        SLUG|NAME, maker NAME, name NAME, owner 0|1, published 0|1, rule ID 0|1 LABEL; 404 not forkable
--   POST /api/playtest/realms/<slug>/fork {rules: [ID...], name?}   -> 200 {slug, path, name, base, from, rules, page}
--        400 an unknown rule or a bad name, 401 no player, 404 not forkable, 429 today's realms
--   POST /api/playtest/realms/<fork>/publish {published?, description?}   -> 200 {published, description}; 403, 404
-- A fork's page is noindex and left off /og's rows until its owner publishes it.
-- Additive: three columns, a table, renamed functions behind dispatchers, new functions.

ALTER TABLE verse.made_realms ADD COLUMN base text;          -- the built-in realm whose module plays it; NULL: a builder realm
ALTER TABLE verse.made_realms ADD COLUMN forked_from text;   -- the realm it was forked from (the base, or another fork)
ALTER TABLE verse.made_realms ADD COLUMN published_ms bigint;  -- its owner published it: indexed and listed

-- The built-in realms that can be forked, and the rule sets their code implements: [{id, label, says}].
CREATE TABLE verse.fork_bases (
  slug text PRIMARY KEY,
  title text NOT NULL,
  style text NOT NULL,
  ground text NOT NULL DEFAULT 'forest',
  rules jsonb NOT NULL DEFAULT '[]'
);
INSERT INTO verse.fork_bases (slug, title, style, ground, rules) VALUES ('everglade', 'Everglade', 'painterly', 'forest', '[
  {"id": "night", "label": "Always night", "says": "the clock is held at midnight: it is always night"},
  {"id": "waves", "label": "Hostile waves", "says": "the town watch comes for you in waves from the gates, stronger each time"},
  {"id": "nodestroy", "label": "No destruction", "says": "spells and the hammer break nothing: the town and its people stand"},
  {"id": "lowgrav", "label": "Low gravity", "says": "gravity is a third of normal: you jump high and fall slowly"}
]');

-- A row feature's key is its id; every other feature's is as before (0050).
CREATE OR REPLACE FUNCTION verse.edit_key(f jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN f->>'feature' = 'row' THEN f->>'variant'
              ELSE (f->>'feature') || coalesce(':' || CASE WHEN f->>'feature' = 'character' THEN regexp_replace(lower(f->>'name'), '[^a-z0-9]+', '-', 'g')
                                                          WHEN f->>'feature' = 'props' THEN f->>'variant' END, '') END $$;

CREATE FUNCTION verse.fork_of(s text) RETURNS verse.made_realms LANGUAGE sql STABLE AS $$
  SELECT * FROM verse.made_realms WHERE slug = s AND base IS NOT NULL $$;
-- The base a realm forks from: a built-in one itself, or a fork's base. NULL: not forkable.
CREATE FUNCTION verse.fork_base_of(s text) RETURNS verse.fork_bases LANGUAGE sql STABLE AS $$
  SELECT b.* FROM verse.fork_bases b WHERE b.slug = coalesce((SELECT base FROM verse.made_realms WHERE slug = s AND base IS NOT NULL), s) $$;
-- Who to credit for a built-in realm: its maker (makers.realms, 0072), else us.
CREATE FUNCTION verse.fork_maker(base text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT name FROM makers.realms WHERE realm = base), 'OpenAgents') $$;
CREATE FUNCTION verse.fork_name_of(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT name FROM verse.made_realms WHERE slug = s), (SELECT title FROM verse.fork_bases WHERE slug = s), s) $$;
-- The rules on in a fork, in the base's order.
CREATE FUNCTION verse.fork_rules_on(m verse.made_realms) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT coalesce(array_agg(r->>'id' ORDER BY o), '{}') FROM verse.fork_bases b, jsonb_array_elements(b.rules) WITH ORDINALITY AS t(r, o)
  WHERE b.slug = m.base AND EXISTS (SELECT 1 FROM jsonb_array_elements(m.features) f WHERE f->>'feature' = 'row' AND f->>'variant' = r->>'id') $$;

-- GET .../fork: what a FORK pane shows.
CREATE FUNCTION verse.fork_info(s text, k text, site_admin boolean) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE
  b verse.fork_bases := verse.fork_base_of(s);
  m verse.made_realms := verse.fork_of(s);
  on_ text[];
BEGIN
  IF b.slug IS NULL THEN RETURN NULL; END IF;
  on_ := CASE WHEN m.slug IS NOT NULL THEN verse.fork_rules_on(m) ELSE '{}' END;
  RETURN 'base ' || b.slug || E'\ntitle ' || b.title || E'\nfork ' || CASE WHEN m.slug IS NULL THEN '0' ELSE '1' END || E'\n'
      || CASE WHEN m.slug IS NULL THEN '' ELSE 'from ' || m.forked_from || '|' || verse.fork_name_of(m.forked_from) || E'\n' END
      || 'maker ' || verse.fork_maker(b.slug) || E'\nname ' || coalesce(m.name, b.title) || E'\n'
      || 'owner ' || CASE WHEN m.slug IS NOT NULL AND verse.edit_may(s, k, site_admin) THEN '1' ELSE '0' END || E'\n'
      || 'published ' || CASE WHEN m.published_ms IS NOT NULL THEN '1' ELSE '0' END || E'\n'
      || coalesce((SELECT string_agg('rule ' || (r->>'id') || ' ' || CASE WHEN (r->>'id') = ANY (on_) THEN '1' ELSE '0' END || ' ' || (r->>'label'), E'\n' ORDER BY o)
                   FROM jsonb_array_elements(b.rules) WITH ORDINALITY AS t(r, o)) || E'\n', '');
END $$;

-- FORK: a made realm whose base is this one's, with the rules chosen. -> {status, slug, path, ...} | {status, error}
CREATE FUNCTION verse.fork_make(s text, body jsonb, k text, per_day int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  b verse.fork_bases := verse.fork_base_of(s);
  src verse.made_realms := verse.fork_of(s);
  asked jsonb := CASE WHEN jsonb_typeof(body) = 'object' AND jsonb_typeof(body->'rules') = 'array' THEN body->'rules' ELSE '[]' END;
  rules text[];
  bad text;
  nm text;
  rows jsonb;
  slug_ text;
BEGIN
  IF b.slug IS NULL THEN RETURN '{"status":404,"error":"this realm can''t be forked"}'; END IF;
  IF rec.id IS NULL THEN RETURN '{"status":401,"error":"no player"}'; END IF;
  IF jsonb_array_length(asked) > 16 THEN RETURN '{"status":400,"error":"too many rules"}'; END IF;
  SELECT y.x INTO bad FROM jsonb_array_elements(asked) a(v), LATERAL (SELECT CASE WHEN jsonb_typeof(a.v) = 'string' THEN a.v #>> '{}' ELSE '?' END x) y
  WHERE NOT EXISTS (SELECT 1 FROM jsonb_array_elements(b.rules) r(v) WHERE r.v->>'id' = y.x) LIMIT 1;
  IF bad IS NOT NULL THEN RETURN jsonb_build_object('status', 400, 'error', 'not a rule here: ' || left(regexp_replace(bad, '[^a-z0-9-]', '', 'g'), 24)); END IF;
  rules := ARRAY(SELECT DISTINCT a.v #>> '{}' FROM jsonb_array_elements(asked) a(v));
  IF jsonb_typeof(body->'name') = 'string' THEN
    nm := btrim(regexp_replace(verse.builder_one_line(body->'name', 40), '[<>&"]', '', 'g'));
    IF (body->>'name') <> '' AND nm = '' THEN RETURN '{"status":400,"error":"a name of plain words"}'; END IF;
  END IF;
  IF (SELECT count(*) FROM verse.made_realms m JOIN verse.worlds w ON w.id = m.world
      WHERE w.owner_player = rec.id AND m.made_ms > verse.now_ms() - 86400000) >= per_day THEN
    RETURN '{"status":429,"error":"that''s today''s realms"}';
  END IF;
  nm := coalesce(nullif(nm, ''), left(verse.fork_name_of(s), 34) || ' Fork');
  -- the board: the base's rows as they are now (or the fork's), then its rules, on as chosen
  IF src.slug IS NOT NULL THEN
    rows := coalesce((SELECT jsonb_agg(f || jsonb_build_object('on', true)) FROM jsonb_array_elements(src.features) f WHERE NOT coalesce((f->>'rule')::boolean, false)), '[]')
         || coalesce((SELECT jsonb_agg(f || jsonb_build_object('on', false)) FROM jsonb_array_elements(src.off) f WHERE NOT coalesce((f->>'rule')::boolean, false)), '[]');
  ELSE
    rows := coalesce((SELECT jsonb_agg(jsonb_build_object('feature', 'row', 'variant', r->>'id', 'label', r->>'label', 'says', r->>'says', 'rule', false,
                                                          'on', coalesce((e.switched->>(r->>'id'))::boolean, true)) ORDER BY o)
                      FROM verse.realm_rights h LEFT JOIN verse.realm_edit e ON e.slug = h.slug, jsonb_array_elements(h.rows) WITH ORDINALITY AS t(r, o)
                      WHERE h.slug = b.slug), '[]');
  END IF;
  rows := rows || coalesce((SELECT jsonb_agg(jsonb_build_object('feature', 'row', 'variant', r->>'id', 'label', r->>'label', 'says', r->>'says', 'rule', true,
                                                                'on', (r->>'id') = ANY (rules)) ORDER BY o)
                            FROM jsonb_array_elements(b.rules) WITH ORDINALITY AS t(r, o)), '[]');
  slug_ := verse.builder_slug(nm);
  INSERT INTO verse.worlds (id, owner_player, owner_account, created_ms) VALUES ('made-' || slug_, rec.id, rec.account, verse.now_ms());
  INSERT INTO verse.made_realms (slug, world, name, style, ground, time, camera, features, off, tools, made_ms, base, forked_from)
  VALUES (slug_, 'made-' || slug_, nm, b.style, b.ground, CASE WHEN 'night' = ANY (rules) THEN 'night' ELSE 'day' END, 'third',
          coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(rows) f WHERE (f->>'on')::boolean), '[]'),
          coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(rows) f WHERE NOT (f->>'on')::boolean), '[]'),
          coalesce(src.tools, (SELECT jsonb_object_agg(t, t = 'map') FROM unnest(verse.builder_tools()) t)), verse.now_ms(), b.slug, s);
  -- the city as it is published now (Everglade's edits, or the fork's), the fork's own from here on
  INSERT INTO verse.published (world, doc, version, published_by, published_at)
  SELECT 'made-' || slug_, p.doc, 1, 'fork of ' || s, extract(epoch FROM clock_timestamp())
  FROM verse.published p WHERE p.world = CASE WHEN src.slug IS NOT NULL THEN 'made-' || s ELSE b.slug END;
  RETURN jsonb_build_object('status', 200, 'slug', slug_, 'path', '/realms/' || slug_, 'name', nm, 'base', b.slug, 'from', s,
                            'rules', to_jsonb(verse.fork_rules_on(verse.fork_of(slug_))));
END $$;

-- PUBLISH: its owner (or a site admin) lists it, with its own line if they send one.
CREATE FUNCTION verse.fork_publish(s text, body jsonb, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms := verse.fork_of(s);
  pub boolean := CASE WHEN jsonb_typeof(body->'published') = 'boolean' THEN (body->>'published')::boolean ELSE true END;
BEGIN
  IF m.slug IS NULL THEN RETURN '{"status":404,"error":"no such fork"}'; END IF;
  IF NOT verse.edit_may(s, k, site_admin) THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  UPDATE verse.made_realms SET
    published_ms = CASE WHEN pub THEN coalesce(published_ms, verse.now_ms()) END,
    description = CASE WHEN jsonb_typeof(body->'description') = 'string'
                       THEN nullif(btrim(regexp_replace(verse.builder_one_line(body->'description', 200), '[<>]', '', 'g')), '') ELSE description END
  WHERE slug = s RETURNING * INTO m;
  RETURN jsonb_build_object('status', 200, 'published', m.published_ms IS NOT NULL, 'description', verse.builder_share_line(m));
END $$;

-- A fork's line when its owner wrote none: what it forks and its rules.
CREATE FUNCTION verse.fork_line(m verse.made_realms) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT 'A fork of ' || verse.fork_name_of(m.forked_from) || ' on the Verse'
      || CASE WHEN m.forked_from <> m.base THEN ' (from ' || b.title || ' by ' || verse.fork_maker(m.base) || ')' ELSE ' by ' || verse.fork_maker(m.base) END
      || coalesce(': ' || lower(nullif((SELECT string_agg(r->>'label', ', ' ORDER BY o) FROM jsonb_array_elements(b.rules) WITH ORDINALITY AS t(r, o)
                                  WHERE (r->>'id') = ANY (verse.fork_rules_on(m))), '')) || '.', ', with the same rules.')
  FROM verse.fork_bases b WHERE b.slug = m.base $$;
CREATE OR REPLACE FUNCTION verse.builder_share_line(m verse.made_realms) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(nullif(btrim(m.description), ''), CASE WHEN m.base IS NOT NULL THEN verse.fork_line(m) ELSE verse.builder_line_of(m) END) $$;

-- A fork's page: the preview tags as a made realm's (0071), what it forks (a link) and the base's
-- module (its page's script, every build names its files) with this fork's slot, slug and rules.
CREATE FUNCTION verse.fork_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,viewport-fit=cover">
<title>' || verse.builder_html(m.name) || ' · a fork of ' || verse.builder_html(b.title) || ' · OpenAgents</title>
' || CASE WHEN m.published_ms IS NULL THEN '<meta name="robots" content="noindex">
' ELSE '' END || '<meta name="description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta property="og:type" content="website">
<meta property="og:site_name" content="OpenAgents">
<meta property="og:title" content="' || verse.builder_html(m.name) || '">
<meta property="og:description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta property="og:url" content="' || verse.builder_html(origin) || '/realms/' || m.slug || '">
<meta property="og:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.builder_card_path(m)) || '">
<meta property="og:image:width" content="1200">
<meta property="og:image:height" content="630">
<meta property="og:image:alt" content="' || verse.builder_html(m.name) || ', a fork of ' || verse.builder_html(b.title) || ' on the Verse">
<meta name="twitter:card" content="summary_large_image">
<meta name="twitter:title" content="' || verse.builder_html(m.name) || '">
<meta name="twitter:description" content="' || verse.builder_html(verse.builder_share_line(m)) || '">
<meta name="twitter:image" content="' || verse.builder_html(origin) || verse.builder_html(verse.builder_card_path(m)) || '">
<meta name="theme-color" content="#000000">
<link rel="icon" type="image/svg+xml" href="/favicon.svg">
<link rel="alternate" type="text/markdown" href="/realms/' || m.slug || '/index.md">
<script>if ("gpu" in navigator) document.documentElement.classList.add("gpu")</script>
<style>
html,body{margin:0;height:100%;background:#000;color:#ededed}
body{overflow:hidden;font-family:ui-sans-serif,system-ui,-apple-system,"Segoe UI",Roboto,"Helvetica Neue",Arial,sans-serif;font-size:14px;line-height:1.5}
canvas{position:fixed;inset:0;width:100%;height:100%;touch-action:none;user-select:none;-webkit-user-select:none;-webkit-touch-callout:none}
canvas[data-cursor=pointer]{cursor:pointer}
main{position:relative;max-width:640px;margin:0 auto;padding:24px 16px}
html:not(.gpu) body{overflow:auto}
a{color:#ededed}
.gpu main{position:absolute;width:1px;height:1px;overflow:hidden;clip-path:inset(50%);white-space:nowrap;padding:0}
</style>
</head>
<body>
<canvas id="coaxial" aria-hidden="true"></canvas>
<main>
<h1>' || verse.builder_html(m.name) || '</h1>
<p>Forked from <a href="/realms/' || m.forked_from || '">' || verse.builder_html(verse.fork_name_of(m.forked_from)) || '</a>, '
    || CASE WHEN m.forked_from <> m.base THEN 'a fork of <a href="/realms/' || b.slug || '">' || verse.builder_html(b.title) || '</a> ' ELSE '' END
    || 'made by ' || verse.builder_html(verse.fork_maker(b.slug)) || '.</p>
<p>' || verse.builder_html(verse.builder_share_line(m)) || '</p>
<p><a href="/realms/' || m.slug || '/index.md">' || verse.builder_html(m.name) || '</a></p>
</main>
<script type="module" data-fork="' || m.slug || '" data-base="' || b.slug || '" data-rules="' || array_to_string(verse.fork_rules_on(m), ',') || '">
const me = document.querySelector("script[data-fork]"), slug = me.dataset.fork;
const t = await (await fetch("/realms/" + me.dataset.base, { cache: "no-store" })).text();
const from = new DOMParser().parseFromString(t, "text/html").querySelector("script[data-app]");
const s = document.createElement("script");
for (const a of from.attributes) s.setAttribute(a.name, a.value);
s.dataset.slot = "made-" + slug; s.dataset.fork = slug; s.dataset.rules = me.dataset.rules;
s.dataset.card = "/api/playtest/realms/" + slug + "/card";
document.body.append(s);
</script>
</body>
</html>
' FROM verse.made_realms m JOIN verse.fork_bases b ON b.slug = m.base WHERE m.slug = s $$;

CREATE FUNCTION verse.fork_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT '# ' || m.name || E'\n\n' || verse.builder_share_line(m)
      || E'\n\nA fork of ' || verse.fork_name_of(m.forked_from) || ' (https://openagents.com/realms/' || m.forked_from || ')'
      || CASE WHEN m.forked_from <> m.base THEN ', itself a fork of ' || b.title || ' (https://openagents.com/realms/' || b.slug || ')' ELSE '' END
      || ', made by ' || verse.fork_maker(b.slug) || ', at https://openagents.com/realms/' || m.slug
      || '. It plays ' || b.title || E' with its own rules; the original never changes.\n\n## Its rules\n\n'
      || coalesce((SELECT string_agg('- ' || (r->>'label'), E'\n' ORDER BY o) FROM jsonb_array_elements(b.rules) WITH ORDINALITY AS t(r, o)
                   WHERE (r->>'id') = ANY (verse.fork_rules_on(m))), '- The same rules as ' || b.title)
      || E'\n\nIts owner switches its rules and the rest of its board with EDIT, by a tap or by voice. Anyone can fork it again with FORK.\n'
  FROM verse.made_realms m JOIN verse.fork_bases b ON b.slug = m.base WHERE m.slug = s $$;

-- The page and the text: a fork's own, else a made realm's as 0071 wrote them.
ALTER FUNCTION verse.builder_page(text, text) RENAME TO builder_page_0071;
ALTER FUNCTION verse.builder_md(text) RENAME TO builder_md_0071;
CREATE FUNCTION verse.builder_page(s text, origin text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NOT NULL) THEN verse.fork_page(s, origin) ELSE verse.builder_page_0071(s, origin) END $$;
CREATE FUNCTION verse.builder_md(s text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM verse.made_realms WHERE slug = s AND base IS NOT NULL) THEN verse.fork_md(s) ELSE verse.builder_md_0071(s) END $$;

-- /og's rows (0071): a fork only once it is published.
CREATE OR REPLACE FUNCTION verse.builder_og_rows(n int) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(string_agg(
    '/realms/' || m.slug || '|made||' || verse.builder_line(to_jsonb(m.name), 120) || '|' || verse.builder_line(to_jsonb(verse.builder_share_line(m)), 240)
    || '|https://openagents.com/realms/' || m.slug || '|' || verse.builder_card_path(m) || '|' || verse.builder_line(to_jsonb(m.name), 120)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'default' ELSE 'made' END || '|1200|630|' || coalesce(m.card_kb, 0)
    || '|' || CASE WHEN m.card_ms IS NULL THEN 'no picture of its own yet (its owner''s next visit takes one)' ELSE '' END
    || '|' || CASE WHEN m.description IS NULL THEN 'line written from what is in it' ELSE '' END || '|', E'\n' ORDER BY m.made_ms DESC) || E'\n', '')
  FROM (SELECT * FROM verse.made_realms WHERE base IS NULL OR published_ms IS NOT NULL ORDER BY made_ms DESC LIMIT greatest(least(n, 200), 1)) m $$;

-- ---- talking revises a fork's board: its rows by id, as a hand-built realm's are (0050)
ALTER FUNCTION verse.edit_revise_ask(text, jsonb, int, text, boolean) RENAME TO edit_revise_ask_0073;

CREATE FUNCTION verse.fork_board(m verse.made_realms) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('id', f->>'variant', 'on', (f->>'on')::boolean)), '[]')
  FROM jsonb_array_elements(verse.edit_made_board(m)) f WHERE f->>'feature' = 'row' $$;

CREATE FUNCTION verse.edit_revise_ask(s text, body jsonb, cap int, k text, site_admin boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m verse.made_realms := verse.fork_of(s);
  role text := verse.edit_role(s, k, site_admin);
  said text := CASE WHEN jsonb_typeof(body) = 'object' THEN verse.npc_text(body->'prompt', 600) ELSE '' END;
  rec verse.players := verse.builder_player(k);
  board jsonb;
  row_id bigint;
  sys text;
  ids jsonb;
BEGIN
  IF m.slug IS NULL THEN RETURN verse.edit_revise_ask_0073(s, body, cap, k, site_admin); END IF;
  IF role NOT IN ('owner', 'admin') THEN RETURN '{"status":403,"error":"only its owner and admins"}'; END IF;
  IF said = '' THEN RETURN '{"status":400,"error":"say how it should change"}'; END IF;
  IF EXISTS (SELECT 1 FROM verse.realm_plans WHERE slug = s AND ms > verse.now_ms() - 2000)
     OR (rec.id IS NOT NULL AND EXISTS (SELECT 1 FROM verse.realm_plans WHERE player = rec.id AND ms > verse.now_ms() - 2000)) THEN
    RETURN '{"status":429,"error":"one at a time"}';
  END IF;
  IF (SELECT count(*) FROM verse.realm_plans WHERE ms > (extract(epoch FROM date_trunc('day', now())) * 1000)::bigint) >= cap THEN
    RETURN '{"status":503,"error":"the day''s plans are used up"}';
  END IF;
  board := jsonb_build_object('features', verse.fork_board(m));
  ids := (SELECT jsonb_agg(f->'variant') FROM jsonb_array_elements(verse.edit_made_board(m)) f WHERE f->>'feature' = 'row');
  sys := 'You edit a realm of the Verse, a 3D world in the browser, for its owner: their fork of ' || verse.fork_name_of(m.base)
      || ', whose rules and features they switch on and off. They said, out loud or typed, how they want it. '
      || 'Answer every feature with on true or false: change only what they asked about and keep the rest as it is now. '
      || E'If what they asked for is not one of these features, keep everything as it is.\nThe features:\n'
      || (SELECT string_agg('- ' || (f->>'variant') || ': ' || coalesce(f->>'says', f->>'label'), E'\n' ORDER BY o)
          FROM jsonb_array_elements(verse.edit_made_board(m)) WITH ORDINALITY AS t(f, o) WHERE f->>'feature' = 'row');
  INSERT INTO verse.realm_plans (ms, player, prompt, board_in, slug) VALUES (verse.now_ms(), rec.id, said, board, s) RETURNING id INTO row_id;
  RETURN verse.builder_fast(jsonb_build_object('status', 200, 'id', row_id, 'request', jsonb_build_object(
    'systemInstruction', jsonb_build_object('parts', jsonb_build_array(jsonb_build_object('text', sys))),
    'contents', jsonb_build_array(jsonb_build_object('role', 'user', 'parts', jsonb_build_array(jsonb_build_object('text',
      'The current board: ' || board::text || E'\nThey now say: ' || said)))),
    'generationConfig', jsonb_build_object('responseMimeType', 'application/json', 'temperature', 0.3, 'maxOutputTokens', 4000,
      'responseSchema', jsonb_build_object('type', 'object', 'required', jsonb_build_array('features'), 'properties', jsonb_build_object(
        'features', jsonb_build_object('type', 'array', 'items', jsonb_build_object('type', 'object', 'required', jsonb_build_array('id', 'on'),
          'properties', jsonb_build_object('id', jsonb_build_object('type', 'string', 'enum', ids), 'on', jsonb_build_object('type', 'boolean'))))))))));
END $$;

-- realms.c asks this one for every revision's answer: a fork's here, any other realm's by 0050's.
CREATE FUNCTION verse.fork_revise_answer(asked jsonb, http int, response text, model text, taken int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pid bigint := (asked->>'id')::bigint;
  s text := (SELECT slug FROM verse.realm_plans WHERE id = pid);
  m verse.made_realms := verse.fork_of(s);
  r jsonb;
  t text;
  d jsonb;
  err text;
  status int := 200;
  turn jsonb := '{}';
  changed int := 0;
  rows jsonb;
BEGIN
  IF m.slug IS NULL THEN RETURN verse.edit_revise_answer(asked, http, response, model, taken); END IF;
  BEGIN r := response::jsonb; EXCEPTION WHEN others THEN r := NULL; END;
  IF http = 200 AND r IS NOT NULL THEN
    SELECT string_agg(p->>'text', '') INTO t FROM jsonb_array_elements(coalesce(r->'candidates'->0->'content'->'parts', '[]')) p
    WHERE coalesce((p->>'thought')::boolean, false) = false AND p ? 'text';
    BEGIN d := t::jsonb; EXCEPTION WHEN others THEN d := NULL; END;
    IF d IS NULL THEN status := 503; err := 'no answer'; END IF;
  ELSE
    status := 503;
    err := CASE WHEN http = 429 THEN 'busy' WHEN http = 0 THEN 'model unreachable' ELSE 'model error ' || http END;
  END IF;
  IF status = 200 THEN
    rows := verse.edit_made_board(m);
    -- only the fork's own row ids, each with a boolean; the rest stay as they are
    SELECT coalesce(jsonb_object_agg(f->>'id', (f->>'on')::boolean), '{}') INTO turn
    FROM jsonb_array_elements(CASE WHEN jsonb_typeof(d->'features') = 'array' THEN d->'features' ELSE '[]' END) f
    WHERE jsonb_typeof(f->'on') = 'boolean' AND EXISTS (SELECT 1 FROM jsonb_array_elements(rows) b WHERE b->>'feature' = 'row' AND b->>'variant' = f->>'id');
    SELECT count(*) INTO changed FROM jsonb_array_elements(rows) b WHERE b->>'feature' = 'row' AND turn ? (b->>'variant')
      AND (turn->>(b->>'variant'))::boolean <> (b->>'on')::boolean;
    rows := (SELECT jsonb_agg(CASE WHEN b->>'feature' = 'row' AND turn ? (b->>'variant') THEN b || jsonb_build_object('on', (turn->>(b->>'variant'))::boolean) ELSE b END ORDER BY o)
             FROM jsonb_array_elements(rows) WITH ORDINALITY AS x(b, o));
    UPDATE verse.made_realms SET
      features = coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(rows) f WHERE (f->>'on')::boolean), '[]'),
      off = coalesce((SELECT jsonb_agg(f - 'on') FROM jsonb_array_elements(rows) f WHERE NOT (f->>'on')::boolean), '[]'),
      settings_ms = verse.now_ms()
    WHERE slug = s;
  END IF;
  UPDATE verse.realm_plans SET board = CASE WHEN status = 200 THEN jsonb_build_object('switched', turn) END, error = err,
    model = fork_revise_answer.model, ms_taken = taken WHERE id = pid;
  IF status <> 200 THEN RETURN jsonb_build_object('status', status, 'error', err); END IF;
  RETURN jsonb_build_object('status', 200, 'plan', pid, 'changed', changed, 'lines', 'plan ' || pid || E'\n' || verse.edit_rows(s));
END $$;
