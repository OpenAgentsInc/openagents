-- Signing in to openagents.com with the phone (docs/phone-sign-in.md).
--
-- /signin asks for a challenge: two random tokens, only their SHA-256 kept
-- here. The challenge goes in the page's QR; the poll token stays in the
-- page. The phone approves the challenge with a NIP-98 signed request that
-- names it in the body; the page's next poll then gets a site session
-- (oa_session cookie, 30 days) bound to the phone's pubkey. Each challenge is
-- good once and for five minutes.
--
-- A member links a phone to a private project two ways: LINK A PHONE on the
-- project page (a challenge of kind 'link', bound to the member who asked),
-- or by opening an invite link on the phone (op 'claim', which spends the
-- invite). Either adds the phone's pubkey as another key of that member in
-- projects.member_keys. A member's keys act as the member everywhere in
-- projects.api, and a site session whose key is a member's opens the page.
CREATE SCHEMA signin;
CREATE TABLE signin.challenges (
  challenge_sha256 text PRIMARY KEY,
  poll_sha256 text NOT NULL UNIQUE,
  kind text NOT NULL CHECK (kind IN ('signin', 'link')),
  slug text,                     -- a link's project and the member who asked
  member text,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL,
  approved_pubkey text,
  approved_ms bigint,
  done_ms bigint                 -- the poll that took the result
);
CREATE TABLE signin.sessions (
  token_sha256 text PRIMARY KEY,
  pubkey text NOT NULL,
  created_ms bigint NOT NULL,
  expires_ms bigint NOT NULL
);
CREATE TABLE projects.member_keys (
  slug text NOT NULL,
  pubkey text NOT NULL,          -- the phone's key
  member text NOT NULL,          -- the member's pubkey in the document
  how text NOT NULL CHECK (how IN ('link', 'invite')),
  added_ms bigint NOT NULL,
  PRIMARY KEY (slug, pubkey)
);

