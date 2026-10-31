-- Realm making behind invites, and the groundwork for accounts that link a Nostr
-- key and a WorkOS identity (docs/auth/2026-10-31-realm-invites-and-auth.md).
--
-- Inert when applied: new schemas, tables and functions only. No existing row,
-- column, constraint or function changes. Nothing reads these until a flag row
-- says so: access.flags has no rows, and a missing row is off. Off, every
-- route answers as before (access.c asks access.flag first and stops there).
--
--   realm_invites  realm creation (plan, pick, plan/doc, MAKE IT, FORK) only for
--                  approved creators and site admins; everyone else gets 403
--                  "invite only". Invites, redeeming and the waitlist answer.
--   auth_v2        /api/auth/v2/*: linking another Nostr key to an account, the
--                  WorkOS AuthKit start and callback (no WorkOS call is built).
--
-- A creator is a "who": an account id (n-<pubkey hex>, e-<email hash>, w-<hash>)
-- or a player id (p-..., only for guests grandfathered for realms they made).
-- An invite code is 32 hex, shown once; only its SHA-256 is kept.
CREATE SCHEMA access;
CREATE SCHEMA auth;

CREATE TABLE access.flags (
  name text PRIMARY KEY CHECK (name IN ('realm_invites', 'auth_v2')),
  on_ boolean NOT NULL,
  set_by text NOT NULL,
  set_ms bigint NOT NULL
);
CREATE TABLE access.creators (
  who text PRIMARY KEY,
  how text NOT NULL CHECK (how IN ('invite', 'request', 'admin', 'grandfather')),
  ref bigint,                    -- the invite or request it came by
  by text,                       -- the admin who approved it, or NULL
  at_ms bigint NOT NULL
);
CREATE TABLE access.invites (
  id bigserial PRIMARY KEY,
  code_sha256 text NOT NULL UNIQUE,
  uses int NOT NULL CHECK (uses BETWEEN 1 AND 1000),
  used int NOT NULL DEFAULT 0,
  note text NOT NULL DEFAULT '',
  by text NOT NULL,              -- the admin's pubkey
  made_ms bigint NOT NULL,
  expires_ms bigint NOT NULL,
  revoked_ms bigint,
  CHECK (used BETWEEN 0 AND uses)
);
CREATE TABLE access.redemptions (
  invite bigint NOT NULL REFERENCES access.invites (id),
  who text NOT NULL,
  at_ms bigint NOT NULL,
  PRIMARY KEY (invite, who)
);
CREATE TABLE access.requests (
  id bigserial PRIMARY KEY,
  who text NOT NULL,             -- the account asking
  pubkey text NOT NULL,          -- the key it logged in with
  note text NOT NULL,
  contact text NOT NULL DEFAULT '',
  state text NOT NULL DEFAULT 'waiting' CHECK (state IN ('waiting', 'approved', 'declined')),
  at_ms bigint NOT NULL,
  decided_ms bigint,
  decided_by text
);
CREATE UNIQUE INDEX requests_one_waiting ON access.requests (who) WHERE state = 'waiting';

-- auth_v2: an account's identities. A Nostr key with no row is its own account,
-- n-<pubkey>, exactly as today; a row moves the key under another account.
CREATE TABLE auth.identities (
  kind text NOT NULL CHECK (kind IN ('nostr', 'workos')),
  subject text NOT NULL,         -- a pubkey (hex), or the WorkOS user id
  account text NOT NULL,
  email text,                    -- WorkOS's verified email, never on a public route
  linked_ms bigint NOT NULL,
  PRIMARY KEY (kind, subject)
);
-- The AuthKit round trip's state: minted at start, spent once at the callback.
CREATE TABLE auth.workos_states (
  state_sha256 text PRIMARY KEY,
  session_pubkey text,           -- logged in already: the WorkOS identity links to this account
  made_ms bigint NOT NULL,
  expires_ms bigint NOT NULL,
  used_ms bigint
);
-- Sessions for an account (a WorkOS-only account has no pubkey for signin.sessions).
CREATE TABLE auth.sessions (
  token_sha256 text PRIMARY KEY,
  account text NOT NULL,
  via text NOT NULL CHECK (via IN ('workos', 'nostr')),
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL
);

CREATE FUNCTION access.flag(want text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT on_ FROM access.flags WHERE name = want), false)
$$;
CREATE FUNCTION access.line(v text) RETURNS text LANGUAGE sql IMMUTABLE AS $$
  SELECT regexp_replace(coalesce(v, ''), '[|\x00-\x1f\x7f]+', ' ', 'g')
$$;
CREATE FUNCTION access.r(status int, body jsonb, lines text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT jsonb_build_object('status', status, 'body', body, 'lines', lines)
$$;
CREATE FUNCTION access.err(status int, why text) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT access.r(status, jsonb_build_object('error', why), why)
$$;

-- The account a pubkey speaks for: a linked identity's, else its own.
CREATE FUNCTION auth.account_of(pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN coalesce(pk, '') !~ '^[0-9a-f]{64}$' THEN NULL
    ELSE coalesce((SELECT account FROM auth.identities WHERE kind = 'nostr' AND subject = pk), 'n-' || pk) END
$$;

-- Whether this requester may make a realm: the player key's id, its account, or the logged-in
-- key's account is a creator. Admins are decided in C (OA_ADMIN_PUBKEYS) and passed in.
CREATE FUNCTION access.is_creator(k text, pk text) RETURNS boolean LANGUAGE sql STABLE AS $$
  SELECT EXISTS (SELECT 1 FROM access.creators c WHERE c.who IN (
    verse.player_of(k),
    (SELECT p.account FROM verse.players p WHERE p.id = verse.player_of(k)),
    auth.account_of(pk)))
$$;
-- 200 may make; 403 invite only. Off: always 200.
CREATE FUNCTION access.gate(k text, pk text, admin boolean) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN NOT access.flag('realm_invites') OR admin OR access.is_creator(k, pk) THEN 200 ELSE 403 END
$$;

-- GET /api/playtest/access: what a page shows.
CREATE FUNCTION access.state(k text, pk text, admin boolean) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT format(E'invites %s\nmake %s\nin %s\ncreator %s\nasked %s\n',
    CASE WHEN access.flag('realm_invites') THEN 1 ELSE 0 END,
    CASE WHEN access.gate(k, pk, admin) = 200 THEN 1 ELSE 0 END,
    CASE WHEN auth.account_of(pk) IS NULL THEN 0 ELSE 1 END,
    CASE WHEN admin OR access.is_creator(k, pk) THEN 1 ELSE 0 END,
    coalesce((SELECT r.state FROM access.requests r WHERE r.who = auth.account_of(pk) ORDER BY r.id DESC LIMIT 1), 'none'))
$$;

CREATE FUNCTION access.approve(who_ text, how_ text, ref_ bigint, by_ text) RETURNS boolean LANGUAGE sql AS $$
  WITH x AS (INSERT INTO access.creators (who, how, ref, by, at_ms) VALUES (who_, how_, ref_, by_, projects.now_ms())
             ON CONFLICT (who) DO NOTHING RETURNING 1)
  SELECT EXISTS (SELECT 1 FROM x)
$$;

-- POST /api/playtest/access/redeem {code}: a logged-in key's account becomes a creator. One use is
-- spent in the same statement that checks the code; an account that is already a creator spends none.
-- Unknown, used up, expired and revoked codes get the same answer.
CREATE FUNCTION access.redeem(body jsonb, pk text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  acct text := auth.account_of(pk);
  code text := CASE WHEN jsonb_typeof(body->'code') = 'string' THEN lower(btrim(body->>'code')) ELSE '' END;
  inv bigint;
BEGIN
  IF acct IS NULL THEN RETURN access.err(401, 'log in first'); END IF;
  IF code !~ '^[0-9a-f]{32}$' THEN RETURN access.err(404, 'that code isn''t good'); END IF;
  IF EXISTS (SELECT 1 FROM access.creators WHERE who = acct) THEN
    RETURN access.r(200, '{"creator": true}', E'creator 1\n');
  END IF;
  UPDATE access.invites SET used = used + 1
    WHERE code_sha256 = projects.sha(code) AND revoked_ms IS NULL AND expires_ms > projects.now_ms() AND used < uses
    RETURNING id INTO inv;
  IF inv IS NULL THEN RETURN access.err(404, 'that code isn''t good'); END IF;
  INSERT INTO access.redemptions VALUES (inv, acct, projects.now_ms());
  PERFORM access.approve(acct, 'invite', inv, NULL);
  RETURN access.r(200, '{"creator": true}', E'creator 1\n');
END $$;

-- POST /api/playtest/access/request {note, contact?}: the waitlist. One waiting request an account;
-- asking again rewrites it. Its note is 1 to 280 characters; contact, when given, an email's shape.
CREATE FUNCTION access.ask(body jsonb, pk text, cap int) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  acct text := auth.account_of(pk);
  note_ text := verse.clean_text(body->'note', 280);
  contact_ text := lower(verse.clean_text(body->'contact', 200));
BEGIN
  IF acct IS NULL THEN RETURN access.err(401, 'log in first'); END IF;
  IF EXISTS (SELECT 1 FROM access.creators WHERE who = acct) THEN RETURN access.r(200, '{"creator": true}', E'creator 1\n'); END IF;
  IF note_ = '' THEN RETURN access.err(400, 'say what you''d make'); END IF;
  IF contact_ <> '' AND contact_ !~ '^[^@\s]+@[^@\s]+\.[^@\s]+$' THEN RETURN access.err(400, 'that email doesn''t look right'); END IF;
  UPDATE access.requests SET note = note_, contact = CASE WHEN body ? 'contact' THEN contact_ ELSE contact END, pubkey = pk, at_ms = projects.now_ms()
    WHERE who = acct AND state = 'waiting';
  IF NOT FOUND THEN
    IF (SELECT count(*) FROM access.requests WHERE state = 'waiting') >= cap THEN RETURN access.err(429, 'slow down'); END IF;
    INSERT INTO access.requests (who, pubkey, note, contact, at_ms) VALUES (acct, pk, note_, contact_, projects.now_ms());
  END IF;
  RETURN access.r(200, '{"state": "waiting"}', E'asked waiting\n');
END $$;

-- ---- the admins' part (access.c checks admin_who first)
CREATE FUNCTION access.admin_lines() RETURNS text LANGUAGE sql STABLE AS $$
  SELECT string_agg(l, E'\n' ORDER BY o, k) || E'\n' FROM (
    SELECT 0 o, 0::bigint k, format('F|%s|%s', n, CASE WHEN access.flag(n) THEN 1 ELSE 0 END) l FROM unnest(ARRAY['realm_invites', 'auth_v2']) n
    UNION ALL
    SELECT 1, -i.id, format('I|%s|%s|%s|%s|%s|%s', i.id, i.uses, i.used, i.expires_ms,
      CASE WHEN i.revoked_ms IS NOT NULL THEN 'revoked' WHEN i.used >= i.uses THEN 'used' WHEN i.expires_ms <= projects.now_ms() THEN 'expired' ELSE 'live' END,
      access.line(i.note)) FROM access.invites i
    UNION ALL
    SELECT 2, CASE WHEN r.state = 'waiting' THEN r.id ELSE 1000000000000 - r.id END,
      format('R|%s|%s|%s|%s|%s|%s', r.id, signin.bech32('npub', r.pubkey), access.line(r.note), access.line(r.contact), r.state, r.at_ms) FROM access.requests r
    UNION ALL
    SELECT 3, -c.at_ms, format('C|%s|%s|%s', access.line(c.who), c.how, c.at_ms) FROM access.creators c
  ) x
$$;

CREATE FUNCTION access.set_flag(body jsonb, by_ text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE n text := body->>'name';
BEGIN
  IF n IS NULL OR n NOT IN ('realm_invites', 'auth_v2') OR jsonb_typeof(body->'on') <> 'boolean' THEN RETURN access.err(400, 'name and on'); END IF;
  INSERT INTO access.flags VALUES (n, (body->>'on')::boolean, by_, projects.now_ms())
    ON CONFLICT (name) DO UPDATE SET on_ = excluded.on_, set_by = excluded.set_by, set_ms = excluded.set_ms;
  RETURN access.r(200, jsonb_build_object('name', n, 'on', (body->>'on')::boolean), NULL);
END $$;

-- An invite: 1 to 1000 uses (1), good for 1 to 90 days (14). The code is in this answer only.
CREATE FUNCTION access.mint(body jsonb, by_ text, site text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  uses_ int := 1; days int := 14;
  code text := left(projects.token(), 32);
  id_ bigint;
BEGIN
  IF body ? 'uses' THEN
    IF jsonb_typeof(body->'uses') <> 'number' OR (body->>'uses')::numeric NOT BETWEEN 1 AND 1000 OR (body->>'uses')::numeric % 1 <> 0 THEN RETURN access.err(400, 'uses: 1 to 1000'); END IF;
    uses_ := (body->>'uses')::int;
  END IF;
  IF body ? 'days' THEN
    IF jsonb_typeof(body->'days') <> 'number' OR (body->>'days')::numeric NOT BETWEEN 1 AND 90 OR (body->>'days')::numeric % 1 <> 0 THEN RETURN access.err(400, 'days: 1 to 90'); END IF;
    days := (body->>'days')::int;
  END IF;
  INSERT INTO access.invites (code_sha256, uses, note, by, made_ms, expires_ms)
    VALUES (projects.sha(code), uses_, verse.clean_text(body->'note', 120), by_, projects.now_ms(), projects.now_ms() + days * 86400000::bigint)
    RETURNING id INTO id_;
  RETURN access.r(200, jsonb_build_object('id', id_, 'code', code, 'uses', uses_, 'days', days, 'link', site || '/invite#code=' || code), NULL);
END $$;

CREATE FUNCTION access.revoke(id_ bigint) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  UPDATE access.invites SET revoked_ms = coalesce(revoked_ms, projects.now_ms()) WHERE id = id_;
  IF NOT FOUND THEN RETURN access.err(404, 'no such invite'); END IF;
  RETURN access.r(200, jsonb_build_object('id', id_, 'state', 'revoked'), NULL);
END $$;

CREATE FUNCTION access.decide(id_ bigint, yes boolean, by_ text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE acct text;
BEGIN
  SELECT who INTO acct FROM access.requests WHERE id = id_;
  IF acct IS NULL THEN RETURN access.err(404, 'no such request'); END IF;
  UPDATE access.requests SET state = CASE WHEN yes THEN 'approved' ELSE 'declined' END, decided_ms = projects.now_ms(), decided_by = by_
    WHERE id = id_ AND state = 'waiting';
  IF NOT FOUND THEN RETURN access.err(409, 'already decided'); END IF;
  IF yes THEN PERFORM access.approve(acct, 'request', id_, by_); END IF;
  RETURN access.r(200, jsonb_build_object('id', id_, 'state', CASE WHEN yes THEN 'approved' ELSE 'declined' END), NULL);
END $$;

-- An admin names a key (hex) or an account id outright.
CREATE FUNCTION access.add(body jsonb, by_ text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE w text := lower(coalesce(body->>'who', ''));
BEGIN
  IF w ~ '^[0-9a-f]{64}$' THEN w := auth.account_of(w);
  ELSIF w !~ '^[enw]-[0-9a-f]{16,64}$' THEN RETURN access.err(400, 'a pubkey (hex) or an account id'); END IF;
  PERFORM access.approve(w, 'admin', NULL, by_);
  RETURN access.r(200, jsonb_build_object('who', w, 'creator', true), NULL);
END $$;
CREATE FUNCTION access.remove(w text) RETURNS jsonb LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM access.creators WHERE who = w;
  IF NOT FOUND THEN RETURN access.err(404, 'not a creator'); END IF;
  RETURN access.r(200, jsonb_build_object('who', w, 'creator', false), NULL);
END $$;

-- Everyone who owns a made realm (its account, else the guest player) becomes a creator.
CREATE FUNCTION access.grandfather(by_ text) RETURNS jsonb LANGUAGE sql AS $$
  WITH owners AS (
    SELECT DISTINCT coalesce(w.owner_account, p.account, w.owner_player) AS who
    FROM verse.made_realms m JOIN verse.worlds w ON w.id = m.world LEFT JOIN verse.players p ON p.id = w.owner_player
  ), x AS (
    INSERT INTO access.creators (who, how, by, at_ms) SELECT who, 'grandfather', by_, projects.now_ms() FROM owners WHERE who IS NOT NULL
    ON CONFLICT (who) DO NOTHING RETURNING 1
  )
  SELECT access.r(200, jsonb_build_object('approved', (SELECT count(*) FROM x)), NULL)
$$;

-- ---- auth_v2 groundwork

-- GET /api/auth/v2/me: the account and its identities, as lines.
CREATE FUNCTION auth.me(pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT format(E'account %s\n', auth.account_of(pk)) || coalesce((
    SELECT string_agg(format(E'id|%s|%s\n', i.kind, CASE WHEN i.kind = 'nostr' THEN signin.bech32('npub', i.subject) ELSE access.line(coalesce(i.email, '')) END), '' ORDER BY i.linked_ms)
    FROM auth.identities i WHERE i.account = auth.account_of(pk)), format(E'id|nostr|%s\n', signin.bech32('npub', pk)))
$$;

-- POST /api/auth/v2/link/nostr: the logged-in key (session) takes another key (its NIP-98
-- signature) into its account. A key that is already another account's, or that has realms or
-- players of its own, is refused: merging accounts is not built.
CREATE FUNCTION auth.link_nostr(session_pk text, new_pk text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE acct text := auth.account_of(session_pk);
BEGIN
  IF acct IS NULL THEN RETURN access.err(401, 'log in first'); END IF;
  IF coalesce(new_pk, '') !~ '^[0-9a-f]{64}$' THEN RETURN access.err(401, 'not signed'); END IF;
  IF auth.account_of(new_pk) = acct THEN RETURN access.r(200, jsonb_build_object('account', acct, 'linked', true), NULL); END IF;
  IF EXISTS (SELECT 1 FROM auth.identities WHERE kind = 'nostr' AND subject = new_pk)
     OR EXISTS (SELECT 1 FROM verse.players WHERE account = 'n-' || new_pk)
     OR EXISTS (SELECT 1 FROM verse.worlds WHERE owner_account = 'n-' || new_pk)
     OR EXISTS (SELECT 1 FROM access.creators WHERE who = 'n-' || new_pk) THEN
    RETURN access.err(409, 'that key is another account''s');
  END IF;
  INSERT INTO auth.identities VALUES ('nostr', session_pk, acct, NULL, projects.now_ms()) ON CONFLICT DO NOTHING;
  INSERT INTO auth.identities VALUES ('nostr', new_pk, acct, NULL, projects.now_ms());
  RETURN access.r(200, jsonb_build_object('account', acct, 'linked', true), NULL);
END $$;

-- The AuthKit start: a state for the round trip (ten minutes, once). The token is the answer's only.
CREATE FUNCTION auth.workos_state(session_pk text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE t text := projects.token();
BEGIN
  INSERT INTO auth.workos_states VALUES (projects.sha(t), CASE WHEN session_pk ~ '^[0-9a-f]{64}$' THEN session_pk END, projects.now_ms(), projects.now_ms() + 600000);
  RETURN t;
END $$;
-- The callback's state, spent: 200 {session_pubkey}, else 400.
CREATE FUNCTION auth.workos_spend(t text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE pk text; found_ boolean;
BEGIN
  UPDATE auth.workos_states SET used_ms = projects.now_ms()
    WHERE state_sha256 = projects.sha(coalesce(t, '')) AND used_ms IS NULL AND expires_ms > projects.now_ms()
    RETURNING session_pubkey, true INTO pk, found_;
  IF NOT coalesce(found_, false) THEN RETURN access.err(400, 'that log in expired; try again'); END IF;
  RETURN access.r(200, jsonb_build_object('session_pubkey', pk), NULL);
END $$;
-- What the callback does with WorkOS's verified profile {id, email} once the code exchange is built:
-- logged in already (session_pk), the identity joins that account; a known identity logs into its
-- account; else a new account w-<hash of the WorkOS id>. Answers {account, token}: a new auth.sessions row.
CREATE FUNCTION auth.workos_link(profile jsonb, session_pk text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  sub text := profile->>'id';
  mail text := lower(nullif(btrim(coalesce(profile->>'email', '')), ''));
  acct text;
  t text := projects.token();
BEGIN
  IF coalesce(sub, '') !~ '^[A-Za-z0-9_]{4,64}$' THEN RETURN access.err(400, 'not a WorkOS user'); END IF;
  SELECT account INTO acct FROM auth.identities WHERE kind = 'workos' AND subject = sub;
  IF acct IS NOT NULL AND session_pk IS NOT NULL AND acct <> auth.account_of(session_pk) THEN
    RETURN access.err(409, 'that login is another account''s');
  END IF;
  IF acct IS NULL THEN
    acct := coalesce(auth.account_of(session_pk), 'w-' || left(projects.sha(sub), 32));
    INSERT INTO auth.identities VALUES ('workos', sub, acct, mail, projects.now_ms());
  ELSE
    UPDATE auth.identities SET email = coalesce(mail, email) WHERE kind = 'workos' AND subject = sub;
  END IF;
  INSERT INTO auth.sessions VALUES (projects.sha(t), acct, 'workos', projects.now_ms(), projects.now_ms() + 30 * 86400000::bigint);
  RETURN access.r(200, jsonb_build_object('account', acct, 'token', t), NULL);
END $$;
CREATE FUNCTION auth.session(t text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.account FROM auth.sessions s WHERE s.token_sha256 = projects.sha(coalesce(t, '')) AND s.expires_ms > projects.now_ms()
$$;
