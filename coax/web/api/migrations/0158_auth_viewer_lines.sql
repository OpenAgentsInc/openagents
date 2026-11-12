-- Who is logged in, as every page reads it (#13207): GET /api/auth/viewer and /api/auth/me move
-- from the old phone sign-in (signin.api 'me', private.c) to our own sessions (auth.c). The lines
-- are the same: K|npub, J|slug for each private project the key is in, a blank line, H|hex key,
-- M|member for each project member the key is, a blank line (auth.c adds A|1 for an admin).
-- The npub's bech32 moves out of the signin schema, which a later migration drops, and every
-- function that read the old sign-in's sessions reads our own (a session's key) instead.

-- bech32 (BIP-173) of a 32-byte hex key under hrp, for npub (as signin.bech32, 0019).
CREATE OR REPLACE FUNCTION auth.bech32(hrp text, hex text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
DECLARE
  cs constant text := 'qpzry9x8gf2tvdw0s3jn54khce6mua7l';
  g constant int[] := ARRAY[996825010, 642813549, 513874426, 1027748829, 705979059];
  b bytea := decode(hex, 'hex');
  v int[] := '{}';
  acc int := 0; bits int := 0; chk int := 1; top int; x int; out text := '';
BEGIN
  FOR i IN 0 .. length(b) - 1 LOOP
    acc := ((acc << 8) | get_byte(b, i)) & 4095; bits := bits + 8;
    WHILE bits >= 5 LOOP bits := bits - 5; v := v || ((acc >> bits) & 31); END LOOP;
  END LOOP;
  IF bits > 0 THEN v := v || ((acc << (5 - bits)) & 31); END IF;
  -- the checksum over the expanded hrp, the data and six zeros
  FOREACH x IN ARRAY (SELECT array_agg(ascii(substr(hrp, i, 1)) >> 5 ORDER BY i) FROM generate_series(1, length(hrp)) i) || 0
      || (SELECT array_agg(ascii(substr(hrp, i, 1)) & 31 ORDER BY i) FROM generate_series(1, length(hrp)) i) || v || ARRAY[0, 0, 0, 0, 0, 0] LOOP
    top := chk >> 25; chk := ((chk & 33554431) << 5) # x;
    FOR k IN 0 .. 4 LOOP IF (top >> k) & 1 = 1 THEN chk := chk # g[k + 1]; END IF; END LOOP;
  END LOOP;
  chk := chk # 1;
  FOREACH x IN ARRAY v LOOP out := out || substr(cs, x + 1, 1); END LOOP;
  FOR k IN 0 .. 5 LOOP out := out || substr(cs, ((chk >> (5 * (5 - k))) & 31) + 1, 1); END LOOP;
  RETURN hrp || '1' || out;
END $$;

-- The viewer's lines for a key (the session's account key, or a NIP-98 signer).
CREATE OR REPLACE FUNCTION auth.viewer_lines(pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN coalesce(pk, '') ~ '^[0-9a-f]{64}$' THEN
    'K|' || auth.bech32('npub', pk) || E'\n'
      || coalesce((SELECT string_agg('J|' || d.slug, E'\n' ORDER BY d.slug) FROM projects.docs d WHERE projects.member_of(d.slug, pk) IS NOT NULL), '') || E'\n'
      || 'H|' || pk || E'\n'
      || coalesce((SELECT string_agg(DISTINCT 'M|' || k.member, E'\n') FROM projects.member_keys k WHERE k.pubkey = pk), '') || E'\n'
  END
$$;

-- The admin page's flags: auth_v2 (the WorkOS and device groundwork) is gone with its routes.
CREATE OR REPLACE FUNCTION access.admin_lines()
 RETURNS text
 LANGUAGE sql
 STABLE
AS $function$
  SELECT string_agg(l, E'\n' ORDER BY o, k) || E'\n' FROM (
    SELECT 0 o, 0::bigint k, format('F|%s|%s', n, CASE WHEN access.flag(n) THEN 1 ELSE 0 END) l FROM unnest(ARRAY['realm_invites']) n
    UNION ALL
    SELECT 1, -i.id, format('I|%s|%s|%s|%s|%s|%s|%s', i.id, i.uses, i.used, i.expires_ms,
      CASE WHEN i.revoked_ms IS NOT NULL THEN 'revoked' WHEN i.used >= i.uses THEN 'used' WHEN i.expires_ms <= projects.now_ms() THEN 'expired' ELSE 'live' END,
      access.line(i.note), projects.chi(i.expires_ms)) FROM access.invites i
    UNION ALL
    SELECT 2, CASE WHEN r.state = 'waiting' THEN r.id ELSE 1000000000000 - r.id END,
      format('R|%s|%s|%s|%s|%s|%s|%s', r.id, auth.bech32('npub', r.pubkey), access.line(r.note), access.line(r.contact), r.state, r.at_ms, projects.chi(r.at_ms)) FROM access.requests r
    UNION ALL
    SELECT 3, -c.at_ms, format('C|%s|%s|%s|%s', access.line(c.who), c.how, c.at_ms, projects.chi(c.at_ms)) FROM access.creators c
  ) x
$function$;

CREATE OR REPLACE FUNCTION access.admin_list(roots text[])
 RETURNS text
 LANGUAGE sql
 STABLE
AS $function$
  SELECT coalesce((SELECT string_agg(format(E'A|%s|%s|root\n', auth.bech32('npub', r), r), '' ORDER BY o)
                   FROM unnest(roots) WITH ORDINALITY AS t(r, o) WHERE r ~ '^[0-9a-f]{64}$'), '')
      || coalesce((SELECT string_agg(format(E'A|%s|%s|db|%s|%s|%s\n', auth.bech32('npub', k.pubkey), k.pubkey,
                                            CASE WHEN k.added_by ~ '^[0-9a-f]{64}$' THEN auth.bech32('npub', k.added_by) ELSE access.line(k.added_by) END,
                                            k.added_ms, access.line(k.note)), '' ORDER BY k.added_ms, k.pubkey)
                   FROM access.admin_keys k WHERE NOT k.pubkey = ANY (roots)), '') $function$;

-- A batch from a logged-in page: analytics.c sends the session's key itself as site.
CREATE OR REPLACE FUNCTION analytics.ingest(batch jsonb, ctx jsonb)
 RETURNS integer
 LANGUAGE plpgsql
AS $function$
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
  IF ctx->>'site' ~ '^[0-9a-f]{64}$' THEN site_npub := ctx->>'site'; END IF;   -- a logged-in session's key (analytics.c)
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
END $function$;

-- Private projects: a logged-in session's key (private.c sends it as site) instead of the old
-- sign-in's cookie; the old page's LINK A PHONE (op link) is gone with it.
CREATE OR REPLACE FUNCTION projects.api_0019(op text, want text, body jsonb, who jsonb)
 RETURNS jsonb
 LANGUAGE plpgsql
AS $function$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  site text := CASE WHEN who->>'site' ~ '^[0-9a-f]{64}$' THEN who->>'site' END;
  now_ms bigint := projects.now_ms();
  m text; why text;
BEGIN
  IF want !~ '^[a-z0-9][a-z0-9-]{0,63}$' THEN RETURN projects.no(); END IF;
  IF op = 'claim' THEN
    IF pk IS NULL THEN RETURN projects.no(); END IF;
    SELECT i.pubkey INTO m FROM projects.invites i
      WHERE i.token_sha256 = projects.sha(coalesce(body->>'token', '')) AND i.slug = want AND i.used_ms IS NULL AND i.expires_ms > now_ms FOR UPDATE;
    IF m IS NULL OR projects.role(want, m) <> 'member' THEN RETURN projects.no(); END IF;
    IF pk <> m AND (projects.role(want, pk) <> '' OR coalesce(projects.member_of(want, pk), m) <> m) THEN   -- the invite stays unspent
      RETURN projects.r(400, '{"error":"that key already has a place in this project"}');
    END IF;
    UPDATE projects.invites SET used_ms = now_ms WHERE token_sha256 = projects.sha(body->>'token');
    why := projects.add_key(want, m, pk, 'invite');
    IF why IS NOT NULL THEN RETURN projects.r(400, jsonb_build_object('error', why)); END IF;
    RETURN projects.r(200, jsonb_build_object('ok', true, 'name', projects.name_of(want, m)));
  END IF;
  IF op = 'redeem' THEN RETURN projects.api_0013(op, want, body, who); END IF;
  -- a project cookie comes first; else a logged-in session's key (private.c) that is this project's
  IF pk IS NULL AND projects.session(want, nullif(who->>'session', '')) IS NULL AND site IS NOT NULL THEN
    pk := projects.member_of(want, site);
    IF pk IS NOT NULL THEN who := who || jsonb_build_object('pubkey', pk, 'admin', false); END IF;
  ELSIF pk IS NOT NULL AND projects.role(want, pk) = '' AND projects.member_of(want, pk) IS NOT NULL THEN
    pk := projects.member_of(want, pk);
    who := who || jsonb_build_object('pubkey', pk);
  END IF;
  IF op = 'logout' THEN
    IF nullif(who->>'session', '') IS NULL THEN RETURN projects.r(200, '{"ok":true}'); END IF;
  END IF;
  RETURN projects.api_0013(op, want, body, who);
END $function$;

CREATE OR REPLACE FUNCTION projects.member_key(want text, who jsonb)
 RETURNS text
 LANGUAGE plpgsql
 STABLE
AS $function$
DECLARE pk text := nullif(who->>'pubkey', '');
BEGIN
  IF pk IS NULL THEN pk := projects.session(want, nullif(who->>'session', '')); END IF;
  IF pk IS NULL AND who->>'site' ~ '^[0-9a-f]{64}$' THEN pk := projects.member_of(want, who->>'site'); END IF;
  IF pk IS NOT NULL AND projects.role(want, pk) = '' THEN pk := projects.member_of(want, pk); END IF;
  IF pk IS NULL OR projects.role(want, pk) <> 'member' THEN RETURN NULL; END IF;
  RETURN pk;
END $function$;

-- auth_v2 is no flag any more (its routes are gone): only realm_invites is.
CREATE OR REPLACE FUNCTION access.set_flag(body jsonb, by_ text)
 RETURNS jsonb
 LANGUAGE plpgsql
AS $function$
DECLARE n text := body->>'name';
BEGIN
  IF n IS NULL OR n <> 'realm_invites' OR jsonb_typeof(body->'on') <> 'boolean' THEN RETURN access.err(400, 'name and on'); END IF;
  INSERT INTO access.flags VALUES (n, (body->>'on')::boolean, by_, projects.now_ms())
    ON CONFLICT (name) DO UPDATE SET on_ = excluded.on_, set_by = excluded.set_by, set_ms = excluded.set_ms;
  RETURN access.r(200, jsonb_build_object('name', n, 'on', (body->>'on')::boolean), NULL);
END $function$;
DELETE FROM access.flags WHERE name = 'auth_v2';