-- The member a key speaks for in a project: the key itself when the document
-- lists it, a member's linked key's member, else NULL.
CREATE FUNCTION projects.member_of(want text, pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT CASE WHEN projects.role(want, pk) = 'member' THEN pk
    ELSE (SELECT k.member FROM projects.member_keys k WHERE k.slug = want AND k.pubkey = pk AND projects.role(want, k.member) = 'member') END
$$;
-- A site session's pubkey, or NULL.
CREATE FUNCTION signin.session(token text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT s.pubkey FROM signin.sessions s WHERE s.token_sha256 = projects.sha(coalesce(token, '')) AND s.expires_ms > projects.now_ms()
$$;
-- Adds pk as another key of member in want. NULL when done, else why not.
CREATE FUNCTION projects.add_key(want text, member text, pk text, how text) RETURNS text LANGUAGE plpgsql AS $$
BEGIN
  IF pk !~ '^[0-9a-f]{64}$' THEN RETURN 'not a key'; END IF;
  IF pk = member THEN RETURN NULL; END IF;
  IF projects.role(want, pk) <> '' OR projects.member_of(want, pk) IS DISTINCT FROM NULL AND projects.member_of(want, pk) <> member THEN
    RETURN 'that key already has a place in this project';
  END IF;
  INSERT INTO projects.member_keys VALUES (want, pk, member, how, projects.now_ms())
    ON CONFLICT (slug, pubkey) DO UPDATE SET member = EXCLUDED.member, how = EXCLUDED.how, added_ms = EXCLUDED.added_ms;
  RETURN NULL;
END $$;

-- bech32 (BIP-173) of a 32-byte hex key under hrp, for npub.
CREATE FUNCTION signin.bech32(hrp text, hex text) RETURNS text LANGUAGE plpgsql IMMUTABLE AS $$
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

-- The sign-in routes (private.c, /api/auth/<op>). who: pubkey (a verified
-- NIP-98 signer, or ''), site (the oa_session cookie's token, or '').
--   challenge            a new sign-in challenge: {challenge, poll, expires_in, url} (url: the QR's, ORIGIN/signin#c=CHALLENGE)
--   approve  (signed)    {challenge, kind}: the phone approves; a link adds its key to the member
--   poll                 {poll}: waiting, or signed in (with cookie) or linked; good once
--   me                   the site session's npub and the projects its key belongs to, as lines (K|npub, J|slug)
--   logout               ends the site session
CREATE FUNCTION signin.api(op text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  now_ms bigint := projects.now_ms();
  c signin.challenges;
  t text; p text; why text;
BEGIN
  IF jsonb_typeof(body) IS DISTINCT FROM 'object' THEN body := '{}'; END IF;
  IF op = 'challenge' THEN
    DELETE FROM signin.challenges WHERE expires_ms < now_ms - 3600000;
    t := projects.token(); p := projects.token();
    INSERT INTO signin.challenges (challenge_sha256, poll_sha256, kind, created_ms, expires_ms)
      VALUES (projects.sha(t), projects.sha(p), 'signin', now_ms, now_ms + 300000);
    RETURN projects.r(200, jsonb_build_object('challenge', t, 'poll', p, 'expires_in', 300, 'url', coalesce(who->>'origin', '') || '/signin#c=' || t));
  ELSIF op = 'approve' THEN
    IF pk IS NULL THEN RETURN projects.no(); END IF;
    SELECT * INTO c FROM signin.challenges
      WHERE challenge_sha256 = projects.sha(coalesce(body->>'challenge', '')) AND kind = coalesce(body->>'kind', '')
        AND approved_pubkey IS NULL AND expires_ms > now_ms FOR UPDATE;
    IF c.challenge_sha256 IS NULL THEN RETURN projects.no(); END IF;
    IF c.kind = 'link' THEN
      IF projects.role(c.slug, c.member) <> 'member' THEN RETURN projects.no(); END IF;
      why := projects.add_key(c.slug, c.member, pk, 'link');
      IF why IS NOT NULL THEN RETURN projects.r(400, jsonb_build_object('error', why)); END IF;
    END IF;
    UPDATE signin.challenges SET approved_pubkey = pk, approved_ms = now_ms WHERE challenge_sha256 = c.challenge_sha256;
    IF c.kind = 'link' THEN
      RETURN projects.r(200, jsonb_build_object('ok', true, 'kind', 'link', 'name', projects.name_of(c.slug, c.member)));
    END IF;
    RETURN projects.r(200, '{"ok":true,"kind":"signin"}');
  ELSIF op = 'poll' THEN
    SELECT * INTO c FROM signin.challenges
      WHERE poll_sha256 = projects.sha(coalesce(body->>'poll', '')) AND done_ms IS NULL AND expires_ms > now_ms FOR UPDATE;
    IF c.challenge_sha256 IS NULL THEN RETURN projects.no(); END IF;
    IF c.approved_pubkey IS NULL THEN RETURN projects.r(200, '{"state":"waiting"}'); END IF;
    UPDATE signin.challenges SET done_ms = now_ms WHERE challenge_sha256 = c.challenge_sha256;
    IF c.kind = 'link' THEN RETURN projects.r(200, '{"state":"linked"}'); END IF;
    t := projects.token();
    INSERT INTO signin.sessions VALUES (projects.sha(t), c.approved_pubkey, now_ms, now_ms + 30 * 86400000::bigint);
    RETURN projects.r(200, jsonb_build_object('state', 'signed-in', 'npub', signin.bech32('npub', c.approved_pubkey))) || jsonb_build_object('cookie', t);
  ELSIF op = 'me' THEN
    pk := signin.session(nullif(who->>'site', ''));
    IF pk IS NULL THEN RETURN projects.r(401, '{"error":"not signed in"}'); END IF;
    RETURN projects.r(200, to_jsonb('K|' || signin.bech32('npub', pk) || E'\n' || coalesce((
      SELECT string_agg('J|' || d.slug, E'\n' ORDER BY d.slug) FROM projects.docs d WHERE projects.member_of(d.slug, pk) IS NOT NULL), '') || E'\n'));
  ELSIF op = 'logout' THEN
    DELETE FROM signin.sessions WHERE token_sha256 = projects.sha(coalesce(who->>'site', ''));
    RETURN projects.r(200, '{"ok":true}');
  END IF;
  RETURN projects.no();
END $$;

-- projects.api, with a member's other keys: a signer or site session whose
-- key is linked acts as its member; 'claim' spends an invite to link the
-- signer; 'link' makes a LINK A PHONE challenge for the member asking.
ALTER FUNCTION projects.api(text, text, jsonb, jsonb) RENAME TO api_0013;
CREATE FUNCTION projects.api(op text, want text, body jsonb, who jsonb) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  pk text := nullif(who->>'pubkey', '');
  site text := nullif(who->>'site', '');
  now_ms bigint := projects.now_ms();
  m text; t text; p text; why text;
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
  -- a project cookie comes first; else a site session whose key is this project's
  IF pk IS NULL AND projects.session(want, nullif(who->>'session', '')) IS NULL AND site IS NOT NULL THEN
    pk := projects.member_of(want, signin.session(site));
    IF pk IS NOT NULL THEN who := who || jsonb_build_object('pubkey', pk, 'admin', false); END IF;
  ELSIF pk IS NOT NULL AND projects.role(want, pk) = '' AND projects.member_of(want, pk) IS NOT NULL THEN
    pk := projects.member_of(want, pk);
    who := who || jsonb_build_object('pubkey', pk);
  END IF;
  IF op = 'logout' THEN
    IF site IS NOT NULL THEN DELETE FROM signin.sessions WHERE token_sha256 = projects.sha(site); END IF;
    IF nullif(who->>'session', '') IS NULL THEN RETURN projects.r(200, '{"ok":true}'); END IF;
  END IF;
  IF op = 'link' THEN
    IF pk IS NULL THEN pk := projects.session(want, nullif(who->>'session', '')); END IF;
    IF pk IS NULL OR projects.role(want, pk) = '' THEN RETURN projects.no(); END IF;
    IF projects.role(want, pk) <> 'member' THEN RETURN projects.r(403, '{"error":"members only"}'); END IF;
    t := projects.token(); p := projects.token();
    INSERT INTO signin.challenges (challenge_sha256, poll_sha256, kind, slug, member, created_ms, expires_ms)
      VALUES (projects.sha(t), projects.sha(p), 'link', want, pk, now_ms, now_ms + 300000);
    RETURN projects.r(200, jsonb_build_object('challenge', t, 'poll', p, 'expires_in', 300, 'url', coalesce(who->>'origin', '') || '/signin#link=' || t));
  END IF;
  RETURN projects.api_0013(op, want, body, who);
END $$;
