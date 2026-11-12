-- chat.record writes the session log as log.coax writes it (docs/protocol/session-log-v1.md;
-- C2 of docs/protocol/2026-11-11-compliance-plan.md, GitHub #13239). 0139 wrote an empty title,
-- model or tool detail as "title":"" where the log's writer leaves the field out, so a new
-- conversation's header (its title is '' until its first message) and a tool with no detail were
-- not the lines Coder writes for the same values. coax/web/api/test_zchat.py now reads every line
-- GET /api/chat/record returns through log.coax's reader and requires each to be written back
-- byte for byte (coax/lib/agent/session/log-profile.coax --read), so the SQL and the Coax can't
-- drift. Only those three fields change; every other line is as 0139 wrote it.

-- ,"key":"value" when there is a value, else nothing (log.coax sl_opt_str).
CREATE FUNCTION chat.jo(k text, v text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT CASE WHEN coalesce(v, '') = '' THEN '' ELSE ',' || chat.js(k, v) END $$;

-- A conversation as the shared session log, one JSON object a line (0139's, with the change above):
--   {"kind":"session","schema":"openagents.session.v1","id":SESSION,"engine":"claude","cwd":FOLDER,"started_us":T[,"title":T][,"model":M]}
--   then each turn: {"kind":"user","text":T}; each tool it used, {"kind":"tool","label":L,"tool":K,"name":N,"id":ID[,"detail":D]}
--   and its {"kind":"result","text":"","error":false,"id":ID}; {"kind":"reply","text":ANSWER}; a failure's
--   {"kind":"notice","notice":"error"|"limit","text":WHY}; and once it ended,
--   {"kind":"end","ending":"finished"|"stopped"|"failed","ms":N,"steps":N,"cost_usd_e6":N}.
CREATE OR REPLACE FUNCTION chat.record(cid bigint) RETURNS text LANGUAGE sql STABLE AS $$
  WITH cv AS (SELECT * FROM chat.conversations WHERE id = cid),
  t AS (SELECT * FROM chat.turns WHERE conv = cid),
  tools AS (SELECT ev.* FROM chat.events ev WHERE ev.turn IN (SELECT id FROM t) AND ev.type = 'agent.tool_use'),
  lines AS (
    SELECT 0::bigint AS o1, 0::bigint AS o2, 0 AS o3, '{' || chat.js('kind', 'session') || ',' || chat.js('schema', 'openagents.session.v1') || ','
        || chat.js('id', cv.session_id::text) || ',' || chat.js('engine', 'claude') || ',' || chat.js('cwd', cv.folder) || ','
        || chat.jn('started_us', cv.created_ms * 1000) || chat.jo('title', cv.title) || chat.jo('model', cv.model) || '}' AS line FROM cv
    UNION ALL SELECT t.id, 0, 0, '{' || chat.js('kind', 'user') || ',' || chat.js('text', t.text) || '}' FROM t
    UNION ALL SELECT e.turn, e.id, 0, '{' || chat.js('kind', 'tool') || ','
        || chat.js('label', coalesce(e.data->>'name', 'tool') || CASE WHEN coalesce(e.data->>'detail', '') = '' THEN '' ELSE '(' || (e.data->>'detail') || ')' END) || ','
        || chat.js('tool', chat.tool_kind(coalesce(e.data->>'name', ''))) || ',' || chat.js('name', coalesce(e.data->>'name', 'tool')) || ','
        || chat.js('id', 'chat-' || e.id) || chat.jo('detail', e.data->>'detail') || '}' FROM tools e
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
