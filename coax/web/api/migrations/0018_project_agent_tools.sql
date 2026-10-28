-- The project agent's document tools (docs/project-agent.md): milestone_edit,
-- goal_propose and document_patch become one new document version through
-- push, from the version the agent read, so oa-api's rules for the agent
-- (kept title, members and agreed goals; a running turn; the edit recorded
-- under the turn) hold for them as for every write. 0013's talk_lines is
-- defined again with the members and the proposed goals' agreements, which
-- landed after 0013 had run. Every function is CREATE OR REPLACE: this file
-- first shipped as 0016_project_agent_tools.sql, and staging and production
-- applied it under that name before the rename.

-- A JSON pointer as a path: "/days/0/milestones/-" → {days,0,milestones,-}.
CREATE OR REPLACE FUNCTION projects.pointer(p text) RETURNS text[] LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN p IS NULL OR left(p, 1) <> '/' THEN NULL
    ELSE (SELECT array_agg(replace(replace(x, '~1', '/'), '~0', '~') ORDER BY o) FROM unnest(string_to_array(substr(p, 2), '/')) WITH ORDINALITY AS u(x, o)) END
$$;

-- RFC 6902 add, remove, replace, move and test over the document; the result
-- is {"doc": ...} or {"error": ...}.
CREATE OR REPLACE FUNCTION projects.patch_apply(doc jsonb, ops jsonb) RETURNS jsonb LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  o jsonb; p text[]; f text[]; parent text[]; last text; v jsonb; holder jsonb; n int := 0;
BEGIN
  IF jsonb_typeof(ops) <> 'array' OR jsonb_array_length(ops) = 0 OR jsonb_array_length(ops) > 50 THEN
    RETURN '{"error":"ops is a list of 1 to 50 operations"}';
  END IF;
  FOR o IN SELECT * FROM jsonb_array_elements(ops) LOOP
    n := n + 1;
    p := projects.pointer(o->>'path');
    IF p IS NULL OR cardinality(p) = 0 THEN RETURN jsonb_build_object('error', 'operation ' || n || ': path is a JSON pointer inside the document, like /days/0/milestones/1/text'); END IF;
    parent := p[1:cardinality(p) - 1]; last := p[cardinality(p)];
    holder := CASE WHEN cardinality(parent) = 0 THEN doc ELSE doc #> parent END;
    IF o->>'op' = 'move' THEN
      f := projects.pointer(o->>'from');
      IF f IS NULL OR doc #> f IS NULL THEN RETURN jsonb_build_object('error', 'operation ' || n || ': nothing at from'); END IF;
      v := doc #> f; doc := doc #- f;
      holder := CASE WHEN cardinality(parent) = 0 THEN doc ELSE doc #> parent END;
      o := jsonb_build_object('op', 'add', 'path', o->'path', 'value', v);
    END IF;
    IF o->>'op' = 'test' THEN
      IF doc #> p IS DISTINCT FROM o->'value' THEN RETURN jsonb_build_object('error', 'operation ' || n || ': test failed: the document is not what you read'); END IF;
    ELSIF o->>'op' = 'remove' THEN
      IF doc #> p IS NULL THEN RETURN jsonb_build_object('error', 'operation ' || n || ': nothing at path'); END IF;
      doc := doc #- p;
    ELSIF o->>'op' = 'replace' THEN
      IF doc #> p IS NULL OR NOT o ? 'value' THEN RETURN jsonb_build_object('error', 'operation ' || n || ': replace needs a value at an existing path'); END IF;
      doc := jsonb_set(doc, p, o->'value', false);
    ELSIF o->>'op' = 'add' THEN
      IF holder IS NULL OR NOT o ? 'value' THEN RETURN jsonb_build_object('error', 'operation ' || n || ': add needs a value and an existing parent'); END IF;
      IF jsonb_typeof(holder) = 'array' THEN
        IF last = '-' OR last = jsonb_array_length(holder)::text THEN
          doc := CASE WHEN cardinality(parent) = 0 THEN doc || jsonb_build_array(o->'value')
                 ELSE jsonb_set(doc, parent, holder || jsonb_build_array(o->'value'), false) END;
        ELSIF last ~ '^[0-9]{1,6}$' AND last::int < jsonb_array_length(holder) THEN
          doc := jsonb_insert(doc, p, o->'value', false);
        ELSE RETURN jsonb_build_object('error', 'operation ' || n || ': index out of range'); END IF;
      ELSIF jsonb_typeof(holder) = 'object' THEN
        doc := jsonb_set(doc, p, o->'value', true);
      ELSE RETURN jsonb_build_object('error', 'operation ' || n || ': the parent is not a list or an object'); END IF;
    ELSE
      RETURN jsonb_build_object('error', 'operation ' || n || ': op is add, remove, replace, move or test');
    END IF;
  END LOOP;
  RETURN jsonb_build_object('doc', doc);
