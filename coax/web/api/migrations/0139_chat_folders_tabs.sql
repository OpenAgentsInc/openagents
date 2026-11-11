-- openagents.com/chat's folders and tabs, and its conversations as the
-- shared session record (docs/chat.md, "Folders, tabs and the record";
-- docs/apps/2026-11-10-apps-roadmap.md, Step 2; GitHub #13156). The desktop
-- app's desk lists its sessions by folder, the folder a session works in, and
-- keeps the open ones as tabs across the top. /chat does the same:
--   - a conversation's folder is the checkout its turns read. Today that is
--     always the public openagents repository (the runner's one checkout), so
--     every conversation is in "openagents"; chat.folder_ok lists the folders
--     the runner can read, and nothing else is accepted.
--   - its tab is its place among the asker's open tabs (1, 2, ...), or NULL
--     when it is not open. A message opens its conversation's tab; the page
--     opens one when a conversation is picked and closes it with its x.
--   - chat.record writes a conversation in the shared session log format
--     (coax/lib/agent/session/log.coax, openagents.session.v1), the same
--     lines the desktop app, Coder and the agent boards read.
-- The old request function is kept as chat.api_v1; chat.api answers the new
-- operations (tab, record) and passes everything else to it.

CREATE FUNCTION chat.folder_ok(f text) RETURNS boolean LANGUAGE sql IMMUTABLE AS
  $$ SELECT f IN ('openagents') $$;

ALTER TABLE chat.conversations
  ADD COLUMN folder text NOT NULL DEFAULT 'openagents' CHECK (chat.folder_ok(folder)),
  ADD COLUMN tab int CHECK (tab IS NULL OR tab > 0);
CREATE INDEX conversations_tabs ON chat.conversations (owner, tab) WHERE tab IS NOT NULL;

-- Opens a conversation's tab, after the last one; one already open keeps its place.
CREATE FUNCTION chat.tab_open(pk text, cid bigint) RETURNS int LANGUAGE plpgsql AS $$
DECLARE at int;
BEGIN
  SELECT tab INTO at FROM chat.conversations WHERE id = cid AND owner = pk FOR UPDATE;
  IF at IS NOT NULL THEN RETURN at; END IF;
  SELECT coalesce(max(tab), 0) + 1 INTO at FROM chat.conversations WHERE owner = pk AND tab IS NOT NULL;
  UPDATE chat.conversations SET tab = at WHERE id = cid AND owner = pk;
  RETURN at;
END $$;
-- Closes it; the tabs after it move up one.
CREATE FUNCTION chat.tab_close(pk text, cid bigint) RETURNS void LANGUAGE plpgsql AS $$
DECLARE at int;
BEGIN
  SELECT tab INTO at FROM chat.conversations WHERE id = cid AND owner = pk FOR UPDATE;
  IF at IS NULL THEN RETURN; END IF;
  UPDATE chat.conversations SET tab = NULL WHERE id = cid AND owner = pk;
  UPDATE chat.conversations SET tab = tab - 1 WHERE owner = pk AND tab > at;
END $$;

-- A tool's kind in the session log (log.coax SL_TOOLS) from its name.
CREATE FUNCTION chat.tool_kind(name text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE name WHEN 'Read' THEN 'read' WHEN 'Grep' THEN 'search' WHEN 'Glob' THEN 'glob'
    WHEN 'WebSearch' THEN 'web' WHEN 'WebFetch' THEN 'fetch' WHEN 'Bash' THEN 'exec'
    WHEN 'Edit' THEN 'edit' WHEN 'Write' THEN 'write' WHEN 'TodoWrite' THEN 'todo' WHEN 'Task' THEN 'agent'
    ELSE CASE WHEN name LIKE 'mcp\_\_%' THEN 'mcp' ELSE 'tool' END END
$$;

-- A conversation as the shared session log, one JSON object a line:
--   {"kind":"session","schema":"openagents.session.v1","id":SESSION,"engine":"claude","cwd":FOLDER,"started_us":T,"title":T,"model":M}
--   then each turn: {"kind":"user","text":T}; each tool it used, {"kind":"tool",...} and its {"kind":"result",...}
--   (the tools' output is not kept here: the result is empty); {"kind":"reply","text":ANSWER};
--   a failure's {"kind":"notice","notice":"error"|"limit","text":WHY}; and once it ended,
--   {"kind":"end","ending":"finished"|"stopped"|"failed","ms":N,"steps":N,"cost_usd_e6":N}.
-- A turn still queued or running has no end line yet.
-- A string field, compact as Coder writes its log ("key":"value", no spaces).
CREATE FUNCTION chat.js(k text, v text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT to_json(k)::text || ':' || to_json(coalesce(v, ''))::text $$;
CREATE FUNCTION chat.jn(k text, v bigint) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT to_json(k)::text || ':' || coalesce(v, 0)::text $$;
CREATE FUNCTION chat.record(cid bigint) RETURNS text LANGUAGE sql STABLE AS $$
  WITH cv AS (SELECT * FROM chat.conversations WHERE id = cid),
  t AS (SELECT * FROM chat.turns WHERE conv = cid),
  tools AS (SELECT ev.* FROM chat.events ev WHERE ev.turn IN (SELECT id FROM t) AND ev.type = 'agent.tool_use'),
  lines AS (
    SELECT 0::bigint AS o1, 0::bigint AS o2, 0 AS o3, '{' || chat.js('kind', 'session') || ',' || chat.js('schema', 'openagents.session.v1') || ','
        || chat.js('id', cv.session_id::text) || ',' || chat.js('engine', 'claude') || ',' || chat.js('cwd', cv.folder) || ','
        || chat.jn('started_us', cv.created_ms * 1000) || ',' || chat.js('title', cv.title) || ',' || chat.js('model', cv.model) || '}' AS line FROM cv
    UNION ALL SELECT t.id, 0, 0, '{' || chat.js('kind', 'user') || ',' || chat.js('text', t.text) || '}' FROM t
    UNION ALL SELECT e.turn, e.id, 0, '{' || chat.js('kind', 'tool') || ','
        || chat.js('label', coalesce(e.data->>'name', 'tool') || CASE WHEN coalesce(e.data->>'detail', '') = '' THEN '' ELSE '(' || (e.data->>'detail') || ')' END) || ','
        || chat.js('tool', chat.tool_kind(coalesce(e.data->>'name', ''))) || ',' || chat.js('name', coalesce(e.data->>'name', 'tool')) || ','
        || chat.js('id', 'chat-' || e.id) || ',' || chat.js('detail', e.data->>'detail') || '}' FROM tools e
    UNION ALL SELECT e.turn, e.id, 1, '{' || chat.js('kind', 'result') || ',' || chat.js('text', '') || ',"error":false,' || chat.js('id', 'chat-' || e.id) || '}' FROM tools e
    UNION ALL SELECT t.id, 9223372036854775806, 0, '{' || chat.js('kind', 'reply') || ',' || chat.js('text', t.answer) || '}' FROM t WHERE t.answer <> ''
    UNION ALL SELECT t.id, 9223372036854775806, 1, '{' || chat.js('kind', 'notice') || ','
        || chat.js('notice', CASE WHEN t.status = 'refused' THEN 'limit' ELSE 'error' END) || ',' || chat.js('text', t.error) || '}'
      FROM t WHERE t.error <> '' AND t.status IN ('failed', 'refused')
    UNION ALL SELECT t.id, 9223372036854775807, 0, '{' || chat.js('kind', 'end') || ','
        || chat.js('ending', CASE t.status WHEN 'done' THEN 'finished' WHEN 'stopped' THEN 'stopped' ELSE 'failed' END) || ','
        || chat.jn('ms', greatest(coalesce(t.ended_ms - coalesce(t.started_ms, t.asked_ms), 0), 0)) || ','
        || chat.jn('steps', (SELECT count(*) FROM tools x WHERE x.turn = t.id)) || ','
        || chat.jn('cost_usd_e6', round(t.cost_usd * 1000000)::bigint) || '}'
      FROM t WHERE t.status IN ('done', 'failed', 'stopped', 'refused')
  )
  SELECT coalesce(string_agg(line, E'\n' ORDER BY o1, o2, o3) || E'\n', '') FROM lines
$$;

-- What the page draws, as lines (0021's, with the folder and the tabs):
--   Z|spent today|cap|credential kind|its last four|expired (1/0)|busy (1/0)|conversation shown (or 0)|record (2: folders and tabs)
--   C|id|title|model|updated|busy|folder|tab (its place among the open tabs, or empty)
--                                         the asker's 60 newest conversations and every open tab, newest first
--   S, U and T as before.
CREATE OR REPLACE FUNCTION chat.lines(pk text, want bigint, cap numeric) RETURNS text LANGUAGE sql STABLE AS $$
  WITH me AS (SELECT * FROM chat.credentials WHERE owner = pk),
  newest AS (SELECT id FROM chat.conversations WHERE owner = pk ORDER BY updated_ms DESC LIMIT 60),
  cs AS (SELECT c.*, EXISTS (SELECT 1 FROM chat.turns t WHERE t.conv = c.id AND t.status IN ('queued', 'running')) AS busy
         FROM chat.conversations c WHERE c.owner = pk AND (c.tab IS NOT NULL OR c.id IN (SELECT id FROM newest))),
  shown AS (SELECT * FROM chat.conversations WHERE owner = pk AND id = coalesce(want, (SELECT id FROM cs ORDER BY updated_ms DESC LIMIT 1))),
  t AS (SELECT * FROM (SELECT * FROM chat.turns WHERE conv = (SELECT id FROM shown) ORDER BY id DESC LIMIT 40) x)
  SELECT string_agg(line, E'\n' ORDER BY o1, o2) || E'\n' FROM (
    SELECT 0 AS o1, 0::bigint AS o2, 'Z|' || to_char(chat.spent(), 'FM9990.00') || '|' || to_char(cap, 'FM9990.00') || '|'
         || coalesce((SELECT kind FROM me), '') || '|' || coalesce((SELECT last4 FROM me), '') || '|'
         || CASE WHEN coalesce((SELECT expired FROM me), false) THEN '1' ELSE '0' END || '|'
         || CASE WHEN EXISTS (SELECT 1 FROM t WHERE status IN ('queued', 'running')) THEN '1' ELSE '0' END || '|'
         || coalesce((SELECT id FROM shown), 0) || '|2' AS line
    UNION ALL SELECT 1, row_number() OVER (ORDER BY updated_ms DESC, id DESC), 'C|' || id || '|' || chat.v(left(title, 80)) || '|' || model || '|' || chat.chi(updated_ms) || '|'
         || CASE WHEN busy THEN '1' ELSE '0' END || '|' || chat.v(folder) || '|' || coalesce(tab::text, '') FROM cs
    UNION ALL SELECT 2, 0, 'S|' || id || '|' || chat.v(left(title, 80)) || '|' || model || '|'
         || CASE WHEN EXISTS (SELECT 1 FROM chat.turns x WHERE x.conv = shown.id AND x.started_ms IS NOT NULL) THEN '1' ELSE '0' END FROM shown
    UNION ALL SELECT 3, t.id * 100, 'U|' || t.id || '|' || t.status || '|' || chat.chi(t.asked_ms) || '|' || t.model || '|' || t.credential || '|'
         || chat.vv(left(t.text, 8000)) || '|' || chat.vv(left(t.answer, 24000)) || '|' || chat.v(left(t.error, 400)) || '|' || chat.v(left(t.working, 160)) || '|'
         || to_char(t.cost_usd, 'FM9990.0000') FROM t
    UNION ALL SELECT 3, e.turn * 100 + 1 + e.k, 'T|' || e.turn || '|' || chat.v(left(e.data->>'name', 40)) || '|' || chat.v(left(coalesce(e.data->>'detail', ''), 200))
      FROM (SELECT ev.*, row_number() OVER (PARTITION BY ev.turn ORDER BY ev.id DESC) AS k FROM chat.events ev
            WHERE ev.turn IN (SELECT id FROM t) AND ev.type = 'agent.tool_use') e WHERE e.k <= 12
  ) all_lines
$$;

ALTER FUNCTION chat.api(text, jsonb, jsonb) RENAME TO api_v1;
-- The request function: 0021's operations (chat.api_v1), and
--   member:  tab {c, open (true/false)}   open or close a conversation's tab
--            record {c}                   the conversation as the session log (a string)
--            new and send also open the conversation's tab.
CREATE FUNCTION chat.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  may boolean := coalesce((who->>'may')::boolean, false);
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  cid bigint := CASE WHEN coalesce(b->>'c', '') ~ '^[0-9]{1,18}$' THEN (b->>'c')::bigint END;
  res jsonb;
  at int;
BEGIN
  IF op IN ('tab', 'record') THEN
    IF pk IS NULL OR NOT may THEN RETURN chat.r(403, '{"error":"admins only"}'); END IF;
    IF cid IS NULL OR NOT EXISTS (SELECT 1 FROM chat.conversations WHERE id = cid AND owner = pk) THEN
      RETURN chat.r(404, '{"error":"no such conversation"}');
    END IF;
    IF op = 'record' THEN RETURN chat.r(200, to_jsonb(chat.record(cid))); END IF;
    IF coalesce(b->>'open', 'true') = 'false' THEN
      PERFORM chat.tab_close(pk, cid);
      RETURN chat.r(200, jsonb_build_object('c', cid, 'tab', NULL));
    END IF;
    at := chat.tab_open(pk, cid);
    RETURN chat.r(200, jsonb_build_object('c', cid, 'tab', at));
  END IF;
  res := chat.api_v1(op, body, who);
  IF op IN ('new', 'send') AND (res->>'status')::int = 200 AND coalesce(res->'body'->>'c', '') ~ '^[0-9]{1,18}$' THEN
    PERFORM chat.tab_open(pk, (res->'body'->>'c')::bigint);
  END IF;
  RETURN res;
END $$;
