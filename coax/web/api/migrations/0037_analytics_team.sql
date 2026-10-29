-- Our own testing is not usage. The team's sessions (the owner's and Kiki's)
-- are kept, but marked auto = 'team', so every "auto IS NULL" visitor count,
-- /admin/analytics included, leaves them out the way it leaves out automation.
--
-- Who is team (analytics.team, written by the service on every start from
-- team.conf and admins.conf, never by hand):
--   npub          an admin's Nostr key (admins.conf): a batch signed with it,
--                 a session or account tied to it, or a site session
--                 (oa_session, /login) of it
--   player        a playtest player id from team.conf: a web player (p-...),
--                 or the old playtest's kiki-<16 hex> (its sessions are
--                 kiki-<16 hex>-..., its pid the 16 hex)
--   email_sha256  sha256 of a lower-cased email from team.conf: a playtest
--                 join with it
-- and a browser marked team: the oa_team cookie (Path=/, two years), set when
-- an admin logs in and on every batch of a team session, so its later visits
-- are team logged out too. Every device (the pid of a session-start or
-- page-view) seen in a team session is a team device (analytics.team_devices),
-- and every session that device has, before or after, is team.
--
-- analytics.ingest marks a batch at the source; analytics.team_tag() spreads
-- it (sessions -> devices -> sessions, to a fixed point) and backfills the
-- rows already kept. The service runs team_tag() on start, so changing
-- team.conf and deploying backfills too. Automation keeps its own mark: a
-- row whose auto is already set is never changed.

CREATE TABLE analytics.team (
  kind text NOT NULL CHECK (kind IN ('npub', 'player', 'email_sha256')),
  value text NOT NULL,
  PRIMARY KEY (kind, value)
);
CREATE TABLE analytics.team_devices (
  pid text PRIMARY KEY,
  ms bigint NOT NULL,
  sid text                   -- the team session it was first seen in
);
ALTER TABLE analytics.sessions ADD COLUMN team boolean NOT NULL DEFAULT false;
CREATE INDEX events_pid ON analytics.events ((p->>'pid')) WHERE ev IN ('session-start', 'page-view');
CREATE INDEX events_npub ON analytics.events (npub) WHERE npub IS NOT NULL;
CREATE INDEX playtesters_sid ON analytics.playtesters (sid);

CREATE FUNCTION analytics.email_sha256(email text) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT encode(sha256(convert_to(lower(btrim(coalesce(email, ''))), 'UTF8')), 'hex') $$;

-- Replaces the team with these rows ({"npub": [...], "player": [...],
-- "email_sha256": [...]}), the service's config; malformed values are dropped.
CREATE FUNCTION analytics.team_set(cfg jsonb) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  DELETE FROM analytics.team;
  INSERT INTO analytics.team (kind, value)
  SELECT DISTINCT k, lower(v) FROM (
    SELECT 'npub' AS k, jsonb_array_elements_text(coalesce(cfg->'npub', '[]')) AS v
    UNION ALL SELECT 'player', jsonb_array_elements_text(coalesce(cfg->'player', '[]'))
    UNION ALL SELECT 'email_sha256', jsonb_array_elements_text(coalesce(cfg->'email_sha256', '[]'))) x
  WHERE (k = 'npub' AND v ~* '^[0-9a-f]{64}$') OR (k = 'email_sha256' AND v ~* '^[0-9a-f]{64}$')
     OR (k = 'player' AND v ~ '^[a-z][a-z0-9-]{3,63}$');
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

-- Is a playtester row (its email and player, in its space) the team's?
CREATE FUNCTION analytics.team_playtester(email text, player text, space text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM analytics.team t WHERE
    (t.kind = 'email_sha256' AND t.value = analytics.email_sha256(email))
    OR (t.kind = 'player' AND player IS NOT NULL AND (t.value = player OR t.value = space || '-' || player)))
$$;

-- Spreads the team's mark and backfills it. Returns what it changed.
CREATE FUNCTION analytics.team_tag() RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  devices int := 0; d int; s int; rounds int := 0;
  sessions_n int; events_n int; playtesters_n int; answers_n int;
  now_ms bigint := floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint;
