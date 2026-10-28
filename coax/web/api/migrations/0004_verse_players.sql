-- Players, guests, saves and published worlds for the Verse playtest
-- (pt-player-endpoints; player.c answers /api/playtest/*). The rules are the
-- demo's (aethyr-cpp tools/serve.py and tools/players.py at
-- demo/kiki-2026-10-28d), kept in SQL so every route reads one rule:
--
-- A player key is a random secret minted for one browser; the page sends it
-- as X-Verse-Player and only its SHA-256 is kept. A guest is a player key
-- and nothing more. Joining by email (never verified, a format check only)
-- links the key to the email's account; a NIP-98 signed request links it to
-- that Nostr key's account. A key linked to one account never moves to
-- another: the join answers with a fresh key for the new account.
--
-- A world made with POST /api/playtest/worlds belongs to the player (and
-- their account); a built-in world (haunted-house) has no owner row and only
-- the site (an admin's NIP-98 signature) owns it. The owner publishes; on a
-- world its owner opened, anyone the server knows may. Progress saves are
-- per account, so only a player with an email (or a Nostr key) keeps them on
-- the server; each slot keeps the current save and the last good one.
CREATE SCHEMA IF NOT EXISTS verse;

CREATE TABLE verse.accounts (
  id text PRIMARY KEY,           -- e-<hash of the email> or n-<pubkey hex>
  email text UNIQUE,             -- never shown on a public route
  npub text UNIQUE,              -- a Nostr pubkey (hex) that signed a request
  name text,
  created_ms bigint NOT NULL
);
CREATE TABLE verse.players (
  id text PRIMARY KEY,           -- p-<16 hex>: the public id
  key_sha256 text UNIQUE NOT NULL,
  account text REFERENCES verse.accounts (id),
  created_ms bigint NOT NULL,
  auto text                      -- the automated tool that minted it
);
CREATE TABLE verse.worlds (      -- worlds with an owner; built-in ones have no row
  id text PRIMARY KEY,
  owner_player text REFERENCES verse.players (id),
  owner_account text REFERENCES verse.accounts (id),
  created_ms bigint NOT NULL
);
CREATE TABLE verse.world_state ( -- the owner's choices, built-in worlds too; a reset keeps them
  world text PRIMARY KEY,
  open_ms bigint,                -- opened to direct publishing then, or null
  settings jsonb NOT NULL DEFAULT '{}',
  settings_ms bigint,
  settings_by text
);
CREATE TABLE verse.published (   -- what visitors of a world see now
  world text PRIMARY KEY,
  doc jsonb NOT NULL,
  version int NOT NULL,
  published_by text NOT NULL,
  published_at double precision NOT NULL   -- seconds, as the demo answers it
);
CREATE TABLE verse.log (         -- append-only: every publish (with its document) and reset
  id bigserial PRIMARY KEY,
  world text NOT NULL,
  version int NOT NULL,
  who text NOT NULL,
  summary text NOT NULL DEFAULT '',
  at double precision NOT NULL,
  state text NOT NULL,           -- published, reset
  by_account text, by_player text,
  doc jsonb
);
CREATE INDEX log_world ON verse.log (world, id);
CREATE TABLE verse.progress (
  account text NOT NULL REFERENCES verse.accounts (id),
  slot text NOT NULL,            -- <world>/<name>
  cur bytea, cur_at double precision,
  prev bytea, prev_at double precision,
  PRIMARY KEY (account, slot)
);

CREATE TABLE verse.nostr_seen (  -- NIP-98 events already used: each signs one request
  id text PRIMARY KEY,
  ms bigint NOT NULL
);

CREATE FUNCTION verse.now_ms() RETURNS bigint LANGUAGE sql VOLATILE AS
  $$ SELECT floor(extract(epoch FROM clock_timestamp()) * 1000)::bigint $$;
CREATE FUNCTION verse.hex(n int) RETURNS text LANGUAGE sql VOLATILE AS
  $$ SELECT left(replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', ''), n) $$;
CREATE FUNCTION verse.b64(b bytea) RETURNS text LANGUAGE sql IMMUTABLE AS
  $$ SELECT translate(encode(b, 'base64'), E'\n', '') $$;
CREATE FUNCTION verse.r(status int, body jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS
  $$ SELECT jsonb_build_object('status', status, 'body', body) $$;

-- A new player key, linked to an account or not.
CREATE FUNCTION verse.mint(acct text, auto text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  k text := replace(gen_random_uuid()::text || gen_random_uuid()::text, '-', '');
  pid text := 'p-' || verse.hex(16);
BEGIN
  INSERT INTO verse.players (id, key_sha256, account, created_ms, auto)
  VALUES (pid, encode(sha256(convert_to(k, 'UTF8')), 'hex'), acct, verse.now_ms(), nullif(auto, ''));
  RETURN jsonb_build_object('key', k, 'id', pid, 'account', acct);
END $$;

-- The key the page should keep after it is tied to acct: the same key, now
-- linked, or a fresh one when it is unknown or belongs to another account.
CREATE FUNCTION verse.link(k text, rec verse.players, acct text, auto text) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  IF rec.id IS NOT NULL AND (rec.account IS NULL OR rec.account = acct) THEN
    UPDATE verse.players SET account = acct WHERE id = rec.id;
    RETURN jsonb_build_object('key', k, 'id', rec.id, 'account', acct);
  END IF;
  RETURN verse.mint(acct, auto);
END $$;

-- True when this player or account owns the world (tools/players.py owns()).
CREATE FUNCTION verse.owns(rec verse.players, acct text, world text) RETURNS boolean LANGUAGE plpgsql STABLE AS $$
DECLARE
  w verse.worlds;
  theirs text;
BEGIN
  SELECT * INTO w FROM verse.worlds WHERE id = world;
  IF w.id IS NULL THEN RETURN false; END IF;
  IF rec.id IS NOT NULL AND w.owner_player = rec.id THEN RETURN true; END IF;
  SELECT account INTO theirs FROM verse.players WHERE id = w.owner_player;
  RETURN EXISTS (SELECT 1 FROM unnest(ARRAY[acct, rec.account]) AS m(a)
                 WHERE a IS NOT NULL AND a IN (w.owner_account, theirs));
END $$;

-- One request. parts: the path after /api/playtest/. who, from player.c:
--   key    the X-Verse-Player header        npub   a verified NIP-98 pubkey
--   site   an admin signed it               auto   the automation mark
--   good   the progress save's container and CRC check out (POST progress)
--   bytes  that save, base64 (as sent)
CREATE FUNCTION verse.api(method text, parts text[], body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
#variable_conflict use_column
DECLARE
  safe text := '^[A-Za-z0-9_-]{1,64}$';
  k text := who->>'key';
  auto text := nullif(who->>'auto', '');
  site boolean := coalesce((who->>'site')::boolean, false);
  npub text := nullif(who->>'npub', '');
  now_s double precision := extract(epoch FROM clock_timestamp());
  n int := coalesce(array_length(parts, 1), 0);
  rec verse.players;
  acct text;
  acct_name text;
  world text := CASE WHEN n >= 2 THEN parts[2] END;
  mine boolean;
  pub verse.published;
  st verse.world_state;
  w verse.worlds;
  who_name text;
  given text;
  email text;
  out jsonb;
  ver int;
  p verse.progress;
  slot_ text;
  at double precision;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  IF k ~ '^[A-Za-z0-9_-]{20,128}$' THEN
    SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  END IF;
  -- A Nostr key that signed: its account, and the sent key linked to it when free.
  IF npub IS NOT NULL THEN
    INSERT INTO verse.accounts (id, npub, created_ms) VALUES ('n-' || npub, npub, verse.now_ms()) ON CONFLICT DO NOTHING;
    acct := 'n-' || npub;
    IF rec.id IS NOT NULL AND rec.account IS NULL THEN
      UPDATE verse.players SET account = acct WHERE id = rec.id;
      rec.account := acct;
    END IF;
  ELSE
    acct := rec.account;
  END IF;
  SELECT name INTO acct_name FROM verse.accounts WHERE id = acct;
  -- The name an edit log shows: the account's, else what a guest typed (a label only).
  given := CASE WHEN jsonb_typeof(body->'user') = 'object' THEN nullif(body->'user'->>'name', '') END;
  who_name := left(coalesce(acct_name, given, 'owner'), 40);
  IF world IS NOT NULL AND parts[1] IN ('world', 'log') AND world !~ safe THEN
    RETURN verse.r(404, '{"error":"unknown"}');
  END IF;
  mine := world IS NOT NULL AND (site OR verse.owns(rec, acct, world));

  -- ---- players and guests
  IF parts = ARRAY['player'] AND method = 'GET' THEN
    IF rec.id IS NULL THEN RETURN verse.r(404, '{"error":"unknown player key"}'); END IF;
    RETURN verse.r(200, jsonb_build_object('id', rec.id, 'account', rec.account,
      'worlds', coalesce((SELECT jsonb_agg(x.id ORDER BY x.id) FROM verse.worlds x WHERE verse.owns(rec, acct, x.id)), '[]')));
  END IF;
  IF parts = ARRAY['player'] AND method = 'POST' THEN
    IF rec.id IS NOT NULL THEN RETURN verse.r(200, jsonb_build_object('key', k, 'id', rec.id, 'account', rec.account)); END IF;
    RETURN verse.r(200, verse.mint(acct, auto));
  END IF;
  IF parts = ARRAY['join'] AND method = 'POST' THEN
    email := lower(btrim(coalesce(body->>'email', '')));
    IF length(email) > 120 OR email !~ '^[^\s@]+@[^\s@.]+(\.[^\s@.]+)+$' THEN
      RETURN verse.r(400, '{"error":"that is not an email address"}');
    END IF;
    INSERT INTO analytics.playtesters (id, at, email, player, sid, how, auto, space)
    VALUES (verse.hex(16), now_s, email, left(body->>'player', 32), left(body->>'sid', 80), left(body->>'how', 16), auto, 'web');
    INSERT INTO verse.accounts (id, email, created_ms)
    VALUES ('e-' || left(encode(sha256(convert_to(email, 'UTF8')), 'hex'), 24), email, verse.now_ms()) ON CONFLICT DO NOTHING;
    acct := 'e-' || left(encode(sha256(convert_to(email, 'UTF8')), 'hex'), 24);
    RETURN verse.r(200, jsonb_build_object('joined', true, 'player', verse.link(k, rec, acct, auto)));
  END IF;
  IF parts = ARRAY['worlds'] AND method = 'POST' THEN
    IF rec.id IS NULL AND acct IS NULL THEN
      RETURN verse.r(403, '{"error":"send a player key (POST /api/playtest/player) to make a world"}');
    END IF;
    world := 'w-' || verse.hex(12);
    INSERT INTO verse.worlds (id, owner_player, owner_account, created_ms) VALUES (world, rec.id, coalesce(acct, rec.account), verse.now_ms());
    RETURN verse.r(200, jsonb_build_object('id', world));
  END IF;

  -- ---- published worlds
  IF parts[1] = 'world' AND n = 2 AND method = 'GET' THEN
    SELECT * INTO pub FROM verse.published WHERE published.world = parts[2];
    IF pub.world IS NULL THEN RETURN verse.r(200, '{"doc":null,"version":0}'); END IF;
    RETURN verse.r(200, jsonb_build_object('doc', pub.doc, 'version', pub.version, 'publishedBy', pub.published_by, 'publishedAt', pub.published_at));
  END IF;
  IF parts[1] = 'world' AND n = 2 AND method = 'POST' THEN
    SELECT * INTO st FROM verse.world_state WHERE world_state.world = parts[2];
    IF NOT mine AND NOT ((rec.id IS NOT NULL OR acct IS NOT NULL) AND st.open_ms IS NOT NULL) THEN
      IF rec.id IS NULL AND acct IS NULL THEN RETURN verse.r(403, '{"error":"log in to publish","offer":true}'); END IF;
      RETURN verse.r(403, '{"error":"only its owner can publish this world","offer":true}');
    END IF;
    IF jsonb_typeof(body->'doc') IS DISTINCT FROM 'object' THEN RETURN verse.r(400, '{"error":"publish needs the world as {\"doc\"}"}'); END IF;
    PERFORM pg_advisory_xact_lock(7731004, hashtext(world));
    SELECT coalesce(max(version), 0) + 1 INTO ver FROM verse.published WHERE published.world = parts[2];
    INSERT INTO verse.published (world, doc, version, published_by, published_at) VALUES (world, body->'doc', ver, who_name, now_s)
    ON CONFLICT (world) DO UPDATE SET doc = EXCLUDED.doc, version = EXCLUDED.version, published_by = EXCLUDED.published_by, published_at = EXCLUDED.published_at;
    INSERT INTO verse.log (world, version, who, summary, at, state, by_account, by_player, doc)
    VALUES (world, ver, who_name, left(coalesce(body->>'summary', ''), 300), now_s, 'published', acct, rec.id, body->'doc');
    RETURN verse.r(200, jsonb_build_object('version', ver));
  END IF;
  IF parts[1] = 'world' AND n = 3 AND parts[3] = 'owner' AND method = 'GET' THEN
    SELECT * INTO w FROM verse.worlds WHERE id = world;
    SELECT * INTO st FROM verse.world_state WHERE world_state.world = parts[2];
    RETURN verse.r(200, jsonb_build_object('claimed', w.id IS NOT NULL, 'you', mine, 'site', site, 'open', st.open_ms IS NOT NULL,
      'name', left(coalesce((SELECT a.name FROM verse.accounts a WHERE a.id = coalesce(w.owner_account,
        (SELECT pl.account FROM verse.players pl WHERE pl.id = w.owner_player))), ''), 40)));
  END IF;
  IF parts[1] = 'world' AND n = 3 AND parts[3] = 'settings' AND method = 'GET' THEN
    SELECT * INTO st FROM verse.world_state WHERE world_state.world = parts[2];
    IF st.settings_ms IS NULL THEN RETURN verse.r(200, '{"settings":{}}'); END IF;
    RETURN verse.r(200, jsonb_build_object('settings', st.settings, 'at', st.settings_ms / 1000.0, 'by', st.settings_by));
  END IF;
  IF parts[1] = 'world' AND n = 3 AND parts[3] = 'settings' AND method = 'POST' THEN
    IF NOT mine THEN RETURN verse.r(403, '{"error":"only the world''s owner changes its settings"}'); END IF;
    IF jsonb_typeof(body->'view') IS DISTINCT FROM 'object' OR NOT (body->'view'->>'view' = ANY (ARRAY['third', 'first', 'top']))
       OR jsonb_typeof(coalesce(body->'view'->'allowToggle', 'false')) <> 'boolean' THEN
      RETURN verse.r(400, '{"error":"expected {\"view\": {\"view\": \"third\"|\"first\"|\"top\", \"allowToggle\": bool}}"}');
    END IF;
    INSERT INTO verse.world_state AS x (world, settings, settings_ms, settings_by)
    VALUES (world, jsonb_build_object('view', jsonb_build_object('view', body->'view'->>'view',
            'allowToggle', coalesce((body->'view'->>'allowToggle')::boolean, false))), verse.now_ms(), who_name)
    ON CONFLICT (world) DO UPDATE SET settings = x.settings || EXCLUDED.settings, settings_ms = EXCLUDED.settings_ms, settings_by = EXCLUDED.settings_by
    RETURNING * INTO st;
    RETURN verse.r(200, jsonb_build_object('settings', st.settings, 'at', st.settings_ms / 1000.0, 'by', st.settings_by));
  END IF;
  IF parts[1] = 'world' AND n = 3 AND parts[3] = 'open' AND method = 'POST' THEN
    IF NOT mine THEN RETURN verse.r(403, '{"error":"only the owner can open a world to publishing"}'); END IF;
    INSERT INTO verse.world_state AS x (world, open_ms)
    VALUES (world, CASE WHEN body->'open' = 'true'::jsonb THEN verse.now_ms() END)
    ON CONFLICT (world) DO UPDATE SET open_ms = EXCLUDED.open_ms;
    RETURN verse.r(200, jsonb_build_object('open', body->'open' = 'true'::jsonb));
  END IF;
  IF parts[1] = 'world' AND n = 3 AND parts[3] = 'reset' AND method = 'POST' THEN
    IF NOT mine THEN RETURN verse.r(403, '{"error":"only the owner can reset a world"}'); END IF;
    DELETE FROM verse.published WHERE published.world = parts[2] RETURNING * INTO pub;
    INSERT INTO verse.log (world, version, who, summary, at, state, by_account, by_player)
    VALUES (world, 0, who_name, 'Reset to the built-in world' || coalesce(' (was version ' || pub.version || ')', ''), now_s, 'reset', acct, rec.id);
    RETURN verse.r(200, jsonb_build_object('reset', true, 'was', coalesce(pub.version, 0)));
  END IF;
  IF parts[1] = 'log' AND n = 2 AND method = 'GET' THEN
    RETURN verse.r(200, coalesce((SELECT jsonb_agg(e ORDER BY id DESC) FROM (
      SELECT l.id, jsonb_build_object('version', l.version, 'who', l.who, 'summary', l.summary, 'at', l.at, 'amount', 0, 'unit', 'sats',
             'state', l.state) || CASE WHEN l.by_account IS NOT NULL OR l.by_player IS NOT NULL
             THEN jsonb_build_object('by', jsonb_build_object('account', l.by_account, 'player', l.by_player)) ELSE '{}' END AS e
      FROM verse.log l WHERE l.world = parts[2] ORDER BY l.id DESC LIMIT 200) q), '[]'));
  END IF;

  -- ---- progress saves (an account's)
  IF parts[1] = 'progress' AND n IN (3, 4) THEN
    IF parts[2] !~ safe OR parts[3] !~ safe OR (n = 4 AND (parts[4] <> 'erase' OR method <> 'POST')) THEN
      RETURN verse.r(404, '{"error":"unknown"}');
    END IF;
    IF acct IS NULL THEN RETURN verse.r(403, '{"error":"join with your email to keep your progress on the server"}'); END IF;
    slot_ := parts[2] || '/' || parts[3];
    IF n = 4 THEN
      DELETE FROM verse.progress WHERE account = acct AND progress.slot = slot_;
      RETURN verse.r(200, '{"erased":true}');
    END IF;
    IF method = 'POST' THEN
      IF NOT coalesce((who->>'good')::boolean, false) THEN RETURN verse.r(400, '{"error":"that is not a good progress save"}'); END IF;
      at := CASE WHEN jsonb_typeof(body->'at') = 'number' THEN (body->>'at')::double precision ELSE now_s * 1000 END;
      SELECT * INTO p FROM verse.progress WHERE account = acct AND progress.slot = slot_ FOR UPDATE;
      IF p.cur IS NOT NULL AND p.cur_at > at THEN
        RETURN verse.r(409, jsonb_build_object('current', jsonb_build_object('bytes', verse.b64(p.cur), 'at', p.cur_at),
          'previous', CASE WHEN p.prev IS NOT NULL THEN jsonb_build_object('bytes', verse.b64(p.prev), 'at', p.prev_at) END));
      END IF;
      INSERT INTO verse.progress AS x (account, slot, cur, cur_at) VALUES (acct, slot_, decode(who->>'bytes', 'base64'), at)
      ON CONFLICT (account, slot) DO UPDATE SET prev = x.cur, prev_at = x.cur_at, cur = EXCLUDED.cur, cur_at = EXCLUDED.cur_at;
    END IF;
    SELECT * INTO p FROM verse.progress WHERE account = acct AND progress.slot = slot_;
    RETURN verse.r(200, jsonb_build_object(
      'current', CASE WHEN p.cur IS NOT NULL THEN jsonb_build_object('bytes', verse.b64(p.cur), 'at', p.cur_at) END,
      'previous', CASE WHEN p.prev IS NOT NULL THEN jsonb_build_object('bytes', verse.b64(p.prev), 'at', p.prev_at) END));
  END IF;
  RETURN verse.r(404, '{"error":"unknown"}');
END $$;