END $$;

-- The days kept in date order, with a day made for a date that has none.
CREATE OR REPLACE FUNCTION projects.with_day(doc jsonb, day text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(doc->'days', '[]')) d WHERE d->>'date' = day) THEN doc
    ELSE jsonb_set(doc, '{days}', (SELECT jsonb_agg(d ORDER BY d->>'date') FROM jsonb_array_elements(
           coalesce(doc->'days', '[]') || jsonb_build_array(jsonb_build_object('date', day, 'milestones', '[]'::jsonb))) d), true) END
$$;

-- A fresh id for a new item, with a prefix.
CREATE OR REPLACE FUNCTION projects.fresh_id(prefix text) RETURNS text LANGUAGE sql VOLATILE AS $$
  SELECT prefix || '-' || substr(md5(random()::text || clock_timestamp()::text), 1, 8)
$$;

-- milestone_edit: add, reword, move or carry over a milestone, or add or
-- reword a backlog (checklist) item. {"doc": ...} or {"error": ...}.
CREATE OR REPLACE FUNCTION projects.milestone_edit(doc jsonb, body jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE
  act text := coalesce(body->>'action', '');
  mid text := nullif(btrim(coalesce(body->>'id', '')), '');
  txt text := nullif(btrim(coalesce(body->>'text', '')), '');
  day text := nullif(btrim(coalesce(body->>'date', '')), '');
  list text := nullif(btrim(coalesce(body->>'checklist', '')), '');
  m jsonb; from_day text;
BEGIN
  IF day IS NOT NULL AND day !~ '^\d{4}-\d{2}-\d{2}$' THEN RETURN '{"error":"date is YYYY-MM-DD"}'; END IF;
  IF txt IS NOT NULL AND length(txt) > 500 THEN RETURN '{"error":"text is at most 500 characters"}'; END IF;
  IF act = 'add' THEN
    IF txt IS NULL THEN RETURN '{"error":"add needs text"}'; END IF;
    IF list IS NOT NULL THEN
      IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) c WHERE c->>'id' = list) THEN
        RETURN '{"error":"no checklist has that id"}'; END IF;
      m := jsonb_build_object('id', projects.fresh_id('i'), 'text', txt, 'status', 'todo');
      RETURN jsonb_build_object('doc', jsonb_set(doc, '{checklists}', (SELECT jsonb_agg(CASE WHEN c->>'id' = list
               THEN jsonb_set(c, '{items}', coalesce(c->'items', '[]') || jsonb_build_array(m)) ELSE c END ORDER BY i)
             FROM jsonb_array_elements(doc->'checklists') WITH ORDINALITY AS z(c, i))), 'id', m->>'id');
    END IF;
    IF day IS NULL THEN RETURN '{"error":"add needs a date (a milestone) or a checklist (a backlog item)"}'; END IF;
    m := jsonb_build_object('id', projects.fresh_id('m'), 'text', txt, 'status', 'todo')
         || CASE WHEN nullif(btrim(coalesce(body->>'owner', '')), '') IS NOT NULL THEN jsonb_build_object('owner', btrim(body->>'owner')) ELSE '{}' END;
    doc := projects.with_day(doc, day);
    RETURN jsonb_build_object('doc', jsonb_set(doc, '{days}', (SELECT jsonb_agg(CASE WHEN d->>'date' = day
             THEN jsonb_set(d, '{milestones}', coalesce(d->'milestones', '[]') || jsonb_build_array(m)) ELSE d END ORDER BY i)
           FROM jsonb_array_elements(doc->'days') WITH ORDINALITY AS z(d, i))), 'id', m->>'id');
  END IF;
  IF mid IS NULL THEN RETURN '{"error":"id is the milestone or item to change"}'; END IF;
  IF act = 'reword' THEN
    IF txt IS NULL THEN RETURN '{"error":"reword needs text"}'; END IF;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(doc->'days', '[]')) d, jsonb_array_elements(coalesce(d->'milestones', '[]')) x WHERE x->>'id' = mid)
       AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) c, jsonb_array_elements(coalesce(c->'items', '[]')) x WHERE x->>'id' = mid) THEN
      RETURN '{"error":"no milestone or item has that id"}'; END IF;
    doc := jsonb_set(doc, '{days}', coalesce((SELECT jsonb_agg(jsonb_set(d, '{milestones}', (SELECT coalesce(jsonb_agg(CASE WHEN x->>'id' = mid
             THEN x || jsonb_build_object('text', txt) ELSE x END ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(d->'milestones', '[]')) WITH ORDINALITY AS y(x, o))) ORDER BY i)
           FROM jsonb_array_elements(coalesce(doc->'days', '[]')) WITH ORDINALITY AS z(d, i)), '[]'), true);
    doc := jsonb_set(doc, '{checklists}', coalesce((SELECT jsonb_agg(jsonb_set(c, '{items}', (SELECT coalesce(jsonb_agg(CASE WHEN x->>'id' = mid
             THEN x || jsonb_build_object('text', txt) ELSE x END ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(c->'items', '[]')) WITH ORDINALITY AS y(x, o))) ORDER BY i)
           FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) WITH ORDINALITY AS z(c, i)), '[]'), true);
    RETURN jsonb_build_object('doc', doc);
  END IF;
  IF act IN ('move', 'carry') THEN
    IF day IS NULL THEN RETURN '{"error":"move and carry need the date to move it to"}'; END IF;
    SELECT x, d->>'date' INTO m, from_day FROM jsonb_array_elements(coalesce(doc->'days', '[]')) d, jsonb_array_elements(coalesce(d->'milestones', '[]')) x
      WHERE x->>'id' = mid LIMIT 1;
    IF m IS NULL THEN RETURN '{"error":"no milestone has that id"}'; END IF;
    IF act = 'carry' THEN m := m || jsonb_build_object('note', btrim(coalesce(m->>'note', '') || ' carried over from ' || from_day)); END IF;
    doc := jsonb_set(doc, '{days}', (SELECT jsonb_agg(jsonb_set(d, '{milestones}', (SELECT coalesce(jsonb_agg(x ORDER BY o), '[]')
             FROM jsonb_array_elements(coalesce(d->'milestones', '[]')) WITH ORDINALITY AS y(x, o) WHERE x->>'id' <> mid)) ORDER BY i)
           FROM jsonb_array_elements(doc->'days') WITH ORDINALITY AS z(d, i)));
    doc := projects.with_day(doc, day);
    RETURN jsonb_build_object('doc', jsonb_set(doc, '{days}', (SELECT jsonb_agg(CASE WHEN d->>'date' = day
             THEN jsonb_set(d, '{milestones}', coalesce(d->'milestones', '[]') || jsonb_build_array(m)) ELSE d END ORDER BY i)
           FROM jsonb_array_elements(doc->'days') WITH ORDINALITY AS z(d, i))));
  END IF;
  RETURN '{"error":"action is add, reword, move or carry"}';
END $$;

-- goal_propose: a new goal, or a change to one, as proposed. A change to an
-- agreed goal is a new proposed goal that names it in "replaces"; the agreed
-- goal stays until the members agree.
CREATE OR REPLACE FUNCTION projects.goal_propose(doc jsonb, body jsonb) RETURNS jsonb LANGUAGE plpgsql VOLATILE AS $$
DECLARE
  gid text := nullif(btrim(coalesce(body->>'id', '')), '');
  txt text := nullif(btrim(coalesce(body->>'text', '')), '');
  g jsonb; nid text;
BEGIN
  IF txt IS NULL OR length(txt) > 500 THEN RETURN '{"error":"text is 1 to 500 characters"}'; END IF;
  IF gid IS NOT NULL THEN
    SELECT x INTO g FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) x WHERE x->>'id' = gid;
    IF g IS NULL THEN RETURN '{"error":"no goal has that id"}'; END IF;
    IF g->>'status' = 'proposed' THEN
      RETURN jsonb_build_object('doc', jsonb_set(doc, '{goals}', (SELECT jsonb_agg(CASE WHEN x->>'id' = gid THEN x || jsonb_build_object('text', txt) ELSE x END ORDER BY i)
               FROM jsonb_array_elements(doc->'goals') WITH ORDINALITY AS z(x, i))), 'id', gid);
    END IF;
  END IF;
  nid := projects.fresh_id('g');
  RETURN jsonb_build_object('doc', jsonb_set(doc, '{goals}', coalesce(doc->'goals', '[]') || jsonb_build_array(
           jsonb_build_object('id', nid, 'text', txt, 'status', 'proposed') || CASE WHEN gid IS NOT NULL THEN jsonb_build_object('replaces', gid) ELSE '{}' END), true),
         'id', nid);
