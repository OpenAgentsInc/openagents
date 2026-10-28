-- Private projects (private.c; docs/private-projects.md): a project's
-- document, every version of it, what agents did and the messages people
-- send them. Only the project's members (the pubkeys in its document) and the
-- agent keys registered for it ever read any of it. The rules are here, in
-- one place: projects.api answers every request private.c hands it.
--
-- Who may do what:
--   a member (a pubkey in the document's members) does everything;
--   an agent (projects.agents, role 'agent') reads, posts notes, plans and
--   messages, changes milestone and checklist status, and may push a document
--   only when it differs from the current one in those statuses (and notes and
--   evidence) and in goals marked "proposed": never an agreed goal, the
--   members or anything else;
--   the first push for a slug is an admin's (OA_ADMIN_PUBKEYS).
-- Anyone else, and any slug that doesn't exist, gets the same 404.
CREATE SCHEMA IF NOT EXISTS projects;

CREATE TABLE projects.docs (
  slug text PRIMARY KEY,
  version int NOT NULL,
  doc jsonb NOT NULL,
  updated_ms bigint NOT NULL,
  updated_by text NOT NULL       -- the pubkey that pushed it
);
CREATE TABLE projects.history (  -- every push, kept
  slug text NOT NULL,
  version int NOT NULL,
  doc jsonb NOT NULL,
  at_ms bigint NOT NULL,
  by_pubkey text NOT NULL,
  PRIMARY KEY (slug, version)
);
CREATE TABLE projects.agents (   -- agent keys a member registered for a project
  slug text NOT NULL,
  pubkey text NOT NULL,
  name text NOT NULL,
  role text NOT NULL CHECK (role IN ('agent')),
  added_ms bigint NOT NULL,
  added_by text NOT NULL,
  PRIMARY KEY (slug, pubkey)
);
CREATE TABLE projects.activity (
  id bigserial PRIMARY KEY,
  slug text NOT NULL,
  at_ms bigint NOT NULL,
  by_pubkey text NOT NULL,
  by_name text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('note', 'plan', 'status')),
  text text NOT NULL
);
CREATE INDEX activity_slug ON projects.activity (slug, at_ms);
CREATE TABLE projects.messages (
  id bigserial PRIMARY KEY,
  slug text NOT NULL,
  at_ms bigint NOT NULL,
  from_pubkey text NOT NULL,
  from_name text NOT NULL,
  text text NOT NULL,
  reply_to bigint REFERENCES projects.messages (id)
);
CREATE INDEX messages_slug ON projects.messages (slug, at_ms);
-- Sign-in: an invite is a random token (only its SHA-256 here) for one
-- member, good once within 24 hours; it makes a session of 30 days, whose
-- token only the browser holds (its SHA-256 here).
CREATE TABLE projects.invites (
  token_sha256 text PRIMARY KEY,
  slug text NOT NULL,
  pubkey text NOT NULL,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL,
  used_ms bigint,
  created_by text NOT NULL
);
CREATE TABLE projects.sessions (
  token_sha256 text PRIMARY KEY,
  slug text NOT NULL,
  pubkey text NOT NULL,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL
);
CREATE TABLE projects.seen (     -- NIP-98 events, each accepted once
  id text PRIMARY KEY,
  ms bigint NOT NULL
);

CREATE FUNCTION projects.now_ms() RETURNS bigint LANGUAGE sql VOLATILE AS
  $$ SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint $$;
