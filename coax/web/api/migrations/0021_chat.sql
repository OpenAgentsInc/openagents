-- openagents.com/chat (docs/chat.md): a direct chat with a Claude agent for
-- general questions, admins only for now. A conversation is a managed
-- session (openagents.agent.managed.v1 ManagedSession: idle or running), each
-- message a turn the hosted runner (coax/web/agent/, general mode) takes, and
-- what the turn did its events (SessionEvent types: user.message,
-- user.interrupt, agent.message, agent.tool_use, session.status_running,
-- session.status_idle, session.error). Each conversation keeps the CLI
-- session id its turns resume.
--
-- A turn runs on the asker's own credential when one is saved (their spend,
-- no cap from us), else on OpenAgents' key under a daily cap for /chat across
-- every model (who.cap_usd, OA_CHAT_DAILY_USD). Saved credentials are sealed
-- by the runner (AES-256-GCM, its key in Secret Manager); this database keeps
-- only the sealed bytes, the kind and the last four characters. Nothing here
-- can open them.

CREATE SCHEMA IF NOT EXISTS chat;

CREATE FUNCTION chat.now_ms() RETURNS bigint LANGUAGE sql VOLATILE AS
  $$ SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint $$;
CREATE FUNCTION chat.r(status int, body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT jsonb_build_object('status', status, 'body', body) $$;
CREATE FUNCTION chat.chi(ms bigint) RETURNS text LANGUAGE sql STABLE AS
  $$ SELECT to_char(to_timestamp(ms / 1000.0) AT TIME ZONE 'America/Chicago', 'Mon DD HH24:MI') $$;
-- A value in a page line: '|' and line breaks would split it.
CREATE FUNCTION chat.v(t text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT regexp_replace(coalesce(t, ''), '[|\r\n\t]', ' ', 'g') $$;
-- A long text in a page line: line breaks become \n, a backslash \\, '|' \p.
CREATE FUNCTION chat.vv(t text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT replace(replace(replace(replace(replace(coalesce(t, ''), E'\\', E'\\\\'), '|', E'\\p'), E'\r', ''), E'\n', E'\\n'), E'\t', '  ') $$;
-- The models /chat offers; nothing else is accepted.
CREATE FUNCTION chat.model_ok(m text) RETURNS boolean LANGUAGE sql IMMUTABLE AS
  $$ SELECT m IN ('claude-opus-5-5', 'claude-sonnet-5-5', 'claude-haiku-5-5') $$;

CREATE TABLE chat.conversations (
  id bigserial PRIMARY KEY,
  owner text NOT NULL,                      -- the asker's pubkey
  title text NOT NULL DEFAULT '',
  model text NOT NULL CHECK (chat.model_ok(model)),
  session_id uuid NOT NULL DEFAULT gen_random_uuid(),   -- the CLI session its turns resume
  status text NOT NULL DEFAULT 'idle' CHECK (status IN ('idle', 'running')),
  created_ms bigint NOT NULL,
  updated_ms bigint NOT NULL
);
CREATE INDEX conversations_owner ON chat.conversations (owner, updated_ms DESC);

CREATE TABLE chat.turns (
  id bigserial PRIMARY KEY,
  conv bigint NOT NULL REFERENCES chat.conversations (id) ON DELETE CASCADE,
  owner text NOT NULL,
  text text NOT NULL,
  model text NOT NULL CHECK (chat.model_ok(model)),
  credential text NOT NULL DEFAULT '' CHECK (credential IN ('', 'own', 'openagents')),   -- set when it starts
  status text NOT NULL CHECK (status IN ('queued', 'running', 'done', 'failed', 'stopped', 'refused')),
  stop boolean NOT NULL DEFAULT false,
  asked_ms bigint NOT NULL,
  started_ms bigint,
  ended_ms bigint,
  resumed boolean NOT NULL DEFAULT false,   -- the runner resumed the conversation's CLI session
  cost_usd numeric(10, 4) NOT NULL DEFAULT 0,
  working text NOT NULL DEFAULT '',         -- one line: what it is doing now
  answer text NOT NULL DEFAULT '',
  error text NOT NULL DEFAULT ''
);
CREATE INDEX turns_conv ON chat.turns (conv, id);
CREATE INDEX turns_queue ON chat.turns (status, id) WHERE status IN ('queued', 'running');

CREATE TABLE chat.events (
  id bigserial PRIMARY KEY,
  turn bigint NOT NULL REFERENCES chat.turns (id) ON DELETE CASCADE,
  at_ms bigint NOT NULL,
  type text NOT NULL,
  data jsonb NOT NULL
);
CREATE INDEX events_turn ON chat.events (turn, id);

-- A person's own credential, sealed by the runner. One a person.
CREATE TABLE chat.credentials (
  owner text PRIMARY KEY,
  kind text NOT NULL CHECK (kind IN ('api', 'oauth')),
  last4 text NOT NULL CHECK (last4 ~ '^[A-Za-z0-9_-]{4}$'),
  sealed text NOT NULL,                     -- base64 of nonce, ciphertext and tag
  expired boolean NOT NULL DEFAULT false,
  saved_ms bigint NOT NULL
);

-- Today's spend on OpenAgents' key in Chicago, every model and person together.
CREATE FUNCTION chat.spent() RETURNS numeric LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(cost_usd), 0) FROM chat.turns
  WHERE credential = 'openagents'
    AND asked_ms >= (extract(epoch FROM (date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago')) * 1000)::bigint
$$;

CREATE FUNCTION chat.cred_label(kind text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE kind WHEN 'api' THEN 'Anthropic API key' WHEN 'oauth' THEN 'Claude subscription token (from claude setup-token)' ELSE '' END
$$;

-- What the page draws, as lines:
--   Z|spent today|cap|credential kind (api, oauth or "")|its last four|expired (1/0)|busy (1/0)|conversation shown (or 0)
--   C|id|title|model|updated|busy                               the asker's conversations, newest first
--   S|id|title|model|resumes (1 once a turn has run)            the one shown
--   U|id|status|at|model|credential|text|answer|error|working|cost   its turns, oldest first (text and answer escaped: chat.vv)
--   T|turn|tool|detail                                          each turn's tool uses (the last 12)
CREATE FUNCTION chat.lines(pk text, want bigint, cap numeric) RETURNS text LANGUAGE sql STABLE AS $$
  WITH me AS (SELECT * FROM chat.credentials WHERE owner = pk),
  cs AS (SELECT c.*, EXISTS (SELECT 1 FROM chat.turns t WHERE t.conv = c.id AND t.status IN ('queued', 'running')) AS busy
         FROM chat.conversations c WHERE c.owner = pk ORDER BY c.updated_ms DESC LIMIT 60),
  shown AS (SELECT * FROM chat.conversations WHERE owner = pk AND id = coalesce(want, (SELECT id FROM cs ORDER BY updated_ms DESC LIMIT 1))),
  t AS (SELECT * FROM (SELECT * FROM chat.turns WHERE conv = (SELECT id FROM shown) ORDER BY id DESC LIMIT 40) x)
  SELECT string_agg(line, E'\n' ORDER BY o1, o2) || E'\n' FROM (
    SELECT 0 AS o1, 0::bigint AS o2, 'Z|' || to_char(chat.spent(), 'FM9990.00') || '|' || to_char(cap, 'FM9990.00') || '|'
         || coalesce((SELECT kind FROM me), '') || '|' || coalesce((SELECT last4 FROM me), '') || '|'
         || CASE WHEN coalesce((SELECT expired FROM me), false) THEN '1' ELSE '0' END || '|'
         || CASE WHEN EXISTS (SELECT 1 FROM t WHERE status IN ('queued', 'running')) THEN '1' ELSE '0' END || '|'
         || coalesce((SELECT id FROM shown), 0) AS line
    UNION ALL SELECT 1, row_number() OVER (ORDER BY updated_ms DESC), 'C|' || id || '|' || chat.v(left(title, 80)) || '|' || model || '|' || chat.chi(updated_ms) || '|' || CASE WHEN busy THEN '1' ELSE '0' END FROM cs
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

-- The request function. who: {pubkey, may (an admin, for now), runner (the runner's key), cap_usd}.
--   member:  state {c}; new {model}; send {c, text, model}; model {c, model}; stop {c}; key_set {kind, last4, sealed}; key_remove
--   runner:  next {}; event {turn, events: [{type, data}]}; end {turn, status, answer, error, cost_usd, resumed, credential_expired}
-- Nothing else, and nobody else.
CREATE FUNCTION chat.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  may boolean := coalesce((who->>'may')::boolean, false);
  runner boolean := coalesce((who->>'runner')::boolean, false);
  cap numeric := coalesce(nullif(who->>'cap_usd', '')::numeric, 10);
  now_ms bigint := chat.now_ms();
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  cid bigint := CASE WHEN coalesce(b->>'c', '') ~ '^[0-9]{1,18}$' THEN (b->>'c')::bigint END;
  m text := coalesce(b->>'model', '');
  t text;
  cv chat.conversations;
  tn chat.turns;
  cr chat.credentials;
  x jsonb;
  st text;
BEGIN
  IF op IN ('next', 'event', 'end') THEN
    IF NOT runner THEN RETURN chat.r(403, '{"error":"the runner only"}'); END IF;
  ELSIF pk IS NULL OR NOT may THEN
    RETURN chat.r(403, '{"error":"admins only"}');
  END IF;

  IF op = 'state' THEN
    RETURN chat.r(200, to_jsonb(coalesce(chat.lines(pk, cid, cap), '')));
  ELSIF op = 'new' THEN
    IF NOT chat.model_ok(m) THEN m := 'claude-opus-5-5'; END IF;
    INSERT INTO chat.conversations (owner, model, created_ms, updated_ms) VALUES (pk, m, now_ms, now_ms) RETURNING * INTO cv;
    RETURN chat.r(200, jsonb_build_object('c', cv.id));
  ELSIF op = 'model' THEN
    IF NOT chat.model_ok(m) THEN RETURN chat.r(400, '{"error":"that model is not offered"}'); END IF;
    UPDATE chat.conversations SET model = m WHERE id = cid AND owner = pk RETURNING * INTO cv;
    IF cv.id IS NULL THEN RETURN chat.r(404, '{"error":"no such conversation"}'); END IF;
    RETURN chat.r(200, jsonb_build_object('c', cv.id, 'model', m));
  ELSIF op = 'send' THEN
    t := btrim(coalesce(b->>'text', ''));
    IF t = '' OR length(t) > 8000 THEN RETURN chat.r(400, '{"error":"a message is 1 to 8000 characters"}'); END IF;
    IF m <> '' AND NOT chat.model_ok(m) THEN RETURN chat.r(400, '{"error":"that model is not offered"}'); END IF;
    IF cid IS NULL THEN
      INSERT INTO chat.conversations (owner, model, created_ms, updated_ms)
        VALUES (pk, CASE WHEN m = '' THEN 'claude-opus-5-5' ELSE m END, now_ms, now_ms) RETURNING * INTO cv;
    ELSE
      SELECT * INTO cv FROM chat.conversations WHERE id = cid AND owner = pk FOR UPDATE;
      IF cv.id IS NULL THEN RETURN chat.r(404, '{"error":"no such conversation"}'); END IF;
    END IF;
    IF EXISTS (SELECT 1 FROM chat.turns WHERE conv = cv.id AND status IN ('queued', 'running')) THEN
      RETURN chat.r(409, '{"error":"the agent is still answering; wait or press STOP"}');
    END IF;
    SELECT * INTO cr FROM chat.credentials WHERE owner = pk;
    IF (cr.owner IS NULL OR cr.expired) AND chat.spent() >= cap THEN
      RETURN chat.r(429, jsonb_build_object('error', 'Today''s $' || to_char(cap, 'FM9990') || ' limit for /chat on the OpenAgents key is reached. Save your own key, or come back after midnight Chicago time.', 'cap', true));
    END IF;
    IF m <> '' THEN cv.model := m; END IF;
    UPDATE chat.conversations SET model = cv.model, updated_ms = now_ms, status = 'running',
      title = CASE WHEN title = '' THEN left(regexp_replace(t, '\s+', ' ', 'g'), 80) ELSE title END WHERE id = cv.id;
    INSERT INTO chat.turns (conv, owner, text, model, status, asked_ms) VALUES (cv.id, pk, t, cv.model, 'queued', now_ms) RETURNING * INTO tn;
    INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, 'user.message', jsonb_build_object('length', length(t)));
    RETURN chat.r(200, jsonb_build_object('c', cv.id, 'turn', tn.id, 'model', cv.model)) || '{"wake":true}';
  ELSIF op = 'stop' THEN
    SELECT * INTO tn FROM chat.turns WHERE conv = cid AND owner = pk AND status IN ('queued', 'running') ORDER BY id DESC LIMIT 1 FOR UPDATE;
    IF tn.id IS NULL THEN RETURN chat.r(409, '{"error":"nothing is running"}'); END IF;
    IF tn.status = 'queued' THEN
      UPDATE chat.turns SET status = 'stopped', stop = true, ended_ms = now_ms WHERE id = tn.id;
      UPDATE chat.conversations SET status = 'idle' WHERE id = tn.conv;
    ELSE
      UPDATE chat.turns SET stop = true WHERE id = tn.id;
    END IF;
    INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, 'user.interrupt', '{}');
    RETURN chat.r(200, jsonb_build_object('turn', tn.id));
  ELSIF op = 'key_set' THEN
    IF coalesce(b->>'kind', '') NOT IN ('api', 'oauth') OR coalesce(b->>'last4', '') !~ '^[A-Za-z0-9_-]{4}$'
       OR coalesce(b->>'sealed', '') !~ '^[A-Za-z0-9+/=]+$' OR length(b->>'sealed') NOT BETWEEN 40 AND 4000 THEN
      RETURN chat.r(400, '{"error":"not a sealed credential"}');
    END IF;
    INSERT INTO chat.credentials (owner, kind, last4, sealed, saved_ms) VALUES (pk, b->>'kind', b->>'last4', b->>'sealed', now_ms)
      ON CONFLICT (owner) DO UPDATE SET kind = excluded.kind, last4 = excluded.last4, sealed = excluded.sealed, expired = false, saved_ms = excluded.saved_ms;
    RETURN chat.r(200, jsonb_build_object('kind', b->>'kind', 'label', chat.cred_label(b->>'kind'), 'last4', b->>'last4'));
  ELSIF op = 'key_remove' THEN
    DELETE FROM chat.credentials WHERE owner = pk;
    RETURN chat.r(200, '{"removed":true}');

  -- the runner
  ELSIF op = 'next' THEN
    -- a turn the runner never ended fails after 20 minutes
    UPDATE chat.turns SET status = 'failed', error = 'the agent stopped answering', ended_ms = now_ms
      WHERE status = 'running' AND started_ms < now_ms - 1200000;
    SELECT * INTO tn FROM chat.turns WHERE status = 'queued' ORDER BY id LIMIT 1 FOR UPDATE SKIP LOCKED;
    IF tn.id IS NULL THEN RETURN chat.r(200, '{"turn":null}'); END IF;
    SELECT * INTO cv FROM chat.conversations WHERE id = tn.conv;
    SELECT * INTO cr FROM chat.credentials WHERE owner = tn.owner AND NOT expired;
    IF cr.owner IS NULL AND chat.spent() >= cap THEN
      UPDATE chat.turns SET status = 'refused', ended_ms = now_ms,
        error = 'Today''s $' || to_char(cap, 'FM9990') || ' limit for /chat on the OpenAgents key is reached.' WHERE id = tn.id;
      UPDATE chat.conversations SET status = 'idle' WHERE id = tn.conv;
      INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, 'session.error', '{"error":"over the daily cap"}');
      RETURN chat.r(200, jsonb_build_object('turn', NULL, 'refused', tn.id));
    END IF;
    UPDATE chat.turns SET status = 'running', started_ms = now_ms, working = 'starting',
      credential = CASE WHEN cr.owner IS NULL THEN 'openagents' ELSE 'own' END WHERE id = tn.id;
    INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, 'session.status_running', '{}');
    RETURN chat.r(200, jsonb_build_object(
      'turn', jsonb_build_object('id', tn.id, 'conv', tn.conv, 'text', tn.text, 'model', tn.model, 'by', 'You', 'session', cv.session_id::text),
      'spent', chat.spent(), 'cap', CASE WHEN cr.owner IS NULL THEN cap ELSE 0 END,
      'credential', CASE WHEN cr.owner IS NULL THEN NULL ELSE jsonb_build_object('kind', cr.kind, 'owner', cr.owner, 'sealed', cr.sealed) END,
      'history', (SELECT coalesce(jsonb_agg(jsonb_build_object('id', h.id, 'by', 'You', 'text', h.text, 'answer', h.answer) ORDER BY h.id), '[]')
                  FROM (SELECT * FROM chat.turns WHERE conv = tn.conv AND status = 'done' AND id < tn.id ORDER BY id DESC LIMIT 10) h)));
  END IF;

  -- event and end: a running turn
  SELECT * INTO tn FROM chat.turns WHERE id = CASE WHEN coalesce(b->>'turn', '') ~ '^[0-9]{1,18}$' THEN (b->>'turn')::bigint END AND status = 'running';
  IF tn.id IS NULL THEN RETURN chat.r(409, '{"error":"that turn is not running"}'); END IF;
  IF op = 'event' THEN
    IF jsonb_typeof(b->'events') <> 'array' OR jsonb_array_length(b->'events') > 50 THEN RETURN chat.r(400, '{"error":"events is a list of at most 50"}'); END IF;
    FOR x IN SELECT * FROM jsonb_array_elements(b->'events') LOOP
      IF x->>'type' = 'working' THEN
        UPDATE chat.turns SET working = left(coalesce(x->'data'->>'text', ''), 160) WHERE id = tn.id;
      ELSIF x->>'type' IN ('agent.message', 'agent.tool_use', 'session.error') THEN
        INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, x->>'type',
          CASE WHEN x->>'type' = 'agent.message' THEN jsonb_build_object('length', length(coalesce(x->'data'->>'text', '')))
               ELSE jsonb_build_object('name', left(coalesce(x->'data'->>'name', ''), 60), 'detail', left(coalesce(x->'data'->>'detail', ''), 300),
                                       'error', left(coalesce(x->'data'->>'error', ''), 300)) END);
      END IF;
    END LOOP;
    RETURN chat.r(200, jsonb_build_object('stop', (SELECT stop FROM chat.turns WHERE id = tn.id)));
  ELSIF op = 'end' THEN
    st := coalesce(b->>'status', 'failed');
    IF st NOT IN ('done', 'failed', 'stopped') THEN st := 'failed'; END IF;
    IF tn.stop AND st = 'done' THEN st := 'stopped'; END IF;
    UPDATE chat.turns SET status = st, ended_ms = now_ms, working = '',
      answer = left(coalesce(b->>'answer', ''), 60000), error = left(coalesce(b->>'error', ''), 2000),
      resumed = coalesce((b->>'resumed')::boolean, false),
      cost_usd = least(greatest(coalesce(nullif(b->>'cost_usd', '')::numeric, 0), 0), 100) WHERE id = tn.id;
    UPDATE chat.conversations SET status = 'idle', updated_ms = now_ms WHERE id = tn.conv;
    IF coalesce((b->>'credential_expired')::boolean, false) AND tn.credential = 'own' THEN
      UPDATE chat.credentials SET expired = true WHERE owner = tn.owner;
    END IF;
    INSERT INTO chat.events (turn, at_ms, type, data) VALUES (tn.id, now_ms, 'session.status_idle', jsonb_build_object('status', st));
    RETURN chat.r(200, jsonb_build_object('turn', tn.id, 'status', st));
  END IF;
  RETURN chat.r(404, '{"error":"not found"}');
END
$$;
