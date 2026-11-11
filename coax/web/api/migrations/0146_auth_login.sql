-- Logging in with the launch set (decision D2): an email code and a Nostr key (#13204, #13205).
-- auth.c holds the keys and the crypto (sealing, NIP-98); these functions hold the rows.
--
-- Email: one message with a 6-digit code and a link. The pending row keeps SHA-256(code || id) and
-- SHA-256 of the link's token, never either; the email itself only sealed to this row (its id is
-- the associated data). A code lasts 10 minutes and allows 5 tries; asking again voids the old one.
-- Both complete only in the browser that asked (SHA-256 of its __Host-oa_pending cookie), so a
-- phisher who starts a login with your address never receives your session when you click.
--
-- Nostr: a challenge the client signs a NIP-98 event over (POST /api/auth/nostr, body
-- {"challenge"}); each challenge and each event id is good once. The phone QR: the page shows a
-- challenge, the app approves it with a NIP-98 signed request, the page's poll gets the session.

-- ---- email
-- A new pending email login (or link, for acct). Voids the same address's earlier codes.
CREATE FUNCTION auth.email_start(id_ text, lookup_ text, sealed_ text, code_sha text, link_sha text, browser_sha text,
                                 client_ text, purpose_ text, acct text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  DELETE FROM auth.pending WHERE kind = 'email' AND target_lookup = lookup_ AND done_at IS NULL;
  DELETE FROM auth.pending WHERE expires_at < now() - interval '1 day';
  INSERT INTO auth.pending (id, kind, purpose, code_sha256, link_sha256, target_lookup, target_sealed, account, browser_sha256, client, expires_at)
  VALUES (id_, 'email', purpose_, code_sha, link_sha, lookup_, sealed_, acct, browser_sha, client_, now() + interval '10 minutes');
END $$;

-- A typed code, for the browser (or app) that asked. state: 'ok' (id, sealed, lookup, purpose,
-- account to finish with), 'wrong' (tries left), 'dead' (5 tries spent or expired), 'none'.
CREATE FUNCTION auth.email_code(browser_sha text, code text, OUT state text, OUT id text, OUT sealed text, OUT lookup text,
                                OUT purpose text, OUT account text, OUT client text, OUT tries_left int) LANGUAGE plpgsql AS $$
DECLARE p auth.pending;
BEGIN
  SELECT * INTO p FROM auth.pending x WHERE x.kind = 'email' AND x.browser_sha256 = browser_sha AND x.done_at IS NULL
   ORDER BY x.created_at DESC LIMIT 1 FOR UPDATE;
  IF p.id IS NULL THEN state := 'none'; RETURN; END IF;
  IF p.expires_at <= now() OR p.tries >= 5 THEN state := 'dead'; RETURN; END IF;
  IF projects.sha(coalesce(code, '') || p.id) <> p.code_sha256 THEN
    UPDATE auth.pending SET tries = tries + 1 WHERE auth.pending.id = p.id;
    state := CASE WHEN p.tries + 1 >= 5 THEN 'dead' ELSE 'wrong' END;
    tries_left := 5 - (p.tries + 1);
    RETURN;
  END IF;
  UPDATE auth.pending SET done_at = now() WHERE auth.pending.id = p.id;
  state := 'ok'; id := p.id; sealed := p.target_sealed; lookup := p.target_lookup; purpose := p.purpose;
  account := p.account; client := p.client;
END $$;

-- The email's link (its token's SHA-256). In the browser that asked: 'ok' as for a code. Elsewhere:
-- 'elsewhere' and nothing is spent (the page shows the code to type where you asked).
CREATE FUNCTION auth.email_link(link_sha text, browser_sha text, OUT state text, OUT id text, OUT sealed text, OUT lookup text,
                                OUT purpose text, OUT account text, OUT client text) LANGUAGE plpgsql AS $$
DECLARE p auth.pending;
BEGIN
  SELECT * INTO p FROM auth.pending x WHERE x.kind = 'email' AND x.link_sha256 = link_sha FOR UPDATE;
  IF p.id IS NULL OR p.done_at IS NOT NULL THEN state := 'none'; RETURN; END IF;
  IF p.expires_at <= now() OR p.tries >= 5 THEN state := 'dead'; RETURN; END IF;
  IF p.browser_sha256 IS DISTINCT FROM browser_sha THEN state := 'elsewhere'; RETURN; END IF;
  UPDATE auth.pending SET done_at = now() WHERE auth.pending.id = p.id;
  state := 'ok'; id := p.id; sealed := p.target_sealed; lookup := p.target_lookup; purpose := p.purpose;
  account := p.account; client := p.client;