BEGIN
  CREATE TEMP TABLE IF NOT EXISTS team_sids (sid text PRIMARY KEY) ON COMMIT DROP;
  DELETE FROM team_sids;
  -- the seeds: sessions already team, an admin's key, a team player or email
  INSERT INTO team_sids SELECT sid FROM analytics.sessions WHERE team ON CONFLICT DO NOTHING;
  INSERT INTO team_sids SELECT x.sid FROM analytics.sessions x JOIN analytics.team t
    ON (t.kind = 'npub' AND t.value = x.npub) OR (t.kind = 'player' AND t.value = x.player) ON CONFLICT DO NOTHING;
  INSERT INTO team_sids SELECT DISTINCT e.sid FROM analytics.events e JOIN analytics.team t ON t.kind = 'npub' AND t.value = e.npub
    ON CONFLICT DO NOTHING;
  INSERT INTO team_sids SELECT DISTINCT p.sid FROM analytics.playtesters p
    WHERE p.sid IS NOT NULL AND p.sid <> '' AND analytics.team_playtester(p.email, p.player, p.space) ON CONFLICT DO NOTHING;
  -- the old playtest's sessions are kiki-<player hex>-<8 hex>
  INSERT INTO team_sids SELECT DISTINCT e.sid FROM analytics.events e JOIN analytics.team t
    ON t.kind = 'player' AND t.value ~ '^[a-z]+-[0-9a-f]{16}$' AND e.sid LIKE t.value || '-%' ON CONFLICT DO NOTHING;
  -- devices <-> sessions, to a fixed point
  LOOP
    rounds := rounds + 1;
    INSERT INTO analytics.team_devices (pid, ms, sid)
    SELECT DISTINCT ON (e.p->>'pid') e.p->>'pid', now_ms, e.sid FROM analytics.events e JOIN team_sids USING (sid)
    WHERE e.ev IN ('session-start', 'page-view') AND e.p->>'pid' ~ '^[A-Za-z0-9_-]{6,64}$'
      AND (e.auto IS NULL OR e.auto = 'team')   -- not from an automated run's own events
    ORDER BY e.p->>'pid', e.ms
    ON CONFLICT DO NOTHING;
    GET DIAGNOSTICS d = ROW_COUNT;
    devices := devices + d;
    INSERT INTO team_sids SELECT DISTINCT e.sid FROM analytics.events e JOIN analytics.team_devices v ON v.pid = e.p->>'pid'
    WHERE e.ev IN ('session-start', 'page-view') ON CONFLICT DO NOTHING;
    GET DIAGNOSTICS s = ROW_COUNT;
    EXIT WHEN (d = 0 AND s = 0) OR rounds >= 20;
  END LOOP;
  -- the backfill; automation keeps its own mark
  UPDATE analytics.sessions x SET team = true FROM team_sids t WHERE x.sid = t.sid AND NOT x.team;
  GET DIAGNOSTICS sessions_n = ROW_COUNT;
  UPDATE analytics.events e SET auto = 'team' FROM team_sids t WHERE e.sid = t.sid AND e.auto IS NULL;
  GET DIAGNOSTICS events_n = ROW_COUNT;
  UPDATE analytics.playtesters p SET auto = 'team'
  WHERE p.auto IS NULL AND (p.sid IN (SELECT sid FROM team_sids) OR analytics.team_playtester(p.email, p.player, p.space));
  GET DIAGNOSTICS playtesters_n = ROW_COUNT;
  UPDATE analytics.survey_answers a SET auto = 'team' FROM team_sids t WHERE a.sid = t.sid AND a.auto IS NULL;
  GET DIAGNOSTICS answers_n = ROW_COUNT;
  RETURN jsonb_build_object('sessions', sessions_n, 'events', events_n, 'devices', devices,
                            'playtesters', playtesters_n, 'answers', answers_n);
END $$;

