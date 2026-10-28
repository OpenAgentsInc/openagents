-- A project's agent on its page (docs/project-agent.md): one shared
-- conversation per project, a queue of members' messages as turns, the
-- events each turn made, and UNDO TURN. The hosted runner (coax/web/agent/)
-- is a key registered for the project with role 'runner'; it takes the next
-- turn, streams events, edits through the same ops as any agent, and ends
-- the turn. Only projects named in oa-api's server config (OA_AGENT_PROJECTS,
-- passed in as who.agent_on) have an agent; for any other project every one
-- of these ops is the same 404 as a project that doesn't exist.
--
-- Event types follow openagents.agent.managed.v1 SessionEvent:
--   user.message  user.interrupt  agent.message  agent.tool_use  agent.edit  agent.commit
--   session.status_running  session.status_idle  session.error
--
-- What the runner may change in a document: everything but v, slug, title,
-- members and the agreed goals (projects.kept). Goals it proposes become
-- agreed only when every member agrees (op agree), as before.

ALTER TABLE projects.agents DROP CONSTRAINT IF EXISTS agents_role_check;
ALTER TABLE projects.agents ADD CONSTRAINT agents_role_check CHECK (role IN ('agent', 'runner'));

CREATE TABLE projects.turns (
  id bigserial PRIMARY KEY,
  slug text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('ask', 'undo')),
  text text NOT NULL,                       -- the member's message ('' for an undo)
  by_pubkey text NOT NULL,
  by_name text NOT NULL,
  status text NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed', 'stopped', 'refused')),
  undo_of bigint REFERENCES projects.turns (id),
  undone_by bigint REFERENCES projects.turns (id),
  stop boolean NOT NULL DEFAULT false,
  asked_ms bigint NOT NULL,
  woken_ms bigint NOT NULL DEFAULT 0,
  started_ms bigint,
  ended_ms bigint,
  version_before int,                       -- the document when the turn started
  version_after int,                        -- the document when it ended, if it edited it
  commit_sha text,                          -- the repository commit it pushed
  files jsonb NOT NULL DEFAULT '[]',
  cost_usd numeric(10, 4) NOT NULL DEFAULT 0,
  working text NOT NULL DEFAULT '',         -- one line: what it is doing now
  answer text NOT NULL DEFAULT '',
  error text NOT NULL DEFAULT ''
);
CREATE INDEX turns_slug ON projects.turns (slug, id);
CREATE TABLE projects.events (
  id bigserial PRIMARY KEY,
  slug text NOT NULL,
  turn bigint NOT NULL REFERENCES projects.turns (id),
  at_ms bigint NOT NULL,
  type text NOT NULL,
  data jsonb NOT NULL
);
CREATE INDEX events_turn ON projects.events (turn, id);
-- Members agreeing to a proposed goal, by its text: a changed text needs agreeing again.
CREATE TABLE projects.agreements (
  slug text NOT NULL,
  goal text NOT NULL,
  goal_text text NOT NULL,
  pubkey text NOT NULL,
  at_ms bigint NOT NULL,
  PRIMARY KEY (slug, goal, goal_text, pubkey)
);