END $$;

-- The account an email logs in to: (acct, email row, made). A new address makes a person's account
-- with the email row's sealed value empty: the caller seals it to the new account and fills it in
-- the same transaction (auth.email_seal). team: the address's SHA-256 is in team.conf.
CREATE FUNCTION auth.email_account(lookup_ text, team_ boolean, OUT acct text, OUT email_id bigint, OUT made boolean) LANGUAGE plpgsql AS $$
BEGIN
  made := false;
  SELECT e.account, e.id INTO acct, email_id FROM auth.emails e WHERE e.lookup = lookup_;
  IF acct IS NOT NULL THEN
    UPDATE auth.emails SET last_used_at = now(), verified_at = coalesce(verified_at, now()) WHERE id = email_id;
    IF team_ THEN UPDATE auth.accounts SET team = true WHERE id = acct; END IF;
    RETURN;
  END IF;
  acct := auth.new_person(NULL);
  UPDATE auth.accounts SET team = team_ WHERE id = acct;
  INSERT INTO auth.emails (account, lookup, sealed, verified_at, is_primary, last_used_at)
  VALUES (acct, lookup_, '', now(), true, now()) RETURNING id INTO email_id;
  made := true;
END $$;
-- Links a verified email to a logged-in account (purpose 'link'). 'ok', 'theirs' (another
-- account has it: log in to that one to merge), or 'already'.
CREATE FUNCTION auth.email_attach(acct text, lookup_ text, OUT state text, OUT email_id bigint) LANGUAGE plpgsql AS $$
DECLARE other text;
BEGIN
  SELECT e.account, e.id INTO other, email_id FROM auth.emails e WHERE e.lookup = lookup_;
  IF other = acct THEN state := 'already'; RETURN; END IF;
  IF other IS NOT NULL THEN state := 'theirs'; email_id := NULL; RETURN; END IF;
  INSERT INTO auth.emails (account, lookup, sealed, verified_at, is_primary)
  VALUES (acct, lookup_, '', now(), NOT EXISTS (SELECT 1 FROM auth.emails WHERE account = acct)) RETURNING id INTO email_id;
  state := 'ok';
END $$;
CREATE FUNCTION auth.email_seal(email_id bigint, sealed_ text) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.emails SET sealed = sealed_ WHERE id = email_id
$$;

-- ---- Nostr
-- A challenge to sign over (5 minutes, once). The caller made it and keeps it.
CREATE FUNCTION auth.nostr_challenge(id_ text, challenge_sha text, client_ text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.pending (id, kind, link_sha256, client, expires_at) VALUES (id_, 'nostr', challenge_sha, client_, now() + interval '5 minutes')
$$;
-- Spends a challenge and the signed event's id together. 'ok', 'replay' (that event was used),
-- 'none' (no such challenge, used, or expired). client: the challenge's.
CREATE FUNCTION auth.nostr_spend(challenge_sha text, event_id text, OUT state text, OUT client text) LANGUAGE plpgsql AS $$
DECLARE p auth.pending;
BEGIN
  DELETE FROM auth.nip98_seen WHERE expires_at < now();
  INSERT INTO auth.nip98_seen (event_id, expires_at) VALUES (event_id, now() + interval '11 minutes') ON CONFLICT DO NOTHING;
  IF NOT FOUND THEN state := 'replay'; RETURN; END IF;
  UPDATE auth.pending x SET done_at = now() WHERE x.kind = 'nostr' AND x.link_sha256 = challenge_sha AND x.done_at IS NULL AND x.expires_at > now()
  RETURNING * INTO p;
  IF p.id IS NULL THEN state := 'none'; RETURN; END IF;
  state := 'ok'; client := p.client;
END $$;
-- Any other NIP-98 event's id, once (the phone's approval, per-request callers on new routes).
CREATE FUNCTION auth.nip98_once(event_id text) RETURNS boolean LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO auth.nip98_seen (event_id, expires_at) VALUES (event_id, now() + interval '11 minutes') ON CONFLICT DO NOTHING;
  RETURN FOUND;
END $$;