CREATE FUNCTION projects.r(status int, body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT jsonb_build_object('status', status, 'body', body) $$;
-- The one refusal: no project, not signed in, not a member, a bad token.
CREATE FUNCTION projects.no() RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT projects.r(404, '{"error":"not found"}') $$;
CREATE FUNCTION projects.sha(t text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT encode(sha256(convert_to(t, 'UTF8')), 'hex') $$;
CREATE FUNCTION projects.token() RETURNS text LANGUAGE sql VOLATILE AS
  $$ SELECT replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', '') $$;
-- Chicago time, as people read it.
CREATE FUNCTION projects.chi(ms bigint) RETURNS text LANGUAGE sql STABLE AS
  $$ SELECT to_char(to_timestamp(ms / 1000.0) AT TIME ZONE 'America/Chicago', 'Mon DD HH24:MI') $$;
-- A value in a page line: '|' and line breaks would split it.
CREATE FUNCTION projects.v(t text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT regexp_replace(coalesce(t, ''), '[|\r\n\t]', ' ', 'g') $$;

-- Why a document is not a good openagents.private-project.v1 for slug, or NULL.
CREATE FUNCTION projects.invalid(doc jsonb, want text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  names text[];
  x jsonb; y jsonb;
BEGIN
  IF jsonb_typeof(doc) <> 'object' THEN RETURN 'the document is not an object'; END IF;
  IF doc->>'v' IS DISTINCT FROM 'openagents.private-project.v1' THEN RETURN 'v must be openagents.private-project.v1'; END IF;
  IF doc->>'slug' IS DISTINCT FROM want THEN RETURN 'slug must be this project''s'; END IF;
  IF coalesce(doc->>'title', '') = '' THEN RETURN 'a title is needed'; END IF;
  IF jsonb_typeof(doc->'members') <> 'array' OR jsonb_array_length(doc->'members') = 0 THEN RETURN 'members must list someone'; END IF;
  FOR x IN SELECT * FROM jsonb_array_elements(doc->'members') LOOP
    IF coalesce(x->>'name', '') = '' THEN RETURN 'every member needs a name'; END IF;
    IF coalesce(x->>'pubkey', '') !~ '^[0-9a-f]{64}$' THEN RETURN 'every member needs a pubkey (64 lowercase hex)'; END IF;
  END LOOP;
  IF (SELECT count(DISTINCT m->>'pubkey') FROM jsonb_array_elements(doc->'members') m) <> jsonb_array_length(doc->'members') THEN
    RETURN 'a pubkey is listed twice'; END IF;
  SELECT array_agg(m->>'name') INTO names FROM jsonb_array_elements(doc->'members') m;
  FOREACH x IN ARRAY ARRAY[coalesce(doc->'rules', '[]'), coalesce(doc->'goals', '[]'), coalesce(doc->'days', '[]'),
                           coalesce(doc->'checklists', '[]'), coalesce(doc->'links', '[]')] LOOP
    IF jsonb_typeof(x) <> 'array' THEN RETURN 'rules, goals, days, checklists and links are lists'; END IF;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR coalesce(x->>'text', '') = '' THEN RETURN 'every goal needs an id and text'; END IF;
    IF x->>'status' NOT IN ('agreed', 'proposed') THEN RETURN 'a goal is agreed or proposed'; END IF;
    IF x->>'status' = 'agreed' AND NOT (SELECT coalesce(array_agg(a), '{}') FROM jsonb_array_elements_text(
         CASE WHEN jsonb_typeof(x->'agreed_by') = 'array' THEN x->'agreed_by' ELSE '[]' END) a) @> names THEN
      RETURN 'an agreed goal names every member in agreed_by'; END IF;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'days', '[]')) LOOP
    IF coalesce(x->>'date', '') !~ '^\d{4}-\d{2}-\d{2}$' THEN RETURN 'a day''s date is YYYY-MM-DD'; END IF;
    IF jsonb_typeof(coalesce(x->'milestones', '[]')) <> 'array' THEN RETURN 'milestones are a list'; END IF;
    FOR y IN SELECT * FROM jsonb_array_elements(coalesce(x->'milestones', '[]')) LOOP
      IF coalesce(y->>'id', '') = '' OR coalesce(y->>'text', '') = '' THEN RETURN 'every milestone needs an id and text'; END IF;
      IF y->>'status' NOT IN ('todo', 'doing', 'done') THEN RETURN 'a milestone is todo, doing or done'; END IF;
    END LOOP;
  END LOOP;
  FOR x IN SELECT * FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) LOOP
    IF coalesce(x->>'id', '') = '' OR jsonb_typeof(coalesce(x->'items', '[]')) <> 'array' THEN RETURN 'every checklist needs an id and items'; END IF;
    FOR y IN SELECT * FROM jsonb_array_elements(coalesce(x->'items', '[]')) LOOP
      IF coalesce(y->>'id', '') = '' OR coalesce(y->>'text', '') = '' THEN RETURN 'every item needs an id and text'; END IF;
      IF y->>'status' NOT IN ('todo', 'doing', 'done') THEN RETURN 'an item is todo, doing or done'; END IF;
    END LOOP;
  END LOOP;
  RETURN NULL;
END $$;

-- What an agent may not change: everything but milestone and item status
-- (with notes and evidence) and the proposed goals.
CREATE FUNCTION projects.fixed(doc jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object(
    'rest', doc - 'days' - 'checklists' - 'goals',
    'goals', (SELECT coalesce(jsonb_agg(g ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'goals', '[]')) WITH ORDINALITY AS z(g, i)
              WHERE g->>'status' = 'agreed'),
    'days', (SELECT coalesce(jsonb_agg(jsonb_build_object('day', d - 'milestones', 'milestones',
               (SELECT coalesce(jsonb_agg(m - 'status' - 'note' ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(d->'milestones', '[]')) WITH ORDINALITY AS x(m, o)))
             ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'days', '[]')) WITH ORDINALITY AS y(d, i)),
    'checklists', (SELECT coalesce(jsonb_agg(jsonb_build_object('list', c - 'items', 'items',
               (SELECT coalesce(jsonb_agg(t - 'status' - 'evidence' ORDER BY o), '[]') FROM jsonb_array_elements(coalesce(c->'items', '[]')) WITH ORDINALITY AS x(t, o)))
             ORDER BY i), '[]') FROM jsonb_array_elements(coalesce(doc->'checklists', '[]')) WITH ORDINALITY AS y(c, i)))
$$;

-- Who pubkey is in slug: 'member', 'agent' or ''. One query whether or not the project exists.
CREATE FUNCTION projects.role(want text, pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE
    WHEN EXISTS (SELECT 1 FROM projects.docs d, jsonb_array_elements(d.doc->'members') m WHERE d.slug = want AND m->>'pubkey' = pk) THEN 'member'
    WHEN EXISTS (SELECT 1 FROM projects.agents a JOIN projects.docs d USING (slug) WHERE a.slug = want AND a.pubkey = pk) THEN 'agent'
    ELSE '' END
$$;
CREATE FUNCTION projects.name_of(want text, pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(
    (SELECT m->>'name' FROM projects.docs d, jsonb_array_elements(d.doc->'members') m WHERE d.slug = want AND m->>'pubkey' = pk LIMIT 1),
    (SELECT name FROM projects.agents WHERE slug = want AND pubkey = pk),
    left(pk, 8))
$$;

-- The live numbers from openagents.com (analytics.events, space web, robots
-- and test runs left out): today in Chicago and the last 7 days.
CREATE FUNCTION projects.numbers() RETURNS TABLE (label text, today bigint, week bigint) LANGUAGE sql STABLE AS $$
  WITH b AS (SELECT (extract(epoch FROM (date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago')) * 1000)::bigint AS today,
                    (extract(epoch FROM ((date_trunc('day', now() AT TIME ZONE 'America/Chicago') - interval '6 days') AT TIME ZONE 'America/Chicago')) * 1000)::bigint AS week),
  s AS (SELECT sid, min(ms) AS first_ms, bool_or(auto IS NOT NULL OR (ev = 'session-start' AND p ? 'agent')) AS automated
        FROM analytics.events, b WHERE ms >= b.week AND space = 'web' GROUP BY sid),
  k AS (SELECT sid, first_ms >= (SELECT today FROM b) AS t FROM s WHERE NOT automated),
  e AS (SELECT x.sid, x.ev, x.p, k.t FROM analytics.events x JOIN k USING (sid) WHERE x.ms >= (SELECT week FROM b)),
  q AS (SELECT sid, bool_or(t) AS t, max(CASE WHEN p->>'step' ~ '^[0-9]{1,2}$' THEN (p->>'step')::int ELSE 0 END) AS step
        FROM e WHERE ev = 'quest' GROUP BY sid)
  SELECT 'Sessions', count(*) FILTER (WHERE t), count(*) FROM k
  UNION ALL SELECT 'Visitors', count(DISTINCT p->>'pid') FILTER (WHERE t), count(DISTINCT p->>'pid') FROM e WHERE ev = 'session-start'
  UNION ALL SELECT 'Entered the Haunted House', count(DISTINCT sid) FILTER (WHERE t), count(DISTINCT sid) FROM e
            WHERE ev = 'world-enter' AND p->>'world' = 'haunted-house'
  UNION ALL SELECT * FROM (SELECT 'Quest step ' || n || CASE WHEN n = 8 THEN ' (done)' ELSE '' END,
                                  count(q.sid) FILTER (WHERE q.t), count(q.sid)
                           FROM generate_series(1, 8) n LEFT JOIN q ON q.step >= n GROUP BY n ORDER BY n) f
$$;

-- The state as JSON, for agents (private.sh get).
CREATE FUNCTION projects.state(want text, pk text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'slug', d.slug, 'version', d.version, 'updated_at', projects.chi(d.updated_ms), 'updated_by', projects.name_of(d.slug, d.updated_by),
    'you', jsonb_build_object('name', projects.name_of(want, pk), 'role', projects.role(want, pk)),
    'today', to_char(now() AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD'),
    'doc', d.doc,
    'agents', (SELECT coalesce(jsonb_agg(jsonb_build_object('name', name, 'pubkey', pubkey, 'role', role) ORDER BY added_ms), '[]') FROM projects.agents WHERE slug = want),
    'activity', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'at', projects.chi(at_ms), 'by', by_name, 'kind', kind, 'text', text) ORDER BY at_ms DESC), '[]')
                 FROM (SELECT * FROM projects.activity WHERE slug = want ORDER BY at_ms DESC LIMIT 50) a),
    'messages', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', id, 'at', projects.chi(at_ms), 'from', from_name, 'text', text, 'reply_to', reply_to) ORDER BY at_ms), '[]')
                 FROM (SELECT * FROM projects.messages WHERE slug = want ORDER BY at_ms DESC LIMIT 50) m),
    'analytics', (SELECT jsonb_agg(jsonb_build_object('label', label, 'today', today, 'week', week)) FROM projects.numbers()))
  FROM projects.docs d WHERE d.slug = want
$$;

-- The state as lines, for the page (private.coax): one "KIND|value|..." line each.
--   Y|you|role   P|title|summary|version|updated|by   T|today|now
--   G|id|status|agreed by|text   D|date|label   M|date|id|status|owner|text|note
--   C|id|title   I|list|id|status|text|evidence   A|id|at|by|kind|text (newest first)
--   X|id|at|from|text|reply to (oldest first)   N|label|today|7 days   S|source   L|label|url
CREATE FUNCTION projects.lines(want text, pk text) RETURNS text LANGUAGE sql STABLE AS $$
  WITH d AS (SELECT * FROM projects.docs WHERE slug = want)
  SELECT string_agg(line, E'\n' ORDER BY o1, o2) || E'\n' FROM (
    SELECT 0 AS o1, 0::bigint AS o2, 'Y|' || projects.v(projects.name_of(want, pk)) || '|' || projects.role(want, pk) AS line
    UNION ALL SELECT 1, 0, 'P|' || projects.v(d.doc->>'title') || '|' || projects.v(d.doc->>'summary') || '|' || d.version || '|'
                  || projects.chi(d.updated_ms) || '|' || projects.v(projects.name_of(want, d.updated_by)) FROM d
    UNION ALL SELECT 2, 0, 'T|' || to_char(now() AT TIME ZONE 'America/Chicago', 'YYYY-MM-DD') || '|' || to_char(now() AT TIME ZONE 'America/Chicago', 'HH24:MI')
    UNION ALL SELECT 3, i, 'G|' || projects.v(g->>'id') || '|' || projects.v(g->>'status') || '|'
                  || projects.v((SELECT string_agg(a, ', ') FROM jsonb_array_elements_text(CASE WHEN jsonb_typeof(g->'agreed_by') = 'array' THEN g->'agreed_by' ELSE '[]' END) a))
                  || '|' || projects.v(g->>'text')
             FROM d, jsonb_array_elements(coalesce(d.doc->'goals', '[]')) WITH ORDINALITY AS x(g, i)
    UNION ALL SELECT 4, i * 1000, 'D|' || projects.v(y->>'date') || '|' || upper(to_char(to_date(y->>'date', 'YYYY-MM-DD'), 'Dy Mon FMDD'))
             FROM d, jsonb_array_elements(coalesce(d.doc->'days', '[]')) WITH ORDINALITY AS z(y, i)
    UNION ALL SELECT 4, i * 1000 + o, 'M|' || projects.v(y->>'date') || '|' || projects.v(m->>'id') || '|' || projects.v(m->>'status') || '|'
                  || projects.v(m->>'owner') || '|' || projects.v(m->>'text') || '|' || projects.v(m->>'note')
             FROM d, jsonb_array_elements(coalesce(d.doc->'days', '[]')) WITH ORDINALITY AS z(y, i),
                  jsonb_array_elements(coalesce(y->'milestones', '[]')) WITH ORDINALITY AS w(m, o)
    UNION ALL SELECT 5, i * 1000, 'C|' || projects.v(c->>'id') || '|' || projects.v(c->>'title')
             FROM d, jsonb_array_elements(coalesce(d.doc->'checklists', '[]')) WITH ORDINALITY AS z(c, i)
    UNION ALL SELECT 5, i * 1000 + o, 'I|' || projects.v(c->>'id') || '|' || projects.v(t->>'id') || '|' || projects.v(t->>'status') || '|'
                  || projects.v(t->>'text') || '|' || projects.v(t->>'evidence')
             FROM d, jsonb_array_elements(coalesce(d.doc->'checklists', '[]')) WITH ORDINALITY AS z(c, i),
                  jsonb_array_elements(coalesce(c->'items', '[]')) WITH ORDINALITY AS w(t, o)
    UNION ALL SELECT 6, -a.at_ms, 'A|' || a.id || '|' || projects.chi(a.at_ms) || '|' || projects.v(a.by_name) || '|' || a.kind || '|' || projects.v(a.text)
             FROM (SELECT * FROM projects.activity WHERE slug = want ORDER BY at_ms DESC LIMIT 40) a
    UNION ALL SELECT 7, m.id, 'X|' || m.id || '|' || projects.chi(m.at_ms) || '|' || projects.v(m.from_name) || '|' || projects.v(m.text) || '|' || coalesce(m.reply_to::text, '')
             FROM (SELECT * FROM projects.messages WHERE slug = want ORDER BY at_ms DESC LIMIT 40) m
    UNION ALL SELECT 8, n, 'N|' || projects.v(label) || '|' || today || '|' || week FROM (SELECT *, row_number() OVER () AS n FROM projects.numbers()) f
    UNION ALL SELECT 9, 0, 'S|Live from openagents.com, robots left out. The old playtest''s data arrives with pt-import-demo-data.'
    UNION ALL SELECT 10, i, 'L|' || projects.v(l->>'label') || '|' || projects.v(l->>'url')
             FROM d, jsonb_array_elements(coalesce(d.doc->'links', '[]')) WITH ORDINALITY AS z(l, i)
  ) all_lines WHERE EXISTS (SELECT 1 FROM d)
$$;

-- A member's session from its cookie token, or NULL.
CREATE FUNCTION projects.session(want text, token text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.pubkey FROM projects.sessions s
  WHERE s.token_sha256 = projects.sha(coalesce(token, '')) AND s.slug = want AND s.expires_ms > projects.now_ms()
    AND projects.role(want, s.pubkey) = 'member'
$$;

-- One request. op is private.c's; who:
--   pubkey  a verified NIP-98 signer (empty: none)    session  the cookie's token (empty: none)
--   admin   the signer is in OA_ADMIN_PUBKEYS           format  'lines' for the page's state
-- The answer: {status, body} and, for a sign-in, cookie (a session token for Set-Cookie).
CREATE FUNCTION projects.api(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  admin boolean := coalesce((who->>'admin')::boolean, false);
  now_ms bigint := projects.now_ms();
  role text := '';
  me text;
  cur projects.docs;
  nd jsonb;
  why text;
  t text;
  n bigint;
  rec record;
BEGIN
  IF want !~ '^[a-z0-9][a-z0-9-]{0,63}$' THEN RETURN projects.no(); END IF;
  -- A sign-in link: the token is the whole proof, good once.
  IF op = 'redeem' THEN
    UPDATE projects.invites SET used_ms = now_ms
      WHERE token_sha256 = projects.sha(coalesce(body->>'token', '')) AND slug = want AND used_ms IS NULL AND expires_ms > now_ms
      RETURNING pubkey INTO me;
    IF me IS NULL OR projects.role(want, me) <> 'member' THEN RETURN projects.no(); END IF;
    t := projects.token();
    INSERT INTO projects.sessions VALUES (projects.sha(t), want, me, now_ms, now_ms + 30 * 86400000::bigint);
    RETURN projects.r(200, '{"ok":true}') || jsonb_build_object('cookie', t);
  END IF;
  -- Who is asking: a signer, else the cookie's member.
  IF pk IS NULL THEN pk := projects.session(want, nullif(who->>'session', '')); END IF;
  IF pk IS NOT NULL THEN role := projects.role(want, pk); END IF;
  IF op = 'shell' THEN RETURN projects.r(CASE WHEN role = 'member' THEN 200 ELSE 404 END, '{}'); END IF;
  SELECT * INTO cur FROM projects.docs WHERE slug = want;
  -- The first push: an admin's, for a slug nobody has.
  IF op = 'push' AND cur.slug IS NULL AND admin AND pk IS NOT NULL THEN role := 'first'; END IF;
  IF role = '' THEN RETURN projects.no(); END IF;
  me := projects.name_of(want, pk);

  IF op = 'state' THEN
    IF who->>'format' = 'lines' THEN RETURN projects.r(200, to_jsonb(projects.lines(want, pk))); END IF;
    RETURN projects.r(200, projects.state(want, pk));
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
    RETURN projects.r(200, jsonb_build_object('version', coalesce(cur.version, 0) + 1));
  ELSIF op = 'note' THEN
    t := btrim(coalesce(body->>'text', ''));
    IF t = '' OR length(t) > 4000 THEN RETURN projects.r(400, '{"error":"text is 1 to 4000 characters"}'); END IF;
    IF coalesce(body->>'kind', 'note') NOT IN ('note', 'plan', 'status') THEN RETURN projects.r(400, '{"error":"kind is note, plan or status"}'); END IF;
    INSERT INTO projects.activity (slug, at_ms, by_pubkey, by_name, kind, text)
      VALUES (want, now_ms, pk, left(coalesce(nullif(btrim(body->>'by'), ''), me), 60), coalesce(body->>'kind', 'note'), t) RETURNING id INTO n;
    RETURN projects.r(200, jsonb_build_object('id', n));
  ELSIF op = 'status' THEN
    -- one milestone's or item's status (and note or evidence), as a new version
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
    -- members' messages nobody has answered yet, oldest first
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
      INSERT INTO projects.agents VALUES (want, me, left(btrim(body->>'name'), 60), 'agent', now_ms, pk)
        ON CONFLICT (slug, pubkey) DO UPDATE SET name = EXCLUDED.name;
    END IF;
    RETURN projects.r(200, '{"ok":true}');
  END IF;
  RETURN projects.no();
END $$;
