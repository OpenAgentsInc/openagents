-- A project turn runs on the credential of the member who asked, when they
-- saved one (docs/chat.md, "Your own key"): the same store as /chat, one
-- per signed-in pubkey (chat.credentials, sealed by the runner). Their turns
-- have no cap from us and don't count toward the project's daily spend;
-- without one the turn runs on OpenAgents' key under the project's cap, as
-- before. Another member's credential is never used.
--
-- projects.api, wrapped:
--   key_owner    the member asking (a cookie, a site session or a signature), for KEY (private.c checks and seals)
--   key_remove   deletes the member's saved credential
--   next         the queued turn's asker's credential goes to the runner with the turn ({kind, owner, sealed}), cap 0
--   end          credential_expired: the asker's saved token is marked expired
--   talk         (lines) adds K|kind|last4|expired for the member asking
-- and each turn line (U) ends with the credential the turn used: own, openagents or ''.

ALTER TABLE projects.turns ADD COLUMN credential text NOT NULL DEFAULT '' CHECK (credential IN ('', 'own', 'openagents'));

CREATE OR REPLACE FUNCTION projects.spent(want text) RETURNS numeric LANGUAGE sql STABLE AS $$
  SELECT coalesce(sum(cost_usd), 0) FROM projects.turns
  WHERE slug = want AND credential <> 'own'
    AND asked_ms >= (extract(epoch FROM (date_trunc('day', now() AT TIME ZONE 'America/Chicago') AT TIME ZONE 'America/Chicago')) * 1000)::bigint
$$;

-- The member a request comes from, as projects.api resolves it, or NULL.
CREATE FUNCTION projects.member_key(want text, who jsonb) RETURNS text LANGUAGE plpgsql STABLE AS $$
DECLARE pk text := nullif(who->>'pubkey', '');
BEGIN
  IF pk IS NULL THEN pk := projects.session(want, nullif(who->>'session', '')); END IF;
  IF pk IS NULL AND nullif(who->>'site', '') IS NOT NULL THEN pk := projects.member_of(want, signin.session(who->>'site')); END IF;
  IF pk IS NOT NULL AND projects.role(want, pk) = '' THEN pk := projects.member_of(want, pk); END IF;
  IF pk IS NULL OR projects.role(want, pk) <> 'member' THEN RETURN NULL; END IF;
  RETURN pk;
END $$;

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
         || coalesce(t.undone_by::text, '') || '|' || projects.v(t.working) || '|' || coalesce(t.undo_of::text, '') || '|' || t.credential
      FROM t
    UNION ALL SELECT 1, e.turn * 1000 + 1 + row_number() OVER (PARTITION BY e.turn ORDER BY e.id),
         'E|' || e.turn || '|' || e.id || '|' || e.type || '|' || projects.v(left(projects.event_text(e.type, e.data), 300))
      FROM projects.events e WHERE e.turn IN (SELECT id FROM t) AND e.type IN ('agent.edit', 'agent.commit', 'session.error', 'user.interrupt')
  ) all_lines WHERE EXISTS (SELECT 1 FROM d)
$$;

ALTER FUNCTION projects.api(text, text, jsonb, jsonb) RENAME TO api_0019;
CREATE FUNCTION projects.api(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m text;
  tn projects.turns;
  cr chat.credentials;
  res jsonb;
  tid bigint;
BEGIN
  IF want !~ '^[a-z0-9][a-z0-9-]{0,63}$' THEN RETURN projects.no(); END IF;
  IF op = 'key_owner' OR op = 'key_remove' THEN
    m := projects.member_key(want, who);
    IF m IS NULL THEN RETURN projects.no(); END IF;
    IF op = 'key_owner' THEN RETURN projects.r(200, jsonb_build_object('pubkey', m)); END IF;
    DELETE FROM chat.credentials WHERE owner = m;
    RETURN projects.r(200, '{"removed":true}');
  END IF;
  IF op = 'next' THEN
    -- the turn next will take, and its asker's own credential
    SELECT * INTO tn FROM projects.turns WHERE slug = want AND status = 'queued' ORDER BY id LIMIT 1;
    IF tn.id IS NOT NULL AND tn.kind = 'ask' THEN
      SELECT * INTO cr FROM chat.credentials WHERE owner = tn.by_pubkey AND NOT expired;
    END IF;
    IF cr.owner IS NOT NULL THEN who := who || '{"cap_usd":"1000000"}'; END IF;
    res := projects.api_0019(op, want, body, who);
    tid := CASE WHEN jsonb_typeof(res->'body'->'turn') = 'object' THEN (res->'body'->'turn'->>'id')::bigint END;
    IF tid IS NULL THEN RETURN res; END IF;
    IF cr.owner IS NOT NULL AND tid = tn.id THEN
      UPDATE projects.turns SET credential = 'own' WHERE projects.turns.id = tn.id;
      RETURN jsonb_set(res, '{body}', (res->'body') || jsonb_build_object('cap', 0, 'spent', 0,
        'credential', jsonb_build_object('kind', cr.kind, 'owner', cr.owner, 'sealed', cr.sealed)));
    END IF;
    UPDATE projects.turns SET credential = 'openagents' WHERE projects.turns.id = tid AND kind = 'ask';
    RETURN res;
  END IF;
  IF op = 'end' THEN
    tid := CASE WHEN coalesce(body->>'turn', '') ~ '^[0-9]{1,18}$' THEN (body->>'turn')::bigint END;
    res := projects.api_0019(op, want, body, who);
    IF (res->>'status')::int = 200 AND coalesce(body->>'credential_expired', '') = 'true' THEN
      UPDATE chat.credentials SET expired = true
        WHERE owner = (SELECT by_pubkey FROM projects.turns WHERE projects.turns.id = tid AND slug = want AND credential = 'own');
    END IF;
    RETURN res;
  END IF;
  res := projects.api_0019(op, want, body, who);
  IF op = 'talk' AND who->>'format' = 'lines' AND (res->>'status')::int = 200 AND jsonb_typeof(res->'body') = 'string' THEN
    m := projects.member_key(want, who);
    SELECT * INTO cr FROM chat.credentials WHERE owner = m;
    res := jsonb_set(res, '{body}', to_jsonb((res->>'body') || 'K|' || coalesce(cr.kind, '') || '|' || coalesce(cr.last4, '') || '|'
                                             || CASE WHEN coalesce(cr.expired, false) THEN '1' ELSE '0' END || E'\n'));
  END IF;
  RETURN res;
END $$;