-- Who pubkey is in slug: 'member', 'agent', 'runner' or ''.
CREATE OR REPLACE FUNCTION projects.role(want text, pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE
    WHEN EXISTS (SELECT 1 FROM projects.docs d, jsonb_array_elements(d.doc->'members') m WHERE d.slug = want AND m->>'pubkey' = pk) THEN 'member'
    ELSE coalesce((SELECT a.role FROM projects.agents a JOIN projects.docs d USING (slug) WHERE a.slug = want AND a.pubkey = pk), '') END
$$;

-- What the project's agent may not change.
CREATE FUNCTION projects.kept(doc jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('v', doc->'v', 'slug', doc->'slug', 'title', doc->'title', 'members', doc->'members',
    'goals', (SELECT coalesce(jsonb_agg(g ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) WITH ORDINALITY AS z(g, i)
              WHERE g->>'status' = 'agreed'))
$$;

-- A day as people read it: 10-29.
CREATE FUNCTION projects.md(date text) RETURNS text LANGUAGE sql IMMUTABLE AS $$ SELECT coalesce(substr(date, 6), '') $$;

-- What changed between two versions, as short lines for the page's diff cards (at most 12).
CREATE FUNCTION projects.diff(a jsonb, b jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  WITH ma AS (SELECT m->>'id' AS id, y->>'date' AS date, m->>'text' AS text, m->>'status' AS status, coalesce(m->>'note', '') AS note
              FROM jsonb_array_elements(coalesce(a->'days', '[]')) y, jsonb_array_elements(coalesce(y->'milestones', '[]')) m),
       mb AS (SELECT m->>'id' AS id, y->>'date' AS date, m->>'text' AS text, m->>'status' AS status, coalesce(m->>'note', '') AS note
              FROM jsonb_array_elements(coalesce(b->'days', '[]')) y, jsonb_array_elements(coalesce(y->'milestones', '[]')) m),
       ia AS (SELECT t->>'id' AS id, c->>'id' AS list, t->>'text' AS text, t->>'status' AS status, coalesce(t->>'evidence', '') AS note
              FROM jsonb_array_elements(coalesce(a->'checklists', '[]')) c, jsonb_array_elements(coalesce(c->'items', '[]')) t),
       ib AS (SELECT t->>'id' AS id, c->>'id' AS list, t->>'text' AS text, t->>'status' AS status, coalesce(t->>'evidence', '') AS note
              FROM jsonb_array_elements(coalesce(b->'checklists', '[]')) c, jsonb_array_elements(coalesce(c->'items', '[]')) t),
       ga AS (SELECT g->>'id' AS id, g->>'text' AS text, g->>'status' AS status FROM jsonb_array_elements(coalesce(a->'goals', '[]')) g),
       gb AS (SELECT g->>'id' AS id, g->>'text' AS text, g->>'status' AS status FROM jsonb_array_elements(coalesce(b->'goals', '[]')) g),
       lines AS (
         SELECT 1 AS o, 'day ' || projects.md(mb.date) || ': + ' || mb.text AS line FROM mb WHERE NOT EXISTS (SELECT 1 FROM ma WHERE ma.id = mb.id)
         UNION ALL SELECT 2, 'day ' || projects.md(ma.date) || ': removed ' || ma.text FROM ma WHERE NOT EXISTS (SELECT 1 FROM mb WHERE mb.id = ma.id)
         UNION ALL SELECT 3, 'day ' || projects.md(mb.date) || ': ' || ma.text || ' → ' || mb.text FROM ma JOIN mb USING (id) WHERE ma.text <> mb.text
         UNION ALL SELECT 4, mb.text || ': day ' || projects.md(ma.date) || ' → day ' || projects.md(mb.date) FROM ma JOIN mb USING (id) WHERE ma.date <> mb.date
         UNION ALL SELECT 5, 'day ' || projects.md(mb.date) || ': ' || mb.text || ' → ' || mb.status FROM ma JOIN mb USING (id) WHERE ma.status <> mb.status
         UNION ALL SELECT 6, 'day ' || projects.md(mb.date) || ': ' || mb.text || ', note: ' || mb.note FROM ma JOIN mb USING (id) WHERE ma.note <> mb.note
         UNION ALL SELECT 7, 'checklist: + ' || ib.text FROM ib WHERE NOT EXISTS (SELECT 1 FROM ia WHERE ia.id = ib.id AND ia.list = ib.list)
         UNION ALL SELECT 8, 'checklist: removed ' || ia.text FROM ia WHERE NOT EXISTS (SELECT 1 FROM ib WHERE ib.id = ia.id AND ib.list = ia.list)
         UNION ALL SELECT 9, 'checklist: ' || ia.text || ' → ' || ib.text FROM ia JOIN ib USING (id, list) WHERE ia.text <> ib.text
         UNION ALL SELECT 10, 'checklist: ' || ib.text || ' → ' || ib.status FROM ia JOIN ib USING (id, list) WHERE ia.status <> ib.status
         UNION ALL SELECT 11, 'checklist: ' || ib.text || ', evidence: ' || ib.note FROM ia JOIN ib USING (id, list) WHERE ia.note <> ib.note
         UNION ALL SELECT 12, 'goal proposed: ' || gb.text FROM gb WHERE gb.status = 'proposed' AND NOT EXISTS (SELECT 1 FROM ga WHERE ga.id = gb.id AND ga.text = gb.text AND ga.status = gb.status)
         UNION ALL SELECT 13, 'goal agreed: ' || gb.text FROM gb WHERE gb.status = 'agreed' AND NOT EXISTS (SELECT 1 FROM ga WHERE ga.id = gb.id AND ga.text = gb.text AND ga.status = gb.status)
         UNION ALL SELECT 14, 'goal removed: ' || ga.text FROM ga WHERE NOT EXISTS (SELECT 1 FROM gb WHERE gb.id = ga.id)
         UNION ALL SELECT 15, k || ' changed' FROM (SELECT DISTINCT k FROM (SELECT jsonb_object_keys(a) AS k UNION SELECT jsonb_object_keys(b)) x) keys
                   WHERE k NOT IN ('days', 'checklists', 'goals', 'v', 'slug') AND (a->k) IS DISTINCT FROM (b->k)
         UNION ALL SELECT 16, 'days: + ' || (y->>'date') FROM jsonb_array_elements(coalesce(b->'days', '[]')) y
                   WHERE NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(a->'days', '[]')) z WHERE z->>'date' = y->>'date'))
  SELECT coalesce(jsonb_agg(left(line, 200) ORDER BY o, line), '[]') FROM (SELECT * FROM lines ORDER BY o, line LIMIT 12) l
$$;

-- Today's spend in Chicago for a project's agent, in US dollars.
CREATE FUNCTION projects.spent(want text) RETURNS numeric LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(cost_usd), 0) FROM projects.turns
  WHERE slug = want AND asked_ms >= (extract(epoch FROM (date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago')) * 1000)::bigint
$$;

-- A turn's events as text for the page: edits, commits and tool uses.
CREATE FUNCTION projects.event_text(type text, data jsonb) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE type
    WHEN 'agent.edit' THEN coalesce(data->>'line', '')
    WHEN 'agent.commit' THEN coalesce(left(data->>'sha', 7), '') || ' · ' || coalesce(data->>'message', '')
         || coalesce(' · ' || (SELECT string_agg(f, ', ') FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(data->'files') = 'array' THEN data->'files' ELSE '[]' END) f), '')
    WHEN 'agent.tool_use' THEN coalesce(data->>'name', '') || coalesce(' ' || nullif(data->>'detail', ''), '')
    WHEN 'session.error' THEN coalesce(data->>'error', '')
    WHEN 'user.interrupt' THEN 'stopped by ' || coalesce(data->>'by', '')
    ELSE '' END
$$;

-- May this turn be undone now: an ask that ended, not undone, that changed
-- something, and the document hasn't moved since (else undo the later turn first).
CREATE FUNCTION projects.undoable(kind text, status text, undone_by bigint, commit_sha text, version_after int, current int)
RETURNS boolean LANGUAGE sql IMMUTABLE AS $$
  SELECT kind = 'ask' AND status IN ('done', 'failed', 'stopped') AND undone_by IS NULL
     AND (commit_sha IS NOT NULL OR version_after IS NOT NULL)
     AND (version_after IS NULL OR version_after = current)
$$;

-- The conversation as lines, for the page's AGENT panel:
--   Z|spent today|cap|busy (1 while a turn is queued or running)|members
--   G|goal id|members who agreed to its current text (proposed goals)
--   U|id|kind|status|at|by|text|answer|error|commit|files|undoable|undone by|working|undo of   (oldest first)
--   E|turn|id|type|text                                                               (each turn's edits and commits)
CREATE FUNCTION projects.talk_lines(want text, cap numeric) RETURNS text LANGUAGE sql STABLE AS $$
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

CREATE FUNCTION projects.talk_json(want text, cap numeric) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('spent', projects.spent(want), 'cap', cap,
    'turns', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', t.id, 'kind', t.kind, 'status', t.status, 'at', projects.chi(t.asked_ms), 'by', t.by_name,
                'text', t.text, 'answer', t.answer, 'error', t.error, 'commit', t.commit_sha, 'files', t.files, 'cost_usd', t.cost_usd,
                'version_before', t.version_before, 'version_after', t.version_after, 'undo_of', t.undo_of, 'undone_by', t.undone_by,
                'events', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', e.id, 'type', e.type, 'data', e.data) ORDER BY e.id), '[]') FROM projects.events e WHERE e.turn = t.id))
              ORDER BY t.id), '[]') FROM (SELECT * FROM projects.turns WHERE slug = want ORDER BY id DESC LIMIT 20) t))
$$;

-- An edit the runner made inside a turn, recorded under it with what changed.
CREATE FUNCTION projects.record_edit(want text, turn_id bigint, before jsonb, after jsonb, op text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO projects.events (slug, turn, at_ms, type, data)
  SELECT want, turn_id, projects.now_ms(), 'agent.edit', jsonb_build_object('op', op, 'line', l)
  FROM (SELECT l FROM jsonb_array_elements_text(projects.diff(before, after)) l
        UNION ALL SELECT 'no visible change' WHERE projects.diff(before, after) = '[]') z(l)
$$;

-- The 0007 request function, with the agent's ops added (the rest as before):
--   member (cookie or signed):  GET talk; POST ask {text}, stop {turn}, undo {turn}, agree {goal}
--   runner (signed):            POST next {}, event {turn, events: [{type, data}]}, end {turn, status, answer, error, commit, files, cost_usd};
--                               push, status and note carry {turn}: a running turn of this project
-- who.agent_on: this project is in the server's approved list; who.cap_usd: its daily cap.
CREATE OR REPLACE FUNCTION projects.api(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  admin boolean := coalesce((who->>'admin')::boolean, false);
  agent_on boolean := coalesce((who->>'agent_on')::boolean, false);
  cap numeric := coalesce(nullif(who->>'cap_usd', '')::numeric, 10);
  now_ms bigint := projects.now_ms();
  role text := '';
  me text;
  cur projects.docs;
  nd jsonb;
  why text;
  t text;
  n bigint;
  tn projects.turns;
  turn_id bigint;
  rec record;
  x jsonb;
BEGIN
  IF want !~ '^[a-z0-9][a-z0-9-]{0,63}$' THEN RETURN projects.no(); END IF;
  IF op = 'redeem' THEN
    UPDATE projects.invites SET used_ms = now_ms
      WHERE token_sha256 = projects.sha(coalesce(body->>'token', '')) AND slug = want AND used_ms IS NULL AND expires_ms > now_ms
      RETURNING pubkey INTO me;
    IF me IS NULL OR projects.role(want, me) <> 'member' THEN RETURN projects.no(); END IF;
    t := projects.token();
    INSERT INTO projects.sessions VALUES (projects.sha(t), want, me, now_ms, now_ms + 30 * 86400000::bigint);
    RETURN projects.r(200, '{"ok":true}') || jsonb_build_object('cookie', t);
  END IF;
  IF pk IS NULL THEN pk := projects.session(want, nullif(who->>'session', '')); END IF;
  IF pk IS NOT NULL THEN role := projects.role(want, pk); END IF;
  IF op = 'shell' THEN RETURN projects.r(CASE WHEN role = 'member' THEN 200 ELSE 404 END, '{}'); END IF;
  SELECT * INTO cur FROM projects.docs WHERE slug = want;
  IF op = 'push' AND cur.slug IS NULL AND admin AND pk IS NOT NULL THEN role := 'first'; END IF;
  IF role = '' THEN RETURN projects.no(); END IF;
  -- The agent's ops exist only for approved projects: elsewhere, the same 404.
  IF op IN ('talk', 'ask', 'stop', 'undo', 'agree', 'next', 'event', 'end') AND NOT agent_on THEN RETURN projects.no(); END IF;
  me := projects.name_of(want, pk);
  -- The runner writes only inside a running turn of this project.
  IF role = 'runner' AND op IN ('push', 'status', 'note', 'message', 'event', 'end') THEN
    turn_id := CASE WHEN coalesce(body->>'turn', '') ~ '^[0-9]{1,18}$' THEN (body->>'turn')::bigint END;
    SELECT * INTO tn FROM projects.turns WHERE id = turn_id AND slug = want AND status = 'running';
    IF tn.id IS NULL OR NOT agent_on THEN RETURN projects.r(403, '{"error":"the project''s agent writes only during a running turn"}'); END IF;
  END IF;

  IF op = 'state' THEN
    IF who->>'format' = 'lines' THEN RETURN projects.r(200, to_jsonb(projects.lines(want, pk))); END IF;
    RETURN projects.r(200, projects.state(want, pk) || jsonb_build_object('agent', agent_on));
  ELSIF op = 'logout' THEN
    DELETE FROM projects.sessions WHERE token_sha256 = projects.sha(coalesce(who->>'session', '')) AND slug = want;
    RETURN projects.r(200, '{"ok":true}');
  ELSIF op = 'push' THEN
    nd := body->'doc';
    IF coalesce((body->>'base')::int, -1) <> coalesce(cur.version, 0) THEN
      RETURN projects.r(409, jsonb_build_object('error', 'the document changed since your copy: get it again', 'version', coalesce(cur.version, 0)));
    END IF;
    IF role = 'agent' AND projects.fixed(nd) IS DISTINCT FROM projects.fixed(cur.doc) THEN
      RETURN projects.r(403, '{"error":"agents change only milestone and checklist status and proposed goals; agreed goals and members change only when the members agree"}');
    END IF;
    IF role = 'runner' AND projects.kept(nd) IS DISTINCT FROM projects.kept(cur.doc) THEN
      RETURN projects.r(403, '{"error":"the agent may not change the title, the members or an agreed goal; propose a goal and the members agree to it"}');
    END IF;
    why := projects.invalid(nd, want);
    IF why IS NOT NULL THEN RETURN projects.r(400, jsonb_build_object('error', why)); END IF;
    IF role = 'first' AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(nd->'members') m WHERE m->>'pubkey' = pk) THEN
      RETURN projects.r(400, '{"error":"the first push lists its pusher as a member"}');
    END IF;
    IF cur.slug IS NULL THEN
      INSERT INTO projects.docs VALUES (want, 1, nd, now_ms, pk);
    ELSE
      UPDATE projects.docs SET version = version + 1, doc = body->'doc', updated_ms = now_ms, updated_by = pk WHERE slug = want;
    END IF;
    INSERT INTO projects.history VALUES (want, coalesce(cur.version, 0) + 1, nd, now_ms, pk);
    IF role = 'runner' THEN PERFORM projects.record_edit(want, tn.id, cur.doc, nd, 'push'); END IF;
    RETURN projects.r(200, jsonb_build_object('version', coalesce(cur.version, 0) + 1));
  ELSIF op = 'note' THEN
    t := btrim(coalesce(body->>'text', ''));
    IF t = '' OR length(t) > 4000 THEN RETURN projects.r(400, '{"error":"text is 1 to 4000 characters"}'); END IF;
    IF coalesce(body->>'kind', 'note') NOT IN ('note', 'plan', 'status') THEN RETURN projects.r(400, '{"error":"kind is note, plan or status"}'); END IF;
    INSERT INTO projects.activity (slug, at_ms, by_pubkey, by_name, kind, text)
      VALUES (want, now_ms, pk, left(coalesce(nullif(btrim(body->>'by'), ''), me), 60), coalesce(body->>'kind', 'note'), t) RETURNING id INTO n;
    IF role = 'runner' THEN
      INSERT INTO projects.events (slug, turn, at_ms, type, data)
        VALUES (want, tn.id, now_ms, 'agent.edit', jsonb_build_object('op', 'note', 'line', coalesce(body->>'kind', 'note') || ': ' || left(t, 160)));
    END IF;
    RETURN projects.r(200, jsonb_build_object('id', n));
  ELSIF op = 'status' THEN
    IF body->>'status' NOT IN ('todo', 'doing', 'done') THEN RETURN projects.r(400, '{"error":"status is todo, doing or done"}'); END IF;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(cur.doc->'days', '[]')) y, jsonb_array_elements(coalesce(y->'milestones', '[]')) m
                   WHERE m->>'id' = body->>'id')
       AND NOT EXISTS (SELECT 1 FROM jsonb_array_elements(coalesce(cur.doc->'checklists', '[]')) y, jsonb_array_elements(coalesce(y->'items', '[]')) m
                   WHERE m->>'id' = body->>'id') THEN
      RETURN projects.r(404, '{"error":"no milestone or item has that id"}');
    END IF;
    nd := cur.doc;
    SELECT jsonb_set(nd, '{days}', coalesce(jsonb_agg(jsonb_set(d, '{milestones}', (
             SELECT coalesce(jsonb_agg(CASE WHEN m->>'id' = body->>'id' THEN
                      (m || jsonb_build_object('status', body->>'status')) || CASE WHEN body ? 'note' THEN jsonb_build_object('note', body->>'note') ELSE '{}' END
                    ELSE m END ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(d->'milestones', '[]')) WITH ORDINALITY AS x(m, o))) ORDER BY i), '[]'))
      INTO nd FROM jsonb_array_elements(coalesce(cur.doc->'days', '[]')) WITH ORDINALITY AS y(d, i);
    SELECT jsonb_set(nd, '{checklists}', coalesce(jsonb_agg(jsonb_set(c, '{items}', (
             SELECT coalesce(jsonb_agg(CASE WHEN m->>'id' = body->>'id' THEN
                      (m || jsonb_build_object('status', body->>'status')) || CASE WHEN body ? 'note' THEN jsonb_build_object('evidence', body->>'note') ELSE '{}' END
                    ELSE m END ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(c->'items', '[]')) WITH ORDINALITY AS x(m, o))) ORDER BY i), '[]'))
      INTO nd FROM jsonb_array_elements(coalesce(cur.doc->'checklists', '[]')) WITH ORDINALITY AS y(c, i);
    IF projects.invalid(nd, want) IS NOT NULL OR projects.fixed(nd) IS DISTINCT FROM projects.fixed(cur.doc) THEN
      RETURN projects.r(400, '{"error":"that status change does not fit the document"}');
    END IF;
    UPDATE projects.docs SET version = version + 1, doc = nd, updated_ms = now_ms, updated_by = pk WHERE slug = want;
    INSERT INTO projects.history VALUES (want, cur.version + 1, nd, now_ms, pk);
    INSERT INTO projects.activity (slug, at_ms, by_pubkey, by_name, kind, text)
      VALUES (want, now_ms, pk, left(coalesce(nullif(btrim(body->>'by'), ''), me), 60), 'status',
              (body->>'id') || ' is ' || (body->>'status') || coalesce(': ' || nullif(btrim(body->>'note'), ''), ''));
    IF role = 'runner' THEN PERFORM projects.record_edit(want, tn.id, cur.doc, nd, 'status'); END IF;
    RETURN projects.r(200, jsonb_build_object('version', cur.version + 1));
  ELSIF op = 'message' THEN
    t := btrim(coalesce(body->>'text', ''));
    IF t = '' OR length(t) > 2000 THEN RETURN projects.r(400, '{"error":"a message is 1 to 2000 characters"}'); END IF;
    n := CASE WHEN coalesce(body->>'reply_to', '') ~ '^[0-9]{1,18}$' THEN (body->>'reply_to')::bigint END;
    IF body ? 'reply_to' AND (n IS NULL OR NOT EXISTS (SELECT 1 FROM projects.messages WHERE id = n AND slug = want)) THEN
      RETURN projects.r(404, '{"error":"no message has that id"}');
    END IF;
    INSERT INTO projects.messages (slug, at_ms, from_pubkey, from_name, text, reply_to)
      VALUES (want, now_ms, pk, left(coalesce(nullif(btrim(body->>'by'), ''), me), 60), t, n) RETURNING id INTO n;
    RETURN projects.r(200, jsonb_build_object('id', n));
  ELSIF op = 'inbox' THEN
    RETURN projects.r(200, (SELECT coalesce(jsonb_agg(jsonb_build_object('id', m.id, 'at', projects.chi(m.at_ms), 'from', m.from_name, 'text', m.text) ORDER BY m.at_ms), '[]')
      FROM projects.messages m WHERE m.slug = want AND m.reply_to IS NULL AND projects.role(want, m.from_pubkey) = 'member'
        AND NOT EXISTS (SELECT 1 FROM projects.messages r WHERE r.reply_to = m.id)));
  ELSIF op = 'invite' THEN
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    me := lower(coalesce(body->>'pubkey', ''));
    IF projects.role(want, me) <> 'member' THEN RETURN projects.r(400, '{"error":"invites are for the document''s members"}'); END IF;
    t := projects.token();
    INSERT INTO projects.invites VALUES (projects.sha(t), want, me, now_ms, now_ms + 86400000, NULL, pk);
    RETURN projects.r(200, jsonb_build_object('token', t, 'expires', projects.chi(now_ms + 86400000) || ' Chicago'));
  ELSIF op = 'agent' THEN
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    me := lower(coalesce(body->>'pubkey', ''));
    IF me !~ '^[0-9a-f]{64}$' OR coalesce(btrim(body->>'name'), '') = '' THEN RETURN projects.r(400, '{"error":"an agent needs a pubkey and a name"}'); END IF;
    IF projects.role(want, me) = 'member' THEN RETURN projects.r(400, '{"error":"that key is a member"}'); END IF;
    IF body->>'role' = 'none' THEN
      DELETE FROM projects.agents WHERE slug = want AND pubkey = me;
    ELSE
      INSERT INTO projects.agents VALUES (want, me, left(btrim(body->>'name'), 60), CASE WHEN body->>'role' = 'runner' THEN 'runner' ELSE 'agent' END, now_ms, pk)
        ON CONFLICT (slug, pubkey) DO UPDATE SET name = EXCLUDED.name, role = EXCLUDED.role;
    END IF;
    RETURN projects.r(200, '{"ok":true}');

  -- ---- the project's agent: members
  ELSIF op = 'talk' THEN
    IF role NOT IN ('member', 'runner') THEN RETURN projects.no(); END IF;
    -- A turn waiting with nobody working on it: wake the runner again (at most every 15 s).
    UPDATE projects.turns SET woken_ms = now_ms
      WHERE id = (SELECT min(id) FROM projects.turns WHERE slug = want AND status = 'queued') AND now_ms - greatest(asked_ms, woken_ms) > 15000
        AND NOT EXISTS (SELECT 1 FROM projects.turns WHERE slug = want AND status = 'running')
      RETURNING id INTO n;
    IF who->>'format' = 'lines' THEN
      RETURN projects.r(200, to_jsonb(projects.talk_lines(want, cap))) || jsonb_build_object('wake', n IS NOT NULL);
    END IF;
    RETURN projects.r(200, projects.talk_json(want, cap)) || jsonb_build_object('wake', n IS NOT NULL);
  ELSIF op = 'ask' THEN
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    t := btrim(coalesce(body->>'text', ''));
    IF t = '' OR length(t) > 2000 THEN RETURN projects.r(400, '{"error":"a message is 1 to 2000 characters"}'); END IF;
    IF (SELECT count(*) FROM projects.turns WHERE slug = want AND status = 'queued') >= 5 THEN
      RETURN projects.r(429, '{"error":"five messages are already waiting for the agent"}');
    END IF;
    INSERT INTO projects.turns (slug, kind, text, by_pubkey, by_name, status, asked_ms, woken_ms)
      VALUES (want, 'ask', t, pk, me, 'queued', now_ms, now_ms) RETURNING id INTO n;
    INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, n, now_ms, 'user.message', jsonb_build_object('by', me));
    RETURN projects.r(200, jsonb_build_object('turn', n)) || '{"wake":true}';
  ELSIF op = 'stop' THEN
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    SELECT * INTO tn FROM projects.turns WHERE slug = want AND id = CASE WHEN coalesce(body->>'turn', '') ~ '^[0-9]{1,18}$' THEN (body->>'turn')::bigint END;
    IF tn.id IS NULL THEN RETURN projects.r(404, '{"error":"no turn has that id"}'); END IF;
    IF tn.status NOT IN ('queued', 'running') THEN RETURN projects.r(409, '{"error":"that turn has ended"}'); END IF;
    UPDATE projects.turns SET stop = true,
      status = CASE WHEN status = 'queued' THEN 'stopped' ELSE status END,
      ended_ms = CASE WHEN status = 'queued' THEN now_ms ELSE ended_ms END WHERE id = tn.id;
    INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, tn.id, now_ms, 'user.interrupt', jsonb_build_object('by', me));
    RETURN projects.r(200, '{"ok":true}');
  ELSIF op = 'undo' THEN
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    SELECT * INTO tn FROM projects.turns WHERE slug = want AND id = CASE WHEN coalesce(body->>'turn', '') ~ '^[0-9]{1,18}$' THEN (body->>'turn')::bigint END;
    IF tn.id IS NULL THEN RETURN projects.r(404, '{"error":"no turn has that id"}'); END IF;
    IF NOT projects.undoable(tn.kind, tn.status, tn.undone_by, tn.commit_sha, tn.version_after, cur.version) THEN
      RETURN projects.r(409, jsonb_build_object('error', CASE
        WHEN tn.undone_by IS NOT NULL THEN 'that turn was undone already'
        WHEN tn.kind <> 'ask' OR tn.status IN ('queued', 'running', 'refused') THEN 'only a turn that ended can be undone'
        WHEN tn.commit_sha IS NULL AND tn.version_after IS NULL THEN 'that turn changed nothing'
        ELSE 'the document changed after that turn: undo the later turn first' END));
    END IF;
    INSERT INTO projects.turns (slug, kind, text, by_pubkey, by_name, status, undo_of, asked_ms, woken_ms, version_before)
      VALUES (want, 'undo', '', pk, me, CASE WHEN tn.commit_sha IS NULL THEN 'done' ELSE 'queued' END, tn.id, now_ms, now_ms, cur.version)
      RETURNING id INTO n;
    UPDATE projects.turns SET undone_by = n WHERE id = tn.id;
    INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, n, now_ms, 'user.message', jsonb_build_object('by', me, 'undo', tn.id));
    -- the document back to the version from before the turn, as a new version
    IF tn.version_after IS NOT NULL AND tn.version_before IS NOT NULL AND tn.version_after <> tn.version_before THEN
      SELECT doc INTO nd FROM projects.history WHERE slug = want AND version = tn.version_before;
      UPDATE projects.docs SET version = version + 1, doc = nd, updated_ms = now_ms, updated_by = pk WHERE slug = want;
      INSERT INTO projects.history VALUES (want, cur.version + 1, nd, now_ms, pk);
      INSERT INTO projects.events (slug, turn, at_ms, type, data)
        SELECT want, n, now_ms, 'agent.edit', jsonb_build_object('op', 'undo', 'line', l) FROM jsonb_array_elements_text(projects.diff(cur.doc, nd)) l;
      UPDATE projects.turns SET version_after = cur.version + 1 WHERE id = n;
    END IF;
    IF tn.commit_sha IS NULL THEN UPDATE projects.turns SET ended_ms = now_ms, started_ms = now_ms WHERE id = n; END IF;
    RETURN projects.r(200, jsonb_build_object('turn', n, 'version', CASE WHEN tn.version_after IS NOT NULL AND tn.version_after <> tn.version_before THEN cur.version + 1 ELSE cur.version END))
      || jsonb_build_object('wake', tn.commit_sha IS NOT NULL);
  ELSIF op = 'agree' THEN
    -- a member agrees to a proposed goal; once every member has, it is agreed
    IF role <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    SELECT g INTO x FROM jsonb_array_elements(coalesce(cur.doc->'goals', '[]')) g WHERE g->>'id' = body->>'goal' AND g->>'status' = 'proposed';
    IF x IS NULL THEN RETURN projects.r(404, '{"error":"no proposed goal has that id"}'); END IF;
    INSERT INTO projects.agreements VALUES (want, x->>'id', x->>'text', pk, now_ms) ON CONFLICT DO NOTHING;
    IF NOT EXISTS (SELECT 1 FROM jsonb_array_elements(cur.doc->'members') m
                   WHERE NOT EXISTS (SELECT 1 FROM projects.agreements a WHERE a.slug = want AND a.goal = x->>'id' AND a.goal_text = x->>'text' AND a.pubkey = m->>'pubkey')) THEN
      SELECT jsonb_set(cur.doc, '{goals}', jsonb_agg(CASE WHEN g->>'id' = x->>'id' THEN
               g || jsonb_build_object('status', 'agreed', 'agreed_by', (SELECT jsonb_agg(m->>'name') FROM jsonb_array_elements(cur.doc->'members') m))
             ELSE g END ORDER BY i)) INTO nd FROM jsonb_array_elements(cur.doc->'goals') WITH ORDINALITY AS z(g, i);
      UPDATE projects.docs SET version = version + 1, doc = nd, updated_ms = now_ms, updated_by = pk WHERE slug = want;
      INSERT INTO projects.history VALUES (want, cur.version + 1, nd, now_ms, pk);
      INSERT INTO projects.activity (slug, at_ms, by_pubkey, by_name, kind, text) VALUES (want, now_ms, pk, me, 'status', 'goal agreed by everyone: ' || (x->>'text'));
      RETURN projects.r(200, jsonb_build_object('agreed', true, 'version', cur.version + 1));
    END IF;
    RETURN projects.r(200, '{"agreed":false}');

  -- ---- the project's agent: the runner
  ELSIF op = 'next' THEN
    IF role <> 'runner' THEN RETURN projects.r(403, '{"error":"the runner only"}'); END IF;
    -- a turn the runner never ended (it was stopped or crashed) fails after 20 minutes
    UPDATE projects.turns SET status = 'failed', error = 'the agent stopped answering', ended_ms = now_ms
      WHERE slug = want AND status = 'running' AND started_ms < now_ms - 1200000;
    IF EXISTS (SELECT 1 FROM projects.turns WHERE slug = want AND status = 'running') THEN RETURN projects.r(200, '{"turn":null,"busy":true}'); END IF;
    SELECT * INTO tn FROM projects.turns WHERE slug = want AND status = 'queued' ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF tn.id IS NULL THEN RETURN projects.r(200, '{"turn":null}'); END IF;
    IF tn.kind = 'ask' AND projects.spent(want) >= cap THEN
      UPDATE projects.turns SET status = 'refused', ended_ms = now_ms,
        error = 'Today''s $' || to_char(cap, 'FM9990') || ' limit for this project''s agent is spent. It starts again at midnight Chicago time.' WHERE id = tn.id;
      INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, tn.id, now_ms, 'session.error', jsonb_build_object('error', 'over the daily cap'));
      RETURN projects.r(200, jsonb_build_object('turn', NULL, 'refused', tn.id));
    END IF;
    UPDATE projects.turns SET status = 'running', started_ms = now_ms, version_before = coalesce(version_before, cur.version), working = 'starting' WHERE id = tn.id;
    INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, tn.id, now_ms, 'session.status_running', '{}');
    RETURN projects.r(200, jsonb_build_object(
      'turn', jsonb_build_object('id', tn.id, 'kind', tn.kind, 'text', tn.text, 'by', tn.by_name, 'undo_of', tn.undo_of,
                                 'revert', (SELECT commit_sha FROM projects.turns WHERE id = tn.undo_of)),
      'version', cur.version, 'spent', projects.spent(want), 'cap', cap,
      'history', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.id, 'by', h.by_name, 'text', h.text, 'answer', h.answer, 'undone', h.undone_by IS NOT NULL) ORDER BY h.id), '[]')
                  FROM (SELECT * FROM projects.turns WHERE slug = want AND kind = 'ask' AND status = 'done' AND id < tn.id ORDER BY id DESC LIMIT 10) h)));
  ELSIF op = 'event' THEN
    IF role <> 'runner' THEN RETURN projects.r(403, '{"error":"the runner only"}'); END IF;
    IF jsonb_typeof(body->'events') <> 'array' OR jsonb_array_length(body->'events') > 50 THEN RETURN projects.r(400, '{"error":"events is a list of at most 50"}'); END IF;
    FOR x IN SELECT * FROM jsonb_array_elements(body->'events') LOOP
      IF x->>'type' = 'working' THEN
        UPDATE projects.turns SET working = left(coalesce(x->'data'->>'text', ''), 120) WHERE id = tn.id;
      ELSIF x->>'type' IN ('agent.message', 'agent.tool_use', 'agent.tool_result', 'agent.commit', 'session.error') AND length((x->'data')::text) <= 8000 THEN
        INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, tn.id, now_ms, x->>'type', coalesce(x->'data', '{}'));
      ELSE
        RETURN projects.r(400, '{"error":"an event is working, agent.message, agent.tool_use, agent.tool_result, agent.commit or session.error, at most 8000 bytes"}');
      END IF;
    END LOOP;
    RETURN projects.r(200, jsonb_build_object('stop', tn.stop));
  ELSIF op = 'end' THEN
    IF role <> 'runner' THEN RETURN projects.r(403, '{"error":"the runner only"}'); END IF;
    IF coalesce(body->>'status', '') NOT IN ('done', 'failed', 'stopped') THEN RETURN projects.r(400, '{"error":"status is done, failed or stopped"}'); END IF;
    IF coalesce(body->>'commit', '') <> '' AND body->>'commit' !~ '^[0-9a-f]{7,64}$' THEN RETURN projects.r(400, '{"error":"commit is a hex sha"}'); END IF;
    UPDATE projects.turns SET status = body->>'status', ended_ms = now_ms, working = '',
      answer = left(coalesce(body->>'answer', ''), 20000), error = left(coalesce(body->>'error', ''), 2000),
      commit_sha = nullif(body->>'commit', ''),
      files = CASE WHEN jsonb_typeof(body->'files') = 'array' THEN body->'files' ELSE '[]' END,
      cost_usd = cost_usd + least(greatest(coalesce(nullif(body->>'cost_usd', '')::numeric, 0), 0), 1000),
      version_after = CASE WHEN tn.kind = 'undo' THEN version_after
                           WHEN EXISTS (SELECT 1 FROM projects.events WHERE turn = tn.id AND type = 'agent.edit' AND data->>'op' IN ('push', 'status')) THEN cur.version END
      WHERE id = tn.id;
    IF coalesce(body->>'commit', '') <> '' THEN
      INSERT INTO projects.events (slug, turn, at_ms, type, data)
        VALUES (want, tn.id, now_ms, 'agent.commit', jsonb_build_object('sha', body->>'commit', 'message', left(coalesce(body->>'commit_message', ''), 200),
                'files', CASE WHEN jsonb_typeof(body->'files') = 'array' THEN body->'files' ELSE '[]' END));
    END IF;
    INSERT INTO projects.events (slug, turn, at_ms, type, data) VALUES (want, tn.id, now_ms, 'session.status_idle', jsonb_build_object('status', body->>'status'));
    RETURN projects.r(200, jsonb_build_object('ok', true, 'more', EXISTS (SELECT 1 FROM projects.turns WHERE slug = want AND status = 'queued')));
  END IF;
  RETURN projects.no();
END $$;