END $$;

-- The agent's document tools: an approved project's runner or agent only
-- (anyone else, or any other project, gets the same 404 as no project), from the version it read, then push.
CREATE OR REPLACE FUNCTION projects.agent_edit(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  cur projects.docs;
  r jsonb; res jsonb;
BEGIN
  IF want !~ '^[a-z0-9][a-z0-9-]{0,63}$' OR pk IS NULL OR NOT coalesce((who->>'agent_on')::boolean, false)
     OR projects.role(want, pk) NOT IN ('runner', 'agent') THEN RETURN projects.no(); END IF;
  SELECT * INTO cur FROM projects.docs WHERE slug = want;
  IF cur.slug IS NULL THEN RETURN projects.no(); END IF;
  IF coalesce(body->>'base', '') !~ '^[0-9]{1,9}$' OR (body->>'base')::int <> cur.version THEN
    RETURN projects.r(409, jsonb_build_object('error', 'the document changed since your copy: get it again', 'version', cur.version));
  END IF;
  r := CASE op WHEN 'patch' THEN projects.patch_apply(cur.doc, body->'ops')
               WHEN 'milestone_edit' THEN projects.milestone_edit(cur.doc, body)
               WHEN 'goal_propose' THEN projects.goal_propose(cur.doc, body) END;
  IF r IS NULL THEN RETURN projects.no(); END IF;
  IF r ? 'error' THEN RETURN projects.r(400, jsonb_build_object('error', r->>'error')); END IF;
  res := projects.api('push', want, jsonb_build_object('base', cur.version, 'doc', r->'doc', 'turn', body->'turn'), who);
  IF (res->>'status')::int = 200 AND r ? 'id' THEN
    res := jsonb_set(res, '{body}', (res->'body') || jsonb_build_object('id', r->>'id'));
  END IF;
  RETURN res;
END $$;

-- The conversation as lines, for the page's AGENT panel:
--   Z|spent today|cap|busy (1 while a turn is queued or running)|members
--   G|goal id|members who agreed to its current text (proposed goals)
--   U|id|kind|status|at|by|text|answer|error|commit|files|undoable|undone by|working|undo of   (oldest first)
--   E|turn|id|type|text                                                               (each turn's edits and commits)
CREATE OR REPLACE FUNCTION projects.talk_lines(want text, cap numeric) RETURNS text LANGUAGE sql STABLE AS $$
  WITH d AS (SELECT * FROM projects.docs WHERE slug = want),
  t AS (SELECT * FROM (SELECT * FROM projects.turns WHERE slug = want ORDER BY id DESC LIMIT 20) x)
  SELECT string_agg(line, E'\n' ORDER BY o1, o2) || E'\n' FROM (
    SELECT 0 AS o1, 0::bigint AS o2, 'Z|' || to_char(projects.spent(want), 'FM9990.00') || '|' || to_char(cap, 'FM9990.00') || '|'
         || CASE WHEN EXISTS (SELECT 1 FROM projects.turns WHERE slug = want AND status IN ('queued', 'running')) THEN '1' ELSE '0' END
         || '|' || (SELECT jsonb_array_length(doc->'members') FROM d) AS line
    UNION ALL SELECT 0, i, 'G|' || projects.v(g->>'id') || '|' || coalesce(projects.v((SELECT string_agg(m->>'name', ', ' ORDER BY o)
           FROM jsonb_array_elements(d.doc->'members') WITH ORDINALITY AS mm(m, o) JOIN projects.agreements a
             ON a.slug = want AND a.goal = g->>'id' AND a.goal_text = g->>'text' AND a.pubkey = m->>'pubkey')), '')
      FROM d, jsonb_array_elements(coalesce(d.doc->'goals', '[]')) WITH ORDINALITY AS z(g, i) WHERE g->>'status' = 'proposed'
    UNION ALL SELECT 1, t.id * 1000, 'U|' || t.id || '|' || t.kind || '|' || t.status || '|' || projects.chi(t.asked_ms) || '|' || projects.v(t.by_name) || '|'
         || projects.v(left(t.text, 1500)) || '|' || projects.v(left(t.answer, 3000)) || '|' || projects.v(t.error) || '|' || coalesce(left(t.commit_sha, 7), '') || '|'
         || jsonb_array_length(t.files) || '|' || CASE WHEN projects.undoable(t.kind, t.status, t.undone_by, t.commit_sha, t.version_after, (SELECT version FROM d)) THEN '1' ELSE '0' END || '|'
         || coalesce(t.undone_by::text, '') || '|' || projects.v(t.working) || '|' || coalesce(t.undo_of::text, '')
      FROM t
    UNION ALL SELECT 1, e.turn * 1000 + 1 + row_number() OVER (PARTITION BY e.turn ORDER BY e.id),
         'E|' || e.turn || '|' || e.id || '|' || e.type || '|' || projects.v(left(projects.event_text(e.type, e.data), 300))
      FROM projects.events e WHERE e.turn IN (SELECT id FROM t) AND e.type IN ('agent.edit', 'agent.commit', 'session.error', 'user.interrupt')
  ) all_lines WHERE EXISTS (SELECT 1 FROM d)
$$;

