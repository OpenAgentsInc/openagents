-- The CLI as a caller (docs/cli/2026-11-01-openagents-cli-v1.md, "What the server needs"):
-- a NIP-98-signed request to /api/playtest/realms acts as its key's account, and lists its realms.
--
-- Additive: one new table, new functions. No existing table, column, row or function changes.
--
-- Every realm rule takes a player key (verse.builder_player, verse.owns, verse.edit_role, the
-- builder's daily cap). A signed caller has no browser key, so its account gets one player of its
-- own, whose key is derived here from a secret only the database holds: auth.caller_key(pubkey)
-- answers it to realms.c for the one request and it is never stored, logged or sent anywhere. The
-- rules then see an account where they used to see a page's key, unchanged: the player's account is
-- auth.account_of(pubkey), so a CLI key linked to an account (auth.identities) owns what that account
-- owns, and a key no account linked is its own account, n-<pubkey>, as on the web.
CREATE TABLE auth.caller_secret (
  one boolean PRIMARY KEY DEFAULT true CHECK (one),
  secret text NOT NULL
);
INSERT INTO auth.caller_secret (secret) VALUES (projects.token() || projects.token());

-- The player key the signer pk's account acts with, made on first use. NULL for anything not a pubkey.
CREATE FUNCTION auth.caller_key(pk text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE
  acct text := auth.account_of(pk);
  k text;
BEGIN
  IF acct IS NULL THEN RETURN NULL; END IF;
  k := 'c' || projects.sha((SELECT secret FROM auth.caller_secret) || ':' || acct);
  IF NOT EXISTS (SELECT 1 FROM verse.players WHERE key_sha256 = projects.sha(k)) THEN
    INSERT INTO verse.accounts (id, npub, created_ms)
      VALUES (acct, CASE WHEN acct = 'n-' || pk THEN pk END, verse.now_ms()) ON CONFLICT DO NOTHING;
    INSERT INTO verse.players (id, key_sha256, account, created_ms)
      VALUES ('p-' || left(projects.sha('caller:' || acct), 16), projects.sha(k), acct, verse.now_ms()) ON CONFLICT DO NOTHING;
  END IF;
  RETURN k;
END $$;

-- GET /api/playtest/realms/mine: the made realms the key's player owns (verse.owns, as every route
-- decides it), newest first. {lines, body: {realms: [{slug, name, rev, published, listed}]}};
-- published is the revision visitors see, 0 when never published.
CREATE FUNCTION verse.realms_mine(k text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE
  rec verse.players := verse.builder_player(k);
  out jsonb;
BEGIN
  SELECT jsonb_build_object(
      'lines', coalesce(string_agg(format(E'realm %s|%s|%s|%s|%s\n', x.slug, x.rev, x.published, CASE WHEN x.listed THEN 1 ELSE 0 END, access.line(x.name)), '' ORDER BY x.made_ms DESC, x.slug), ''),
      'body', jsonb_build_object('realms', coalesce(jsonb_agg(jsonb_build_object('slug', x.slug, 'name', x.name, 'rev', x.rev, 'published', x.published, 'listed', x.listed)
                                                             ORDER BY x.made_ms DESC, x.slug), '[]')))
    INTO out
    FROM (SELECT m.slug, m.name, m.rev, coalesce(m.published_rev, 0) AS published, NOT m.unlisted AS listed, m.made_ms
          FROM verse.made_realms m WHERE rec.id IS NOT NULL AND verse.owns(rec, NULL, m.world)) x;
  RETURN out;
END $$;

-- ---- Device login for the CLI (in 0098 too: the realm work takes 0099 and up) (docs/cli/2026-11-01-openagents-cli-v1.md, "Log in"): a new key on a
-- computer asks to join an account, a person logged in on the web approves it at /login/device,
-- and the key is that account's from then on (auth.identities, as POST /api/auth/v2/link/nostr
-- links a key). Routes: access.c, /api/auth/v2/device/* and /api/auth/v2/unlink.
--
-- Additive: one new table, one new nullable column (auth.identities.label), new functions; auth.me
-- (0090) is replaced to print a label after an identity that has one, the lines before unchanged.
--
-- The code a person approves is 128 bits, kept only as its SHA-256, good for ten minutes and
-- approved once. The request's id is how the key that started it asks after it; only that key's
-- signature reads the answer.
ALTER TABLE auth.identities ADD COLUMN label text;   -- "cli:MACHINE" for a key a device login linked

CREATE TABLE auth.device_logins (
  id text PRIMARY KEY,               -- 16 hex: the poll handle, read only with the starting key's signature
  code_sha256 text NOT NULL UNIQUE,
  pubkey text NOT NULL,              -- the key asking to join
  machine text NOT NULL,
  made_ms bigint NOT NULL,
  expires_ms bigint NOT NULL,
  account text,                      -- approved: the account it joined
  approved_ms bigint,
  approved_by text                   -- the approving session's pubkey
);
CREATE INDEX device_logins_key ON auth.device_logins (pubkey, made_ms);

-- A machine name as it is shown and kept: printable, at most 64 characters, never empty.
CREATE FUNCTION auth.device_machine(m text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT nullif(left(btrim(access.line(m)), 64), '')
$$;

-- POST /api/auth/v2/device/start {"machine": NAME}, signed by the new key: {id, code, expires_ms}.
-- The code is in this answer only. A key has at most five open requests.
CREATE FUNCTION auth.device_start(pk text, body jsonb, ttl_ms bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  m text := auth.device_machine(body->>'machine');
  code text := left(projects.sha(projects.token()), 32);
  id_ text := left(projects.sha(projects.token()), 16);
  now_ bigint := projects.now_ms();
BEGIN
  IF coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN RETURN access.err(401, 'sign it with the new key'); END IF;
  IF jsonb_typeof(body->'machine') IS DISTINCT FROM 'string' OR m IS NULL THEN RETURN access.err(400, 'name the machine'); END IF;
  IF (SELECT count(*) FROM auth.device_logins d WHERE d.pubkey = pk AND d.account IS NULL AND d.expires_ms > now_) >= 5 THEN
    RETURN access.err(429, 'five requests are open: approve one or wait ten minutes');
  END IF;
  INSERT INTO auth.device_logins (id, code_sha256, pubkey, machine, made_ms, expires_ms)
    VALUES (id_, projects.sha(code), pk, m, now_, now_ + ttl_ms);
  RETURN access.r(200, jsonb_build_object('id', id_, 'code', code, 'expires_ms', now_ + ttl_ms),
                  format(E'id %s\ncode %s\nexpires_ms %s\n', id_, code, now_ + ttl_ms));
END $$;

-- GET /api/auth/v2/device/<id>, signed by the key that started it: waiting, approved (and the
-- account) or expired. Anyone else's signature: 404, as if there were none.
CREATE FUNCTION auth.device_poll(pk text, id_ text) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE d auth.device_logins;
BEGIN
  SELECT * INTO d FROM auth.device_logins x WHERE x.id = id_ AND x.pubkey = pk;
  IF d.id IS NULL THEN RETURN access.err(404, 'no such request'); END IF;
  IF d.account IS NOT NULL THEN
    RETURN access.r(200, jsonb_build_object('state', 'approved', 'account', d.account), format(E'state approved\naccount %s\n', d.account));
  END IF;
  IF d.expires_ms <= projects.now_ms() THEN RETURN access.r(200, jsonb_build_object('state', 'expired'), E'state expired\n'); END IF;
  RETURN access.r(200, jsonb_build_object('state', 'waiting'), E'state waiting\n');
END $$;

-- POST /api/auth/v2/device/look {"code"}: what /login/device shows before APPROVE. {machine, state}.
CREATE FUNCTION auth.device_look(body jsonb) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE d auth.device_logins;
BEGIN
  SELECT * INTO d FROM auth.device_logins x WHERE x.code_sha256 = projects.sha(coalesce(body->>'code', ''));
  IF d.id IS NULL THEN RETURN access.err(404, 'that code isn''t good'); END IF;
  RETURN access.r(200, jsonb_build_object('machine', d.machine,
      'state', CASE WHEN d.account IS NOT NULL THEN 'approved' WHEN d.expires_ms <= projects.now_ms() THEN 'expired' ELSE 'waiting' END), NULL);
END $$;

-- POST /api/auth/v2/device/approve {"code"}, logged in: the device's key joins the session's
-- account, labelled cli:MACHINE. Once; ten minutes. A key that is another account's, or owns
-- realms, players or creator rows of its own, is refused (merging accounts is not built); the
-- player auth.caller_key made for a key's own account (0098) owns nothing and doesn't count.
CREATE FUNCTION auth.device_approve(session_pk text, body jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  acct text := auth.account_of(session_pk);
  d auth.device_logins;
  now_ bigint := projects.now_ms();
BEGIN
  IF acct IS NULL THEN RETURN access.err(401, 'log in first'); END IF;
  SELECT * INTO d FROM auth.device_logins x WHERE x.code_sha256 = projects.sha(coalesce(body->>'code', '')) FOR UPDATE;
  IF d.id IS NULL THEN RETURN access.err(404, 'that code isn''t good'); END IF;
  IF d.account IS NOT NULL THEN RETURN access.err(409, 'that computer was approved already'); END IF;
  IF d.expires_ms <= now_ THEN RETURN access.err(410, 'that code expired: run openagents login again'); END IF;
  IF auth.account_of(d.pubkey) <> acct AND (
       EXISTS (SELECT 1 FROM auth.identities WHERE kind = 'nostr' AND subject = d.pubkey)
       OR EXISTS (SELECT 1 FROM verse.players WHERE account = 'n-' || d.pubkey AND id <> 'p-' || left(projects.sha('caller:n-' || d.pubkey), 16))
       OR EXISTS (SELECT 1 FROM verse.worlds WHERE owner_account = 'n-' || d.pubkey)
       OR EXISTS (SELECT 1 FROM access.creators WHERE who = 'n-' || d.pubkey)) THEN
    RETURN access.err(409, 'that key is another account''s');
  END IF;
  UPDATE auth.device_logins SET account = acct, approved_ms = now_, approved_by = session_pk WHERE id = d.id;
  INSERT INTO auth.identities (kind, subject, account, email, linked_ms) VALUES ('nostr', session_pk, acct, NULL, now_) ON CONFLICT DO NOTHING;
  INSERT INTO auth.identities (kind, subject, account, email, linked_ms, label) VALUES ('nostr', d.pubkey, acct, NULL, now_, 'cli:' || d.machine)
    ON CONFLICT (kind, subject) DO NOTHING;
  RETURN access.r(200, jsonb_build_object('account', acct, 'machine', d.machine, 'approved', true), format(E'approved %s\n', access.line(d.machine)));
END $$;

-- POST /api/auth/v2/unlink {"pubkey"}, by a key of the account (a session or a signature): that
-- key is the account's no more, and is its own account again. The account's first key (its id is
-- n-<that key>) stays.
CREATE FUNCTION auth.unlink(caller_pk text, body jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  acct text := auth.account_of(caller_pk);
  target text := lower(coalesce(body->>'pubkey', ''));
BEGIN
  IF acct IS NULL THEN RETURN access.err(401, 'log in first'); END IF;
  IF target !~ '^[0-9a-f]{64}$' THEN RETURN access.err(400, 'name a pubkey (64 hex)'); END IF;
  IF acct = 'n-' || target THEN RETURN access.err(409, 'that is the account''s own key'); END IF;
  DELETE FROM auth.identities WHERE kind = 'nostr' AND subject = target AND account = acct;
  IF NOT FOUND THEN RETURN access.err(404, 'that key isn''t this account''s'); END IF;
  RETURN access.r(200, jsonb_build_object('account', acct, 'unlinked', target), format(E'unlinked %s\n', target));
END $$;

-- GET /api/auth/v2/me with labels: "id|nostr|npub...|cli:MACHINE" for a key a device login linked.
CREATE OR REPLACE FUNCTION auth.me(pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT format(E'account %s\n', auth.account_of(pk)) || coalesce((
    SELECT string_agg(format(E'id|%s|%s%s\n', i.kind, CASE WHEN i.kind = 'nostr' THEN signin.bech32('npub', i.subject) ELSE access.line(coalesce(i.email, '')) END,
                             CASE WHEN i.label IS NOT NULL THEN '|' || access.line(i.label) ELSE '' END), '' ORDER BY i.linked_ms)
    FROM auth.identities i WHERE i.account = auth.account_of(pk)), format(E'id|nostr|%s\n', signin.bech32('npub', pk)))
$$;
