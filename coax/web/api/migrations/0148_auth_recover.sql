-- Step-up, linking and the delayed recovery (docs/auth/identity-and-methods.md, decision D10;
-- #13212, #13213). What may happen is decided in auth-policy.coax and auth-recover-policy.coax;
-- these rows hold the facts, and the constraints below refuse what no code may do:
--
--   * a recovery request is never ready sooner than 72 hours after it was asked (D10). There is
--     no staff override: no function here, and no API route, moves ready_at or skips the wait
--   * an account has at most one open recovery request
--   * every recovery step, link and unlink is in auth.audit (written by auth-recover.c)
--
-- A session is "limited" when it logged in with email alone (L1) and the account has a stronger
-- way in: a Nostr key, a passkey, a device or an unspent recovery code (auth.strong_ways). It may
-- read and play, never change the account, until it steps up with one of those or recovery ends.

-- ---- recovery requests: 72 hours, once at a time, cancellable from any channel
ALTER TABLE auth.recovery_requests
  ADD COLUMN asked_session text,                 -- the session that asked (ses_...)
  ADD COLUMN cancel_sha256 text UNIQUE,          -- the notices' "cancel" link token, as SHA-256
  ADD COLUMN cancelled_via text CHECK (cancelled_via IN ('session', 'link', 'key', 'login', 'freeze')),
  ADD COLUMN done_key bigint,                    -- the key that replaced the lost ways in
  ADD CONSTRAINT recovery_waits_72_hours CHECK (ready_at >= asked_at + interval '72 hours'),
  ADD CONSTRAINT recovery_once CHECK (cancelled_at IS NULL OR done_at IS NULL);
CREATE UNIQUE INDEX recovery_requests_open ON auth.recovery_requests (account) WHERE cancelled_at IS NULL AND done_at IS NULL;
CREATE INDEX recovery_account ON auth.recovery (account) WHERE spent_at IS NULL;

-- Proofs of a key made while logged in (step up, link a key, finish recovery): a challenge for this
-- account, signed once.
ALTER TABLE auth.pending DROP CONSTRAINT pending_purpose_check;
ALTER TABLE auth.pending ADD CONSTRAINT pending_purpose_check CHECK (purpose IN ('login', 'link', 'stepup', 'finish'));

-- Security notices the account was sent (link, unlink, recovery): what the app and the Security
-- page list. No secrets, no addresses. Kept 90 days (auth.notices_sweep).
CREATE TABLE auth.notices (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts (id),
  at timestamptz NOT NULL DEFAULT now(),
  what text NOT NULL CHECK (what ~ '^[a-z][a-z0-9_.]{1,47}$'),
  words text NOT NULL CHECK (length(words) <= 400),
  sent jsonb NOT NULL DEFAULT '{}'      -- {"email": n, "nostr": n, "failed": n}
);
CREATE INDEX notices_account ON auth.notices (account, at);

DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON auth.notices TO %I', r);
      EXECUTE format('GRANT USAGE, SELECT ON SEQUENCE auth.notices_id_seq TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT ON auth.notices TO %I', r);
    END IF;
  END LOOP;
END $$;

-- ---- facts the policies read
-- The stronger ways in: keys, passkeys, devices not revoked, unspent recovery codes.
CREATE FUNCTION auth.strong_ways(acct text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT (SELECT count(*) FROM auth.nostr_keys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.passkeys WHERE account = acct)::int
       + (SELECT count(*) FROM auth.devices WHERE account = acct AND revoked_at IS NULL)::int
       + (SELECT count(*) FROM auth.recovery WHERE account = acct AND spent_at IS NULL)::int
$$;

-- session_look (0143) with the account's stronger ways in, so the gate can tell a limited session.
DROP FUNCTION auth.session_look(text);
CREATE FUNCTION auth.session_look(sha text) RETURNS TABLE (
  id text, account text, kind text, verse_account text, level text, via_kind text, client text,
  created_ms bigint, last_seen_ms bigint, rotated_ms bigint, l2_ms bigint, now_ms bigint,
  current boolean, good_epoch boolean, frozen boolean, team boolean, pubkey text, strong int)
LANGUAGE sql STABLE AS $$
  SELECT s.id, s.account, a.kind, a.verse_account, s.level, s.via_kind, s.client,
         (extract(epoch FROM s.created_at) * 1000)::bigint, (extract(epoch FROM s.last_seen_at) * 1000)::bigint,
         (extract(epoch FROM s.rotated_at) * 1000)::bigint, (extract(epoch FROM s.l2_at) * 1000)::bigint,
         (extract(epoch FROM now()) * 1000)::bigint,
         s.token_sha256 = sha, s.epoch = a.session_epoch, a.frozen_at IS NOT NULL, a.team,
         coalesce((SELECT k.pubkey FROM auth.nostr_keys k WHERE s.via_kind = 'nostr' AND k.id = s.via_id),
                  (SELECT k.pubkey FROM auth.nostr_keys k WHERE k.account = a.id ORDER BY k.last_used_at DESC NULLS LAST, k.id DESC LIMIT 1)),
         auth.strong_ways(a.id)
    FROM auth.sessions s JOIN auth.accounts a ON a.id = s.account
   WHERE (s.token_sha256 = sha OR (s.prev_sha256 = sha AND s.prev_until > now())) AND a.deleted_at IS NULL
$$;

-- What a step-up prompt offers: the kinds of stronger ways in this account has.
CREATE FUNCTION auth.methods_of(acct text) RETURNS text[] LANGUAGE sql STABLE AS $$
  SELECT array_remove(ARRAY[
    CASE WHEN EXISTS (SELECT 1 FROM auth.nostr_keys WHERE account = acct) THEN 'nostr' END,
    CASE WHEN EXISTS (SELECT 1 FROM auth.passkeys WHERE account = acct) THEN 'passkey' END,
    CASE WHEN EXISTS (SELECT 1 FROM auth.devices WHERE account = acct AND revoked_at IS NULL) THEN 'device' END,
    CASE WHEN EXISTS (SELECT 1 FROM auth.recovery WHERE account = acct AND spent_at IS NULL) THEN 'recovery_code' END], NULL)
$$;

-- A session proved L2 now (a key's signature or a recovery code): its level and clock, and the way.
CREATE FUNCTION auth.session_raise(sid text, kind_ text, via bigint) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.sessions SET level = 'L2', l2_at = now(), via_kind = kind_, via_id = via WHERE id = sid RETURNING 1)
  SELECT EXISTS (SELECT 1 FROM u)
$$;

-- ---- proofs made while logged in: a challenge for one account and purpose, signed once
CREATE FUNCTION auth.proof_challenge(id_ text, challenge_sha text, acct text, purpose_ text) RETURNS void LANGUAGE sql AS $$
  INSERT INTO auth.pending (id, kind, purpose, link_sha256, account, expires_at)
  VALUES (id_, 'nostr', purpose_, challenge_sha, acct, now() + interval '5 minutes')
$$;
-- 'ok', 'replay' (the event was used), or 'none' (no such challenge for this account and purpose,
-- or it ended).
CREATE FUNCTION auth.proof_spend(challenge_sha text, acct text, purpose_ text, event_id text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE pid text;
BEGIN
  SELECT p.id INTO pid FROM auth.pending p
   WHERE p.link_sha256 = challenge_sha AND p.account = acct AND p.purpose = purpose_ AND p.kind = 'nostr'
     AND p.done_at IS NULL AND p.expires_at > now() FOR UPDATE;
  IF pid IS NULL THEN RETURN 'none'; END IF;
  IF NOT auth.nip98_once(event_id) THEN RETURN 'replay'; END IF;
  UPDATE auth.pending SET done_at = now() WHERE id = pid;
  RETURN 'ok';
END $$;

-- ---- linking and unlinking
-- Who holds a key: 0 nobody, 1 this account, 2 another.
CREATE FUNCTION auth.key_owner(pk text, acct text) RETURNS int LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT CASE WHEN account = acct THEN 1 ELSE 2 END FROM auth.nostr_keys WHERE pubkey = pk), 0)
$$;
CREATE FUNCTION auth.key_link(acct text, pk text, label_ text) RETURNS bigint LANGUAGE sql AS $$
  INSERT INTO auth.nostr_keys (account, pubkey, label, last_used_at) VALUES (acct, pk, nullif(left(label_, 80), ''), now()) RETURNING id
$$;
-- An account's ways in, each with what the Security page shows (never an address: emails sealed).
CREATE FUNCTION auth.ways_of(acct text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT jsonb_build_object(
    'emails', coalesce((SELECT jsonb_agg(jsonb_build_object('id', e.id, 'sealed', e.sealed,
                          'added_ms', (extract(epoch FROM e.created_at) * 1000)::bigint) ORDER BY e.id) FROM auth.emails e WHERE e.account = acct), '[]'),
    'keys', coalesce((SELECT jsonb_agg(jsonb_build_object('id', k.id, 'pubkey', k.pubkey, 'label', coalesce(k.label, ''),
                          'added_ms', (extract(epoch FROM k.created_at) * 1000)::bigint) ORDER BY k.id) FROM auth.nostr_keys k WHERE k.account = acct), '[]'),
    'passkeys', (SELECT count(*) FROM auth.passkeys WHERE account = acct),
    'recovery_code', EXISTS (SELECT 1 FROM auth.recovery WHERE account = acct AND spent_at IS NULL),
    'ways', auth.login_ways(acct), 'strong', auth.strong_ways(acct))
$$;
-- Removes one way in (kind email, nostr or passkey; id its row) and ends every session it made.
-- How many sessions ended; NULL when there is no such row on the account. The last way in is
-- refused by keep_a_way_in (0142): "Add another way to log in first".
CREATE FUNCTION auth.unlink(acct text, kind_ text, id_ bigint) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ = 'email' THEN DELETE FROM auth.emails WHERE id = id_ AND account = acct;
  ELSIF kind_ = 'nostr' THEN DELETE FROM auth.nostr_keys WHERE id = id_ AND account = acct;
  ELSIF kind_ = 'passkey' THEN DELETE FROM auth.passkeys WHERE id = id_ AND account = acct;
  ELSE RETURN NULL; END IF;
  IF NOT FOUND THEN RETURN NULL; END IF;
  DELETE FROM auth.sessions WHERE account = acct AND via_kind = kind_ AND via_id = id_;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

-- ---- recovery codes: scrypt(words, salt, N = 2^17, r = 8, p = 1), single use
-- A new code; any unspent one is spent (the newest is the only one that works).
CREATE FUNCTION auth.code_new(acct text, hash bytea, salt_ bytea) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE id_ bigint;
BEGIN
  INSERT INTO auth.recovery (account, code_scrypt, salt) VALUES (acct, hash, salt_) RETURNING id INTO id_;
  UPDATE auth.recovery SET spent_at = now() WHERE account = acct AND spent_at IS NULL AND id <> id_;
  RETURN id_;
END $$;
CREATE FUNCTION auth.code_spend(id_ bigint) RETURNS boolean LANGUAGE sql AS $$
  WITH u AS (UPDATE auth.recovery SET spent_at = now() WHERE id = id_ AND spent_at IS NULL RETURNING 1) SELECT EXISTS (SELECT 1 FROM u)
$$;
-- The account an email (any of its lookups) or a key names, for a recovery code login.
CREATE FUNCTION auth.account_named(lookups text[], pk text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce((SELECT e.account FROM auth.emails e WHERE e.lookup = ANY(lookups) LIMIT 1),
                  (SELECT k.account FROM auth.nostr_keys k WHERE k.pubkey = pk))
$$;

-- ---- the delayed recovery
-- The newest request's times (milliseconds; 0 for not yet); no row when there was none.
CREATE FUNCTION auth.recovery_of(acct text) RETURNS TABLE (id bigint, asked_ms bigint, ready_ms bigint, cancelled_ms bigint, done_ms bigint, now_ms bigint, cancelled_via text)
LANGUAGE sql STABLE AS $$
  SELECT r.id, (extract(epoch FROM r.asked_at) * 1000)::bigint, (extract(epoch FROM r.ready_at) * 1000)::bigint,
         coalesce((extract(epoch FROM r.cancelled_at) * 1000)::bigint, 0), coalesce((extract(epoch FROM r.done_at) * 1000)::bigint, 0),
         (extract(epoch FROM now()) * 1000)::bigint, r.cancelled_via
    FROM auth.recovery_requests r WHERE r.account = acct ORDER BY r.id DESC LIMIT 1
$$;
-- Asks: ready 72 hours from now. The table's check refuses anything sooner.
CREATE FUNCTION auth.recovery_ask(acct text, sid text, cancel_sha text, OUT id bigint, OUT asked_ms bigint, OUT ready_ms bigint) LANGUAGE plpgsql AS $$
BEGIN
  INSERT INTO auth.recovery_requests AS r (account, asked_at, ready_at, asked_session, cancel_sha256)
  VALUES (acct, now(), now() + interval '72 hours', sid, cancel_sha)
  RETURNING r.id, (extract(epoch FROM r.asked_at) * 1000)::bigint, (extract(epoch FROM r.ready_at) * 1000)::bigint INTO id, asked_ms, ready_ms;
END $$;
-- Cancels the account's open request (any channel). How many: 0 or 1.
CREATE FUNCTION auth.recovery_cancel(acct text, via text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.recovery_requests SET cancelled_at = now(), cancelled_via = via
   WHERE account = acct AND cancelled_at IS NULL AND done_at IS NULL;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;
-- The notices' cancel link: the account whose open request it cancelled, or NULL.
CREATE FUNCTION auth.recovery_cancel_by_link(sha text) RETURNS text LANGUAGE plpgsql AS $$
DECLARE acct text;
BEGIN
  UPDATE auth.recovery_requests SET cancelled_at = now(), cancelled_via = 'link'
   WHERE cancel_sha256 = sha AND cancelled_at IS NULL AND done_at IS NULL RETURNING account INTO acct;
  RETURN acct;
END $$;
-- Finishes a ready request: the new key replaces every lost one. The account's other keys and
-- passkeys go, its devices are revoked, unspent recovery codes are spent, every other session
-- ends, and this session becomes L2 by the new key. Checks the wait again here (never sooner than
-- ready_at). The new key's row id; raises when the request isn't ready or the key is another's.
CREATE FUNCTION auth.recovery_finish(acct text, sid text, pk text, label_ text) RETURNS bigint LANGUAGE plpgsql AS $$
DECLARE rid bigint; kid bigint;
BEGIN
  SELECT r.id INTO rid FROM auth.recovery_requests r
   WHERE r.account = acct AND r.cancelled_at IS NULL AND r.done_at IS NULL AND r.ready_at <= now()
     AND r.asked_at <= now() - interval '72 hours' FOR UPDATE;
  IF rid IS NULL THEN RAISE EXCEPTION 'auth: recovery is not ready' USING ERRCODE = 'check_violation'; END IF;
  INSERT INTO auth.nostr_keys (account, pubkey, label, last_used_at) VALUES (acct, pk, nullif(left(label_, 80), ''), now()) RETURNING id INTO kid;
  DELETE FROM auth.nostr_keys WHERE account = acct AND id <> kid;
  DELETE FROM auth.passkeys WHERE account = acct;
  UPDATE auth.devices SET revoked_at = now() WHERE account = acct AND revoked_at IS NULL;
  UPDATE auth.recovery SET spent_at = now() WHERE account = acct AND spent_at IS NULL;
  DELETE FROM auth.sessions WHERE account = acct AND id <> sid;
  UPDATE auth.sessions SET level = 'L2', l2_at = now(), via_kind = 'nostr', via_id = kid WHERE id = sid;
  UPDATE auth.recovery_requests SET done_at = now(), done_key = kid WHERE id = rid;
  RETURN kid;
END $$;

-- Freeze (0143) also cancels a pending recovery: "This wasn't me" from any notice stops it.
CREATE OR REPLACE FUNCTION auth.freeze(acct text) RETURNS int LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  UPDATE auth.accounts SET frozen_at = coalesce(frozen_at, now()) WHERE id = acct;
  UPDATE auth.recovery_requests SET cancelled_at = now(), cancelled_via = 'freeze'
   WHERE account = acct AND cancelled_at IS NULL AND done_at IS NULL;
  n := auth.sessions_end_all(acct);
  RETURN n;
END $$;

-- ---- notices
CREATE FUNCTION auth.notice_add(acct text, what_ text, words_ text) RETURNS bigint LANGUAGE sql AS $$
  INSERT INTO auth.notices (account, what, words) VALUES (acct, what_, left(words_, 400)) RETURNING id
$$;
CREATE FUNCTION auth.notice_sent(id_ bigint, sent_ jsonb) RETURNS void LANGUAGE sql AS $$
  UPDATE auth.notices SET sent = sent_ WHERE id = id_
$$;
CREATE FUNCTION auth.notices_of(acct text) RETURNS jsonb LANGUAGE sql STABLE AS $$
  SELECT coalesce(jsonb_agg(jsonb_build_object('at_ms', (extract(epoch FROM n.at) * 1000)::bigint, 'what', n.what, 'words', n.words, 'sent', n.sent)
                            ORDER BY n.id DESC), '[]'::jsonb)
    FROM (SELECT * FROM auth.notices WHERE account = acct ORDER BY id DESC LIMIT 50) n
$$;
CREATE FUNCTION auth.notices_sweep() RETURNS int LANGUAGE sql AS $$
  WITH d AS (DELETE FROM auth.notices WHERE at < now() - interval '90 days' RETURNING 1) SELECT count(*)::int FROM d
$$;