-- A page's batch, as 0015 kept it, marked team at the source: the oa_team
-- cookie (ctx team = '1'), an admin's key (signed, the session's, the
-- player's account's, or the site session's: ctx site, the oa_session
-- token), a team player, a session already team or joined as the team, or
-- a pid the team's.
-- Automation keeps its mark. A session that becomes team spreads it
-- (team_tag) once.
CREATE OR REPLACE FUNCTION analytics.ingest(batch jsonb, ctx jsonb) RETURNS int LANGUAGE plpgsql AS $$
DECLARE
  s text := batch->>'sid';
  now_ms bigint := (ctx->>'ms')::bigint;
  auto text := coalesce(nullif(ctx->>'auto', ''), CASE WHEN batch->>'auto' ~ '^[a-z][a-z0-9-]{0,31}$' THEN batch->>'auto' END);
  agent text := coalesce(nullif(ctx->>'agent', ''), analytics.bot_agent(ctx->>'ua'));
  k text := ctx->>'key';
  pl verse.players;
  acct_npub text;
  sess_npub text;
  site_npub text;
  was_team boolean;
  team_ boolean;
  npub_ text := nullif(ctx->>'npub', '');
  have int;
  kept int;
BEGIN
  IF jsonb_typeof(batch) IS DISTINCT FROM 'object' OR s IS NULL OR s !~ '^[A-Za-z0-9_:-]{8,80}$'
     OR jsonb_typeof(batch->'events') IS DISTINCT FROM 'array' THEN
    RETURN -1;
  END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO pl FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
    SELECT a.npub INTO acct_npub FROM verse.accounts a WHERE a.id = pl.account;
  END IF;
  IF ctx->>'site' ~ '^[0-9a-f]{64}$' THEN site_npub := signin.session(ctx->>'site'); END IF;
  SELECT x.events, x.npub, x.team INTO have, sess_npub, was_team FROM analytics.sessions x WHERE x.sid = s;
  npub_ := coalesce(npub_, sess_npub, acct_npub);
  have := coalesce(have, 0);
  was_team := coalesce(was_team, false);
  team_ := auto IS NULL AND (was_team OR coalesce(ctx->>'team', '') = '1'
    OR EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'npub' AND t.value IN (npub_, acct_npub, site_npub))
    OR (pl.id IS NOT NULL AND EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'player' AND t.value = pl.id))
    OR EXISTS (SELECT 1 FROM analytics.playtesters j WHERE j.sid = s AND j.auto = 'team')   -- joined as the team before this batch
    OR EXISTS (SELECT 1 FROM jsonb_array_elements(batch->'events') e JOIN analytics.team_devices v ON v.pid = e->'p'->>'pid'
               WHERE jsonb_typeof(e) = 'object' AND jsonb_typeof(e->'p') = 'object'));
  team_ := coalesce(team_, false);
  IF team_ THEN auto := 'team'; END IF;
  WITH raw AS (
    SELECT e, ord FROM jsonb_array_elements(batch->'events') WITH ORDINALITY AS a(e, ord) WHERE ord <= 50
  ), clean AS (
    SELECT ord, e->>'ev' AS ev,
           CASE WHEN jsonb_typeof(e->'page') = 'string' THEN left(e->>'page', 32) ELSE '' END AS page,
           CASE WHEN jsonb_typeof(e->'ct') = 'number' THEN floor((e->>'ct')::numeric)::bigint END AS ct,
           analytics.clean_props(e->'p') AS p
    FROM raw WHERE jsonb_typeof(e) = 'object' AND e->>'ev' ~ '^[a-z][a-z0-9-]{0,31}$'
  ), capped AS (
    SELECT * FROM clean ORDER BY ord LIMIT greatest(0, 5000 - have)
  )
  INSERT INTO analytics.events (ms, sid, acct, npub, ev, page, ct, p, auto, space, host)
  SELECT now_ms, s, pl.account, npub_, ev, page, ct,
         CASE WHEN ev = 'session-start' AND agent IS NOT NULL THEN p || jsonb_build_object('agent', agent) ELSE p END,
         auto, 'web', nullif(ctx->>'host', '')
  FROM capped ORDER BY ord;
  GET DIAGNOSTICS kept = ROW_COUNT;
  INSERT INTO analytics.sessions AS x (sid, first_ms, last_ms, host, ip, ua, agent, region, subdivision, city, latlong, rtt_ms, tls, device, npub, events, player, team)
  VALUES (s, now_ms, now_ms, nullif(ctx->>'host', ''), nullif(ctx->>'ip', ''), left(nullif(ctx->>'ua', ''), 400), agent,
          nullif(ctx->>'region', ''), nullif(ctx->>'subdivision', ''), nullif(ctx->>'city', ''), nullif(ctx->>'latlong', ''),
          CASE WHEN ctx->>'rtt' ~ '^[0-9]{1,9}$' THEN (ctx->>'rtt')::int END, nullif(ctx->>'tls', ''), nullif(ctx->>'device', ''),
          npub_, kept, pl.id, team_)
  ON CONFLICT (sid) DO UPDATE SET last_ms = greatest(x.last_ms, EXCLUDED.last_ms), events = x.events + EXCLUDED.events,
    npub = coalesce(x.npub, EXCLUDED.npub), ip = coalesce(x.ip, EXCLUDED.ip), ua = coalesce(x.ua, EXCLUDED.ua),
    agent = coalesce(x.agent, EXCLUDED.agent), region = coalesce(x.region, EXCLUDED.region),
    subdivision = coalesce(x.subdivision, EXCLUDED.subdivision), city = coalesce(x.city, EXCLUDED.city),
    latlong = coalesce(x.latlong, EXCLUDED.latlong), rtt_ms = coalesce(x.rtt_ms, EXCLUDED.rtt_ms),
    tls = coalesce(x.tls, EXCLUDED.tls), device = coalesce(x.device, EXCLUDED.device),
    player = coalesce(x.player, EXCLUDED.player), team = x.team OR EXCLUDED.team;
  -- newly team, or a pid of this batch not yet known: spread it
  IF team_ AND (NOT was_team OR EXISTS (
       SELECT 1 FROM jsonb_array_elements(batch->'events') e
       WHERE jsonb_typeof(e) = 'object' AND e->>'ev' IN ('session-start', 'page-view') AND jsonb_typeof(e->'p') = 'object'
         AND e->'p'->>'pid' ~ '^[A-Za-z0-9_-]{6,64}$'
         AND NOT EXISTS (SELECT 1 FROM analytics.team_devices v WHERE v.pid = e->'p'->>'pid'))) THEN
    PERFORM analytics.team_tag();
  END IF;
  RETURN kept;
