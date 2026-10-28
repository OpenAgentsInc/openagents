ALTER TABLE analytics.sessions ADD COLUMN player text;
CREATE INDEX sessions_player ON analytics.sessions (player);

ALTER FUNCTION verse.api(text, text[], jsonb, jsonb) RENAME TO api_0011;

CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  k text := who->>'key';
  auto text := nullif(who->>'auto', '');
  signer text := nullif(who->>'npub', '');
  b jsonb := CASE WHEN jsonb_typeof(body) = 'object' THEN body ELSE '{}' END;
  now_s double precision := extract(epoch FROM clock_timestamp());
  rec verse.players;
  acct text;
  own text;
  email text;
  out jsonb;
  sid_ text;
  planet_ text;
  worlds_n int := 0;
  saves_n int := 0;
  sessions_n int := 0;
BEGIN
  IF NOT (method = 'POST' AND parts IN (ARRAY['join'], ARRAY['signin'])) THEN RETURN verse.api_0011(method, parts, body, who); END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  IF signer IS NOT NULL THEN
    INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-' || signer, signer, verse.now_ms()) ON CONFLICT DO NOTHING;
  END IF;

  IF parts = ARRAY['join'] THEN
    email := lower(btrim(coalesce(b->>'email', '')));
    IF length(email) > 120 OR email !~ '^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$' THEN
      RETURN verse.r(400, '{"error":"that is not an email address"}');
    END IF;
    INSERT INTO analytics.playtesters (id, at, email, player, sid, how, auto, space)
    VALUES (verse.hex(16), now_s, email, left(b->>'player', 32), left(b->>'sid', 80), left(b->>'how', 16), auto, 'web');
    IF rec.id IS NOT NULL AND rec.account IS NOT NULL THEN
      RETURN verse.r(200, jsonb_build_object('joined', true, 'player', jsonb_build_object('key', k, 'id', rec.id, 'account', rec.account)));
    END IF;
    IF signer IS NOT NULL THEN
      acct := 'n-' || signer;
    ELSE
      acct := 'g-' || verse.hex(24);
      INSERT INTO verse.accounts (id, created_ms) VALUES (acct, verse.now_ms());
    END IF;
    RETURN verse.r(200, jsonb_build_object('joined', true, 'player', verse.link(k, rec, acct, auto)));
  END IF;

  IF signer IS NULL THEN RETURN verse.r(401, '{"error":"sign in with a NIP-98 signed request"}'); END IF;
  acct := 'n-' || signer;
  IF rec.id IS NULL THEN
    out := verse.mint(acct, auto);
  ELSIF rec.account IS NULL OR rec.account = acct OR rec.account LIKE 'g-%' THEN
    own := CASE WHEN rec.account LIKE 'g-%' THEN rec.account END;
    UPDATE verse.players SET account = acct WHERE id = rec.id OR (own IS NOT NULL AND account = own);
    UPDATE verse.worlds SET owner_account = acct
    WHERE (owner_player = rec.id AND (owner_account IS NULL OR owner_account = own)) OR (own IS NOT NULL AND owner_account = own);
    GET DIAGNOSTICS worlds_n = ROW_COUNT;
    IF NOT EXISTS (SELECT 1 FROM verse.awards WHERE user_id = acct) THEN
      SELECT a.planet INTO planet_ FROM verse.awards a JOIN verse.worlds w ON w.id = a.planet
      WHERE w.owner_account = acct ORDER BY a.at DESC LIMIT 1;
      IF planet_ IS NOT NULL THEN
        UPDATE verse.awards SET user_id = acct WHERE planet = planet_ AND user_id <> acct;
        UPDATE verse.planets SET owner_id = acct WHERE id = planet_;
      END IF;
    END IF;
    IF own IS NOT NULL THEN
      INSERT INTO verse.progress AS x (account, slot, cur, cur_at, prev, prev_at)
      SELECT acct, g.slot, g.cur, g.cur_at, g.prev, g.prev_at FROM verse.progress g WHERE g.account = own
      ON CONFLICT (account, slot) DO UPDATE SET
        cur = CASE WHEN EXCLUDED.cur_at > x.cur_at THEN EXCLUDED.cur ELSE x.cur END,
        cur_at = greatest(EXCLUDED.cur_at, x.cur_at),
        prev = CASE WHEN EXCLUDED.cur_at > x.cur_at THEN x.cur ELSE coalesce(x.prev, EXCLUDED.cur) END,
        prev_at = CASE WHEN EXCLUDED.cur_at > x.cur_at THEN x.cur_at ELSE coalesce(x.prev_at, EXCLUDED.cur_at) END;
      GET DIAGNOSTICS saves_n = ROW_COUNT;
      DELETE FROM verse.progress WHERE account = own;
    END IF;
    out := jsonb_build_object('key', k, 'id', rec.id, 'account', acct);
  ELSE
    out := verse.mint(acct, auto);
  END IF;

  sid_ := CASE WHEN b->>'sid' ~ '^[A-Za-z0-9_:-]{8,80}$' THEN b->>'sid' END;
  UPDATE analytics.sessions SET npub = signer, player = coalesce(player, out->>'id')
  WHERE npub IS NULL AND ((rec.id IS NOT NULL AND player = rec.id) OR (sid = sid_ AND (player IS NULL OR player = rec.id)));
  GET DIAGNOSTICS sessions_n = ROW_COUNT;
  RETURN verse.r(200, jsonb_build_object('signedIn', true, 'npub', signer, 'player', out,
    'moved', jsonb_build_object('worlds', worlds_n, 'saves', saves_n, 'sessions', sessions_n)));
END $$;

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
  SELECT x.events, x.npub INTO have, sess_npub FROM analytics.sessions x WHERE x.sid = s;
  npub_ := coalesce(npub_, sess_npub, acct_npub);
  have := coalesce(have, 0);
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
  INSERT INTO analytics.sessions AS x (sid, first_ms, last_ms, host, ip, ua, agent, region, subdivision, city, latlong, rtt_ms, tls, device, npub, events, player)
  VALUES (s, now_ms, now_ms, nullif(ctx->>'host', ''), nullif(ctx->>'ip', ''), left(nullif(ctx->>'ua', ''), 400), agent,
          nullif(ctx->>'region', ''), nullif(ctx->>'subdivision', ''), nullif(ctx->>'city', ''), nullif(ctx->>'latlong', ''),
          CASE WHEN ctx->>'rtt' ~ '^[0-9]{1,9}$' THEN (ctx->>'rtt')::int END, nullif(ctx->>'tls', ''), nullif(ctx->>'device', ''),
          npub_, kept, pl.id)
  ON CONFLICT (sid) DO UPDATE SET last_ms = greatest(x.last_ms, EXCLUDED.last_ms), events = x.events + EXCLUDED.events,
    npub = coalesce(x.npub, EXCLUDED.npub), ip = coalesce(x.ip, EXCLUDED.ip), ua = coalesce(x.ua, EXCLUDED.ua),
    agent = coalesce(x.agent, EXCLUDED.agent), region = coalesce(x.region, EXCLUDED.region),
    subdivision = coalesce(x.subdivision, EXCLUDED.subdivision), city = coalesce(x.city, EXCLUDED.city),
    latlong = coalesce(x.latlong, EXCLUDED.latlong), rtt_ms = coalesce(x.rtt_ms, EXCLUDED.rtt_ms),
    tls = coalesce(x.tls, EXCLUDED.tls), device = coalesce(x.device, EXCLUDED.device),
    player = coalesce(x.player, EXCLUDED.player);
  RETURN kept;
END $$;