-- ---- the phone approves the web
CREATE FUNCTION auth.phone_start(id_ text, challenge_sha text, poll_sha text, browser_sha text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.pending (id, kind, link_sha256, poll_sha256, browser_sha256, expires_at)
  VALUES (id_, 'phone', challenge_sha, poll_sha, browser_sha, now() + interval '5 minutes')
$$;
-- The app, signed by its account's key: 'ok', 'none' (no such challenge, or expired), 'already'.
CREATE FUNCTION auth.phone_approve(challenge_sha text, pk text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE p auth.pending;
BEGIN
  SELECT * INTO p FROM auth.pending x WHERE x.kind = 'phone' AND x.link_sha256 = challenge_sha FOR UPDATE;
  IF p.id IS NULL OR p.expires_at <= now() OR p.done_at IS NOT NULL THEN RETURN 'none'; END IF;
  IF p.approved_pubkey IS NOT NULL THEN RETURN 'already'; END IF;
  UPDATE auth.pending SET approved_pubkey = pk WHERE id = p.id;
  RETURN 'ok';
END $$;
-- The page's poll, from the browser that showed the QR: 'waiting', 'expired', or 'ok' with the
-- approving key (once).
CREATE FUNCTION auth.phone_poll(poll_sha text, browser_sha text, OUT state text, OUT pubkey text) LANGUAGE plpgsql AS $$
DECLARE p auth.pending;
BEGIN
  SELECT * INTO p FROM auth.pending x WHERE x.kind = 'phone' AND x.poll_sha256 = poll_sha AND x.browser_sha256 = browser_sha FOR UPDATE;
  IF p.id IS NULL OR p.done_at IS NOT NULL THEN state := 'none'; RETURN; END IF;
  IF p.approved_pubkey IS NULL THEN state := CASE WHEN p.expires_at <= now() THEN 'expired' ELSE 'waiting' END; RETURN; END IF;
  UPDATE auth.pending SET done_at = now() WHERE id = p.id;
  state := 'ok'; pubkey := p.approved_pubkey;
END $$;

-- ---- the guest's player joins the account once (identity-and-methods.md, Guests)
-- k: the browser's X-Verse-Player key. A guest player (no account, or a g- guest account) and its
-- worlds and saves move to the game account vacct; a player that is another account's stays. The
-- player the page should keep: {key, id, account}, or NULL without a key.
CREATE FUNCTION auth.attach_guest(vacct text, k text) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE
  rec verse.players;
  own text;
BEGIN
  IF coalesce(k, '') !~ '^[A-Za-z0-9_-]{20,128}$' OR vacct IS NULL THEN RETURN NULL; END IF;
  SELECT * INTO rec FROM verse.players WHERE key_sha256 = encode(sha256(convert_to(k, 'UTF8')), 'hex');
  IF rec.id IS NULL THEN RETURN NULL; END IF;
  IF rec.account IS NOT NULL AND rec.account <> vacct AND rec.account NOT LIKE 'g-%' THEN RETURN NULL; END IF;
  own := CASE WHEN rec.account LIKE 'g-%' THEN rec.account END;
  UPDATE verse.players SET account = vacct WHERE id = rec.id OR (own IS NOT NULL AND account = own);
  UPDATE verse.worlds SET owner_account = vacct
   WHERE (owner_player = rec.id AND (owner_account IS NULL OR owner_account = own)) OR (own IS NOT NULL AND owner_account = own);
  IF own IS NOT NULL THEN
    UPDATE verse.progress g SET account = vacct
     WHERE g.account = own AND NOT EXISTS (SELECT 1 FROM verse.progress x WHERE x.account = vacct AND x.slot = g.slot);
    DELETE FROM verse.progress WHERE account = own;
  END IF;
  RETURN jsonb_build_object('key', k, 'id', rec.id, 'account', vacct);
END $$;

-- ---- who is logged in (GET /api/auth/me): the account, its kind and team mark, and its ways in.
-- Emails come back sealed (auth.c opens and masks them); keys as hex.
CREATE FUNCTION auth.me_of(acct text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object('account', a.id, 'kind', a.kind, 'team', a.team,
           'emails', coalesce((SELECT jsonb_agg(jsonb_build_object('id', e.id, 'sealed', e.sealed, 'primary', e.is_primary) ORDER BY e.id)
                               FROM auth.emails e WHERE e.account = a.id), '[]'::jsonb),
           'keys', coalesce((SELECT jsonb_agg(jsonb_build_object('id', k.id, 'pubkey', k.pubkey, 'label', k.label) ORDER BY k.id)
                             FROM auth.nostr_keys k WHERE k.account = a.id), '[]'::jsonb),
           'passkeys', (SELECT count(*) FROM auth.passkeys p WHERE p.account = a.id))
    FROM auth.accounts a WHERE a.id = acct
$$;