END $$;

-- Rows that come in another way (the demo import, import_demo.sql, and a
-- playtest join) are marked by the same rules once they are in.
CREATE FUNCTION analytics.team_after_events() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM new_rows n WHERE n.auto IS NULL AND (
       EXISTS (SELECT 1 FROM analytics.team t WHERE (t.kind = 'npub' AND t.value = n.npub)
                 OR (t.kind = 'player' AND t.value ~ '^[a-z]+-[0-9a-f]{16}$' AND n.sid LIKE t.value || '-%'))
       OR EXISTS (SELECT 1 FROM analytics.sessions x WHERE x.sid = n.sid AND x.team)
       OR EXISTS (SELECT 1 FROM analytics.playtesters j WHERE j.sid = n.sid AND j.auto = 'team')
       OR (n.ev IN ('session-start', 'page-view') AND EXISTS (SELECT 1 FROM analytics.team_devices v WHERE v.pid = n.p->>'pid')))) THEN
    PERFORM analytics.team_tag();
  END IF;
  RETURN NULL;
END $$;
CREATE TRIGGER team_after_events AFTER INSERT ON analytics.events
  REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION analytics.team_after_events();

CREATE FUNCTION analytics.team_after_playtesters() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF EXISTS (SELECT 1 FROM new_rows n WHERE n.auto IS NULL AND analytics.team_playtester(n.email, n.player, n.space)) THEN
    PERFORM analytics.team_tag();
  END IF;
  RETURN NULL;
END $$;
CREATE TRIGGER team_after_playtesters AFTER INSERT ON analytics.playtesters
  REFERENCING NEW TABLE AS new_rows FOR EACH STATEMENT EXECUTE FUNCTION analytics.team_after_playtesters();
